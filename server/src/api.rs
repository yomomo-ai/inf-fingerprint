use axum::body::Bytes;
use axum::extract::{ConnectInfo, Path, State};
use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPool;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::features::Features;
use crate::matcher::{self, BucketCache, RequestContext};
use crate::merger::{self, MergeSource};

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub bucket_cache: BucketCache,
    pub match_threshold: f64,
    pub ambiguous_threshold: f64,
    pub max_candidates: usize,
}

pub fn router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/identify", post(identify))
        .route("/v1/feedback", post(feedback))
        .route(
            "/v1/visitors/{visitor_id}/canonical",
            get(resolve_canonical),
        )
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(Arc::new(state))
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
}

async fn health(State(state): State<Arc<AppState>>) -> ApiResult<impl IntoResponse> {
    sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await?;
    Ok((
        StatusCode::OK,
        Json(HealthResponse {
            status: "ok",
            version: env!("CARGO_PKG_VERSION"),
        }),
    ))
}

#[derive(Serialize)]
pub struct IdentifyResponse {
    // Force hyphenated-string serialization. uuid's default serde adapts
    // to the wire format (string for JSON, raw 16 bytes for msgpack); the
    // SDK's response struct types `visitor_id: String`, so the bytes-path
    // breaks decoding. Pin to hyphenated regardless of format.
    #[serde(with = "uuid::serde::hyphenated")]
    pub visitor_id: uuid::Uuid,
    pub match_kind: matcher::MatchKind,
    pub score: f64,
    pub second_score: f64,
    /// Margin-based confidence in `[0, 1]`. `0.99+` means the top
    /// candidate dominates the second; `0.5` means the matcher couldn't
    /// distinguish two candidates. Caller-side risk logic should compare
    /// this rather than raw `score`, since it normalizes across
    /// population and threshold drift.
    pub confidence: f64,
    pub candidates: Vec<matcher::CandidateScore>,
    pub drift: Vec<&'static str>,
    pub observation_count: i64,
    pub via_persistence: bool,
}

fn margin_confidence(score: f64, second_score: f64) -> f64 {
    // No candidates at all → no confidence in any specific identity.
    if !score.is_finite() || score <= 0.0 {
        return 0.0;
    }
    // Single candidate with positive score → fully confident in it.
    if !second_score.is_finite() {
        return 1.0;
    }
    // Sigmoid of the score gap. The choice of scale (`/ 5.0`) means a gap
    // of 5 nats already produces ~0.73 confidence; a gap of 15 saturates
    // near 1.0. Tuned to match the observed score distribution where
    // genuine matches typically score 25-50 above NEG_INFINITY.
    let z = (score - second_score) / 5.0;
    1.0 / (1.0 + (-z).exp())
}

async fn identify(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    body: Bytes,
) -> ApiResult<Response> {
    // Wire format is msgpack on both directions. We deserialize into
    // serde_json::Value (rmp-serde supports this transparently) so the
    // existing untyped-walking Features extractor works unchanged.
    let raw: serde_json::Value = rmp_serde::from_slice(&body)
        .map_err(|e| ApiError::BadRequest(format!("bad msgpack: {}", e)))?;

    let features = Features::from_json(&raw)
        .ok_or_else(|| ApiError::BadRequest("malformed feature payload".to_string()))?;

    let req = build_request_context(&headers, &addr);

    let outcome = matcher::identify(
        &state.pool,
        &state.bucket_cache,
        &features,
        &raw,
        &req,
        state.match_threshold,
        state.ambiguous_threshold,
        state.max_candidates,
    )
    .await?;

    let confidence = margin_confidence(outcome.score, outcome.second_score);
    let response_body = IdentifyResponse {
        visitor_id: outcome.visitor_id,
        match_kind: outcome.match_kind,
        score: outcome.score,
        second_score: outcome.second_score,
        confidence,
        candidates: outcome.candidates,
        drift: outcome.drift,
        observation_count: outcome.observation_count,
        via_persistence: outcome.via_persistence,
    };

    let bytes = rmp_serde::to_vec_named(&response_body)
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("encode response: {}", e)))?;
    let mut resp = (StatusCode::OK, bytes).into_response();
    resp.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/msgpack"),
    );
    Ok(resp)
}

