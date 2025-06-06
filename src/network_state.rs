use std::collections::HashMap;

use js_sys::{ArrayBuffer, Object, Uint8Array};
use layer8_primitives::{
    compression::decompress_data_gzip,
    crypto::{self, Jwk, generate_key_pair, jwk_from_map},
    types::{self, Request, new_client},
};
use reqwest::header::HeaderValue;
use url::Url;
use uuid::Uuid;
use wasm_bindgen::{JsCast, JsError, JsValue, UnwrapThrowExt, prelude::wasm_bindgen};
use web_sys::{Blob, FileReaderSync, FormData, Response, ResponseInit};

use crate::{
    js::{INDEXED_DB_CACHE, INDEXED_DB_CACHE_TTL},
    js_glue::js_imports::check_if_asset_exists,
    js_imports_prelude::*,
    types::CACHE_STORAGE_LIMIT,
};
use crate::{
    js::{PROVIDER_REGISTER, get_base_url},
    js_glue::js_imports::parse_form_data_to_array,
};

#[derive(Debug, Default, Clone)]
pub(crate) struct NetworkState {
    // These environment values are essential for the tunnel to work
    pub client_uuid: String,
    pub symmetric_key: Jwk,
    pub provider_session: String,
    pub static_paths: Vec<String>,

    pub proxy_url: String,
    pub client: Option<types::Client>,
    pub public_key_jwk: Jwk,
    pub private_key_jwk: Jwk,
    // TODO:
    // user_proxy_jwt
    // provider_public_key
    // ephemeral_client_key_pair
}

/// This is the object that the JS API interacts with. It is a marker for the ProviderRegistry to identify which
/// NetworkState to use when doing the `fetch` and `get_static` operations.
#[wasm_bindgen]
pub struct NetworkStateHandler(pub(crate) String);

#[wasm_bindgen]
impl NetworkStateHandler {
    /// This function is an override of the fetch function. It's arguments are a URL and an options object.
    pub async fn fetch(&self, url: String, options: JsValue) -> Result<Response, JsError> {
        let base_url = get_base_url(&url);
        if base_url.ne(&self.0) {
            return Err(JsError::new(&format!(
                "the NetworkStateHandler is for `{}` but we're calling the url `{}` instead",
                self.0, base_url
            )));
        }

        let mut network_state = PROVIDER_REGISTER
            .with_borrow(|map| map.get(&self.0).cloned())
            .expect_throw("we expect the NetworkState to be present since the handler exists");
        let proxy_url = network_state.proxy_url.clone();

        let mut err_cache = JsError::new("");
        for _ in 1..=3 {
            match network_state.fetch(url.clone(), options.clone()).await {
                Ok(val) => return Ok(val),
                Err((status, err)) => {
                    if status == -1 || status >= 500 {
                        // un-retryable errors
                        return Err(err);
                    }

                    err_cache = err;
                    network_state = NetworkState::new(&url, &proxy_url).await.map_err(|e| JsError::new(e.as_str()))?;
                }
            }
        }

        Err(err_cache)
    }

    /// This function is called to retrieve the static file.
    /// It is expected to be called with a URL string.
    #[wasm_bindgen(js_name = _static)]
    pub async fn get_static(&self, url: String) -> Result<String, JsError> {
        let base_url = get_base_url(&url);
        if base_url.ne(&self.0) {
            return Err(JsError::new(&format!(
                "the NetworkStateHandler if for `{}` but we're calling the url `{}` instead",
                self.0, base_url
            )));
        }

        let mut network_state = PROVIDER_REGISTER
            .with_borrow(|map| map.get(&self.0).cloned())
            .expect_throw("we expect the NetworkState to be present since the handler exists");
        let proxy_url = network_state.proxy_url.clone();
        let mut err_cache = JsError::new("");

        for _ in 1..=3 {
            match network_state.get_static(url.clone()).await {
                Ok(val) => return Ok(val),
                Err((status, err)) => {
                    if status == -1 || status >= 500 {
                        // un-retryable errors
                        return Err(err);
                    }

                    err_cache = err;
                    network_state = NetworkState::new(&url, &proxy_url).await.map_err(|e| JsError::new(e.as_str()))?;
                }
            }
        }

        Err(err_cache)
    }
}

