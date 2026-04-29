use serde::Serialize;
use wasm_bindgen::JsValue;

#[derive(Serialize)]
pub struct QuirksFp {
    pub ios: IosQuirks,
    pub gamut: GamutSignals,
    pub features: FeatureFlags,
}

#[derive(Serialize)]
pub struct IosQuirks {
    /// `window.safari` exists only in real Safari, not WKWebView (so absent inside WeChat / Alipay etc.).
    pub has_window_safari: bool,
    /// `navigator.standalone` is iOS-only.
    pub has_standalone_prop: bool,
    pub standalone_value: Option<bool>,
    /// `window.GestureEvent` is iOS-only.
    pub has_gesture_event: bool,
    /// `webkitConvertPointFromNodeToPage` exists on real iOS WebKit only.
    pub has_webkit_convert_point: bool,
    /// `screen.availLeft` exists on Safari only.
    pub has_screen_avail_left: bool,
    /// WKWebView injects `webkit.messageHandlers` for native bridge.
    pub has_webkit_message_handlers: bool,
    /// `MouseEvent.prototype.webkitForce` indicates 3D-Touch hardware on older iPhones.
    pub has_webkit_force: bool,
}

#[derive(Serialize)]
pub struct GamutSignals {
    pub p3: Option<bool>,
    pub rec2020: Option<bool>,
    pub srgb: Option<bool>,
    pub dynamic_range_high: Option<bool>,
    pub prefers_contrast_more: Option<bool>,
    pub inverted_colors: Option<bool>,
    pub forced_colors: Option<bool>,
    pub prefers_color_scheme_dark: Option<bool>,
    pub prefers_reduced_motion: Option<bool>,
}

#[derive(Serialize)]
pub struct FeatureFlags {
    pub webgpu: bool,
    pub offscreen_canvas: bool,
    pub shared_worker: bool,
    pub service_worker: bool,
    pub clipboard: bool,
    pub storage_estimate: bool,
    pub bluetooth: bool,
    pub usb: bool,
    pub serial: bool,
    pub hid: bool,
    pub xr: bool,
    pub ndef_reader: bool,
    pub credentials: bool,
    pub locks: bool,
    pub idle_detector: bool,
    pub virtual_keyboard: bool,
    pub web_share: bool,
    pub wake_lock: bool,
    pub presentation: bool,
}

pub fn collect() -> Option<QuirksFp> {
    let window = crate::ctx::window()?;
    let win_js: &JsValue = window.as_ref();
    let navigator = window.navigator();
    let nav_js: &JsValue = navigator.as_ref();

    let ios = IosQuirks {
        has_window_safari: crate::ctx::prop_exists(win_js, "safari"),
        has_standalone_prop: crate::ctx::prop_exists(nav_js, "standalone"),
        standalone_value: crate::ctx::prop_bool(nav_js, "standalone"),
        has_gesture_event: crate::ctx::prop_exists(win_js, "GestureEvent"),
        has_webkit_convert_point: crate::ctx::prop_exists(
            win_js,
            "webkitConvertPointFromNodeToPage",
        ),
        has_screen_avail_left: window
            .screen()
            .ok()
            .map(|s| crate::ctx::prop_exists(s.as_ref(), "availLeft"))
            .unwrap_or(false),
        has_webkit_message_handlers: probe_webkit_handlers(win_js),
        has_webkit_force: probe_webkit_force(win_js),
    };

    let gamut = GamutSignals {
        p3: media_match(&window, "(color-gamut: p3)"),
        rec2020: media_match(&window, "(color-gamut: rec2020)"),
        srgb: media_match(&window, "(color-gamut: srgb)"),
        dynamic_range_high: media_match(&window, "(dynamic-range: high)"),
        prefers_contrast_more: media_match(&window, "(prefers-contrast: more)"),
        inverted_colors: media_match(&window, "(inverted-colors: inverted)"),
        forced_colors: media_match(&window, "(forced-colors: active)"),
        prefers_color_scheme_dark: media_match(&window, "(prefers-color-scheme: dark)"),
        prefers_reduced_motion: media_match(&window, "(prefers-reduced-motion: reduce)"),
    };

    let features = FeatureFlags {
        webgpu: crate::ctx::prop_exists(nav_js, "gpu"),
        offscreen_canvas: crate::ctx::prop_exists(win_js, "OffscreenCanvas"),
        shared_worker: crate::ctx::prop_exists(win_js, "SharedWorker"),
        service_worker: crate::ctx::prop_exists(nav_js, "serviceWorker"),
        clipboard: crate::ctx::prop_exists(nav_js, "clipboard"),
        storage_estimate: probe_storage_estimate(nav_js),
        bluetooth: crate::ctx::prop_exists(nav_js, "bluetooth"),
        usb: crate::ctx::prop_exists(nav_js, "usb"),
        serial: crate::ctx::prop_exists(nav_js, "serial"),
        hid: crate::ctx::prop_exists(nav_js, "hid"),
        xr: crate::ctx::prop_exists(nav_js, "xr"),
        ndef_reader: crate::ctx::prop_exists(win_js, "NDEFReader"),
        credentials: crate::ctx::prop_exists(nav_js, "credentials"),
        locks: crate::ctx::prop_exists(nav_js, "locks"),
        idle_detector: crate::ctx::prop_exists(win_js, "IdleDetector"),
        virtual_keyboard: crate::ctx::prop_exists(nav_js, "virtualKeyboard"),
        web_share: crate::ctx::prop_exists(nav_js, "share"),
        wake_lock: crate::ctx::prop_exists(nav_js, "wakeLock"),
        presentation: crate::ctx::prop_exists(nav_js, "presentation"),
    };

    Some(QuirksFp {
        ios,
        gamut,
        features,
    })
}

fn media_match(window: &web_sys::Window, query: &str) -> Option<bool> {
    let ml = window.match_media(query).ok()??;
    Some(ml.matches())
}

fn probe_webkit_handlers(win: &JsValue) -> bool {
    crate::ctx::prop_object(win, "webkit")
        .and_then(|wk| crate::ctx::prop_object(&wk, "messageHandlers"))
        .is_some()
}

fn probe_webkit_force(win: &JsValue) -> bool {
    let Some(me) = crate::ctx::prop_object(win, "MouseEvent") else {
        return false;
    };
    let Some(proto) = crate::ctx::prop_object(&me, "prototype") else {
        return false;
    };
    crate::ctx::prop_exists(&proto, "webkitForce")
}

fn probe_storage_estimate(nav_js: &JsValue) -> bool {
    let Some(storage) = crate::ctx::prop_object(nav_js, "storage") else {
        return false;
    };
    crate::ctx::prop_exists(&storage, "estimate")
}
