use std::cell::RefCell;
use std::{cell::Cell, collections::HashMap};

use js_sys::{Array, Object, Uint8Array};
use layer8_primitives::compression::{compress_gzip_and_encode_b64, decompress_data_gzip};
use reqwest::header::HeaderValue;
use serde_json::{json, Value};
use url::Url;
use uuid::Uuid;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{File, FormData, Response, ResponseInit};

use layer8_primitives::crypto::{self, generate_key_pair, jwk_from_map};
use layer8_primitives::types::{self, new_client};

use crate::js_glue::js_imports::check_if_asset_exists;
use crate::js_imports_prelude::*;
use crate::types::{DbCache, InitConfig, Uniqueness, CACHE_STORAGE_LIMIT};

const INTERCEPTOR_VERSION: &str = "0.0.14";
const INDEXED_DB_CACHE: &str = "_layer8cache";
/// The cache time-to-live for the IndexedDB cache is 2 days.
const INDEXED_DB_CACHE_TTL: i32 = 60 * 60 * 24 * 2 * 1000; // 2 days in milliseconds

thread_local! {
    pub(crate) static PUB_JWK_ECDH:  Cell<Option<crypto::Jwk>> = const { Cell::new(None) };
    pub(crate) static PRIVATE_JWK_ECDH: Cell<Option<crypto::Jwk>> = const { Cell::new(None) };
    pub(crate) static USER_SYMMETRIC_KEY: RefCell<Option<crypto::Jwk> >= const { RefCell::new(None) };
    pub(crate) static UP_JWT: Cell<String> = Cell::new("".to_string());
    pub(crate) static ENCRYPTED_TUNNEL_FLAG: Cell<bool> = const { Cell::new(false) };
    pub(crate) static UUID: Cell<String> = Cell::new("".to_string());

    // static LAYER8_LIGHT_SAIL_URL: Cell<String> = Cell::new("".to_string());
    static COUNTER: Cell<i32> = const { Cell::new(0) };

    static STATIC_PATHS: Cell<Vec<String>> = const { Cell::new(vec![]) };
    static L8_CLIENTS: Cell<HashMap<String, types::Client>> = Cell::new(HashMap::new());

    /// The cache instantiates with the `_layer8cache` IndexedDB.
    static INDEXED_DBS: HashMap<String, crate::types::DbCache> = HashMap::from([
        (
            INDEXED_DB_CACHE.to_string(),
            DbCache {
                store: "static".to_string(),
                key_path: "url".to_string(),
                indexes: crate::types::Indexes{
                    url: Uniqueness { unique: true },
                    _exp: Uniqueness { unique: false },
                    body: Uniqueness { unique: false },
                    _type: Uniqueness { unique: false },
                }
            },
        )
    ]);
}

