use std::{collections::HashMap, default};

use base64::{engine::general_purpose::STANDARD as base64_enc_dec, Engine};
use reqwest::header::{self, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::crypto::jwk;

pub trait InterceptorClient {
    fn get_url(&self) -> &Url;
    async fn r#do(
        &self,
        request: &Request,
        shared_secret: &jwk,
        backend_url: &str,
        is_static: bool,
        up_jwt: &str,
        uuid: &str,
    ) -> Result<Vec<u8>, String>;
}

struct Client(Url);

impl Client {
    async fn transfer(
        &self,
        shared_secret: &jwk,
        req: &Request,
        url: &Url,
        is_static: bool,
        up_jwt: &str,
        uuid: &str,
    ) -> Result<Response, String> {
        if up_jwt == "" || uuid == "" {
            return Err("up_jwt and uuid are required".to_string());
        }

        let response_data = self
            .r#do(req, shared_secret, url.as_str(), is_static, up_jwt, uuid)
            .await?;

        serde_json::from_slice::<Response>(&response_data).map_err(|e| e.to_string())
    }
}

fn new_client(protocol: &str, host: &str, port: u16) -> Result<impl InterceptorClient, String> {
    url::Url::parse(&format!("{}://{}:{}", protocol, host, port))
        .map_err(|e| e.to_string())
        .map(|val| Client(val))
}

impl InterceptorClient for Client {
    fn get_url(&self) -> &Url {
        &self.0
    }

    async fn r#do(
        &self,
        request: &Request,
        shared_secret: &jwk,
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
            header_map
                .insert(
                    "X-Forwarded-Host",
                    url.host()
                        .expect("expected host to be present; qed")
                        .to_string()
                        .parse()
                        .expect("expected host as header value to be valid; qed"),
                )
                .expect("expected header to be inserted; qed");

            header_map
                .insert(
                    "X-Forwarded-Proto",
                    HeaderValue::from_str(url.scheme()).expect("expected scheme to be valid; qed"),
                )
                .expect("expected header to be inserted; qed");

            header_map
                .insert(
                    "Content-Type",
                    HeaderValue::from_str("application/json")
                        .expect("expected content type to be valid; qed"),
                )
                .expect("expected header to be inserted; qed");

            header_map
                .insert(
                    "up-JWT",
                    HeaderValue::from_str(up_jwt).expect("expected up-JWT to be valid; qed"),
                )
                .expect("expected header to be inserted; qed");

            header_map
                .insert(
                    "x-client-uuid",
                    HeaderValue::from_str(uuid).expect("expected x-client-uuid to be valid; qed"),
                )
                .expect("expected header to be inserted; qed");

            if is_static {
                header_map
                    .insert(
                        "X-Static",
                        HeaderValue::from_str("true").expect("expected X-Static to be valid; qed"),
                    )
                    .expect("expected header to be inserted; qed");
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

#[derive(Deserialize, Serialize)]
pub struct Request {
    pub method: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Response {
    pub status: i32,
    pub status_text: String,
    pub headers: HashMap<String, String>,
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
        base64_enc_dec.encode_string(&data, &mut val);
        RoundtripEnvelope { data: val }
    }

    fn decode(&self) -> Result<Vec<u8>, base64::DecodeError> {
        let mut val = Vec::new();
        base64_enc_dec.decode_vec(&self.data, &mut val)?;
        Ok(val)
    }

    fn to_json_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap()
    }

    fn from_json_bytes(data: &[u8]) -> Self {
        serde_json::from_slice(data).unwrap()
    }
}
