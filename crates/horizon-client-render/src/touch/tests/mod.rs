// Adapted from Starling's touch.rs tests (see the header of touch.rs).
use super::*;

mod gestures;

fn role(t: &TouchControls, id: u64) -> Role {
    t.fingers[&id].0
}

#[test]
fn factory_swap_stays_in_top_row_and_moved_swap_stays_put() {
    for (w, h) in [(2560, 1600), (2400, 1080), (1080, 2400), (480, 854)] {
        for left_handed in [false, true] {
            for mode in Mode::ALL {
                let mut t = TouchControls::default();
                t.viewport(w, h);
                t.settings.left_handed = left_handed;
                t.settings.mode = mode;
                assert!((t.placement(Control::Offhand).center[1] - 0.055).abs() < 0.0001);
                let swap = t.placement(Control::Offhand).rect(t.aspect());
                for control in [
                    Control::Debug,
                    Control::Perspective,
                    Control::Chat,
                    Control::Pause,
                    Control::Settings,
                    Control::Inventory,
                    Control::Sprint,
                    Control::Sneak,
                    Control::Jump,
                ] {
                    let other = t.placement(control).rect(t.aspect());
                    let overlaps = swap[0] < other[0] + other[2]
                        && other[0] < swap[0] + swap[2]
                        && swap[1] < other[1] + other[3]
                        && other[1] < swap[1] + swap[3];
                    assert!(!overlaps, "{w}x{h}, {left_handed}, {mode:?}: {control:?}");
                }
                t.settings.layouts[mode.index()][Control::Offhand as usize].center = [0.55, 0.43];
                let moved = t.placement(Control::Offhand).center;
                assert!((moved[0] - if left_handed { 0.45 } else { 0.55 }).abs() < 0.0001);
                assert_eq!(moved[1], 0.43);
            }
        }
    }
}

#[test]
fn a_pressed_control_clicks_once_and_the_world_does_not() {
    let mut t = TouchControls::default();
    assert!(!t.take_clicked());
    t.update(1, t.placement(Control::Jump).center, true, false);
    assert!(t.take_clicked());
    assert!(!t.take_clicked());
    t.update(1, t.placement(Control::Jump).center, false, true);
    assert!(!t.take_clicked(), "a lift is no click");
    t.clear();
    t.update(2, [0.5, 0.3], true, false);
    assert!(!t.take_clicked(), "a touch on the world is no button");
}

#[test]
fn swap_hands_sends_one_press_and_shield_holds_until_release() {
    let mut t = TouchControls::default();
    assert!(t.visible(Control::Offhand));
    t.settings.offhand_button = false;
    assert!(!t.visible(Control::Offhand));
    t.settings.offhand_button = true;
    assert!(t.visible(Control::Offhand));
    t.update(1, t.placement(Control::Offhand).center, true, false);
    assert!(t.take_clicked());
    assert!(!t.take_clicked(), "once per press");
    assert_eq!(
        t.key_events(),
        [(SWAP_HANDS.into(), true), (SWAP_HANDS.into(), false)]
    );
    t.clear();
    t.settings.mode = Mode::Tap;
    t.shield_available = true;
    t.update(2, t.placement(Control::Shield).center, true, false);
    assert!(t.shield_held());
    // Sliding off the button keeps the shield up.
    t.update(2, [0.5, 0.4], false, false);
    assert!(t.shield_held());
    assert!(t.poll_state().triggers[0] > 0.0);
    t.update(2, [0.5, 0.4], false, true);
    assert!(!t.shield_held());
    assert_eq!(t.poll_state().triggers[0], 0.0);
}

#[test]
fn shield_uses_the_use_button_in_crosshair_layouts() {
    let mut t = TouchControls {
        shield_available: true,
        ..TouchControls::default()
    };
    assert!(!t.visible(Control::Shield));
    let p = t.placement(Control::Use).center;
    t.update(1, p, true, false);
    assert!(t.poll_state().triggers[0] > 0.0);
    t.update(1, p, false, true);
    assert_eq!(t.poll_state().triggers[0], 0.0);
    for mode in [Mode::Tap, Mode::Dpad] {
        t.clear();
        t.settings.mode = mode;
        assert!(t.visible(Control::Shield));
        let p = t.placement(Control::Shield).center;
        t.update(2, p, true, false);
        assert!(t.shield_held() && t.poll_state().triggers[0] > 0.0);
        t.update(2, p, false, true);
        assert!(!t.shield_held());
        t.force_crosshair = true;
        assert!(!t.visible(Control::Shield));
        t.force_crosshair = false;
        t.settings.shield_button = false;
        assert!(!t.visible(Control::Shield));
        t.settings.shield_button = true;
    }
    // Hiding the use button brings the shield button back.
    t.settings.mode = Mode::Crosshair;
    t.settings.button_visibility[Control::Use as usize] = layout::HIDDEN;
    assert!(t.visible(Control::Shield));
    t.shield_available = false;
    assert!(!t.visible(Control::Shield));
}

