use base64::{engine::general_purpose::URL_SAFE as base64_enc_dec, Engine as _};
use js_sys::{ArrayBuffer, Function, Uint8Array};
use std::{cell::RefCell, collections::HashMap};
use tokio::sync::oneshot;
use wasm_bindgen::prelude::*;
use web_sys::{BinaryType, Blob, Event, FileReaderSync, MessageEvent, MessageEventInit, WebSocket as BrowserWebSocket};

use layer8_primitives::{
    crypto::{generate_key_pair, jwk_from_map, KeyUse},
    types::{Layer8Envelope, WebSocketMetadata, WebSocketPayload},
};

use crate::{
    js::{rebuild_url, ENCRYPTED_TUNNEL_FLAG, PUB_JWK_ECDH, UP_JWT, USER_SYMMETRIC_KEY},
    js_imports_prelude::*,
};

thread_local! {
    // This static variable will help us keep track of our websocket wrapper.
    static LAYER8_SOCKETS: RefCell<HashMap<String, WasmWebSocket>> = RefCell::new(HashMap::new());
}

/// The configuration object for the WebSocket.
#[wasm_bindgen(getter_with_clone)]
#[derive(Debug, Default)]
pub struct InitConfig {
    pub url: String,
    pub proxy: String,
    pub protocols: Option<Vec<String>>,
}

impl InitConfig {
    fn new(obj: js_sys::Object) -> Result<Self, JsError> {
        let mut init_config = InitConfig::default();
        let entries = object_entries(&obj);
        for entry in entries.iter() {
            let val = js_sys::Array::from(&entry); // [key, value] result from Object.entries
            match val.get(0).as_string().ok_or(JsError::new("expected object key to be a string"))?.as_str() {
                "url" => {
                    init_config.url = val
                        .get(1)
                        .as_string()
                        .ok_or(JsError::new("expected `InitConfig.url` value to be a string"))?;
                }

                "proxy" => {
                    init_config.proxy = val
                        .get(1)
                        .as_string()
                        .ok_or(JsError::new("expected `InitConfig.proxy` value to be a string"))?;
                }

                "protocols" => {
                    if val.get(1).is_instance_of::<js_sys::Array>() {
                        let protocols = js_sys::Array::from(&val.get(1));
                        let mut protocol_list = Vec::new();
                        for protocol in protocols.iter() {
                            protocol_list.push(
                                protocol
                                    .as_string()
                                    .ok_or(JsError::new("expected `InitConfig.protocols` value to be a string"))?,
                            );
                        }
                        init_config.protocols = Some(protocol_list);
                    } else if val.get(1).is_string() {
                        let protocols = val
                            .get(1)
                            .as_string()
                            .ok_or(JsError::new("expected `InitConfig.protocols` value to be a string"))?;

                        init_config.protocols = Some(vec![protocols]);
                    } else {
                        return Err(JsError::new("expected `InitConfig.protocols` value to be a string or an array"));
                    }
                }

                _ => {
                    // we rather pipe the issues now than have them silently ignored
                    return Err(JsError::new(&format!(
                        "unexpected key in `InitConfig`: {}",
                        val.get(0).as_string().expect_throw("expected object key to be a string")
                    )));
                }
            }
        }

        Ok(init_config)
    }
}

// The WebSocket input-output stream using the browser's WebSocket API.
#[derive(Debug)]
struct WasmWebSocket {
    // This is the actual WebSocket object.
    socket: BrowserWebSocket,
}

/// This is a reference to the WebSocket object.
/// The implementation does not support SharedArrayBuffers.
#[wasm_bindgen(js_name = L8WebSocket)]
#[derive(Debug)]
struct WasmWebSocketRef(String);

impl Drop for WasmWebSocket {
    fn drop(&mut self) {
        console_log!("Dropping WebSocketRef");
        LAYER8_SOCKETS.with_borrow_mut(|val| {
            val.remove(&rebuild_url(&self.socket.url()));
        });
    }
}

