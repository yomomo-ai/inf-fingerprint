//! Feature extractor — pulls match-relevant fields out of the raw client JSON.
//!
//! Kept loosely typed (`serde_json::Value` walking) so a non-breaking schema
//! addition on the client side doesn't force a server redeploy.

use serde_json::Value;
use xxhash_rust::xxh3::Xxh3;

#[derive(Debug, Clone)]
pub struct Features {
    pub canonical_ua_hash: String,
    pub bucket_key: Vec<u8>,

    pub math_fp_hash: Option<String>,
    pub webgl_params_hash: Option<String>,
    pub webgl_render_hash: Option<String>,
    pub webgl_render_stable: Option<bool>,
    pub canvas_hash: Option<String>,
    pub canvas_stable: Option<bool>,
    pub audio_hash: Option<String>,
    pub audio_stable_checksum: Option<f64>,
    pub audio_stable: Option<bool>,
    pub speech_voices_hash: Option<String>,
    pub fonts_sorted_hash: Option<String>,
    pub dom_rect_hash: Option<String>,

    pub screen_w: Option<i32>,
    pub screen_h: Option<i32>,
    pub device_pixel_ratio: Option<f64>,
    pub color_depth: Option<i32>,
    pub hw_concurrency: Option<f64>,
    pub device_memory: Option<f64>,
    pub max_touch_points: Option<i32>,

    pub timezone: Option<String>,
    pub locale: Option<String>,
    pub language_tag: Option<String>,

    pub in_app: Option<String>,
    pub in_app_version: Option<String>,
    pub in_app_version_code: Option<String>,
    pub wechat_platform: Option<String>,
    pub device_vendor: Option<String>,
    pub system_rom: Option<String>,
    pub system_version: Option<String>,
    pub device_model: Option<String>,
    pub android_build: Option<String>,

    pub ua_consistent: Option<bool>,
    pub user_agent: Option<String>,
}

impl Features {
    pub fn from_json(v: &Value) -> Option<Features> {
        let china = v.get("china")?;
        let components = v.get("components")?;
        let integrity = v.get("integrity");

        let canonical_ua_hash = string_at(china, "canonical_ua_hash")?;

        let canvas = components.get("canvas");
        let webgl = components.get("webgl");
        let webgl_render = components.get("webgl_render");
        let audio = components.get("audio");
        let speech = components.get("speech");
        let fonts = components.get("fonts");
        let dom = components.get("dom");
        let math = components.get("math");
        let screen = components.get("screen");
        let navigator = components.get("navigator");
        let timezone = components.get("timezone");

        let math_fp_hash = math.and_then(|m| string_at(m, "hash"));
        let webgl_params_hash = webgl.and_then(|w| string_at(w, "params_hash"));
        let webgl_render_hash = webgl_render.and_then(|w| string_at(w, "pixel_hash"));
        let webgl_render_stable = webgl_render.and_then(|w| w.get("stable")?.as_bool());
        let canvas_hash = canvas.and_then(|c| string_at(c, "hash"));
        let canvas_stable = canvas.and_then(|c| c.get("stable")?.as_bool());
        let audio_hash = audio.and_then(|a| string_at(a, "hash"));
        let audio_stable_checksum = audio.and_then(|a| a.get("stable_checksum")?.as_f64());
        let audio_stable = audio.and_then(|a| a.get("stable")?.as_bool());
        let speech_voices_hash = speech.and_then(|s| string_at(s, "hash"));
        let fonts_sorted_hash = fonts.map(|f| {
            let mut names: Vec<String> = f
                .get("available")
                .and_then(|a| a.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            names.sort();
            hash_string(&names.join(","))
        });
        let dom_rect_hash = dom.and_then(|d| string_at(d, "rect_hash"));

        let screen_w = screen
            .and_then(|s| s.get("width")?.as_i64())
            .map(|n| n as i32);
        let screen_h = screen
            .and_then(|s| s.get("height")?.as_i64())
            .map(|n| n as i32);
        let device_pixel_ratio = screen.and_then(|s| s.get("device_pixel_ratio")?.as_f64());
        let color_depth = screen
            .and_then(|s| s.get("color_depth")?.as_i64())
            .map(|n| n as i32);
        let hw_concurrency = navigator.and_then(|n| n.get("hardware_concurrency")?.as_f64());
        let device_memory = navigator.and_then(|n| n.get("device_memory")?.as_f64());
        let max_touch_points = navigator
            .and_then(|n| n.get("max_touch_points")?.as_i64())
            .map(|n| n as i32);

        let timezone = timezone.and_then(|t| string_at(t, "timezone"));
        let locale = timezone.as_ref().and_then(|_| {
            v.get("components")?
                .get("timezone")?
                .get("locale")?
                .as_str()
                .map(String::from)
        });
        let language_tag = string_at(china, "language_tag");

        let in_app = string_at(china, "in_app");
        let in_app_version = string_at(china, "in_app_version");
        let in_app_version_code = string_at(china, "in_app_version_code");
        let wechat_platform = string_at(china, "wechat_platform");
        let device_vendor = string_at(china, "device_vendor");
        let system_rom = string_at(china, "system_rom");
        let system_version = string_at(china, "system_version");
        let device_model = string_at(china, "device_model");
        let android_build = string_at(china, "android_build");

        let ua_consistent = integrity.and_then(|i| i.get("ua_consistent")?.as_bool());
        let user_agent = string_at(china, "user_agent");

        let bucket_key = compute_bucket_key(
            &canonical_ua_hash,
            screen_w,
            screen_h,
            device_pixel_ratio,
            hw_concurrency,
            math_fp_hash.as_deref(),
        );

        Some(Features {
            canonical_ua_hash,
            bucket_key,
            math_fp_hash,
            webgl_params_hash,
            webgl_render_hash,
            webgl_render_stable,
            canvas_hash,
            canvas_stable,
            audio_hash,
            audio_stable_checksum,
            audio_stable,
            speech_voices_hash,
            fonts_sorted_hash,
            dom_rect_hash,
            screen_w,
            screen_h,
            device_pixel_ratio,
            color_depth,
            hw_concurrency,
            device_memory,
            max_touch_points,
            timezone,
            locale,
            language_tag,
            in_app,
            in_app_version,
            in_app_version_code,
            wechat_platform,
            device_vendor,
            system_rom,
            system_version,
            device_model,
            android_build,
            ua_consistent,
            user_agent,
        })
    }
}

fn string_at(v: &Value, key: &str) -> Option<String> {
    v.get(key)?.as_str().map(String::from)
}

fn hash_string(s: &str) -> String {
    format!("{:016x}", xxhash_rust::xxh3::xxh3_64(s.as_bytes()))
}

fn compute_bucket_key(
    canonical_ua_hash: &str,
    screen_w: Option<i32>,
    screen_h: Option<i32>,
    dpr: Option<f64>,
    hw_concurrency: Option<f64>,
    math_fp_hash: Option<&str>,
) -> Vec<u8> {
    let mut h = Xxh3::new();
    h.update(canonical_ua_hash.as_bytes());
    h.update(b"|");
    if let (Some(w), Some(ht), Some(d)) = (screen_w, screen_h, dpr) {
        h.update(format!("{}x{}@{}", w, ht, d).as_bytes());
    }
    h.update(b"|");
    if let Some(hc) = hw_concurrency {
        h.update(format!("{}", hc as i32).as_bytes());
    }
    h.update(b"|");
    if let Some(m) = math_fp_hash {
        h.update(m.as_bytes());
    }
    let n = h.digest128();
    n.to_le_bytes().to_vec()
}