fn build_request_context(headers: &HeaderMap, addr: &SocketAddr) -> RequestContext {
    // X-Forwarded-For is set by edges/CDNs. Take the leftmost (originating
    // client). Fall back to socket peer when behind no proxy.
    let xff_ip = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let real_ip = headers
        .get("x-real-ip")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty());
    let ip = xff_ip.or(real_ip).or_else(|| Some(addr.ip().to_string()));

    let dnt = headers
        .get("dnt")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim() == "1");

    RequestContext {
        ip,
        user_agent: header_str(headers, "user-agent"),
        accept_language: header_str(headers, "accept-language"),
        sec_ch_ua: header_str(headers, "sec-ch-ua"),
        sec_ch_ua_platform: header_str(headers, "sec-ch-ua-platform"),
        referer: header_str(headers, "referer"),
        dnt,
    }
}

fn header_str(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

// ====== /v1/feedback ======
//
// Closed-loop identity correction. Callers that learn ground-truth
// identity (user logs in, binds a phone number, completes KYC) can tell
// us "these N visitor_ids are actually one person" and the matcher
// reassigns observations + collapses signatures + audits the merge.
//
// Authn for this endpoint is enforced at the edge (nginx requires
// X-API-Key for /v1/feedback per fp.conf); the server trusts whatever
// nginx passes through. Adding a second key check here would create a
// second source of truth for the bypass key, which we explicitly chose
// not to do.

#[derive(Deserialize)]
struct FeedbackRequest {
    operation: String,
    visitor_ids: Vec<Uuid>,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Serialize)]
struct FeedbackResponse {
    #[serde(with = "uuid::serde::hyphenated")]
    canonical_visitor_id: Uuid,
    /// All visitor_ids that were folded into the canonical (excludes the
    /// canonical itself). Caller-side: any reference to one of these
    /// should be migrated to canonical_visitor_id.
    merged_visitor_ids: Vec<String>,
    /// Total observation count after merge (canonical's prior count +
    /// every merged visitor's count).
    observation_count: i64,
}

async fn feedback(State(state): State<Arc<AppState>>, body: Bytes) -> ApiResult<Response> {
    let req: FeedbackRequest = rmp_serde::from_slice(&body)
        .map_err(|e| ApiError::BadRequest(format!("bad msgpack: {}", e)))?;

    if req.operation != "merge" {
        return Err(ApiError::BadRequest(format!(
            "unsupported operation: {} (expected 'merge')",
            req.operation
        )));
    }
    if req.visitor_ids.len() < 2 {
        return Err(ApiError::BadRequest(
            "merge requires at least 2 visitor_ids; first is canonical, rest fold into it"
                .to_string(),
        ));
    }
    let reason = req.reason.as_deref().unwrap_or("feedback_api");

    let outcome = merger::merge_visitors(
        &state.pool,
        &state.bucket_cache,
        &req.visitor_ids,
        reason,
        MergeSource::FeedbackApi,
    )
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("merge: {}", e)))?;

    let response_body = FeedbackResponse {
        canonical_visitor_id: outcome.canonical_visitor_id,
        merged_visitor_ids: outcome
            .merged_visitor_ids
            .iter()
            .map(|u| u.hyphenated().to_string())
            .collect(),
        observation_count: outcome.total_observation_count,
    };

    let bytes = rmp_serde::to_vec_named(&response_body)
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("encode response: {}", e)))?;
    let mut resp = (StatusCode::OK, bytes).into_response();
    resp.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/msgpack"),
    );
    Ok(resp)
}

// ====== /v1/visitors/{visitor_id}/canonical ======
//
// Caller stored visitor_id "X" days/weeks ago; X may since have been
// merged-away by feedback or auto-merge. This endpoint resolves any
// stored id to the current canonical id by following the merge chain.

#[derive(Serialize)]
struct CanonicalResponse {
    #[serde(with = "uuid::serde::hyphenated")]
    canonical_visitor_id: Uuid,
    /// True when the input visitor_id has itself been merged away. False
    /// when the input is already canonical. Useful for callers to detect
    /// "you should update your stored id" without diffing strings.
    redirected: bool,
}

async fn resolve_canonical(
    State(state): State<Arc<AppState>>,
    Path(visitor_id): Path<Uuid>,
) -> ApiResult<Json<CanonicalResponse>> {
    let canonical = merger::resolve_canonical(&state.pool, visitor_id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("resolve_canonical: {}", e)))?;
    Ok(Json(CanonicalResponse {
        canonical_visitor_id: canonical,
        redirected: canonical != visitor_id,
    }))
}
