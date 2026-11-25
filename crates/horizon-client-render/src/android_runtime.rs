// Adapted from Starling (https://github.com/starling-client/starling, `android` branch at
// 150a414b09a1, the input part of `crates/horizon-client-render/src/android_runtime.rs`), written
// by Vasysik and the Starling contributors. Copyright (c) 2026 Ludovic Marie and the Horizon
// contributors, under the same MIT license as Horizon (LICENSE at the repository root).

//! The Android input runtime: which input device is in charge, the connected
//! controllers and the keypad profile, held once for the whole process. The
//! Android event backend, the options screen and the game all reach the same
//! state through these functions, from whichever thread delivers the event.
//!
//! Pure state over [`android_gamepad`](crate::android_gamepad) and
//! [`keypad`](crate::keypad): nothing here talks to the OS, so it builds and is
//! tested on every platform; only the Android build feeds it.
use crate::android_gamepad::{Devices, InputMode, Selection};
use crate::gamepad::PadState;
use crate::keypad::{Keypad, Settings};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex, MutexGuard, OnceLock, PoisonError};
use winit::event::{DeviceEvent, ElementState};
use winit::keyboard::KeyCode;

static SELECTION: LazyLock<Mutex<Selection>> = LazyLock::new(Mutex::default);
static GAMEPADS: LazyLock<Mutex<Devices>> = LazyLock::new(Mutex::default);
static KEYPAD: LazyLock<Mutex<Keypad>> = LazyLock::new(Mutex::default);
static KEYPAD_ENABLED: AtomicBool = AtomicBool::new(false);

/// A lock that a panic elsewhere never makes unusable: the state stays
/// consistent between calls, so input keeps working.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The input mode picked in the options.
pub fn input_mode() -> InputMode {
    lock(&SELECTION).mode()
}

/// The device in charge now (never `Auto`).
pub fn active_input() -> InputMode {
    lock(&SELECTION).active()
}

/// Pick an input mode. A change lets go of every held controller and keypad
/// input, so nothing stays pressed across the switch.
pub fn configure_input(mode: InputMode) {
    let changed = lock(&SELECTION).configure(mode);
    if changed {
        clear_gamepads();
        clear_keypad();
    }
}

/// A device of `kind` was used: in `Auto` it takes over. Returns whether it
/// may act (false when the player picked another mode). Leaving the
/// controller lets go of its held buttons.
pub fn note_input(kind: InputMode) -> bool {
    let (accepted, left_controller) = {
        let mut selection = lock(&SELECTION);
        let was = selection.active();
        let accepted = selection.note(kind);
        (
            accepted,
            accepted && was == InputMode::Controller && kind != InputMode::Controller,
        )
    };
    if left_controller {
        clear_gamepads();
    }
    accepted
}

/// A raw event of the controller with Android device id `id`: motion axes
/// and buttons by Android code. Returns whether the event was a controller's
/// (the caller then keeps it from other handlers); a meaningful one also
/// notes the controller as used. A removed device forgets every controller,
/// the platform then lists those still there ([`retain_gamepads`]).
pub fn gamepad_event(id: i32, event: &DeviceEvent) -> bool {
    let activity = {
        let mut devices = lock(&GAMEPADS);
        match *event {
            DeviceEvent::Motion { axis, value } => devices.axis(id, axis, value),
            DeviceEvent::Button { button, state } => {
                devices.key(id, button, state == ElementState::Pressed)
            }
            DeviceEvent::Removed => {
                devices.clear();
                false
            }
            _ => return false,
        }
    };
    if activity {
        note_input(InputMode::Controller);
    }
    true
}

/// Forget the controllers that are no longer connected.
pub fn retain_gamepads(ids: &[i32]) {
    lock(&GAMEPADS).retain(ids);
}

/// Let go of every controller's held input.
pub fn clear_gamepads() {
    lock(&GAMEPADS).clear();
}

/// The Android device id of the controller in charge (for its rumble), if a
/// controller is the active input.
pub fn active_gamepad_id() -> Option<i32> {
    if active_input() != InputMode::Controller {
        return None;
    }
    lock(&GAMEPADS).active_id()
}

/// The controller in charge as a pad, once per frame; nothing while another
/// device is the active input.
pub fn gamepad_state() -> PadState {
    if active_input() != InputMode::Controller {
        return PadState::default();
    }
    lock(&GAMEPADS).poll()
}