/// This function is called to retrieve the static file.
/// It is expected to be called with a URL string.
///
/// The alias for this function is `static`, for backwards compatibility purposes.
#[wasm_bindgen(js_name = _static)]
pub async fn get_static(url: String) -> Result<String, JsError> {
    if url.is_empty() {
        return Err(JsError::new("Invalid url provided to fetch call"));
    }

    match check_if_asset_exists(INDEXED_DB_CACHE, &url).await {
        Ok(val) => {
            if let Some(val) = val.as_string() {
                if !val.is_empty() {
                    // if file is in cache, short-circuit
                    return Ok(val);
                }
            }
        }
        Err(e) => {
            console_log!(&format!("error is {:?}", e));
            return Err(JsError::new(&format!(
                "Error occurred interacting with IndexDB: {}",
                e.as_string().unwrap_or("error unwrappable".to_string())
            )));
        }
    };

    let static_paths = STATIC_PATHS.with(|v| {
        let val = v.take();
        v.replace(val.clone());
        val
    });

    let json_body = format!("{{\"__url_path\": \"{}\"}}", url);
    let req_url = rebuild_url(&url);

    let client = L8_CLIENTS.with(|v| {
        let val = v.take();
        v.replace(val.clone());
        val.get(&req_url).cloned()
    });

    let mut assets_glob_url = req_url.clone();
    for static_path in static_paths.iter() {
        if url.contains(static_path) {
            assets_glob_url.push_str(static_path);
            break;
        }
    }

    console_log!(&format!("Request URL: {}", req_url));

    let req = types::Request {
        method: "GET".to_string(),
        headers: HashMap::from([("content-type".to_string(), "application/json".to_string())]),
        body: json_body.as_bytes().to_vec(),
        url_path: Some(assets_glob_url.clone()),
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
                return Err(JsError::new("symmetric key not found."));
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
            .expect_throw("client could not be found. This is likely due to the encrypted tunnel not being established correctly.")
            .r#do(&req, &symmetric_key, &req_url, true, &up_jwt, &uuid)
            .await;
        match res {
            Ok(val) => val,
            Err(e) => {
                console_log!(&format!(
                    "Failed to fetch: {}\nWith request {:?}\nWith headers {:?}",
                    e,
                    String::from_utf8_lossy(&req.body),
                    req.headers
                ));
                return Err(JsError::new(&e));
            }
        }
    };

    let file_type = {
        let file_type = res.headers.iter().find(|(k, _)| k.trim().eq_ignore_ascii_case("Content-Type"));

        match file_type {
            Some(val) => val.1.clone(),
            None => {
                return Err(JsError::new("Content-Type header not found."));
            }
        }
    };

    // decompress the file if we compressed it
    let body = match decompress_data_gzip(&res.body) {
        Ok(val) => {
            console_log!("File decompressed successfully");
            val
        }
        Err(e) => {
            if e.eq("invalid gzip header") {
                res.body
            } else {
                return Err(JsError::new(&format!("Error occurred decompressing file: {}", e)));
            }
        }
    };

    let object_url = match serve_static(
        INDEXED_DB_CACHE,
        &body,
        CACHE_STORAGE_LIMIT.with(|v| v.get()),
        &file_type,
        &url,
        INDEXED_DB_CACHE_TTL,
    )
    .await
    {
        Ok(val) => val.as_string().expect_throw("expected object url to be a string").to_string(),
        Err(e) => {
            return Err(JsError::new(&format!(
                "Error occurred interacting with IndexDB: {}",
                e.as_string().unwrap_or("error unwrappable".to_string())
            )));
        }
    };

    console_log!(&format!("Object URL: {:?}", object_url));
    Ok(object_url)
}

/// This function is called to check if the encrypted tunnel is open.
/// Returning a boolean value.
#[allow(non_snake_case)]
#[wasm_bindgen(js_name = checkEncryptedTunnel)]
pub async fn check_encrypted_tunnel() -> bool {
    ENCRYPTED_TUNNEL_FLAG.get()
}

