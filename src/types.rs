use std::collections::HashMap;

use base64::{self, engine::general_purpose::URL_SAFE as base64_enc_dec, Engine as _};
use reqwest::header::HeaderValue;
use serde::{Deserialize, Serialize};
use url::Url;
use wasm_bindgen::prelude::*;

use crate::crypto::Jwk;

#[derive(Clone)]
pub struct Client(Url);

pub fn new_client(url: &str) -> Result<Client, String> {
    url::Url::parse(url).map_err(|e| e.to_string()).map(Client)
}

impl Client {
    pub async fn r#do(
        &self,
        request: &Request,
        shared_secret: &Jwk,
        backend_url: &str,
        is_static: bool,
        up_jwt: &str,
        uuid: &str,
    ) -> Result<Response, String> {
        self.transfer(request, shared_secret, backend_url, is_static, up_jwt, uuid)
            .await
    }

    async fn transfer(
        &self,
        request: &Request,
        shared_secret: &Jwk,
        backend_url: &str,
        is_static: bool,
        up_jwt: &str,
        uuid: &str,
    ) -> Result<Response, String> {
        if up_jwt.is_empty() || uuid.is_empty() {
            return Err("up_jwt and uuid are required".to_string());
        }

        let response_data = self
            .do_(request, shared_secret, backend_url, is_static, up_jwt, uuid)
            .await?;
        serde_json::from_slice::<Response>(&response_data).map_err(|e| e.to_string())
    }

    async fn do_(
        &self,
        request: &Request,
        shared_secret: &Jwk,
        backend_url: &str,
        is_static: bool,
        up_jwt: &str,
        uuid: &str,
    ) -> Result<Vec<u8>, String> {
        let request_data = RoundtripEnvelope::encode(
            &shared_secret
                .symmetric_encrypt(
                    &serde_json::to_vec(request)
                        .map_err(|e| format!("Failed to serialize request: {}", e))?,
                )
                .map_err(|e| format!("Failed to encrypt request: {}", e))?,
        )
        .to_json_bytes();

        let url = {
            if is_static {
                &self
                    .0
                    .join(Url::parse(backend_url).map_err(|e| e.to_string())?.path())
                    .map_err(|e| e.to_string())?
            } else {
                &self.0
            }
        };

        // adding headers
        let mut header_map = reqwest::header::HeaderMap::new();
        {
            header_map.insert(
                "X-Forwarded-Host",
                url.host()
                    .expect("expected host to be present; qed")
                    .to_string()
                    .parse()
                    .expect("expected host as header value to be valid; qed"),
            );

            header_map.insert(
                "X-Forwarded-Proto",
                HeaderValue::from_str(url.scheme()).expect("expected scheme to be valid; qed"),
            );

            header_map.insert(
                "Content-Type",
                HeaderValue::from_str("application/json")
                    .expect("expected content type to be valid; qed"),
            );

            header_map.insert(
                "up-JWT",
                HeaderValue::from_str(up_jwt).expect("expected up-JWT to be valid; qed"),
            );

            header_map.insert(
                "x-client-uuid",
                HeaderValue::from_str(uuid).expect("expected x-client-uuid to be valid; qed"),
            );

            if is_static {
                header_map.insert(
                    "X-Static",
                    HeaderValue::from_str("true").expect("expected X-Static to be valid; qed"),
                );
            }
        }

        let server_resp = reqwest::Client::new()
            .post(url.as_str())
            .body(request_data)
            .headers(header_map)
            .send()
            .await
            .map_err(|e| format!("Failed to send request: {}", e))?;

        let body = server_resp
            .bytes()
            .await
            .map_err(|e| format!("Failed to read response: {}", e))?;

        let response_data = RoundtripEnvelope::from_json_bytes(&body)
            .decode()
            .map_err(|e| format!("Failed to decode response: {}", e))?;

        shared_secret
            .symmetric_decrypt(&response_data)
            .map_err(|e| format!("Failed to decrypt response: {}", e))
    }
}

