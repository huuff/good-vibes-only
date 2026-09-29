//! JNI plumbing shared by the Android-only code paths.
//!
//! Two kinds of process reach Rust here: the app's UI (tao has set up
//! ndk-context with the activity) and a bare broadcast receiver woken by a
//! reminder alarm or a notification action, with no activity at all. So
//! nothing below touches ndk-context unless the UI is known to be up; the
//! receiver hands in its own context through the `Java_…` entry points,
//! and both paths end up sharing one application context.

use std::panic::AssertUnwindSafe;
use std::sync::OnceLock;

use jni::objects::{GlobalRef, JClass, JObject, JValue};
use jni::sys::{jboolean, jlong};
use jni::{JNIEnv, JavaVM};

struct App {
    vm: JavaVM,
    /// `Context.getApplicationContext()` — valid for the process lifetime.
    context: GlobalRef,
}

static APP: OnceLock<App> = OnceLock::new();

fn init(env: &mut JNIEnv, context: &JObject) -> Option<&'static App> {
    if let Some(app) = APP.get() {
        return Some(app);
    }
    let vm = env.get_java_vm().ok()?;
    let app_context = env
        .call_method(
            context,
            "getApplicationContext",
            "()Landroid/content/Context;",
            &[],
        )
        .ok()?
        .l()
        .ok()?;
    let context = env.new_global_ref(app_context).ok()?;
    // A concurrent init may have won; either value is the same context.
    let _ = APP.set(App { vm, context });
    APP.get()
}

fn app() -> Option<&'static App> {
    if let Some(app) = APP.get() {
        return Some(app);
    }
    // Not initialized by a receiver, so this is the UI process: tao has
    // published the activity through ndk-context.
    let ctx = ndk_context::android_context();
    let vm = unsafe { JavaVM::from_raw(ctx.vm().cast()) }.ok()?;
    let mut env = vm.attach_current_thread().ok()?;
    let activity = unsafe { JObject::from_raw(ctx.context().cast()) };
    init(&mut env, &activity)
}

/// Run `f` with a JNI env and the application context. Local references
/// die with a local frame (the UI thread never returns to Java, so they'd
/// otherwise pile up), and a thrown Java exception becomes `None`.
pub fn with_env<R>(
    f: impl for<'a, 'b> FnOnce(&mut JNIEnv<'a>, &JObject<'b>) -> jni::errors::Result<R>,
) -> Option<R> {
    let app = app()?;
    let mut env = app.vm.attach_current_thread().ok()?;
    let result = env.with_local_frame(16, |env| f(env, app.context.as_obj()));
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
    }
    result.ok()
}

/// Like [`with_env`], with the activity instead of the application
/// context. UI process only.
fn with_activity<R>(
    f: impl for<'a, 'b> FnOnce(&mut JNIEnv<'a>, &JObject<'b>) -> jni::errors::Result<R>,
) -> Option<R> {
    let activity = ndk_context::android_context().context();
    with_env(|env, _| {
        let activity = unsafe { JObject::from_raw(activity.cast()) };
        f(env, &activity)
    })
}

/// The Kotlin side (android/MainActivity.kt). Loaded through the app's
/// class loader: `FindClass` from a native thread only sees system classes.
fn helper<'a>(env: &mut JNIEnv<'a>, context: &JObject) -> jni::errors::Result<JClass<'a>> {
    let loader = env
        .call_method(context, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])?
        .l()?;
    let name = env.new_string("dev.dioxus.main.TallyReminders")?;
    let class = env
        .call_method(
            &loader,
            "loadClass",
            "(Ljava/lang/String;)Ljava/lang/Class;",
            &[JValue::Object(&name)],
        )?
        .l()?;
    Ok(class.into())
}

pub mod reminders {
    use chrono::{Datelike, NaiveDateTime, Timelike};
    use jni::objects::{JObject, JValue};

    use super::{helper, with_activity, with_env};
    use crate::i18n::Strings;

    const CTX: &str = "Landroid/content/Context;";
    const STR: &str = "Ljava/lang/String;";

