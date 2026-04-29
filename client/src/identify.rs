//! End-to-end identification flow: collect features → POST to backend →
//! return a stable identity. Cached in localStorage so repeat calls within
//! the TTL window are zero-network.
//!
//! Public API (JS):
//!
//! ```js
//! import init, { identify } from "inf-fingerprint";
//! await init();
//! const id = await identify({ endpoint: "https://fp.example.com/v1/identify" });
//! console.log(id.visitorId, id.matchKind, id.fromServer);
//! ```

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Headers, Request, RequestCredentials, RequestInit, RequestMode, Response};

const CACHE_KEY: &str = "__inf_fp_identity_cache";
const CACHE_VERSION: u8 = 1;

#[derive(Serialize, Deserialize, Clone)]
pub struct IdentityResult {
    pub visitor_id: String,
    pub match_kind: String,
    pub score: f64,
    pub second_score: f64,
    pub observation_count: i64,
    pub via_persistence: bool,
    pub from_server: bool,
    pub cached: bool,
    pub cached_at_ms: f64,
}

#[derive(Serialize)]
struct CachedIdentity {
    version: u8,
    visitor_id: String,
    match_kind: String,
    score: f64,
    second_score: f64,
    observation_count: i64,
    via_persistence: bool,
    cached_at_ms: f64,
}

#[derive(Deserialize)]
struct CachedIdentityRead {
    version: u8,
    visitor_id: String,
    match_kind: String,
    score: f64,
    second_score: f64,
    observation_count: i64,
    via_persistence: bool,
    cached_at_ms: f64,
}

/// Run the full identification pipeline.
///
/// `options`: a JS object with these fields (all optional except `endpoint`):
///   - `endpoint`: string — server URL, e.g. `"https://fp.example.com/v1/identify"`
///   - `apiKey`: string — sent as `X-API-Key` header
///   - `cacheTtlSeconds`: number — how long to reuse a cached identity (default 86400)
///   - `forceRefresh`: bool — bypass cache (default false)
///   - `timeoutMs`: number — fetch abort timeout (default 5000)
///
/// Returns a JS object `{ visitorId, matchKind, score, secondScore,
/// observationCount, viaPersistence, fromServer, cached, cachedAtMs }`.
///
/// Falls back to a locally-derived `visitorId` if the server is unreachable
/// (sets `fromServer: false`, `matchKind: "offline"`).
#[wasm_bindgen(js_name = identify)]
pub async fn identify(options: JsValue) -> Result<JsValue, JsValue> {
    let endpoint = crate::ctx::prop_string(&options, "endpoint")
        .ok_or_else(|| JsValue::from_str("identify(): `endpoint` is required"))?;
    let api_key = crate::ctx::prop_string(&options, "apiKey");
    let cache_ttl_s = crate::ctx::prop_number(&options, "cacheTtlSeconds").unwrap_or(86_400.0);
    let force_refresh = crate::ctx::prop_bool(&options, "forceRefresh").unwrap_or(false);
    let timeout_ms = crate::ctx::prop_number(&options, "timeoutMs").unwrap_or(5_000.0) as i32;

    // Fast path: serve from cache if fresh.
    if !force_refresh {
        if let Some(cached) = read_cache(cache_ttl_s) {
            return serde_wasm_bindgen::to_value(&cached)
                .map_err(|e| JsValue::from_str(&e.to_string()));
        }
    }

    // Collect features and the local fallback identity in one pass.
    let fp = crate::get_fingerprint().await?;
    let payload = fp.to_json()?;
    let payload_str = js_sys::JSON::stringify(&payload)?
        .as_string()
        .unwrap_or_else(|| "{}".to_string());
    let local_visitor_id = fp.visitor_id();

    // Server roundtrip. On any failure, fall back to local visitor_id.
    let server_result =
        post_features(&endpoint, api_key.as_deref(), &payload_str, timeout_ms).await;

    let identity = match server_result {
        Ok(server) => {
            let result = IdentityResult {
                visitor_id: server.visitor_id,
                match_kind: server.match_kind,
                score: server.score,
                second_score: server.second_score.unwrap_or(f64::NEG_INFINITY),
                observation_count: server.observation_count,
                via_persistence: server.via_persistence,
                from_server: true,
                cached: false,
                cached_at_ms: now_ms(),
            };
            write_cache(&result);
            result
        }
        Err(e) => {
            web_sys::console::warn_1(&JsValue::from_str(&format!(
                "inf-fingerprint: server unreachable, falling back to local — {:?}",
                e
            )));
            IdentityResult {
                visitor_id: local_visitor_id,
                match_kind: "offline".to_string(),
                score: 0.0,
                second_score: f64::NEG_INFINITY,
                observation_count: 0,
                via_persistence: false,
                from_server: false,
                cached: false,
                cached_at_ms: now_ms(),
            }
        }
    };

    serde_wasm_bindgen::to_value(&identity).map_err(|e| JsValue::from_str(&e.to_string()))
}

