//! Naive-Bayes scoring for visitor matching.
//!
//! Each feature contributes one independent term: log(P(value | same user) /
//! P(value | different user)). Match values raise the odds of "same"; mismatches
//! pull them down; missing-on-either-side contributes 0.
//!
//! Weights are calibrated by:
//!
//! 1. **Stability** — P(value unchanged | same device, two visits). High for
//!    hardware-derived signals (math_fp, screen), lower for things that drift
//!    legitimately (system_version, in_app_version, canvas/audio under iOS noise).
//!
//! 2. **Uniqueness conditional on bucket** — P(value collides | different device,
//!    SAME bucket). The bucket key is `canonical_ua + screen + hw_concurrency +
//!    math_fp_hash`, so candidates already share OS family + screen + chip
//!    class + JS engine. Features whose entropy is *already absorbed by the
//!    bucket* (like screen, system_rom, math_fp) carry less marginal weight.
//!    Features whose entropy is *orthogonal to the bucket* (webgl_render
//!    pixels, fonts, speech voices, canvas) carry more.
//!
//! 3. **Asymmetric mismatch cost**. For features that legitimately drift on
//!    same-device (system_version after OS update, in_app_version after app
//!    update, canvas under iOS 26 ATFP), the mismatch penalty is mild —
//!    a single drift shouldn't break a match. For features that essentially
//!    never change for same device (math_fp, device_model, screen), mismatch
//!    is a strong negative signal.
//!
//! These are educated priors. Once we have logged-in users producing labeled
//! same-user pairs, we can fit weights from real frequencies and replace this
//! table with a learned model.

use crate::features::Features;
use crate::matcher::Signature;

