use serde::Serialize;
use wasm_bindgen::prelude::*;
use xxhash_rust::xxh3::Xxh3;

mod china;
mod components;
mod ctx;
mod hash;
mod identify;

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
    /// Includes noise-sensitive signals (canvas / webgl-render / audio raw hash).
    /// Useful for short-window analytics; flips between same-device sessions when the
    /// browser injects per-call noise (Brave farbling, Safari 17+ ATFP, iOS 26+).
    visitor_id_strict: String,
    version: &'static str,
    confidence: f32,
    integrity: Integrity,
    components: Components,
    china: china::ChinaSignals,
}

#[derive(Serialize)]
pub struct Integrity {
    /// Canvas rendered twice, hashes match.
    pub canvas_stable: bool,
    /// WebGL render-pixel rendered twice, bytes match.
    pub webgl_render_stable: bool,
    /// Audio rendered N times, all identical.
    pub audio_stable: bool,
    /// `navigator.platform` / UA / userAgentData are mutually consistent.
    pub ua_consistent: bool,
    pub noisy_count: u32,
}

#[wasm_bindgen]
impl Fingerprint {
    #[wasm_bindgen(getter, js_name = visitorId)]
    pub fn visitor_id(&self) -> String {
        self.inner.visitor_id.clone()
    }