impl WasmWebSocket {
    async fn init_(options: js_sys::Object) -> Result<WasmWebSocketRef, JsValue> {
        let options = InitConfig::new(options)?;

        // if already present, return the existing socket ref
        if let Some(val) = LAYER8_SOCKETS.with_borrow(|val| {
            if val.get(&rebuild_url(&options.url)).is_some() {
                Some(rebuild_url(&options.url))
            } else {
                None
            }
        }) {
            return Ok(WasmWebSocketRef(val));
        }

        let (private_jwk_ecdh, pub_jwk_ecdh) = generate_key_pair(KeyUse::Ecdh)?;
        PUB_JWK_ECDH.with(|v| {
            v.set(Some(private_jwk_ecdh.clone()));
        });

        let b64_pub_jwk = pub_jwk_ecdh.export_as_base64();
        let proxy = format!("{}/init-tunnel?backend={}", options.proxy, options.url);

        console_log!(&format!("Connecting to proxy: {}", proxy));

        let socket = BrowserWebSocket::new(&proxy).map_err(|_e| {
            console_log!(&format!("Failed to connect to proxy: {:?}", _e));
            JsValue::from_str("Failed to connect to proxy")
        })?;

        // waiting for the connection to be ready
        {
            let (tx, rx) = oneshot::channel();
            let closure = Closure::once(move |_event: Event| {
                console_log!("Connected to proxy");
                tx.send(())
                    .expect_throw("Failed to send ready state; this is a bug in the code, please report it to the developers")
            });

            socket.set_onopen(Some(closure.as_ref().unchecked_ref()));

            rx.await.map_err(|_e| {
                console_log!("Failed to connect to proxy");
                JsValue::from_str("Failed to connect to proxy")
            })?;

            console_log!("Connected to proxy");
            socket.set_onopen(None);
        }

        // let's make the initECDH handshake first
        let resp_bytes = {
            let (tx, rx) = oneshot::channel();
            let closure = Closure::once(move |event: MessageEvent| {
                let mut resp_ = Vec::new();
                let data = Uint8Array::new(&event.data());
                resp_.extend(data.to_vec());
                tx.send(resp_).unwrap();
            });

            console_log!("Setting onmessage");
            socket.set_onmessage(Some(closure.as_ref().unchecked_ref()));

            // sending the public key
            socket
                .send_with_str(&b64_pub_jwk)
                .map(|v| {
                    console_log!(&format!("Sent public key: {}", b64_pub_jwk));
                    v
                })
                .map_err(|_e| {
                    console_log!(&format!("Failed to send public key: {:?}", _e));
                    JsValue::from_str("Failed to send public key")
                })?;

            console_log!("Waiting for response");

            // this will be a blocking operation; we need to wait for the response
            rx.await
                .map(|v| {
                    console_log_("Received response");
                    socket.set_onmessage(None); // reset the onmessage callback
                    v
                })
                .map_err(|e| JsValue::from_str(&e.to_string()))?
        };

        console_log!("Decoding response");

        let mut proxy_data = {
            let envelope = Layer8Envelope::from_json_bytes(&resp_bytes).map_err(|_e| {
                console_log!(&format!(
                    "Failed to decode response: {}, Data is :{}",
                    _e,
                    String::from_utf8_lossy(resp_bytes.as_ref())
                ));

                JsValue::from_str("Failed to decode response")
            })?;

            match envelope {
                Layer8Envelope::Raw(raw) => serde_json::from_slice::<serde_json::Map<String, serde_json::Value>>(&raw).map_err(|_e| {
                    console_log!(&format!(
                        "Failed to decode response: {}, Data is :{}",
                        _e,
                        String::from_utf8_lossy(resp_bytes.as_ref())
                    ));

                    JsValue::from_str("Failed to decode response")
                })?,
                _ => {
                    return Err(JsValue::from_str("Expected raw response"));
                }
            }
        };

        UP_JWT.set(proxy_data.remove("up-JWT").ok_or("up_jwt not found")?.as_str().unwrap().to_string());

        let shared_key = private_jwk_ecdh.get_ecdh_shared_secret(&jwk_from_map(proxy_data)?)?;
        USER_SYMMETRIC_KEY.set(Some(shared_key.clone()));
        ENCRYPTED_TUNNEL_FLAG.set(true);

        LAYER8_SOCKETS.with_borrow_mut(|val| {
            val.insert(rebuild_url(&options.url), WasmWebSocket { socket });
        });

        Ok(WasmWebSocketRef(rebuild_url(&options.url)))
    }
}

