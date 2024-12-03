use js_sys::Object;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = Object, js_name = entries)]
    fn object_entries(obj: &Object) -> js_sys::Array;
}