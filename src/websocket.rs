use std::{cell::Cell, collections::HashMap, net::TcpStream};

use js_sys::{ArrayBuffer, Function};
use wasm_bindgen::prelude::*;
use web_sys::{BinaryType, Blob, FileReaderSync};

use layer8_primitives::crypto::{generate_key_pair, jwk_from_map, KeyUse};
use layer8_tungstenite::{connect, stream::MaybeTlsStream, Bytes, Message, Utf8Bytes, WebSocket as Layer8WebSocket};

use crate::js::{rebuild_url, ENCRYPTED_TUNNEL_FLAG, PUB_JWK_ECDH, UP_JWT, USER_SYMMETRIC_KEY};
use crate::js_imports_prelude::*;

thread_local! {
    // This static variable will help us keep track of the websocket streamer. Also we use it here since we can't export generic implementations
    // to the JS class.
    static LAYER8_SOCKETS: Cell<HashMap<String, Layer8WebSocket<MaybeTlsStream<TcpStream>>>> = Cell::new(HashMap::new());
}

/// A websocket client. This is an indirection over the `WebSocket` API: <https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.WebSocket.html>.
/// The indirection serves to to maintain a consistent API for the client, regardless of the underlying implementation.
///
/// This client first initiates the handshake with the proxy and provider for the ECDH key exchange. After that is done, we are be able to send and receive messages.
/// It is import to note that this client is expected to be long lived.
///
/// Warning: This API is not final and may change in the future.
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct WebSocket {
    url: String,
    onopen: JsValue,
    onerror: JsValue,
    onclose: JsValue,
    onmessage: JsValue,
}

impl Drop for WebSocket {
    fn drop(&mut self) {
        LAYER8_SOCKETS.with(|v| {
            let mut val = v.take();
            val.remove(self.url.as_str());
            v.set(val);
        });
    }
}

impl Default for WebSocket {
    fn default() -> Self {
        WebSocket {
            url: String::new(),
            onopen: JsValue::NULL,
            onerror: JsValue::NULL,
            onclose: JsValue::NULL,
            onmessage: JsValue::NULL,
        }
    }
}

#[wasm_bindgen]
impl WebSocket {
    #[wasm_bindgen(constructor)] // change to JsError
    pub async fn new(url: &str, proxy: &str) -> Result<Self, JsValue> {
        // if already present, return the existing socket
        let val = LAYER8_SOCKETS.with(|v| {
            let val = v.take();
            let url_ = rebuild_url(url);
            if val.contains_key(&url_) {
                v.set(val);
                Some(WebSocket {
                    url: url_.to_string(),
                    ..Default::default()
                })
            } else {
                v.set(val);
                None
            }
        });

        if let Some(val) = val {
            return Ok(val);
        }

        let (private_jwk_ecdh, pub_jwk_ecdh) = generate_key_pair(KeyUse::Ecdh)?;
        PUB_JWK_ECDH.with(|v| {
            v.set(Some(private_jwk_ecdh.clone()));
        });

        let b64_pub_jwk = pub_jwk_ecdh.export_as_base64();
        let proxy = format!("{proxy}/init-tunnel?backend={url}");

        let (mut socket, _) = connect(proxy).expect("Can't connect to port");

        socket
            .send(Message::Text(Utf8Bytes::from(b64_pub_jwk)))
            .map_err(|e| JsValue::from_str(&format!("{}", e)))?;

        // we expect the response to be a binary message
        let resp = socket.read().map_err(|e| JsValue::from_str(&format!("{}", e)))?;

        let resp_bytes = match resp {
            Message::Binary(data) => data,
            _ => {
                ENCRYPTED_TUNNEL_FLAG.set(false);
                return Err(JsValue::from_str("The response from the proxy is not binary."));
            }
        };

        let mut proxy_data: serde_json::Map<String, serde_json::Value> = serde_json::from_slice(&resp_bytes.as_ref()).map_err(|val| {
            console_log!(&format!(
                "Failed to decode response: {}, Data is :{}",
                val,
                String::from_utf8_lossy(resp_bytes.as_ref())
            ));

            JsValue::from_str("Failed to decode response")
        })?;

        UP_JWT.set(
            proxy_data
                .remove("up-JWT")
                .ok_or("up_jwt not found")?
                .as_str()
                .unwrap() // infalliable
                .to_string(),
        );

        let shared_key = private_jwk_ecdh.get_ecdh_shared_secret(&jwk_from_map(proxy_data)?)?;
        socket.set_shared_secret(shared_key.clone());
        USER_SYMMETRIC_KEY.set(Some(shared_key));
        ENCRYPTED_TUNNEL_FLAG.set(true);

        LAYER8_SOCKETS.with(|v| {
            let mut val = v.take();
            val.insert(rebuild_url(url), socket);
            v.set(val);
        });

        Ok(WebSocket {
            url: url.to_string(),
            ..Default::default()
        })
    }