#[test]
fn sprint_toggle_survives_release_and_clears_with_focus() {
    let mut t = TouchControls::default();
    t.settings.sprint_toggle = true;
    let point = t.placement(Control::Sprint).center;
    t.update(1, point, true, false);
    t.update(1, point, false, true);
    assert!(t.poll_state().held(Button::LeftStick));
    t.update(2, point, true, false);
    t.update(2, point, false, true);
    assert!(!t.poll_state().held(Button::LeftStick));
    t.update(3, point, true, false);
    t.clear();
    assert!(!t.poll_state().held(Button::LeftStick));
    t.settings.sprint_toggle = false;
    t.update(4, point, true, false);
    assert!(t.poll_state().held(Button::LeftStick));
    t.update(4, point, false, true);
    assert!(!t.poll_state().held(Button::LeftStick));
}

#[test]
fn button_scale_changes_hit_areas_not_saved_layouts() {
    let mut t = TouchControls::default();
    t.viewport(1920, 1080);
    let before = t.placement(Control::Jump);
    let stored = t.settings.placement(Control::Jump).size;
    t.settings.button_scale = 1.5;
    assert!((t.placement(Control::Jump).size - before.size * 1.5).abs() < 0.00001);
    assert_eq!(t.settings.placement(Control::Jump).size, stored);
}

#[test]
fn big_controls_stay_on_narrow_and_wide_surfaces() {
    let mut t = TouchControls::default();
    for (w, h) in [(400, 1000), (1920, 1080)] {
        t.viewport(w, h);
        t.settings.layouts[0][Control::Jump as usize].center = [0.99, 0.99];
        t.settings.layouts[0][Control::Jump as usize].size = 0.45;
        t.settings.button_scale = 2.0;
        let r = t.placement(Control::Jump).rect(t.aspect());
        assert!(r[0] >= 0.0 && r[1] >= 0.0 && r[0] + r[2] <= 1.0001 && r[1] + r[3] <= 1.0001);
    }
}

#[test]
fn portrait_spreads_the_factory_top_row_but_not_moved_buttons() {
    let mut t = TouchControls::default();
    t.viewport(480, 854);
    let top = [
        Control::Debug,
        Control::Perspective,
        Control::Chat,
        Control::Pause,
        Control::Settings,
        Control::Offhand,
    ];
    let rects: Vec<_> = top
        .iter()
        .map(|c| t.placement(*c).rect(t.aspect()))
        .collect();
    for pair in rects.windows(2) {
        assert!(pair[0][0] + pair[0][2] < pair[1][0]);
    }
    t.settings.layouts[0][Control::Chat as usize].center = [0.25, 0.2];
    assert_eq!(t.placement(Control::Chat).center, [0.25, 0.2]);
}

#[test]
fn shortcut_only_overlay_keeps_menus_without_movement() {
    let mut t = TouchControls {
        auxiliary_only: true,
        ..TouchControls::default()
    };
    t.settings.button_visibility[Control::Debug as usize] = layout::SHOWN;
    assert!(!t.visible(Control::Move) && !t.visible(Control::Jump));
    assert!(t.visible(Control::Debug));
    let at = t.placement(Control::Debug).center;
    assert!(t.auxiliary_at(at));
    assert!(!t.auxiliary_at(t.settings.placement(Control::Jump).center));
    t.update(77, at, true, false);
    assert!(t.debug_requested && t.owns_pointer(77));
    t.update(77, at, false, true);
    assert!(!t.owns_pointer(77));
}

#[test]
fn shortcuts_take_their_finger_and_leave_movement_alone() {
    let mut t = TouchControls::default();
    t.settings.button_visibility[Control::Debug as usize] = layout::SHOWN;
    let [x, y] = t.settings.placement(Control::Move).center;
    t.update(1, [x, y], true, false);
    t.update(1, [x, y - 0.08], false, false);
    let moving = t.poll_state().left;
    assert_ne!(moving, [0.0; 2]);
    for (id, control) in [
        (2, Control::Debug),
        (3, Control::Perspective),
        (4, Control::Chat),
        (5, Control::Settings),
    ] {
        let point = t.settings.placement(control).center;
        t.update(id, point, true, false);
        assert!(matches!(role(&t, id), Role::Ignore));
        assert_eq!(t.poll_state().left, moving);
        assert_eq!(t.poll_state().triggers, [0.0; 2]);
        t.update(id, point, false, true);
    }
    assert!(t.debug_requested && t.perspective_requested);
    assert!(t.chat_requested && t.settings_requested);
    t.clear();
    assert!(!t.debug_requested && !t.perspective_requested && !t.chat_requested);
}

