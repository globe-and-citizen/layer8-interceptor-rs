use std::{collections::HashMap, str::Bytes, time::SystemTime, vec};

use base64::{self, engine::general_purpose::STANDARD as base64_enc_dec, Engine as _};
use jsonwebtoken::{EncodingKey, Header};
use rand::{rngs::SmallRng, RngCore, SeedableRng};
use reqwest::header;
use serde::{Deserialize, Serialize};

use layer8_interceptor_rs::{
    crypto::{base64_to_jwk, generate_key_pair, Jwk, KeyUse},
    types::{Request, Response},
};
use uuid::Uuid;

// Claims is an arbitrary struct that will be encoded to a JWT.
#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    expires_at: i64,
}

#[test]
fn jwt_to_derivatives_test() {
    let (private_key, public_key) = generate_key_pair(KeyUse::Ecdh).unwrap();
    // able to change to public key and private key derivatives
    _ = private_key.public_key_derivative().unwrap();
    _ = private_key.secret_key_derivative().unwrap();
    _ = public_key.public_key_derivative().unwrap();
    assert!(public_key.secret_key_derivative().is_err());
}

#[tokio::test]
async fn roundtrip_test() {
    let (priv_key_server, pub_key_server) = generate_key_pair(KeyUse::Ecdh).unwrap();

    // Testing the init-tunnel endpoint
    {
        let uuid = Uuid::new_v4().to_string();

        let (priv_key_client, pub_key_client) = generate_key_pair(KeyUse::Ecdh).unwrap();
        let base64_pub_key = pub_key_client.export_as_base64();

        let req = Request {
            headers: HashMap::from([
                ("x-ecdh-init".to_string(), base64_pub_key.clone()),
                ("x-client-uuid".to_string(), uuid),
            ]),
            method: "POST".to_string(),
            body: Vec::from(base64_pub_key.as_bytes()),
        };

        let resp = init_tunnel_mock_handler(req, priv_key_server, pub_key_server);

        //   let up_jwt = resp.headers.get("up_JWT").unwrap();
    }
}

// TODO: Implement the init-tunnel endpoint with something like warp, or jsut creating an actual endpoint that callable over HTTP @Osoro
fn init_tunnel_mock_handler(req: Request, server_priv_key: Jwk, server_pub_key: Jwk) -> Response {
    let token = generate_token("mock_secret").unwrap();
    let server_public_jwk = base64_to_jwk(req.headers.get("x-ecdh-init").unwrap()).unwrap();
    _ = server_priv_key
        .get_ecdh_shared_secret(&server_public_jwk)
        .unwrap();

    let json_body = serde_json::json!({
        "up_JWT": token,
        "server_pubKeyECDH": server_pub_key,
    })
    .to_string();

    Response {
        status: 200,
        body: json_body.as_bytes().to_vec(),
        headers: vec![("up_JWT".to_string(), token)],
        ..Default::default()
    }
}

// TODO: see [`init_tunnel_mock_handler`] documentation
fn default_mock_handler(req_url: url::Url, shared_key: Jwk, req: Request) -> Response {
    let host = req.headers.get("X-Forwarded-Host").unwrap();
    assert_eq!(req_url.host_str().unwrap(), host);

    let protocol = req.headers.get("X-Forwarded-Proto").unwrap();
    assert_eq!(req_url.scheme(), protocol);

    let req_body = serde_json::from_slice::<HashMap<String, String>>(&req.body).unwrap();

    // it is expected that the body is encrypted and encoded in base64 format
    // and set to the "data" key of the request body
    let data = base64_enc_dec
        .decode(&req_body.get("data").unwrap())
        .unwrap();

    let decrypted = shared_key.symmetric_decrypt(&data).unwrap();

    // Seems confusing that we are deserializing the request again, but bear in mind that the request
    // was encrypted and encoded in base64 format before being sent to the server, and the one on the function signature
    // is a convenience wrapper to mock a http roungtrip call
    //
    // We are only interested in the fact that the request was decrypted successfully
    _ = serde_json::from_slice::<Request>(&decrypted).unwrap();

    let response_body = serde_json::json!({
        "test": "test-response",
    })
    .to_string();

    // encrypt and return response
    let res = Response {
        body: response_body.as_bytes().to_vec(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("X-Test-Header".to_string(), "test-response".to_string()),
        ],
        status: 200,
        status_text: "OK".to_string(),
    };

    let enc_res = shared_key
        .symmetric_encrypt(serde_json::to_string(&res).unwrap().as_bytes())
        .unwrap();

    let enc_res_json = serde_json::json!({
        "data": base64_enc_dec.encode(&enc_res),
    })
    .to_string();

    Response {
        status: 200,
        body: enc_res_json.as_bytes().to_vec(),
        ..Default::default()
    }
}

fn generate_token(secret_key: &str) -> Result<String, String> {
    let claims = Claims {
        expires_at: SystemTime::now()
            .checked_add(std::time::Duration::from_secs(60 * 60 * 24 * 7))
            .unwrap()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64,
    };

    let header = Header::new(jsonwebtoken::Algorithm::HS256);
    let secret_key = EncodingKey::from_secret(secret_key.as_ref());
    jsonwebtoken::encode(&header, &claims, &secret_key).map_err(|e| e.to_string())
}
