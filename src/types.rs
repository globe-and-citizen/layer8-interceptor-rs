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

// #[allow(non_snake_case)]
// #[wasm_bindgen]
// impl DbCache {
// #[wasm_bindgen(constructor)]
// pub fn new(store: String, key_path: String, indexes: Indexes) -> DbCache {
//     DbCache { store, key_path, indexes }
// }

// #[wasm_bindgen(getter)]
// pub fn store(&self) -> String {
//     self.store.clone()
// }

// #[wasm_bindgen(setter)]
// pub fn set_store(&mut self, store: String) {
//     self.store = store;
// }

// #[wasm_bindgen(getter)]
// pub fn key_path(&self) -> String {
//     self.key_path.clone()
// }

// #[wasm_bindgen(setter)]
// pub fn set_key_path(&mut self, key_path: String) {
//     self.key_path = key_path;
// }

// #[wasm_bindgen(getter)]
// pub fn indexes(&self) -> js_sys::Object {
//     let obj = js_sys::Object::new();
//     js_sys::Reflect::set(
//         &obj,
//         &"url".into(),
//         &serde_wasm_bindgen::to_value(&self.indexes.url).expect_throw("failed to serialize url index"),
//     )
//     .unwrap();
//     js_sys::Reflect::set(
//         &obj,
//         &"_exp".into(),
//         &serde_wasm_bindgen::to_value(&self.indexes._exp).expect_throw("failed to serialize url index"),
//     )
//     .unwrap();
//     obj
// }

// #[wasm_bindgen(setter)]
// pub fn set_indexes(&mut self, indexes: Indexes) {
//     self.indexes = indexes;
// }
// }

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct Indexes {
    pub(crate) url: Uniqueness,
    pub(crate) _exp: Uniqueness,
    pub(crate) body: Uniqueness,
    pub(crate) _type: Uniqueness,
}

#[allow(non_snake_case)]
#[wasm_bindgen]
impl Indexes {
    #[wasm_bindgen(constructor)]
    pub fn new(url: Uniqueness, _exp: Uniqueness, body: Uniqueness, _type: Uniqueness) -> Indexes {
        Indexes { url, _exp, body, _type }
    }

    #[wasm_bindgen(getter)]
    pub fn url(&self) -> Uniqueness {
        self.url.clone()
    }

    #[wasm_bindgen(setter)]
    pub fn set_url(&mut self, url: Uniqueness) {
        self.url = url;
    }

    #[wasm_bindgen(getter)]
    pub fn _exp(&self) -> Uniqueness {
        self._exp.clone()
    }

    #[allow(non_snake_case)] // Need the underscore to match the JS property name.
    #[wasm_bindgen(setter)]
    pub fn set__exp(&mut self, exp: Uniqueness) {
        self._exp = exp;
    }

    #[wasm_bindgen(getter)]
    pub fn body(&self) -> Uniqueness {
        self.body.clone()
    }

    #[wasm_bindgen(setter)]
    pub fn set_body(&mut self, body: Uniqueness) {
        self.body = body;
    }

    #[wasm_bindgen(getter)]
    pub fn _type(&self) -> Uniqueness {
        self._type.clone()
    }

    #[allow(non_snake_case)] // Need the underscore to match the JS property name.
    #[wasm_bindgen(setter)]
    pub fn set__type(&mut self, _type: Uniqueness) {
        self._type = _type;
    }
}

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct Uniqueness {
    pub(crate) unique: bool,
}

#[allow(non_snake_case)]
#[wasm_bindgen]
impl Uniqueness {
    #[wasm_bindgen(constructor)]
    pub fn new(unique: bool) -> Uniqueness {
        Uniqueness { unique }
    }

    #[wasm_bindgen(getter)]
    pub fn unique(&self) -> bool {
        self.unique
    }

    #[wasm_bindgen(setter)]
    pub fn set_unique(&mut self, unique: bool) {
        self.unique = unique;
    }
}
