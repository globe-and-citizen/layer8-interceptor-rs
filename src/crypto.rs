use std::{collections::HashMap, io::Read};

use aes_gcm::{
    aead::{Aead, Nonce},
    AeadCore, KeyInit,
};
use base64::{self, engine::general_purpose::STANDARD as base64_enc_dec, Engine as _};
use rand::{
    rngs::{OsRng, StdRng},
    Rng, SeedableRng,
};
use secp256k1::{ecdh::SharedSecret, PublicKey, Secp256k1, SecretKey};
use serde::{Deserialize, Serialize};

pub trait KeyPairDerivative {
    fn public_key(&self) -> Result<Option<PublicKey>, String>;
    fn secret_key(&self) -> Result<Option<SecretKey>, String>;
}

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

impl KeyPairDerivative for Jwk {
    fn public_key(&self) -> Result<Option<PublicKey>, String> {
        let public_key = {
            let coordinate_x = base64_enc_dec
                .decode(&self.coordinate_x)
                .map_err(|e| format!("Failed to decode x coordinate: {}", e))?;
            let coordinate_y = base64_enc_dec
                .decode(&self.coordinate_y)
                .map_err(|e| format!("Failed to decode y coordinate: {}", e))?;

            let mut public_key_bytes = [4u8; 65];
            public_key_bytes[1..33].copy_from_slice(&coordinate_x);
            public_key_bytes[33..].copy_from_slice(&coordinate_y);

            PublicKey::from_slice(&public_key_bytes)
                .map_err(|e| format!("Failed to create public key: {}", e))?
        };

        Ok(Some(public_key))
    }

    fn secret_key(&self) -> Result<Option<SecretKey>, String> {
        if self.coordinate_d.is_empty() {
            return Ok(None);
        }

        let secret_key = {
            let coordinate_d = base64_enc_dec
                .decode(&self.coordinate_d)
                .map_err(|e| format!("Failed to decode d coordinate: {}", e))?;
            SecretKey::from_slice(&coordinate_d)
                .map_err(|e| format!("Failed to create secret key: {}", e))?
        };

        Ok(Some(secret_key))
    }
}

impl Jwk {
    pub fn symmetric_encrypt(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        if !self.key_ops.contains(&"encrypt".to_string()) {
            return Err("receiver key_ops must contain 'encrypt'".to_string());
        }

        let block_cipher = {
            let coordinate_x = base64_enc_dec
                .decode(&self.coordinate_x)
                .map_err(|e| format!("Failed to decode x coordinate: {}", e))?;
            aes_gcm::Aes256Gcm::new_from_slice(&coordinate_x)
                .map_err(|e| format!("Failed to create block cipher: {}", e))?
        };

        block_cipher
            .encrypt(&aes_gcm::Aes256Gcm::generate_nonce(&mut OsRng), data)
            .map_err(|e| format!("Failed to encrypt data: {}", e))
    }

    pub fn convert_to_key_pairs(&self) -> Result<(), String> {
        todo!()
    }

    pub fn symmetric_decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, String> {
        if ciphertext.is_empty() {
            return Err("ciphertext is empty".to_string());
        }

        if !self.key_ops.contains(&"decrypt".to_string()) {
            return Err("receiver key_ops must contain 'decrypt'".to_string());
        }

        let block_cipher = {
            let coordinate_x = base64_enc_dec
                .decode(&self.coordinate_x)
                .map_err(|e| format!("Failed to decode x coordinate: {}", e))?;
            aes_gcm::Aes256Gcm::new_from_slice(&coordinate_x)
                .map_err(|e| format!("Failed to create block cipher: {}", e))?
        };

        // +-------------------+--------------------+
        // |        Nonce      |   CipherText       |
        // +-------------------+--------------------+
        //  <---- 12 bytes --->
        let (nonce, cipher_text) = ciphertext.split_at(NONCE_SIZE);
        let nonce = Nonce::<aes_gcm::Aes256Gcm>::from_slice(nonce);
        block_cipher
            .decrypt(nonce, cipher_text)
            .map_err(|e| format!("Failed to decrypt data: {}", e.to_string()))
    }

    pub fn get_ecdh_shared_secret(&self, public_key: &Jwk) -> Result<Jwk, String> {
        // must be a public key
        if !public_key.coordinate_d.is_empty() {
            return Err("public key must not contain a private key".to_string());
        }

        // must have 'deriveKey' in its key_ops
        if !public_key.key_ops.contains(&"deriveKey".to_string()) {
            return Err("public key must contain 'deriveKey' in its key_ops".to_string());
        }

        // the calling key must be a private key
        if self.coordinate_d.is_empty() {
            return Err(
                "The associated type expected a private key, does not contain coordinate_d"
                    .to_string(),
            );
        }

        if !self.key_ops.contains(&"deriveKey".to_string()) {
            return Err("The associated type expected a private key, does not contain 'deriveKey' in key_ops".to_string());
        }

        // getting the secret key's derivation
        let secret_key = self
            .secret_key()?
            .expect("the secret key has already been validated");

        let public_key = public_key
            .public_key()?
            .expect("the public key has already been validated");

        let shared_secret = SharedSecret::new(&public_key, &secret_key);

        Ok(Jwk {
            key_type: "EC".to_string(),
            key_ops: vec!["encrypt".to_string(), "decrypt".to_string()],
            key_id: format!("shared_{}", {
                let mut key_id = "shared_".to_string();
                for i in &self.key_id.as_bytes()[4..] {
                    key_id.push(*i as char);
                }
                key_id
            }),
            crv: self.crv.clone(),
            coordinate_x: base64_enc_dec.encode(shared_secret.as_ref()),
            ..Default::default()
        })
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
