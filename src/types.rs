use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[derive(Clone)]
#[wasm_bindgen]
pub struct DbCache {
    pub(crate) store: String,
    pub(crate) key_path: String,
    pub(crate) indexes: Indexes,
}

#[allow(non_snake_case)]
#[wasm_bindgen]
impl DbCache {
    #[wasm_bindgen(constructor)]
    pub fn new(store: String, key_path: String, indexes: Indexes) -> DbCache {
        DbCache { store, key_path, indexes }
    }

    #[wasm_bindgen(getter)]
    pub fn store(&self) -> String {
        self.store.clone()
    }

    #[wasm_bindgen(setter)]
    pub fn set_store(&mut self, store: String) {
        self.store = store;
    }

    #[wasm_bindgen(getter)]
    pub fn key_path(&self) -> String {
        self.key_path.clone()
    }

    #[wasm_bindgen(setter)]
    pub fn set_key_path(&mut self, key_path: String) {
        self.key_path = key_path;
    }

    #[wasm_bindgen(getter)]
    pub fn indexes(&self) -> js_sys::Object {
        let obj = js_sys::Object::new();
        js_sys::Reflect::set(
            &obj,
            &"url".into(),
            &serde_wasm_bindgen::to_value(&self.indexes.url).expect_throw("failed to serialize url index"),
        )
        .unwrap();
        js_sys::Reflect::set(
            &obj,
            &"_exp".into(),
            &serde_wasm_bindgen::to_value(&self.indexes._exp).expect_throw("failed to serialize url index"),
        )
        .unwrap();
        obj
    }

    #[wasm_bindgen(setter)]
    pub fn set_indexes(&mut self, indexes: Indexes) {
        self.indexes = indexes;
    }
}

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
