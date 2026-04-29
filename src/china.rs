use serde::Serialize;
use wasm_bindgen::JsValue;

use crate::hash::hash_bytes;

#[derive(Serialize, PartialEq, Eq, Clone, Copy, Debug)]
#[serde(rename_all = "snake_case")]
pub enum InAppBrowser {
    Unknown,
    WeChat,
    WeChatMiniProgram,
    WeChatWork,
    Alipay,
    AlipayMiniProgram,
    DingTalk,
    Feishu,
    LarkInternational,
    Qq,
    QqBrowser,
    Weibo,
    Douyin,
    Toutiao,
    Kuaishou,
    XiaoHongShu,
    Bilibili,
    Meituan,
    Eleme,
    Taobao,
    Jd,
    Pinduoduo,
    Baidu,
    UcBrowser,
    Quark,
    SougouBrowser,
    QhBrowser360,
    MiuiBrowser,
    HuaweiBrowser,
    VivoBrowser,
    OppoBrowser,
    SamsungBrowser,
}

#[derive(Serialize, PartialEq, Eq, Clone, Copy, Debug)]
#[serde(rename_all = "snake_case")]
pub enum DeviceVendor {
    Unknown,
    Apple,
    Huawei,
    Honor,
    Xiaomi,
    Redmi,
    Vivo,
    Oppo,
    OnePlus,
    Realme,
    Samsung,
    Lenovo,
    Meizu,
    Zte,
}

#[derive(Serialize, PartialEq, Eq, Clone, Copy, Debug)]
#[serde(rename_all = "snake_case")]
pub enum SystemRom {
    Unknown,
    Ios,
    Ipados,
    Macos,
    HarmonyOS,
    Emui,
    Miui,
    HyperOs,
    ColorOs,
    OriginOs,
    OneUi,
    Flyme,
    Android,
}

#[derive(Serialize, PartialEq, Eq, Clone, Copy, Debug)]
#[serde(rename_all = "snake_case")]
pub enum WeChatPlatform {
    Unknown,
    AndroidArm64,
    AndroidArm,
    AndroidLegacy,
    Ios,
    Windows,
    Mac,
}

#[derive(Serialize)]
pub struct ChinaSignals {
    pub in_app: InAppBrowser,
    pub in_app_version: Option<String>,
    /// Hex code from MicroMessenger/8.0.54.2760(0x28003653) — opaque but stable per app version.
    pub in_app_version_code: Option<String>,
    /// Decoded WeChat platform from the hex code's upper byte.
    pub wechat_platform: WeChatPlatform,
    pub device_vendor: DeviceVendor,
    pub system_rom: SystemRom,
    pub system_version: Option<String>,
    /// Device model string embedded in UA: V2307A, M2102K1AC, PGT-AL10, MI MAX, ...
    pub device_model: Option<String>,
    /// Android build code (Build/UP1A.231005.007 → UP1A.231005.007).
    pub android_build: Option<String>,
    pub language_tag: Option<String>,
    pub kernel: Kernel,
    pub bridges: Bridges,
    pub volatile: Volatile,
    /// Hash of the stable UA tokens only — used in visitor_id, immune to per-instance noise.
    pub canonical_ua_hash: String,
    pub user_agent: String,
}

#[derive(Serialize, Default)]
pub struct Kernel {
    /// XWEB/<n> — current WeChat Android kernel (replaced X5/TBS around 2019).
    pub xweb: Option<String>,
    /// MMWEBSDK/<YYYYMMDD> — WeChat WebSDK build date.
    pub mmweb_sdk: Option<String>,
    /// TBS/<n> — legacy Tencent X5 kernel version.
    pub x5_tbs: Option<String>,
    /// Chrome/<x.x.x.x> — underlying Chromium version on Android.
    pub chromium: Option<String>,
    /// AppleWebKit/<x.x> — WebKit version on iOS Safari/in-app.
    pub webkit: Option<String>,
    /// Mobile/<build> — iOS WebKit build code (e.g. 15E148).
    pub ios_mobile_build: Option<String>,
    /// arm64 / armv7 / x86_64 — extracted from WeChat/<arch>, ABI/<arch>, or CPU x86_64.
    pub arch: Option<String>,
    /// `wv` marker indicates Android WebView (vs full browser).
    pub is_webview: bool,
}