#[test]
fn contextual_use_button_is_opt_in_and_stays_until_release() {
    let mut t = TouchControls::default();
    t.settings.mode = Mode::Tap;
    t.context_use = true;
    assert!(!t.visible(Control::Use));
    t.settings.contextual_interact = true;
    assert!(t.visible(Control::Use));
    let at = t.settings.placement(Control::Use).center;
    t.update(1, at, true, false);
    t.context_use = false;
    assert!(t.visible(Control::Use));
    assert_eq!(t.poll_state().triggers[0], 1.0);
    t.update(1, at, false, true);
    assert!(!t.visible(Control::Use));
    t.settings.mode = Mode::Crosshair;
    assert!(t.visible(Control::Use));
}

#[test]
fn quick_taps_survive_one_poll_and_shared_buttons_release_together() {
    let mut t = TouchControls::default();
    let jump = t.settings.placement(Control::Jump).center;
    let attack = t.settings.placement(Control::Attack).center;
    t.update(1, jump, true, false);
    t.update(1, jump, false, true);
    assert!(t.poll_state().held(Button::South));
    assert!(!t.poll_state().held(Button::South));
    t.update(2, attack, true, false);
    t.update(3, attack, true, false);
    t.update(2, attack, false, true);
    assert_eq!(t.state().triggers[1], 1.0);
    t.update(3, attack, false, true);
    assert_eq!(t.state().triggers[1], 0.0);
}

#[test]
fn joystick_camera_and_jump_have_their_own_fingers() {
    let mut t = TouchControls::default();
    let [x, y] = t.settings.placement(Control::Move).center;
    t.update(1, [x, y], true, false);
    t.update(1, [x + 0.07, y - 0.12], false, false);
    t.update(2, [0.6, 0.4], true, false);
    let d = t.update(2, [0.65, 0.45], false, false);
    assert!((d[0] - 0.05).abs() < 0.0001);
    t.update(3, t.settings.placement(Control::Jump).center, true, false);
    assert!(t.state().held(Button::South));
    assert!(t.state().left[0] > 0.6);
    // A second finger on the joystick neither moves nor steals it.
    t.update(4, [x - 0.04, y], true, false);
    t.update(4, [x - 0.11, y - 0.06], false, false);
    assert!(t.state().left[0] > 0.6);
    t.clear();
    assert_eq!(t.poll_state(), PadState::default());
    assert_eq!(t.update(9, [0.5, 0.5], false, false), [0.0; 2]);
}

#[test]
fn joystick_sprint_starts_past_the_rim_and_stops_inside() {
    let mut t = TouchControls::default();
    t.settings.sprint_with_joystick = true;
    let [x, y] = t.settings.placement(Control::Move).center;
    t.update(1, [x, y], true, false);
    t.update(1, [x, y - 0.14], false, false);
    assert!(!t.state().held(Button::LeftStick));
    t.update(1, [x, y - 0.18], false, false);
    assert!(t.state().held(Button::LeftStick));
    assert!((t.state().left[1] - 1.0).abs() < 1e-6);
    t.update(1, [x, y - 0.14], false, false);
    assert!(!t.state().held(Button::LeftStick));
}

#[test]
fn dpad_slides_between_directions_crouches_on_double_tap_and_mirrors() {
    let mut t = TouchControls::default();
    t.settings.mode = Mode::Dpad;
    t.update(1, [0.13, 0.66], true, false);
    assert_eq!(t.state().left, [0.0, 1.0]);
    t.update(1, [0.20, 0.76], false, false);
    assert_eq!(t.state().left, [1.0, 0.0]);
    t.clear();
    t.update(10, [0.13, 0.76], true, false);
    assert!(!t.poll_state().held(Button::RightStick));
    t.update(10, [0.13, 0.76], false, true);
    t.update(12, [0.13, 0.76], true, false);
    assert!(t.poll_state().held(Button::RightStick));
    t.clear();
    // Flying, Bedrock's down button sits under up (Meridian): the D-pad's centre is empty.
    t.vertical_movement = true;
    t.update(13, [0.13, 0.76], true, false);
    assert!(!t.state().held(Button::RightStick));
    t.clear();
    t.vertical_movement = false;
    t.settings.swap_jump_sneak = true;
    t.update(11, [0.13, 0.76], true, false);
    assert!(t.state().held(Button::South));
    t.clear();
    t.settings.left_handed = true;
    t.update(2, [0.87, 0.66], true, false);
    assert_eq!(t.state().left, [0.0, 1.0]);
}

#[test]
fn hidden_sprint_button_sends_nothing() {
    let mut t = TouchControls::default();
    t.settings.sprint_button = false;
    let center = t.settings.placement(Control::Sprint).center;
    t.update(1, center, true, false);
    assert!(!t.poll_state().held(Button::LeftStick));
    t.update(1, center, false, true);
    t.settings.sprint_button = true;
    t.update(2, center, true, false);
    assert!(t.poll_state().held(Button::LeftStick));
    t.update(2, center, false, true);
    assert!(!t.poll_state().held(Button::LeftStick));
}