/// Use this keypad profile, on or off as it says.
pub fn configure_keypad_settings(settings: &Settings) {
    lock(&KEYPAD).configure(settings);
    KEYPAD_ENABLED.store(settings.enabled, Ordering::Relaxed);
}

/// Turn the keypad profile on or off, letting go of its held keys.
pub fn configure_keypad(enabled: bool) {
    KEYPAD_ENABLED.store(enabled, Ordering::Relaxed);
    clear_keypad();
}

pub fn keypad_enabled() -> bool {
    KEYPAD_ENABLED.load(Ordering::Relaxed)
}

/// A key event for the keypad profile. Returns true when the profile is on
/// and the key is one of its own (the caller keeps it from the game's
/// keyboard input).
pub fn keypad_key(key: KeyCode, down: bool) -> bool {
    keypad_enabled() && lock(&KEYPAD).key(key, down)
}

/// Let go of every key the keypad profile holds.
pub fn clear_keypad() {
    lock(&KEYPAD).clear();
}

/// The keypad profile's held actions as a pad, once per frame.
pub fn keypad_state() -> PadState {
    lock(&KEYPAD).poll()
}

/// The keypad profile and the controller in charge together, once per frame:
/// what the game's controller path reads on Android.
pub fn pad_state() -> PadState {
    keypad_state().merge(&gamepad_state())
}

/// Meridian: what a mouse did, taken from Android's raw input before winit makes touches of it
/// (the platform's input hook), for the game to replay as a computer's mouse.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MouseEvent {
    /// The pointer moved to this window position, in pixels.
    Moved(f64, f64),
    Button(winit::event::MouseButton, bool),
    /// The wheel, in lines: right and up positive (winit's `LineDelta`).
    Wheel(f32, f32),
    /// How far the mouse went, in pixels, right and down positive: the view turns by it.
    Motion(f64, f64),
}

/// The most mouse events kept for a game that isn't reading them (a save on quit).
const MOUSE_QUEUE: usize = 512;
static MOUSE: Mutex<Vec<(winit::event::DeviceId, MouseEvent)>> = Mutex::new(Vec::new());
/// The game reads the mouse (from its first loop iteration). Until then the hook leaves the
/// mouse to winit, so the loading screen gets it as a finger.
static MOUSE_READ: AtomicBool = AtomicBool::new(false);

/// Whether the game reads the mouse yet; until it does, the hook leaves the mouse to winit.
pub fn mouse_read() -> bool {
    MOUSE_READ.load(Ordering::Relaxed)
}

/// A mouse event from the platform's input hook, for the game's next loop iteration.
pub fn mouse_event(device: winit::event::DeviceId, event: MouseEvent) {
    let mut queue = lock(&MOUSE);
    if queue.len() < MOUSE_QUEUE {
        queue.push((device, event));
    }
}

/// The mouse's events since the last call; the hook takes the mouse from the first call on.
pub fn take_mouse_events() -> Vec<(winit::event::DeviceId, MouseEvent)> {
    MOUSE_READ.store(true, Ordering::Relaxed);
    std::mem::take(&mut *lock(&MOUSE))
}

/// Meridian: how the activity captures the mouse (`MeridianActivity.capturePointer`), set by the
/// platform: captured, the pointer hides and its moves come to the hook as relative motion.
static POINTER_CAPTURE: OnceLock<fn(bool)> = OnceLock::new();

/// Sets the platform's pointer capture; the first one set stays.
pub fn set_pointer_capture(capture: fn(bool)) {
    let _ = POINTER_CAPTURE.set(capture);
}

/// Captures the mouse for the game's mouse look, or lets it go. Fingers are not affected.
pub fn capture_pointer(capture: bool) {
    if let Some(platform) = POINTER_CAPTURE.get() {
        platform(capture);
    }
}

/// Meridian: how the platform opens a web page (`MeridianActivity.openLink`).
static OPEN_LINK: OnceLock<fn(&str) -> bool> = OnceLock::new();

/// Sets the platform's browser; the first one set stays.
pub fn set_open_link(open: fn(&str) -> bool) {
    let _ = OPEN_LINK.set(open);
}

/// Opens an https:// page in the browser (the Microsoft sign-in); false where none can.
pub fn open_link(url: &str) -> bool {
    OPEN_LINK.get().is_some_and(|platform| platform(url))
}

/// Meridian: how the platform opens its file picker for a skin (`MeridianActivity.pickSkin`).
static PICK_FILE: OnceLock<fn() -> bool> = OnceLock::new();

