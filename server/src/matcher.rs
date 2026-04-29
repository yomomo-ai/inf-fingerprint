use anyhow::{Context, Result};
use serde::Serialize;
use sqlx::postgres::PgPool;
use sqlx::types::Json as SqlxJson;
use uuid::Uuid;

use crate::bayes;
use crate::features::Features;

#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MatchKind {
    Exact,
    Fuzzy,
    Ambiguous,
    New,
}

#[derive(Serialize, Debug, Clone)]
pub struct CandidateScore {
    pub visitor_id: Uuid,
    pub score: f64,
    pub hits: Vec<(&'static str, f64)>,
}

#[derive(Debug)]
pub struct Outcome {
    pub visitor_id: Uuid,
    pub match_kind: MatchKind,
    pub score: f64,
    pub second_score: f64,
    pub candidates: Vec<CandidateScore>,
    pub drift: Vec<&'static str>,
    pub observation_count: i64,
    /// True when the match came from the persistence-cookie fast path rather
    /// than the bucket scan. Useful to spot cookie-stuffing fraud.
    pub via_persistence: bool,
}

#[derive(Debug, Clone)]
pub struct Signature {
    pub visitor_id: Uuid,
    pub canonical_ua_hash: String,
    pub math_fp_hash: Option<String>,
    pub webgl_params_hash: Option<String>,
    pub webgl_render_hash: Option<String>,
    pub canvas_hash: Option<String>,
    pub audio_hash: Option<String>,
    pub audio_stable_checksum: Option<f64>,
    pub speech_voices_hash: Option<String>,
    pub fonts_sorted_hash: Option<String>,
    pub dom_rect_hash: Option<String>,
    pub screen_w: Option<i32>,
    pub screen_h: Option<i32>,
    pub device_pixel_ratio: Option<f64>,
    pub hw_concurrency: Option<f64>,
    pub timezone: Option<String>,
    pub locale: Option<String>,
    pub device_model: Option<String>,
    pub system_rom: Option<String>,
    pub system_version: Option<String>,
    pub in_app_version_code: Option<String>,
    pub android_build: Option<String>,
    pub client_visitor_id: Option<String>,
    /// Stored but not currently scored — toggles too frequently to be useful
    /// for matching. Kept around for behavioral analysis later.
    #[allow(dead_code)]
    pub battery_charging: Option<bool>,
    pub battery_level: Option<f64>,
    pub storage_quota_bytes: Option<i64>,
    pub ua_architecture: Option<String>,
    pub ua_model: Option<String>,
    pub ua_platform_version: Option<String>,
    pub webrtc_public_ips: Vec<String>,
}

/// HTTP-derived request context. Captured for observation logging and to
/// supplement client-side signals (server-derived IP is harder to spoof than
/// client-side WebRTC).
#[derive(Debug, Clone, Default)]
pub struct RequestContext {
    pub ip: Option<String>,
    pub user_agent: Option<String>,
    pub accept_language: Option<String>,
    pub sec_ch_ua: Option<String>,
    pub sec_ch_ua_platform: Option<String>,
    pub referer: Option<String>,
    pub dnt: Option<bool>,
}

pub async fn identify(
    pool: &PgPool,
    f: &Features,
    raw: &serde_json::Value,
    req: &RequestContext,
    match_threshold: f64,
    ambiguous_threshold: f64,
    max_candidates: usize,
) -> Result<Outcome> {
    let mut tx = pool.begin().await?;

    // Fast path: client-side persistence cookie. If the claimed id resolves to
    // a stored signature AND that signature scores well against incoming
    // features, we trust the claim — single indexed query, no bucket scan.
    if let Some(claimed) = f.client_visitor_id.as_deref() {
        if let Some(row) = sqlx::query_as::<_, SignatureRow>(SIG_SELECT_BY_CLIENT_ID)
            .bind(claimed)
            .fetch_optional(&mut *tx)
            .await
            .context("client_visitor_id lookup")?
        {
            let sig = row.to_signature();
            let s = bayes::score(&sig, f);
            if s.total >= ambiguous_threshold {
                let drift = compute_drift_against_db(&[row], sig.visitor_id, f);
                update_signature(&mut tx, sig.visitor_id, f).await?;
                let count = bump_observation(&mut tx, sig.visitor_id).await?;
                let kind = if s.total >= match_threshold + 10.0 {
                    MatchKind::Exact
                } else if s.total >= match_threshold {
                    MatchKind::Fuzzy
                } else {
                    MatchKind::Ambiguous
                };
                insert_observation(&mut tx, sig.visitor_id, f, raw, req, s.total, kind).await?;
                tx.commit().await?;
                return Ok(Outcome {
                    visitor_id: sig.visitor_id,
                    match_kind: kind,
                    score: s.total,
                    second_score: f64::NEG_INFINITY,
                    candidates: vec![CandidateScore {
                        visitor_id: sig.visitor_id,
                        score: s.total,
                        hits: s.hits,
                    }],
                    drift,
                    observation_count: count,
                    via_persistence: true,
                });
            }
            // Cookie value collides with a known visitor but features don't
            // match — likely cookie reset on a different device, or stolen
            // cookie. Fall through to bucket scan; we'll either re-match by
            // features or create a new visitor.
        }
    }

    // Bucket scan.
    let candidates = sqlx::query_as::<_, SignatureRow>(SIG_SELECT_BY_BUCKET)
        .bind(&f.bucket_key)
        .bind(max_candidates as i64)
        .fetch_all(&mut *tx)
        .await
        .context("fetching bucket candidates")?;

    let mut scored: Vec<CandidateScore> = candidates
        .iter()
        .map(|row| {
            let sig = row.to_signature();
            let s = bayes::score(&sig, f);
            CandidateScore {
                visitor_id: sig.visitor_id,
                score: s.total,
                hits: s.hits,
            }
        })
        .collect();
    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let best = scored.first().cloned();
    let second_score = scored.get(1).map(|c| c.score).unwrap_or(f64::NEG_INFINITY);

    let (visitor_id, match_kind, drift) = match &best {
        Some(top) if top.score >= match_threshold => {
            let drift = compute_drift_against_db(&candidates, top.visitor_id, f);
            update_signature(&mut tx, top.visitor_id, f).await?;
            let kind = if top.score >= match_threshold + 10.0 {
                MatchKind::Exact
            } else {
                MatchKind::Fuzzy
            };
            (top.visitor_id, kind, drift)
        }
        Some(top) if top.score >= ambiguous_threshold => {
            let id = create_visitor(&mut tx, f).await?;
            (id, MatchKind::Ambiguous, Vec::new())
        }
        _ => {
            let id = create_visitor(&mut tx, f).await?;
            (id, MatchKind::New, Vec::new())
        }
    };

    let count = bump_observation(&mut tx, visitor_id).await?;
    insert_observation(
        &mut tx,
        visitor_id,
        f,
        raw,
        req,
        best.as_ref().map(|c| c.score).unwrap_or(0.0),
        match_kind,
    )
    .await?;

    tx.commit().await?;

    Ok(Outcome {
        visitor_id,
        match_kind,
        score: best.as_ref().map(|c| c.score).unwrap_or(0.0),
        second_score,
        candidates: scored.into_iter().take(5).collect(),
        drift,
        observation_count: count,
        via_persistence: false,
    })
}

const SIG_COLUMNS: &str = "visitor_id, canonical_ua_hash, math_fp_hash, webgl_params_hash,
    webgl_render_hash, canvas_hash, audio_hash, audio_stable_checksum,
    speech_voices_hash, fonts_sorted_hash, dom_rect_hash,
    screen_w, screen_h, device_pixel_ratio, hw_concurrency,
    timezone, locale, device_model, system_rom, system_version,
    in_app_version_code, android_build,
    client_visitor_id, battery_charging, battery_level, storage_quota_bytes,
    ua_architecture, ua_model, ua_platform_version, webrtc_public_ips";

const SIG_SELECT_BY_BUCKET: &str = const_format::concatcp!(
    "SELECT ",
    SIG_COLUMNS,
    " FROM signatures WHERE bucket_key = $1 LIMIT $2"
);
const SIG_SELECT_BY_CLIENT_ID: &str = const_format::concatcp!(
    "SELECT ",
    SIG_COLUMNS,
    " FROM signatures WHERE client_visitor_id = $1 LIMIT 1"
);

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct SignatureRow {
    visitor_id: Uuid,
    canonical_ua_hash: String,
    math_fp_hash: Option<String>,
    webgl_params_hash: Option<String>,
    webgl_render_hash: Option<String>,
    canvas_hash: Option<String>,
    audio_hash: Option<String>,
    audio_stable_checksum: Option<f64>,
    speech_voices_hash: Option<String>,
    fonts_sorted_hash: Option<String>,
    dom_rect_hash: Option<String>,
    screen_w: Option<i32>,
    screen_h: Option<i32>,
    device_pixel_ratio: Option<f64>,
    hw_concurrency: Option<f64>,
    timezone: Option<String>,
    locale: Option<String>,
    device_model: Option<String>,
    system_rom: Option<String>,
    system_version: Option<String>,
    in_app_version_code: Option<String>,
    android_build: Option<String>,
    client_visitor_id: Option<String>,
    battery_charging: Option<bool>,
    battery_level: Option<f64>,
    storage_quota_bytes: Option<i64>,
    ua_architecture: Option<String>,
    ua_model: Option<String>,
    ua_platform_version: Option<String>,
    webrtc_public_ips: Option<Vec<String>>,
}

impl SignatureRow {
    fn to_signature(&self) -> Signature {
        Signature {
            visitor_id: self.visitor_id,
            canonical_ua_hash: self.canonical_ua_hash.clone(),
            math_fp_hash: self.math_fp_hash.clone(),
            webgl_params_hash: self.webgl_params_hash.clone(),
            webgl_render_hash: self.webgl_render_hash.clone(),
            canvas_hash: self.canvas_hash.clone(),
            audio_hash: self.audio_hash.clone(),
            audio_stable_checksum: self.audio_stable_checksum,
            speech_voices_hash: self.speech_voices_hash.clone(),
            fonts_sorted_hash: self.fonts_sorted_hash.clone(),
            dom_rect_hash: self.dom_rect_hash.clone(),
            screen_w: self.screen_w,
            screen_h: self.screen_h,
            device_pixel_ratio: self.device_pixel_ratio,
            hw_concurrency: self.hw_concurrency,
            timezone: self.timezone.clone(),
            locale: self.locale.clone(),
            device_model: self.device_model.clone(),
            system_rom: self.system_rom.clone(),
            system_version: self.system_version.clone(),
            in_app_version_code: self.in_app_version_code.clone(),
            android_build: self.android_build.clone(),
            client_visitor_id: self.client_visitor_id.clone(),
            battery_charging: self.battery_charging,
            battery_level: self.battery_level,
            storage_quota_bytes: self.storage_quota_bytes,
            ua_architecture: self.ua_architecture.clone(),
            ua_model: self.ua_model.clone(),
            ua_platform_version: self.ua_platform_version.clone(),
            webrtc_public_ips: self.webrtc_public_ips.clone().unwrap_or_default(),
        }
    }
}

fn compute_drift_against_db(
    candidates: &[SignatureRow],
    matched_id: Uuid,
    f: &Features,
) -> Vec<&'static str> {
    let Some(row) = candidates.iter().find(|r| r.visitor_id == matched_id) else {
        return Vec::new();
    };
    let mut drift: Vec<&'static str> = Vec::new();
    if !same_str_opt(&row.webgl_render_hash, &f.webgl_render_hash) {
        drift.push("webgl_render");
    }
    if !same_str_opt(&row.canvas_hash, &f.canvas_hash) {
        drift.push("canvas");
    }
    if !same_str_opt(&row.audio_hash, &f.audio_hash) {
        drift.push("audio");
    }
    if !same_str_opt(&row.speech_voices_hash, &f.speech_voices_hash) {
        drift.push("speech_voices");
    }
    if !same_str_opt(&row.fonts_sorted_hash, &f.fonts_sorted_hash) {
        drift.push("fonts");
    }
    if !same_str_opt(&row.system_version, &f.system_version) {
        drift.push("system_version");
    }
    if !same_str_opt(&row.in_app_version_code, &f.in_app_version_code) {
        drift.push("in_app_version_code");
    }
    if row.webrtc_public_ips.as_deref().unwrap_or(&[]) != f.webrtc_public_ips.as_slice() {
        drift.push("webrtc_public_ips");
    }
    drift
}

