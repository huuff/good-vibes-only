//! Platform-aware civil-date clock.
//!
//! Chrono can fall back to UTC on Android when the process cannot discover
//! the device's IANA time zone. Ask Android's Java runtime for its calendar
//! fields instead; those always use the time zone selected in system settings.

use chrono::NaiveDate;

#[cfg(not(target_os = "android"))]
pub fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}

#[cfg(target_os = "android")]
pub fn today() -> NaiveDate {
    android_today().unwrap_or_else(|| chrono::Local::now().date_naive())
}

#[cfg(target_os = "android")]
fn android_today() -> Option<NaiveDate> {
    use jni::objects::{JObject, JValue};

    // java.util.Calendar.getInstance() uses TimeZone.getDefault(), which is
    // backed by Android's system time-zone setting (including DST).
    const YEAR: i32 = 1;
    const MONTH: i32 = 2;
    const DAY_OF_MONTH: i32 = 5;

    let ctx = ndk_context::android_context();
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }.ok()?;
    let mut env = vm.attach_current_thread().ok()?;
    let calendar = env
        .call_static_method(
            "java/util/Calendar",
            "getInstance",
            "()Ljava/util/Calendar;",
            &[],
        )
        .ok()?
        .l()
        .ok()?;

    let mut get = |field| {
        env.call_method(&calendar, "get", "(I)I", &[JValue::Int(field)])
            .ok()?
            .i()
            .ok()
    };
    let year = get(YEAR)?;
    let month = get(MONTH)? + 1; // Calendar months are zero-based.
    let day = get(DAY_OF_MONTH)?;

    // Keep the imported object type explicit so JNI API changes cannot turn
    // the local reference into an accidentally owned global reference.
    let _: &JObject<'_> = &calendar;
    NaiveDate::from_ymd_opt(year, month as u32, day as u32)
}
