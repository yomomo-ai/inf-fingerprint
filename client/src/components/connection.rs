use serde::Serialize;
use wasm_bindgen::JsValue;

#[derive(Serialize)]
pub struct ConnectionFp {
    pub effective_type: Option<String>,
    pub downlink: Option<f64>,
    pub downlink_max: Option<f64>,
    pub rtt: Option<f64>,
    pub save_data: Option<bool>,
    pub connection_type: Option<String>,
}

pub fn collect() -> Option<ConnectionFp> {
    let nav = crate::ctx::navigator()?;
    let nav_js: &JsValue = nav.as_ref();
    let conn = crate::ctx::prop_object(nav_js, "connection")
        .or_else(|| crate::ctx::prop_object(nav_js, "mozConnection"))
        .or_else(|| crate::ctx::prop_object(nav_js, "webkitConnection"))?;

    Some(ConnectionFp {
        effective_type: crate::ctx::prop_string(&conn, "effectiveType"),
        downlink: crate::ctx::prop_number(&conn, "downlink"),
        downlink_max: crate::ctx::prop_number(&conn, "downlinkMax"),
        rtt: crate::ctx::prop_number(&conn, "rtt"),
        save_data: crate::ctx::prop_bool(&conn, "saveData"),
        connection_type: crate::ctx::prop_string(&conn, "type"),
    })
}