// This block implements the browser APIs for the WebAssembly interop.
#[wasm_bindgen(js_class = L8WebSocket)]
impl WasmWebSocketRef {
    #[allow(dead_code, reason = "This is for API compatibility with the browser's WebSocket API.")]
    #[wasm_bindgen(getter)]
    /// Getter for the `url` field of this object.
    pub fn url(&self) -> String {
        self.0.clone()
    }

    #[allow(dead_code, reason = "This is for API compatibility with the browser's WebSocket API.")]
    #[wasm_bindgen(getter)]
    /// Getter for the `readyState` field of this object.
    pub fn ready_state(&self) -> u16 {
        LAYER8_SOCKETS
            .with_borrow(|val| val.get(&self.0).map(|val| val.socket.ready_state()))
            .unwrap_or_default()
    }

    #[allow(dead_code, reason = "This is for API compatibility with the browser's WebSocket API.")]
    #[wasm_bindgen(getter)]
    /// Getter for the `bufferedAmount` field of this object.
    pub fn buffered_amount(&self) -> u32 {
        LAYER8_SOCKETS
            .with_borrow(|val| val.get(&self.0).map(|val| val.socket.buffered_amount()))
            .unwrap_or_default()
    }

    #[allow(dead_code, reason = "This is for API compatibility with the browser's WebSocket API.")]
    #[wasm_bindgen(getter)]
    /// Getter for the `onopen` field of this object.
    pub fn onopen(&self) -> Option<Function> {
        LAYER8_SOCKETS
            .with_borrow(|val| val.get(&self.0).map(|val| val.socket.onopen()))
            .unwrap_or_default()
    }

    #[allow(dead_code, reason = "This is for API compatibility with the browser's WebSocket API.")]
    #[wasm_bindgen(js_name = onopen, setter)]
    /// Setter for the `onopen` field of this object.
    pub fn set_onopen(&self, value: Option<Function>) {
        LAYER8_SOCKETS.with_borrow_mut(|val| {
            if let Some(stream) = val.get_mut(&self.0) {
                stream.socket.set_onopen(value.as_ref());
            }
        });
    }

    #[allow(dead_code, reason = "This is for API compatibility with the browser's WebSocket API.")]
    #[wasm_bindgen(getter)]
    /// Getter for the `onerror` field of this object.
    pub fn onerror(&self) -> Option<Function> {
        LAYER8_SOCKETS
            .with_borrow(|val| val.get(&self.0).map(|val| val.socket.onerror()))
            .unwrap_or_default()
    }

    #[allow(dead_code, reason = "This is for API compatibility with the browser's WebSocket API.")]
    #[wasm_bindgen(js_name = onerror, setter)]
    /// Setter for the `onerror` field of this object.
    pub fn set_onerror(&self, value: Option<Function>) {
        LAYER8_SOCKETS.with_borrow_mut(|val| {
            if let Some(stream) = val.get_mut(&self.0) {
                stream.socket.set_onerror(value.as_ref());
            }
        });
    }

    #[allow(dead_code, reason = "This is for API compatibility with the browser's WebSocket API.")]
    #[wasm_bindgen(getter)]
    /// Getter for the `onclose` field of this object.
    pub fn onclose(&self) -> Option<Function> {
        LAYER8_SOCKETS
            .with_borrow(|val| val.get(&self.0).map(|val| val.socket.onclose()))
            .unwrap_or_default()
    }

    #[allow(dead_code, reason = "This is for API compatibility with the browser's WebSocket API.")]
    #[wasm_bindgen(js_name = onclose, setter)]
    /// Setter for the `onclose` field of this object.
    pub fn set_onclose(&self, value: Option<Function>) {
        LAYER8_SOCKETS.with_borrow_mut(|val| {
            if let Some(stream) = val.get_mut(&self.0) {
                stream.socket.set_onclose(value.as_ref());
            }
        });
    }

    #[allow(dead_code, reason = "This is for API compatibility with the browser's WebSocket API.")]
    #[wasm_bindgen(getter)]
    /// Getter for the `extensions` field of this object.
    pub fn extensions(&self) -> String {
        LAYER8_SOCKETS
            .with_borrow(|val| val.get(&self.0).map(|val| val.socket.extensions()))
            .unwrap_or_default()
    }