fn same_str_opt(a: &Option<String>, b: &Option<String>) -> bool {
    match (a, b) {
        (Some(x), Some(y)) => x == y,
        (None, None) => true,
        _ => false,
    }
}

async fn create_visitor(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    f: &Features,
) -> Result<Uuid> {
    let id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO visitors (created_at, last_seen_at, observation_count)
           VALUES (now(), now(), 0)
           RETURNING visitor_id"#,
    )
    .fetch_one(&mut **tx)
    .await
    .context("inserting visitor")?;

    sqlx::query(
        r#"INSERT INTO signatures (
            visitor_id, bucket_key, canonical_ua_hash, math_fp_hash,
            webgl_params_hash, webgl_render_hash, canvas_hash, audio_hash,
            audio_stable_checksum, speech_voices_hash, fonts_sorted_hash,
            dom_rect_hash, screen_w, screen_h, device_pixel_ratio, color_depth,
            hw_concurrency, device_memory, max_touch_points, timezone, locale,
            language_tag, in_app, in_app_version, in_app_version_code,
            wechat_platform, device_vendor, system_rom, system_version,
            device_model, android_build, ua_consistent,
            client_visitor_id, battery_charging, battery_level,
            storage_quota_bytes, storage_usage_bytes,
            ua_architecture, ua_bitness, ua_model, ua_platform_version, ua_full_version,
            webrtc_public_ips, webrtc_local_ips,
            updated_at
           )
           VALUES (
            $1, $2, $3, $4,
            $5, $6, $7, $8,
            $9, $10, $11,
            $12, $13, $14, $15, $16,
            $17, $18, $19, $20, $21,
            $22, $23, $24, $25,
            $26, $27, $28, $29,
            $30, $31, $32,
            $33, $34, $35,
            $36, $37,
            $38, $39, $40, $41, $42,
            $43, $44,
            now()
           )"#,
    )
    .bind(id)
    .bind(&f.bucket_key)
    .bind(&f.canonical_ua_hash)
    .bind(&f.math_fp_hash)
    .bind(&f.webgl_params_hash)
    .bind(&f.webgl_render_hash)
    .bind(&f.canvas_hash)
    .bind(&f.audio_hash)
    .bind(f.audio_stable_checksum)
    .bind(&f.speech_voices_hash)
    .bind(&f.fonts_sorted_hash)
    .bind(&f.dom_rect_hash)
    .bind(f.screen_w)
    .bind(f.screen_h)
    .bind(f.device_pixel_ratio)
    .bind(f.color_depth)
    .bind(f.hw_concurrency)
    .bind(f.device_memory)
    .bind(f.max_touch_points)
    .bind(&f.timezone)
    .bind(&f.locale)
    .bind(&f.language_tag)
    .bind(&f.in_app)
    .bind(&f.in_app_version)
    .bind(&f.in_app_version_code)
    .bind(&f.wechat_platform)
    .bind(&f.device_vendor)
    .bind(&f.system_rom)
    .bind(&f.system_version)
    .bind(&f.device_model)
    .bind(&f.android_build)
    .bind(f.ua_consistent)
    .bind(&f.client_visitor_id)
    .bind(f.battery_charging)
    .bind(f.battery_level)
    .bind(f.storage_quota_bytes)
    .bind(f.storage_usage_bytes)
    .bind(&f.ua_architecture)
    .bind(&f.ua_bitness)
    .bind(&f.ua_model)
    .bind(&f.ua_platform_version)
    .bind(&f.ua_full_version)
    .bind(&f.webrtc_public_ips)
    .bind(&f.webrtc_local_ips)
    .execute(&mut **tx)
    .await
    .context("inserting signature")?;

    Ok(id)
}

