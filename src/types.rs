use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::js_imports;

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
///    staticPath:  string;
///    // The list of paths to serve static assets from.
///    staticPaths: string[];
/// }
/// ```
#[derive(Default)]
pub(crate) struct InitConfig {
    pub(crate) providers: Vec<String>,
    pub(crate) proxy: String,
    pub(crate) static_paths: Vec<String>,
}

impl TryFrom<js_sys::Object> for InitConfig {
    type Error = JsError;

    fn try_from(obj: js_sys::Object) -> Result<Self, Self::Error> {
        let entries = js_imports::object_entries(&obj);

        let mut init_config = InitConfig::default();
        for entry in entries.iter() {
            let val = js_sys::Array::from(&entry); // [key, value] result from Object.entries
            match val.get(0).as_string().ok_or(JsError::new("expected object key to be a string"))?.as_str() {
                "providers" => {
                    if !val.get(1).is_instance_of::<js_sys::Array>() {
                        return Err(JsError::new("expected `InitConfig.providers` value to be an array"));
                    }

                    for provider in js_sys::Array::from(&val.get(1)).iter() {
                        let value: String = provider
                            .as_string()
                            .ok_or(JsError::new("expected `InitConfig.provider` value to be a string"))?;
                        init_config.providers.push(value);
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
