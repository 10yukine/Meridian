// Copyright (c) 2026 Ludovic Marie and the Horizon contributors.
// SPDX-License-Identifier: MIT
//! Controllers and mice, taken from each raw Android input event before winit's Android backend
//! makes touches of them or drops them (winit's input hook, `third_party/winit`). Adapted from
//! Starling's `crates/horizon-client/src/android.rs` (`mod input`) at 5432370d5736; the game reads
//! both through `horizon_client_render::android_runtime`.
use horizon_client_render::android_runtime::{self, MouseEvent};
use std::sync::Mutex;
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton};
use winit::platform::android::activity::{
    InputStatus,
    input::{Axis, InputEvent, KeyAction, KeyEvent, MotionAction, MotionEvent},
};

/// `InputDevice.SOURCE_GAMEPAD`, `SOURCE_JOYSTICK`, `SOURCE_MOUSE` and `SOURCE_MOUSE_RELATIVE`
/// (the captured pointer's) without their class bits.
const SOURCE_GAMEPAD: u32 = 0x400;
const SOURCE_JOYSTICK: u32 = 0x0100_0000;
const SOURCE_MOUSE: u32 = 0x2000;
const SOURCE_MOUSE_RELATIVE: u32 = 0x0002_0000;
/// The controller axes the game reads, with their Android `AXIS_*` codes.
const PAD_AXES: [(Axis, u32); 10] = [
    (Axis::X, 0),
    (Axis::Y, 1),
    (Axis::Z, 11),
    (Axis::Rz, 14),
    (Axis::HatX, 15),
    (Axis::HatY, 16),
    (Axis::Ltrigger, 17),
    (Axis::Rtrigger, 18),
    (Axis::Gas, 22),
    (Axis::Brake, 23),
];
/// `MotionEvent.BUTTON_PRIMARY`, `_SECONDARY` and `_TERTIARY`. Android makes the Back and Forward
/// keys of the back and forward buttons.
const BUTTONS: [(u32, MouseButton); 3] = [
    (1, MouseButton::Left),
    (2, MouseButton::Right),
    (4, MouseButton::Middle),
];

/// The mouse's buttons held and pointer position at its last event, and whether a press that
/// reported no button holds the primary one.
static MOUSE: Mutex<(u32, Option<(f32, f32)>, bool)> = Mutex::new((0, None, false));

/// The hook: the events taken are answered as handled; the others go on to winit (a keyboard's
/// keys, fingers).
pub(crate) fn hook(event: &InputEvent<'_>) -> Option<InputStatus> {
    match event {
        InputEvent::KeyEvent(key) => key_input(key),
        InputEvent::MotionEvent(motion) => motion_input(motion),
        _ => None,
    }
}

fn key_input(key: &KeyEvent<'_>) -> Option<InputStatus> {
    let source: u32 = key.source().into();
    if source & SOURCE_GAMEPAD == 0 {
        return None;
    }
    let state = match key.action() {
        KeyAction::Down => ElementState::Pressed,
        KeyAction::Up => ElementState::Released,
        _ => return None,
    };
    let button = u32::from(key.key_code());
    // Only a controller's own buttons: its Back and Home stay Android's.
    horizon_client_render::android_gamepad::button(button)?;
    android_runtime::gamepad_event(key.device_id(), &DeviceEvent::Button { button, state })
        .then_some(InputStatus::Handled)
}

fn motion_input(motion: &MotionEvent<'_>) -> Option<InputStatus> {
    let source: u32 = motion.source().into();
    if source & SOURCE_JOYSTICK != 0 {
        if motion.pointer_count() > 0 {
            let pointer = motion.pointer_at_index(0);
            for (axis, code) in PAD_AXES {
                let value = f64::from(pointer.axis_value(axis));
                android_runtime::gamepad_event(
                    motion.device_id(),
                    &DeviceEvent::Motion { axis: code, value },
                );
            }
        }
        return Some(InputStatus::Handled);
    }
    let relative = source & SOURCE_MOUSE_RELATIVE != 0;
    // Before the game reads the mouse, winit keeps it (as a finger).
    if (source & SOURCE_MOUSE == 0 && !relative) || !android_runtime::mouse_read() {
        return None;
    }
    if motion.pointer_count() == 0 {
        return Some(InputStatus::Handled);
    }
    // winit's only way to name a device it didn't report itself; the game tells input sources
    // apart by it, never looks the device up.
    let device = DeviceId::dummy();
    let pointer = motion.pointer_at_index(0);
    let mut mouse = MOUSE.lock().unwrap_or_else(|e| e.into_inner());
    let mut events = Vec::new();
    // Where the pointer moved from, for a mouse that reports no relative motion.
    let mut moved_by = (0.0, 0.0);
    if !relative && motion.action() != MotionAction::HoverExit {
        let at = (pointer.x(), pointer.y());
        if mouse.1 != Some(at) {
            if let Some(from) = mouse.1 {
                moved_by = (at.0 - from.0, at.1 - from.1);
            }
            mouse.1 = Some(at);
            events.push(MouseEvent::Moved(f64::from(at.0), f64::from(at.1)));
        }
    }
    // How far it went, the moves batched into this event included: captured, its X and Y; free,
    // its RELATIVE_X and RELATIVE_Y, which turn the view while the game holds the pointer.
    let (x, y) = if relative {
        (Axis::X, Axis::Y)
    } else {
        (Axis::RelativeX, Axis::RelativeY)
    };
    let (mut dx, mut dy) = (pointer.axis_value(x), pointer.axis_value(y));
    for earlier in pointer.history() {
        dx += earlier.axis_value(x);
        dy += earlier.axis_value(y);
    }
    // No relative axes (adb's `input mouse`, some mice and trackpads): the pointer's own move.
    if dx == 0.0 && dy == 0.0 && !relative {
        (dx, dy) = moved_by;
    }
    if dx != 0.0 || dy != 0.0 {
        events.push(MouseEvent::Motion(f64::from(dx), f64::from(dy)));
    }
    if motion.action() == MotionAction::Scroll {
        let right = pointer.axis_value(Axis::Hscroll);
        let up = pointer.axis_value(Axis::Vscroll);
        if right != 0.0 || up != 0.0 {
            // winit's wheel moves the content: right is negative.
            events.push(MouseEvent::Wheel(-right, up));
        }
    }
    let mut buttons = motion.button_state().0;
    // A mouse that reports no button with its press (adb's `input mouse`, some trackpads) presses
    // the primary one until it lifts. A real mouse names its button.
    match motion.action() {
        MotionAction::Down if buttons == 0 => mouse.2 = true,
        MotionAction::Up | MotionAction::Cancel => mouse.2 = false,
        _ => {}
    }
    if mouse.2 {
        buttons |= 1;
    }
    for (bit, button) in BUTTONS {
        if (buttons ^ mouse.0) & bit != 0 {
            events.push(MouseEvent::Button(button, buttons & bit != 0));
        }
    }
    for event in events {
        android_runtime::mouse_event(device, event);
    }
    mouse.0 = buttons;
    Some(InputStatus::Handled)
}
