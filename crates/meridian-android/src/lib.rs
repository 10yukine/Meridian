//! Meridian's Android entry (`libmeridian.so`). NativeActivity calls [`android_main`], which runs
//! the game with the app's private folder as its game folder and home: saves, options and logs,
//! and the prepared resources at `.local/horizon-client/<version>`, where horizon-prepare writes
//! them on a PC. android-activity sends stdout and stderr to logcat (tag `RustStdoutStderr`).
#![cfg(target_os = "android")]

use winit::platform::android::activity::AndroidApp;

mod activity;
mod input;

/// The guest stack Horizon's main thread needs (`horizon_guest_stack`), as `RUST_MIN_STACK` reads it.
const MAIN_STACK: &str = "142606336";
const _: () = assert!(
    horizon_guest_stack::GUEST_STACK_BYTES + horizon_guest_stack::HOST_HEADROOM == 142_606_336
);

/// Runs when Android loads the library, before NativeActivity starts the thread that calls
/// [`android_main`]: android-activity spawns it with std's default stack, so the default becomes
/// the guest stack. The game must run on that thread (its looper is the app's).
// ponytail: every std thread spawned without a stack size now reserves 136 MiB of address space
// (not memory); give android_main its own sized thread if android-activity ever allows it.
#[used]
#[unsafe(link_section = ".init_array")]
static MAIN_STACK_AT_LOAD: extern "C" fn() = {
    extern "C" fn set_main_stack() {
        // SAFETY: runs while the library loads, before any thread of this library exists.
        unsafe { std::env::set_var("RUST_MIN_STACK", MAIN_STACK) };
    }
    set_main_stack
};

#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    let Some(home) = app.internal_data_path() else {
        eprintln!("Meridian: Android gave the app no internal data folder");
        return;
    };
    let tmp = home.join("tmp");
    if let Err(error) = std::fs::create_dir_all(&tmp).and_then(|()| std::env::set_current_dir(&home)) {
        eprintln!("Meridian: cannot use {}: {error}", home.display());
        return;
    }
    // SAFETY: no thread of ours reads the environment yet: the game has not started.
    // Android's /data/local/tmp is not the app's to write.
    unsafe {
        std::env::set_var("HORIZON_HOME", &home);
        std::env::set_var("TMPDIR", &tmp);
    }
    if horizon_client::ANDROID_APP.set(app).is_err() {
        // winit makes one event loop per process; a fresh process gets a fresh one.
        eprintln!("Meridian: activity started again in a running process; exiting for a fresh one");
        std::process::exit(0);
    }
    // Controllers and mice reach the game through this hook, not as winit's fake touches.
    winit::platform::android::set_input_hook(input::hook);
    horizon_client_render::android_runtime::set_pointer_capture(activity::capture_pointer);
    horizon_game::main();
    // Ending the process when the game ends keeps Android from reusing it for the next launch.
    std::process::exit(0);
}
