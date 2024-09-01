enum KeyType {
    ECDSA,
    ECDH,
}

pub struct jwk {
    pub key_ops: Vec<String>, // ["sign", "verify", "encrypt", "decrypt", "wrapKey", "unwrapKey", "deriveKey", "deriveBits"]
    pub kty: String,          // "EC", "RSA"
    pub kid: String,          // Key ID
    pub crv: String,          // "P-256"
    pub x: String,            // x coordinate as base64 URL encoded string.
    pub y: String,            // y coordinate as base64 URL encoded string.
    pub d: String,            // d coordinate as base64 URL encoded string. Private keys only.
}

impl jwk {
    pub fn symmetric_encrypt(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        todo!()
    }

    pub fn symmetric_decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, String> {
        todo!()
    }
}

pub fn generate_key_pair(key_use: KeyType) -> Result<(jwk, jwk), String> {
    todo!()
}
