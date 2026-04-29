use serde::Serialize;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

#[derive(Serialize)]
pub struct CanvasFp {
    pub hash: String,
    pub data_url_len: usize,
    pub winding: bool,
    pub cjk_text_width: f64,
}

pub fn collect() -> Option<CanvasFp> {
    let document = crate::ctx::document()?;
    let canvas: HtmlCanvasElement = document.create_element("canvas").ok()?.dyn_into().ok()?;
    canvas.set_width(280);
    canvas.set_height(60);

    let ctx: CanvasRenderingContext2d = canvas.get_context("2d").ok()??.dyn_into().ok()?;

    ctx.rect(0.0, 0.0, 10.0, 10.0);
    ctx.rect(2.0, 2.0, 6.0, 6.0);
    let winding = ctx.is_point_in_path_with_f64(5.0, 5.0);

    ctx.clear_rect(0.0, 0.0, 280.0, 60.0);
    ctx.begin_path();

    let _ = ctx.set_global_composite_operation("source-over");
    ctx.set_text_baseline("alphabetic");
    ctx.set_fill_style_str("#069");
    ctx.set_font("11pt 'Arial Unicode MS', 'Microsoft YaHei', sans-serif");
    let _ = ctx.fill_text("inf-fp 你好\u{1F44B}\u{1F600}", 2.0, 22.0);

    ctx.set_fill_style_str("rgba(102, 204, 0, 0.7)");
    ctx.set_font("18pt 'PingFang SC', 'Helvetica Neue', sans-serif");
    let _ = ctx.fill_text("ABC abc 中文测试", 4.0, 45.0);

    ctx.begin_path();
    ctx.set_fill_style_str("rgba(255, 0, 0, 0.5)");
    let _ = ctx.arc(50.0, 50.0, 25.0, 0.0, std::f64::consts::PI * 2.0);
    ctx.fill();
    ctx.set_fill_style_str("rgba(0, 0, 255, 0.5)");
    let _ = ctx.arc(70.0, 50.0, 25.0, 0.0, std::f64::consts::PI * 2.0);
    ctx.fill();

    ctx.set_font("16px sans-serif");
    let cjk_width = ctx
        .measure_text("你好世界")
        .ok()
        .map(|m| m.width())
        .unwrap_or(0.0);

    let data_url = canvas.to_data_url().ok()?;
    Some(CanvasFp {
        hash: crate::hash::hash_bytes(data_url.as_bytes()),
        data_url_len: data_url.len(),
        winding,
        cjk_text_width: cjk_width,
    })
}
