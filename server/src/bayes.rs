//! Naive-Bayes scoring with hand-tuned per-feature log-likelihood-ratios.
//!
//! Each feature contributes an independent log(P(value | same)/P(value | different))
//! term. Match values are positive (raise odds of same-user); mismatches are
//! negative; missing → 0 (no evidence either way).
//!
//! These weights are calibrated by intuition + agent research. As real labels
//! accumulate (login → known same-user pairs), the table should be re-fit.

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

    add_str(
        &mut total,
        &mut hits,
        "canonical_ua",
        Some(sig.canonical_ua_hash.as_str()),
        Some(f.canonical_ua_hash.as_str()),
        6.0,
        -6.0,
    );
    add_str(
        &mut total,
        &mut hits,
        "math_fp",
        sig.math_fp_hash.as_deref(),
        f.math_fp_hash.as_deref(),
        5.0,
        -5.0,
    );
    // Noise-sensitive renders: only let them contribute when the client reported `stable`.
    // If the device is in a noise-injection regime (Brave farbling, iOS 26 ATFP), an
    // accidental match would be misleading, so we treat it as no evidence.
    if f.webgl_render_stable.unwrap_or(true) {
        add_str(
            &mut total,
            &mut hits,
            "webgl_render",
            sig.webgl_render_hash.as_deref(),
            f.webgl_render_hash.as_deref(),
            5.0,
            -1.0,
        );
    }
    if f.canvas_stable.unwrap_or(true) {
        add_str(
            &mut total,
            &mut hits,
            "canvas",
            sig.canvas_hash.as_deref(),
            f.canvas_hash.as_deref(),
            4.0,
            -1.0,
        );
    }
    if f.audio_stable.unwrap_or(true) {
        add_str(
            &mut total,
            &mut hits,
            "audio",
            sig.audio_hash.as_deref(),
            f.audio_hash.as_deref(),
            3.5,
            0.0,
        );
    }

    if let (Some(a), Some(b)) = (sig.audio_stable_checksum, f.audio_stable_checksum) {
        let v = if (a - b).abs() < 0.005 { 2.5 } else { -0.5 };
        total += v;
        hits.push(("audio_stable_checksum", v));
    }

    add_str(
        &mut total,
        &mut hits,
        "speech_voices",
        sig.speech_voices_hash.as_deref(),
        f.speech_voices_hash.as_deref(),
        4.0,
        -1.5,
    );
    add_str(
        &mut total,
        &mut hits,
        "fonts",
        sig.fonts_sorted_hash.as_deref(),
        f.fonts_sorted_hash.as_deref(),
        3.5,
        -1.0,
    );
    add_str(
        &mut total,
        &mut hits,
        "dom_rect",
        sig.dom_rect_hash.as_deref(),
        f.dom_rect_hash.as_deref(),
        3.0,
        -1.0,
    );
    add_str(
        &mut total,
        &mut hits,
        "webgl_params",
        sig.webgl_params_hash.as_deref(),
        f.webgl_params_hash.as_deref(),
        3.0,
        -1.0,
    );

    if let (Some(sw), Some(sh), Some(dpr)) = (f.screen_w, f.screen_h, f.device_pixel_ratio) {
        let dims_match = Some(sw) == sig.screen_w
            && Some(sh) == sig.screen_h
            && match sig.device_pixel_ratio {
                Some(a) => (a - dpr).abs() < 0.01,
                None => false,
            };
        let v = if dims_match { 2.0 } else { -1.0 };
        total += v;
        hits.push(("screen", v));
    }

    add_str(
        &mut total,
        &mut hits,
        "timezone",
        sig.timezone.as_deref(),
        f.timezone.as_deref(),
        1.5,
        -2.5,
    );
    add_str(
        &mut total,
        &mut hits,
        "locale",
        sig.locale.as_deref(),
        f.locale.as_deref(),
        0.5,
        -0.5,
    );

    if let (Some(a), Some(b)) = (sig.hw_concurrency, f.hw_concurrency) {
        let v = if (a - b).abs() < 0.5 { 1.0 } else { -1.5 };
        total += v;
        hits.push(("hw_concurrency", v));
    }

    add_str(
        &mut total,
        &mut hits,
        "device_model",
        sig.device_model.as_deref(),
        f.device_model.as_deref(),
        3.0,
        -3.0,
    );
    add_str(
        &mut total,
        &mut hits,
        "system_rom",
        sig.system_rom.as_deref(),
        f.system_rom.as_deref(),
        1.5,
        -2.0,
    );
    add_str(
        &mut total,
        &mut hits,
        "system_version",
        sig.system_version.as_deref(),
        f.system_version.as_deref(),
        1.0,
        -0.5,
    );
    add_str(
        &mut total,
        &mut hits,
        "in_app_version_code",
        sig.in_app_version_code.as_deref(),
        f.in_app_version_code.as_deref(),
        2.0,
        -0.3,
    );
    add_str(
        &mut total,
        &mut hits,
        "android_build",
        sig.android_build.as_deref(),
        f.android_build.as_deref(),
        2.0,
        -2.0,
    );

    if matches!(f.ua_consistent, Some(false)) {
        total -= 4.0;
        hits.push(("ua_inconsistency_penalty", -4.0));
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
        }
    }

    #[test]
    fn full_match_clears_match_threshold() {
        let s = score(&sig(), &feat());
        assert!(s.total >= 30.0, "expected ≥30, got {}", s.total);
    }

    #[test]
    fn ua_mismatch_swings_score_by_at_least_twelve() {
        let baseline = score(&sig(), &feat()).total;
        let mut f = feat();
        f.canonical_ua_hash = "different-ua".into();
        let s = score(&sig(), &f);
        assert!(
            baseline - s.total >= 12.0,
            "expected ≥12 swing, got {} → {}",
            baseline,
            s.total
        );
    }

    #[test]
    fn timezone_mismatch_significantly_penalizes() {
        let mut f = feat();
        f.timezone = Some("America/Los_Angeles".into());
        let s = score(&sig(), &f);
        let baseline = score(&sig(), &feat()).total;
        assert!(s.total < baseline - 3.0);
    }

    #[test]
    fn ua_inconsistency_subtracts_penalty() {
        let mut f = feat();
        f.ua_consistent = Some(false);
        let s = score(&sig(), &f);
        let baseline = score(&sig(), &feat()).total;
        assert!((baseline - s.total - 4.0).abs() < 0.01);
    }

    #[test]
    fn noisy_canvas_skipped_not_counted_as_mismatch() {
        let mut f = feat();
        f.canvas_stable = Some(false);
        f.canvas_hash = Some("totally-different".into());
        let s = score(&sig(), &f);
        // Canvas would normally be -1 on mismatch, but stable=false skips it.
        assert!(!s.hits.iter().any(|(name, _)| *name == "canvas"));
    }
}
