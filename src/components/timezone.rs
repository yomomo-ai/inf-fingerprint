use serde::Serialize;
use wasm_bindgen::{JsCast, JsValue};

#[derive(Serialize)]
pub struct TimezoneFp {
    pub timezone: Option<String>,
    pub timezone_offset_minutes: i32,
    pub locale: Option<String>,
    pub calendar: Option<String>,
    pub numbering_system: Option<String>,
    pub date_format_sample: Option<String>,
}

pub fn collect() -> Option<TimezoneFp> {
    let global = js_sys::global();
    let intl = crate::ctx::prop_object(&global, "Intl")?;
    let dtf_ctor: js_sys::Function = crate::ctx::prop_object(&intl, "DateTimeFormat")?
        .dyn_into()
        .ok()?;
    let dtf = js_sys::Reflect::construct(&dtf_ctor, &js_sys::Array::new()).ok()?;

    let resolved_fn: js_sys::Function = crate::ctx::prop_object(&dtf, "resolvedOptions")?
        .dyn_into()
        .ok()?;
    let resolved = resolved_fn.call0(&dtf).ok()?;

    let now = js_sys::Date::new_0();
    let timezone_offset_minutes = -(now.get_timezone_offset() as i32);

    Some(TimezoneFp {
        timezone: crate::ctx::prop_string(&resolved, "timeZone"),
        timezone_offset_minutes,
        locale: crate::ctx::prop_string(&resolved, "locale"),
        calendar: crate::ctx::prop_string(&resolved, "calendar"),
        numbering_system: crate::ctx::prop_string(&resolved, "numberingSystem"),
        date_format_sample: format_sample(),
    })
}

fn format_sample() -> Option<String> {
    let date = js_sys::Date::new_with_year_month_day(2020, 0, 15);
    let s: js_sys::JsString = date.to_locale_date_string("default", &JsValue::UNDEFINED);
    s.as_string()
}