/// This function is an override of the fetch function. It's arguments are a URL and an options object.
#[wasm_bindgen]
pub async fn fetch(url: String, options: JsValue) -> Result<Response, JsError> {
    if !ENCRYPTED_TUNNEL_FLAG.get() {
        return Err(JsError::new("Encrypted tunnel is closed. Reload page."));
    }

    let mut req = types::Request {
        method: "GET".to_string(),
        url_path: Some(url.clone()),
        ..Default::default()
    };

    let mut js_body = JsValue::null();
    if !options.is_null() && !options.is_undefined() {
        let options = Object::from(options);
        // [[key, value], ...] result from Object.entries
        let entries = object_entries(&options);

        for entry in entries.iter() {
            // [key, value] item array
            let key_value_entry = js_sys::Array::from(&entry);
            let key = key_value_entry.get(0);
            let value = key_value_entry.get(1);
            if key.is_null() || key.is_undefined() || !key.is_string() {
                continue;
            }

            let key = key.as_string().expect_throw("key is a string; qed");

            match key.to_lowercase().as_str() {
                "method" => {
                    req.method = value.as_string().unwrap_or("GET".to_string());
                }
                "headers" => {
                    let headers = object_entries(Object::try_from(&value).expect_throw("expected headers to be a [key, val] object array; qed"));

                    headers.iter().for_each(|header| {
                        let header_entries = js_sys::Array::from(&header);
                        req.headers.insert(
                            header_entries.get(0).as_string().expect_throw("key is a string; qed"),
                            header_entries.get(1).as_string().expect_throw("value is a string; qed"),
                        );
                    });
                }
                "body" => {
                    js_body = value;
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

    let backend_url = rebuild_url(&url);
    let client = {
        let l8_clients = L8_CLIENTS.with(|v| {
            let val = v.take();
            v.replace(val.clone());
            val
        });

        match l8_clients.get(&backend_url).cloned() {
            Some(client) => client,
            None => {
                return Err(JsError::new("client could not be found"));
            }
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
                    return Err(JsError::new("symmetric key not found."));
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
                req.body = js_body.as_string().expect_throw("expected body to be a string; qed").as_bytes().to_vec();
            }

            req.url_path = Some(url.clone());
            match client.r#do(&req, &symmetric_key, &backend_url, true, &up_jwt, &uuid).await {
                Ok(res) => res,
                Err(e) => {
                    console_log!(&format!("Failed to fetch: {}\nWith request {:?}", e, req));

                    return Err(JsError::new(&e));
                }
            }
        }
        "multipart/form-data" => {
            req.headers
                .insert("Content-Type".to_string(), "application/layer8.buffer+json".to_string());

            if js_body.is_null() || js_body.is_undefined() {
                return Err(JsError::new("No body provided to fetch call."));
            }

            // populating the form data from the body
            let form_data = match construct_form_data(&js_body).await {
                Ok(val) => val,
                Err(e) => {
                    return Err(JsError::new(&e));
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
                        return Err(JsError::new("symmetric key not found."));
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
            match client.r#do(&req, &symmetric_key, &backend_url, true, &up_jwt, &uuid).await {
                Ok(res) => res,
                Err(e) => {
                    return Err(JsError::new(&e));
                }
            }
        }

        _ => types::Response {
            status: 400,
            status_text: "Content-Type not supported".to_string(),
            ..Default::default()
        },
    };

    let response_init = ResponseInit::new();
    let headers = web_sys::Headers::new().unwrap();
    for (key, value) in res.headers.iter() {
        headers
            .append(key, value)
            .expect_throw("expected headers to be appended to the web_sys::Headers object; qed");
    }

    response_init.set_headers(&headers);
    response_init.set_status(res.status);
    response_init.set_status_text(&res.status_text);

    let mut body = res.body;
    let response = match Response::new_with_opt_u8_array_and_init(Some(&mut body), &response_init) {
        Ok(val) => val,
        Err(e) => {
            return Err(JsError::new(&format!("{:?}", e)));
        }
    };

    Ok(response)
}

async fn construct_form_data(js_body: &JsValue) -> Result<HashMap<String, Value>, String> {
    let mut form_data = HashMap::<String, Value>::new();
    for entry in FormData::entries(&FormData::from(js_body.clone())) {
        let entry = entry.map_err(|err| {
            let msg = format!(
                "FormData entry error: {}",
                err.as_string().unwrap_or("issue getting FormData entry".to_string())
            );
            console_error!(&msg);
            msg
        })?;

        if entry.is_null() || entry.is_undefined() {
            // we skip null or undefined entries if any
            continue;
        }

        // [key, value]
        let key_val_entry = Array::from(&entry);
        let key = key_val_entry.get(0).as_string().ok_or("Object key was not a string".to_string())?;
        let value = key_val_entry.get(1);

        let data = {
            if value.is_instance_of::<File>() {
                construct_file_data(value).await?
            } else {
                match get_constructor_name(&value).as_str() {
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
                }
            }
        };

        if form_data.contains_key(&key) {
            if !form_data.get(&key).unwrap().is_array() {
                let old_value = form_data.get(&key).unwrap();
                form_data.insert(key, json!([old_value, data]));
            } else {
                let old_value = form_data.get_mut(&key).unwrap();
                old_value
                    .as_array_mut()
                    .expect_throw("expected old value to be an array; qed above")
                    .push(data);
            }
        } else {
            form_data.insert(key, serde_json::to_value(data).unwrap());
        }
    }

    Ok(form_data)
}

fn get_constructor_name(obj: &JsValue) -> String {
    // Get the prototype of the object
    let prototype = get_prototype_of(obj).expect_throw("expected prototype to be present since FormData has a finite set of prototypes (?!)");

    // Get the constructor from the prototype using Reflect.get
    let constructor = js_sys::Reflect::get(&prototype, &JsValue::from("constructor")).expect_throw("expected constructor to be present; qed");

    // Check if the constructor is a function
    if constructor.is_function() {
        // Convert the function to string and extract the name
        let constructor_name = to_string(&constructor);

        // Extract the name from the function string
        if let Some(name_start) = constructor_name.find("function ") {
            let name_end = constructor_name.find("(").unwrap_or(constructor_name.len());
            let name = &constructor_name[name_start + 9..name_end].trim(); // Skip "function "
            return name.to_string();
        }
    }

    "undefined".to_string()
}

async fn construct_file_data(value: JsValue) -> Result<serde_json::Value, String> {
    let reflect_get = js_sys::Reflect::get;

    let name = reflect_get(&value, &JsValue::from("name"))
        .map_err(|_| "expected name to be present".to_string())?
        .as_string()
        .ok_or("expected name to be a string".to_string())?;

    let size = reflect_get(&value, &JsValue::from("size"))
        .map_err(|_| "expected size to be present".to_string())?
        .as_f64()
        .ok_or("expected size to be a number".to_string())?;

    let type_ = reflect_get(&value, &JsValue::from("type"))
        .map_err(|_| "expected type to be present".to_string())?
        .as_string()
        .ok_or("expected type to be a string".to_string())?;

    let reader = File::from(value)
        .stream()
        .get_reader()
        .dyn_into::<web_sys::ReadableStreamDefaultReader>()
        .map_err(|_| "issue getting ReadableStreamDefaultReader entry".to_string())?;

    let mut buff = Vec::new();
    loop {
        let chunk_object = JsFuture::from(reader.read())
            .await
            .map_err(|_| "Read".to_string())?
            .dyn_into::<Object>()
            .map_err(|_| "issue casting Object entry".to_string())?;

        let done = reflect_get(&chunk_object, &JsValue::from_str("done"))
            .map_err(|_| "issue casting Object entry".to_string())?
            .as_bool()
            .ok_or("this value will always be boolean".to_string())?;

        if done {
            break;
        }

        let chunk = reflect_get(&chunk_object, &JsValue::from_str("value"))
            .map_err(|_| "issue casting Object entry".to_string())?
            .dyn_into::<Uint8Array>()
            .map_err(|_| "we're copying bytes".to_string())?;

        let buff_len = buff.len();
        buff.resize(buff_len + chunk.length() as usize, 0);
        chunk.copy_to(&mut buff[buff_len..]);
    }

    // compress and encode the file
    let buff = compress_gzip_and_encode_b64(&buff).map_err(|e| format!("Failed to compress and encode file: {}", e))?;
    console_log!(&format!("File `{name}` compressed and encoded successfully"));

    Ok(json!({
        "_type": "File",
        "name": name,
        "size": size,
        "type": type_,
        "buff": buff,
    }))
}

/// Test promise resolution/rejection from the console.
#[allow(non_snake_case)]
#[wasm_bindgen(js_name = testWASM)]
pub async fn test_wasm(arg: JsValue) -> Result<String, JsError> {
    if arg.is_null() || arg.is_undefined() {
        return Err(JsError::new("The argument is null or undefined."));
    }

    Ok(format!("WASM Interceptor version {INTERCEPTOR_VERSION} successfully loaded. Argument passed: {:?}. To test promise rejection, call with no argument.", arg))
}

/// This function is called to check the persistence of the WASM module.
#[allow(non_snake_case)]
#[wasm_bindgen(js_name = persistenceCheck)]
pub async fn persistence_check() -> i32 {
    let counter = COUNTER.with(|v| {
        v.set(v.get() + 1);
        v.get()
    });

    console_log!(&format!("WASM Counter: {}", counter));
    counter
}

/// This function is called to initialize the encrypted tunnel.
/// The mode is a dead argument; for backwards compatibility.
///
/// The config object is expected to have the following structure:
/// ```js
/// export interface InitConfig {
///    // The list of providers to establish the encrypted tunnel with.
///    providers: string[];
///    // The proxy URL to establish the encrypted tunnel.
///    proxy: string;
///    // Deprecated: `staticPath` is used for backwards compatibility, use `staticPaths` instead.
///    staticPath: string | undefined;
///    // The list of paths to serve static assets from.
///    staticPaths: string[] | undefined;
///    // The maximum size of assets to cache. The value is in MB.
///    cacheAssetLimit: number | undefined;
/// }
/// ```
#[allow(non_snake_case)]
#[wasm_bindgen(js_name = initEncryptedTunnel)]
pub async fn init_encrypted_tunnel(init_config: js_sys::Object, _: Option<String>) -> Result<(), JsError> {
    let init_config = InitConfig::new(init_config).await?;

    // populating the staticPaths static
    STATIC_PATHS.with(|v| {
        console_log!(&format!("Static paths: {:?}", init_config.static_paths));
        v.replace(init_config.static_paths.clone());
    });

    let cache = INDEXED_DBS.with(|v| {
        let val = v.get(INDEXED_DB_CACHE).expect_throw("expected indexed db to be present; qed");
        val.clone()
    });

    clear_expired_cache(INDEXED_DB_CACHE, cache);

    let mut providers = Vec::new();
    for provider in init_config.providers.iter() {
        console_log!(&format!("Establishing encrypted tunnel with provider: {}", provider));
        init_tunnel(provider, &init_config.proxy).await.map_err(|e| {
            console_error!(&format!("Failed to establish encrypted tunnel with provider: {}. Error: {}", provider, e));
            JsError::new(&e)
        })?;

        providers.push(provider);
        console_log!(&format!("Encrypted tunnel established with provider: {}", provider));
    }

    console_log!(&format!("Encrypted tunnel established with providers: {:?}", providers));
    Ok(())
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
        // .body(b64_pub_jwk.clone())
        .headers({
            let mut headers = reqwest::header::HeaderMap::new();
            let uuid = Uuid::new_v4().to_string();
            UUID.set(uuid.clone());

            headers.insert("x-ecdh-init", HeaderValue::from_str(&b64_pub_jwk).unwrap());
            headers.insert("x-client-uuid", HeaderValue::from_str(&uuid).unwrap());
            headers
        })
        .send()
        .await
        .map_err(|e| {
            console_log!(&format!("Failed to send request: {}", e));
            e.to_string()
        })?;

    if res.status().eq(&401) {
        ENCRYPTED_TUNNEL_FLAG.set(false);
        return Err(String::from("401 response from proxy, user is not authorized."));
    }

    let res_bytes = res.bytes().await.map_err(|e| {
        console_log!(&format!("Failed to read response: {}", e));
        e.to_string()
    })?;
    let mut proxy_data: serde_json::Map<String, serde_json::Value> = serde_json::from_slice(res_bytes.as_ref()).map_err(|val| {
        console_log!(&format!(
            "Failed to decode response: {}, Data is :{}",
            val,
            String::from_utf8_lossy(res_bytes.as_ref())
        ));
        val.to_string()
    })?;

    UP_JWT.set(
        proxy_data
            .remove("up-JWT")
            .ok_or("up_jwt not found")?
            .as_str()
            .unwrap() // infalliable
            .to_string(),
    );

    USER_SYMMETRIC_KEY.set(Some(private_jwk_ecdh.get_ecdh_shared_secret(&jwk_from_map(proxy_data)?)?));
    ENCRYPTED_TUNNEL_FLAG.set(true);

    let proxy_url = Url::parse(&proxy).map_err(|e| e.to_string())?;

    let url_proxy_ = &format!(
        "{}://{}:{}",
        proxy_url.scheme(),
        proxy_url.host().expect_throw("expected host to be present; qed"),
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

    console_log!(&format!("Encrypted tunnel established with provider: {}", provider));
    Ok(())
}

pub(crate) fn rebuild_url(url: &str) -> String {
    console_log!(&format!("Rebuilding URL: `{}`", url));

    let url = url::Url::parse(url).expect_throw("expected provider to be a valid URL; qed");
    let rebuilt_url = format!("{}://{}", url.scheme(), url.host_str().expect_throw("expected host to be present; qed"));
    match url.port() {
        Some(port) => format!("{}:{}", rebuilt_url, port),
        None => rebuilt_url,
    }
}