    #[allow(dead_code, reason = "This is for API compatibility with the browser's WebSocket API.")]
    #[wasm_bindgen(getter)]
    /// Getter for the `protocol` field of this object.
    pub fn protocol(&self) -> String {
        LAYER8_SOCKETS
            .with_borrow(|val| val.get(&self.0).map(|val| val.socket.protocol()))
            .unwrap_or_default()
    }

    #[allow(dead_code, reason = "This is for API compatibility with the browser's WebSocket API.")]
    #[wasm_bindgen(getter)]
    /// Getter for the `binaryType` field of this object.
    pub fn onmessage(&self) -> Option<Function> {
        LAYER8_SOCKETS
            .with_borrow(|val| val.get(&self.0).map(|val| val.socket.onmessage()))
            .unwrap_or_default()
    }

    #[allow(dead_code, reason = "This is for API compatibility with the browser's WebSocket API.")]
    #[wasm_bindgen(js_name = onmessage, setter)]
    /// Setter for the `binaryType` field of this object.
    pub fn set_onmessage(&self, value: Option<Function>) {
        // self.on_receive(value);
        LAYER8_SOCKETS.with_borrow_mut(|val| {
            if let Some(stream) = val.get_mut(&self.0) {
                // we need yo overwrite the on
                stream.socket.set_onmessage(Some(&preprocess_on_message(value)));
            }
        });
    }

    #[allow(dead_code, reason = "This is for API compatibility with the browser's WebSocket API.")]
    #[wasm_bindgen(js_name = addEventListener)]
    /// Sets an event listener on the WebSocket object.
    /// It intercepts the `message` event and plugs in our custom logic to decrypt the message.
    pub fn add_event_listener(&self, type_: &str, listener: Option<Function>) {
        LAYER8_SOCKETS.with_borrow_mut(|val| {
            let stream = val.get_mut(&self.0).expect_throw(&format!("Socket with url {} not found", self.0));
            if type_.eq_ignore_ascii_case("message") {
                stream.socket.set_onmessage(Some(&preprocess_on_message(listener)));
                return;
            }

            if let Some(listener) = &listener {
                stream
                    .socket
                    .add_event_listener_with_callback(type_, listener)
                    .expect_throw(&format!("Failed to add event listener for type: {}", type_))
            }
        });
    }

    #[allow(dead_code, reason = "This is for API compatibility with the browser's WebSocket API.")]
    #[wasm_bindgen(js_name = binaryType, getter)]
    /// Getter for the `binaryType` field of this object.
    pub fn binary_type(&self) -> BinaryType {
        LAYER8_SOCKETS
            .with_borrow(|val| val.get(&self.0).map(|val| val.socket.binary_type()))
            .unwrap_or(BinaryType::Arraybuffer)
    }

    #[allow(dead_code, reason = "This is for API compatibility with the browser's WebSocket API.")]
    #[wasm_bindgen(js_name = binaryType, setter)]
    /// Setter for the `binaryType` field of this object.
    /// This is a no-op since we layer8 dictates the binary type.
    pub fn set_binary_type(&self, _: BinaryType) {}