#[derive(Serialize)]
pub struct Bridges {
    pub weixin: BridgeSpec,
    pub alipay: BridgeSpec,
    pub dingtalk: BridgeSpec,
    pub feishu: BridgeSpec,
    pub bytedance_tt: BridgeSpec,
    pub mqq: BridgeSpec,
    pub uc: BridgeSpec,
    pub baidu: BridgeSpec,
    pub jd: BridgeSpec,
    pub taobao: BridgeSpec,
    pub douyin: BridgeSpec,
    pub webkit_message_handlers: bool,
}

#[derive(Serialize, Default)]
pub struct BridgeSpec {
    pub present: bool,
    /// Names of function-typed methods on the bridge object.
    pub methods: Vec<&'static str>,
    /// Whether the corresponding `*Ready` event/object is present.
    pub ready_event: bool,
}

#[derive(Serialize, Default)]
pub struct Volatile {
    /// MMWEBID/<n> — per WebView instance, NOT stable per device.
    pub mmweb_id: Option<String>,
    /// NetType/<WIFI|4G|5G|...> — current network.
    pub net_type: Option<String>,
    /// Process/<toolsmp|tools|appbrand> — sub-process; varies between H5 and mini-program.
    pub process: Option<String>,
    /// Google Play Services version, usually 0 or absent on mainland devices.
    pub gp_version: Option<String>,
    /// Request-Source / Request-Channel / Weixin / qcloudcdn-* — per-request markers.
    pub request_markers: Vec<String>,
}

pub fn detect() -> ChinaSignals {
    let ua = ua_string();
    let mp_env = detect_mini_program_env();
    let in_app = detect_in_app(&ua, mp_env.as_deref());
    let in_app_version = extract_in_app_version(&ua, in_app);
    let in_app_version_code = extract_in_app_version_code(&ua, in_app);
    let wechat_platform = decode_wechat_platform(in_app_version_code.as_deref());
    let language_tag = extract_kv(&ua, "Language");
    let kernel = parse_kernel(&ua);
    let (device_vendor, system_rom, system_version) = detect_device(&ua);
    let device_model = extract_device_model(&ua, system_rom);
    let android_build =
        extract_after(&ua, "Build/").and_then(|s| if s.is_empty() { None } else { Some(s) });
    let bridges = detect_bridges();
    let volatile = parse_volatile(&ua, mp_env.clone());

    let canonical_ua_hash = canonical_hash(
        in_app,
        in_app_version.as_deref(),
        in_app_version_code.as_deref(),
        device_vendor,
        system_rom,
        system_version.as_deref(),
        device_model.as_deref(),
        android_build.as_deref(),
        language_tag.as_deref(),
        &kernel,
    );

    ChinaSignals {
        in_app,
        in_app_version,
        in_app_version_code,
        wechat_platform,
        device_vendor,
        system_rom,
        system_version,
        device_model,
        android_build,
        language_tag,
        kernel,
        bridges,
        volatile,
        canonical_ua_hash,
        user_agent: ua,
    }
}

fn ua_string() -> String {
    crate::ctx::navigator()
        .and_then(|n| n.user_agent().ok())
        .unwrap_or_default()
}