    #[wasm_bindgen(getter, js_name = visitorIdStrict)]
    pub fn visitor_id_strict(&self) -> String {
        self.inner.visitor_id_strict.clone()
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

    /// Internal: msgpack bytes for fast wire encoding. Skips the JS bridge
    /// entirely (no serde-wasm-bindgen, no JSON.stringify), encoding directly
    /// from the Rust struct in WASM linear memory.
    pub(crate) fn to_msgpack(&self) -> Result<Vec<u8>, JsValue> {
        rmp_serde::to_vec_named(&self.inner)
            .map_err(|e| JsValue::from_str(&format!("msgpack encode: {}", e)))
    }
}

#[wasm_bindgen(js_name = getFingerprint)]
pub async fn get_fingerprint() -> Result<Fingerprint, JsValue> {
    let china_signals = china::detect();
    let components = components::collect().await;

    let integrity = check_integrity(&components, &china_signals);
    let visitor_id = compute_visitor_id(&components, &china_signals, &integrity, false);
    let visitor_id_strict = compute_visitor_id(&components, &china_signals, &integrity, true);
    let confidence = score_confidence(&components, &china_signals, &integrity);

    Ok(Fingerprint {
        inner: FingerprintData {
            visitor_id,
            visitor_id_strict,
            version: env!("CARGO_PKG_VERSION"),
            confidence,
            integrity,
            components,
            china: china_signals,
        },
    })
}

fn check_integrity(c: &Components, cn: &china::ChinaSignals) -> Integrity {
    let canvas_stable = c.canvas.as_ref().map(|x| x.stable).unwrap_or(true);
    let webgl_render_stable = c.webgl_render.as_ref().map(|x| x.stable).unwrap_or(true);
    let audio_stable = c.audio.as_ref().map(|x| x.stable).unwrap_or(true);
    let ua_consistent = check_ua_consistency(c, cn);

    let mut noisy_count = 0u32;
    if c.canvas.is_some() && !canvas_stable {
        noisy_count += 1;
    }
    if c.webgl_render.is_some() && !webgl_render_stable {
        noisy_count += 1;
    }
    if c.audio.is_some() && !audio_stable {
        noisy_count += 1;
    }
    if !ua_consistent {
        noisy_count += 1;
    }

    Integrity {
        canvas_stable,
        webgl_render_stable,
        audio_stable,
        ua_consistent,
        noisy_count,
    }
}

fn check_ua_consistency(c: &Components, cn: &china::ChinaSignals) -> bool {
    let Some(n) = &c.navigator else {
        return true;
    };
    let ua = &cn.user_agent;
    let platform = n.platform.as_deref().unwrap_or("");

    let ua_apple = ua.contains("iPhone") || ua.contains("iPad") || ua.contains("Mac OS");
    let plat_apple =
        platform.contains("iPhone") || platform.contains("iPad") || platform.starts_with("Mac");
    let ua_android = ua.contains("Android");
    let plat_android = platform.starts_with("Linux");

    if ua_apple != plat_apple {
        return false;
    }
    if ua_android && !plat_android {
        return false;
    }

    if let Some(uah) = &n.ua_client_hints {
        if let Some(uah_plat) = &uah.platform {
            if uah_plat == "iOS" && !ua_apple {
                return false;
            }
            if uah_plat == "Android" && !ua_android {
                return false;
            }
        }
    }
    true
}

fn compute_visitor_id(
    c: &Components,
    cn: &china::ChinaSignals,
    integrity: &Integrity,
    strict: bool,
) -> String {
    let mut h = Xxh3::new();

    h.update(b"inf-fp-v1");
    h.update(cn.canonical_ua_hash.as_bytes());

    if let Some(m) = &c.math {
        h.update(m.hash.as_bytes());
    }
    if let Some(w) = &c.webgl {
        h.update(w.params_hash.as_bytes());
    }
    if let Some(s) = &c.screen {
        h.update(
            format!(
                "{}x{}@{}|{}|{}",
                s.width, s.height, s.device_pixel_ratio, s.color_depth, s.pixel_depth
            )
            .as_bytes(),
        );
    }
    if let Some(n) = &c.navigator {
        let mut langs = n.languages.clone();
        langs.sort();
        h.update(
            format!(
                "{}|{}|{}|{}",
                n.hardware_concurrency,
                n.platform.as_deref().unwrap_or(""),
                n.max_touch_points,
                langs.join(",")
            )
            .as_bytes(),
        );
    }
    if let Some(tz) = &c.timezone {
        h.update(tz.timezone.as_deref().unwrap_or("").as_bytes());
        h.update(b"|");
        h.update(tz.locale.as_deref().unwrap_or("").as_bytes());
    }
    if let Some(sp) = &c.speech {
        h.update(sp.hash.as_bytes());
    }
    if let Some(t) = &c.touch {
        h.update(format!("{}|{}", t.max_touch_points, t.pointer_event).as_bytes());
    }
    if let Some(q) = &c.quirks {
        h.update(
            format!(
                "{}|{}|{}|{}|{}|{}",
                q.ios.has_window_safari,
                q.ios.has_gesture_event,
                q.ios.has_webkit_message_handlers,
                q.gamut.p3.unwrap_or(false),
                q.features.webgpu,
                q.features.offscreen_canvas,
            )
            .as_bytes(),
        );
    }
    if let Some(d) = &c.dom {
        h.update(d.rect_hash.as_bytes());
    }
    if let Some(p) = &c.perf {
        h.update(&p.time_resolution_ms.to_le_bytes());
    }
    if let Some(f) = &c.fonts {
        let mut fs = f.available.clone();
        fs.sort();
        h.update(fs.join(",").as_bytes());
    }

    // Noise-sensitive components: only included when stable, or always in strict mode.
    if strict || integrity.canvas_stable {
        if let Some(canvas) = &c.canvas {
            h.update(canvas.hash.as_bytes());
        }
    }
    if strict || integrity.webgl_render_stable {
        if let Some(wr) = &c.webgl_render {
            h.update(wr.pixel_hash.as_bytes());
        }
    }
    if strict || integrity.audio_stable {
        if let Some(a) = &c.audio {
            h.update(a.hash.as_bytes());
        }
    } else if let Some(a) = &c.audio {
        // Audio is noisy but the rounded median is still device-discriminating.
        h.update(&a.stable_checksum.to_le_bytes());
    }

    format!("{:016x}", h.digest())
}

fn score_confidence(c: &Components, cn: &china::ChinaSignals, integrity: &Integrity) -> f32 {
    let signals = [
        c.canvas.is_some(),
        c.webgl.is_some(),
        c.webgl_render.is_some(),
        c.audio.is_some(),
        c.screen.is_some(),
        c.navigator.is_some(),
        c.timezone.is_some(),
        c.fonts.is_some(),
        c.touch.is_some(),
        c.permissions.is_some(),
        c.math.is_some(),
        c.speech.is_some(),
        c.connection.is_some(),
        c.perf.is_some(),
        c.dom.is_some(),
        c.webrtc.is_some(),
        c.quirks.is_some(),
        cn.in_app != china::InAppBrowser::Unknown,
    ];
    let have = signals.iter().filter(|b| **b).count();
    let base = have as f32 / signals.len() as f32;
    let penalty = (integrity.noisy_count as f32) * 0.05;
    (base - penalty).max(0.0)
}
