use serde::Serialize;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

#[derive(Serialize)]
pub struct SpeechFp {
    pub voices: Vec<VoiceInfo>,
    pub voice_count: usize,
    pub hash: String,
}

#[derive(Serialize)]
pub struct VoiceInfo {
    pub name: String,
    pub lang: String,
    pub voice_uri: String,
    pub local_service: bool,
    pub default: bool,
}

pub async fn collect() -> Option<SpeechFp> {
    let voices = list_voices();
    if !voices.is_empty() {
        return Some(build(voices));
    }

    // iOS Safari/WeChat returns voices asynchronously after page load — wait briefly and retry.
    let _ = sleep_ms(60).await;
    let voices = list_voices();
    if !voices.is_empty() {
        return Some(build(voices));
    }

    let _ = sleep_ms(240).await;
    let voices = list_voices();
    if voices.is_empty() {
        return None;
    }
    Some(build(voices))
}

fn list_voices() -> Vec<VoiceInfo> {
    let Some(window) = crate::ctx::window() else {
        return Vec::new();
    };
    let win_js: &JsValue = window.as_ref();
    let Some(speech) = crate::ctx::prop_object(win_js, "speechSynthesis") else {
        return Vec::new();
    };
    let Some(get_voices_val) = crate::ctx::prop_object(&speech, "getVoices") else {
        return Vec::new();
    };
    let get_voices: js_sys::Function = match get_voices_val.dyn_into() {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };
    let voices_val = match get_voices.call0(&speech) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let voices_arr: js_sys::Array = match voices_val.dyn_into() {
        Ok(a) => a,
        Err(_) => return Vec::new(),
    };

    let mut out: Vec<VoiceInfo> = (0..voices_arr.length())
        .filter_map(|i| {
            let v = voices_arr.get(i);
            let name = crate::ctx::prop_string(&v, "name")?;
            Some(VoiceInfo {
                name,
                lang: crate::ctx::prop_string(&v, "lang").unwrap_or_default(),
                voice_uri: crate::ctx::prop_string(&v, "voiceURI").unwrap_or_default(),
                local_service: crate::ctx::prop_bool(&v, "localService").unwrap_or(false),
                default: crate::ctx::prop_bool(&v, "default").unwrap_or(false),
            })
        })
        .collect();
    out.sort_by(|a, b| a.voice_uri.cmp(&b.voice_uri).then(a.name.cmp(&b.name)));
    out
}

fn build(voices: Vec<VoiceInfo>) -> SpeechFp {
    let mut buf: Vec<u8> = Vec::with_capacity(voices.len() * 64);
    for v in &voices {
        buf.extend_from_slice(v.voice_uri.as_bytes());
        buf.push(b'|');
        buf.extend_from_slice(v.name.as_bytes());
        buf.push(b'|');
        buf.extend_from_slice(v.lang.as_bytes());
        buf.push(b'|');
        buf.push(if v.local_service { b'1' } else { b'0' });
        buf.push(if v.default { b'1' } else { b'0' });
        buf.push(b'\n');
    }
    let hash = crate::hash::hash_bytes(&buf);
    let voice_count = voices.len();
    SpeechFp {
        voices,
        voice_count,
        hash,
    }
}

async fn sleep_ms(ms: i32) -> Result<(), JsValue> {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        if let Some(w) = web_sys::window() {
            let _ = w.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
        }
    });
    JsFuture::from(promise).await.map(|_| ())
}
