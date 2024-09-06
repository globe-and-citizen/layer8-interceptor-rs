use std::{cell::Cell, collections::HashMap, str::FromStr};

use js_sys::{Object, Promise};
use rand::rngs::SmallRng;
use rand::{Rng, RngCore, SeedableRng};
use reqwest::header::{HeaderName, HeaderValue};
use url::Url;
use wasm_bindgen::prelude::*;

mod crypto;
mod types;
use crypto::{generate_key_pair, jwk_from_map};
use types::new_client;

/// This block imports Javascript functions that are provided by the JS Runtime.
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn console_log(s: &str);

    #[wasm_bindgen(js_namespace = console, js_name = error)]
    fn console_error(s: &str);

    #[wasm_bindgen(js_namespace = Object, js_name = entries)]
    fn object_entries(obj: &Object) -> js_sys::Array;
}

/// This block import JavaScript functionality that is not mapped by the wasm-bindgen tool.
#[wasm_bindgen(module = "src/js/indexed_db.js")]
extern "C" {
    fn open_db(db_name: &str, db_cache: types::DbCache);
    fn clear_expired_cache(db_name: &str);
    fn serve_static(
        db_name: &str,
        body: &[u8],
        file_type: &str,
        url: &str,
        exp_in_seconds: u32,
    ) -> String;
}

const INTERCEPTOR_VERSION: &str = "0.0.14";
const INDEXED_DB_CACHE: &str = "_layer8cache";
/// The cache time-to-live for the IndexedDB cache is 2 days.
const INDEXED_DB_CACHE_TTL: u32 = 60 * 60 * 24 * 2; // 2 days in seconds

