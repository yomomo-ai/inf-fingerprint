use serde::Serialize;

mod audio;
mod canvas;
mod connection;
mod dom_render;
mod fonts;
mod math_fp;
mod navigator;
mod perf;
mod permissions;
mod quirks;
mod screen;
mod speech;
mod timezone;
mod touch;
mod webgl;
mod webgl_render;
mod webrtc;

pub use audio::AudioFp;
pub use canvas::CanvasFp;
pub use connection::ConnectionFp;
pub use dom_render::DomRenderFp;
pub use fonts::FontsFp;
pub use math_fp::MathFp;
pub use navigator::NavigatorFp;
pub use perf::PerfFp;
pub use permissions::PermissionsFp;
pub use quirks::QuirksFp;
pub use screen::ScreenFp;
pub use speech::SpeechFp;
pub use timezone::TimezoneFp;
pub use touch::TouchFp;
pub use webgl::WebglFp;
pub use webgl_render::WebglRenderFp;
pub use webrtc::WebrtcFp;

#[derive(Serialize, Default)]
pub struct Components {
    pub canvas: Option<CanvasFp>,
    pub webgl: Option<WebglFp>,
    pub webgl_render: Option<WebglRenderFp>,
    pub audio: Option<AudioFp>,
    pub screen: Option<ScreenFp>,
    pub navigator: Option<NavigatorFp>,
    pub timezone: Option<TimezoneFp>,
    pub fonts: Option<FontsFp>,
    pub touch: Option<TouchFp>,
    pub permissions: Option<PermissionsFp>,
    pub math: Option<MathFp>,
    pub speech: Option<SpeechFp>,
    pub connection: Option<ConnectionFp>,
    pub perf: Option<PerfFp>,
    pub dom: Option<DomRenderFp>,
    pub webrtc: Option<WebrtcFp>,
    pub quirks: Option<QuirksFp>,
}

pub async fn collect() -> Components {
    let canvas = canvas::collect();
    let webgl = webgl::collect();
    let webgl_render = webgl_render::collect();
    let screen = screen::collect();
    let navigator = navigator::collect();
    let timezone = timezone::collect();
    let fonts = fonts::collect();
    let touch = touch::collect();
    let connection = connection::collect();
    let perf = perf::collect();
    let math = Some(math_fp::collect());
    let dom = dom_render::collect();
    let quirks = quirks::collect();

    let audio = audio::collect().await;
    let permissions = permissions::collect().await;
    let speech = speech::collect().await;
    let webrtc = webrtc::collect().await;

    Components {
        canvas,
        webgl,
        webgl_render,
        audio,
        screen,
        navigator,
        timezone,
        fonts,
        touch,
        permissions,
        math,
        speech,
        connection,
        perf,
        dom,
        webrtc,
        quirks,
    }
}