/// Sets the platform's file picker; the first one set stays.
pub fn set_pick_file(pick: fn() -> bool) {
    let _ = PICK_FILE.set(pick);
}

/// Opens the platform's picker for a PNG skin, which leaves the chosen file in the launcher's
/// folder (`picked-skin.png`); false where none can open.
pub fn pick_file() -> bool {
    PICK_FILE.get().is_some_and(|platform| platform())
}

static PICK_IMPORT: OnceLock<fn(&str, &str, u64) -> bool> = OnceLock::new();

/// Sets the platform's general import picker; the first one set stays.
pub fn set_pick_import(pick: fn(&str, &str, u64) -> bool) {
    let _ = PICK_IMPORT.set(pick);
}

/// Opens the platform's SAF picker for `mime`, writing the chosen file to `files/<dest_name>`
/// (and its display name to `files/<dest_name>.name`), or `files/<dest_name>.cancelled`.
pub fn pick_import(mime: &str, dest_name: &str, max_bytes: u64) -> bool {
    PICK_IMPORT
        .get()
        .is_some_and(|platform| platform(mime, dest_name, max_bytes))
}

static SAVE_SCREENSHOT: OnceLock<fn(&str) -> bool> = OnceLock::new();

/// Sets the platform's gallery screenshot saver; the first one set stays.
pub fn set_save_screenshot(save: fn(&str) -> bool) {
    let _ = SAVE_SCREENSHOT.set(save);
}

/// Copies a PNG screenshot into the system Pictures/Meridian gallery (MediaStore on Android 10+).
pub fn save_screenshot(path: &str) -> bool {
    SAVE_SCREENSHOT
        .get()
        .is_some_and(|platform| platform(path))
}

/// Meridian: how the platform puts text on the clipboard (`MeridianActivity.copyText`).
static COPY_TEXT: OnceLock<fn(&str)> = OnceLock::new();

/// Sets the platform's clipboard; the first one set stays.
pub fn set_copy_text(copy: fn(&str)) {
    let _ = COPY_TEXT.set(copy);
}

/// Puts `text` on the clipboard; false where there is none.
pub fn copy_text(text: &str) -> bool {
    COPY_TEXT.get().map(|platform| platform(text)).is_some()
}

/// Meridian: how the platform reads the on-screen keyboard's height (`MeridianActivity.imeHeight`).
static IME_HEIGHT: OnceLock<fn() -> u32> = OnceLock::new();

/// Sets the platform's keyboard height; the first one set stays.
pub fn set_ime_height(height: fn() -> u32) {
    let _ = IME_HEIGHT.set(height);
}

/// How far the on-screen keyboard covers the window, in pixels: 0 when it's away. The window
/// doesn't shrink for it (fullscreen), so text fields lift above it themselves.
pub fn ime_height() -> u32 {
    IME_HEIGHT.get().map_or(0, |platform| platform())
}

/// Meridian: how the platform reads its display cutout (`MeridianActivity.safeInsets`).
static SAFE_INSETS: OnceLock<fn() -> [u32; 4]> = OnceLock::new();

/// Sets the platform's cutout insets; the first one set stays.
pub fn set_safe_insets(insets: fn() -> [u32; 4]) {
    let _ = SAFE_INSETS.set(insets);
}

/// The display cutout's (a notch's) insets as left, top, right, bottom, in window pixels: zero
/// without one. The window reaches under it, so the launcher keeps its controls clear.
pub fn safe_insets() -> [u32; 4] {
    SAFE_INSETS.get().map_or([0; 4], |platform| platform())
}

/// Meridian: how the platform shares a file (`MeridianActivity.shareFile`).
static SHARE_FILE: OnceLock<fn(&str) -> bool> = OnceLock::new();

/// Sets the platform's share sheet; the first one set stays.
pub fn set_share_file(share: fn(&str) -> bool) {
    let _ = SHARE_FILE.set(share);
}

/// Offers the file at `path` (a diagnostic ZIP) to the platform's share sheet; false where none can.
pub fn share_file(path: &str) -> bool {
    SHARE_FILE.get().is_some_and(|platform| platform(path))
}

/// Meridian: how the platform buzzes the phone (`MeridianActivity.vibrate`).
static VIBRATE: OnceLock<fn(&[(u32, u8)])> = OnceLock::new();

/// Sets the platform's vibration motor; the first one set stays.
pub fn set_vibrate(vibrate: fn(&[(u32, u8)])) {
    let _ = VIBRATE.set(vibrate);
}