fn detect_in_app(ua: &str, mp_env: Option<&str>) -> InAppBrowser {
    use InAppBrowser::*;
    let lc = ua.to_ascii_lowercase();

    if lc.contains("wxwork") || lc.contains("wechatwork") {
        return WeChatWork;
    }
    if ua.contains("MicroMessenger") {
        if matches!(mp_env, Some("miniprogram")) {
            return WeChatMiniProgram;
        }
        return WeChat;
    }
    if ua.contains("AlipayClient") || ua.contains("AliApp(AP") {
        if matches!(mp_env, Some("miniprogram")) {
            return AlipayMiniProgram;
        }
        return Alipay;
    }
    if ua.contains("DingTalk") || lc.contains("dingtalk") {
        return DingTalk;
    }
    if ua.contains("Lark") || lc.contains("lark/") {
        if lc.contains("locale=zh") || lc.contains("language/zh") || lc.contains("larklocale=zh") {
            return Feishu;
        }
        return LarkInternational;
    }
    if ua.contains("aweme") || lc.contains("douyin") {
        return Douyin;
    }
    if ua.contains("NewsArticle") || ua.contains("news_article") || lc.contains("toutiao") {
        return Toutiao;
    }
    if lc.contains("kwai") || lc.contains("kuaishou") {
        return Kuaishou;
    }
    if ua.contains("xhsdiscover") || lc.contains("xiaohongshu") {
        return XiaoHongShu;
    }
    if lc.contains("bilibili") {
        return Bilibili;
    }
    if ua.contains("Meituan") || lc.contains("meituan") {
        return Meituan;
    }
    if lc.contains("eleme") {
        return Eleme;
    }
    if lc.contains("taobao") || lc.contains("aliapp(tb") {
        return Taobao;
    }
    if lc.contains("jdapp") || lc.contains("jingdong") {
        return Jd;
    }
    if lc.contains("pinduoduo") {
        return Pinduoduo;
    }
    if lc.contains("weibo") {
        return Weibo;
    }
    if ua.contains("MQQBrowser") {
        return QqBrowser;
    }
    if ua.contains("QQ/") {
        return Qq;
    }
    if ua.contains("baiduboxapp") || lc.contains("baidu") {
        return Baidu;
    }
    if ua.contains("UCBrowser") {
        return UcBrowser;
    }
    if ua.contains("Quark/") {
        return Quark;
    }
    if ua.contains("SogouMSE") || lc.contains("sogoubrowser") {
        return SougouBrowser;
    }
    if lc.contains("qhbrowser") || lc.contains("360browser") {
        return QhBrowser360;
    }
    if ua.contains("MiuiBrowser") || ua.contains("XiaoMi/MiuiBrowser") {
        return MiuiBrowser;
    }
    if ua.contains("HuaweiBrowser") {
        return HuaweiBrowser;
    }
    if ua.contains("VivoBrowser") {
        return VivoBrowser;
    }
    if ua.contains("HeyTapBrowser") || ua.contains("OppoBrowser") {
        return OppoBrowser;
    }
    if ua.contains("SamsungBrowser") {
        return SamsungBrowser;
    }
    Unknown
}

fn extract_in_app_version(ua: &str, app: InAppBrowser) -> Option<String> {
    use InAppBrowser::*;
    let token = match app {
        WeChat | WeChatMiniProgram => "MicroMessenger/",
        WeChatWork => "wxwork/",
        Alipay | AlipayMiniProgram => "AlipayClient/",
        DingTalk => "DingTalk/",
        Feishu | LarkInternational => "Lark/",
        Qq => "QQ/",
        QqBrowser => "MQQBrowser/",
        Douyin => "aweme/",
        Toutiao => "NewsArticle/",
        XiaoHongShu => "xhsdiscover/",
        Baidu => "baiduboxapp/",
        UcBrowser => "UCBrowser/",
        Quark => "Quark/",
        MiuiBrowser => "MiuiBrowser/",
        HuaweiBrowser => "HuaweiBrowser/",
        VivoBrowser => "VivoBrowser/",
        OppoBrowser => "HeyTapBrowser/",
        SamsungBrowser => "SamsungBrowser/",
        _ => return None,
    };
    extract_after(ua, token)
}

fn decode_wechat_platform(code: Option<&str>) -> WeChatPlatform {
    let code = match code {
        Some(c) => c,
        None => return WeChatPlatform::Unknown,
    };
    let stripped = code.trim_start_matches("0x").trim_start_matches("0X");
    let n = match u32::from_str_radix(stripped, 16) {
        Ok(n) => n,
        Err(_) => return WeChatPlatform::Unknown,
    };
    // Upper byte encodes platform/architecture per observed UAs in user-agents.io
    // and WeChat client builds: 0x18 iOS, 0x26-0x27 older Android, 0x28 Android-arm64,
    // 0x67 Windows, 0x73-0x74 macOS native.
    match (n >> 24) & 0xff {
        0x18 => WeChatPlatform::Ios,
        0x26 => WeChatPlatform::AndroidLegacy,
        0x27 => WeChatPlatform::AndroidArm,
        0x28 => WeChatPlatform::AndroidArm64,
        0x63 | 0x67 => WeChatPlatform::Windows,
        0x73 | 0x74 => WeChatPlatform::Mac,
        _ => WeChatPlatform::Unknown,
    }
}

