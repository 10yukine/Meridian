// Copyright (c) 2026 Ludovic Marie and the Horizon contributors.
// SPDX-License-Identifier: MIT
//! Calls into `MeridianActivity` (`packaging/android/java`), for what the NDK can't do. Adapted
//! from Starling's `with_activity` and `capture_pointer` (`crates/horizon-client/src/android.rs`)
//! at 5432370d5736.
use jni::objects::{JObject, JValue};

/// Runs `f` with the activity on the calling thread, which stays attached to the VM. None before
/// the game starts, or when the call threw (the exception is logged and cleared).
fn with_activity<R>(
    f: impl FnOnce(&mut jni::JNIEnv, &JObject) -> jni::errors::Result<R>,
) -> Option<R> {
    let app = horizon_client::ANDROID_APP.get()?;
    // SAFETY: the VM and the activity are NativeActivity's, alive as long as the process; the
    // activity is a global reference, which this neither deletes nor keeps.
    let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr().cast()) }.ok()?;
    let mut env = vm.attach_current_thread_permanently().ok()?;
    let activity = unsafe { JObject::from_raw(app.activity_as_ptr().cast()) };
    let result = f(&mut env, &activity);
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
    }
    result.ok()
}

/// `MeridianActivity.capturePointer`: captured, the mouse's pointer hides and its moves come as
/// relative motion (`input.rs`).
pub(crate) fn capture_pointer(capture: bool) {
    with_activity(|env, activity| {
        env.call_method(activity, "capturePointer", "(Z)V", &[JValue::Bool(u8::from(capture))])
            .map(drop)
    });
}

/// `MeridianActivity.openLink`: the browser on an https:// page; false where none can.
pub(crate) fn open_link(url: &str) -> bool {
    with_activity(|env, activity| {
        let url = env.new_string(url)?;
        env.call_method(activity, "openLink", "(Ljava/lang/String;)Z", &[JValue::Object(&url)])?
            .z()
    })
    .unwrap_or(false)
}

/// `MeridianActivity.pickSkin`: the system's file picker for a PNG skin; false where none opens.
pub(crate) fn pick_skin() -> bool {
    with_activity(|env, activity| env.call_method(activity, "pickSkin", "()Z", &[])?.z())
        .unwrap_or(false)
}

/// `MeridianActivity.copyText`: `text` on the clipboard.
pub(crate) fn copy_text(text: &str) {
    with_activity(|env, activity| {
        let text = env.new_string(text)?;
        env.call_method(
            activity,
            "copyText",
            "(Ljava/lang/String;)V",
            &[JValue::Object(&text)],
        )
        .map(drop)
    });
}

/// `MeridianActivity.imeHeight`: how far the on-screen keyboard covers the window, in pixels.
pub(crate) fn ime_height() -> u32 {
    with_activity(|env, activity| env.call_method(activity, "imeHeight", "()I", &[])?.i())
        .map_or(0, |pixels| pixels.max(0) as u32)
}

/// `MeridianActivity.restart`: the app starts again in a fresh process once this one ends.
pub(crate) fn restart() {
    with_activity(|env, activity| env.call_method(activity, "restart", "()V", &[]).map(drop));
}
