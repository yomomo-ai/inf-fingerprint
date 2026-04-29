use serde::Serialize;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{OfflineAudioContext, OscillatorType};

#[derive(Serialize)]
pub struct AudioFp {
    pub hash: String,
    pub sample_checksum: f64,
    pub sample_count: u32,
    pub mode: &'static str,
}

pub async fn collect() -> Option<AudioFp> {
    try_collect().await.ok()
}

async fn try_collect() -> Result<AudioFp, JsValue> {
    let ctx = OfflineAudioContext::new_with_number_of_channels_and_length_and_sample_rate(
        1, 5000, 44100.0,
    )?;

    let osc = ctx.create_oscillator()?;
    osc.set_type(OscillatorType::Triangle);
    osc.frequency().set_value(10000.0);

    let comp = ctx.create_dynamics_compressor()?;
    comp.threshold().set_value(-50.0);
    comp.knee().set_value(40.0);
    comp.ratio().set_value(12.0);
    comp.attack().set_value(0.0);
    comp.release().set_value(0.25);

    osc.connect_with_audio_node(&comp)?;
    comp.connect_with_audio_node(&ctx.destination())?;
    osc.start()?;

    let buffer_promise = ctx.start_rendering()?;
    let buffer = JsFuture::from(buffer_promise).await?;
    let buffer: web_sys::AudioBuffer = buffer.dyn_into()?;

    let channel: Vec<f32> = buffer.get_channel_data(0)?;
    let len = channel.len();
    let start = 4500.min(len);
    let end = 5000.min(len);
    let window = &channel[start..end];

    let mut checksum: f64 = 0.0;
    let mut bytes: Vec<u8> = Vec::with_capacity(window.len() * 4);
    for &v in window {
        checksum += v.abs() as f64;
        bytes.extend_from_slice(&v.to_le_bytes());
    }

    Ok(AudioFp {
        hash: crate::hash::hash_bytes(&bytes),
        sample_checksum: checksum,
        sample_count: window.len() as u32,
        mode: "offline",
    })
}