#[derive(Deserialize, Serialize, Default)]
pub struct Request {
    pub method: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Response {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// This struct is used to serialize and deserialize the encrypted data, for the purpose of
/// "round-tripping" the data through the proxy server.
#[derive(Deserialize, Serialize)]
struct RoundtripEnvelope {
    data: String,
}

impl RoundtripEnvelope {
    fn encode(data: &[u8]) -> Self {
        let mut val = String::new();
        base64_enc_dec.encode_string(data, &mut val);
        RoundtripEnvelope { data: val }
    }

    fn decode(&self) -> Result<Vec<u8>, base64::DecodeError> {
        let mut val = Vec::new();
        base64_enc_dec.decode_vec(&self.data, &mut val)?;
        Ok(val)
    }

    fn to_json_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("RoundtripEnvelope implements Serialize")
    }

    fn from_json_bytes(data: &[u8]) -> Self {
        serde_json::from_slice(data)
            .expect("Error with RoundtripEnvelope deserialization, check the payload")
    }
}

#[derive(Clone)]
#[wasm_bindgen]
pub struct DbCache {
    pub(crate) store: String,
    pub(crate) key_path: String,
    pub(crate) indexes: Indexes,
}

#[allow(non_snake_case)]
#[wasm_bindgen]
impl DbCache {
    #[wasm_bindgen(constructor)]
    pub fn new(store: String, key_path: String, indexes: Indexes) -> DbCache {
        DbCache {
            store,
            key_path,
            indexes,
        }
    }

    #[wasm_bindgen(getter)]
    pub fn store(&self) -> String {
        self.store.clone()
    }

    #[wasm_bindgen(setter)]
    pub fn set_store(&mut self, store: String) {
        self.store = store;
    }

    #[wasm_bindgen(getter)]
    pub fn key_path(&self) -> String {
        self.key_path.clone()
    }

    #[wasm_bindgen(setter)]
    pub fn set_key_path(&mut self, key_path: String) {
        self.key_path = key_path;
    }

    #[wasm_bindgen(getter)]
    pub fn indexes(&self) -> js_sys::Object {
        let obj = js_sys::Object::new();
        js_sys::Reflect::set(
            &obj,
            &"url".into(),
            &serde_wasm_bindgen::to_value(&self.indexes.url)
                .expect("failed to serialize url index"),
        )
        .unwrap();
        js_sys::Reflect::set(
            &obj,
            &"_exp".into(),
            &serde_wasm_bindgen::to_value(&self.indexes._exp)
                .expect("failed to serialize url index"),
        )
        .unwrap();
        obj
    }

    #[wasm_bindgen(setter)]
    pub fn set_indexes(&mut self, indexes: Indexes) {
        self.indexes = indexes;
    }
}

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct Indexes {
    pub(crate) url: Uniqueness,
    pub(crate) _exp: Uniqueness,
}

#[allow(non_snake_case)]
#[wasm_bindgen]
impl Indexes {
    #[wasm_bindgen(constructor)]
    pub fn new(url: Uniqueness, _exp: Uniqueness) -> Indexes {
        Indexes { url, _exp }
    }

    #[wasm_bindgen(getter)]
    pub fn url(&self) -> Uniqueness {
        self.url.clone()
    }

    #[wasm_bindgen(setter)]
    pub fn set_url(&mut self, url: Uniqueness) {
        self.url = url;
    }

    #[wasm_bindgen(getter)]
    pub fn _exp(&self) -> Uniqueness {
        self._exp.clone()
    }

    #[allow(non_snake_case)] // Need the underscore to match the JS property name.
    #[wasm_bindgen(setter)]
    pub fn set__exp(&mut self, exp: Uniqueness) {
        self._exp = exp;
    }
}

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct Uniqueness {
    pub(crate) unique: bool,
}

#[allow(non_snake_case)]
#[wasm_bindgen]
impl Uniqueness {
    #[wasm_bindgen(constructor)]
    pub fn new(unique: bool) -> Uniqueness {
        Uniqueness { unique }
    }

    #[wasm_bindgen(getter)]
    pub fn unique(&self) -> bool {
        self.unique
    }

    #[wasm_bindgen(setter)]
    pub fn set_unique(&mut self, unique: bool) {
        self.unique = unique;
    }
}
