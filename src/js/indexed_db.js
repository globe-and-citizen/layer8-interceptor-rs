"use strict";

// We have this here as there is no native support for IndexedDB in wasm_bindgen
// TODO: https://github.com/rustwasm/gloo/issues/68#issuecomment-606951683
export function open_db(db_name, db_cache) {
    let db = window.indexedDB.open(db_name).onerror(function (_) {
        console.log("Error opening IndexedDB database");
    });

    db.onupgradeneeded(function (event) {
        console.log("Success opening IndexedDB database");
        var db = event.target.result;

        if (db_cache == null || db_cache == undefined) {
            return;
        }

        db.createObjectStore(db_cache.store, {
            keyPath: db_cache.keyPath,
        });

        db.createIndex("url", "url", {
            unique: db_cache.indexes.url.unique,
        });

        db.createIndex("_exp", "_exp", {
            unique: db_cache.indexes._exp.unique,
        });
    });

    return db;
}

// Interacts with the IndexedDB method to clear expired cache
export function clear_expired_cache(db_name) {
    let db = open_db(db_name, null);
    db.onsuccess(function (event) {
        var db = event.target.result;
        var transaction = db.transaction("static", "readwrite");
        var store = transaction.objectStore("static");
        var index = store.index("_exp");

        // get all the expired items
        var bound = index.openCursor(IDBKeyRange.upperBound(Date.now()));
        index.openCursor(bound).onsuccess(function (event) {
            var cursor = event.target.result;
            if (cursor) {
                store.delete(cursor.primaryKey);
                cursor.continue();
            }
        }
        );
    });
}

// Interacts with the IndexedDB method transact with the cache
export function serving_static(db_name, body, file_type, url, _exp){
    // todo: implement this function
}