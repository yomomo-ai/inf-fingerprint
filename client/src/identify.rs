//! End-to-end identification flow: collect features → POST to backend →
//! return a stable identity. Cached in localStorage so repeat calls within
//! the TTL window are zero-network. Past the SWR threshold the cached value
//! is still served instantly while a fresh fetch runs in the background.
//!
//! Public API (JS):
//!
//! ```js
//! import init, { identify } from "inf-fingerprint";
//! await init();
//! const id = await identify({ endpoint: "https://fp.example.com/v1/identify" });
//! console.log(id.visitor_id, id.match_kind, id.from_server);
//! ```

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{Headers, Request, RequestInit, RequestMode, Response};

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
    /// True when the cached entry has crossed the staleSeconds threshold and
    /// a background refresh has been scheduled. The current call still gets
    /// the cached value; the refresh will populate the cache for next time.
    pub stale: bool,
    pub cached_at_ms: f64,
}

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

impl CachedIdentity {
    /// `key=value` line format. Avoids pulling in serde_json (~30 KB gz on
    /// the wasm bundle) just to (de)serialize this 8-field struct.
    /// `visitor_id` and `match_kind` come from server-controlled output that
    /// excludes `\n`, so newline is a safe row delimiter.
    fn to_storage_string(&self) -> String {
        format!(
            "v={}\nid={}\nk={}\ns={}\nss={}\nc={}\np={}\nt={}",
            self.version,
            self.visitor_id,
            self.match_kind,
            self.score,
            self.second_score,
            self.observation_count,
            if self.via_persistence { 1 } else { 0 },
            self.cached_at_ms,
        )
    }

    fn from_storage_string(raw: &str) -> Option<Self> {
        let mut version: Option<u8> = None;
        let mut visitor_id: Option<String> = None;
        let mut match_kind: Option<String> = None;
        let mut score: Option<f64> = None;
        let mut second_score: Option<f64> = None;
        let mut observation_count: Option<i64> = None;
        let mut via_persistence: Option<bool> = None;
        let mut cached_at_ms: Option<f64> = None;

        for line in raw.split('\n') {
            let (key, value) = line.split_once('=')?;
            match key {
                "v" => version = value.parse().ok(),
                "id" => visitor_id = Some(value.to_string()),
                "k" => match_kind = Some(value.to_string()),
                "s" => score = value.parse().ok(),
                "ss" => second_score = value.parse().ok(),
                "c" => observation_count = value.parse().ok(),
                "p" => via_persistence = Some(value == "1"),
                "t" => cached_at_ms = value.parse().ok(),
                _ => {}
            }
        }

        Some(Self {
            version: version?,
            visitor_id: visitor_id?,
            match_kind: match_kind?,
            score: score?,
            second_score: second_score?,
            observation_count: observation_count?,
            via_persistence: via_persistence?,
            cached_at_ms: cached_at_ms?,
        })
    }
}