/// Buzzes the phone through `steps` of (milliseconds, strength 1..=255); nothing without a motor.
pub fn vibrate(steps: &[(u32, u8)]) {
    if let Some(platform) = VIBRATE.get() {
        platform(steps);
    }
}

/// Meridian: how the platform ticks for a pressed control (`MeridianActivity.tick`).
static TICK: OnceLock<fn()> = OnceLock::new();

/// Sets the platform's key-press tick; the first one set stays.
pub fn set_tick(tick: fn()) {
    let _ = TICK.set(tick);
}

/// The phone's own key-press feedback, which follows its touch-feedback setting.
pub fn tick() {
    if let Some(platform) = TICK.get() {
        platform();
    }
}

/// Meridian: how the platform holds the Wi-Fi multicast lock (`MeridianActivity.multicast`).
static MULTICAST: OnceLock<fn(bool)> = OnceLock::new();

/// Sets the platform's multicast lock; the first one set stays.
pub fn set_multicast(multicast: fn(bool)) {
    let _ = MULTICAST.set(multicast);
}

/// Takes (`true`) or gives back (`false`) the multicast lock; a no-op where there is none. The
/// lock counts, so every `true` needs its own `false`.
pub fn multicast(on: bool) {
    if let Some(platform) = MULTICAST.get() {
        platform(on);
    }
}

/// Meridian: how the platform listens to the gyroscope (`MeridianActivity.gyroscope`, `takeGyro`).
static GYROSCOPE: OnceLock<fn(bool) -> bool> = OnceLock::new();
static TAKE_GYRO: OnceLock<fn() -> Option<[f32; 3]>> = OnceLock::new();

/// Sets the platform's gyroscope; the first ones set stay.
pub fn set_gyroscope(gyroscope: fn(bool) -> bool, take: fn() -> Option<[f32; 3]>) {
    let _ = GYROSCOPE.set(gyroscope);
    let _ = TAKE_GYRO.set(take);
}

/// Starts or stops listening to the gyroscope; false when the phone has none.
pub fn gyroscope(on: bool) -> bool {
    GYROSCOPE.get().is_some_and(|platform| platform(on))
}

/// What the gyroscope turned since the last call (radians about the device's x and y) and the
/// display's rotation (`Surface.ROTATION_*`), for `touch::gyro::gyro_turn`.
pub fn take_gyro() -> Option<[f32; 3]> {
    TAKE_GYRO.get().and_then(|platform| platform())
}

/// Meridian: how the platform reports why the app's earlier processes ended
/// (`MeridianActivity.exitReasons`).
static EXIT_REASONS: OnceLock<fn() -> String> = OnceLock::new();

/// Sets the platform's exit history; the first one set stays.
pub fn set_exit_reasons(reasons: fn() -> String) {
    let _ = EXIT_REASONS.set(reasons);
}

/// Why the app's earlier processes ended, one per line; empty where the platform keeps no record.
pub fn exit_reasons() -> String {
    EXIT_REASONS.get().map_or_else(String::new, |platform| platform())
}

/// Meridian: ahead-of-time wasm mod cache hooks (`horizon_mod_wasm`), set by `meridian-android`
/// so the launcher warms `.horizon-cache/wasm` before handing off to the game.
static MODS_NEED_COMPILE: OnceLock<fn(&std::path::Path) -> bool> = OnceLock::new();
static PRECOMPILE_MODS: OnceLock<fn(&std::path::Path)> = OnceLock::new();

/// Sets the platform's wasm mod cache compiler hooks; the first ones set stay.
pub fn set_mod_compiler(needs: fn(&std::path::Path) -> bool, compile: fn(&std::path::Path)) {
    let _ = MODS_NEED_COMPILE.set(needs);
    let _ = PRECOMPILE_MODS.set(compile);
}

/// Whether `game_dir` has enabled wasm mods whose compiled cache is missing.
pub fn mods_need_compile(game_dir: &std::path::Path) -> bool {
    MODS_NEED_COMPILE
        .get()
        .is_some_and(|platform| platform(game_dir))
}

/// Precompiles `game_dir`'s enabled wasm mods into its `.horizon-cache/wasm`.
pub fn precompile_mods(game_dir: &std::path::Path) {
    if let Some(platform) = PRECOMPILE_MODS.get() {
        platform(game_dir);
    }
}

#[cfg(test)]
mod tests;
