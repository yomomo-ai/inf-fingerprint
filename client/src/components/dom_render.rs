use serde::Serialize;
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

#[derive(Serialize)]
pub struct DomRenderFp {
    pub rect_hash: String,
    pub emoji_hash: String,
    pub probes: Vec<RectProbe>,
    pub emoji: RectProbe,
}

#[derive(Serialize)]
pub struct RectProbe {
    pub label: &'static str,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

const PROBES: &[(&str, &str)] = &[
    ("rotate3d_x", "transform:rotate3d(15,0,0,45deg);"),
    ("rotate3d_y", "transform:rotate3d(0,15,0,45deg);"),
    ("rotate3d_z", "transform:rotate3d(0,0,15,45deg);"),
    (
        "perspective_z",
        "transform:perspective(100px) translateZ(101.5px);",
    ),
    ("matrix_id", "transform:matrix(1,0,0,1,0,0);"),
    ("scale_subpx", "transform:scale(1.000999);"),
    ("scale_neg", "transform:scale(-1.000999);"),
    ("rotate_30", "transform:rotate(30deg);"),
    ("skew_15_30", "transform:skew(15deg,30deg);"),
    ("translate_subpx", "transform:translate(0.5px,0.7px);"),
];

const BASE_STYLE: &str = "position:absolute;left:0;top:0;font:16px sans-serif;width:200px;height:50px;line-height:50px;display:block;";

pub fn collect() -> Option<DomRenderFp> {
    let document = crate::ctx::document()?;
    let body = document.body()?;

    let container: HtmlElement = document.create_element("div").ok()?.dyn_into().ok()?;
    container
        .set_attribute(
            "style",
            "position:absolute;left:-9999px;top:0;width:200px;visibility:hidden;",
        )
        .ok()?;
    let _ = body.append_child(&container);

    let mut probes = Vec::with_capacity(PROBES.len());
    for (label, transform_style) in PROBES {
        let div: HtmlElement = match document
            .create_element("div")
            .ok()
            .and_then(|e| e.dyn_into().ok())
        {
            Some(d) => d,
            None => continue,
        };
        div.set_inner_text("inf-fp-probe \u{4e2d}\u{6587}");
        let _ = div.set_attribute("style", &format!("{}{}", BASE_STYLE, transform_style));
        let _ = container.append_child(&div);
        let r = div.get_bounding_client_rect();
        probes.push(RectProbe {
            label,
            x: r.x(),
            y: r.y(),
            w: r.width(),
            h: r.height(),
        });
    }

    let emoji_div: HtmlElement = document.create_element("div").ok()?.dyn_into().ok()?;
    emoji_div
        .set_inner_text("\u{1F44B}\u{1F600}\u{1F3AF}\u{1F525}\u{1F4AF}\u{1F680}\u{2705}\u{26A1}");
    let _ = emoji_div.set_attribute(
        "style",
        "position:absolute;left:0;top:0;font-size:200px;transform:scale(1.000999);display:inline-block;",
    );
    let _ = container.append_child(&emoji_div);
    let er = emoji_div.get_bounding_client_rect();
    let emoji = RectProbe {
        label: "emoji_subpx",
        x: er.x(),
        y: er.y(),
        w: er.width(),
        h: er.height(),
    };

    let _ = body.remove_child(&container);

    let rect_hash = hash_probes(&probes);
    let emoji_hash = hash_probes(std::slice::from_ref(&emoji));

    Some(DomRenderFp {
        rect_hash,
        emoji_hash,
        probes,
        emoji,
    })
}

fn hash_probes(probes: &[RectProbe]) -> String {
    let mut buf: Vec<u8> = Vec::with_capacity(probes.len() * 40);
    for p in probes {
        buf.extend_from_slice(p.label.as_bytes());
        buf.push(b':');
        buf.extend_from_slice(&p.x.to_le_bytes());
        buf.extend_from_slice(&p.y.to_le_bytes());
        buf.extend_from_slice(&p.w.to_le_bytes());
        buf.extend_from_slice(&p.h.to_le_bytes());
        buf.push(b'|');
    }
    crate::hash::hash_bytes(&buf)
}