impl NetworkState {
    /// This operation initializes a new NetworkState. It updates
    pub(crate) async fn new(provider_url: &str, proxy_url: &str) -> Result<Self, String> {
        let mut network_state = NetworkState::default();

        // Adding the client and the proxy url to the network_state
        {
            let proxy_url = Url::parse(proxy_url).map_err(|e| e.to_string())?;
            let proxy_proxy = &format!(
                "{}://{}:{}",
                proxy_url.scheme(),
                proxy_url.host().expect_throw("expected host to be present; qed"),
                proxy_url.port().unwrap_or(443)
            );

            network_state.proxy_url = proxy_url.to_string();
            network_state.client = Some(new_client(proxy_proxy).map_err(|e| e.to_string())?);
        }

        // Create client_uuid and generate pub&priv key pair, add values to the network state
        let base_url = get_base_url(provider_url);
        {
            let client_uuid = uuid::Uuid::new_v4().to_string();
            let (private_jwk_ecdh, pub_jwk_ecdh) = generate_key_pair(crypto::KeyUse::Ecdh)?;

            network_state.private_key_jwk = private_jwk_ecdh;
            network_state.public_key_jwk = pub_jwk_ecdh;
            network_state.client_uuid = client_uuid;
        }

        let b64_pub_jwk = network_state.public_key_jwk.export_as_base64();
        let proxy = format!("{}/init-tunnel?backend={}", get_base_url(&network_state.proxy_url), base_url);

        let res = reqwest::Client::new()
            .post(&proxy)
            .headers({
                let mut headers = reqwest::header::HeaderMap::new();
                headers.insert(
                    "x-ecdh-init",
                    HeaderValue::from_str(&b64_pub_jwk).expect_throw("expected b64_pub_jwk to be a valid header value"),
                );
                headers.insert(
                    "x-client-uuid",
                    HeaderValue::from_str(&network_state.client_uuid).expect_throw("expected uuid to be a valid header value"),
                );
                headers
            })
            .send()
            .await
            .map_err(|e| {
                console_log!(&format!("Failed to send request: {}", e));
                e.to_string()
            })?;

        console_log!(&format!("Logging the response: {:?}", res));

        if res.status().eq(&401) {
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

        network_state.provider_session = proxy_data
            .remove("up-JWT")
            .ok_or("up_jwt not found")?
            .as_str()
            .expect_throw("we expect the data type of tje jwt to be a string")
            .to_string();

        network_state.symmetric_key = network_state.private_key_jwk.get_ecdh_shared_secret(&jwk_from_map(proxy_data)?)?;
        console_log!(&format!("Encrypted tunnel established with provider: {}", base_url));

        // update the network state to the PROVIDER_REGISTER cache
        PROVIDER_REGISTER.with_borrow_mut(|map| map.insert(base_url, network_state.clone()));
        Ok(network_state)
    }

    async fn fetch(&self, url: String, options: JsValue) -> Result<Response, (i16, JsError)> {
        let (js_body, mut req_metadata) = retrieve_body_and_req_metadata(&url, options).map_err(|e| (-1, e))?;
        let req = generate_req_from_js_body(js_body, &mut req_metadata).await.map_err(|e| (-1, e))?;

        if req.body.is_empty() {
            req_metadata.headers.insert("layer8-empty-body".to_string(), "true".to_string());
        }

        let base_url = get_base_url(&url);
        req_metadata.url_path = Some(url.clone());
        let res = match self
            .client
            .clone()
            .expect_throw("we expect the client to be present")
            .r#do(
                (&req, &req_metadata),
                &self.symmetric_key,
                &base_url,
                false,
                &self.provider_session,
                &self.client_uuid,
            )
            .await
        {
            Ok(res) => res,
            Err((status, e)) => {
                return Err((
                    status,
                    JsError::new(&format!("failed to fetch: {},  with request_metadata {:?}", e, req_metadata)),
                ));
            }
        };

        let response_init = ResponseInit::new();
        let headers = web_sys::Headers::new().expect_throw("expected headers to be created");
        for (key, value) in res.headers.iter() {
            headers
                .append(key, value)
                .expect_throw("expected headers to be appended to the web_sys::Headers object");
        }

        response_init.set_headers(&headers);
        response_init.set_status(res.status);
        response_init.set_status_text(&res.status_text);

        let mut body = res.body;
        let response = match Response::new_with_opt_u8_array_and_init(Some(&mut body), &response_init) {
            Ok(val) => val,
            Err(e) => {
                return Err((1, JsError::new(&format!("{:?}", e))));
            }
        };

        Ok(response)
    }

