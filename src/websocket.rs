use std::{cell::Cell, io::Cursor};

use js_sys::{ArrayBuffer, Function, Object, Uint8Array};
use wasm_bindgen::prelude::*;
use web_sys::{BinaryType, Blob, WebSocket};

use layer8_tungstenite::layer8_streamer::Layer8Streamer;

thread_local! {
    // This static variable will help us keep track of the websocket streamer. Also we use it here since we can't export generic implementations
    // to the JS class.
    static WS_STREAM: Cell<Layer8Streamer<Cursor<Vec<u8>>>> = Cell::new(Layer8Streamer::new(Cursor::new(Vec::new()), None));
}

/// A websocket client. This is an indirection over the `WebSocket` API: <https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.WebSocket.html>.
/// The indirection serves to to maintain a consistent API for the client, regardless of the underlying implementation.
///
/// This client first initiates the handshake with the proxy and provider for the ECDH key exchange. After that is done, we are be able to send and receive messages.
/// It is import to note that this client is expected to be long lived.
///
/// TODO: work on overridden methods @Osoro
#[wasm_bindgen]
pub struct Layer8WebsocketClient(WebSocket);

#[wasm_bindgen]
impl Layer8WebsocketClient {
    #[wasm_bindgen(constructor)]
    pub fn new(url: &str) -> Result<Self, JsValue> {
        Ok(Layer8WebsocketClient(WebSocket::new(url)?))
    }

    #[wasm_bindgen(getter)]
    pub fn url(&self) -> String {
        self.0.url()
    }

    #[wasm_bindgen(getter)]
    pub fn ready_state(&self) -> u16 {
        self.0.ready_state()
    }

    #[wasm_bindgen(getter)]
    pub fn buffered_amount(&self) -> u32 {
        self.0.buffered_amount()
    }

    #[wasm_bindgen(getter)]
    pub fn onopen(&self) -> Option<Function> {
        self.0.onopen()
    }

    #[wasm_bindgen(setter)]
    pub fn set_onopen(&self, value: Option<Function>) {
        self.0.set_onopen(value.as_ref());
    }

    #[wasm_bindgen(getter)]
    pub fn onerror(&self) -> Option<Function> {
        self.0.onerror()
    }

    #[wasm_bindgen(setter)]
    pub fn set_onerror(&self, value: Option<Function>) {
        self.0.set_onerror(value.as_ref());
    }

    #[wasm_bindgen(getter)]
    pub fn onclose(&self) -> Option<Function> {
        self.0.onclose()
    }

    #[wasm_bindgen(setter)]
    pub fn set_onclose(&self, value: Option<Function>) {
        self.0.set_onclose(value.as_ref());
    }

    #[wasm_bindgen(getter)]
    pub fn extensions(&self) -> String {
        self.0.extensions()
    }

    #[wasm_bindgen(getter)]
    pub fn protocol(&self) -> String {
        self.0.protocol()
    }

    #[wasm_bindgen(getter)]
    pub fn onmessage(&self) -> Option<Function> {
        self.0.onmessage()
    }

    #[wasm_bindgen(setter)]
    pub fn set_onmessage(&self, value: Option<Function>) {
        self.0.set_onmessage(value.as_ref());
    }

    #[wasm_bindgen(getter)]
    pub fn binary_type(&self) -> BinaryType {
        self.0.binary_type()
    }

    #[wasm_bindgen(setter)]
    pub fn set_binary_type(&self, value: BinaryType) {
        self.0.set_binary_type(value);
    }

    pub fn close(&self) -> Result<(), JsValue> {
        self.0.close()
    }

    pub fn send(&self, data: &JsValue) -> Result<(), JsValue> {
        if data.is_string() {
            self.0.send_with_str(&data.as_string().unwrap())
        } else if data.is_instance_of::<Blob>() {
            self.0.send_with_blob(data.unchecked_ref())
        } else if data.is_instance_of::<ArrayBuffer>() {
            self.0.send_with_array_buffer(data.unchecked_ref())
        } else if data.is_object() {
            self.0.send_with_array_buffer_view(&Object::from(data.clone()))
        } else if data.is_instance_of::<Uint8Array>() {
            self.0
                .send_with_u8_array(&data.clone().dyn_into::<Uint8Array>().expect("check already done; qed").to_vec())
        } else {
            Err(JsValue::from_str("Unsupported data type"))
        }
    }
}

// TODO: map API 1:1 from socket.io
pub mod socket_io {}
