use std::collections::HashMap;

use aes_gcm::{
    aead::{Aead, Nonce},
    AeadCore, KeyInit,
};
use base64::{self, engine::general_purpose::STANDARD as base64_enc_dec, Engine as _};
use rand::{
    rngs::{OsRng, StdRng},
    Rng, SeedableRng,
};
use secp256k1::Secp256k1;
use serde::{Deserialize, Serialize};

pub enum KeyUse {
    Ecdsa,
    Ecdh,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Jwk {
    #[serde(rename = "use")]
    pub key_ops: Vec<String>, // ["sign", "verify", "encrypt", "decrypt", "wrapKey", "unwrapKey", "deriveKey", "deriveBits"]
    pub key_type: String, // "EC", "RSA"
    pub key_id: String,   // Key ID
    pub crv: String,      // "P-256"
    #[serde(rename = "x")]
    pub coordinate_x: String, // x coordinate as base64 URL encoded string.
    #[serde(rename = "y")]
    pub coordinate_y: String, // y coordinate as base64 URL encoded string.
    #[serde(rename = "d")]
    #[serde(skip_serializing_if = "String::is_empty")]
    pub coordinate_d: String, // d coordinate as base64 URL encoded string. Private keys only.
}

// AES-GCM uses a nonce size of 12 bytes. Reference: https://crypto.stackexchange.com/a/41610
const NONCE_SIZE: usize = 12;

pub fn generate_key_pair(key_use: KeyUse) -> Result<(Jwk, Jwk), String> {
    let id = {
        let mut id = [0u8; 16];
        let mut rng = StdRng::from_entropy();
        rng.fill(&mut id);
        id
    };

    // Generate an ECDSA key pair of the P-256 curve.
    let (secret_key, public_key) = Secp256k1::new().generate_keypair(&mut OsRng);

    let coordinate_d = base64_enc_dec.encode(secret_key.secret_bytes()); // Private key; d coordinate
    let pub_key_uncompressed = public_key.serialize_uncompressed(); // Public key; x and y coordinates
    let coordinate_x = base64_enc_dec.encode(&pub_key_uncompressed[1..33]); // x coordinate
    let coordinate_y = base64_enc_dec.encode(&pub_key_uncompressed[33..]); // y coordinate

    let private_jwk = {
        let private_key_use = match key_use {
            KeyUse::Ecdh => "sign".to_string(),
            KeyUse::Ecdsa => "deriveKey".to_string(),
        };

        Jwk {
            key_type: "EC".to_string(),
            crv: "P-256".to_string(),
            key_id: format!("priv_{}", base64_enc_dec.encode(id)),
            key_ops: vec![private_key_use],
            coordinate_d,
            coordinate_x: coordinate_x.clone(),
            coordinate_y: coordinate_y.clone(),
        }
    };

    let public_jwk = {
        let pub_key_use = match key_use {
            KeyUse::Ecdh => "verify".to_string(),
            KeyUse::Ecdsa => "deriveKey".to_string(),
        };

        Jwk {
            key_type: "EC".to_string(),
            crv: "P-256".to_string(),
            key_id: format!("pub_{}", base64_enc_dec.encode(id)),
            key_ops: vec![pub_key_use],
            coordinate_x,
            coordinate_y,
            ..Default::default()
        }
    };

    Ok((private_jwk, public_jwk))
}

impl Jwk {
    pub fn symmetric_encrypt(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        if !self.key_ops.contains(&"encrypt".to_string()) {
            return Err("receiver key_ops must contain 'encrypt'".to_string());
        }

        let coordinate_x = base64_enc_dec
            .decode(&self.coordinate_x)
            .map_err(|e| format!("Failed to decode x coordinate: {}", e))?;

        let block_cipher = aes_gcm::Aes256Gcm::new_from_slice(&coordinate_x)
            .map_err(|e| format!("Failed to create block cipher: {}", e))?;

        block_cipher
            .encrypt(&aes_gcm::Aes256Gcm::generate_nonce(&mut OsRng), data)
            .map_err(|e| format!("Failed to encrypt data: {}", e))
    }

    pub fn symmetric_decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, String> {
        if ciphertext.is_empty() {
            return Err("receiver key_ops must contain 'decrypt'".to_string());
        }

        if !self.key_ops.contains(&"decrypt".to_string()) {
            return Err("receiver key_ops must contain 'decrypt'".to_string());
        }

        let coordinate_x = base64_enc_dec
            .decode(&self.coordinate_x)
            .map_err(|e| format!("Failed to decode x coordinate: {}", e))?;

        let block_cipher = aes_gcm::Aes256Gcm::new_from_slice(&coordinate_x)
            .map_err(|e| format!("Failed to create block cipher: {}", e))?;

        // +-------------------+--------------------+
        // |        Nonce      |   CipherText       |
        // +-------------------+--------------------+
        //  <---- 12 bytes --->
        let (nonce, cipher_text) = ciphertext.split_at(NONCE_SIZE);
        let nonce = Nonce::<aes_gcm::Aes256Gcm>::from_slice(nonce);
        block_cipher
            .decrypt(nonce, cipher_text)
            .map_err(|e| format!("Failed to decrypt data: {}", e))
    }

    pub fn export_as_base64(&self) -> String {
        let jwk_json =
            serde_json::to_string(self).expect("Jwk implements Serialize and Deserialize");
        base64_enc_dec.encode(jwk_json.as_bytes())
    }
}

pub fn jwk_from_map(map: HashMap<String, serde_json::Value>) -> Result<Jwk, String> {
    let server_pub_key = map
        .get("server_pubKeyECDH")
        .ok_or("server_pubKeyECDH not found")?
        .clone();

    serde_json::from_value::<Jwk>(server_pub_key)
        .map_err(|e| format!("Failed to deserialize server_pubKeyECDH: {}", e))
}
