use serde::Serialize;
use wasm_bindgen::JsValue;

#[derive(Serialize)]
pub struct ScreenFp {
    pub width: u32,
    pub height: u32,
    pub avail_width: u32,
    pub avail_height: u32,
    pub color_depth: u16,
    pub pixel_depth: u16,
    pub device_pixel_ratio: f64,
    pub orientation_type: Option<String>,
    pub orientation_angle: Option<i32>,
    pub inner_width: Option<u32>,
    pub inner_height: Option<u32>,
    pub outer_width: Option<u32>,
    pub outer_height: Option<u32>,
}

pub fn collect() -> Option<ScreenFp> {
    let window = crate::ctx::window()?;
    let screen = window.screen().ok()?;
    let screen_js: &JsValue = screen.as_ref();

    let orientation = crate::ctx::prop_object(screen_js, "orientation");
    let orientation_type = orientation
        .as_ref()
        .and_then(|o| crate::ctx::prop_string(o, "type"));
    let orientation_angle = orientation
        .as_ref()
        .and_then(|o| crate::ctx::prop_number(o, "angle"))
        .map(|v| v as i32);

    Some(ScreenFp {
        width: screen.width().unwrap_or(0).max(0) as u32,
        height: screen.height().unwrap_or(0).max(0) as u32,
        avail_width: screen.avail_width().unwrap_or(0).max(0) as u32,
        avail_height: screen.avail_height().unwrap_or(0).max(0) as u32,
        color_depth: screen.color_depth().unwrap_or(0).max(0) as u16,
        pixel_depth: screen.pixel_depth().unwrap_or(0).max(0) as u16,
        device_pixel_ratio: window.device_pixel_ratio(),
        orientation_type,
        orientation_angle,
        inner_width: window
            .inner_width()
            .ok()
            .and_then(|v| v.as_f64())
            .map(|v| v as u32),
        inner_height: window
            .inner_height()
            .ok()
            .and_then(|v| v.as_f64())
            .map(|v| v as u32),
        outer_width: window
            .outer_width()
            .ok()
            .and_then(|v| v.as_f64())
            .map(|v| v as u32),
        outer_height: window
            .outer_height()
            .ok()
            .and_then(|v| v.as_f64())
            .map(|v| v as u32),
    })
}
