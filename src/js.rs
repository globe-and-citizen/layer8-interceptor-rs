use std::{cell::Cell, collections::HashMap};

use base64::{self, engine::general_purpose::URL_SAFE as base64_enc_dec, Engine as _};
use js_sys::{ArrayBuffer, Object, Promise, Uint8Array};
use reqwest::header::HeaderValue;
use serde_json::{json, Value};
use url::Url;
use uuid::Uuid;
use wasm_bindgen::prelude::*;
use web_sys::{FormData, Response, ResponseInit};

use crate::crypto::{self, generate_key_pair, jwk_from_map};
use crate::types::{self, new_client};

/// This block imports Javascript functions that are provided by the JS Runtime.
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn console_log(s: &str);

    #[wasm_bindgen(js_namespace = console, js_name = error)]
    fn console_error(s: &str);

    #[wasm_bindgen(js_namespace = Object, js_name = entries)]
    fn object_entries(obj: &Object) -> js_sys::Array;

    #[wasm_bindgen(js_namespace = Object)]
    fn get_prototype_of(obj: &JsValue) -> JsValue;

    #[wasm_bindgen(js_namespace = Function, js_name = toString)]
    fn to_string(func: &JsValue) -> JsValue;
}

/// This block imports JavaScript functionality that is not mapped by the wasm-bindgen tool.
#[wasm_bindgen(module = "/src/js/indexed_db.js")]
extern "C" {
    fn open_db(db_name: &str, db_cache: types::DbCache);
    fn clear_expired_cache(db_name: &str, db_cache: types::DbCache);
    fn serve_static(db_name: &str, body: &[u8], file_type: &str, url: &str, exp_in_seconds: u32) -> String;
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

#[wasm_bindgen(js_name = static_)]
pub async fn get_static(url: JsValue) -> Promise {
    let req_url = match url.is_string() {
        true => Url::parse(&url.as_string().unwrap()).unwrap(),
        false => {
            return Promise::reject(&JsError::new("Invalid url provided to fetch call").into());
        }
    };

    console_log(&format!("Fetching static file: {}", req_url.as_str()));

    let client = L8_CLIENTS.with(|v| {
        let val = v.take();
        v.replace(val.clone());
        console_log(&format!("Parsed URI: {}", &rebuild_url(req_url.as_ref())));
        val.get(&rebuild_url(req_url.as_ref())).cloned()
    });

    console_log(&format!("Client: {:?}", client));

    let static_path = STATIC_PATH.with(|v| {
        let val = v.take();
        v.replace(val.clone());
        val
    });

    let host = format!("{}{}", req_url.host().unwrap(), static_path);
    let json_body = format!("{{\"__url_path\": \"{}\"}}", req_url.as_str().replacen(&host, "", 1));

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

        let res = client.unwrap().r#do(&req, &symmetric_key, req_url.as_str(), true, &up_jwt, &uuid).await;
        match res {
            Ok(val) => val,
            Err(e) => {
                return Promise::reject(&JsError::new(&e).into());
            }
        }
    };

    let file_type = {
        let file_type = res.headers.iter().find(|(_, v)| v.trim().eq_ignore_ascii_case("Content-Type"));

        match file_type {
            Some(val) => val.1.clone(),
            None => {
                return Promise::reject(&JsError::new("Content-Type header not found.").into());
            }
        }
    };

    let object_url = serve_static(INDEXED_DB_CACHE, &res.body, &file_type, req_url.as_str(), INDEXED_DB_CACHE_TTL);

    Promise::resolve(&JsValue::from(object_url))
}

#[allow(non_snake_case)]
#[wasm_bindgen(js_name = checkEncryptedTunnel)]
pub fn check_encrypted_tunnel() -> Promise {
    Promise::resolve(&JsValue::from(ENCRYPTED_TUNNEL_FLAG.get()))
}

