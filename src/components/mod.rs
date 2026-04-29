use serde::Serialize;

mod audio;
mod canvas;
mod fonts;
mod navigator;
mod permissions;
mod screen;
mod timezone;
mod touch;
mod webgl;

pub use audio::AudioFp;
pub use canvas::CanvasFp;
pub use fonts::FontsFp;
pub use navigator::NavigatorFp;
pub use permissions::PermissionsFp;
pub use screen::ScreenFp;
pub use timezone::TimezoneFp;
pub use touch::TouchFp;
pub use webgl::WebglFp;

#[derive(Serialize, Default)]
pub struct Components {
    pub canvas: Option<CanvasFp>,
    pub webgl: Option<WebglFp>,
    pub audio: Option<AudioFp>,
    pub screen: Option<ScreenFp>,
    pub navigator: Option<NavigatorFp>,
    pub timezone: Option<TimezoneFp>,
    pub fonts: Option<FontsFp>,
    pub touch: Option<TouchFp>,
    pub permissions: Option<PermissionsFp>,
}

pub async fn collect() -> Components {
    let canvas = canvas::collect();
    let webgl = webgl::collect();
    let screen = screen::collect();
    let navigator = navigator::collect();
    let timezone = timezone::collect();
    let fonts = fonts::collect();
    let touch = touch::collect();
    let audio = audio::collect().await;
    let permissions = permissions::collect().await;

    Components {
        canvas,
        webgl,
        audio,
        screen,
        navigator,
        timezone,
        fonts,
        touch,
        permissions,
    }
}
