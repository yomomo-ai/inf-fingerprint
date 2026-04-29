use serde::Serialize;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

#[derive(Serialize, Default)]
pub struct PermissionsFp {
    pub geolocation: Option<String>,
    pub notifications: Option<String>,
    pub push: Option<String>,
    pub camera: Option<String>,
    pub microphone: Option<String>,
    pub clipboard_read: Option<String>,
    pub persistent_storage: Option<String>,
}

pub async fn collect() -> Option<PermissionsFp> {
    let nav = crate::ctx::navigator()?;
    let perms = nav.permissions().ok()?;

    Some(PermissionsFp {
        geolocation: query(&perms, "geolocation").await,
        notifications: query(&perms, "notifications").await,
        push: query(&perms, "push").await,
        camera: query(&perms, "camera").await,
        microphone: query(&perms, "microphone").await,
        clipboard_read: query(&perms, "clipboard-read").await,
        persistent_storage: query(&perms, "persistent-storage").await,
    })
}

async fn query(perms: &web_sys::Permissions, name: &str) -> Option<String> {
    let desc = js_sys::Object::new();
    js_sys::Reflect::set(&desc, &"name".into(), &name.into()).ok()?;
    let promise = perms.query(&desc).ok()?;
    let result = JsFuture::from(promise).await.ok()?;
    let status: web_sys::PermissionStatus = result.dyn_into().ok()?;
    Some(format!("{:?}", status.state()).to_lowercase())
}
