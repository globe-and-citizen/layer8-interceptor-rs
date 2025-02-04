use base64::{engine::general_purpose::URL_SAFE as base64_enc_dec, Engine as _};
use js_sys::{ArrayBuffer, Function, Uint8Array};
use std::{cell::RefCell, collections::HashMap};
use wasm_bindgen::prelude::*;
use web_sys::{BinaryType, Blob, FileReaderSync, MessageEvent, MessageEventInit, WebSocket as BrowserWebSocket};

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
pub struct InitConfig {
    pub url: String,
    pub proxy: String,
    pub protocols: Option<Vec<String>>,
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
        LAYER8_SOCKETS.with_borrow_mut(|val| {
            val.remove(&rebuild_url(&self.socket.url()));
        });
    }
}

impl WasmWebSocket {
    async fn init(options: InitConfig) -> Result<WasmWebSocketRef, JsValue> {
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

        let socket = BrowserWebSocket::new(&proxy).map_err(|e| {
            console_log!(&format!("Failed to connect to proxy: {:?}", e));
            JsValue::from_str("Failed to connect to proxy")
        })?;

        console_log!(&format!("Connected to proxy: {}", proxy));

        socket.send_with_str(&b64_pub_jwk).map_err(|e| {
            console_log!(&format!("Failed to send public key: {:?}", e));
            JsValue::from_str("Failed to send public key")
        })?;

        // we expect the response to be a binary message
        let resp_bytes = {
            let (tx, rx) = std::sync::mpsc::channel();

            let closure = {
                let tx = tx.clone();
                let closure = Closure::once(move |data: MessageEvent| {
                    let mut resp_ = Vec::new();
                    let data = Uint8Array::new(&data.data());
                    resp_.extend(data.to_vec());
                    tx.send(resp_).unwrap();
                });

                closure.into_js_value()
            };

            socket.set_onmessage(Some(&Function::from(closure)));

            // this will be a blocking operation; we need to wait for the response
            rx.recv().map_err(|e| JsValue::from_str(&format!("{}", e)))?
        };

        let mut proxy_data = {
            let envelope = Layer8Envelope::from_json_bytes(&resp_bytes).map_err(|e| {
                console_log!(&format!(
                    "Failed to decode response: {}, Data is :{}",
                    e,
                    String::from_utf8_lossy(resp_bytes.as_ref())
                ));

                JsValue::from_str("Failed to decode response")
            })?;

            match envelope {
                Layer8Envelope::Raw(raw) => serde_json::from_slice::<serde_json::Map<String, serde_json::Value>>(&raw).map_err(|e| {
                    console_log!(&format!(
                        "Failed to decode response: {}, Data is :{}",
                        e,
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

    #[allow(dead_code, reason = "This is for API compatibility with the browser's WebSocket API.")]
    /// Constructor for the `WebSocket` object.
    #[wasm_bindgen(constructor)]
    pub async fn new(options: InitConfig) -> Result<Self, JsValue> {
        if options.url.is_empty() {
            return Err(JsValue::from_str("url is required."));
        }

        if options.proxy.is_empty() {
            return Err(JsValue::from_str("proxy_url is required."));
        }

        WasmWebSocket::init(options).await
    }

    #[allow(dead_code, reason = "This is for API compatibility with the browser's WebSocket API.")]
    /// close the connection
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