/// `MicroMessenger/8.0.54.2760(0x28003653)` → `0x28003653`.
fn extract_in_app_version_code(ua: &str, app: InAppBrowser) -> Option<String> {
    if !matches!(
        app,
        InAppBrowser::WeChat | InAppBrowser::WeChatMiniProgram | InAppBrowser::WeChatWork
    ) {
        return None;
    }
    let i = ua.find("MicroMessenger/")?;
    let tail = &ua[i..];
    let lp = tail.find('(')?;
    let rp = tail[lp..].find(')')?;
    Some(tail[lp + 1..lp + rp].to_string())
}

fn parse_kernel(ua: &str) -> Kernel {
    Kernel {
        xweb: extract_kv(ua, "XWEB"),
        mmweb_sdk: extract_kv(ua, "MMWEBSDK"),
        x5_tbs: extract_kv(ua, "TBS"),
        chromium: extract_kv(ua, "Chrome"),
        webkit: extract_kv(ua, "AppleWebKit"),
        ios_mobile_build: extract_kv(ua, "Mobile"),
        arch: extract_arch(ua),
        is_webview: ua.contains("; wv)") || ua.contains(" wv)"),
    }
}

fn extract_arch(ua: &str) -> Option<String> {
    if let Some(v) = extract_kv(ua, "ABI") {
        return Some(v);
    }
    if let Some(v) = extract_kv(ua, "WeChat") {
        return Some(v);
    }
    if ua.contains("x86_64") {
        return Some("x86_64".to_string());
    }
    if ua.contains("aarch64") || ua.contains("arm64") {
        return Some("arm64".to_string());
    }
    None
}

fn extract_after(ua: &str, token: &str) -> Option<String> {
    let idx = ua.find(token)?;
    let after = &ua[idx + token.len()..];
    let end = after.find([' ', ';', ')', '(']).unwrap_or(after.len());
    Some(after[..end].to_string())
}

fn extract_kv(ua: &str, key: &str) -> Option<String> {
    let needle = format!("{}/", key);
    extract_after(ua, &needle)
}