#[test]
fn forced_crosshair_brings_back_attack_and_use_and_ignored_fingers_do_nothing() {
    let mut t = TouchControls::default();
    t.settings.mode = Mode::Tap;
    assert!(!t.crosshair() && !t.visible(Control::Attack));
    t.force_crosshair = true;
    assert!(t.crosshair() && t.visible(Control::Attack) && t.visible(Control::Use));
    t.settings.button_visibility[Control::Attack as usize] = layout::HIDDEN;
    assert!(!t.visible(Control::Attack));
    t.ignore(5);
    assert!(t.owns_pointer(5));
    assert_eq!(t.update(5, [0.9, 0.9], false, false), [0.0; 2]);
    assert_eq!(t.state(), PadState::default());
    t.tap(Button::East);
    assert!(t.poll_state().held(Button::East));
    assert!(!t.poll_state().held(Button::East));
}

#[test]
fn a_world_touch_turns_the_camera_only_once_it_drags() {
    let mut t = TouchControls::default();
    t.update(1, [0.6, 0.4], true, false);
    assert!(matches!(role(&t, 1), Role::Look { dragged: false, .. }));
    // A small wobble is not a drag.
    assert_eq!(t.update(1, [0.605, 0.4], false, false), [0.0; 2]);
    let d = t.update(1, [0.7, 0.45], false, false);
    assert!((d[0] - 0.095).abs() < 1e-5 && (d[1] - 0.05).abs() < 1e-5);
    assert_eq!(t.poll_state(), PadState::default());
    t.update(1, [0.7, 0.45], false, true);
    assert!(!t.owns_pointer(1));
    // With button-drag look off, buttons never turn the camera.
    t.settings.attack_button_look = false;
    let jump = t.placement(Control::Jump).center;
    t.update(2, jump, true, false);
    assert_eq!(
        t.update(2, [jump[0] - 0.1, jump[1]], false, false),
        [0.0; 2]
    );
}

#[test]
fn a_tap_on_the_hotbar_selects_that_slot_in_every_scheme() {
    for mode in Mode::ALL {
        let mut t = TouchControls::default();
        t.settings.mode = mode;
        // Before the HUD's size is known the hotbar is just more world.
        let fourth = [(133.5 + 3.0 * 20.0) / 427.0, 230.0 / 240.0];
        t.update(1, fourth, true, false);
        assert!(matches!(role(&t, 1), Role::Look { .. }), "{mode:?}");
        t.update(1, fourth, false, true);
        t.clear();
        t.hud_size(427.0, 240.0);
        t.update(2, fourth, true, false);
        t.update(2, fourth, false, true);
        let key = String::from("key.hotbar.4");
        assert_eq!(
            t.key_events(),
            [(key.clone(), true), (key, false)],
            "{mode:?}"
        );
        assert_eq!(t.take_tap(), None, "{mode:?}: not a world tap");
    }
}

#[test]
fn swimming_or_flying_on_the_dpad_stacks_up_over_down_where_jump_was() {
    let mut t = TouchControls::default();
    t.viewport(2400, 1080);
    t.settings.mode = Mode::Dpad;
    let jump = t.placement(Control::Jump);
    let pad = t.placement(Control::Move);
    assert!((t.placement(Control::Sneak).center[1] - pad.center[1]).abs() < 0.01);
    for flying in [false, true] {
        t.swimming = !flying;
        t.vertical_movement = flying;
        let (up, down) = (t.placement(Control::Jump), t.placement(Control::Sneak));
        assert_eq!(up.center[0], jump.center[0], "flying {flying}");
        assert_eq!(down.center[0], jump.center[0], "flying {flying}");
        let ([_, up_y, _, up_h], [_, down_y, _, _]) = (up.rect(t.aspect()), down.rect(t.aspect()));
        assert!(
            up_y + up_h <= down_y,
            "flying {flying}: up over down, apart"
        );
        // A finger there holds sneak, not the D-pad's crouch toggle.
        t.update(1, down.center, true, false);
        assert!(!t.state().held(Button::RightStick), "flying {flying}");
        assert_eq!(t.key_events(), [(HOLD_SNEAK.into(), true)], "flying {flying}");
        t.update(1, down.center, false, true);
        assert_eq!(t.key_events(), [(HOLD_SNEAK.into(), false)], "flying {flying}");
    }
    t.vertical_movement = false;
    assert_eq!(t.placement(Control::Jump), jump, "back on land");
}

