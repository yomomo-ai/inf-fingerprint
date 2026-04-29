use serde::Serialize;
use wasm_bindgen::JsValue;

#[derive(Serialize)]
pub struct TouchFp {
    pub max_touch_points: i32,
    pub touch_event: bool,
    pub touch_start: bool,
    pub pointer_event: bool,
    pub coarse_pointer: Option<bool>,
    pub hover_capable: Option<bool>,
}

pub fn collect() -> Option<TouchFp> {
    let nav = crate::ctx::navigator()?;
    let window = crate::ctx::window()?;
    let win_js: &JsValue = window.as_ref();

    Some(TouchFp {
        max_touch_points: nav.max_touch_points(),
        touch_event: crate::ctx::prop_exists(win_js, "TouchEvent"),
        touch_start: crate::ctx::prop_exists(win_js, "ontouchstart"),
        pointer_event: crate::ctx::prop_exists(win_js, "PointerEvent"),
        coarse_pointer: media_query(&window, "(pointer: coarse)"),
        hover_capable: media_query(&window, "(hover: hover)"),
    })
}

fn media_query(window: &web_sys::Window, query: &str) -> Option<bool> {
    let ml = window.match_media(query).ok()??;
    Some(ml.matches())
}