/// Pull device model out of the parenthesized prefix `(Linux; Android 14; V2307A Build/...)`
/// or `(iPhone; CPU iPhone OS 17_5 like Mac OS X)`.
fn extract_device_model(ua: &str, rom: SystemRom) -> Option<String> {
    if matches!(rom, SystemRom::Ios | SystemRom::Ipados) {
        return None;
    }
    let lp = ua.find('(')?;
    let rp = ua[lp..].find(')')?;
    let inside = &ua[lp + 1..lp + rp];
    let parts: Vec<&str> = inside.split(';').map(|s| s.trim()).collect();
    for part in parts {
        if part.eq_ignore_ascii_case("wv") || part.eq_ignore_ascii_case("u") {
            continue;
        }
        if part.starts_with("Android ")
            || part.starts_with("Linux")
            || part.starts_with("U;")
            || part.starts_with("CPU ")
            || part.starts_with("HarmonyOS ")
            || part.starts_with("HyperOS ")
            || part.starts_with("OriginOS ")
            || part.starts_with("ColorOS ")
            || part.starts_with("MIUI ")
            || part.starts_with("EMUI ")
            || part.starts_with("Flyme ")
        {
            continue;
        }
        if let Some(model) = part.split(" Build/").next() {
            let trimmed = model.trim();
            if !trimmed.is_empty() && !trimmed.eq_ignore_ascii_case("Linux") {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn detect_mini_program_env() -> Option<String> {
    let window = crate::ctx::window()?;
    let win_js: &JsValue = window.as_ref();
    crate::ctx::prop_string(win_js, "__wxjs_environment")
}

fn detect_bridges() -> Bridges {
    let Some(window) = crate::ctx::window() else {
        return empty_bridges();
    };
    let win_js: &JsValue = window.as_ref();

    Bridges {
        weixin: probe_bridge(
            win_js,
            "WeixinJSBridge",
            &["invoke", "call", "on"],
            "WeixinJSBridgeReady",
        ),
        alipay: probe_bridge(
            win_js,
            "AlipayJSBridge",
            &["call", "invoke"],
            "AlipayJSBridgeReady",
        ),
        dingtalk: probe_bridge(
            win_js,
            "DingTalkJSBridge",
            &["call", "invoke"],
            "DingTalkJSBridgeReady",
        ),
        feishu: probe_bridge(
            win_js,
            "h5sdk",
            &["init", "ready", "biz", "config"],
            "h5sdkReady",
        ),
        bytedance_tt: probe_bridge(win_js, "tt", &["call", "invoke", "on"], "ttJSBridgeReady"),
        mqq: probe_bridge(win_js, "mqq", &["app", "ui", "media"], ""),
        uc: probe_bridge(win_js, "ucapi", &[], "ucapi_ready"),
        baidu: probe_bridge(
            win_js,
            "BoxJSBridge",
            &["call", "invoke"],
            "BoxJSBridgeReady",
        ),
        jd: probe_bridge(
            win_js,
            "MobileJSBridge",
            &["call", "invoke"],
            "MobileJSBridgeReady",
        ),
        taobao: probe_bridge(win_js, "WindVane", &["call", "fireEvent"], "WindVaneReady"),
        douyin: probe_bridge(
            win_js,
            "ToutiaoJSBridge",
            &["call", "invoke"],
            "ToutiaoJSBridgeReady",
        ),
        webkit_message_handlers: probe_webkit_handlers(win_js),
    }
}

fn empty_bridges() -> Bridges {
    Bridges {
        weixin: BridgeSpec::default(),
        alipay: BridgeSpec::default(),
        dingtalk: BridgeSpec::default(),
        feishu: BridgeSpec::default(),
        bytedance_tt: BridgeSpec::default(),
        mqq: BridgeSpec::default(),
        uc: BridgeSpec::default(),
        baidu: BridgeSpec::default(),
        jd: BridgeSpec::default(),
        taobao: BridgeSpec::default(),
        douyin: BridgeSpec::default(),
        webkit_message_handlers: false,
    }
}

fn probe_bridge(
    win: &JsValue,
    name: &str,
    method_candidates: &'static [&'static str],
    ready_event: &str,
) -> BridgeSpec {
    let Some(obj) = crate::ctx::prop_object(win, name) else {
        return BridgeSpec::default();
    };
    let methods: Vec<&'static str> = method_candidates
        .iter()
        .copied()
        .filter(|m| is_function(&obj, m))
        .collect();
    let ready_event = !ready_event.is_empty() && crate::ctx::prop_exists(win, ready_event);
    BridgeSpec {
        present: true,
        methods,
        ready_event,
    }
}

fn is_function(obj: &JsValue, name: &str) -> bool {
    let Some(v) = crate::ctx::prop_object(obj, name) else {
        return false;
    };
    v.is_function()
}

fn probe_webkit_handlers(win: &JsValue) -> bool {
    crate::ctx::prop_object(win, "webkit")
        .and_then(|wk| crate::ctx::prop_object(&wk, "messageHandlers"))
        .is_some()
}

fn parse_volatile(ua: &str, _mp_env: Option<String>) -> Volatile {
    let mut request_markers = Vec::new();
    for tok in ["Weixin", "qcloudcdn", "Request-Source=", "Request-Channel="] {
        if ua.contains(tok) {
            if let Some(idx) = ua.find(tok) {
                let after = &ua[idx..];
                let end = after.find([' ', ';']).unwrap_or(after.len());
                request_markers.push(after[..end].to_string());
            }
        }
    }
    Volatile {
        mmweb_id: extract_kv(ua, "MMWEBID"),
        net_type: extract_kv(ua, "NetType"),
        process: extract_kv(ua, "Process"),
        gp_version: extract_kv(ua, "GPVersion"),
        request_markers,
    }
}

fn detect_device(ua: &str) -> (DeviceVendor, SystemRom, Option<String>) {
    let lc = ua.to_ascii_lowercase();

    let vendor = if ua.contains("iPhone") || ua.contains("iPad") || ua.contains("iPod") {
        DeviceVendor::Apple
    } else if lc.contains("honor") {
        DeviceVendor::Honor
    } else if lc.contains("huawei") || lc.contains("emui") || lc.contains("harmonyos") {
        DeviceVendor::Huawei
    } else if lc.contains("redmi") {
        DeviceVendor::Redmi
    } else if lc.contains("xiaomi")
        || lc.contains("miui")
        || lc.contains("mi ")
        || lc.contains("hyperos")
    {
        DeviceVendor::Xiaomi
    } else if lc.contains("vivo") {
        DeviceVendor::Vivo
    } else if lc.contains("oneplus") {
        DeviceVendor::OnePlus
    } else if lc.contains("realme") {
        DeviceVendor::Realme
    } else if lc.contains("oppo") || lc.contains("coloros") || lc.contains("heytap") {
        DeviceVendor::Oppo
    } else if lc.contains("samsung") || lc.contains("sm-") {
        DeviceVendor::Samsung
    } else if lc.contains("lenovo") {
        DeviceVendor::Lenovo
    } else if lc.contains("meizu") || lc.contains("flyme") {
        DeviceVendor::Meizu
    } else if lc.contains("zte") {
        DeviceVendor::Zte
    } else {
        DeviceVendor::Unknown
    };

    let (rom, version) = if ua.contains("iPad") {
        let v = extract_after(ua, "OS ").map(|s| s.replace('_', "."));
        (SystemRom::Ipados, v)
    } else if ua.contains("iPhone") || ua.contains("iPod") {
        let v = extract_after(ua, "OS ").map(|s| s.replace('_', "."));
        (SystemRom::Ios, v)
    } else if ua.contains("Mac OS X") {
        let v = extract_after(ua, "Mac OS X ").map(|s| s.replace('_', "."));
        (SystemRom::Macos, v)
    } else if lc.contains("harmonyos") {
        (SystemRom::HarmonyOS, extract_after(ua, "HarmonyOS "))
    } else if lc.contains("emui") {
        (SystemRom::Emui, extract_kv(ua, "EMUI"))
    } else if lc.contains("hyperos") {
        (SystemRom::HyperOs, extract_after(ua, "HyperOS "))
    } else if lc.contains("miui") {
        (SystemRom::Miui, extract_kv(ua, "MIUI"))
    } else if lc.contains("coloros") {
        (SystemRom::ColorOs, extract_after(ua, "ColorOS "))
    } else if lc.contains("originos") {
        (SystemRom::OriginOs, extract_after(ua, "OriginOS "))
    } else if lc.contains("flyme") {
        (SystemRom::Flyme, extract_after(ua, "Flyme "))
    } else if lc.contains("oneui") {
        (SystemRom::OneUi, None)
    } else if ua.contains("Android") {
        (SystemRom::Android, extract_after(ua, "Android "))
    } else {
        (SystemRom::Unknown, None)
    };

    (vendor, rom, version)
}

#[allow(clippy::too_many_arguments)]
fn canonical_hash(
    in_app: InAppBrowser,
    in_app_version: Option<&str>,
    in_app_version_code: Option<&str>,
    device_vendor: DeviceVendor,
    system_rom: SystemRom,
    system_version: Option<&str>,
    device_model: Option<&str>,
    android_build: Option<&str>,
    language_tag: Option<&str>,
    kernel: &Kernel,
) -> String {
    let payload = format!(
        "ia:{:?}|iav:{}|iac:{}|dv:{:?}|sr:{:?}|sv:{}|dm:{}|ab:{}|lt:{}|xw:{}|ms:{}|x5:{}|cr:{}|wk:{}|imb:{}|ar:{}|wv:{}",
        in_app,
        in_app_version.unwrap_or(""),
        in_app_version_code.unwrap_or(""),
        device_vendor,
        system_rom,
        system_version.unwrap_or(""),
        device_model.unwrap_or(""),
        android_build.unwrap_or(""),
        language_tag.unwrap_or(""),
        kernel.xweb.as_deref().unwrap_or(""),
        kernel.mmweb_sdk.as_deref().unwrap_or(""),
        kernel.x5_tbs.as_deref().unwrap_or(""),
        kernel.chromium.as_deref().unwrap_or(""),
        kernel.webkit.as_deref().unwrap_or(""),
        kernel.ios_mobile_build.as_deref().unwrap_or(""),
        kernel.arch.as_deref().unwrap_or(""),
        kernel.is_webview,
    );
    hash_bytes(payload.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(ua: &str, key: &str) -> Option<String> {
        extract_kv(ua, key)
    }

    #[test]
    fn extract_kv_handles_token_at_end() {
        let ua = "Mozilla/5.0 ... MicroMessenger/8.0.42 NetType/WIFI Language/zh_CN";
        assert_eq!(k(ua, "NetType"), Some("WIFI".to_string()));
        assert_eq!(k(ua, "Language"), Some("zh_CN".to_string()));
        assert_eq!(k(ua, "Missing"), None);
    }

    #[test]
    fn extract_kv_handles_token_before_paren() {
        let ua = "Mozilla/5.0 (TBS/045901; iPhone) AppleWebKit/605.1.15";
        assert_eq!(k(ua, "TBS"), Some("045901".to_string()));
    }

    #[test]
    fn detect_in_app_wechat_first() {
        let ua = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) MicroMessenger/8.0.42 NetType/WIFI Language/zh_CN";
        assert_eq!(detect_in_app(ua, None), InAppBrowser::WeChat);
    }

    #[test]
    fn detect_in_app_wechat_miniprogram() {
        let ua = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) MicroMessenger/8.0.42 MiniProgramEnv/Windows";
        assert_eq!(
            detect_in_app(ua, Some("miniprogram")),
            InAppBrowser::WeChatMiniProgram
        );
    }

    #[test]
    fn detect_in_app_alipay() {
        let ua = "Mozilla/5.0 (Linux; U; Android 13) AlipayClient/10.6.20.7000 ChannelId(2)";
        assert_eq!(detect_in_app(ua, None), InAppBrowser::Alipay);
    }

    #[test]
    fn detect_device_apple_ios() {
        let ua = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_5_1 like Mac OS X)";
        let (v, r, ver) = detect_device(ua);
        assert_eq!(v, DeviceVendor::Apple);
        assert_eq!(r, SystemRom::Ios);
        assert!(ver.unwrap().starts_with("17.5"));
    }

    #[test]
    fn detect_device_huawei_harmony() {
        let ua = "Mozilla/5.0 (Linux; HarmonyOS 4.0; PGT-AL10) HuaweiBrowser/15.0";
        let (v, r, _) = detect_device(ua);
        assert_eq!(v, DeviceVendor::Huawei);
        assert_eq!(r, SystemRom::HarmonyOS);
    }

    #[test]
    fn extract_xweb_mmweb_tokens() {
        let ua = "Mozilla/5.0 (Linux; Android 14; V2307A Build/UP1A.231005.007; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/130.0.6723.103 Mobile Safari/537.36 XWEB/1300259 MMWEBSDK/20241103 MMWEBID/154 MicroMessenger/8.0.54.2760(0x28003653) WeChat/arm64 Weixin NetType/WIFI Language/zh_CN ABI/arm64";
        assert_eq!(k(ua, "XWEB"), Some("1300259".to_string()));
        assert_eq!(k(ua, "MMWEBSDK"), Some("20241103".to_string()));
        assert_eq!(k(ua, "MMWEBID"), Some("154".to_string()));
        assert_eq!(k(ua, "Chrome"), Some("130.0.6723.103".to_string()));
        assert_eq!(k(ua, "ABI"), Some("arm64".to_string()));
    }

    #[test]
    fn extract_device_model_android() {
        let ua =
            "Mozilla/5.0 (Linux; Android 14; V2307A Build/UP1A.231005.007; wv) AppleWebKit/537.36";
        assert_eq!(
            extract_device_model(ua, SystemRom::Android),
            Some("V2307A".to_string())
        );
    }

    #[test]
    fn extract_device_model_xiaomi() {
        let ua = "Mozilla/5.0 (Linux; Android 12; M2102K1AC Build/SKQ1.220213.001; wv) AppleWebKit/537.36";
        assert_eq!(
            extract_device_model(ua, SystemRom::Android),
            Some("M2102K1AC".to_string())
        );
    }

    #[test]
    fn extract_device_model_legacy_mi_max() {
        let ua = "Mozilla/5.0 (Linux; Android 7.0; MI MAX Build/NRD90M; wv) AppleWebKit/537.36";
        assert_eq!(
            extract_device_model(ua, SystemRom::Android),
            Some("MI MAX".to_string())
        );
    }

    #[test]
    fn extract_android_build() {
        let ua =
            "Mozilla/5.0 (Linux; Android 14; V2307A Build/UP1A.231005.007; wv) AppleWebKit/537.36";
        assert_eq!(
            extract_after(ua, "Build/"),
            Some("UP1A.231005.007".to_string())
        );
    }

    #[test]
    fn extract_micro_messenger_hex_code() {
        let ua = "...MicroMessenger/8.0.54.2760(0x28003653) WeChat/arm64...";
        assert_eq!(
            extract_in_app_version_code(ua, InAppBrowser::WeChat),
            Some("0x28003653".to_string())
        );
    }

    #[test]
    fn decode_wechat_platform_from_hex() {
        assert_eq!(
            decode_wechat_platform(Some("0x28003653")),
            WeChatPlatform::AndroidArm64
        );
        assert_eq!(
            decode_wechat_platform(Some("0x18003133")),
            WeChatPlatform::Ios
        );
        assert_eq!(
            decode_wechat_platform(Some("0x67000000")),
            WeChatPlatform::Windows
        );
        assert_eq!(
            decode_wechat_platform(Some("0x27001543")),
            WeChatPlatform::AndroidArm
        );
        assert_eq!(decode_wechat_platform(None), WeChatPlatform::Unknown);
        assert_eq!(
            decode_wechat_platform(Some("garbage")),
            WeChatPlatform::Unknown
        );
    }

    #[test]
    fn parse_kernel_full_wechat_android() {
        let ua = "Mozilla/5.0 (Linux; Android 14; V2307A Build/UP1A.231005.007; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/130.0.6723.103 Mobile Safari/537.36 XWEB/1300259 MMWEBSDK/20241103 MMWEBID/154 MicroMessenger/8.0.54.2760(0x28003653) WeChat/arm64 Weixin NetType/WIFI Language/zh_CN ABI/arm64";
        let kernel = parse_kernel(ua);
        assert_eq!(kernel.xweb.as_deref(), Some("1300259"));
        assert_eq!(kernel.mmweb_sdk.as_deref(), Some("20241103"));
        assert_eq!(kernel.chromium.as_deref(), Some("130.0.6723.103"));
        assert_eq!(kernel.arch.as_deref(), Some("arm64"));
        assert!(kernel.is_webview);
    }

    #[test]
    fn parse_volatile_wechat_android() {
        let ua = "...XWEB/1300259 MMWEBSDK/20241103 MMWEBID/154 MicroMessenger/8.0.54 WeChat/arm64 Weixin NetType/WIFI Language/zh_CN ABI/arm64";
        let v = parse_volatile(ua, None);
        assert_eq!(v.mmweb_id.as_deref(), Some("154"));
        assert_eq!(v.net_type.as_deref(), Some("WIFI"));
        assert!(v.request_markers.iter().any(|m| m == "Weixin"));
    }

    #[test]
    fn canonical_hash_stable_across_volatile_changes() {
        // Same device, two different webview instances — only MMWEBID and NetType differ.
        let ua_a = "Mozilla/5.0 (Linux; Android 14; V2307A Build/UP1A.231005.007; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/130.0.6723.103 Mobile Safari/537.36 XWEB/1300259 MMWEBSDK/20241103 MMWEBID/154 MicroMessenger/8.0.54.2760(0x28003653) WeChat/arm64 NetType/WIFI Language/zh_CN ABI/arm64";
        let ua_b = "Mozilla/5.0 (Linux; Android 14; V2307A Build/UP1A.231005.007; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/130.0.6723.103 Mobile Safari/537.36 XWEB/1300259 MMWEBSDK/20241103 MMWEBID/9999 MicroMessenger/8.0.54.2760(0x28003653) WeChat/arm64 NetType/4G Language/zh_CN ABI/arm64";

        let make = |ua: &str| {
            let in_app = detect_in_app(ua, None);
            canonical_hash(
                in_app,
                extract_in_app_version(ua, in_app).as_deref(),
                extract_in_app_version_code(ua, in_app).as_deref(),
                detect_device(ua).0,
                detect_device(ua).1,
                detect_device(ua).2.as_deref(),
                extract_device_model(ua, detect_device(ua).1).as_deref(),
                extract_after(ua, "Build/").as_deref(),
                extract_kv(ua, "Language").as_deref(),
                &parse_kernel(ua),
            )
        };

        assert_eq!(
            make(ua_a),
            make(ua_b),
            "canonical hash should be stable across MMWEBID/NetType changes"
        );
    }
}