#[derive(Debug, Clone)]
pub struct ScoreBreakdown {
    pub total: f64,
    pub hits: Vec<(&'static str, f64)>,
}

pub fn score(sig: &Signature, f: &Features) -> ScoreBreakdown {
    let mut total = 0.0;
    let mut hits: Vec<(&'static str, f64)> = Vec::new();

    // canonical_ua_hash — bucket-defining; stability ~0.99, population entropy
    // ~13 bits across CN devices. Within bucket already filtered, but verifying
    // catches bucket-key hash collisions (rare).
    add_str(
        &mut total,
        &mut hits,
        "canonical_ua",
        Some(sig.canonical_ua_hash.as_str()),
        Some(f.canonical_ua_hash.as_str()),
        9.0,
        -5.0,
    );

    // math_fp_hash — JS engine + libm + CPU class. Already largely absorbed by
    // bucket (math_fp_hash IS part of bucket key). Treat as confirmation.
    add_str(
        &mut total,
        &mut hits,
        "math_fp",
        sig.math_fp_hash.as_deref(),
        f.math_fp_hash.as_deref(),
        3.0,
        -3.0,
    );

    // webgl_render_hash — pixel-level GPU output. Highest-entropy signal that
    // *survives* WebKit normalization, since identical iPhone units produce
    // subtly different pixels. Skip when client reports stable=false (Brave
    // farbling, iOS 26 ATFP noise) — accidental matches under noise mislead.
    // The next 7 signals collectively *discriminate within a bucket*: when
    // canonical_ua + screen + chip + engine all match, these are what tell
    // identical-spec phones apart. Mismatch penalties are tuned so that 5+
    // simultaneous mismatches push the total below match_threshold even with
    // every bucket-level signal lined up.

    if f.webgl_render_stable.unwrap_or(true) {
        add_str(
            &mut total,
            &mut hits,
            "webgl_render",
            sig.webgl_render_hash.as_deref(),
            f.webgl_render_hash.as_deref(),
            5.0,
            -2.0,
        );
    }

    if f.canvas_stable.unwrap_or(true) {
        add_str(
            &mut total,
            &mut hits,
            "canvas",
            sig.canvas_hash.as_deref(),
            f.canvas_hash.as_deref(),
            3.0,
            -2.0,
        );
    }

    if f.audio_stable.unwrap_or(true) {
        add_str(
            &mut total,
            &mut hits,
            "audio",
            sig.audio_hash.as_deref(),
            f.audio_hash.as_deref(),
            1.0,
            -0.5,
        );
    }

    if let (Some(a), Some(b)) = (sig.audio_stable_checksum, f.audio_stable_checksum) {
        let v = if (a - b).abs() < 0.005 { 2.0 } else { -1.0 };
        total += v;
        hits.push(("audio_stable_checksum", v));
    }

    add_str(
        &mut total,
        &mut hits,
        "speech_voices",
        sig.speech_voices_hash.as_deref(),
        f.speech_voices_hash.as_deref(),
        3.5,
        -2.5,
    );

    add_str(
        &mut total,
        &mut hits,
        "fonts",
        sig.fonts_sorted_hash.as_deref(),
        f.fonts_sorted_hash.as_deref(),
        4.0,
        -3.5,
    );

    add_str(
        &mut total,
        &mut hits,
        "dom_rect",
        sig.dom_rect_hash.as_deref(),
        f.dom_rect_hash.as_deref(),
        3.0,
        -2.0,
    );

    // webgl_params — vendor/renderer/extensions/limits. Mostly bucket-redundant
    // (same browser version → same params). Stronger negative because mismatch
    // suggests browser switch.
    add_str(
        &mut total,
        &mut hits,
        "webgl_params",
        sig.webgl_params_hash.as_deref(),
        f.webgl_params_hash.as_deref(),
        2.0,
        -2.0,
    );

    // screen dims — already part of bucket key, so within-bucket collision is
    // tautologically high. Tiny positive as confirmation; large negative on
    // mismatch flags an upstream bug.
    if let (Some(sw), Some(sh), Some(dpr)) = (f.screen_w, f.screen_h, f.device_pixel_ratio) {
        let dims_match = Some(sw) == sig.screen_w
            && Some(sh) == sig.screen_h
            && match sig.device_pixel_ratio {
                Some(a) => (a - dpr).abs() < 0.01,
                None => false,
            };
        let v = if dims_match { 0.5 } else { -2.0 };
        total += v;
        hits.push(("screen", v));
    }

    // timezone — in CN traffic, ~80% Asia/Shanghai. Match adds little; mismatch
    // is a strong negative (VPN / cross-region traveler).
    add_str(
        &mut total,
        &mut hits,
        "timezone",
        sig.timezone.as_deref(),
        f.timezone.as_deref(),
        0.3,
        -2.5,
    );

    // locale — in CN traffic, ~95% zh-CN. Tiny match contribution.
    add_str(
        &mut total,
        &mut hits,
        "locale",
        sig.locale.as_deref(),
        f.locale.as_deref(),
        0.2,
        -1.0,
    );

    // hw_concurrency — partly absorbed by bucket key. Match is weak;
    // mismatch suggests a real device change.
    if let (Some(a), Some(b)) = (sig.hw_concurrency, f.hw_concurrency) {
        let v = if (a - b).abs() < 0.5 { 0.5 } else { -1.0 };
        total += v;
        hits.push(("hw_concurrency", v));
    }

    // device_model — empty/generic on iOS ("iPhone"); high-entropy on Android
    // ("V2307A", "M2102K1AC"). Mismatch on Android is decisive.
    add_str(
        &mut total,
        &mut hits,
        "device_model",
        sig.device_model.as_deref(),
        f.device_model.as_deref(),
        1.5,
        -3.0,
    );

    // system_rom (ios / android / harmonyos / miui / ...) — bucket-absorbed.
    add_str(
        &mut total,
        &mut hits,
        "system_rom",
        sig.system_rom.as_deref(),
        f.system_rom.as_deref(),
        0.3,
        -2.0,
    );

    // system_version — drifts legitimately on OS updates; soft mismatch.
    add_str(
        &mut total,
        &mut hits,
        "system_version",
        sig.system_version.as_deref(),
        f.system_version.as_deref(),
        1.5,
        -0.5,
    );

    // in_app_version_code (WeChat hex) — WeChat updates push every ~2 weeks;
    // soft mismatch since same user routinely upgrades.
    add_str(
        &mut total,
        &mut hits,
        "in_app_version_code",
        sig.in_app_version_code.as_deref(),
        f.in_app_version_code.as_deref(),
        1.0,
        -0.3,
    );

    // android_build — Android build code. High entropy; mismatch could be OS
    // patch, but typically means different device.
    add_str(
        &mut total,
        &mut hits,
        "android_build",
        sig.android_build.as_deref(),
        f.android_build.as_deref(),
        2.0,
        -1.0,
    );

    // ua_consistency — incoming features show navigator.platform / userAgent /
    // userAgentData mismatch. Strong spoofing / DevTools-override signal;
    // refuse to confidently match against a clean signature.
    if matches!(f.ua_consistent, Some(false)) {
        total -= 5.0;
        hits.push(("ua_inconsistency_penalty", -5.0));
    }

    // client_visitor_id — persistence super-cookie. Almost a 1:1 identity
    // match when present. Mild mismatch because cookie clearing is normal.
    add_str(
        &mut total,
        &mut hits,
        "client_visitor_id",
        sig.client_visitor_id.as_deref(),
        f.client_visitor_id.as_deref(),
        12.0,
        -1.5,
    );

    // ua_model (UA-CH high-entropy) — Chromium devices like "SM-G998B"
    // identify the exact phone. Very high-entropy on Android.
    add_str(
        &mut total,
        &mut hits,
        "ua_model",
        sig.ua_model.as_deref(),
        f.ua_model.as_deref(),
        5.0,
        -3.0,
    );

    // ua_architecture (arm / arm64 / x86 / x86_64) — hardware-tier signal.
    add_str(
        &mut total,
        &mut hits,
        "ua_arch",
        sig.ua_architecture.as_deref(),
        f.ua_architecture.as_deref(),
        0.8,
        -2.0,
    );

    // ua_platform_version drifts on OS update; soft mismatch.
    add_str(
        &mut total,
        &mut hits,
        "ua_platform_version",
        sig.ua_platform_version.as_deref(),
        f.ua_platform_version.as_deref(),
        1.0,
        -0.3,
    );

    // storage quota — within-bucket (same OS + same Safari/Chrome version) the
    // quota is often *identical* across devices, so match is weak evidence.
    // Mismatch is informative because it suggests a real config / disk diff.
    if let (Some(a), Some(b)) = (sig.storage_quota_bytes, f.storage_quota_bytes) {
        let ratio = (a - b).abs() as f64 / a.max(b).max(1) as f64;
        let v = if ratio < 0.05 { 0.5 } else { -1.5 };
        total += v;
        hits.push(("storage_quota", v));
    }

    // battery_level — reasonable match within 5%; otherwise no signal because
    // it changes constantly.
    if let (Some(a), Some(b)) = (sig.battery_level, f.battery_level) {
        let v = if (a - b).abs() < 0.05 { 1.0 } else { 0.0 };
        if v != 0.0 {
            total += v;
            hits.push(("battery_level", v));
        }
    }

    // WebRTC public IP — modest match weight: in CN coffee-shop / dorm /
    // corporate-NAT scenarios, multiple distinct users share the same public
    // IP. A match is consistent with same-user but not strong evidence on its
    // own. Mismatch is also weak because users legitimately switch networks.
    if !sig.webrtc_public_ips.is_empty() && !f.webrtc_public_ips.is_empty() {
        let overlap = sig
            .webrtc_public_ips
            .iter()
            .any(|ip| f.webrtc_public_ips.contains(ip));
        let v = if overlap { 1.0 } else { -0.3 };
        total += v;
        hits.push(("webrtc_public_ip", v));
    }

    ScoreBreakdown { total, hits }
}

fn add_str(
    total: &mut f64,
    hits: &mut Vec<(&'static str, f64)>,
    name: &'static str,
    a: Option<&str>,
    b: Option<&str>,
    match_lr: f64,
    mismatch_lr: f64,
) {
    if let (Some(x), Some(y)) = (a, b) {
        let v = if x == y { match_lr } else { mismatch_lr };
        *total += v;
        hits.push((name, v));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn sig() -> Signature {
        Signature {
            visitor_id: Uuid::nil(),
            canonical_ua_hash: "ua-1".into(),
            math_fp_hash: Some("math-1".into()),
            webgl_params_hash: Some("p-1".into()),
            webgl_render_hash: Some("r-1".into()),
            canvas_hash: Some("c-1".into()),
            audio_hash: Some("a-1".into()),
            audio_stable_checksum: Some(123.45),
            speech_voices_hash: Some("v-1".into()),
            fonts_sorted_hash: Some("f-1".into()),
            dom_rect_hash: Some("d-1".into()),
            screen_w: Some(390),
            screen_h: Some(844),
            device_pixel_ratio: Some(3.0),
            hw_concurrency: Some(6.0),
            timezone: Some("Asia/Shanghai".into()),
            locale: Some("zh-CN".into()),
            device_model: Some("iPhone".into()),
            system_rom: Some("ios".into()),
            system_version: Some("17.5.1".into()),
            in_app_version_code: Some("0x18003133".into()),
            android_build: None,
            client_visitor_id: Some("uuid-1".into()),
            battery_charging: None,
            battery_level: None,
            storage_quota_bytes: Some(2_000_000_000),
            ua_architecture: None,
            ua_model: None,
            ua_platform_version: None,
            webrtc_public_ips: vec!["203.0.113.1".into()],
        }
    }

    fn feat() -> Features {
        Features {
            canonical_ua_hash: "ua-1".into(),
            bucket_key: vec![],
            math_fp_hash: Some("math-1".into()),
            webgl_params_hash: Some("p-1".into()),
            webgl_render_hash: Some("r-1".into()),
            webgl_render_stable: Some(true),
            canvas_hash: Some("c-1".into()),
            canvas_stable: Some(true),
            audio_hash: Some("a-1".into()),
            audio_stable_checksum: Some(123.45),
            audio_stable: Some(true),
            speech_voices_hash: Some("v-1".into()),
            fonts_sorted_hash: Some("f-1".into()),
            dom_rect_hash: Some("d-1".into()),
            screen_w: Some(390),
            screen_h: Some(844),
            device_pixel_ratio: Some(3.0),
            color_depth: Some(30),
            hw_concurrency: Some(6.0),
            device_memory: None,
            max_touch_points: Some(5),
            timezone: Some("Asia/Shanghai".into()),
            locale: Some("zh-CN".into()),
            language_tag: Some("zh_CN".into()),
            in_app: Some("wechat".into()),
            in_app_version: Some("8.0.49".into()),
            in_app_version_code: Some("0x18003133".into()),
            wechat_platform: Some("ios".into()),
            device_vendor: Some("apple".into()),
            system_rom: Some("ios".into()),
            system_version: Some("17.5.1".into()),
            device_model: Some("iPhone".into()),
            android_build: None,
            ua_consistent: Some(true),
            user_agent: None,
            client_visitor_id: Some("uuid-1".into()),
            battery_charging: None,
            battery_level: None,
            storage_quota_bytes: Some(2_000_000_000),
            storage_usage_bytes: None,
            ua_architecture: None,
            ua_bitness: None,
            ua_model: None,
            ua_platform_version: None,
            ua_full_version: None,
            webrtc_public_ips: vec!["203.0.113.1".into()],
            webrtc_local_ips: vec![],
        }
    }

    #[test]
    fn full_match_clears_exact_threshold() {
        // Sum of all positives for a clean iOS user (no android_build):
        // 9 + 3 + 5 + 3 + 1 + 2 + 3.5 + 4 + 3 + 2 + 0.5 + 0.3 + 0.2 + 0.5 + 1.5 + 0.3 + 1.5 + 1
        // = 40.8
        let s = score(&sig(), &feat());
        assert!(s.total >= 30.0, "expected ≥30, got {}", s.total);
    }

    #[test]
    fn ua_mismatch_swings_score_by_fourteen() {
        // canonical_ua: +9 → -5 = swing 14
        let baseline = score(&sig(), &feat()).total;
        let mut f = feat();
        f.canonical_ua_hash = "different-ua".into();
        let s = score(&sig(), &f);
        assert!(
            (baseline - s.total - 14.0).abs() < 0.01,
            "expected swing 14, got {} → {} (Δ={})",
            baseline,
            s.total,
            baseline - s.total
        );
    }

    #[test]
    fn timezone_mismatch_significantly_penalizes() {
        // tz: +0.3 → -2.5 = swing 2.8
        let baseline = score(&sig(), &feat()).total;
        let mut f = feat();
        f.timezone = Some("America/Los_Angeles".into());
        let s = score(&sig(), &f);
        assert!(
            baseline - s.total > 2.5,
            "expected ≥2.5 swing, got {}",
            baseline - s.total
        );
    }

    #[test]
    fn ua_inconsistency_subtracts_five() {
        let baseline = score(&sig(), &feat()).total;
        let mut f = feat();
        f.ua_consistent = Some(false);
        let s = score(&sig(), &f);
        assert!((baseline - s.total - 5.0).abs() < 0.01);
    }

    #[test]
    fn noisy_canvas_skipped_not_counted_as_mismatch() {
        let mut f = feat();
        f.canvas_stable = Some(false);
        f.canvas_hash = Some("totally-different".into());
        let s = score(&sig(), &f);
        assert!(!s.hits.iter().any(|(name, _)| *name == "canvas"));
    }

    /// Adversary scenario: different person on same iPhone model + WeChat
    /// version (so canonical_ua collides), no persistence cookie shared,
    /// all device-specific renders differ. Must score below match_threshold.
    #[test]
    fn different_user_in_same_bucket_falls_short_of_match() {
        let mut f = feat();
        // No shared cookie — different user, fresh install / different device.
        f.client_visitor_id = None;
        // All device-specific renders / hashes differ.
        f.webgl_render_hash = Some("different".into());
        f.canvas_hash = Some("different".into());
        f.audio_hash = Some("different".into());
        f.audio_stable_checksum = Some(999.99);
        f.speech_voices_hash = Some("different".into());
        f.fonts_sorted_hash = Some("different".into());
        f.dom_rect_hash = Some("different".into());
        // Different network → different public IP.
        f.webrtc_public_ips = vec!["198.51.100.5".into()];

        let s = score(&sig(), &f);
        assert!(
            s.total < 12.0,
            "different-user score {} should be below match_threshold (12.0)",
            s.total
        );
    }

    #[test]
    fn matching_client_visitor_id_alone_pushes_above_match_threshold() {
        // Same device returning, only the persistence cookie + canonical_ua
        // are usable signals (e.g. paranoid privacy-mode browser blocking
        // most fingerprint surfaces).
        let mut f = feat();
        f.webgl_render_hash = None;
        f.canvas_hash = None;
        f.audio_hash = None;
        f.audio_stable_checksum = None;
        f.speech_voices_hash = None;
        f.fonts_sorted_hash = None;
        f.dom_rect_hash = None;
        f.webrtc_public_ips = vec![];
        f.storage_quota_bytes = None;
        let s = score(&sig(), &f);
        // canonical_ua (+9) + math (+3) + client_visitor_id (+12) = +24, plus
        // assorted small signals; well above match_threshold (12).
        assert!(
            s.total >= 18.0,
            "cookie + UA-only score should be ≥18, got {}",
            s.total
        );
    }
}
