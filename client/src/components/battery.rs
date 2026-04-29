use serde::Serialize;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

#[derive(Serialize)]
pub struct BatteryFp {
    pub charging: Option<bool>,
    pub level: Option<f64>,
    pub charging_time: Option<f64>,
    pub discharging_time: Option<f64>,
}

pub async fn collect() -> Option<BatteryFp> {
    let nav = crate::ctx::navigator()?;
    let nav_js: &JsValue = nav.as_ref();
    let get_battery = crate::ctx::prop_object(nav_js, "getBattery")?;
    let func: js_sys::Function = get_battery.dyn_into().ok()?;
    let promise_val = func.call0(nav_js).ok()?;
    let promise: js_sys::Promise = promise_val.dyn_into().ok()?;
    let battery = JsFuture::from(promise).await.ok()?;

    Some(BatteryFp {
        charging: crate::ctx::prop_bool(&battery, "charging"),
        level: crate::ctx::prop_number(&battery, "level"),
        charging_time: crate::ctx::prop_number(&battery, "chargingTime"),
        discharging_time: crate::ctx::prop_number(&battery, "dischargingTime"),
    })
}
