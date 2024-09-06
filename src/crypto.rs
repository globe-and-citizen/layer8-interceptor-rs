use std::collections::HashMap;

use aes_gcm::KeyInit;
use base64::{self, engine::general_purpose::STANDARD as base64_enc_dec, Engine as _};
use rand::{
    rngs::{OsRng, SmallRng},
    Rng, SeedableRng,
};

pub enum KeyUse {
    Ecdsa,
    Ecdh,
}

#[derive(Debug, Clone, Default)]
pub struct Jwk {
    pub key_ops: Vec<String>, // ["sign", "verify", "encrypt", "decrypt", "wrapKey", "unwrapKey", "deriveKey", "deriveBits"]
    pub key_type: String,     // "EC", "RSA"
    pub key_id: String,       // Key ID
    pub crv: String,          // "P-256"
    pub coordinate_x: String, // x coordinate as base64 URL encoded string.
    pub coordinate_y: String, // y coordinate as base64 URL encoded string.
    pub coordinate_d: String, // d coordinate as base64 URL encoded string. Private keys only.
}

/// A key pair is a tuple of two JWKs, the first one being the private key and the second one being the public key.
pub type KeyPair = (Jwk, Jwk);

pub fn generate_key_pair(key_use: KeyUse) -> Result<KeyPair, String> {
    let id = {
        let mut id = [0u8; 16];
        let mut small_rng = SmallRng::from_entropy();
        small_rng.fill(&mut id);
        id
    };

    let private_key_use = match key_use {
        KeyUse::Ecdh => "sign".to_string(),
        KeyUse::Ecdsa => "deriveKey".to_string(),
    };

    let mut private_jwk = Jwk {
        key_type: "EC".to_string(),
        crv: "P-256".to_string(),
        key_id: format!("priv_{}", base64_enc_dec.encode(id)),
        key_ops: vec![private_key_use],
        ..Default::default()
    };

    let pub_key_use = match key_use {
        KeyUse::Ecdh => "verify".to_string(),
        KeyUse::Ecdsa => "deriveKey".to_string(),
    };

    let mut public_jwk = Jwk {
        key_ops: vec![pub_key_use],
        ..Default::default()
    };

    Ok((private_jwk, public_jwk))
}

impl Jwk {
    // func (ss *JWK) SymmetricEncrypt(data []byte) ([]byte, error) {
    //     if !slices.Contains(ss.Key_ops, "encrypt") {
    //         return nil, fmt.Errorf("Receiver Key_ops must include 'encrypt' ")
    //     }

    //     ssBS, err := base64.URLEncoding.DecodeString(ss.X)
    //     if err != nil {
    //         return nil, fmt.Errorf("Unable to interpret ss.X coordinate as byte slice: %w", err)
    //     }
    //     blockCipher, err := aes.NewCipher(ssBS)
    //     if err != nil {
    //         return nil, fmt.Errorf("Symmetric encryption failed @ 1 : %w", err)
    //     }
    //     aesgcm, err := cipher.NewGCM(blockCipher)
    //     if err != nil {
    //         return nil, fmt.Errorf("Symmetric encryption failed @ 2: %w", err)
    //     }
    //     nonce := make([]byte, aesgcm.NonceSize())
    //     if _, err = io.ReadFull(rand.Reader, nonce); err != nil {
    //         return nil, fmt.Errorf("Symmetric encryption failed @ 3: %w", err)
    //     }

    //     cipherText := aesgcm.Seal(nonce, nonce, data, nil)

    //     return cipherText, nil
    // }

    pub fn symmetric_encrypt(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        if !self.key_ops.contains(&"encrypt".to_string()) {
            return Err("receiver key_ops must contain 'encrypt'".to_string());
        }

        let mut ss_bs = Vec::new();
        base64_enc_dec
            .decode_vec(self.coordinate_x.as_bytes(), &mut ss_bs)
            .map_err(|e| format!("Unable to interpret ss.X coordinate as byte slice: {}", e))?;

        let block_cipher = aes_gcm::Aes256Gcm::generate_key(OsRng);

        todo!()
    }

    pub fn symmetric_decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, String> {
        todo!()
    }

    pub fn export_as_base64(&self) -> Result<String, String> {
        todo!()
    }
}

pub fn jwk_from_map(map: HashMap<String, serde_json::Value>) -> Result<Jwk, String> {
    todo!()
}
