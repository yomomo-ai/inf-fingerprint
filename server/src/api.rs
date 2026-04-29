use axum::body::Bytes;
use axum::extract::{ConnectInfo, State};
use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use sqlx::postgres::PgPool;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::error::{ApiError, ApiResult};
use crate::features::Features;
use crate::matcher::{self, RequestContext};

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub api_key: Option<String>,
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
    pub visitor_id: uuid::Uuid,
    pub match_kind: matcher::MatchKind,
    pub score: f64,
    pub second_score: f64,
    pub candidates: Vec<matcher::CandidateScore>,
    pub drift: Vec<&'static str>,
    pub observation_count: i64,
    pub via_persistence: bool,
}

async fn identify(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    body: Bytes,
) -> ApiResult<Response> {
    check_api_key(&state, &headers)?;

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
        &features,
        &raw,
        &req,
        state.match_threshold,
        state.ambiguous_threshold,
        state.max_candidates,
    )
    .await?;

    let response_body = IdentifyResponse {
        visitor_id: outcome.visitor_id,
        match_kind: outcome.match_kind,
        score: outcome.score,
        second_score: outcome.second_score,
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

fn check_api_key(state: &AppState, headers: &HeaderMap) -> ApiResult<()> {
    let Some(expected) = state.api_key.as_deref() else {
        return Ok(());
    };
    let provided = headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if constant_time_eq(provided.as_bytes(), expected.as_bytes()) {
        Ok(())
    } else {
        Err(ApiError::Unauthorized)
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}