#[wasm_bindgen]
pub async fn fetch(url: JsValue, args: JsValue) -> Promise {
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
        method: "GET".to_string(),
        ..Default::default()
    };

    let mut js_body = JsValue::null();
    if !args.is_null() && !args.is_undefined() {
        // the options object is expected to be a dictionary
        let options = match Object::try_from(&args) {
            Some(options) => options,
            None => {
                return Promise::reject(&JsError::new("Invalid options object provided.").into());
            }
        };

        // [[key, value], ...] result from Object.entries
        let entries = object_entries(options);

        for entry in entries.iter() {
            // [key, value] item array
            let key_value_entry = js_sys::Array::from(&entry);
            if key_value_entry.get(0).is_null() || key_value_entry.get(0).is_undefined() || !key_value_entry.get(0).is_string() {
                continue;
            }

            let key = key_value_entry.get(0).as_string().expect("key is a string; qed");

            match key.to_lowercase().as_str() {
                "method" => {
                    req.method = key_value_entry.get(1).as_string().unwrap_or("GET".to_string());
                }
                "headers" => {
                    let headers =
                        object_entries(Object::try_from(&key_value_entry.get(1)).expect("expected headers to be a [key, val] object array; qed"));

                    headers.iter().for_each(|header| {
                        let header_entries = js_sys::Array::from(&header);
                        req.headers.insert(
                            header_entries.get(0).as_string().expect("key is a string; qed"),
                            header_entries.get(1).as_string().expect("value is a string; qed"),
                        );
                    });
                }
                "body" => {
                    js_body = key_value_entry.get(1);
                    if !js_body.is_null() && !js_body.is_undefined() && js_body.is_instance_of::<FormData>() {
                        req.headers.insert("Content-Type".to_string(), "multipart/form-data".to_string());
                    }
                }
                _ => {}
            }
        }

        // if content type is not provided, we default to "application/json"
        if !req.headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("Content-Type")) {
            req.headers.insert("Content-Type".to_string(), "application/json".to_string());
        }
    }

    let l8_client_res = L8_CLIENTS.with(|v| {
        let val = v.take();
        v.replace(val.clone());
        val.get(&rebuild_url(&req_url)).cloned()
    });

    let client = match l8_client_res {
        Some(client) => client,
        None => {
            return Promise::reject(&JsError::new("client could not be found").into());
        }
    };

    let content_type = req
        .headers
        .get("Content-Type")
        .map(|v| v.as_str())
        .unwrap_or("application/json")
        .to_lowercase();

    let res = match &content_type[..] {
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

            // the js_body is expected to be a valid json string
            if !js_body.is_null() && !js_body.is_undefined() {
                req.body = js_body.as_string().expect("expected body to be a string; qed").as_bytes().to_vec();
            }

            match client.r#do(&req, &symmetric_key, &req_url, true, &up_jwt, &uuid).await {
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
            req.headers
                .insert("Content-Type".to_string(), "application/layer8.buffer+json".to_string());

            if js_body.is_null() || js_body.is_undefined() {
                return Promise::reject(&JsError::new("No body provided to fetch call.").into());
            }

            let req_url = {
                let req_url_ = url::Url::parse(&req_url).expect("expected url to be a valid URL; qed");
                req_url.clone().replacen(&req_url_.host().unwrap().to_string(), "", 1)
            };

            // populating the form data from the body
            let form_data = match construct_form_data(&js_body, &req_url) {
                Ok(val) => val,
                Err(e) => {
                    return Promise::reject(&JsError::new(&e).into());
                }
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

            req.body = serde_json::to_vec(&form_data).unwrap();
            match client.r#do(&req, &symmetric_key, &req_url, false, &up_jwt, &uuid).await {
                Ok(res) => res,
                Err(e) => {
                    return Promise::reject(&JsError::new(&e).into());
                }
            }
        }

        _ => types::Response {
            status: 400,
            status_text: "Content-Type not supported".to_string(),
            ..Default::default()
        },
    };

    if res.status >= 100 && res.status < 300 {
        let response_init = ResponseInit::new();
        let headers = web_sys::Headers::new().unwrap();
        for (key, value) in res.headers.iter() {
            headers
                .append(key, value)
                .expect("expected headers to be appended to the web_sys::Headers object; qed");
        }

        response_init.set_headers(&headers);
        response_init.set_status(res.status);
        response_init.set_status_text(&res.status_text);

        let mut body = res.body;
        let response = match Response::new_with_opt_u8_array_and_init(Some(&mut body), &response_init) {
            Ok(val) => val,
            Err(e) => {
                return Promise::reject(&JsError::new(&format!("{:?}", e)).into());
            }
        };

        return Promise::resolve(&response);
    }

    console_error(&format!("Fetch failed with status: {}, statusText: {}", res.status, res.status_text));

    Promise::reject(&JsError::new(&res.status_text).into())
}