#[test]
fn a_key_button_holds_its_key_until_the_finger_lifts_or_the_controls_clear() {
    let mut t = TouchControls::default();
    t.viewport(2400, 1080);
    let mode = t.settings.mode.index();
    t.settings.keys[mode].push(layout::KeyButton {
        key: "G".into(),
        place: layout::NEW_KEY,
        toggle: false,
    });
    let at = layout::NEW_KEY.center;
    let g = |down| (format!("{KEYBOARD}G"), down);
    t.update(1, at, true, false);
    assert_eq!(t.key_events(), [g(true)]);
    assert!(t.owns_pointer(1));
    assert_eq!(t.poll_state(), PadState::default(), "no controller button");
    t.update(1, at, false, true);
    assert_eq!(t.key_events(), [g(false)]);
    // Cleared while held (the window lost focus, a screen opened), the key still lets go.
    t.update(2, at, true, false);
    t.clear();
    assert_eq!(t.key_events(), [g(true), g(false)]);
    // A name the editor never offers is no button: the touch is the world's.
    t.settings.keys[mode][0].key = "W".into();
    t.update(3, at, true, false);
    assert!(t.key_events().is_empty());
}

/// A toggle key button: a tap presses, the next tap lets go, a clear lets go too; its finger holds
/// nothing, so a lift sends no event.
#[test]
fn a_toggle_key_button_presses_on_one_tap_and_lets_go_on_the_next() {
    let mut t = TouchControls::default();
    t.viewport(2400, 1080);
    let mode = t.settings.mode.index();
    t.settings.keys[mode].push(layout::KeyButton {
        key: "G".into(),
        place: layout::NEW_KEY,
        toggle: true,
    });
    let at = layout::NEW_KEY.center;
    let g = |down| (format!("{KEYBOARD}G"), down);
    t.update(1, at, true, false);
    t.update(1, at, false, true);
    assert_eq!(t.key_events(), [g(true)]);
    t.update(1, at, true, false);
    t.update(1, at, false, true);
    assert_eq!(t.key_events(), [g(false)]);
    // Latched, then cleared (an editor or a screen opening): it lets go.
    t.update(1, at, true, false);
    t.update(1, at, false, true);
    t.clear();
    assert_eq!(t.key_events(), [g(true), g(false)]);
    // The screen's own path lets go of it too, once.
    t.update(1, at, true, false);
    t.update(1, at, false, true);
    t.release_latched();
    t.release_latched();
    assert_eq!(t.key_events(), [g(true), g(false)]);
}

/// Pick sends the game's pick block action, down on the press and up on the lift.
#[test]
fn the_pick_button_sends_the_pick_block_action() {
    assert!(layout::KEYS.contains(&"Pick"));
    let mut t = TouchControls::default();
    t.viewport(2400, 1080);
    let mode = t.settings.mode.index();
    t.settings.keys[mode].push(layout::KeyButton {
        key: "Pick".into(),
        place: layout::NEW_KEY,
        toggle: false,
    });
    t.settings.sanitize();
    assert_eq!(t.settings.keys[mode].len(), 1, "a known key");
    let at = layout::NEW_KEY.center;
    let pick = |down| ("action:key.pickItem".to_owned(), down);
    t.update(1, at, true, false);
    assert_eq!(t.key_events(), [pick(true)]);
    t.update(1, at, false, true);
    assert_eq!(t.key_events(), [pick(false)]);
}

#[test]
fn a_hotbar_hold_drops_one_item_once_and_a_tap_only_selects() {
    let mut t = TouchControls::default();
    t.hud_size(427.0, 240.0);
    let fourth = [(133.5 + 3.0 * 20.0) / 427.0, 230.0 / 240.0];
    let drop = |down| ("key.drop".to_owned(), down);
    // A tap selects and drops nothing.
    t.update(1, fourth, true, false);
    t.key_events();
    t.poll_state();
    t.update(1, fourth, false, true);
    assert!(t.key_events().is_empty());
    // A hold drops once, after DROP_HOLD, and a longer hold doesn't repeat.
    t.update(2, fourth, true, false);
    t.key_events();
    t.poll_state();
    assert!(t.key_events().is_empty(), "too soon");
    if let Some((Role::Hotbar { since, .. }, _)) = t.fingers.get_mut(&2) {
        *since = Instant::now() - DROP_HOLD;
    }
    t.poll_state();
    assert_eq!(t.key_events(), [drop(true), drop(false)]);
    t.poll_state();
    assert!(t.key_events().is_empty());
    assert_eq!(t.state(), PadState::default());
    t.update(2, fourth, false, true);
    assert!(!t.owns_pointer(2));
}

