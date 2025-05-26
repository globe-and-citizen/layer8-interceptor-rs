use std::{cell::RefCell, collections::HashMap};

use wasm_bindgen::prelude::*;

use crate::js_imports_prelude::*;
use crate::network_state::NetworkState;
use crate::types::{DbCache, InitConfig, Uniqueness};

const INTERCEPTOR_VERSION: &str = env!("CARGO_PKG_VERSION");
pub(crate) const INDEXED_DB_CACHE: &str = "_layer8cache";
/// The cache time-to-live for the IndexedDB cache is 2 days.
pub(crate) const INDEXED_DB_CACHE_TTL: i32 = 60 * 60 * 24 * 2 * 1000; // 2 days in milliseconds

thread_local! {
    pub(crate) static PROVIDER_REGISTER: RefCell<HashMap<String, NetworkState>> = RefCell::new(HashMap::new());

    static COUNTER: RefCell<i32> = const { RefCell::new(0) };

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

/// This function is called to check if the encrypted tunnel is open.
/// Returning a boolean value.
#[allow(non_snake_case)]
#[wasm_bindgen(js_name = checkEncryptedTunnel)]
pub async fn check_encrypted_tunnel(provider: Option<String>) -> bool {
    if provider.is_none() {
        return false;
    }
    PROVIDER_REGISTER.with_borrow(|v| v.get(&get_base_url(&provider.unwrap_throw())).is_some())
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
///
/// If a client for the provider already exists, no calls are made to the proxy.
///
/// The config object is expected to have the following structure:
/// ```js
/// export interface InitConfig {
///    // The provider to establish the encrypted tunnel with.
///    provider: string;
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
pub async fn init_encrypted_tunnel(init_config: js_sys::Object, _: Option<String>) -> Result<NetworkState, JsError> {
    console_log!(&format!("Interceptor version is {}", INTERCEPTOR_VERSION));

    let init_config = InitConfig::new(init_config).await?;

    let cache = INDEXED_DBS.with(|v| {
        let val = v.get(INDEXED_DB_CACHE).expect_throw("expected indexed db to be present; qed");
        val.clone()
    });

    clear_expired_cache(INDEXED_DB_CACHE, cache);

    let provider = get_base_url(&init_config.provider);

    // before we initialize creation of a client check if one is already linked with the provider
    let network_state = match PROVIDER_REGISTER.with_borrow_mut(|map| map.get(&provider).cloned()) {
        Some(val) => val,
        None => {
            console_log!(&format!("Establishing encrypted tunnel with provider: {}", provider));
            let mut network_state = NetworkState::new(&provider, &init_config.proxy).await.map_err(|e| {
                console_error!(&format!("Failed to establish encrypted tunnel with provider: {}. Error: {}", provider, e));
                JsError::new(&e)
            })?;

            network_state.static_paths = init_config.static_paths;
            PROVIDER_REGISTER.with_borrow_mut(|map| map.insert(provider.clone(), network_state.clone()));
            network_state
        }
    };

    console_log!(&format!("Encrypted tunnel established with provider: {}", provider));

    Ok(network_state)
}

pub(crate) fn get_base_url(url: &str) -> String {
    console_log!(&format!("Rebuilding URL: `{}`", url));

    let url = url::Url::parse(url).expect_throw("expected provider to be a valid URL; qed");
    let rebuilt_url = format!("{}://{}", url.scheme(), url.host_str().expect_throw("expected host to be present; qed"));
    match url.port() {
        Some(port) => format!("{}:{}", rebuilt_url, port),
        None => rebuilt_url,
    }
}
