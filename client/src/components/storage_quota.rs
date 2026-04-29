use serde::Serialize;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

#[derive(Serialize)]
pub struct StorageQuotaFp {
    pub quota_bytes: Option<f64>,
    pub usage_bytes: Option<f64>,
}

pub async fn collect() -> Option<StorageQuotaFp> {
    let nav = crate::ctx::navigator()?;
    let nav_js: &JsValue = nav.as_ref();
    let storage = crate::ctx::prop_object(nav_js, "storage")?;
    let estimate = crate::ctx::prop_object(&storage, "estimate")?;
    let func: js_sys::Function = estimate.dyn_into().ok()?;
    let promise_val = func.call0(&storage).ok()?;
    let promise: js_sys::Promise = promise_val.dyn_into().ok()?;
    let result = JsFuture::from(promise).await.ok()?;

    Some(StorageQuotaFp {
        quota_bytes: crate::ctx::prop_number(&result, "quota"),
        usage_bytes: crate::ctx::prop_number(&result, "usage"),
    })
}
