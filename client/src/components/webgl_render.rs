use serde::Serialize;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{HtmlCanvasElement, WebGlProgram, WebGlRenderingContext as Gl, WebGlShader};

#[derive(Serialize)]
pub struct WebglRenderFp {
    pub pixel_hash: String,
    pub pixel_hash_replay: Option<String>,
    /// Two back-to-back renders produced identical bytes. False indicates noise injection
    /// (Brave farbling, iOS 26 ATFP, etc.) — exclude this signal from visitor_id.
    pub stable: bool,
    pub read_width: u32,
    pub read_height: u32,
}

const VS_SRC: &str = "attribute vec2 attrVertex;varying vec2 varyinTexCoordinate;uniform vec2 uniformOffset;void main(){varyinTexCoordinate=attrVertex+uniformOffset;gl_Position=vec4(attrVertex,0,1);}";
const FS_SRC: &str = "precision mediump float;varying vec2 varyinTexCoordinate;void main(){gl_FragColor=vec4(varyinTexCoordinate,1,1);}";

pub fn collect() -> Option<WebglRenderFp> {
    let document = crate::ctx::document()?;
    let canvas: HtmlCanvasElement = document.create_element("canvas").ok()?.dyn_into().ok()?;
    canvas.set_width(256);
    canvas.set_height(128);

    let attrs = js_sys::Object::new();
    js_sys::Reflect::set(&attrs, &"preserveDrawingBuffer".into(), &JsValue::TRUE).ok()?;
    js_sys::Reflect::set(&attrs, &"antialias".into(), &JsValue::FALSE).ok()?;
    js_sys::Reflect::set(&attrs, &"alpha".into(), &JsValue::FALSE).ok()?;

    let raw = canvas
        .get_context_with_context_options("webgl", &attrs)
        .ok()
        .flatten()
        .or_else(|| {
            canvas
                .get_context_with_context_options("experimental-webgl", &attrs)
                .ok()
                .flatten()
        })?;
    let gl: Gl = raw.dyn_into().ok()?;

    let program = compile_program(&gl, VS_SRC, FS_SRC)?;
    gl.use_program(Some(&program));

    let vertices: [f32; 6] = [-0.9, -0.7, 0.8, -0.7, 0.0, 0.5];
    let buffer = gl.create_buffer()?;
    gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&buffer));
    unsafe {
        let view = js_sys::Float32Array::view(&vertices);
        gl.buffer_data_with_array_buffer_view(Gl::ARRAY_BUFFER, &view, Gl::STATIC_DRAW);
    }

    let attr_loc = gl.get_attrib_location(&program, "attrVertex");
    if attr_loc < 0 {
        return None;
    }
    gl.enable_vertex_attrib_array(attr_loc as u32);
    gl.vertex_attrib_pointer_with_i32(attr_loc as u32, 2, Gl::FLOAT, false, 0, 0);

    let offset_loc = gl.get_uniform_location(&program, "uniformOffset")?;
    gl.uniform2f(Some(&offset_loc), 1.0, 1.0);

    let read_w = (gl.drawing_buffer_width() / 15).max(1) as u32;
    let read_h = (gl.drawing_buffer_height() / 6).max(1) as u32;

    let pass1 = render_and_read(&gl, read_w, read_h)?;
    let hash1 = crate::hash::hash_bytes(&pass1);

    let (hash2, stable) = match render_and_read(&gl, read_w, read_h) {
        Some(pass2) => {
            let h = crate::hash::hash_bytes(&pass2);
            let same = pass2 == pass1;
            (Some(h), same)
        }
        None => (None, false),
    };

    Some(WebglRenderFp {
        pixel_hash: hash1,
        pixel_hash_replay: hash2,
        stable,
        read_width: read_w,
        read_height: read_h,
    })
}

fn render_and_read(gl: &Gl, w: u32, h: u32) -> Option<Vec<u8>> {
    gl.clear_color(0.0, 0.0, 0.0, 0.0);
    gl.clear(Gl::COLOR_BUFFER_BIT);
    gl.draw_arrays(Gl::LINE_LOOP, 0, 3);

    let mut pixels: Vec<u8> = vec![0u8; (w as usize) * (h as usize) * 4];
    gl.read_pixels_with_opt_u8_array(
        0,
        0,
        w as i32,
        h as i32,
        Gl::RGBA,
        Gl::UNSIGNED_BYTE,
        Some(&mut pixels),
    )
    .ok()?;
    Some(pixels)
}

fn compile_program(gl: &Gl, vs: &str, fs: &str) -> Option<WebGlProgram> {
    let vs = compile_shader(gl, Gl::VERTEX_SHADER, vs)?;
    let fs = compile_shader(gl, Gl::FRAGMENT_SHADER, fs)?;
    let program = gl.create_program()?;
    gl.attach_shader(&program, &vs);
    gl.attach_shader(&program, &fs);
    gl.link_program(&program);
    let linked = gl
        .get_program_parameter(&program, Gl::LINK_STATUS)
        .as_bool()
        .unwrap_or(false);
    if linked {
        Some(program)
    } else {
        None
    }
}

fn compile_shader(gl: &Gl, kind: u32, src: &str) -> Option<WebGlShader> {
    let shader = gl.create_shader(kind)?;
    gl.shader_source(&shader, src);
    gl.compile_shader(&shader);
    let compiled = gl
        .get_shader_parameter(&shader, Gl::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false);
    if compiled {
        Some(shader)
    } else {
        None
    }
}
