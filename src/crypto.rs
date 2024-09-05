use std::collections::HashMap;

pub enum KeyType {
    Ecdsa,
    Ecdh,
}

#[derive(Debug, Clone, Default)]
pub struct Jwk {
    pub key_ops: Vec<String>, // ["sign", "verify", "encrypt", "decrypt", "wrapKey", "unwrapKey", "deriveKey", "deriveBits"]
    pub kty: String,          // "EC", "RSA"
    pub kid: String,          // Key ID
    pub crv: String,          // "P-256"
    pub x: String,            // x coordinate as base64 URL encoded string.
    pub y: String,            // y coordinate as base64 URL encoded string.
    pub d: String,            // d coordinate as base64 URL encoded string. Private keys only.
}

impl Jwk {
    pub fn symmetric_encrypt(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        todo!()
    }

    pub fn symmetric_decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, String> {
        todo!()
    }

    pub fn export_as_base64(&self) -> Result<String, String> {
        todo!()
    }
}

pub fn generate_key_pair(key_use: KeyType) -> Result<(Jwk, Jwk), String> {
    todo!()
}

pub fn jwk_from_map(map: HashMap<String, serde_json::Value>) -> Result<Jwk, String> {
    todo!()
}