fn construct_form_data(js_body: &JsValue, url_path: &str) -> Result<HashMap<String, Value>, String> {
    let js_body_object = Object::try_from(js_body).expect("expected body to be an object; qed");
    let js_body_entries = object_entries(js_body_object);

    let mut form_data = HashMap::from([(
        "__url_path".to_string(),
        serde_json::to_value(HashMap::from([(
            "_type".to_string(),
            json!({
                "_type": "String",
                "value": url_path.to_string(),
            }),
        )]))
        .unwrap(),
    )]);
    for entry in js_body_entries.entries() {
        // [key, value] item array
        let entry = {
            let entry = entry.expect("expected entry to be an array of [key, value] item array; qed");

            if entry.is_null() || entry.is_undefined() {
                // we skip null or undefined entries if any
                continue;
            }

            let entry_object = Object::try_from(&entry).expect("expected entry to be an object; qed");
            object_entries(entry_object)
        };

        let key = entry.get(0).as_string().expect("expected key to be a string; qed");
        let value = entry.get(1);

        let reflect_get = js_sys::Reflect::get;

        let data = match get_constructor_name(&value).as_str() {
            "File" => json!({
                "_type": "File",
                "name": reflect_get(&value, &JsValue::from("name")).expect("expected name to be present; qed").as_string().
                    expect("expected name to be a string; qed"),
                "size": reflect_get(&value, &JsValue::from("size")).expect("expected size to be present; qed").as_f64().
                    expect("expected size to be a number; qed"),
                "type": reflect_get(&value, &JsValue::from("type")).expect("expected type to be present; qed").as_string().
                    expect("expected type to be a string; qed"),
                "buff": base64_enc_dec.encode(Uint8Array::new(&ArrayBuffer::from(value)).to_vec())
            }),
            "String" => json!({
                "_type": "String",
                "value": value.as_string().unwrap(),
            }),
            "Number" => json!({
                "_type": "Number",
                "value": value.as_f64().unwrap(),
            }),
            "Boolean" => json!({
                "_type": "Boolean",
                "value": value.as_bool().unwrap(),
            }),
            x => return Err(format!("Unsupported value type: {} for key: {}", x, key)),
        };

        if let Some(old_value) = form_data.get(&key) {
            // convert the old value to a hashmap
            let old_value =
                serde_json::from_value::<HashMap<String, Value>>(old_value.clone()).expect("valid json can be converted to a hashmap; qed");
            let mut new_value =
                serde_json::from_value::<HashMap<String, Value>>(data.clone()).expect("valid json can be converted to a hashmap; qed");

            // merge the old value with the new value
            new_value.extend(old_value);
            form_data.insert(key, serde_json::to_value(new_value).unwrap());
            continue;
        }

        // overwrite the form data key
        form_data.insert(key, serde_json::to_value(data).unwrap());
    }

    Ok(form_data)
}

fn get_constructor_name(obj: &JsValue) -> String {
    // Get the prototype of the object
    let prototype = get_prototype_of(obj);

    // Get the constructor from the prototype using Reflect.get
    let constructor = js_sys::Reflect::get(&prototype, &JsValue::from("constructor")).expect("expected constructor to be present; qed");

    // Check if the constructor is a function
    if constructor.is_function() {
        // Convert the function to string and extract the name
        let constructor_str = to_string(&constructor);
        let constructor_name = constructor_str.as_string().unwrap_or_default();

        // Extract the name from the function string
        if let Some(name_start) = constructor_name.find("function ") {
            let name_end = constructor_name.find("(").unwrap_or(constructor_name.len());
            let name = &constructor_name[name_start + 9..name_end].trim(); // Skip "function "
            return name.to_string();
        }
    }

    "undefined".to_string()
}

/// Test promise resolution/rejection from the console.
#[allow(non_snake_case)]
#[wasm_bindgen(js_name = testWASM)]
pub fn test_wasm(arg: JsValue) -> Promise {
    if arg.is_null() || arg.is_undefined() {
        let err = JsError::new("The argument is null or undefined.");
        return Promise::reject(&err.into());
    }

    let msg = format!("WASM Interceptor version {INTERCEPTOR_VERSION} successfully loaded. Argument passed: {:?}. To test promise rejection, call with no argument.", arg);
    Promise::resolve(&JsValue::from(msg))
}

#[allow(non_snake_case)]
#[wasm_bindgen(js_name = persistenceCheck)]
pub fn persistence_check() -> Promise {
    let counter = COUNTER.with(|v| {
        v.set(v.get() + 1);
        v.get()
    });

    console_log(&format!("WASM Counter: {}", counter));
    Promise::resolve(&JsValue::from(counter))
}