    #[wasm_bindgen(getter)]
    pub fn url(&self) -> String {
        self.url.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn ready_state(&self) -> u16 {
        return 1; //todo
    }

    #[wasm_bindgen(getter)]
    pub fn buffered_amount(&self) -> u32 {
        return 0; // todo
    }

    #[wasm_bindgen(getter)]
    pub fn onopen(&self) -> Option<Function> {
        self.onopen.clone()
    }

    #[wasm_bindgen(setter)]
    pub fn set_onopen(&mut self, value: Option<Function>) {
        self.onopen = value;
    }

    #[wasm_bindgen(getter)]
    pub fn onerror(&self) -> Option<Function> {
        self.onerror.clone()
    }

    #[wasm_bindgen(setter)]
    pub fn set_onerror(&mut self, value: Option<Function>) {
        self.onerror = value;
    }

    #[wasm_bindgen(getter)]
    pub fn onclose(&self) -> Option<Function> {
        self.onclose.clone()
    }

    #[wasm_bindgen(setter)]
    pub fn set_onclose(&mut self, value: Option<Function>) {
        self.onclose = value;
    }

    #[wasm_bindgen(getter)]
    pub fn extensions(&self) -> String {
        return String::new(); // todo
    }

    #[wasm_bindgen(getter)]
    pub fn protocol(&self) -> String {
        return String::new(); // todo
    }

    #[wasm_bindgen(getter)]
    pub fn onmessage(&self) -> Option<Function> {
        self.onmessage.clone()
    }

    #[wasm_bindgen(setter)]
    pub fn set_onmessage(&mut self, value: Option<Function>) {
        self.onmessage = value;
    }

    #[wasm_bindgen(getter)]
    pub fn binary_type(&self) -> BinaryType {
        BinaryType::Arraybuffer // TODO
    }

    #[wasm_bindgen(setter)]
    pub fn set_binary_type(&self, _value: BinaryType) {
        // TODO
    }

    pub fn close(&self) -> Result<(), JsValue> {
        // self.0.close()
        Ok(())
    }

    pub fn send(&self, data: JsValue) -> Result<(), JsValue> {
        console_log!(format!("Sending data: {:?}", data));
        let socket = LAYER8_SOCKETS.with(|v| {
            let val = v.take();
            let val = val.get_mut(self.url.as_str()).ok_or("Socket not found").map_err(|e| {
                v.set(val);
                JsValue::from_str(e)
            });
            val
        })?;

        let reader = FileReaderSync::new()?;

        if data.is_string() {
            let msg = Message::Text(Utf8Bytes::from(data.as_string().unwrap()));
            socket.send(msg).map_err(|e| JsValue::from_str(&format!("{}", e)))?;

            LAYER8_SOCKETS.with(|v| {
                let mut val = v.take();
                val.insert(self.url.clone(), *socket);
                v.set(val);
            });
            Ok(())
        } else if data.is_instance_of::<Blob>() {
            let val = reader
                .read_as_binary_string(&data.dyn_into::<Blob>().expect("check already done; qed"))?
                .as_bytes();

            socket
                .send(Message::Binary(Bytes::from(val)))
                .map_err(|e| JsValue::from_str(&format!("{}", e)))?;

            Ok(())
        // } else if data.is_instance_of::<ArrayBuffer>() {
        //     let data = ArrayBuffer::from(data);

        //     let data = data.dyn_into::<ArrayBuffer>().expect("check already done; qed")?;

        //     let val = reader.read_as_binary_string()?;
        //     self.0.send_with_array_buffer(data.unchecked_ref())
        // } else if data.is_object() {
        //     self.0.send_with_array_buffer_view(&Object::from(data.clone()))
        // } else if data.is_instance_of::<Uint8Array>() {
        //     self.0
        //         .send_with_u8_array(&data.clone().dyn_into::<Uint8Array>().expect("check already done; qed").to_vec())
        } else {
            Err(JsValue::from_str("Unsupported data type"))
        }
    }
}

// TODO: map API 1:1 from socket.io
pub mod socket_io {}
