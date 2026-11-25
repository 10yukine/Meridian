// Copyright (c) 2026 Ludovic Marie and the Horizon contributors.
// SPDX-License-Identifier: MIT
//! Calls into `MeridianActivity` (`packaging/android/java`), for what the NDK can't do. Adapted
//! from Starling's `with_activity` and `capture_pointer` (`crates/horizon-client/src/android.rs`)
//! at 5432370d5736.
use jni::objects::{JObject, JString, JValue};

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

/// `MeridianActivity.pickImport`: the system's file picker for an importable file; false where none opens.
pub(crate) fn pick_import(mime: &str, dest_name: &str, max_bytes: u64) -> bool {
    with_activity(|env, activity| {
        let mime = env.new_string(mime)?;
        let dest = env.new_string(dest_name)?;
        env.call_method(
            activity,
            "pickImport",
            "(Ljava/lang/String;Ljava/lang/String;J)Z",
            &[
                JValue::Object(&mime),
                JValue::Object(&dest),
                JValue::Long(max_bytes as i64),
            ],
        )?
        .z()
    })
    .unwrap_or(false)
}

/// `MeridianActivity.saveScreenshot`: copies a saved PNG screenshot into Android's Pictures/Meridian gallery.
pub(crate) fn save_screenshot(path: &str) -> bool {
    with_activity(|env, activity| {
        let path = env.new_string(path)?;
        env.call_method(
            activity,
            "saveScreenshot",
            "(Ljava/lang/String;)Z",
            &[JValue::Object(&path)],
        )?
        .z()
    })
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

/// `MeridianActivity.safeInsets`: the display cutout's insets (left, top, right, bottom), pixels.
pub(crate) fn safe_insets() -> [u32; 4] {
    with_activity(|env, activity| {
        let array = env.call_method(activity, "safeInsets", "()[I", &[])?.l()?;
        let mut insets = [0; 4];
        env.get_int_array_region(&jni::objects::JIntArray::from(array), 0, &mut insets)?;
        Ok(insets)
    })
    .map_or([0; 4], |insets| insets.map(|pixels| pixels.max(0) as u32))
}

/// `MeridianActivity.exitReasons`: why the app's earlier processes ended, for a diagnostic report.
pub(crate) fn exit_reasons() -> String {
    with_activity(|env, activity| {
        let text = env.call_method(activity, "exitReasons", "()Ljava/lang/String;", &[])?.l()?;
        if text.is_null() {
            return Ok(String::new());
        }
        Ok(env.get_string(&JString::from(text))?.into())
    })
    .unwrap_or_default()
}

/// `MeridianActivity.shareFile`: the share sheet for the diagnostic ZIP at `path`; false where the
/// activity won't serve it.
pub(crate) fn share_file(path: &str) -> bool {
    with_activity(|env, activity| {
        let path = env.new_string(path)?;
        env.call_method(activity, "shareFile", "(Ljava/lang/String;)Z", &[JValue::Object(&path)])?
            .z()
    })
    .unwrap_or(false)
}

/// `MeridianActivity.vibrate`: the phone's motor through `steps` of (milliseconds, strength).
pub(crate) fn vibrate(steps: &[(u32, u8)]) {
    // The thread stays attached, so the two arrays go with a local frame of their own.
    with_activity(|env, activity| {
        let timings: Vec<i64> = steps.iter().map(|s| i64::from(s.0)).collect();
        let amplitudes: Vec<i32> = steps.iter().map(|s| i32::from(s.1)).collect();
        env.with_local_frame(4, |env| {
            let timings_array = env.new_long_array(timings.len() as i32)?;
            env.set_long_array_region(&timings_array, 0, &timings)?;
            let amplitudes_array = env.new_int_array(amplitudes.len() as i32)?;
            env.set_int_array_region(&amplitudes_array, 0, &amplitudes)?;
            env.call_method(
                activity,
                "vibrate",
                "([J[I)V",
                &[
                    JValue::Object(&timings_array),
                    JValue::Object(&amplitudes_array),
                ],
            )
            .map(drop)
        })
    });
}

/// `MeridianActivity.tick`: the phone's key-press haptic.
pub(crate) fn tick() {
    with_activity(|env, activity| env.call_method(activity, "tick", "()V", &[]).map(drop));
}

/// `MeridianActivity.multicast`: takes or gives back the Wi-Fi multicast lock (LAN discovery).
pub(crate) fn multicast(on: bool) {
    with_activity(|env, activity| {
        env.call_method(activity, "multicast", "(Z)V", &[JValue::Bool(u8::from(on))]).map(drop)
    });
}

/// `MeridianActivity.gyroscope`: starts or stops the gyroscope; false when the phone has none.
pub(crate) fn gyroscope(on: bool) -> bool {
    with_activity(|env, activity| {
        env.call_method(activity, "gyroscope", "(Z)Z", &[JValue::Bool(u8::from(on))])?
            .z()
    })
    .unwrap_or(false)
}

/// `MeridianActivity.gyroscope` and `takeGyro` are adapted from Starling's `android.rs` calls of
/// the same names. `takeGyro`: what the gyroscope turned since the last call (radians about the
/// device's x and y) and the display's rotation. Asked every frame, so its array goes with a
/// local frame of its own.
pub(crate) fn take_gyro() -> Option<[f32; 3]> {
    with_activity(|env, activity| {
        env.with_local_frame(4, |env| {
            let turned = env.call_method(activity, "takeGyro", "()[F", &[])?.l()?;
            let mut out = [0.0f32; 3];
            env.get_float_array_region(&jni::objects::JFloatArray::from(turned), 0, &mut out)?;
            Ok(out)
        })
    })
}

/// `MeridianActivity.restart`: the app starts again in a fresh process once this one ends.
pub(crate) fn restart() {
    with_activity(|env, activity| env.call_method(activity, "restart", "()V", &[]).map(drop));
}
