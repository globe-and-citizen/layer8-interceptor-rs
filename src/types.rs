use std::cell::Cell;

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::js_glue::js_imports::{self, get_storage_estimate};

// These statics are declared here to avoid import cycles if we coupled them with the rest in `./js.rs`.
thread_local! {
    /// We are using a default asset size limit ot 50MB. This value can be overridden by the initialization config.
    pub(crate) static CACHE_STORAGE_LIMIT: Cell<u32> = const { Cell::new(50) };
}

/// This type represents the configuration object that is passed to the `init` function.
///
/// The config object is expected to have the following structure:
/// ```js
/// export interface InitConfig {
///    // The list of providers to establish the encrypted tunnel with.
///    providers:   string[];
///    // The proxy URL to establish the encrypted tunnel.
///    proxy:       string;
///    // Deprecated: `staticPath` is used for backwards compatibility, use `staticPaths` instead.
///    staticPath:  string | undefined;
///    // The list of paths to serve static assets from.
///    staticPaths: string[] | undefined;
///    // The maximum size of assets to cache. The value is in MB.
///    cacheAssetLimit: number | undefined;
/// }
/// ```
#[derive(Default)]
pub(crate) struct InitConfig {
    pub(crate) proxy: String,
    pub(crate) static_paths: Vec<String>,
    pub(crate) providers: Vec<String>,
}

impl InitConfig {
    pub async fn new(obj: js_sys::Object) -> Result<Self, JsError> {
        let mut init_config = InitConfig::default();

        let entries = js_imports::object_entries(&obj);
        for entry in entries.iter() {
            let val = js_sys::Array::from(&entry); // [key, value] result from Object.entries
            match val.get(0).as_string().ok_or(JsError::new("expected object key to be a string"))?.as_str() {
                "providers" => {
                    if !val.get(1).is_instance_of::<js_sys::Array>() {
                        return Err(JsError::new("expected `InitConfig.providers` value to be an array"));
                    }

                    for provider in js_sys::Array::from(&val.get(1)).iter() {
                        init_config.providers.push(
                            provider
                                .as_string()
                                .ok_or(JsError::new("expected `InitConfig.provider` value to be a string"))?,
                        )
                    }
                }

                "proxy" => {
                    init_config.proxy = val
                        .get(1)
                        .as_string()
                        .ok_or(JsError::new("expected `InitConfig.proxy` value to be a string"))?;
                }

                "staticPath" => {
                    let path = val
                        .get(1)
                        .as_string()
                        .ok_or(JsError::new("expected `InitConfig.staticPath` value to be a string"))?;
                    init_config.static_paths.push(path);
                }

                "staticPaths" => {
                    // paths is a list of strings
                    if !val.get(1).is_instance_of::<js_sys::Array>() {
                        return Err(JsError::new("expected `InitConfig.staticPaths` value to be an array"));
                    }

                    for path in js_sys::Array::from(&val.get(1)).iter() {
                        let value = path
                            .as_string()
                            .ok_or(JsError::new("expected `InitConfig.staticPaths` value to be a string"))?;
                        init_config.static_paths.push(value);
                    }
                }

                "cacheAssetLimit" => {
                    // if we can't get the storage estimate, we rely on the default value
                    let estimate = get_storage_estimate().await;
                    if let Ok(estimate) = estimate {
                        let mut val = val
                            .get(1)
                            .as_f64()
                            .ok_or(JsError::new("expected `InitConfig.cacheAssetLimit` value to be a number"))?;

                        let estimate = estimate.as_f64().expect_throw("expected storage estimate to be a number");

                        if val > estimate {
                            // we are going with half the estimate
                            // estimates are usually [very large]<https://developer.mozilla.org/en-US/play?id=qHEOFcbSol%2Bevp8cXcV4AHeiMNC9eg1hPfouaBm%2Fdv3CX6MmH3pAqbE018v9o2C0XOIUTTJe%2BTlzxxbC>
                            val = estimate * 0.5;
                        }

                        CACHE_STORAGE_LIMIT.with(|limit| limit.set(val as u32));
                    }
                }

                _ => {
                    // we rather pipe the issues now than have them silently ignored
                    return Err(JsError::new(&format!(
                        "unexpected key in `InitConfig`: {}",
                        val.get(0).as_string().expect_throw("expected object key to be a string")
                    )));
                }
            }
        }

        Ok(init_config)
    }
}

#[derive(Clone)]
#[wasm_bindgen(getter_with_clone)]
pub struct DbCache {
    pub store: String,
    pub key_path: String,
    pub indexes: Indexes,
}

#[wasm_bindgen(getter_with_clone)]
#[derive(Clone, Serialize, Deserialize)]
pub struct Indexes {
    pub url: Uniqueness,
    pub _exp: Uniqueness,
    pub body: Uniqueness,
    pub _type: Uniqueness,
}

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct Uniqueness {
    pub unique: bool,
}