thread_local! {
    static LAYER8_LIGHT_SAIL_URL: Cell<String> = Cell::new("".to_string());
    static COUNTER: Cell<u32> = const { Cell::new(0) };
    static ENCRYPTED_TUNNEL_FLAG: Cell<bool> = const { Cell::new(false) };
    static PRIVATE_JWK_ECDH: Cell<Option<crypto::Jwk>> = const { Cell::new(None) };
    static PUB_JWK_ECDH:  Cell<Option<crypto::Jwk>> = const { Cell::new(None) };
    static USER_SYMMETRIC_KEY: Cell<Option<crypto::Jwk> >= const { Cell::new(None) };
    static UP_JWT: Cell<String> = Cell::new("".to_string());
    static UUID: Cell<String> = Cell::new("".to_string());
    static STATIC_PATH: Cell<String> = Cell::new("".to_string());
    static L8_CLIENTS: Cell<HashMap<String, types::Client>> = Cell::new(HashMap::new());

    /// The cache instantiates with the `_layer8cache` IndexedDB.
    static INDEXED_DBS: HashMap<String, types::DbCache> = HashMap::from([
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
pub async fn get_static(url: JsValue) -> Promise {
    let req_url = match url.is_string() {
        true => Url::parse(&url.as_string().unwrap()).unwrap(),
        false => {
            return Promise::reject(&JsError::new("Invalid url provided to fetch call").into());
        }
    };

    let client = L8_CLIENTS.with(|v| {
        let val = v.take();
        v.replace(val.clone());
        let parsed_uri = format!("{}://{}", req_url.scheme(), req_url.host().unwrap());
        val.get(&parsed_uri).cloned()
    });

    let static_path = STATIC_PATH.with(|v| {
        let val = v.take();
        v.replace(val.clone());
        val
    });

    let host = format!("{}{}", req_url.host().unwrap(), static_path);
    let json_body = format!(
        "{{\"__url_path\": \"{}\"}}",
        req_url.as_str().replacen(&host, "", 1)
    );

    let req = types::Request {
        method: "GET".to_string(),
        headers: HashMap::new(),
        body: json_body.as_bytes().to_vec(),
    };

    let symmetric_key = {
        let symmetric_key = USER_SYMMETRIC_KEY.with(|v| {
            let val = v.take();
            v.replace(val.clone());
            val
        });

        match symmetric_key {
            Some(key) => key,
            None => {
                return Promise::reject(&JsError::new("symmetric key not found.").into());
            }
        }
    };

    let res = {
        let up_jwt = UP_JWT.with(|v| {
            let val = v.take();
            v.replace(val.clone());
            val
        });

        let uuid = UUID.with(|v| {
            let val = v.take();
            v.replace(val.clone());
            val
        });

        let res = client
            .unwrap()
            .r#do(&req, &symmetric_key, req_url.as_str(), true, &up_jwt, &uuid)
            .await;
        match res {
            Ok(val) => val,
            Err(e) => {
                return Promise::reject(&JsError::new(&e).into());
            }
        }
    };

    let file_type = {
        let file_type = res
            .headers
            .iter()
            .find(|(_, v)| v.trim().eq_ignore_ascii_case("Content-Type"));

        match file_type {
            Some(val) => val.1.clone(),
            None => {
                return Promise::reject(&JsError::new("Content-Type header not found.").into());
            }
        }
    };

    let object_url = serve_static(
        INDEXED_DB_CACHE,
        &res.body,
        &file_type,
        req_url.as_str(),
        INDEXED_DB_CACHE_TTL,
    );

    Promise::resolve(&JsValue::from(object_url))
}

#[wasm_bindgen(js_name = checkEncryptedTunnel)]
pub fn check_encrypted_tunnel() -> Promise {
    Promise::resolve(&JsValue::from(ENCRYPTED_TUNNEL_FLAG.get()))
}

#[wasm_bindgen]
pub async fn fetch(url: JsValue, args: js_sys::Array) -> Promise {
    if !ENCRYPTED_TUNNEL_FLAG.get() {
        return Promise::reject(&JsError::new("Encrypted tunnel is closed. Reload page.").into());
    }

    let req_url = match url.is_string() {
        true => url.as_string().unwrap(),
        false => {
            return Promise::reject(&JsError::new("Invalid url provided to fetch call").into());
        }
    };

    let mut req = types::Request {
        ..Default::default()
    };

    let options = if args.length() > 0 {
        args.pop()
    } else {
        JsValue::null()
    };

    if !options.is_null() && !options.is_undefined() {
        // the options object is expected to be a dictionary
        let options = match Object::try_from(&options) {
            Some(options) => options,
            None => {
                return Promise::reject(&JsError::new("Invalid options object provided.").into());
            }
        };

        // [[key, value], ...] result from Object.entries
        let entries = object_entries(options);

        // let's find the method, if none is provided, we default to "GET"
        entries.find(&mut |entry, _, _| {
            // [key, value] item array
            let key_value_entry = js_sys::Array::from(&entry);
            if key_value_entry.get(0).is_null()
                || key_value_entry.get(0).is_undefined()
                || !key_value_entry.get(0).is_string()
            {
                return false;
            }

            if key_value_entry
                .get(0)
                .as_string()
                .expect("key is a string; qed")
                .eq_ignore_ascii_case("method")
            {
                req.method = key_value_entry
                    .get(1)
                    .as_string()
                    .unwrap_or("GET".to_string());
                return true;
            }

            false
        });

        // let's find the headers, if none is provided, we leave as an empty hashmap
        entries.find(&mut |entry, _, _| {
            // [key, value] item array
            let key_value_entry = js_sys::Array::from(&entry);
            if key_value_entry.get(0).is_null()
                || key_value_entry.get(0).is_undefined()
                || !key_value_entry.get(0).is_string()
            {
                return false;
            }

            if key_value_entry
                .get(0)
                .as_string()
                .expect("key is a string; qed")
                .eq_ignore_ascii_case("headers")
            {
                // [[key, value], ...] result from Object.entries
                let headers = object_entries(
                    Object::try_from(&key_value_entry.get(1))
                        .expect("expected headers to be a [key, val] object array; qed"),
                );

                headers.iter().for_each(|header| {
                    let header_entries = js_sys::Array::from(&header);
                    req.headers.insert(
                        header_entries
                            .get(0)
                            .as_string()
                            .expect("key is a string; qed"),
                        header_entries
                            .get(1)
                            .as_string()
                            .expect("value is a string; qed"),
                    );
                });

                return true;
            }

            false
        });

        // if content type is not provided, we default to "application/json"
        if req
            .headers
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case("Content-Type"))
        {
            req.headers
                .insert("Content-Type".to_string(), "application/json".to_string());
        }

        // let's get the body
        entries.find(&mut |entry, _, _| {
            // [key, value] item array
            let key_value_entry = js_sys::Array::from(&entry);
            if key_value_entry.get(0).is_null()
                || key_value_entry.get(0).is_undefined()
                || !key_value_entry.get(0).is_string()
            {
                return false;
            }

            if key_value_entry
                .get(0)
                .as_string()
                .expect("key is a string; qed")
                .eq_ignore_ascii_case("body")
            {
                req.body = key_value_entry
                    .get(1)
                    .as_string()
                    .expect("value is a string; qed")
                    .as_bytes()
                    .to_vec();
                return true;
            }

            false
        });

        for entry in entries.iter() {
            let val = js_sys::Array::from(&entry);
            match val
                .get(0)
                .as_string()
                .expect("key is a string; qed")
                .as_str()
            {
                "body" => {
                    let body = &val.get(1);

                    if body.is_null() {}

                    // res.body = body.to_vec();
                }

                _ => {
                    return Promise::reject(
                        &JsError::new("Invalid options object provided.").into(),
                    );
                }
            }
        }
    }

    let l8_client_res = L8_CLIENTS.with(|v| {
        let val = v.take();
        v.replace(val.clone());
        val.get(&req_url).cloned()
    });

    let client = match l8_client_res {
        Some(client) => client,
        None => {
            return Promise::reject(&JsError::new("client could not be found").into());
        }
    };

    let res = match req
        .headers
        .get("Content-Type")
        .map(|v| v.as_str())
        .unwrap_or("application/json")
    {
        "application/json" => {
            let symmetric_key = USER_SYMMETRIC_KEY.with(|v| {
                let val = v.take();
                v.replace(val.clone());
                val
            });

            let symmetric_key = match symmetric_key {
                Some(key) => key,
                None => {
                    return Promise::reject(&JsError::new("symmetric key not found.").into());
                }
            };

            let up_jwt = UP_JWT.with(|v| {
                let val = v.take();
                v.replace(val.clone());
                val
            });

            let uuid = UUID.with(|v| {
                let val = v.take();
                v.replace(val.clone());
                val
            });

            match client
                .r#do(&req, &symmetric_key, &req_url, false, &up_jwt, &uuid)
                .await
            {
                Ok(res) => types::Response {
                    body: res.body,
                    headers: {
                        let mut header_map = Vec::new();
                        for (key, value) in req.headers.iter() {
                            header_map.push((key.clone(), value.clone()));
                        }
                        header_map
                    },
                    status: 200,
                    ..Default::default()
                },
                Err(e) => {
                    return Promise::reject(&JsError::new(&e).into());
                }
            }
        }
        "multipart/form-data" => {
            // req.body = new FormData(req.body);
            types::Response {
                ..Default::default() // todo: implement
            }
        }

        // unimplemented, making the compiler happy
        _ => types::Response {
            ..Default::default() // todo: implement
        },
    };

    if res.status >= 100 && res.status < 300 {
        todo!()
    }

    Promise::resolve(&JsValue::null()) //
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
    let counter = COUNTER.with(|v| {
        v.set(v.get() + 1);
        v.get()
    });

    console_log(&format!("WASM Counter: {}", counter));
    Promise::resolve(&JsValue::from(counter))
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
                STATIC_PATH.with(|v| {
                    let path = val.get(1).as_string().expect("staticPath is a string; qed");
                    v.replace(path);
                });
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
    let (pivate_jwk_ecdh, pub_jwk_ecdh) = generate_key_pair(crypto::KeyUse::Ecdh)?;
    PRIVATE_JWK_ECDH.with(|v| {
        v.set(Some(pivate_jwk_ecdh.clone()));
    });

    let b64_pub_jwk = pub_jwk_ecdh.export_as_base64()?;

    let proxy = format!("{proxy}/init-tunnel?backend={provider}");

    let res = reqwest::Client::new()
        .post(&proxy)
        .body(b64_pub_jwk.clone())
        .headers({
            let mut headers = reqwest::header::HeaderMap::new();
            let uuid = {
                let mut small_rng = SmallRng::from_entropy();
                small_rng.next_u32().to_string()
            };
            UUID.set(uuid.clone());

            headers.insert(
                "x-ecdh-init",
                HeaderValue::from_str(&b64_pub_jwk).expect(""),
            );
            headers.insert("x-client-uuid", HeaderValue::from_str(&uuid).expect(""));
            headers
        })
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if res.status().eq(&401) {
        ENCRYPTED_TUNNEL_FLAG.with(|v| {
            v.set(false);
        });
        return Err(String::from(
            "401 response from proxy, user is not authorized.",
        ));
    }

    let mut proxy_data: HashMap<String, serde_json::Value> =
        serde_json::from_slice(&res.bytes().await.map_err(|e| e.to_string())?).unwrap();

    UP_JWT.set(
        proxy_data
            .remove("up_jwt")
            .expect("expected up_jwt to be present; qed")
            .as_str()
            .expect("expected up_jwt to be a string; qed")
            .to_string(),
    );
    USER_SYMMETRIC_KEY.replace(Some(jwk_from_map(proxy_data)?));
    ENCRYPTED_TUNNEL_FLAG.replace(true);

    let proxy_url = Url::parse(&proxy).map_err(|e| e.to_string())?;
    let port = if proxy_url.port().is_none() {
        "443"
    } else {
        "80"
    };

    let provider_client = new_client(&format!(
        "{}://{}:{}",
        proxy_url.scheme(),
        proxy_url.host().unwrap(),
        port
    ))
    .map_err(|e| {
        ENCRYPTED_TUNNEL_FLAG.set(false);
        e.to_string()
    })?;

    L8_CLIENTS.with(|v| {
        let mut map = v.take();
        map.insert(provider.to_string(), provider_client);
        v.replace(map);
    });

    Ok(console_log(&format!(
        "Encrypted tunnel established with provider: {}",
        provider
    )))
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
