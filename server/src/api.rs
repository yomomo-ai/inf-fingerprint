use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
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
use crate::matcher;

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
}

async fn identify(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<Json<IdentifyResponse>> {
    check_api_key(&state, &headers)?;

    let features = Features::from_json(&body)
        .ok_or_else(|| ApiError::BadRequest("malformed feature payload".to_string()))?;

    let outcome = matcher::identify(
        &state.pool,
        &features,
        &body,
        addr.ip(),
        state.match_threshold,
        state.ambiguous_threshold,
        state.max_candidates,
    )
    .await?;

    Ok(Json(IdentifyResponse {
        visitor_id: outcome.visitor_id,
        match_kind: outcome.match_kind,
        score: outcome.score,
        second_score: outcome.second_score,
        candidates: outcome.candidates,
        drift: outcome.drift,
        observation_count: outcome.observation_count,
    }))
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
