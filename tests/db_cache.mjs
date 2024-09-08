"use-strict";

test("db_cache_object", async (t) => {
    const retObj = {
        store: "store",
        keyPath: "key",
        indexes: [{
            "url": {
                unique: true
            },
            "_exp" :{
                unique: false
            }
        }],
    };

    

    t.pass();
})
