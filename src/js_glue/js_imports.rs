use js_sys::Object;
use wasm_bindgen::prelude::*;

use crate::types::DbCache;

/// This block imports Javascript functions that are provided by the JS Runtime.
#[wasm_bindgen]
extern "C" {
    #[cfg(debug_assertions)]
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    pub fn console_log_(s: &str);

    #[cfg(debug_assertions)]
    #[wasm_bindgen(js_namespace = console, js_name = error)]
    pub fn console_error_(s: &str);

    #[wasm_bindgen(js_namespace = Object, js_name = entries)]
    pub fn object_entries(obj: &Object) -> js_sys::Array;

    #[wasm_bindgen(js_namespace = Object, catch)]
    pub fn get_prototype_of(obj: &JsValue) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_namespace = Function, js_name = toString)]
    pub fn to_string(func: &JsValue) -> String;
}

/// This block imports JavaScript functionality that is not mapped by the wasm-bindgen tool.
#[wasm_bindgen(module = "/src/js_glue/glue_indexed_db.js")]
extern "C" {
    /// This operation clears the cache of a specific database.
    pub fn clear_expired_cache(db_name: &str, db_cache: DbCache);
    #[wasm_bindgen(catch)]
    pub async fn serve_static(
        db_name: &str,
        body: &[u8],
        asset_size_limit: u32,
        file_type: &str,
        url: &str,
        exp_in_seconds: i32,
    ) -> Result<JsValue, JsValue>;

    /// This operation checks if an asset exists in the cache, if it does, it returns the asset.
    #[wasm_bindgen(js_name = check_if_exists, catch)]
    pub async fn check_if_asset_exists(db_name: &str, url: &str) -> Result<JsValue, JsValue>;

    /// This operation retrieves the storage estimate of the cache.
    #[wasm_bindgen(catch)]
    pub async fn get_storage_estimate() -> Result<JsValue, JsValue>;
}

#[macro_export]
#[cfg(not(debug_assertions))]
macro_rules! console_log {
    ($msg:expr) => {
        ()
    };
}

#[macro_export]
#[cfg(debug_assertions)]
macro_rules! console_log {
    ($msg:expr) => {
        console_log_($msg)
    };
}

#[macro_export]
#[cfg(not(debug_assertions))]
macro_rules! console_error {
    ($msg:expr) => {
        ()
    };
}

#[macro_export]
#[cfg(debug_assertions)]
macro_rules! console_error {
    ($msg:expr) => {
        console_error_($msg)
    };
}

pub use console_error;
pub use console_log;
