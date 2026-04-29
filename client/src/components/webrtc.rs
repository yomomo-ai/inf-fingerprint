use std::cell::RefCell;
use std::rc::Rc;

use serde::Serialize;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{RtcConfiguration, RtcPeerConnection, RtcSessionDescriptionInit};

#[derive(Serialize)]
pub struct WebrtcFp {
    pub local_ips: Vec<String>,
    pub local_ipv6: Vec<String>,
    /// Public IP captured via STUN srflx/prflx candidates — survives most VPNs
    /// because STUN is UDP and many split-tunnel configs leak it.
    pub public_ips: Vec<String>,
    pub mdns_hosts: Vec<String>,
    pub candidate_count: u32,
    pub completed: bool,
}

pub async fn collect() -> Option<WebrtcFp> {
    try_collect().await.ok()
}

async fn try_collect() -> Result<WebrtcFp, JsValue> {
    let config = RtcConfiguration::new();
    let ice_servers = js_sys::Array::new();
    for url in [
        "stun:stun.l.google.com:19302",
        "stun:stun.miwifi.com:3478",
        "stun:stun.qq.com:3478",
    ] {
        let server = js_sys::Object::new();
        let urls = js_sys::Array::new();
        urls.push(&JsValue::from_str(url));
        let _ = js_sys::Reflect::set(&server, &"urls".into(), &urls);
        ice_servers.push(&server);
    }
    let _ = js_sys::Reflect::set(config.as_ref(), &"iceServers".into(), &ice_servers);

    let pc = RtcPeerConnection::new_with_configuration(&config)?;
    let _ = pc.create_data_channel("inf-fp");

    let raw: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let completed_cb = completed.clone();

    let raw_cb = raw.clone();
    let cb = Closure::<dyn FnMut(JsValue)>::new(move |evt: JsValue| {
        let cand = match js_sys::Reflect::get(&evt, &JsValue::from_str("candidate")) {
            Ok(c) => c,
            Err(_) => return,
        };
        if cand.is_null() || cand.is_undefined() {
            *completed_cb.borrow_mut() = true;
            return;
        }
        if let Ok(s_val) = js_sys::Reflect::get(&cand, &JsValue::from_str("candidate")) {
            if let Some(s) = s_val.as_string() {
                if !s.is_empty() {
                    raw_cb.borrow_mut().push(s);
                }
            }
        }
    });
    pc.set_onicecandidate(Some(cb.as_ref().unchecked_ref()));

    let offer_promise = pc.create_offer();
    let offer = JsFuture::from(offer_promise).await?;
    let offer_init: RtcSessionDescriptionInit = offer.unchecked_into();
    JsFuture::from(pc.set_local_description(&offer_init)).await?;

    // Adaptive wait: poll completion every 100ms, exit on null-candidate event.
    // Hard cap at 1500ms for networks where STUN is silently dropped.
    for _ in 0..15 {
        sleep_ms(100).await?;
        if *completed.borrow() {
            break;
        }
    }
    pc.close();

    let raw_candidates: Vec<String> = raw.borrow().clone();
    let done = *completed.borrow();
    drop(cb);

    let mut local_ipv4: Vec<String> = Vec::new();
    let mut local_ipv6: Vec<String> = Vec::new();
    let mut public_ips: Vec<String> = Vec::new();
    let mut mdns_hosts: Vec<String> = Vec::new();

    for c in &raw_candidates {
        let kind = candidate_type(c);
        let Some(addr) = parse_address(c) else {
            continue;
        };
        if addr.ends_with(".local") {
            if !mdns_hosts.contains(&addr) {
                mdns_hosts.push(addr);
            }
            continue;
        }
        if matches!(kind.as_deref(), Some("srflx") | Some("prflx")) {
            if !public_ips.contains(&addr) {
                public_ips.push(addr);
            }
            continue;
        }
        if addr.contains(':') {
            if !local_ipv6.contains(&addr) {
                local_ipv6.push(addr);
            }
        } else if !local_ipv4.contains(&addr) {
            local_ipv4.push(addr);
        }
    }

    Ok(WebrtcFp {
        local_ips: local_ipv4,
        local_ipv6,
        public_ips,
        mdns_hosts,
        candidate_count: raw_candidates.len() as u32,
        completed: done,
    })
}

/// Candidate format: `candidate:0 1 UDP 2122252543 192.168.1.5 53345 typ host`
/// Address is field index 4; type follows the `typ` keyword.
fn parse_address(candidate: &str) -> Option<String> {
    candidate.split_whitespace().nth(4).map(|s| s.to_string())
}

fn candidate_type(candidate: &str) -> Option<String> {
    let mut parts = candidate.split_whitespace();
    while let Some(p) = parts.next() {
        if p == "typ" {
            return parts.next().map(|s| s.to_string());
        }
    }
    None
}

async fn sleep_ms(ms: i32) -> Result<(), JsValue> {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        if let Some(w) = web_sys::window() {
            let _ = w.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
        }
    });
    JsFuture::from(promise).await.map(|_| ())
}