#[derive(Deserialize)]
struct ServerResponse {
    visitor_id: String,
    match_kind: String,
    score: f64,
    #[serde(default)]
    second_score: Option<f64>,
    observation_count: i64,
    #[serde(default)]
    via_persistence: bool,
}

async fn post_features(
    endpoint: &str,
    api_key: Option<&str>,
    body: &str,
    timeout_ms: i32,
) -> Result<ServerResponse, JsValue> {
    let opts = RequestInit::new();
    opts.set_method("POST");
    opts.set_mode(RequestMode::Cors);
    opts.set_credentials(RequestCredentials::Include);
    opts.set_body(&JsValue::from_str(body));

    let headers = Headers::new()?;
    headers.set("Content-Type", "application/json")?;
    if let Some(k) = api_key {
        headers.set("X-API-Key", k)?;
    }
    opts.set_headers(&headers);

    // AbortController-based timeout.
    let abort = web_sys::AbortController::new()?;
    opts.set_signal(Some(&abort.signal()));
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let abort_clone = abort.clone();
    let cb = Closure::once_into_js(move || abort_clone.abort());
    let _ = window
        .set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), timeout_ms);

    let request = Request::new_with_str_and_init(endpoint, &opts)?;
    let response_value = JsFuture::from(window.fetch_with_request(&request)).await?;
    let response: Response = response_value.dyn_into()?;

    if !response.ok() {
        return Err(JsValue::from_str(&format!(
            "fingerprint server returned HTTP {}",
            response.status()
        )));
    }

    let text_value = JsFuture::from(response.text()?).await?;
    let text = text_value
        .as_string()
        .ok_or_else(|| JsValue::from_str("response body not a string"))?;
    let parsed: ServerResponse =
        serde_json::from_str(&text).map_err(|e| JsValue::from_str(&format!("bad json: {}", e)))?;
    Ok(parsed)
}

fn read_cache(ttl_seconds: f64) -> Option<IdentityResult> {
    let win = web_sys::window()?;
    let storage = win.local_storage().ok()??;
    let raw = storage.get_item(CACHE_KEY).ok()??;
    let parsed: CachedIdentityRead = serde_json::from_str(&raw).ok()?;
    if parsed.version != CACHE_VERSION {
        return None;
    }
    let age_ms = now_ms() - parsed.cached_at_ms;
    if age_ms < 0.0 || age_ms > ttl_seconds * 1000.0 {
        return None;
    }
    Some(IdentityResult {
        visitor_id: parsed.visitor_id,
        match_kind: parsed.match_kind,
        score: parsed.score,
        second_score: parsed.second_score,
        observation_count: parsed.observation_count,
        via_persistence: parsed.via_persistence,
        from_server: true,
        cached: true,
        cached_at_ms: parsed.cached_at_ms,
    })
}

fn write_cache(identity: &IdentityResult) {
    let Some(win) = web_sys::window() else {
        return;
    };
    let Ok(Some(storage)) = win.local_storage() else {
        return;
    };
    let cached = CachedIdentity {
        version: CACHE_VERSION,
        visitor_id: identity.visitor_id.clone(),
        match_kind: identity.match_kind.clone(),
        score: identity.score,
        second_score: identity.second_score,
        observation_count: identity.observation_count,
        via_persistence: identity.via_persistence,
        cached_at_ms: identity.cached_at_ms,
    };
    if let Ok(s) = serde_json::to_string(&cached) {
        let _ = storage.set_item(CACHE_KEY, &s);
    }
}

fn now_ms() -> f64 {
    js_sys::Date::now()
}
