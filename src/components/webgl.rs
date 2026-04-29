use serde::Serialize;
use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, WebGlRenderingContext as Gl};

#[derive(Serialize)]
pub struct WebglFp {
    pub vendor: Option<String>,
    pub renderer: Option<String>,
    pub unmasked_vendor: Option<String>,
    pub unmasked_renderer: Option<String>,
    pub version: Option<String>,
    pub shading_language_version: Option<String>,
    pub max_texture_size: Option<i32>,
    pub max_viewport: Option<(i32, i32)>,
    pub max_anisotropy: Option<f32>,
    pub aliased_line_width_range: Option<(f32, f32)>,
    pub aliased_point_size_range: Option<(f32, f32)>,
    pub extensions: Vec<String>,
    pub shader_precision: ShaderPrecision,
    pub params_hash: String,
}

#[derive(Serialize, Default)]
pub struct ShaderPrecision {
    pub vert_high_float_precision: Option<i32>,
    pub frag_high_float_precision: Option<i32>,
    pub vert_med_float_precision: Option<i32>,
    pub frag_med_float_precision: Option<i32>,
}

pub fn collect() -> Option<WebglFp> {
    let document = crate::ctx::document()?;
    let canvas: HtmlCanvasElement = document.create_element("canvas").ok()?.dyn_into().ok()?;

    let raw = canvas
        .get_context("webgl2")
        .ok()
        .flatten()
        .or_else(|| canvas.get_context("webgl").ok().flatten())
        .or_else(|| canvas.get_context("experimental-webgl").ok().flatten())?;
    let ctx: Gl = raw.dyn_into().ok()?;

    let vendor = ctx
        .get_parameter(Gl::VENDOR)
        .ok()
        .and_then(|v| v.as_string());
    let renderer = ctx
        .get_parameter(Gl::RENDERER)
        .ok()
        .and_then(|v| v.as_string());
    let version = ctx
        .get_parameter(Gl::VERSION)
        .ok()
        .and_then(|v| v.as_string());
    let shading_language_version = ctx
        .get_parameter(Gl::SHADING_LANGUAGE_VERSION)
        .ok()
        .and_then(|v| v.as_string());
    let max_texture_size = ctx
        .get_parameter(Gl::MAX_TEXTURE_SIZE)
        .ok()
        .and_then(|v| v.as_f64())
        .map(|f| f as i32);

    let mut unmasked_vendor = None;
    let mut unmasked_renderer = None;
    if matches!(ctx.get_extension("WEBGL_debug_renderer_info"), Ok(Some(_))) {
        const UNMASKED_VENDOR_WEBGL: u32 = 0x9245;
        const UNMASKED_RENDERER_WEBGL: u32 = 0x9246;
        unmasked_vendor = ctx
            .get_parameter(UNMASKED_VENDOR_WEBGL)
            .ok()
            .and_then(|v| v.as_string());
        unmasked_renderer = ctx
            .get_parameter(UNMASKED_RENDERER_WEBGL)
            .ok()
            .and_then(|v| v.as_string());
    }

    let max_anisotropy = match ctx.get_extension("EXT_texture_filter_anisotropic") {
        Ok(Some(_)) => {
            const MAX_ANISO: u32 = 0x84FF;
            ctx.get_parameter(MAX_ANISO)
                .ok()
                .and_then(|v| v.as_f64())
                .map(|f| f as f32)
        }
        _ => None,
    };

    let aliased_line_width_range = read_f32_range(&ctx, Gl::ALIASED_LINE_WIDTH_RANGE);
    let aliased_point_size_range = read_f32_range(&ctx, Gl::ALIASED_POINT_SIZE_RANGE);
    let max_viewport = read_i32_pair(&ctx, Gl::MAX_VIEWPORT_DIMS);

    let extensions = ctx
        .get_supported_extensions()
        .map(|arr| {
            (0..arr.length())
                .filter_map(|i| arr.get(i).as_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let shader_precision = read_shader_precision(&ctx);

    let mut sorted_ext = extensions.clone();
    sorted_ext.sort();
    let payload = format!(
        "{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}",
        vendor,
        renderer,
        unmasked_vendor,
        unmasked_renderer,
        version,
        shading_language_version,
        max_texture_size,
        max_viewport,
        max_anisotropy,
        aliased_line_width_range,
        aliased_point_size_range,
        sorted_ext,
    );
    let params_hash = crate::hash::hash_bytes(payload.as_bytes());

    Some(WebglFp {
        vendor,
        renderer,
        unmasked_vendor,
        unmasked_renderer,
        version,
        shading_language_version,
        max_texture_size,
        max_viewport,
        max_anisotropy,
        aliased_line_width_range,
        aliased_point_size_range,
        extensions,
        shader_precision,
        params_hash,
    })
}

fn read_f32_range(ctx: &Gl, pname: u32) -> Option<(f32, f32)> {
    let v = ctx.get_parameter(pname).ok()?;
    let arr: js_sys::Float32Array = v.dyn_into().ok()?;
    if arr.length() >= 2 {
        Some((arr.get_index(0), arr.get_index(1)))
    } else {
        None
    }
}

fn read_i32_pair(ctx: &Gl, pname: u32) -> Option<(i32, i32)> {
    let v = ctx.get_parameter(pname).ok()?;
    let arr: js_sys::Int32Array = v.dyn_into().ok()?;
    if arr.length() >= 2 {
        Some((arr.get_index(0), arr.get_index(1)))
    } else {
        None
    }
}

fn read_shader_precision(ctx: &Gl) -> ShaderPrecision {
    let mut sp = ShaderPrecision::default();
    if let Some(fmt) = ctx.get_shader_precision_format(Gl::VERTEX_SHADER, Gl::HIGH_FLOAT) {
        sp.vert_high_float_precision = Some(fmt.precision());
    }
    if let Some(fmt) = ctx.get_shader_precision_format(Gl::FRAGMENT_SHADER, Gl::HIGH_FLOAT) {
        sp.frag_high_float_precision = Some(fmt.precision());
    }
    if let Some(fmt) = ctx.get_shader_precision_format(Gl::VERTEX_SHADER, Gl::MEDIUM_FLOAT) {
        sp.vert_med_float_precision = Some(fmt.precision());
    }
    if let Some(fmt) = ctx.get_shader_precision_format(Gl::FRAGMENT_SHADER, Gl::MEDIUM_FLOAT) {
        sp.frag_med_float_precision = Some(fmt.precision());
    }
    sp
}
