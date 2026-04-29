use serde::Serialize;
use wasm_bindgen::{JsCast, JsValue};

#[derive(Serialize)]
pub struct NavigatorFp {
    pub user_agent: String,
    pub language: Option<String>,
    pub languages: Vec<String>,
    pub platform: Option<String>,
    pub vendor: Option<String>,
    pub vendor_sub: Option<String>,
    pub product: Option<String>,
    pub product_sub: Option<String>,
    pub hardware_concurrency: f64,
    pub device_memory: Option<f64>,
    pub max_touch_points: i32,
    pub do_not_track: Option<String>,
    pub cookie_enabled: Option<bool>,
    pub online: bool,
    pub pdf_viewer_enabled: Option<bool>,
    pub webdriver: Option<bool>,
    pub ua_client_hints: Option<UaHints>,
    pub cn_navigator_keys: Vec<String>,
}

#[derive(Serialize)]
pub struct UaHints {
    pub mobile: Option<bool>,
    pub platform: Option<String>,
    pub brands: Vec<String>,
}

pub fn collect() -> Option<NavigatorFp> {
    let nav = crate::ctx::navigator()?;
    let nav_js: &JsValue = nav.as_ref();

    let user_agent = nav.user_agent().unwrap_or_default();
    let language = nav.language();
    let languages: Vec<String> = nav
        .languages()
        .iter()
        .filter_map(|v| v.as_string())
        .collect();
    let platform = nav.platform().ok();

    Some(NavigatorFp {
        user_agent,
        language,
        languages,
        platform,
        vendor: crate::ctx::prop_string(nav_js, "vendor"),
        vendor_sub: crate::ctx::prop_string(nav_js, "vendorSub"),
        product: crate::ctx::prop_string(nav_js, "product"),
        product_sub: crate::ctx::prop_string(nav_js, "productSub"),
        hardware_concurrency: nav.hardware_concurrency(),
        device_memory: crate::ctx::prop_number(nav_js, "deviceMemory"),
        max_touch_points: nav.max_touch_points(),
        do_not_track: crate::ctx::prop_string(nav_js, "doNotTrack"),
        cookie_enabled: crate::ctx::prop_bool(nav_js, "cookieEnabled"),
        online: nav.on_line(),
        pdf_viewer_enabled: crate::ctx::prop_bool(nav_js, "pdfViewerEnabled"),
        webdriver: crate::ctx::prop_bool(nav_js, "webdriver"),
        ua_client_hints: read_ua_hints(nav_js),
        cn_navigator_keys: probe_cn_navigator_keys(nav_js),
    })
}

fn read_ua_hints(nav_js: &JsValue) -> Option<UaHints> {
    let ua_data = crate::ctx::prop_object(nav_js, "userAgentData")?;
    let mobile = crate::ctx::prop_bool(&ua_data, "mobile");
    let platform = crate::ctx::prop_string(&ua_data, "platform");

    let brands_val = crate::ctx::prop_object(&ua_data, "brands");
    let brands: Vec<String> = brands_val
        .and_then(|v| v.dyn_into::<js_sys::Array>().ok())
        .map(|arr| {
            (0..arr.length())
                .filter_map(|i| {
                    let entry = arr.get(i);
                    let brand = crate::ctx::prop_string(&entry, "brand")?;
                    let version = crate::ctx::prop_string(&entry, "version").unwrap_or_default();
                    Some(format!("{} {}", brand, version))
                })
                .collect()
        })
        .unwrap_or_default();

    Some(UaHints {
        mobile,
        platform,
        brands,
    })
}

fn probe_cn_navigator_keys(nav_js: &JsValue) -> Vec<String> {
    const PROBES: &[&str] = &[
        "ucapi",
        "miuiBrowser",
        "alipayQuickPay",
        "weibo",
        "qq",
        "ttJSCore",
        "tt",
        "openWeixin",
        "openQQ",
        "x5",
    ];
    PROBES
        .iter()
        .filter(|k| crate::ctx::prop_exists(nav_js, k))
        .map(|s| s.to_string())
        .collect()
}