    // This marker is &mut because: <>
    async fn get_static(&self, url: String) -> Result<String, (i16, JsError)> {
        if url.is_empty() {
            return Err((-1, JsError::new("Invalid url provided to fetch call")));
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
                console_log!(&format!("IndexDB error {:?}", e));
                return Err((
                    -1,
                    JsError::new(&format!(
                        "error interacting with IndexDB: {}",
                        e.as_string().unwrap_or(format!("error unwrappable: {:?}", e))
                    )),
                ));
            }
        };

        let base_url = get_base_url(&url);
        let mut assets_glob_url = base_url.clone();
        for static_path in self.static_paths.iter() {
            if url.contains(static_path) {
                assets_glob_url.push_str(static_path);
                break;
            }
        }

        console_log!(&format!("Request URL: {}", base_url));

        let req_metadata = types::RequestMetadata {
            method: "GET".to_string(),
            headers: HashMap::from([
                ("content-type".to_string(), "application/json".to_string()),
                ("layer8-empty-body".to_string(), "true".to_string()),
            ]),
            url_path: Some(
                Url::parse(&url.clone())
                    .map_err(|e| (-1, JsError::new(&format!("url provided is invalid, {}", e))))?
                    .to_string(),
            ),
        };

        let res = {
            let res = self
                .client
                .clone()
                .expect_throw("we expect the client to be present")
                .r#do(
                    (&Request::default(), &req_metadata),
                    &self.symmetric_key,
                    &base_url,
                    true,
                    &self.provider_session,
                    &self.client_uuid,
                )
                .await;

            match res {
                Ok(val) => {
                    console_log!("File fetched successfully");
                    console_log!(&format!("Response: {:?}", val));
                    val
                }
                Err((status, e)) => {
                    return Err((
                        status,
                        JsError::new(&format!("f to fetch: {}, with request metadata {:?}", e, req_metadata)),
                    ));
                }
            }
        };

        let file_type = {
            let file_type = res.headers.iter().find(|(k, _)| k.trim().eq_ignore_ascii_case("Content-Type"));
            match file_type {
                Some((_, val)) => val.clone(),
                None => {
                    return Err((-1, JsError::new("Content-Type header not found.")));
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
                    return Err((-1, JsError::new(&format!("Error occurred decompressing file: {}", e))));
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
                return Err((
                    -1,
                    JsError::new(&format!(
                        "Error occurred interacting with IndexDB: {}",
                        e.as_string().expect_throw("error unwrappable")
                    )),
                ));
            }
        };

        console_log!(&format!("Object URL: {:?}", object_url));
        Ok(object_url)
    }
}