#[test]
fn a_double_tap_on_the_dpads_up_arrow_sprints_while_held_and_walking_forward() {
    let mut t = TouchControls::default();
    t.settings.mode = Mode::Dpad;
    let up = [0.13, 0.66];
    // One tap walks.
    t.update(1, up, true, false);
    assert!(!t.state().held(Button::LeftStick));
    t.update(1, up, false, true);
    // The second tap sprints while it is held and the pad points forward.
    t.update(2, up, true, false);
    assert_eq!(t.state().left, [0.0, 1.0]);
    assert!(t.state().held(Button::LeftStick));
    t.update(2, [0.20, 0.76], false, false);
    assert!(
        !t.state().held(Button::LeftStick),
        "sliding to the right arrow"
    );
    t.update(2, up, false, false);
    assert!(t.state().held(Button::LeftStick));
    t.update(2, up, false, true);
    assert!(!t.state().held(Button::LeftStick));
    // A third tap, long after the second, is a first tap again.
    t.last_up_tap = Some(Instant::now() - DOUBLE_TAP);
    t.update(3, up, true, false);
    assert!(!t.state().held(Button::LeftStick));
    t.clear();
    assert!(t.dpad_sprint.is_none() && t.last_up_tap.is_none());
}

#[test]
fn the_quick_move_switch_sits_beside_the_close_button_and_mirrors() {
    let mut t = TouchControls::default();
    t.viewport(2408, 1080);
    let (close, quick) = (t.close_place(), t.quick_move_place());
    assert!(quick.center[0] < close.center[0] && quick.center[1] == close.center[1]);
    assert_eq!(quick.size, close.size);
    let (c, q) = (close.rect(t.aspect()), quick.rect(t.aspect()));
    assert!(q[0] + q[2] < c[0], "no overlap");
    assert!(t.quick_move_at(quick.center) && !t.quick_move_at(close.center));
    assert!(t.close_at(close.center) && !t.close_at(quick.center));
    t.settings.left_handed = true;
    assert!(t.quick_move_place().center[0] > t.close_place().center[0]);
}

#[test]
fn a_forgotten_inventory_finger_lets_north_go_so_the_next_press_is_new() {
    let mut t = TouchControls::default();
    t.viewport(2400, 1080);
    let at = t.placement(Control::Inventory).center;
    // The press opens the inventory; the lift goes to the inventory, not to the controls.
    t.update(1, at, true, false);
    assert!(t.poll_state().held(Button::North));
    assert!(t.poll_state().held(Button::North), "still held: the lift never came");
    t.forget(1);
    assert!(!t.owns_pointer(1));
    assert!(!t.poll_state().held(Button::North));
    // The next tap, with the same finger id, is a fresh press.
    t.update(1, at, true, false);
    assert!(t.poll_state().held(Button::North));
}

#[test]
fn a_forgotten_key_finger_lets_its_key_go_and_unknown_fingers_change_nothing() {
    let mut t = TouchControls::default();
    t.viewport(2400, 1080);
    let mode = t.settings.mode.index();
    t.settings.keys[mode].push(layout::KeyButton {
        key: "G".into(),
        place: layout::NEW_KEY,
        toggle: false,
    });
    let at = layout::NEW_KEY.center;
    t.update(1, at, true, false);
    let _ = t.key_events();
    t.forget(1);
    assert_eq!(t.key_events(), [(format!("{KEYBOARD}G"), false)]);
    t.forget(7);
    assert!(t.key_events().is_empty());
    assert_eq!(t.poll_state(), PadState::default());
}

#[test]
fn a_forgotten_world_finger_is_no_tap() {
    let mut t = TouchControls::default();
    t.settings.mode = Mode::Tap;
    t.viewport(2400, 1080);
    t.update(1, [0.5, 0.4], true, false);
    assert!(matches!(role(&t, 1), Role::Look { .. }));
    t.forget(1);
    assert_eq!(t.take_tap(), None);
    t.poll_state();
    assert_eq!(t.aim, None);
    // A lift that comes through `update` is a tap, as before.
    t.update(2, [0.5, 0.4], true, false);
    t.update(2, [0.5, 0.4], false, true);
    assert!(t.take_tap().is_some());
}

#[test]
fn the_down_button_holds_sneak_while_flying_or_swimming_and_toggles_on_the_ground() {
    let down = (HOLD_SNEAK.to_owned(), true);
    let up = (HOLD_SNEAK.to_owned(), false);
    for mode in [Mode::Crosshair, Mode::Tap, Mode::Dpad] {
        for (flying, swimming) in [(true, false), (false, true)] {
            let mut t = TouchControls::default();
            t.viewport(2400, 1080);
            t.settings.mode = mode;
            t.vertical_movement = flying;
            t.swimming = swimming;
            let at = t.placement(Control::Sneak).center;
            t.update(1, at, true, false);
            assert_eq!(t.key_events(), [down.clone()], "{mode:?}");
            assert!(!t.poll_state().held(Button::RightStick), "{mode:?}: no toggle");
            assert!(t.sneak_held(), "{mode:?}: the game keeps the hold alive by this");
            t.update(1, at, false, true);
            assert!(!t.sneak_held(), "{mode:?}");
            assert_eq!(t.key_events(), [up.clone()], "{mode:?}");
            // `forget` and `clear` let it go too.
            t.update(2, at, true, false);
            t.forget(2);
            assert_eq!(t.key_events(), [down.clone(), up.clone()], "{mode:?}");
            t.update(3, at, true, false);
            t.clear();
            assert_eq!(t.key_events(), [down.clone(), up.clone()], "{mode:?}");
        }
    }
    // On the ground the joystick schemes' sneak button is still the pad's toggle button.
    for mode in [Mode::Crosshair, Mode::Tap] {
        let mut t = TouchControls::default();
        t.viewport(2400, 1080);
        t.settings.mode = mode;
        let at = t.placement(Control::Sneak).center;
        t.update(1, at, true, false);
        assert!(t.poll_state().held(Button::RightStick), "{mode:?}");
        assert!(t.key_events().is_empty(), "{mode:?}");
    }
}