    /// Constructor for the `WebSocket` object.
    #[allow(dead_code, reason = "This is for API compatibility with the browser's WebSocket API.")]
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        WasmWebSocketRef("".to_string())
    }

    /// The options object is expected to have the following structure:
    /// ```js
    /// export interface InitConfig {
    ///     // The URL of the service provider.
    ///     url: string;
    ///     // The Layer8 proxy URL to connect to.
    ///     proxy: string;
    ///     // The protocols to use for the ws connection.
    ///     protocols?: string | string[] | undefined;
    /// }
    /// ```
    #[allow(dead_code)]
    pub async fn init(&mut self, options: js_sys::Object) -> Result<(), JsValue> {
        *self = WasmWebSocket::init_(options).await?;
        Ok(())
    }

    /// close the connection
    #[allow(dead_code, reason = "This is for API compatibility with the browser's WebSocket API.")]
    pub fn close(&self, code: Option<u16>, reason: Option<String>) -> Result<(), JsValue> {
        LAYER8_SOCKETS.with_borrow_mut(|val| {
            if let Some(val) = val.get_mut(&self.0) {
                match (code, reason) {
                    (Some(code), Some(reason)) => val.socket.close_with_code_and_reason(code, &reason),
                    (Some(code), None) => val.socket.close_with_code(code),
                    _ => val.socket.close(),
                }
            } else {
                Err(JsValue::from_str("Socket not found"))
            }
        })
    }

    #[allow(dead_code, reason = "This is for API compatibility with the browser's WebSocket API.")]
    pub fn send(&self, data: JsValue) -> Result<(), JsValue> {
        console_log!(&format!("Sending data: {:?}", data));
        let reader = FileReaderSync::new()?;

        let symmetric_key = match USER_SYMMETRIC_KEY.with_borrow(|v| v.clone()) {
            Some(v) => v,
            None => return Err(JsValue::from_str("Symmetric key not found")),
        };

        let metadata = serde_json::to_vec(&WebSocketMetadata {
            backend_url: self.0.to_string(),
        })
        .map_err(|e| JsValue::from_str(&format!("{}", e)))?;

        let mut ws_exchange = WebSocketPayload {
            payload: "".to_string(),
            metadata,
        };

        // Types checked: string, Blob, ArrayBuffer, Uint8Array
        if data.is_string() {
            let encrypted = symmetric_key.symmetric_encrypt(data.as_string().unwrap().as_bytes())?;
            ws_exchange.payload = base64_enc_dec.encode(&encrypted);
        } else if data.is_instance_of::<Blob>() {
            let data = {
                let array = reader.read_as_array_buffer(&data.dyn_into::<Blob>().expect("check already done; qed"))?;
                Uint8Array::new(&array).to_vec()
            };

            let encrypted = symmetric_key.symmetric_encrypt(&data)?;
            ws_exchange.payload = base64_enc_dec.encode(&encrypted);
        } else if data.is_instance_of::<ArrayBuffer>() {
            let data = Uint8Array::new(&data.dyn_into::<ArrayBuffer>().expect("check already done; qed")).to_vec();

            let encrypted = symmetric_key.symmetric_encrypt(&data)?;
            ws_exchange.payload = base64_enc_dec.encode(&encrypted);
        } else if data.is_instance_of::<Uint8Array>() {
            let data = data.dyn_into::<Uint8Array>().expect("check already done; qed").to_vec();
            let encrypted = symmetric_key.symmetric_encrypt(&data)?;
            ws_exchange.payload = base64_enc_dec.encode(&encrypted);
        } else {
            return Err(JsValue::from_str("Unsupported data type"));
        }

        LAYER8_SOCKETS.with_borrow_mut(|v| {
            let ws = v.get_mut(&rebuild_url(self.0.as_str())).ok_or("Socket not found")?;
            let data = serde_json::to_vec(&ws_exchange).map_err(|e| e.to_string())?;
            ws.socket.send_with_u8_array(&data)
        })
    }
}

// this block decrypts the incoming message before passing it to the client.
fn preprocess_on_message(pipeline: Option<Function>) -> Function {
    let decrypt_callback = Closure::wrap(Box::new(move |message: MessageEvent| {
        let symmetric_key = match USER_SYMMETRIC_KEY.with_borrow(|v| v.clone()) {
            Some(v) => v,
            None => {
                console_log!("Symmetric key not found");
                return;
            }
        };

        let data: JsValue = {
            let payload = serde_json::from_slice::<WebSocketPayload>(&Uint8Array::new(&message.data()).to_vec())
                .expect_throw("Failed to parse WebSocketPayload")
                .payload;

            let slice = symmetric_key
                .symmetric_decrypt(&base64_enc_dec.decode(&payload).expect_throw("Failed to decode base64 payload"))
                .unwrap();

            Uint8Array::from(slice.as_slice()).into()
        };

        let msg_event = {
            let msg_init = MessageEventInit::new();
            msg_init.set_data(&data);
            MessageEvent::new_with_event_init_dict("message", &msg_init).expect_throw("Failed to create MessageEventInit")
        };

        pipeline.as_ref().map(|pipeline| pipeline.call1(&JsValue::NULL, &msg_event).unwrap());
    }) as Box<dyn FnMut(MessageEvent)>);

    decrypt_callback.into_js_value().dyn_into().unwrap()
}

// TODO: map API 1:1 from socket.io
pub mod socket_io {}