    fn call(
        name: &str,
        sig: &str,
        args: impl for<'a> FnOnce(&mut jni::JNIEnv<'a>) -> Vec<Arg<'a>>,
    ) {
        with_env(|env, context| {
            let class = helper(env, context)?;
            let owned = args(env);
            let mut values = vec![JValue::Object(context)];
            values.extend(owned.iter().map(Arg::value));
            env.call_static_method(&class, name, sig, &values)?;
            Ok(())
        });
    }

    /// Arguments after the leading Context.
    enum Arg<'a> {
        Long(i64),
        Int(i32),
        Obj(JObject<'a>),
    }

    impl Arg<'_> {
        fn value(&self) -> JValue<'_, '_> {
            match self {
                Arg::Long(v) => JValue::Long(*v),
                Arg::Int(v) => JValue::Int(*v),
                Arg::Obj(o) => JValue::Object(o),
            }
        }
    }

    fn string<'a>(env: &mut jni::JNIEnv<'a>, s: &str) -> Arg<'a> {
        Arg::Obj(env.new_string(s).map(JObject::from).unwrap_or_default())
    }

    pub fn schedule(id: u64, at: NaiveDateTime) {
        call("schedule", &format!("({CTX}JIIIII)V"), |_| {
            vec![
                Arg::Long(id as i64),
                Arg::Int(at.year()),
                Arg::Int(at.month() as i32),
                Arg::Int(at.day() as i32),
                Arg::Int(at.hour() as i32),
                Arg::Int(at.minute() as i32),
            ]
        });
    }

    pub fn cancel(id: u64) {
        call("cancel", &format!("({CTX}J)V"), |_| {
            vec![Arg::Long(id as i64)]
        });
    }

    pub fn dismiss(id: u64) {
        call("dismiss", &format!("({CTX}J)V"), |_| {
            vec![Arg::Long(id as i64)]
        });
    }

    pub fn notify(id: u64, t: &Strings, title: &str, text: &str, later: bool) {
        call(
            "notify",
            &format!("({CTX}J{STR}{STR}{STR}{STR}{STR})V"),
            |env| {
                vec![
                    Arg::Long(id as i64),
                    string(env, t.notif_channel),
                    string(env, title),
                    string(env, text),
                    string(env, t.notif_done),
                    if later {
                        string(env, t.notif_later)
                    } else {
                        Arg::Obj(JObject::null())
                    },
                ]
            },
        );
    }

    pub fn confirm_done(id: u64, t: &Strings, name: &str) {
        let title = crate::i18n::fill(t.notif_done_title, &[&name]);
        call("confirmDone", &format!("({CTX}J{STR}{STR}{STR})V"), |env| {
            vec![
                Arg::Long(id as i64),
                string(env, t.notif_channel),
                string(env, &title),
                string(env, t.notif_done_text),
            ]
        });
    }

    pub fn allowed() -> bool {
        with_env(|env, context| {
            let class = helper(env, context)?;
            env.call_static_method(
                &class,
                "allowed",
                format!("({CTX})Z"),
                &[JValue::Object(context)],
            )?
            .z()
        })
        .unwrap_or(false)
    }

    pub fn request_permission() {
        with_activity(|env, activity| {
            let class = helper(env, activity)?;
            env.call_static_method(
                &class,
                "requestPermission",
                format!("({CTX})V"),
                &[JValue::Object(activity)],
            )?;
            Ok(())
        });
    }

    pub fn open_settings() {
        call("openSettings", &format!("({CTX})V"), |_| Vec::new());
    }
}

/// Receiver entry points share this: adopt the receiver's context, and
/// never let a panic unwind into the JVM.
fn entry(mut env: JNIEnv, context: JObject, f: impl FnOnce()) {
    let _ = std::panic::catch_unwind(AssertUnwindSafe(|| {
        if init(&mut env, &context).is_some() {
            f();
        }
    }));
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_dioxus_main_ReminderReceiver_nativeFire(
    env: JNIEnv,
    _this: JObject,
    context: JObject,
    id: jlong,
    snoozed: jboolean,
) {
    entry(env, context, || {
        crate::reminders::on_alarm(id as u64, snoozed != 0)
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_dioxus_main_ReminderReceiver_nativeDone(
    env: JNIEnv,
    _this: JObject,
    context: JObject,
    id: jlong,
) {
    entry(env, context, || crate::reminders::on_done(id as u64));
}

/// Boot, clock or time-zone change, app update: alarms were dropped or
/// are now at the wrong wall-clock time.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_dioxus_main_RescheduleReceiver_nativeSync(
    env: JNIEnv,
    _this: JObject,
    context: JObject,
) {
    entry(env, context, || {
        crate::reminders::sync(&crate::store::Data::load())
    });
}