async fn update_signature(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    visitor_id: Uuid,
    f: &Features,
) -> Result<()> {
    sqlx::query(
        r#"UPDATE signatures SET
            math_fp_hash = COALESCE($2, math_fp_hash),
            webgl_params_hash = COALESCE($3, webgl_params_hash),
            webgl_render_hash = COALESCE($4, webgl_render_hash),
            canvas_hash = COALESCE($5, canvas_hash),
            audio_hash = COALESCE($6, audio_hash),
            audio_stable_checksum = COALESCE($7, audio_stable_checksum),
            speech_voices_hash = COALESCE($8, speech_voices_hash),
            fonts_sorted_hash = COALESCE($9, fonts_sorted_hash),
            dom_rect_hash = COALESCE($10, dom_rect_hash),
            screen_w = COALESCE($11, screen_w),
            screen_h = COALESCE($12, screen_h),
            device_pixel_ratio = COALESCE($13, device_pixel_ratio),
            hw_concurrency = COALESCE($14, hw_concurrency),
            timezone = COALESCE($15, timezone),
            locale = COALESCE($16, locale),
            device_model = COALESCE($17, device_model),
            system_version = COALESCE($18, system_version),
            in_app_version_code = COALESCE($19, in_app_version_code),
            android_build = COALESCE($20, android_build),
            client_visitor_id = COALESCE($21, client_visitor_id),
            battery_charging = COALESCE($22, battery_charging),
            battery_level = COALESCE($23, battery_level),
            storage_quota_bytes = COALESCE($24, storage_quota_bytes),
            ua_architecture = COALESCE($25, ua_architecture),
            ua_model = COALESCE($26, ua_model),
            ua_platform_version = COALESCE($27, ua_platform_version),
            webrtc_public_ips = COALESCE($28, webrtc_public_ips),
            updated_at = now()
           WHERE visitor_id = $1"#,
    )
    .bind(visitor_id)
    .bind(&f.math_fp_hash)
    .bind(&f.webgl_params_hash)
    .bind(&f.webgl_render_hash)
    .bind(&f.canvas_hash)
    .bind(&f.audio_hash)
    .bind(f.audio_stable_checksum)
    .bind(&f.speech_voices_hash)
    .bind(&f.fonts_sorted_hash)
    .bind(&f.dom_rect_hash)
    .bind(f.screen_w)
    .bind(f.screen_h)
    .bind(f.device_pixel_ratio)
    .bind(f.hw_concurrency)
    .bind(&f.timezone)
    .bind(&f.locale)
    .bind(&f.device_model)
    .bind(&f.system_version)
    .bind(&f.in_app_version_code)
    .bind(&f.android_build)
    .bind(&f.client_visitor_id)
    .bind(f.battery_charging)
    .bind(f.battery_level)
    .bind(f.storage_quota_bytes)
    .bind(&f.ua_architecture)
    .bind(&f.ua_model)
    .bind(&f.ua_platform_version)
    .bind(if f.webrtc_public_ips.is_empty() {
        None
    } else {
        Some(&f.webrtc_public_ips)
    })
    .execute(&mut **tx)
    .await
    .context("updating signature")?;
    Ok(())
}