#[allow(non_snake_case)]
#[wasm_bindgen(js_name = initEncryptedTunnel)]
pub async fn init_encrypted_tunnel(this_: js_sys::Object, args: js_sys::Array) -> JsValue {
    let mut providers = Vec::new();
    // let mut proxy = "https://layer8devproxy.net".to_string(); // set LAYER8_PROXY in the environment to override
    let mut proxy = "http://localhost:5001".to_string();
    let mut mode = "prod".to_string();
    if args.length() > 1 {
        mode = args.as_string().expect("the mode expected to be a string; qed");
    }

    let cache = INDEXED_DBS.with(|v| {
        let val = v.get(INDEXED_DB_CACHE).expect("expected indexed db to be present; qed");
        val.clone()
    });

    clear_expired_cache(INDEXED_DB_CACHE, cache);

    let mut error_destructuring = false;
    let entries = object_entries(&this_);
    for entry in entries.iter() {
        let val = js_sys::Array::from(&entry); // [key, value] result from Object.entries
        match val.get(0).as_string().expect("key is a string; qed").as_str() {
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
        console_log("Error: unable to destructure the layer8 encrypted object.");
        return Promise::reject(&err.into()).into();
    }

    for provider in providers.clone() {
        console_log(&format!("Establishing encrypted tunnel with provider: {}", provider));
        if let Err(err) = init_tunnel(&provider, &proxy).await {
            console_error(&format!(
                "Failed to establish encrypted tunnel with provider: {}. Error: {}",
                provider, err
            ));
            let err = JsError::new(&err);
            return Promise::reject(&err.into()).into();
        } else {
            console_log(&format!("Encrypted tunnel established with provider: {}", provider));
        }
    }

    console_log(&format!("Encrypted tunnel established with providers: {:?}", providers));
    Promise::resolve(&JsValue::null()).into()
}

async fn init_tunnel(provider: &str, proxy: &str) -> Result<(), String> {
    let _provider_url = rebuild_url(provider);
    let (private_jwk_ecdh, pub_jwk_ecdh) = generate_key_pair(crypto::KeyUse::Ecdh)?;

    PRIVATE_JWK_ECDH.with(|v| {
        v.set(Some(private_jwk_ecdh.clone()));
    });

    let b64_pub_jwk = pub_jwk_ecdh.export_as_base64();

    let proxy = format!("{proxy}/init-tunnel?backend={provider}");

    let res = reqwest::Client::new()
        .post(&proxy)
        .body(b64_pub_jwk.clone())
        .headers({
            let mut headers = reqwest::header::HeaderMap::new();
            let uuid = Uuid::new_v4().to_string();
            UUID.set(uuid.clone());

            headers.insert("x-ecdh-init", HeaderValue::from_str(&b64_pub_jwk).expect(""));
            headers.insert("x-client-uuid", HeaderValue::from_str(&uuid).expect(""));
            headers
        })
        .send()
        .await
        .map_err(|e| {
            console_log(&format!("Failed to send request: {}", e));
            e.to_string()
        })?;

    if res.status().eq(&401) {
        ENCRYPTED_TUNNEL_FLAG.with(|v| {
            v.set(false);
        });
        return Err(String::from("401 response from proxy, user is not authorized."));
    }

    let mut proxy_data: serde_json::Map<String, serde_json::Value> = serde_json::from_slice(&res.bytes().await.map_err(|e| {
        console_log(&format!("Failed to read response: {}", e));
        e.to_string()
    })?)
    .map_err(|val| {
        console_log(&format!("Failed to decode response: {}", val));
        val.to_string()
    })?;

    UP_JWT.set(
        proxy_data
            .remove("up-JWT")
            .ok_or("up_jwt not found")?
            .as_str()
            .ok_or("expected up_jwt to be a string")?
            .to_string(),
    );

    USER_SYMMETRIC_KEY.replace(Some(private_jwk_ecdh.get_ecdh_shared_secret(&jwk_from_map(proxy_data)?)?));
    ENCRYPTED_TUNNEL_FLAG.replace(true);

    let proxy_url = Url::parse(&proxy).map_err(|e| e.to_string())?;

    let url_proxy_ = &format!(
        "{}://{}:{}",
        proxy_url.scheme(),
        proxy_url.host().expect("expected host to be present; qed"),
        proxy_url.port().unwrap_or(443)
    );

    let provider_client = new_client(url_proxy_).map_err(|e| {
        ENCRYPTED_TUNNEL_FLAG.set(false);
        e.to_string()
    })?;

    L8_CLIENTS.with(|v| {
        let mut map = v.take();
        map.insert(provider.to_string(), provider_client);
        v.replace(map);
    });

    Ok(console_log(&format!("Encrypted tunnel established with provider: {}", provider)))
}

fn rebuild_url(url: &str) -> String {
    let url = url::Url::parse(url).expect("expected provider to be a valid URL; qed");
    let mut rebuilt_url = url.scheme().to_string() + "://" + url.host_str().expect("expected host to be present; qed");
    if let Some(port) = url.port() {
        rebuilt_url.push_str(&format!(":{}", port.to_string().as_str()));
    }

    rebuilt_url
}
