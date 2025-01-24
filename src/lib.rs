pub mod js;
#[cfg(feature = "websocket")]
pub mod websocket;

pub(crate) mod js_glue;
pub(crate) mod types;
pub(crate) mod js_imports_prelude {
    use crate::js_glue;
    pub use js_glue::js_imports::{clear_expired_cache, console_error, console_log, get_prototype_of, object_entries, serve_static, to_string};

    #[cfg(debug_assertions)]
    pub use js_glue::js_imports::{console_error_, console_log_};
}
