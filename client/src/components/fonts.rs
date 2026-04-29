use serde::Serialize;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

#[derive(Serialize)]
pub struct FontsFp {
    pub available: Vec<String>,
    pub baseline_widths: BaselineWidths,
}

#[derive(Serialize)]
pub struct BaselineWidths {
    pub serif: f64,
    pub sans_serif: f64,
    pub monospace: f64,
}

const TEST_TEXT: &str = "mmmmmmmmmlli中文测试";
const TEST_SIZE_PX: u32 = 72;

const FONT_LIST: &[&str] = &[
    "Arial",
    "Arial Black",
    "Arial Narrow",
    "Calibri",
    "Cambria",
    "Candara",
    "Consolas",
    "Courier",
    "Courier New",
    "Georgia",
    "Helvetica",
    "Helvetica Neue",
    "Impact",
    "Lucida Console",
    "Lucida Sans Unicode",
    "Tahoma",
    "Times",
    "Times New Roman",
    "Trebuchet MS",
    "Verdana",
    "PingFang SC",
    "PingFang TC",
    "PingFang HK",
    "Hiragino Sans GB",
    "Heiti SC",
    "Heiti TC",
    "STHeiti",
    "STSong",
    "STKaiti",
    "STFangsong",
    "STXihei",
    "SimSun",
    "SimSun-ExtB",
    "NSimSun",
    "SimHei",
    "FangSong",
    "KaiTi",
    "Microsoft YaHei",
    "Microsoft YaHei UI",
    "Microsoft JhengHei",
    "MingLiU",
    "PMingLiU",
    "DengXian",
    "Source Han Sans CN",
    "Source Han Sans SC",
    "Source Han Serif CN",
    "Source Han Serif SC",
    "Noto Sans CJK SC",
    "Noto Sans SC",
    "Noto Serif SC",
    "Noto Sans Mono CJK SC",
    "Sarasa Gothic SC",
    "Sarasa Mono SC",
    "HarmonyOS Sans",
    "HarmonyOS Sans SC",
    "HarmonyOS Sans TC",
    "HarmonyOS_Sans_SC",
    "MIUI",
    "MI LANTING",
    "Mi Sans",
    "OPPOSans",
    "OPPO Sans",
    "OPlusSans",
    "ColorOS Sans",
    "VivoSans",
    "Vivo Sans",
    "DroidSansFallback",
    "Apple Color Emoji",
    "Segoe UI Emoji",
    "Noto Color Emoji",
];

const BASELINES: &[&str] = &["serif", "sans-serif", "monospace"];

pub fn collect() -> Option<FontsFp> {
    let document = crate::ctx::document()?;
    let canvas: HtmlCanvasElement = document.create_element("canvas").ok()?.dyn_into().ok()?;
    canvas.set_width(800);
    canvas.set_height(100);
    let ctx: CanvasRenderingContext2d = canvas.get_context("2d").ok()??.dyn_into().ok()?;
    ctx.set_text_baseline("top");

    let mut baseline = [0.0f64; 3];
    for (i, fam) in BASELINES.iter().enumerate() {
        ctx.set_font(&format!("{}px {}", TEST_SIZE_PX, fam));
        baseline[i] = ctx.measure_text(TEST_TEXT).ok()?.width();
    }

    let mut available = Vec::new();
    for font in FONT_LIST {
        for (i, fam) in BASELINES.iter().enumerate() {
            ctx.set_font(&format!("{}px '{}', {}", TEST_SIZE_PX, font, fam));
            let w = ctx.measure_text(TEST_TEXT).ok()?.width();
            if (w - baseline[i]).abs() > 0.5 {
                available.push((*font).to_string());
                break;
            }
        }
    }

    Some(FontsFp {
        available,
        baseline_widths: BaselineWidths {
            serif: baseline[0],
            sans_serif: baseline[1],
            monospace: baseline[2],
        },
    })
}
