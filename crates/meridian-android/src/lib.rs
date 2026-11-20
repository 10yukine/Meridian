//! Meridian's Android entry (`libmeridian.so`). NativeActivity calls [`android_main`], which runs
//! the game with the app's private folder as its game folder and home: saves, options and logs,
//! and the prepared resources at `.local/horizon-client/<version>`, where horizon-prepare writes
//! them on a PC. android-activity sends stdout and stderr to logcat (tag `RustStdoutStderr`).
#![cfg(target_os = "android")]

use horizon_client::launcher_backend::{
    launch::{self, Launch},
    profile_dir,
};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
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

/// The game runs in this process: an exit before it returns is a crash (the crash handler and the
/// game's errors end the process with `std::process::exit`).
static GAME_RUNNING: AtomicBool = AtomicBool::new(false);

unsafe extern "C" {
    fn atexit(callback: extern "C" fn()) -> std::ffi::c_int;
}

/// Back to the launcher when the game ends the process itself (a crash); the launcher says so
/// (`launch::closed_unexpectedly`).
extern "C" fn back_to_launcher() {
    if GAME_RUNNING.load(Relaxed) {
        activity::restart();
    }
}

#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    eprintln!("Meridian: android_main");
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
        // The driver's compiled pipelines, kept for the next process (the vendored wgpu-hal's
        // disk cache): each switch between launcher and game skips compiling them again.
        std::env::set_var("HORIZON_VK_PIPELINE_CACHE", home.join("vk-pipeline-cache.bin"));
    }
    if horizon_client::ANDROID_APP.set(app).is_err() {
        // winit makes one event loop per process; a fresh process gets a fresh one.
        eprintln!("Meridian: activity started again in a running process; exiting for a fresh one");
        std::process::exit(0);
    }
    // Controllers and mice reach the game through this hook, not as winit's fake touches.
    winit::platform::android::set_input_hook(input::hook);
    horizon_client_render::android_runtime::set_pointer_capture(activity::capture_pointer);
    horizon_client_render::android_runtime::set_open_link(activity::open_link);
    horizon_client_render::android_runtime::set_pick_file(activity::pick_skin);
    horizon_client_render::android_runtime::set_ime_height(activity::ime_height);
    // The launcher and the game each run in a process of their own (winit makes one event loop
    // per process); `launch.json` says which this one is (docs/launcher-spec.md, "Hand-off").
    let launch = launch::take(&home);
    trace_startup(&home, if launch.is_some() { "game" } else { "launcher" });
    match launch {
        None => match horizon_client::game::mobile_launcher_host::run(&home) {
            Ok(true) => activity::restart(),
            Ok(false) => {}
            Err(error) => eprintln!("Meridian: the launcher stopped: {error}"),
        },
        Some(launch) => {
            if enter(&home, &launch) {
                let _ = launch::game_started(&home);
                GAME_RUNNING.store(true, Relaxed);
                // SAFETY: registers a plain function, which libc calls as the process exits.
                unsafe { atexit(back_to_launcher) };
                horizon_game::main();
                GAME_RUNNING.store(false, Relaxed);
                launch::game_ended(&home);
            }
            // Back to the launcher once the game ends.
            activity::restart();
        }
    }
    // Ending the process when the game ends keeps Android from reusing it for the next launch.
    std::process::exit(0);
}

/// A developer's switch: with `files/startup-trace` present, this start writes its timeline
/// (`HORIZON_STARTUP_TRACE`, Chrome trace events) to `files/startup-trace-<which>.json`.
fn trace_startup(home: &Path, which: &str) {
    if home.join("startup-trace").exists() {
        // SAFETY: no thread of ours reads the environment yet: nothing has started.
        unsafe {
            std::env::set_var(
                "HORIZON_STARTUP_TRACE",
                home.join(format!("startup-trace-{which}.json")),
            );
        }
    }
}

/// The game folder and account the launcher chose, as `--game-dir` and `--session-file` give
/// them on a computer: the profile's folder, the resources every profile shares, the account.
fn enter(home: &Path, launch: &Launch) -> bool {
    let dir = profile_dir(home, &launch.profile);
    if let Err(error) = std::fs::create_dir_all(&dir).and_then(|()| std::env::set_current_dir(&dir))
    {
        eprintln!("Meridian: cannot use profile {}: {error}", launch.profile);
        return false;
    }
    // A new profile plays the way Vanilla does (options, graphics, touch layout).
    if let Err(error) = launch::inherit_settings(home, &dir) {
        eprintln!("Meridian: profile {} keeps the default settings: {error}", launch.profile);
    }
    // SAFETY: no thread of ours reads the environment yet: the game has not started.
    unsafe {
        std::env::set_var("HORIZON_RESOURCES", launch::resources_dir(home));
        if launch.session {
            std::env::set_var("HORIZON_SESSION_FILE", launch::session_file(home));
        }
        if let Some(name) = &launch.name {
            std::env::set_var("HORIZON_PLAYER_NAME", name);
        }
    }
    true
}