async fn bump_observation(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    visitor_id: Uuid,
) -> Result<i64> {
    let count: i64 = sqlx::query_scalar(
        r#"UPDATE visitors
           SET observation_count = observation_count + 1, last_seen_at = now()
           WHERE visitor_id = $1
           RETURNING observation_count"#,
    )
    .bind(visitor_id)
    .fetch_one(&mut **tx)
    .await?;
    Ok(count)
}

async fn insert_observation(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    visitor_id: Uuid,
    f: &Features,
    raw: &serde_json::Value,
    req: &RequestContext,
    score: f64,
    kind: MatchKind,
) -> Result<()> {
    let kind_str = match kind {
        MatchKind::Exact => "exact",
        MatchKind::Fuzzy => "fuzzy",
        MatchKind::Ambiguous => "ambiguous",
        MatchKind::New => "new",
    };
    let ip = req.ip.clone().unwrap_or_else(|| "0.0.0.0".to_string());
    sqlx::query(
        r#"INSERT INTO observations (
            visitor_id, observed_at, bucket_key, match_score, match_kind,
            ip_address, user_agent, raw_features,
            request_user_agent, request_accept_language, request_sec_ch_ua,
            request_sec_ch_ua_platform, request_referer, request_dnt
           )
           VALUES (
            $1, now(), $2, $3, $4,
            $5::inet, $6, $7,
            $8, $9, $10,
            $11, $12, $13
           )"#,
    )
    .bind(visitor_id)
    .bind(&f.bucket_key)
    .bind(score)
    .bind(kind_str)
    .bind(ip)
    .bind(&f.user_agent)
    .bind(SqlxJson(raw))
    .bind(&req.user_agent)
    .bind(&req.accept_language)
    .bind(&req.sec_ch_ua)
    .bind(&req.sec_ch_ua_platform)
    .bind(&req.referer)
    .bind(req.dnt)
    .execute(&mut **tx)
    .await
    .context("inserting observation")?;
    Ok(())
}