async fn generate_req_from_js_body(js_body: JsValue, req_metadata: &mut types::RequestMetadata) -> Result<types::Request, JsError> {
    let mut req = types::Request::default();

    // we don't care about the content-type; as long as the data is encrypted and custom protocols like websockets
    // and other upgrades are handled by separate extensions logic; see [`websocket::WasmWebSocket`]
    if js_body.is_null() || js_body.is_undefined() {
        console_log!("No body provided, using empty body");
        return Ok(req);
    }

    match js_body {
        x if x.is_string() => {
            let value = x.as_string().expect_throw("check asserted; js_body is an instance of String").to_string();
            req.body = value.as_bytes().to_vec();
        }

        x if x.is_instance_of::<Blob>() => {
            let reader = FileReaderSync::new().expect_throw("failed to create FileReaderSync");
            let array = reader
                .read_as_array_buffer(&x.dyn_into::<Blob>().expect_throw("check asserted, js_body is an instance of Blob"))
                .map_err(|e| JsError::new(&e.as_string().unwrap_or("failed to read a Blob instance".to_string())))?;
            req.body = Uint8Array::new(&array).to_vec()
        }

        x if x.is_instance_of::<ArrayBuffer>() => req.body = Uint8Array::new(&x.dyn_into::<ArrayBuffer>().unwrap_throw()).to_vec(),

        x if x.is_instance_of::<Uint8Array>() => req.body = x.dyn_into::<Uint8Array>().unwrap_throw().to_vec(),

        x if x.is_instance_of::<FormData>() => {
            console_log!("FormData detected");
            let boundary = format!("---------------------------{}", Uuid::new_v4());

            // we expect it to be Uint8Array
            let val = parse_form_data_to_array(x.dyn_into::<FormData>().unwrap_throw(), boundary.clone())
                .await
                .map_err(|e| JsError::new(&format!("failed to parse FormData: {:?}", e)))?
                .dyn_into::<Uint8Array>()
                .map_err(|e| JsError::new(&format!("failed to convert FormData to Uint8Array: {:?}", e)))?;

            console_log!(&format!("Form body length: {}", val.length()));
            req.body = val.to_vec();
            req_metadata
                .headers
                .insert("Content-Type".to_string(), format!("multipart/form-data; boundary={}", boundary));
        }

        _ => {
            console_error!(&format!("Could not determine the datatype of the body: {:?}", js_body));
            console_log!(&format!("Debug value: {:?}", js_body.js_typeof()));
            return Err(JsError::new("unsupported data type"));
        }
    }

    Ok(req)
}

fn retrieve_body_and_req_metadata(url: &str, options: JsValue) -> Result<(JsValue, types::RequestMetadata), JsError> {
    let mut req_metadata = types::RequestMetadata {
        method: "GET".to_string(),
        url_path: Some(url.to_string()),
        ..Default::default()
    };

    let mut js_body = JsValue::null();
    if options.is_null() || options.is_undefined() {
        return Ok((js_body, req_metadata));
    }

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

        let key = key.as_string().unwrap_throw().to_lowercase();
        if key.as_str() == "method" {
            req_metadata.method = value.as_string().unwrap_or("GET".to_string());
        }

        if key.as_str() == "headers" {
            let headers = object_entries(Object::try_from(&value).expect_throw("expected headers to be a [key, val] object array"));
            headers.iter().for_each(|header| {
                let header_entries = js_sys::Array::from(&header);
                let header_name = header_entries.get(0).as_string().expect_throw("expected header name as a string");

                if header_name.trim().eq_ignore_ascii_case("content-length") {
                    return;
                }

                let header_value = header_entries.get(1).as_string().expect_throw("expected header value as a string");
                req_metadata.headers.insert(header_name, header_value);
            });
        }

        if key.as_str() == "body" {
            js_body = value;
            if !js_body.is_null() && !js_body.is_undefined() && js_body.is_instance_of::<FormData>() {
                req_metadata.headers.insert("Content-Type".to_string(), "multipart/form-data".to_string());
            }
        }
    }

    // if content type is not provided, we default to "application/json"
    if !req_metadata.headers.iter().any(|(k, _)| k.trim().eq_ignore_ascii_case("Content-Type")) {
        req_metadata.headers.insert("Content-Type".to_string(), "application/json".to_string());
    }

    Ok((js_body, req_metadata))
}
