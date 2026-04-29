use wasm_bindgen::JsValue;

pub fn window() -> Option<web_sys::Window> {
    web_sys::window()
}

pub fn document() -> Option<web_sys::Document> {
    window().and_then(|w| w.document())
}

pub fn navigator() -> Option<web_sys::Navigator> {
    window().map(|w| w.navigator())
}

pub fn prop_string(target: &JsValue, name: &str) -> Option<String> {
    let v = js_sys::Reflect::get(target, &JsValue::from_str(name)).ok()?;
    if v.is_undefined() || v.is_null() {
        None
    } else {
        v.as_string()
    }
}

pub fn prop_number(target: &JsValue, name: &str) -> Option<f64> {
    let v = js_sys::Reflect::get(target, &JsValue::from_str(name)).ok()?;
    if v.is_undefined() || v.is_null() {
        None
    } else {
        v.as_f64()
    }
}

pub fn prop_bool(target: &JsValue, name: &str) -> Option<bool> {
    let v = js_sys::Reflect::get(target, &JsValue::from_str(name)).ok()?;
    if v.is_undefined() || v.is_null() {
        None
    } else {
        v.as_bool()
    }
}

pub fn prop_exists(target: &JsValue, name: &str) -> bool {
    match js_sys::Reflect::get(target, &JsValue::from_str(name)) {
        Ok(v) => !v.is_undefined(),
        Err(_) => false,
    }
}

pub fn prop_object(target: &JsValue, name: &str) -> Option<JsValue> {
    let v = js_sys::Reflect::get(target, &JsValue::from_str(name)).ok()?;
    if v.is_undefined() || v.is_null() {
        None
    } else {
        Some(v)
    }
}
