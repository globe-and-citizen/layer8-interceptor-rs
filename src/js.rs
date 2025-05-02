use std::{cell::Cell, cell::RefCell, collections::HashMap};

use js_sys::{ArrayBuffer, Object, Uint8Array};
use reqwest::header::HeaderValue;
use url::Url;
use uuid::Uuid;
use wasm_bindgen::prelude::*;
use web_sys::{Blob, FileReaderSync, FormData, Response, ResponseInit};

use crate::js_glue::js_imports::{check_if_asset_exists, parse_form_data_to_array};
use crate::js_imports_prelude::*;
use crate::types::{DbCache, InitConfig, Uniqueness, CACHE_STORAGE_LIMIT};
use layer8_primitives::{
    compression::decompress_data_gzip,
    crypto::{self, generate_key_pair, jwk_from_map},
    types::{self, new_client, Request},
};

const INTERCEPTOR_VERSION: &str = "0.0.14";
const INDEXED_DB_CACHE: &str = "_layer8cache";
/// The cache time-to-live for the IndexedDB cache is 2 days.
const INDEXED_DB_CACHE_TTL: i32 = 60 * 60 * 24 * 2 * 1000; // 2 days in milliseconds

thread_local! {
    static HTTP_PUB_JWK_ECDH:  Cell<Option<crypto::Jwk>> = const { Cell::new(None) };
    static HTTP_PRIVATE_JWK_ECDH: Cell<Option<crypto::Jwk>> = const { Cell::new(None) };
    static HTTP_USER_SYMMETRIC_KEY: RefCell<Option<crypto::Jwk> >= const { RefCell::new(None) };
    static HTTP_UP_JWT: RefCell<String> = RefCell::new("".to_string());
    static HTTP_ENCRYPTED_TUNNEL_FLAG: Cell<bool> = const { Cell::new(false) };
    static HTTP_UUID: RefCell<HashMap<String,String>> = RefCell::new(HashMap::new());
    static COUNTER: RefCell<i32> = const { RefCell::new(0) };
    static STATIC_PATHS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static L8_CLIENTS: RefCell<HashMap<String, types::Client>> = RefCell::new(HashMap::new());

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

    if !HTTP_ENCRYPTED_TUNNEL_FLAG.get() {
        return Err(JsError::new("Encrypted tunnel is closed. Reload page."));
    }

    let static_paths = STATIC_PATHS.with(|v| {
        let val = v.take();
        v.replace(val.clone());
        val
    });

    let req_url = rebuild_url(&url);

    let client = L8_CLIENTS.with_borrow(|v| v.get(&req_url).cloned());

    let mut assets_glob_url = req_url.clone();
    for static_path in static_paths.iter() {
        if url.contains(static_path) {
            assets_glob_url.push_str(static_path);
            break;
        }
    }

    console_log!(&format!("Request URL: {}", req_url));

    let req_metadata = types::RequestMetadata {
        method: "GET".to_string(),
        headers: HashMap::from([
            ("content-type".to_string(), "application/json".to_string()),
            ("layer8-empty-body".to_string(), "true".to_string()),
        ]),
        url_path: Some(url.clone()),
    };

    let symmetric_key = {
        let symmetric_key = HTTP_USER_SYMMETRIC_KEY.with(|v| {
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
        let up_jwt = HTTP_UP_JWT.with(|v| {
            let val = v.take();
            v.replace(val.clone());
            val
        });

        let provider = rebuild_url(url.as_str());
        let uuid = HTTP_UUID
            .with_borrow(|v| v.get(&provider).cloned())
            .ok_or_else(|| JsError::new(&format!("UUID not found for resource provider, {}.", provider)))?;

        let res = client
            .expect_throw("client could not be found. This is likely due to the encrypted tunnel not being established correctly.")
            .r#do((&Request::default(), &req_metadata), &symmetric_key, &req_url, true, &up_jwt, &uuid)
            .await;

        match res {
            Ok(val) => {
                console_log!("File fetched successfully");
                console_log!(&format!("Response: {:?}", val));
                val
            }
            Err(e) => {
                console_log!(&format!("Failed to fetch: {}\nWith request metadata {:?}", e, req_metadata));
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
    HTTP_ENCRYPTED_TUNNEL_FLAG.get()
}

/// This function is an override of the fetch function. It's arguments are a URL and an options object.
#[wasm_bindgen]
pub async fn fetch(url: String, options: JsValue) -> Result<Response, JsError> {
    if !HTTP_ENCRYPTED_TUNNEL_FLAG.get() {
        return Err(JsError::new("Encrypted tunnel is closed. Reload page."));
    }

    let mut req_metadata = types::RequestMetadata {
        method: "GET".to_string(),
        url_path: Some(url.clone()),
        ..Default::default()
    };

    let mut req = types::Request { ..Default::default() };

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
                    req_metadata.method = value.as_string().unwrap_or("GET".to_string());
                }
                "headers" => {
                    let headers = object_entries(Object::try_from(&value).expect_throw("expected headers to be a [key, val] object array; qed"));

                    headers.iter().for_each(|header| {
                        let header_entries = js_sys::Array::from(&header);
                        let header_name = header_entries.get(0).as_string().expect_throw("key is a string; qed");
                        if header_name.trim().eq_ignore_ascii_case("content-length") {
                            return;
                        }

                        req_metadata
                            .headers
                            .insert(header_name, header_entries.get(1).as_string().expect_throw("value is a string; qed"));
                    });
                }
                "body" => {
                    js_body = value;
                    if !js_body.is_null() && !js_body.is_undefined() && js_body.is_instance_of::<FormData>() {
                        req_metadata.headers.insert("Content-Type".to_string(), "multipart/form-data".to_string());
                    }
                }
                _ => {}
            }
        }

        // if content type is not provided, we default to "application/json"
        if !req_metadata.headers.iter().any(|(k, _)| k.trim().eq_ignore_ascii_case("Content-Type")) {
            req_metadata.headers.insert("Content-Type".to_string(), "application/json".to_string());
        }
    }

    let backend_url = rebuild_url(&url);
    let client = match L8_CLIENTS.with_borrow(|v| v.get(&backend_url).cloned()) {
        Some(client) => client,
        None => {
            return Err(JsError::new("client could not be found"));
        }
    };

    let symmetric_key = {
        let val = HTTP_USER_SYMMETRIC_KEY.with(|v| {
            let val = v.take();
            v.replace(val.clone());
            val
        });

        match val {
            Some(key) => key,
            None => {
                return Err(JsError::new("symmetric key not found."));
            }
        }
    };

    let up_jwt = HTTP_UP_JWT.with(|v| {
        let val = v.take();
        v.replace(val.clone());
        val
    });

    let uuid = HTTP_UUID
        .with_borrow(|v| v.get(&backend_url).cloned())
        .ok_or_else(|| JsError::new(&format!("UUID not found for resource provider, {}.", backend_url)))?;

    // we don't care about the content-type; as long as the data is encrypted and custom protocols like websockets
    // and other upgrades are handled by separate extensions logic; see [`websocket::WasmWebSocket`]
    if !js_body.is_null() && !js_body.is_undefined() {
        match js_body {
            x if x.is_string() => {
                let value = x
                    .as_string()
                    .expect_throw("check asserted; js_body is an instance of String; qed")
                    .to_string();
                req.body = value.as_bytes().to_vec();
            }

            x if x.is_instance_of::<Blob>() => {
                let reader = FileReaderSync::new().expect_throw("Failed to create FileReaderSync");
                let array = reader
                    .read_as_array_buffer(&x.dyn_into::<Blob>().expect_throw("check asserted, js_body is an instance of Blob; qed"))
                    .map_err(|e| JsError::new(&e.as_string().unwrap_throw()))?;
                req.body = Uint8Array::new(&array).to_vec()
            }

            x if x.is_instance_of::<ArrayBuffer>() => req.body = Uint8Array::new(&x.dyn_into::<ArrayBuffer>().unwrap_throw()).to_vec(),

            x if x.is_instance_of::<Uint8Array>() => req.body = x.dyn_into::<Uint8Array>().unwrap_throw().to_vec(),

            x if x.is_instance_of::<FormData>() => {
                let boundary = format!("---------------------------{}", Uuid::new_v4());

                // we expect it to be Uint8Array
                let val = parse_form_data_to_array(x.dyn_into::<FormData>().unwrap_throw(), boundary.clone())
                    .await
                    .map_err(|e| JsError::new(&format!("Failed to parse FormData: {:?}", e)))?
                    .dyn_into::<Uint8Array>()
                    .map_err(|e| JsError::new(&format!("Failed to convert FormData to Uint8Array: {:?}", e)))?;

                req.body = val.to_vec();
                req_metadata
                    .headers
                    .insert("Content-Type".to_string(), format!("multipart/form-data; boundary={}", boundary));
            }

            _ => {
                console_error!(&format!("Could not determine the datatype of the body: {:?}", js_body));
                console_log!(&format!("Debug value: {:?}", js_body.js_typeof()));
                return Err(JsError::new("Unsupported data type"));
            }
        }
    }

    if req.body.is_empty() {
        req_metadata.headers.insert("layer8-empty-body".to_string(), "true".to_string());
    }

    req_metadata.url_path = Some(url.clone());
    let res = match client
        .r#do((&req, &req_metadata), &symmetric_key, &backend_url, true, &up_jwt, &uuid)
        .await
    {
        Ok(res) => res,
        Err(e) => {
            console_log!(&format!("Failed to fetch: {}\nWith request {:?}", e, req));

            return Err(JsError::new(&e));
        }
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

/// Test promise resolution/rejection from the console.
#[allow(non_snake_case)]
#[wasm_bindgen(js_name = testWASM)]
pub async fn test_wasm(arg: JsValue) -> Result<String, JsError> {
    if arg.is_null() || arg.is_undefined() {
        return Err(JsError::new("The argument is null or undefined."));
    }

    Ok(format!(
        "WASM Interceptor version {INTERCEPTOR_VERSION} successfully loaded. Argument passed: {:?}. To test promise rejection, call with no argument.",
        arg
    ))
}

/// This function is called to check the persistence of the WASM module.
#[allow(non_snake_case)]
#[wasm_bindgen(js_name = persistenceCheck)]
pub async fn persistence_check() -> i32 {
    let counter = COUNTER.with_borrow_mut(|v| {
        *v += 1;
        *v
    });

    console_log!(&format!("WASM Counter: {}", counter));
    counter
}

/// This function is called to initialize the encrypted tunnel.
/// The mode is a dead argument; for backwards compatibility.
///
/// If a client for the provider already exists, no calls are made to the proxy.
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

        // before we initialize creation of a client check if one is already linked with the provider
        if L8_CLIENTS.with_borrow(|v| v.get(provider).is_some()) {
            console_log!(&format!("Encrypted tunnel established with provider: {}", provider));
            continue;
        }

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

    HTTP_PRIVATE_JWK_ECDH.with(|v| {
        v.set(Some(private_jwk_ecdh.clone()));
    });

    let b64_pub_jwk = pub_jwk_ecdh.export_as_base64();

    let proxy = format!("{proxy}/init-tunnel?backend={provider}");

    let res = reqwest::Client::new()
        .post(&proxy)
        .headers({
            let mut headers = reqwest::header::HeaderMap::new();

            let uuid = Uuid::new_v4().to_string();
            HTTP_UUID.with_borrow_mut(|val| {
                val.insert(_provider_url, uuid.clone());
            });

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
        HTTP_ENCRYPTED_TUNNEL_FLAG.set(false);
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

    HTTP_UP_JWT.set(
        proxy_data
            .remove("up-JWT")
            .ok_or("up_jwt not found")?
            .as_str()
            .expect_throw("we expect the data type of tje jwt to be a string")
            .to_string(),
    );

    HTTP_USER_SYMMETRIC_KEY.set(Some(private_jwk_ecdh.get_ecdh_shared_secret(&jwk_from_map(proxy_data)?)?));
    HTTP_ENCRYPTED_TUNNEL_FLAG.set(true);

    let proxy_url = Url::parse(&proxy).map_err(|e| e.to_string())?;

    let url_proxy_ = &format!(
        "{}://{}:{}",
        proxy_url.scheme(),
        proxy_url.host().expect_throw("expected host to be present; qed"),
        proxy_url.port().unwrap_or(443)
    );

    let provider_client = new_client(url_proxy_).map_err(|e| {
        HTTP_ENCRYPTED_TUNNEL_FLAG.set(false);
        e.to_string()
    })?;

    L8_CLIENTS.with_borrow_mut(|val| val.insert(provider.to_string(), provider_client));

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