#[test]
fn a_control_draws_pressed_only_for_the_finger_that_landed_on_it() {
    let mut t = TouchControls::default();
    t.viewport(2400, 1080);
    let jump = t.placement(Control::Jump);
    let rect = jump.rect(t.aspect());
    let pressed = |t: &TouchControls| t.pressed_by_finger(Control::Jump, rect);
    // A look finger from empty sky slides onto jump: nothing is pressed, and nothing fires.
    t.update(1, [0.5, 0.2], true, false);
    t.update(1, jump.center, false, false);
    assert!(!pressed(&t));
    assert!(!t.poll_state().held(Button::South));
    t.update(1, jump.center, false, true);
    // A finger that landed on jump is pressed while it is over it, and not once it slides off.
    t.update(2, jump.center, true, false);
    assert!(pressed(&t));
    assert!(t.poll_state().held(Button::South));
    t.update(2, [0.5, 0.2], false, false);
    assert!(!pressed(&t));
    t.update(2, [0.5, 0.2], false, true);
    assert!(!pressed(&t));
    // A shortcut (Ignore role) shows pressed too, and lifting, forgetting and clearing end it.
    t.update(3, jump.center, true, false);
    t.forget(3);
    assert!(!pressed(&t));
    t.update(4, jump.center, true, false);
    t.clear();
    assert!(!pressed(&t));
}

