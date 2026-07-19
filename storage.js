// WASM storage bridge for macroquad-toolkit persistence.
// Loaded after sapp_jsutils.js and before the game wasm.

var storage_plugin = {
    register_plugin: function(importObject) {
        importObject.env.storage_set_extern = function(key_obj, value_obj) {
            var key = consume_js_object(key_obj);
            var value = consume_js_object(value_obj);
            try {
                localStorage.setItem(key, value);
            } catch (e) {
                console.error("storage_set failed:", e);
            }
        };

        importObject.env.storage_get_extern = function(key_obj) {
            var key = consume_js_object(key_obj);
            try {
                var value = localStorage.getItem(key);
                if (value === null) {
                    return -1;
                }
                return js_object(value);
            } catch (e) {
                console.error("storage_get failed:", e);
                return -1;
            }
        };

        importObject.env.storage_remove_extern = function(key_obj) {
            var key = consume_js_object(key_obj);
            try {
                localStorage.removeItem(key);
            } catch (e) {
                console.error("storage_remove failed:", e);
            }
        };

        importObject.env.storage_exists_extern = function(key_obj) {
            var key = consume_js_object(key_obj);
            try {
                return localStorage.getItem(key) !== null;
            } catch (e) {
                return false;
            }
        };
    }
};

miniquad_add_plugin(storage_plugin);
