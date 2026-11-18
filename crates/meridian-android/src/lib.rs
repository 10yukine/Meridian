//! Meridian's Android entry (`libmeridian.so`). NativeActivity calls [`android_main`], which runs
//! the game with the app's private folder as its game folder and home: saves, options and logs,
//! and the prepared resources at `.local/horizon-client/<version>`, where horizon-prepare writes
//! them on a PC.
#![cfg(target_os = "android")]

use std::io::BufRead;
use winit::platform::android::activity::AndroidApp;

#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    stdio_to_logcat();
    let Some(home) = app.internal_data_path() else {
        eprintln!("Meridian: Android gave the app no internal data folder");
        return;
    };
    let tmp = home.join("tmp");
    if let Err(error) = std::fs::create_dir_all(&tmp).and_then(|()| std::env::set_current_dir(&home)) {
        eprintln!("Meridian: cannot use {}: {error}", home.display());
        return;
    }
    // SAFETY: no other thread reads the environment: the game has not started, and the logcat
    // thread only reads its pipe. Android's /data/local/tmp is not the app's to write.
    unsafe {
        std::env::set_var("HORIZON_HOME", &home);
        std::env::set_var("TMPDIR", &tmp);
    }
    if horizon_client::ANDROID_APP.set(app).is_err() {
        // winit makes one event loop per process; a fresh process gets a fresh one.
        eprintln!("Meridian: activity started again in a running process; exiting for a fresh one");
        std::process::exit(0);
    }
    match horizon_guest_stack::builder()
        .name("game".into())
        .spawn(horizon_game::main)
    {
        Ok(game) => {
            let _ = game.join();
        }
        Err(error) => eprintln!("Meridian: cannot start the game thread: {error}"),
    }
    // Ending the process when the game ends keeps Android from reusing it for the next launch.
    std::process::exit(0);
}

/// Android discards a native app's stdout and stderr: send both to logcat (tag `Meridian`), line
/// by line, so the game's messages and panics show up in `adb logcat -s Meridian`.
fn stdio_to_logcat() {
    let mut fds = [0; 2];
    // SAFETY: fds has room for the two descriptors pipe writes.
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        return;
    }
    // SAFETY: both descriptors are open; stdout and stderr now write into the pipe.
    unsafe {
        libc::dup2(fds[1], libc::STDOUT_FILENO);
        libc::dup2(fds[1], libc::STDERR_FILENO);
        libc::close(fds[1]);
    }
    // SAFETY: fds[0] is the pipe's open read end, owned by this File from here on.
    let read = unsafe { <std::fs::File as std::os::fd::FromRawFd>::from_raw_fd(fds[0]) };
    let _ = std::thread::Builder::new()
        .name("logcat".into())
        .spawn(move || {
            let mut reader = std::io::BufReader::new(read);
            let mut line = Vec::new();
            // Lossy, so a line that isn't UTF-8 can't stop the reader and fill the pipe.
            while reader.read_until(b'\n', &mut line).is_ok_and(|read| read > 0) {
                let text = String::from_utf8_lossy(&line).trim_end().replace('\0', "");
                if let Ok(text) = std::ffi::CString::new(text) {
                    // SAFETY: tag and text are NUL-terminated and live through the call.
                    unsafe { __android_log_write(ANDROID_LOG_INFO, c"Meridian".as_ptr(), text.as_ptr()) };
                }
                line.clear();
            }
        });
}

const ANDROID_LOG_INFO: libc::c_int = 4;

#[link(name = "log")]
unsafe extern "C" {
    fn __android_log_write(
        priority: libc::c_int,
        tag: *const libc::c_char,
        text: *const libc::c_char,
    ) -> libc::c_int;
}