#[test]
fn layout_profiles_page_and_new_settings_rows_work_and_cancel() {
    let dir = std::env::temp_dir().join(format!(
        "meridian-touch-mod-profiles-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let custom = Settings {
        sprint_with_joystick: true,
        offhand_button: false,
        ..Settings::default()
    };
    profiles::save_profile(&dir, "Profile 1", &custom).unwrap();
    let file = dir.join(editor::FILE);

    let mut t = TouchControls::default();
    t.viewport(2400, 1080);
    let original = t.settings.clone();
    let tap = |t: &mut TouchControls, p: [f32; 2]| {
        t.edit_at_test(p, &file);
    };

    t.open_editor();
    // Tap "Layout profiles" in HUB ([0.29, 0.38, 0.42, 0.09]) -> pick "Profile 1" (row 1).
    tap(&mut t, [0.50, 0.425]);
    tap(&mut t, [0.45, 0.29]);
    assert!(t.settings.sprint_with_joystick);
    assert!(!t.settings.offhand_button);

    // Open Settings ([0.29, 0.49, 0.42, 0.09]): verify AutoSprint, ButtonScale, OffhandButton,
    // AttackDragLook fit without overlap and mutate Settings.
    tap(&mut t, [0.50, 0.535]);
    // Auto-sprint is row 2 (col 0): [0.06, 0.13 + 2*0.112 = 0.354, 0.43, 0.098]
    tap(&mut t, [0.20, 0.40]);
    assert!(!t.settings.sprint_with_joystick);
    // Button scale + is row 4 (col 0): [0.06, 0.13 + 4*0.112 = 0.578, 0.43, 0.098], plus at x+w-0.035 = 0.455
    tap(&mut t, [0.455, 0.625]);
    assert!((t.settings.button_scale - 1.1).abs() < 1e-4);
    // Offhand button is row 5 (col 0): [0.06, 0.13 + 5*0.112 = 0.690, 0.43, 0.098]
    tap(&mut t, [0.20, 0.735]);
    assert!(t.settings.offhand_button);
    // Button drags look is row 0 (col 1): [0.51, 0.13, 0.43, 0.098]
    tap(&mut t, [0.65, 0.175]);
    assert!(!t.settings.attack_button_look);

    // Back -> Cancel -> ConfirmDiscard restores original.
    tap(&mut t, [0.50, 0.90]);
    tap(&mut t, [0.39, 0.695]);
    tap(&mut t, [0.625, 0.615]);
    assert!(!t.editing());
    assert_eq!(t.settings, original);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn top_row_buttons_pack_with_one_texel_gap_across_aspects() {
    let top = [
        Control::Debug,
        Control::Perspective,
        Control::Chat,
        Control::Pause,
        Control::Settings,
        Control::Offhand,
    ];
    for (w, h) in [(2408, 1080), (1920, 1080), (2560, 1600), (480, 854)] {
        for left_handed in [false, true] {
            for button_scale in [1.0, 2.0] {
                let mut t = TouchControls::default();
                t.viewport(w, h);
                t.settings.left_handed = left_handed;
                t.settings.button_scale = button_scale;
                let aspect = w as f32 / h as f32;
                let mut rects: Vec<_> = top.iter().map(|c| t.placement(*c).rect(aspect)).collect();
                if left_handed {
                    rects.reverse();
                }
                let btn_w = rects[0][2];
                for i in 0..rects.len() {
                    let r = rects[i];
                    assert!(r[0] >= 0.0 && r[0] + r[2] <= 1.0001, "{w}x{h} {r:?}");
                    let cx_sum = (r[0] + r[2] / 2.0) + (rects[5 - i][0] + rects[5 - i][2] / 2.0);
                    assert!((cx_sum - 1.0).abs() < 1e-4, "{w}x{h} sum={cx_sum}");
                }
                for pair in rects.windows(2) {
                    let gap = pair[1][0] - (pair[0][0] + pair[0][2]);
                    assert!(gap > 0.0, "{w}x{h} overlap gap={gap}");
                    assert!(
                        (gap - btn_w / 18.0).abs() < 1e-4,
                        "{w}x{h} gap={gap} expected={}",
                        btn_w / 18.0
                    );
                }
            }
        }
    }
    let mut t = TouchControls::default();
    t.viewport(1920, 1080);
    t.settings.layouts[0][Control::Chat as usize].center = [0.25, 0.2];
    assert_eq!(t.placement(Control::Chat).center, [0.25, 0.2]);
}

#[test]
fn docked_inventory_cell_aligns_with_hotbar_divider_at_full_opacity() {
    for (w, h) in [(427.0, 240.0), (480.0, 240.0)] {
        let mut t = TouchControls::default();
        t.viewport(1920, 1080);
        t.hud_size(w, h);
        let p = t.placement(Control::Inventory);
        assert_eq!(p.opacity, 1.0);
        let cx = (w as i32 / 2) as f32;
        let size = p.size * h;
        let left = p.center[0] * w - size / 2.0;
        let right = p.center[0] * w + size / 2.0;
        let top = p.center[1] * h - size / 2.0;
        let bottom = p.center[1] * h + size / 2.0;
        assert!((left - (cx + 89.0)).abs() < 1e-4, "left={left}");
        assert!((right - (cx + 111.0)).abs() < 1e-4, "right={right}");
        assert!((top - (h - 22.0)).abs() < 1e-4, "top={top}");
        assert!((bottom - h).abs() < 1e-4, "bottom={bottom}");

        t.settings.inventory_on_hotbar = false;
        let standalone = t.placement(Control::Inventory);
        assert_eq!(standalone, t.settings.placement(Control::Inventory));
    }
}

#[test]
fn holding_buttons_and_zoom_key_can_pan_the_camera_while_held() {
    let mut t = TouchControls::default();
    t.viewport(2400, 1080);
    let mode = t.settings.mode.index();
    t.settings.keys[mode].push(layout::KeyButton {
        key: "C".into(),
        place: layout::NEW_KEY,
        toggle: false,
    });

    // Holding Attack, Use, Jump, or Zoom ("C") pans the view once dragged past DRAG_SLOP
    // while keeping the button/key held, and small wobble does not move the view.
    for (id, point) in [
        (1, t.placement(Control::Attack).center),
        (2, t.placement(Control::Use).center),
        (3, t.placement(Control::Jump).center),
        (4, layout::NEW_KEY.center),
    ] {
        t.update(id, point, true, false);
        assert_eq!(t.update(id, [point[0] + 0.002, point[1]], false, false), [0.0; 2]);
        let panned = t.update(id, [point[0] - 0.12, point[1] + 0.04], false, false);
        assert_ne!(panned, [0.0; 2], "id {id} should pan while held");
        t.update(id, [point[0] - 0.12, point[1] + 0.04], false, true);
    }
    assert_eq!(
        t.key_events(),
        [(format!("{KEYBOARD}C"), true), (format!("{KEYBOARD}C"), false)]
    );

    // Shield in Tap mode also pans while held when enabled, and stops panning when disabled.
    t.settings.mode = Mode::Tap;
    t.shield_available = true;
    let shield = t.placement(Control::Shield).center;
    t.update(5, shield, true, false);
    assert!(t.shield_held());
    assert_ne!(
        t.update(5, [shield[0] - 0.12, shield[1]], false, false),
        [0.0; 2]
    );
    assert!(t.shield_held());
    t.update(5, [shield[0] - 0.12, shield[1]], false, true);

    t.settings.attack_button_look = false;
    t.update(6, shield, true, false);
    assert_eq!(
        t.update(6, [shield[0] - 0.12, shield[1]], false, false),
        [0.0; 2]
    );
    t.update(6, [shield[0] - 0.12, shield[1]], false, true);
}
