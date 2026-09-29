//! Platform-aware civil-date clock.
//!
//! Chrono can fall back to UTC on Android when the process cannot discover
//! the device's IANA time zone. Ask Android's Java runtime for its calendar
//! fields instead; those always use the time zone selected in system settings.

use chrono::{NaiveDate, NaiveDateTime};

#[cfg(not(target_os = "android"))]
pub fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}

#[cfg(not(target_os = "android"))]
pub fn now() -> NaiveDateTime {
    chrono::Local::now().naive_local()
}

#[cfg(target_os = "android")]
pub fn today() -> NaiveDate {
    now().date()
}

#[cfg(target_os = "android")]
pub fn now() -> NaiveDateTime {
    android_now().unwrap_or_else(|| chrono::Local::now().naive_local())
}

#[cfg(target_os = "android")]
fn android_now() -> Option<NaiveDateTime> {
    use jni::objects::JValue;

    // java.util.Calendar.getInstance() uses TimeZone.getDefault(), which is
    // backed by Android's system time-zone setting (including DST).
    const YEAR: i32 = 1;
    const MONTH: i32 = 2;
    const DAY_OF_MONTH: i32 = 5;
    const HOUR_OF_DAY: i32 = 11;
    const MINUTE: i32 = 12;
    const SECOND: i32 = 13;

    let [year, month, day, hour, minute, second] = crate::android::with_env(|env, _| {
        let calendar = env
            .call_static_method(
                "java/util/Calendar",
                "getInstance",
                "()Ljava/util/Calendar;",
                &[],
            )?
            .l()?;
        let mut fields = [0; 6];
        for (slot, field) in
            fields
                .iter_mut()
                .zip([YEAR, MONTH, DAY_OF_MONTH, HOUR_OF_DAY, MINUTE, SECOND])
        {
            *slot = env
                .call_method(&calendar, "get", "(I)I", &[JValue::Int(field)])?
                .i()?;
        }
        Ok(fields)
    })?;
    // Calendar months are zero-based.
    NaiveDate::from_ymd_opt(year, month as u32 + 1, day as u32)?.and_hms_opt(
        hour as u32,
        minute as u32,
        second as u32,
    )
}
