//! Feature extractor — pulls match-relevant fields out of the raw client JSON.
//!
//! Kept loosely typed (`serde_json::Value` walking) so a non-breaking schema
//! addition on the client side doesn't force a server redeploy.

use serde_json::Value;
use xxhash_rust::xxh3::Xxh3;

/// Recall-bucket dimensions. Each is an *independent* high-stability,
/// high-cardinality feature — a signature is indexed under every dimension
/// it has a value for, and a candidate matches if ANY single dimension
/// agrees with the request. Bayes does the actual scoring with the full
/// feature set; bucketing is pure recall optimization, not discrimination.
///
/// These constants must match the `bucket_kind` values in migration
/// `0003_signature_recall_buckets.sql`.
pub const BUCKET_KIND_CANVAS: i16 = 0;
pub const BUCKET_KIND_WEBGL_RENDER: i16 = 1;
pub const BUCKET_KIND_WEBGL_PARAMS: i16 = 2;
pub const BUCKET_KIND_FONTS: i16 = 3;
pub const BUCKET_KIND_AUDIO: i16 = 4;

#[derive(Debug, Clone)]
pub struct Features {
    pub canonical_ua_hash: String,
    /// Recall hooks: `(bucket_kind, hex_hash_string)` pairs. Sent to the
    /// matcher's multi-bucket lookup as a UNION over all dimensions.
    pub bucket_recall_keys: Vec<(i16, String)>,

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

    // Persistence super-cookie. Strongest single signal when present + verified.
    pub client_visitor_id: Option<String>,

    // Battery (Chromium / X5 / XWEB; absent on iOS Safari).
    pub battery_charging: Option<bool>,
    pub battery_level: Option<f64>,

    // StorageManager.estimate() — quota fairly stable per device.
    pub storage_quota_bytes: Option<i64>,
    pub storage_usage_bytes: Option<i64>,

    // UA Client Hints high-entropy (Chromium async API).
    pub ua_architecture: Option<String>,
    pub ua_bitness: Option<String>,
    pub ua_model: Option<String>,
    pub ua_platform_version: Option<String>,
    pub ua_full_version: Option<String>,

    // WebRTC IPs.
    pub webrtc_public_ips: Vec<String>,
    pub webrtc_local_ips: Vec<String>,
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

        // New: persistence + battery + storage + UA-hints + webrtc public IP.
        let persist = components.get("persist");
        let battery = components.get("battery");
        let storage_quota = components.get("storage_quota");
        let ua_high = components.get("ua_high");
        let webrtc = components.get("webrtc");

        let client_visitor_id = persist.and_then(|p| string_at(p, "client_visitor_id"));
        let battery_charging = battery.and_then(|b| b.get("charging")?.as_bool());
        let battery_level = battery.and_then(|b| b.get("level")?.as_f64());
        let storage_quota_bytes = storage_quota
            .and_then(|s| s.get("quota_bytes")?.as_f64())
            .map(|v| v as i64);
        let storage_usage_bytes = storage_quota
            .and_then(|s| s.get("usage_bytes")?.as_f64())
            .map(|v| v as i64);

        let ua_architecture = ua_high.and_then(|u| string_at(u, "architecture"));
        let ua_bitness = ua_high.and_then(|u| string_at(u, "bitness"));
        let ua_model = ua_high.and_then(|u| string_at(u, "model"));
        let ua_platform_version = ua_high.and_then(|u| string_at(u, "platform_version"));
        let ua_full_version = ua_high.and_then(|u| string_at(u, "ua_full_version"));

        let extract_str_array = |key: &str| -> Vec<String> {
            webrtc
                .and_then(|w| w.get(key)?.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default()
        };
        let webrtc_public_ips = extract_str_array("public_ips");
        let webrtc_local_ips = extract_str_array("local_ips");

        let bucket_recall_keys = compute_bucket_recall_keys(
            canvas_hash.as_deref(),
            webgl_render_hash.as_deref(),
            webgl_params_hash.as_deref(),
            fonts_sorted_hash.as_deref(),
            audio_hash.as_deref(),
        );

        Some(Features {
            canonical_ua_hash,
            bucket_recall_keys,
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
            client_visitor_id,
            battery_charging,
            battery_level,
            storage_quota_bytes,
            storage_usage_bytes,
            ua_architecture,
            ua_bitness,
            ua_model,
            ua_platform_version,
            ua_full_version,
            webrtc_public_ips,
            webrtc_local_ips,
        })
    }
}

fn string_at(v: &Value, key: &str) -> Option<String> {
    v.get(key)?.as_str().map(String::from)
}

fn hash_string(s: &str) -> String {
    format!("{:016x}", xxhash_rust::xxh3::xxh3_64(s.as_bytes()))
}

fn compute_bucket_recall_keys(
    canvas_hash: Option<&str>,
    webgl_render_hash: Option<&str>,
    webgl_params_hash: Option<&str>,
    fonts_sorted_hash: Option<&str>,
    audio_hash: Option<&str>,
) -> Vec<(i16, String)> {
    let mut keys = Vec::with_capacity(5);
    let mut push = |kind: i16, v: Option<&str>| {
        if let Some(s) = v {
            if !s.is_empty() {
                keys.push((kind, s.to_string()));
            }
        }
    };
    push(BUCKET_KIND_CANVAS, canvas_hash);
    push(BUCKET_KIND_WEBGL_RENDER, webgl_render_hash);
    push(BUCKET_KIND_WEBGL_PARAMS, webgl_params_hash);
    push(BUCKET_KIND_FONTS, fonts_sorted_hash);
    push(BUCKET_KIND_AUDIO, audio_hash);
    keys
}

/// Cache key for the bucket-cache map. Same set of recall keys (regardless
/// of order) → same cache hit. Different recall keys → different cache slot.
pub fn recall_cache_key(keys: &[(i16, String)]) -> Vec<u8> {
    let mut sorted: Vec<&(i16, String)> = keys.iter().collect();
    sorted.sort();
    let mut h = Xxh3::new();
    for (kind, value) in sorted {
        h.update(&kind.to_le_bytes());
        h.update(b":");
        h.update(value.as_bytes());
        h.update(b"|");
    }
    h.digest128().to_le_bytes().to_vec()
}
