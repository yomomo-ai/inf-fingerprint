use serde::Serialize;
use wasm_bindgen::JsValue;

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

#[derive(Serialize)]
pub struct ChinaSignals {
    pub in_app: InAppBrowser,
    pub in_app_version: Option<String>,
    pub mini_program_env: Option<String>,
    pub net_type: Option<String>,
    pub language_tag: Option<String>,
    pub x5_kernel: Option<X5Info>,
    pub bridges: Bridges,
    pub device_vendor: DeviceVendor,
    pub system_rom: SystemRom,
    pub system_version: Option<String>,
    pub user_agent: String,
}

#[derive(Serialize)]
pub struct X5Info {
    pub version: String,
}

#[derive(Serialize, Default)]
pub struct Bridges {
    pub weixin_jsbridge: bool,
    pub alipay_jsbridge: bool,
    pub dd_jsbridge: bool,
    pub feishu_jsbridge: bool,
    pub tt_jsbridge: bool,
    pub mqq_app: bool,
    pub uc_api: bool,
    pub baidu_jsbridge: bool,
    pub jd_app_unite: bool,
    pub taobao_jsbridge: bool,
    pub douyin_jsbridge: bool,
}

pub fn detect() -> ChinaSignals {
    let ua = ua_string();
    let mini_program_env = detect_mini_program_env();
    let in_app = detect_in_app(&ua, mini_program_env.as_deref());
    let in_app_version = extract_in_app_version(&ua, in_app);
    let net_type = extract_kv(&ua, "NetType");
    let language_tag = extract_kv(&ua, "Language");
    let x5_kernel = extract_kv(&ua, "TBS").map(|version| X5Info { version });
    let bridges = detect_bridges();
    let (device_vendor, system_rom, system_version) = detect_device(&ua);

    ChinaSignals {
        in_app,
        in_app_version,
        mini_program_env,
        net_type,
        language_tag,
        x5_kernel,
        bridges,
        device_vendor,
        system_rom,
        system_version,
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

fn detect_mini_program_env() -> Option<String> {
    let window = crate::ctx::window()?;
    let win_js: &JsValue = window.as_ref();
    crate::ctx::prop_string(win_js, "__wxjs_environment")
}

fn detect_bridges() -> Bridges {
    let Some(window) = crate::ctx::window() else {
        return Bridges::default();
    };
    let win_js: &JsValue = window.as_ref();
    Bridges {
        weixin_jsbridge: crate::ctx::prop_exists(win_js, "WeixinJSBridge"),
        alipay_jsbridge: crate::ctx::prop_exists(win_js, "AlipayJSBridge"),
        dd_jsbridge: crate::ctx::prop_exists(win_js, "dd")
            || crate::ctx::prop_exists(win_js, "DingTalkJSBridge")
            || crate::ctx::prop_exists(win_js, "DDJSBridge"),
        feishu_jsbridge: crate::ctx::prop_exists(win_js, "h5sdk")
            || crate::ctx::prop_exists(win_js, "lark"),
        tt_jsbridge: crate::ctx::prop_exists(win_js, "tt"),
        mqq_app: crate::ctx::prop_exists(win_js, "qq")
            || crate::ctx::prop_exists(win_js, "mqq")
            || crate::ctx::prop_exists(win_js, "QQAPI"),
        uc_api: crate::ctx::prop_exists(win_js, "ucapi")
            || crate::ctx::prop_exists(win_js, "ucweb"),
        baidu_jsbridge: crate::ctx::prop_exists(win_js, "_bd_share_config")
            || crate::ctx::prop_exists(win_js, "BoxJSBridge"),
        jd_app_unite: crate::ctx::prop_exists(win_js, "MobileJSBridge"),
        taobao_jsbridge: crate::ctx::prop_exists(win_js, "WindVane")
            || crate::ctx::prop_exists(win_js, "lib"),
        douyin_jsbridge: crate::ctx::prop_exists(win_js, "ToutiaoJSBridge"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_kv_handles_token_at_end() {
        let ua = "Mozilla/5.0 ... MicroMessenger/8.0.42 NetType/WIFI Language/zh_CN";
        assert_eq!(extract_kv(ua, "NetType"), Some("WIFI".to_string()));
        assert_eq!(extract_kv(ua, "Language"), Some("zh_CN".to_string()));
        assert_eq!(extract_kv(ua, "Missing"), None);
    }

    #[test]
    fn extract_kv_handles_token_before_paren() {
        let ua = "Mozilla/5.0 (TBS/045901; iPhone) AppleWebKit/605.1.15";
        assert_eq!(extract_kv(ua, "TBS"), Some("045901".to_string()));
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
}
