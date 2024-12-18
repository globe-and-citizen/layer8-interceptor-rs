// We have this here as there is no native support for IndexedDB in wasm_bindgen
// TODO: https://github.com/rustwasm/gloo/issues/68#issuecomment-606951683
function open_db(db_name, db_cache) {
    if (!db_name) {
        console.error('The db_name is required.');
        return null;
    }

    let db
    try {
        db = window.indexedDB.open(db_name, 2)
    } catch (e) {
        console.error('Error opening IndexedDB database: ', e)
        return null
    }

    // if the db_cache object is not provided, return the db object
    if (!db_cache) {
        console.log('No cache object provided.')
        return db
    }

    db.onupgradeneeded = function (event) {
        var objectStore = event.target.result.createObjectStore(db_cache.store, {
            keyPath: db_cache.keyPath
        })

        objectStore.createIndex('url', 'url', {
            unique: db_cache.indexes.url.unique
        })
        objectStore.createIndex('_exp', '_exp', {
            unique: db_cache.indexes._exp.unique
        })
        objectStore.createIndex('body', 'body', {
            unique: db_cache.indexes.body.unique
        })
        objectStore.createIndex('_type', '_type', {
            unique: db_cache.indexes._type.unique
        })
    }

    return db
}

// Interacts with the IndexedDB method to clear expired cache
export function clear_expired_cache(db_name, db_cache) {
    let db = open_db(db_name, db_cache)
    if (db === null) {
        return
    }

    db.onsuccess = function (event) {
        var db = event.target.result
        var transaction = db.transaction('static', 'readwrite')
        var store = transaction.objectStore('static')
        var index = store.index('_exp')
        var bound = IDBKeyRange.upperBound(Date.now())

        index.openCursor(bound).onsuccess = function (event) {
            var cursor = event.target.result
            if (cursor) {
                store.delete(cursor.primaryKey)
                cursor.continue()
            }
        }
    }
}

// Interacts with the IndexedDB method transact with the cache
export function serve_static(db_name, body, file_type, url, _exp) {
    let db = open_db(db_name, body)
    if (!db)
        return

    db.onsuccess = function (event) {
        var db = event.target.result
        var transaction = db.transaction(['static'], 'readwrite')
        var store = transaction.objectStore('static')

        try {
            store.put(
                {
                    url: url,
                    body: body,
                    _type: file_type,
                    _exp: _exp
                },
                'static'
            )
        } catch (error) {
            console.log(error)
        }
    }

    const blob = new Blob([body], {
        type: file_type
    });

    return URL.createObjectURL(blob)
}

export function check_if_exists(db_name, url) {
    return new Promise((resolve, reject) => {
        let db = open_db(db_name);

        if (!db) {
            resolve(null);
            return;
        }

        db.onsuccess = function (event) {
            var db = event.target.result;
            var transaction = db.transaction(['static'], 'readonly');
            var store = transaction.objectStore('static');
            var index = store.index('url');
            var request = index.get(url);

            request.onsuccess = function (event) {
                if (request.result && request.result.body) {
                    const blob = new Blob([request.result.body], { type: request.result._type });
                    resolve(URL.createObjectURL(blob));
                    return;
                } else {
                    console.log('Asset not found in cache');
                    resolve(null);
                }
            };

            request.onerror = function (event) {
                console.log('Error retrieving asset from cache: ', event.target.error);
                reject(event.target.error);
            };
        };

        db.onerror = function (event) {
            console.log('Error opening database: ', event.target.error);
            reject(event.target.error);
        };
    });
}