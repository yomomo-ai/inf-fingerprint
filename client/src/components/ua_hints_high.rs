//! High-entropy UA Client Hints.
//!
//! `navigator.userAgentData.getHighEntropyValues([...])` returns architecture,
//! bitness, model (e.g. "SM-G998B"), full platform version, and the full
//! browser brand+version list. Chromium-only (so X5/XWEB/UC/Quark/Samsung yes,
//! iOS Safari no — and absence is itself a signal).

use serde::Serialize;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

#[derive(Serialize)]
pub struct UaHighEntropyFp {
    pub architecture: Option<String>,
    pub bitness: Option<String>,
    pub model: Option<String>,
    pub platform: Option<String>,
    pub platform_version: Option<String>,
    pub ua_full_version: Option<String>,
    pub mobile: Option<bool>,
    pub wow64: Option<bool>,
    pub full_version_list: Vec<String>,
    pub form_factor: Option<String>,
}

const KEYS: &[&str] = &[
    "architecture",
    "bitness",
    "model",
    "platformVersion",
    "uaFullVersion",
    "wow64",
    "fullVersionList",
    "formFactor",
];

pub async fn collect() -> Option<UaHighEntropyFp> {
    let nav = crate::ctx::navigator()?;
    let nav_js: &JsValue = nav.as_ref();
    let ua_data = crate::ctx::prop_object(nav_js, "userAgentData")?;
    let getter = crate::ctx::prop_object(&ua_data, "getHighEntropyValues")?;
    let func: js_sys::Function = getter.dyn_into().ok()?;

    let arr = js_sys::Array::new();
    for k in KEYS {
        arr.push(&JsValue::from_str(k));
    }
    let promise_val = func.call1(&ua_data, &arr).ok()?;
    let promise: js_sys::Promise = promise_val.dyn_into().ok()?;
    let result = JsFuture::from(promise).await.ok()?;

    let full_version_list = crate::ctx::prop_object(&result, "fullVersionList")
        .and_then(|v| v.dyn_into::<js_sys::Array>().ok())
        .map(|arr| {
            (0..arr.length())
                .filter_map(|i| {
                    let entry = arr.get(i);
                    let brand = crate::ctx::prop_string(&entry, "brand")?;
                    let version = crate::ctx::prop_string(&entry, "version").unwrap_or_default();
                    Some(format!("{} {}", brand, version))
                })
                .collect()
        })
        .unwrap_or_default();

    Some(UaHighEntropyFp {
        architecture: crate::ctx::prop_string(&result, "architecture"),
        bitness: crate::ctx::prop_string(&result, "bitness"),
        model: crate::ctx::prop_string(&result, "model"),
        platform: crate::ctx::prop_string(&result, "platform"),
        platform_version: crate::ctx::prop_string(&result, "platformVersion"),
        ua_full_version: crate::ctx::prop_string(&result, "uaFullVersion"),
        mobile: crate::ctx::prop_bool(&result, "mobile"),
        wow64: crate::ctx::prop_bool(&result, "wow64"),
        full_version_list,
        form_factor: crate::ctx::prop_string(&result, "formFactor"),
    })
}
