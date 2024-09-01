use core::time;
use std::collections::HashMap;
use std::sync::Mutex;

use crypto::generate_key_pair;
use js_sys::Object;
use js_sys::Promise;
use reqwest::header::HeaderValue;
use uuid::Uuid;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsValue;

mod crypto;
pub mod types;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn console_log(s: &str);

    #[wasm_bindgen(js_namespace = console, js_name = error)]
    fn console_error(s: &str);

    #[wasm_bindgen(js_namespace = indexedDB, js_name = open)]
    fn indexed_db_open(db_name: &str) -> Object;

    #[wasm_bindgen(js_namespace = Object, js_name = entries)]
    fn object_entries(obj: &Object) -> js_sys::Array;
}

#[wasm_bindgen(module = "src/js/indexed_db.js")]
extern "C" {
    fn open_db(db_name: &str, db_cache: types::DbCache);
    fn clear_expired_cache(db_name: &str);
}

const INTERCEPTOR_VERSION: &str = "0.0.14";
const INDEXED_DB_CACHE: &str = "_layer8cache";

// Global state should be "ok" since a web-assembly module is a singleton.
lazy_static::lazy_static! {
    static ref STATIC_PATH: Mutex<String> = Mutex::new(String::from(""));
    static ref ENCRYPTED_TUNNEL_FLAG : Mutex<bool> = Mutex::new(false);
    static ref COUNTER: Mutex<u32> = Mutex::new(0);

    /// The cache time-to-live for the IndexedDB cache is 2 days.
    static ref INDEXED_DB_CACHE_TTL: time::Duration = time::Duration::new(60, 0) * 60 * 24 * 2;

    /// The cache instantiates with the `_layer8cache` IndexedDB.
    static ref INDEXED_DBS: HashMap<String, types::DbCache> = HashMap::from([
        (
            INDEXED_DB_CACHE.to_string(),
            types::DbCache {
                store: "static".to_string(),
                key_path: "url".to_string(),
                indexes: types::Indexes{
                    url: types::Uniqueness { unique: true },
                    _exp: types::Uniqueness { unique: false },
                }
            },
        )
    ]);
}

#[wasm_bindgen(js_name = static)]
pub fn get_static(url: JsValue) -> Promise {
    todo!()
}

#[wasm_bindgen(js_name = checkEncryptedTunnel)]
pub fn check_encrypted_tunnel() -> Promise {
    Promise::resolve(&JsValue::from(
        *ENCRYPTED_TUNNEL_FLAG
            .lock()
            .expect("expected ENCRYPTED_TUNNEL_FLAG to be locked; qed"),
    ))
}

#[wasm_bindgen]
pub fn fetch(url: JsValue, args: js_sys::Array) -> Promise {
    todo!()
}

/// Test promise resolution/rejection from the console.
#[wasm_bindgen(js_name = testWASM)]
pub fn test_wasm(arg: JsValue) -> Promise {
    if arg.is_null() || arg.is_undefined() {
        let err = JsError::new("The argument is null or undefined.");
        return Promise::reject(&err.into());
    }

    let msg =    format!("WASM Interceptor version {INTERCEPTOR_VERSION} successfully loaded. Argument passed: {:?}. To test promise rejection, call with no argument.", arg);
    Promise::resolve(&JsValue::from(msg))
}

#[wasm_bindgen(js_name = persistenceCheck)]
pub fn persistence_check() -> Promise {
    let mut counter = COUNTER.lock().unwrap();
    *counter += 1;
    console_log(&format!("WASM Counter: {}", *counter));
    Promise::resolve(&JsValue::from(*counter))
}

#[wasm_bindgen(js_name = initEncryptedTunnel)]
pub async fn init_encrypted_tunnel(args: js_sys::Array) -> Promise {
    let mut providers = Vec::new();
    let mut proxy = "https://layer8devproxy.net".to_string(); // set LAYER8_PROXY in the environment to override
    let mut mode = "prod".to_string();
    if args.length() > 1 {
        mode = args
            .get(1)
            .as_string()
            .expect("the mode expected to be a string; qed");
    }

    // clear cache; TODO: is there some form of concurrency to do this in the background?
    clear_expired_cache(INDEXED_DB_CACHE);

    let mut error_destructuring = false;
    let entries = object_entries(&js_sys::global());
    for entry in entries.iter() {
        let val = js_sys::Array::from(&entry); // [key, value] result from Object.entries
        match val
            .get(0)
            .as_string()
            .expect("key is a string; qed")
            .as_str()
        {
            "providers" => {
                // providers is a list of strings
                let providers_entries = js_sys::Array::from(&val.get(1));
                providers_entries.iter().for_each(|provider| {
                    providers.push(provider.as_string().expect("provider is a string; qed"));
                });
            }

            "proxy" => {
                if mode == "dev" {
                    proxy = val.get(1).as_string().expect("proxy is a string; qed");
                } else if let Ok(val) = std::env::var("LAYER8_PROXY") {
                    proxy = val;
                }
            }

            "staticPath" => {
                *STATIC_PATH.lock().unwrap() =
                    val.get(1).as_string().expect("staticPath is a string; qed")
            }

            _ => {
                error_destructuring = true;
            }
        }
    }

    if error_destructuring {
        let err = JsError::new("Unable to destructure the layer8 encrypted object.");
        return Promise::reject(&err.into());
    }

    for provider in providers {
        if let Err(err) = init_tunnel(&provider, &proxy).await {
            let err = JsError::new(&err);
            return Promise::reject(&err.into());
        }
    }

    Promise::resolve(&JsValue::null())
}

async fn init_tunnel(provider: &str, proxy: &str) -> Result<(), String> {
    let provider_url = rebuild_url(provider);
    let (pivate_jwk_ecdh, pub_jwk_ecdh) = generate_key_pair(crypto::KeyType::Ecdh)?;

    let b64_pub_jwk = pub_jwk_ecdh.export_as_base64()?;

    let proxy = format!("{proxy}/init-tunnel?backend={provider}");

    let res = reqwest::Client::new()
        .post(&proxy)
        .body(b64_pub_jwk.clone())
        .headers({
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(
                "x-ecdh-init",
                HeaderValue::from_str(&b64_pub_jwk).expect(""),
            );
            headers.insert(
                "x-client-uuid",
                HeaderValue::from_str(&Uuid::new_v4().to_string()).expect(""),
            );
            headers
        })
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if res.status().eq(&401) {
        *ENCRYPTED_TUNNEL_FLAG.lock().unwrap() = false;
        return Err(String::from(
            "401 response from proxy, user is not authorized.",
        ));
    }

    let mut proxy_data: HashMap<String, serde_json::Value> =
        serde_json::from_slice(&res.bytes().await.map_err(|e| e.to_string())?).unwrap();

    if let Some((_, up_jwt)) = proxy_data.remove_entry("up-JWT") {
        // to cont...
    }

    todo!()
}

fn rebuild_url(url: &str) -> String {
    let url = url::Url::parse(url).expect("expected provider to be a valid URL; qed");
    let mut rebuilt_url = url.scheme().to_string()
        + "://"
        + url.host_str().expect("expected host to be present; qed");
    if let Some(port) = url.port() {
        rebuilt_url.push_str(&format!(":{}", port.to_string().as_str()));
    }

    rebuilt_url
}
