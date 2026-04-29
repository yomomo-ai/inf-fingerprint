use serde::Serialize;
use wasm_bindgen::prelude::*;

mod china;
mod components;
mod ctx;
mod hash;

use components::Components;

#[wasm_bindgen(start)]
pub fn _start() {
    #[cfg(feature = "panic_hook")]
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub struct Fingerprint {
    inner: FingerprintData,
}

#[derive(Serialize)]
struct FingerprintData {
    visitor_id: String,
    version: &'static str,
    confidence: f32,
    components: Components,
    china: china::ChinaSignals,
}

#[wasm_bindgen]
impl Fingerprint {
    #[wasm_bindgen(getter, js_name = visitorId)]
    pub fn visitor_id(&self) -> String {
        self.inner.visitor_id.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn version(&self) -> String {
        self.inner.version.to_string()
    }

    #[wasm_bindgen(getter)]
    pub fn confidence(&self) -> f32 {
        self.inner.confidence
    }

    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&self.inner).map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

#[wasm_bindgen(js_name = getFingerprint)]
pub async fn get_fingerprint() -> Result<Fingerprint, JsValue> {
    let china_signals = china::detect();
    let components = components::collect().await;

    let confidence = score_confidence(&components, &china_signals);
    let visitor_id = hash::compute_visitor_id(&components, &china_signals);

    Ok(Fingerprint {
        inner: FingerprintData {
            visitor_id,
            version: env!("CARGO_PKG_VERSION"),
            confidence,
            components,
            china: china_signals,
        },
    })
}

fn score_confidence(c: &Components, cn: &china::ChinaSignals) -> f32 {
    let signals = [
        c.canvas.is_some(),
        c.webgl.is_some(),
        c.audio.is_some(),
        c.screen.is_some(),
        c.navigator.is_some(),
        c.timezone.is_some(),
        c.fonts.is_some(),
        c.touch.is_some(),
        c.permissions.is_some(),
        cn.in_app != china::InAppBrowser::Unknown,
    ];
    signals.iter().filter(|b| **b).count() as f32 / signals.len() as f32
}
