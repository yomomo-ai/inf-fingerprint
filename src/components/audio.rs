use serde::Serialize;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{OfflineAudioContext, OscillatorType};

#[derive(Serialize)]
pub struct AudioFp {
    pub hash: String,
    pub sample_checksum: f64,
    /// Median-of-N checksum rounded to 2 decimals. Resilient against per-render noise
    /// injection (Safari 17+ private mode, iOS 26 ATFP).
    pub stable_checksum: f64,
    pub sample_count: u32,
    /// True when N renders produced identical output. False indicates noise injection.
    pub stable: bool,
    pub renders: u32,
}

pub async fn collect() -> Option<AudioFp> {
    let mut checksums = Vec::with_capacity(5);
    let mut first_bytes: Option<Vec<u8>> = None;
    let mut all_identical = true;

    for i in 0..5 {
        let Some((checksum, bytes)) = render_one().await else {
            if i == 0 {
                return None;
            }
            break;
        };
        if let Some(prev) = &first_bytes {
            if prev != &bytes {
                all_identical = false;
            }
        } else {
            first_bytes = Some(bytes);
        }
        checksums.push(checksum);
    }

    if checksums.is_empty() {
        return None;
    }

    let primary_bytes = first_bytes.unwrap_or_default();
    let primary_checksum = checksums[0];

    // Median + round to 2 decimals = stable across noise.
    let mut sorted = checksums.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = sorted[sorted.len() / 2];
    let stable_checksum = (median * 100.0).round() / 100.0;

    Some(AudioFp {
        hash: crate::hash::hash_bytes(&primary_bytes),
        sample_checksum: primary_checksum,
        stable_checksum,
        sample_count: (primary_bytes.len() / 4) as u32,
        stable: all_identical,
        renders: checksums.len() as u32,
    })
}

async fn render_one() -> Option<(f64, Vec<u8>)> {
    try_render().await.ok()
}

async fn try_render() -> Result<(f64, Vec<u8>), JsValue> {
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
    Ok((checksum, bytes))
}
