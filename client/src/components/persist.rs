use serde::Serialize;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;

#[derive(Serialize)]
pub struct PersistFp {
    /// Locally-generated UUID, written to localStorage + cookie. Survives
    /// cookie-only-clear and localStorage-only-clear; only fully resets when
    /// both are wiped.
    pub client_visitor_id: String,
    pub from_local_storage: bool,
    pub from_session_storage: bool,
    pub from_cookie: bool,
    pub created_now: bool,
}

const KEY: &str = "__inf_fp_id";
const COOKIE_NAME: &str = "_inf_fp_id";

pub fn collect() -> Option<PersistFp> {
    let window = crate::ctx::window()?;
    let win_js: &JsValue = window.as_ref();
    let document = window.document()?;
    let doc_js: &JsValue = document.as_ref();

    let ls_id = read_storage(win_js, "localStorage", KEY);
    let ss_id = read_storage(win_js, "sessionStorage", KEY);
    let cookie_id = read_cookie(doc_js, COOKIE_NAME);

    let from_local_storage = ls_id.is_some();
    let from_session_storage = ss_id.is_some();
    let from_cookie = cookie_id.is_some();

    let id = ls_id.clone().or(cookie_id.clone()).or(ss_id.clone());
    let created_now = id.is_none();
    let final_id = id.unwrap_or_else(generate_uuid);

    write_storage(win_js, "localStorage", KEY, &final_id);
    write_storage(win_js, "sessionStorage", KEY, &final_id);
    write_cookie(doc_js, COOKIE_NAME, &final_id);

    Some(PersistFp {
        client_visitor_id: final_id,
        from_local_storage,
        from_session_storage,
        from_cookie,
        created_now,
    })
}

fn read_storage(win: &JsValue, prop: &str, key: &str) -> Option<String> {
    let storage = crate::ctx::prop_object(win, prop)?;
    let func: js_sys::Function = crate::ctx::prop_object(&storage, "getItem")?
        .dyn_into()
        .ok()?;
    let v = func.call1(&storage, &JsValue::from_str(key)).ok()?;
    if v.is_null() || v.is_undefined() {
        None
    } else {
        v.as_string()
    }
}

fn write_storage(win: &JsValue, prop: &str, key: &str, value: &str) {
    let Some(storage) = crate::ctx::prop_object(win, prop) else {
        return;
    };
    let Some(set_item) = crate::ctx::prop_object(&storage, "setItem") else {
        return;
    };
    let Ok(func) = set_item.dyn_into::<js_sys::Function>() else {
        return;
    };
    let _ = func.call2(&storage, &JsValue::from_str(key), &JsValue::from_str(value));
}

fn read_cookie(doc: &JsValue, name: &str) -> Option<String> {
    let cookie = crate::ctx::prop_string(doc, "cookie")?;
    let prefix = format!("{}=", name);
    cookie
        .split("; ")
        .find_map(|p| p.strip_prefix(&prefix))
        .map(|s| s.to_string())
}

fn write_cookie(doc: &JsValue, name: &str, value: &str) {
    // 10-year max-age (Chrome caps at 400 days; we ask for the moon anyway).
    // SameSite=Lax preserves same-origin POSTs but denies cross-site reads.
    // Secure unset → works on http://localhost during dev; production behind
    // https will get Secure-by-default in modern browsers.
    let cookie = format!(
        "{}={}; Max-Age=315360000; Path=/; SameSite=Lax",
        name, value
    );
    let _ = js_sys::Reflect::set(
        doc,
        &JsValue::from_str("cookie"),
        &JsValue::from_str(&cookie),
    );
}

/// crypto.randomUUID() if available, else a hex string from Math.random().
fn generate_uuid() -> String {
    if let Some(window) = crate::ctx::window() {
        let win_js: &JsValue = window.as_ref();
        if let Some(crypto) = crate::ctx::prop_object(win_js, "crypto") {
            if let Some(uuid_fn) = crate::ctx::prop_object(&crypto, "randomUUID") {
                if let Ok(func) = uuid_fn.dyn_into::<js_sys::Function>() {
                    if let Ok(v) = func.call0(&crypto) {
                        if let Some(s) = v.as_string() {
                            return s;
                        }
                    }
                }
            }
        }
    }
    fallback_uuid()
}

fn fallback_uuid() -> String {
    let mut s = String::with_capacity(36);
    let positions = [(0, 8), (9, 4), (14, 4), (19, 4), (24, 12)];
    for (i, (offset, len)) in positions.iter().enumerate() {
        if i > 0 {
            s.push('-');
        }
        let _ = offset;
        for j in 0..*len {
            let r = js_sys::Math::random();
            let mut nibble = (r * 16.0) as u32;
            if i == 2 && j == 0 {
                // version 4
                nibble = 4;
            }
            if i == 3 && j == 0 {
                // variant 1 (10xx)
                nibble = (nibble & 0x3) | 0x8;
            }
            s.push(std::char::from_digit(nibble, 16).unwrap_or('0'));
        }
    }
    s
}