/// Run the full identification pipeline.
///
/// `options`: a JS object with these fields (all optional except `endpoint`):
///   - `endpoint`: string — server URL, e.g. `"https://fp.example.com/v1/identify"`
///   - `apiKey`: string — sent as `X-API-Key` header
///   - `cacheTtlSeconds`: number — hard cache expiry (default 86400 = 24h)
///   - `staleSeconds`: number — cache age past which a background refresh
///     fires (default cacheTtlSeconds / 2). Caller still gets the cached
///     value immediately; the refresh updates cache for the next call.
///   - `forceRefresh`: bool — bypass cache (default false)
///   - `timeoutMs`: number — fetch abort timeout (default 5000)
///
/// Falls back to a locally-derived `visitor_id` if the server is unreachable
/// (sets `from_server: false`, `match_kind: "offline"`).
#[wasm_bindgen(js_name = identify)]
pub async fn identify(options: JsValue) -> Result<JsValue, JsValue> {
    let endpoint = crate::ctx::prop_string(&options, "endpoint")
        .ok_or_else(|| JsValue::from_str("identify(): `endpoint` is required"))?;
    let api_key = crate::ctx::prop_string(&options, "apiKey");
    let cache_ttl_s = crate::ctx::prop_number(&options, "cacheTtlSeconds").unwrap_or(86_400.0);
    let stale_s = crate::ctx::prop_number(&options, "staleSeconds").unwrap_or(cache_ttl_s / 2.0);
    let force_refresh = crate::ctx::prop_bool(&options, "forceRefresh").unwrap_or(false);
    let timeout_ms = crate::ctx::prop_number(&options, "timeoutMs").unwrap_or(5_000.0) as i32;

    if !force_refresh {
        if let Some(mut cached) = read_cache(cache_ttl_s) {
            let age_ms = now_ms() - cached.cached_at_ms;
            if age_ms > stale_s * 1000.0 {
                cached.stale = true;
                // Fire-and-forget background refresh. We don't await it; the
                // current caller already has a usable identity.
                let endpoint_bg = endpoint.clone();
                let api_key_bg = api_key.clone();
                spawn_local(async move {
                    let _ = collect_and_post(&endpoint_bg, api_key_bg.as_deref(), timeout_ms).await;
                });
            }
            return serde_wasm_bindgen::to_value(&cached)
                .map_err(|e| JsValue::from_str(&e.to_string()));
        }
    }

    // Cache miss / forced refresh: block on the full pipeline.
    let identity = collect_and_post(&endpoint, api_key.as_deref(), timeout_ms).await;
    serde_wasm_bindgen::to_value(&identity).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Run the full pipeline and write to cache on success. Always returns an
/// IdentityResult — server failure becomes an `offline` result without
/// cache write.
async fn collect_and_post(
    endpoint: &str,
    api_key: Option<&str>,
    timeout_ms: i32,
) -> IdentityResult {
    let fp = match crate::get_fingerprint().await {
        Ok(fp) => fp,
        Err(e) => {
            web_sys::console::warn_1(&JsValue::from_str(&format!(
                "inf-fingerprint: feature collection failed — {:?}",
                e
            )));
            return offline_result(String::new());
        }
    };

    let local_visitor_id = fp.visitor_id();
    let body_bytes = match fp.to_msgpack() {
        Ok(b) => b,
        Err(e) => {
            web_sys::console::warn_1(&JsValue::from_str(&format!(
                "inf-fingerprint: msgpack encode failed — {:?}",
                e
            )));
            return offline_result(local_visitor_id);
        }
    };

    match post_features(endpoint, api_key, &body_bytes, timeout_ms).await {
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
                stale: false,
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
            offline_result(local_visitor_id)
        }
    }
}

fn offline_result(local_visitor_id: String) -> IdentityResult {
    IdentityResult {
        visitor_id: local_visitor_id,
        match_kind: "offline".to_string(),
        score: 0.0,
        second_score: f64::NEG_INFINITY,
        observation_count: 0,
        via_persistence: false,
        from_server: false,
        cached: false,
        stale: false,
        cached_at_ms: now_ms(),
    }
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
    body: &[u8],
    timeout_ms: i32,
) -> Result<ServerResponse, JsValue> {
    let opts = RequestInit::new();
    opts.set_method("POST");
    opts.set_mode(RequestMode::Cors);
    opts.set_body(&js_sys::Uint8Array::from(body));

    let headers = Headers::new()?;
    headers.set("Content-Type", "application/msgpack")?;
    headers.set("Accept", "application/msgpack")?;
    if let Some(k) = api_key {
        headers.set("X-API-Key", k)?;
    }
    opts.set_headers(&headers);

    let abort = web_sys::AbortController::new()?;
    opts.set_signal(Some(&abort.signal()));
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let abort_clone = abort.clone();
    let cb = Closure::once_into_js(move || abort_clone.abort());
    let _ = window
        .set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), timeout_ms);

    let request = Request::new_with_str_and_init(endpoint, &opts)?;
    let response: Response = JsFuture::from(window.fetch_with_request(&request))
        .await?
        .dyn_into()?;

    if !response.ok() {
        return Err(JsValue::from_str(&format!(
            "fingerprint server returned HTTP {}",
            response.status()
        )));
    }

    let buffer = JsFuture::from(response.array_buffer()?).await?;
    let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
    rmp_serde::from_slice(&bytes).map_err(|e| JsValue::from_str(&format!("bad msgpack: {}", e)))
}

fn read_cache(ttl_seconds: f64) -> Option<IdentityResult> {
    let win = web_sys::window()?;
    let storage = win.local_storage().ok()??;
    let raw = storage.get_item(CACHE_KEY).ok()??;
    let parsed = CachedIdentity::from_storage_string(&raw)?;
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
        stale: false,
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
    let _ = storage.set_item(CACHE_KEY, &cached.to_storage_string());
}

fn now_ms() -> f64 {
    js_sys::Date::now()
}
