pub mod js;
pub(crate) mod js_imports;
pub(crate) mod types;

pub(crate) mod js_imports_prelude {
    use crate::js_imports;
    pub use js_imports::{clear_expired_cache, console_error, console_log, get_prototype_of, object_entries, serve_static, to_string};

    #[cfg(debug_assertions)]
    pub use js_imports::{console_error_, console_log_};
}
