// Adapted from Starling (https://github.com/starling-client/starling, `android` branch at
// 150a414b09a1, `crates/horizon-client-render/src/touch.rs`), written by Vasysik and the
// Starling contributors. Copyright (c) 2026 Ludovic Marie and the Horizon contributors, under the
// same MIT license as Horizon (LICENSE at the repository root).

//! On-screen touch controls for touch-screen devices (Android first). Nothing
//! here runs unless a touch surface is in use.
//!
//! [`TouchControls`] turns fingers into the controller state the game already
//! reads ([`PadState`]). Each finger gets a role when it lands (the joystick or
//! D-pad, a button, or a look gesture on the world) and keeps it until it
//! lifts, so moving, looking and pressing never take each other's fingers.
pub mod aim;
mod draw;
mod editor;
pub mod gyro;
pub(crate) mod key_icons;
pub mod layout;
mod menu;
pub mod pojav;
pub mod profiles;
pub use aim::{
    attack_indicator, camera_delta, hotbar_slot, item_places_entity_on_tap, item_uses_on_tap,
    projectile_crosshair, screen_direction,
};
pub use layout::{Control, Mode, Placement, Settings};
pub use menu::{MenuButton, MenuEvent, MenuFinger};

use crate::gamepad::{Button, PadState};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

/// A world touch held this long without dragging becomes a hold (mining, eating).
const HOLD_DELAY: Duration = Duration::from_millis(350);
/// How long the marker of a short world tap stays.
const MARKER_TIME: Duration = Duration::from_millis(180);
/// Two taps on the D-pad's sneak button closer than this toggle crouching.
const DOUBLE_TAP: Duration = Duration::from_millis(300);
/// A finger held this long on a hotbar slot drops one of its item.
const DROP_HOLD: Duration = Duration::from_millis(500);
/// The gap between a menu button and the close button beside it, in surface heights.
const MENU_BUTTON_GAP: f32 = 0.012;
/// A look finger that moves farther than this (a fraction of the surface
/// height) is a camera drag, not a tap.
pub const DRAG_SLOP: f32 = 0.018;
/// The swap-hands action a [`Control::Offhand`] tap sends.
const SWAP_HANDS: &str = "action:key.swapOffhand";
/// Meridian: a key button's events ([`TouchControls::key_events`]) name its key after this, as
/// [`layout::KEYS`] does ("keyboard:B").
pub const KEYBOARD: &str = "keyboard:";
/// Meridian: the "Pick" key button's action: pick block, which is the middle mouse button's.
const PICK_BLOCK: &str = "action:key.pickItem";
/// Meridian: the action of a finger that holds sneak ([`Role::Hold`]): down on landing, up on
/// lifting, never the pad's sneak toggle. The game acts on it in `touch_actions`.
pub const HOLD_SNEAK: &str = "hold:sneak";

/// What a finger does, decided when it lands.
#[derive(Clone, Copy, Debug)]
enum Role {
    /// Drives the joystick or D-pad, measured from this anchor.
    Move([f32; 2]),
    /// Turns the camera; without a drag, a short tap or a long hold acts on
    /// the world where it landed.
    Look {
        start: [f32; 2],
        since: Instant,
        /// Moved past [`DRAG_SLOP`]: a camera turn, not a tap.
        dragged: bool,
        /// Whether a hold uses the item (eat, draw a bow) instead of mining,
        /// as [`TouchControls::hold_uses_item`] said when the finger landed.
        held_use: bool,
        /// Became a hold before it dragged; it keeps acting while it turns.
        committed: bool,
    },
    /// Landed on a control that acted at once; nothing more until it lifts.
    Ignore,
    /// A hotbar slot: it selected itself when the finger landed, and a long hold drops one item.
    Hotbar { since: Instant, dropped: bool },
    /// Holds the shield up.
    Shield,
    /// Holds a controller button.
    Button(Button),
    /// Meridian: holds a key the player added as a button (one of [`layout::KEYS`]).
    Key(&'static str),
    /// Meridian: holds an action (sneak: [`HOLD_SNEAK`]) until the finger lifts.
    Hold(&'static str),
}

/// Meridian: what key button `key` sends: that key, or for Pick the pick block action.
fn key_event(key: &str) -> String {
    match key {
        "Pick" => PICK_BLOCK.into(),
        _ => format!("{KEYBOARD}{key}"),
    }
}

/// The event that lets go of what a finger held, if it held a key or an action.
fn release_event(role: &Role) -> Option<(String, bool)> {
    match role {
        Role::Key(key) => Some((key_event(key), false)),
        Role::Hold(action) => Some(((*action).into(), false)),
        _ => None,
    }
}

/// The on-screen controls' input state. The game feeds it touches with
/// [`Self::update`] and reads [`Self::poll_state`] once a tick.
#[derive(Default)]
pub struct TouchControls {
    pub settings: Settings,
    /// The game needs the crosshair whatever the scheme.
    pub force_crosshair: bool,
    /// The held item is aimed along the crosshair ([`projectile_crosshair`]).
    pub projectile_crosshair: bool,
    /// The player has a shield, so the shield button may show.
    pub shield_available: bool,
    /// Only the shortcut buttons (chat, pause, settings, F3, F5): a keyboard
    /// or controller moves the player.
    pub auxiliary_only: bool,
    /// The use button would act on what the crosshair points at (shown with
    /// `contextual_interact` in the tap schemes).
    pub context_use: bool,
    /// Flying: jump and sneak move up and down.
    pub vertical_movement: bool,
    pub swimming: bool,
    pub mounted: bool,
    /// The player is sneaking: the sneak button shows held.
    pub sneaking: bool,
    /// How far the block under a world hold is broken (0..1), drawn in the
    /// touch marker.
    pub break_progress: f32,
    /// A long hold on the world uses the held item instead of mining. Read when
    /// a finger lands, so a change mid-hold (the last apple eaten) can't turn
    /// eating into mining.
    pub hold_uses_item: bool,
    /// Where this tick's world action aims (0..1 of the surface), set by
    /// [`Self::poll_state`].
    pub aim: Option<[f32; 2]>,
    /// A world hold is active this tick, and whether it uses the item.
    pub holding: bool,
    pub holding_use: bool,
    /// A button was pressed ([`Self::take_clicked`]).
    clicked: bool,
    pub chat_requested: bool,
    pub settings_requested: bool,
    pub debug_requested: bool,
    pub perspective_requested: bool,
    fingers: BTreeMap<u64, (Role, [f32; 2])>,
    /// Meridian: buttons/hold-keys keep their landing point so slight finger wobble doesn't turn the
    /// view until dragged past [`DRAG_SLOP`].
    button_drag: BTreeMap<u64, ([f32; 2], bool)>,
    /// Meridian: the control each finger landed on, for [`Self::draw`]: a control shows pressed
    /// for its own finger, not for one that slides over it.
    landed: BTreeMap<u64, Control>,
    /// Presses that must reach the game even if released before the next poll.
    pending: PadState,
    /// Key or action presses for the game's key bindings: (token, down).
    key_events: Vec<(String, bool)>,
    /// Meridian: the toggle key buttons pressed now (a tap pressed each, the next lets it go).
    latched_keys: Vec<&'static str>,
    tap_aim: Option<[f32; 2]>,
    /// A short world tap waiting for the game, with the finger's `held_use`.
    pending_tap: Option<bool>,
    last_marker: Option<([f32; 2], Instant)>,
    last_sneak_tap: Option<Instant>,
    sprint_latched: bool,
    /// Meridian: when the D-pad's up arrow last landed a finger, for a double tap.
    last_up_tap: Option<Instant>,
    /// Meridian: the finger whose double tap on the D-pad's up arrow sprints while it is held.
    dpad_sprint: Option<u64>,
    aspect: f32,
    /// Meridian: the HUD's size in GUI pixels (0 until known), for the inventory's hotbar cell.
    hud: [f32; 2],
    editor: Option<editor::Editor>,
}

impl TouchControls {
    /// Whether attack and use act on the crosshair rather than on a tap.
    pub fn crosshair(&self) -> bool {
        self.force_crosshair || self.settings.crosshair()
    }

    /// The surface size in pixels; controls stay square on any shape.
    pub fn viewport(&mut self, width: u32, height: u32) {
        self.aspect = width.max(1) as f32 / height.max(1) as f32;
    }

    /// Meridian: the HUD's size in GUI pixels (`HudBatch::width`, `height`), where the hotbar
    /// is drawn: the inventory button sits at its end.
    pub fn hud_size(&mut self, width: f32, height: f32) {
        self.hud = [width, height];
    }

    /// Meridian: the hotbar slot under `point` (0..1 of the surface), once the HUD's size is known.
    fn hotbar_at(&self, point: [f32; 2]) -> Option<usize> {
        let [width, height] = self.hud;
        (width > 0.0 && height > 0.0)
            .then(|| hotbar_slot(width, height, [point[0] * width, point[1] * height]))
            .flatten()
    }

    /// Meridian: the inventory's cell docked seamlessly onto the hotbar's right end (`hotbar.png`
    /// is 182×22 GUI pixels at `(w as i32 / 2 - 91, h as i32 - 22)`). Overlapping slot 8's right
    /// rim (`cx + 89..cx + 111`) replaces its outer black edge with the matching 4px slot divider
    /// at exact 20px slot pitch and full hotbar opacity. None until the HUD's size is known.
    fn hotbar_cell(&self) -> Option<Placement> {
        let [width, height] = self.hud;
        (width > 0.0 && height > 0.0).then(|| {
            let cx = (width as i32 / 2) as f32;
            let h = (height as i32) as f32;
            Placement {
                center: [(cx + 100.0) / width, (h - 11.0) / height],
                size: 22.0 / height,
                opacity: 1.0,
            }
        })
    }

    fn aspect(&self) -> f32 {
        if self.aspect > 0.0 {
            self.aspect
        } else {
            16.0 / 9.0
        }
    }

    /// Where a dynamic joystick may appear, as `[x, y, w, h]`: a strip
    /// `dynamic_area_x` of the width wide on the joystick's side, and
    /// `dynamic_area_y` of the height tall centred on it. Touches outside it
    /// stay camera gestures.
    pub fn dynamic_area(&self) -> [f32; 4] {
        let home = self.placement(Control::Move);
        let width = self.settings.dynamic_area_x.clamp(0.0, 1.0);
        let height = self.settings.dynamic_area_y.clamp(0.0, 1.0);
        let y = (home.center[1] - height / 2.0).clamp(0.0, 1.0 - height);
        let x = if home.center[0] < 0.5 {
            0.0
        } else {
            1.0 - width
        };
        [x, y, width, height]
    }

    fn dynamic_joystick(&self) -> bool {
        self.settings.dynamic_joystick && self.settings.mode != Mode::Dpad
    }

    /// The shortcuts that stay with [`Self::auxiliary_only`].
    fn auxiliary_control(c: Control) -> bool {
        matches!(
            c,
            Control::Debug
                | Control::Chat
                | Control::Pause
                | Control::Settings
                | Control::Perspective
        )
    }

    /// Whether `point` is on a visible shortcut button.
    pub fn auxiliary_at(&self, point: [f32; 2]) -> bool {
        Control::ALL.into_iter().any(|c| {
            Self::auxiliary_control(c)
                && self.visible(c)
                && layout::contains(self.placement(c).rect(self.aspect()), point)
        })
    }

    /// Meridian: where key button `i` of the current scheme is on this surface, scaled and kept on
    /// screen as the built-in controls are.
    pub(super) fn key_place(&self, i: usize) -> Option<Placement> {
        self.settings.key_placement(i).map(|p| self.fit(p))
    }

    /// Meridian: the key button under `point`, the last-drawn one first (they're drawn over the
    /// built-in controls). None with a keyboard in use (`auxiliary_only`).
    fn key_at(&self, point: [f32; 2]) -> Option<(&'static str, bool)> {
        if self.auxiliary_only {
            return None;
        }
        let keys = self.settings.mode_keys();
        (0..keys.len()).rev().find_map(|i| {
            let p = self.key_place(i)?;
            layout::contains(p.rect(self.aspect()), point)
                .then(|| keys[i].name().map(|name| (name, keys[i].toggle)))
                .flatten()
        })
    }

    /// Meridian: where the button that closes an inventory is on this surface.
    pub(super) fn close_place(&self) -> Placement {
        self.fit(self.settings.close_placement())
    }

    /// Meridian: whether `point` (0..1 of the surface) is on the close button, which shows over an
    /// open inventory.
    pub fn close_at(&self, point: [f32; 2]) -> bool {
        layout::contains(self.close_place().rect(self.aspect()), point)
    }

    /// Meridian: where the quick-move switch is, beside the close button on the side facing the
    /// screen's middle.
    pub(super) fn quick_move_place(&self) -> Placement {
        let mut p = self.close_place();
        let side = if self.settings.left_handed { 1.0 } else { -1.0 };
        let wide = p.size / self.aspect();
        p.center[0] = (p.center[0] + side * (wide + MENU_BUTTON_GAP / self.aspect()))
            .clamp(wide / 2.0, 1.0 - wide / 2.0);
        p
    }

    /// Meridian: whether `point` (0..1 of the surface) is on the quick-move switch, which shows
    /// beside the close button over an open inventory.
    pub fn quick_move_at(&self, point: [f32; 2]) -> bool {
        layout::contains(self.quick_move_place().rect(self.aspect()), point)
    }

    /// The visible control under `point`, the last-drawn one first.
    pub fn control_at(&self, point: [f32; 2]) -> Option<Control> {
        Control::ALL.into_iter().rev().find(|&c| {
            if !self.visible(c) {
                return false;
            }
            if c == Control::Move && self.dynamic_joystick() {
                let area = self.dynamic_area();
                return area[2] > 0.0
                    && area[3] > 0.0
                    && !self.moving()
                    && layout::contains(area, point);
            }
            layout::contains(self.placement(c).rect(self.aspect()), point)
        })
    }

    fn moving(&self) -> bool {
        self.fingers
            .values()
            .any(|(r, _)| matches!(r, Role::Move(_)))
    }

    /// Whether this finger belongs to the touch controls.
    pub fn owns_pointer(&self, id: u64) -> bool {
        self.fingers.contains_key(&id)
    }

    /// Hands a finger to something else (a menu) until it lifts.
    pub fn ignore(&mut self, id: u64) {
        self.fingers.insert(id, (Role::Ignore, [0.0; 2]));
    }

    /// Presses `button` for one poll.
    pub fn tap(&mut self, button: Button) {
        self.pending.set(button, true);
    }

    fn visible(&self, c: Control) -> bool {
        let visibility = self.settings.button_visibility[c as usize];
        if c == Control::Shield
            && (!self.shield_available
                || (self.force_crosshair
                    && visibility == layout::FOLLOW_SCHEME
                    && self.settings.button_visibility[Control::Use as usize]
                        == layout::FOLLOW_SCHEME))
        {
            // A forced crosshair brings back the use button, which raises the shield too.
            return false;
        }
        if self.auxiliary_only && !Self::auxiliary_control(c) {
            return false;
        }
        if self.force_crosshair
            && matches!(c, Control::Attack | Control::Use)
            && visibility == layout::FOLLOW_SCHEME
        {
            return true;
        }
        self.settings.visible(c)
            || (c == Control::Use
                && visibility != layout::HIDDEN
                && self.settings.contextual_interact
                && (self.context_use
                    || self
                        .fingers
                        .values()
                        .any(|(r, _)| matches!(r, Role::Button(Button::LeftTrigger)))))
    }

    /// Where a control is on this surface: the saved placement, scaled by
    /// `button_scale` and kept on screen.
    fn placement(&self, c: Control) -> Placement {
        if c == Control::Inventory
            && self.settings.inventory_on_hotbar
            && let Some(cell) = self.hotbar_cell()
        {
            return cell;
        }
        let aspect = self.aspect();
        let mut p = self.settings.placement(c);
        if matches!(c, Control::Jump | Control::Sneak)
            && let Some(stack) = self.vertical_stack()
        {
            p = stack[usize::from(c == Control::Sneak)];
        }
        // Pack the factory top row snugly side-by-side around the horizontal centre (1-texel gap,
        // 19/18 of the rendered button width, matching Bedrock's top bar) at every aspect ratio.
        // Moved or resized buttons stay where the player saved them.
        if p.center[1] == 0.055 && p.size == 0.075 {
            let mirror = |x: f32| {
                if self.settings.left_handed {
                    1.0 - x
                } else {
                    x
                }
            };
            let top = match c {
                Control::Debug => Some((0.375, 0usize)),
                Control::Perspective => Some((0.425, 1)),
                Control::Chat => Some((0.475, 2)),
                Control::Pause => Some((0.525, 3)),
                Control::Settings => Some((0.575, 4)),
                Control::Offhand => Some((0.625, 5)),
                _ => None,
            };
            if let Some((factory_x, slot)) = top
                && (p.center[0] - mirror(factory_x)).abs() < 0.0001
            {
                const TOP_ROW_STEP: f32 = 19.0 / 18.0;
                let total_units = 5.0 * TOP_ROW_STEP + 1.0;
                let scale = self.settings.button_scale.max(f32::EPSILON);
                let max_scaled = (aspect * 0.92 / total_units).min(aspect).min(1.0);
                let scaled_size = (p.size * scale).min(max_scaled);
                p.size = scaled_size / scale;
                let btn_w = scaled_size / aspect;
                let sign = if self.settings.left_handed { -1.0 } else { 1.0 };
                p.center[0] = 0.5 + sign * (slot as f32 - 2.5) * btn_w * TOP_ROW_STEP;
            }
        }
        self.fit(p)
    }

    /// Meridian: swimming or flying on the D-pad with sneak in its centre, Bedrock stacks up over
    /// down where jump is and leaves the centre empty: jump's and sneak's places then.
    fn vertical_stack(&self) -> Option<[Placement; 2]> {
        let s = &self.settings;
        if s.mode != Mode::Dpad || !(self.swimming || self.vertical_movement) {
            return None;
        }
        let [pad, jump, sneak] =
            [Control::Move, Control::Jump, Control::Sneak].map(|c| s.placement(c));
        let near = pad.size / 6.0;
        let centred = (sneak.center[0] - pad.center[0]).abs() * self.aspect() < near
            && (sneak.center[1] - pad.center[1]).abs() < near;
        if !centred {
            return None;
        }
        let step = jump.size / 2.0 + 0.004;
        let at = |dy: f32| Placement {
            center: [jump.center[0], jump.center[1] + dy],
            ..jump
        };
        Some([at(-step), at(step)])
    }

    /// Scales a placement by `button_scale` and keeps it inside the surface.
    fn fit(&self, mut p: Placement) -> Placement {
        let aspect = self.aspect();
        p.size = (p.size * self.settings.button_scale).min(aspect).min(1.0);
        let half = [p.size / aspect / 2.0, p.size / 2.0];
        for (c, half) in p.center.iter_mut().zip(half) {
            *c = c.clamp(half, 1.0 - half);
        }
        p
    }

    /// The controller button a control holds, if it holds one.
    fn button(c: Control) -> Option<Button> {
        Some(match c {
            Control::Sprint => Button::LeftStick,
            Control::Jump => Button::South,
            Control::Sneak => Button::RightStick,
            Control::Attack => Button::RightTrigger,
            Control::Use | Control::Shield => Button::LeftTrigger,
            Control::Pause => Button::Start,
            Control::Inventory => Button::North,
            _ => return None,
        })
    }

    /// Whether a finger holds the shield button.
    pub fn shield_held(&self) -> bool {
        self.settings.shield_button
            && self.shield_available
            && self
                .fingers
                .values()
                .any(|(r, _)| matches!(r, Role::Shield))
    }

    /// Where the touch marker goes in the tap schemes: under a look finger, or
    /// briefly where a short tap landed.
    pub fn pointer(&self) -> Option<[f32; 2]> {
        if self.crosshair() {
            return None;
        }
        self.fingers
            .values()
            .find_map(|(r, point)| matches!(r, Role::Look { .. }).then_some(*point))
            .or_else(|| {
                self.last_marker
                    .filter(|(_, t)| t.elapsed() < MARKER_TIME)
                    .map(|(p, _)| p)
            })
    }

    /// The aim point of a world action: the crosshair, or where the finger is.
    fn aim_at(&self, point: [f32; 2]) -> [f32; 2] {
        if self.settings.crosshair() || self.projectile_crosshair {
            [0.5, 0.5]
        } else {
            point
        }
    }

    fn world_gestures(&self) -> bool {
        !self.crosshair() || self.settings.crosshair_gestures
    }

    /// Meridian: Bedrock-style same-finger camera panning while holding gameplay action/key buttons
    /// (attack/break, use/place, jump, sprint, sneak, shield, held key buttons such as Zoom).
    fn button_can_pan(role: &Role) -> bool {
        matches!(
            role,
            Role::Button(
                Button::RightTrigger
                    | Button::LeftTrigger
                    | Button::South
                    | Button::LeftStick
                    | Button::RightStick
            ) | Role::Shield
                | Role::Hold(_)
                | Role::Key(_)
        )
    }

    /// One touch event: finger `id` at `point` (0..1 of the surface), as it
    /// lands (`started`), moves, or lifts (`ended`). Returns how far the
    /// camera should turn, in surface fractions (see [`camera_delta`]).
    pub fn update(&mut self, id: u64, point: [f32; 2], started: bool, ended: bool) -> [f32; 2] {
        if ended {
            self.lift(id);
            return [0.0; 2];
        }
        if started {
            self.land(id, point);
            return [0.0; 2];
        }
        let aspect = self.aspect();
        let button_look = self.settings.attack_button_look;
        let Some((role, previous)) = self.fingers.get_mut(&id) else {
            return [0.0; 2];
        };
        let turns = match role {
            Role::Look {
                start,
                since,
                dragged,
                committed,
                ..
            } => {
                if !*dragged && since.elapsed() >= HOLD_DELAY {
                    *committed = true;
                }
                *dragged |= ((point[0] - start[0]) * aspect).hypot(point[1] - start[1]) > DRAG_SLOP;
                *dragged
            }
            role if button_look && Self::button_can_pan(role) => {
                if let Some((start, dragged)) = self.button_drag.get_mut(&id) {
                    *dragged |=
                        ((point[0] - start[0]) * aspect).hypot(point[1] - start[1]) > DRAG_SLOP;
                    *dragged
                } else {
                    false
                }
            }
            _ => false,
        };
        let delta = [point[0] - previous[0], point[1] - previous[1]];
        *previous = point;
        if turns { delta } else { [0.0; 2] }
    }

    /// Takes finger `id` out of the controls, letting go of what it held (a key button's key goes
    /// up in [`Self::key_events`]). The finger's role, if the controls had it.
    fn drop_finger(&mut self, id: u64) -> Option<Role> {
        if self.dpad_sprint == Some(id) {
            self.dpad_sprint = None;
        }
        self.landed.remove(&id);
        self.button_drag.remove(&id);
        let (role, _) = self.fingers.remove(&id)?;
        self.key_events.extend(release_event(&role));
        Some(role)
    }

    /// Meridian: whether a finger holds sneak (the down button while flying or swimming).
    pub fn sneak_held(&self) -> bool {
        self.fingers
            .values()
            .any(|(role, _)| matches!(role, Role::Hold(_)))
    }

    /// Meridian: finger `id` ended somewhere the controls don't see (a screen its press opened
    /// took the lift): it lets go with no tap or gesture, so a button it held (the inventory's
    /// North) comes up and its next press is a new one. A pending latched press still reaches the
    /// game. Unknown ids do nothing.
    pub fn forget(&mut self, id: u64) {
        self.drop_finger(id);
    }

    fn lift(&mut self, id: u64) {
        let Some(Role::Look {
            start,
            since,
            dragged,
            held_use,
            ..
        }) = self.drop_finger(id)
        else {
            return;
        };
        if self.world_gestures() && !dragged && since.elapsed() < HOLD_DELAY {
            self.tap_aim = Some(self.aim_at(start));
            self.last_marker = Some((start, Instant::now()));
            self.pending_tap = Some(held_use);
        }
    }

    fn land(&mut self, id: u64, point: [f32; 2]) {
        // Meridian: a key button presses its key until the finger lifts; a toggle button presses
        // it on one tap and lets it go on the next, its finger holding nothing.
        if let Some((key, toggle)) = self.key_at(point) {
            self.clicked = true;
            if toggle {
                let down = !self.latched_keys.contains(&key);
                self.latched_keys.retain(|k| *k != key);
                if down {
                    self.latched_keys.push(key);
                }
                self.key_events.push((key_event(key), down));
                self.fingers.insert(id, (Role::Ignore, point));
            } else {
                self.key_events.push((key_event(key), true));
                self.button_drag.insert(id, (point, false));
                self.fingers.insert(id, (Role::Key(key), point));
            }
            return;
        }
        let control = self.control_at(point);
        match control {
            Some(c) => self.landed.insert(id, c),
            None => self.landed.remove(&id),
        };
        let role = match control {
            Some(Control::Move) if !self.moving() => {
                let anchor = if self.dynamic_joystick() {
                    point
                } else {
                    self.placement(Control::Move).center
                };
                if self.settings.mode == Mode::Dpad && self.dpad_up_at(anchor, point) {
                    // Meridian: a double tap on the D-pad's up arrow sprints while it is held.
                    let now = Instant::now();
                    if self
                        .last_up_tap
                        .take()
                        .is_some_and(|t| now.duration_since(t) < DOUBLE_TAP)
                    {
                        self.dpad_sprint = Some(id);
                    } else {
                        self.last_up_tap = Some(now);
                    }
                }
                Role::Move(anchor)
            }
            Some(Control::Shield) => {
                self.clicked = true;
                self.pending.triggers[0] = 1.0;
                Role::Shield
            }
            Some(Control::Offhand) => {
                self.clicked = true;
                self.key_events.push((SWAP_HANDS.into(), true));
                self.key_events.push((SWAP_HANDS.into(), false));
                Role::Ignore
            }
            Some(Control::Chat) => {
                self.chat_requested = true;
                Role::Ignore
            }
            Some(Control::Settings) => {
                self.settings_requested = true;
                Role::Ignore
            }
            Some(Control::Debug) => {
                self.debug_requested = true;
                Role::Ignore
            }
            Some(Control::Perspective) => {
                self.perspective_requested = true;
                Role::Ignore
            }
            // Meridian: flying and swimming, the down button holds sneak instead of toggling it.
            Some(Control::Sneak) if self.vertical_movement || self.swimming => {
                self.clicked = true;
                self.key_events.push((HOLD_SNEAK.into(), true));
                Role::Hold(HOLD_SNEAK)
            }
            Some(Control::Sneak)
                if self.settings.mode == Mode::Dpad
                    && !self.vertical_movement
                    && !self.swimming
                    && !self.mounted =>
            {
                // The D-pad's centre crouches on a double tap.
                let now = Instant::now();
                if self
                    .last_sneak_tap
                    .take()
                    .is_some_and(|t| now.duration_since(t) < DOUBLE_TAP)
                {
                    Role::Button(Button::RightStick)
                } else {
                    self.last_sneak_tap = Some(now);
                    Role::Ignore
                }
            }
            Some(Control::Sprint) if self.settings.sprint_toggle => {
                self.sprint_latched = !self.sprint_latched;
                Role::Ignore
            }
            Some(c) => Self::button(c).map_or(Role::Ignore, Role::Button),
            // Meridian: a hotbar slot selects itself, as its number key does.
            None if let Some(slot) = self.hotbar_at(point) => {
                let key = format!("key.hotbar.{}", slot + 1);
                self.key_events.extend([(key.clone(), true), (key, false)]);
                Role::Hotbar {
                    since: Instant::now(),
                    dropped: false,
                }
            }
            None => Role::Look {
                start: point,
                since: Instant::now(),
                dragged: false,
                held_use: self.hold_uses_item,
                committed: false,
            },
        };
        if Self::button_can_pan(&role) {
            self.button_drag.insert(id, (point, false));
        }
        self.fingers.insert(id, (role, point));
        // Latch the press so a tap shorter than a tick still reaches the game.
        match role {
            Role::Button(Button::LeftTrigger) => self.pending.triggers[0] = 1.0,
            Role::Button(Button::RightTrigger) => self.pending.triggers[1] = 1.0,
            Role::Button(button) => self.pending.set(button, true),
            _ => {}
        }
        // Meridian: a button or a hotbar slot landed (the others set it where they act).
        self.clicked |= matches!(role, Role::Button(_) | Role::Hotbar { .. });
    }

    /// Whether a control landed since the last call: the game ticks the phone for it.
    pub fn take_clicked(&mut self) -> bool {
        std::mem::take(&mut self.clicked)
    }

    /// Meridian: whether `point` is in the D-pad's up arrow, a third of the radius off its anchor
    /// and no more sideways, as [`Self::state`] reads it.
    fn dpad_up_at(&self, anchor: [f32; 2], point: [f32; 2]) -> bool {
        let radius = self.placement(Control::Move).size / 2.0;
        (point[0] - anchor[0]).abs() * self.aspect() / radius < 1.0 / 3.0
            && (anchor[1] - point[1]) / radius >= 1.0 / 3.0
    }

    /// Meridian: lets go of the keys toggle buttons press (in [`Self::key_events`]).
    pub fn release_latched(&mut self) {
        for key in self.latched_keys.drain(..) {
            self.key_events.push((key_event(key), false));
        }
    }

    /// Lets go of everything: fingers, latched presses, requests. Held and toggled key buttons let
    /// their keys go (in [`Self::key_events`]).
    pub fn clear(&mut self) {
        self.release_latched();
        for (role, _) in self.fingers.values() {
            self.key_events.extend(release_event(role));
        }
        self.fingers.clear();
        self.button_drag.clear();
        self.landed.clear();
        self.last_sneak_tap = None;
        self.last_up_tap = None;
        self.dpad_sprint = None;
        self.sprint_latched = false;
        self.pending = PadState::default();
        self.aim = None;
        self.tap_aim = None;
        self.pending_tap = None;
        self.last_marker = None;
        self.holding_use = false;
        self.holding = false;
        self.chat_requested = false;
        self.settings_requested = false;
        self.debug_requested = false;
        self.perspective_requested = false;
    }

    /// A short world tap since the last call, and whether it uses the item.
    /// A tap never holds attack or use into the next tick: the game acts once
    /// at [`Self::aim`].
    pub fn take_tap(&mut self) -> Option<bool> {
        self.pending_tap.take()
    }

    /// Key or action presses for the game's key bindings since the last call.
    pub fn key_events(&mut self) -> Vec<(String, bool)> {
        std::mem::take(&mut self.key_events)
    }

    /// This tick's controller state, with presses that already lifted, and
    /// sets [`Self::aim`], [`Self::holding`] and [`Self::holding_use`].
    pub fn poll_state(&mut self) -> PadState {
        let mut state = self.state().merge(&std::mem::take(&mut self.pending));
        // Meridian: a hotbar slot held long enough drops one of its item, once per hold.
        for (role, _) in self.fingers.values_mut() {
            if let Role::Hotbar { since, dropped } = role
                && !*dropped
                && since.elapsed() >= DROP_HOLD
            {
                *dropped = true;
                self.key_events
                    .extend([("key.drop".into(), true), ("key.drop".into(), false)]);
            }
        }
        self.aim = self.tap_aim.take();
        self.holding_use = false;
        self.holding = false;
        if !self.world_gestures() {
            return state;
        }
        let mut hold = None;
        for (role, point) in self.fingers.values_mut() {
            if let Role::Look {
                since,
                dragged,
                held_use,
                committed,
                ..
            } = role
            {
                if !*dragged && since.elapsed() >= HOLD_DELAY {
                    *committed = true;
                }
                if !*dragged || *committed {
                    hold = Some((*point, *committed, *held_use));
                    if *committed {
                        break;
                    }
                }
            }
        }
        if let Some((point, committed, held_use)) = hold {
            self.aim = Some(self.aim_at(point));
            if committed {
                self.holding = true;
                self.holding_use = held_use;
                state.triggers[usize::from(!held_use)] = 1.0;
            }
        }
        state
    }

    /// Keeps the current hold using the item, even after the last one was
    /// eaten or the target changed.
    pub fn latch_use(&mut self) {
        if let Some((Role::Look { held_use, .. }, _)) = self.fingers.values_mut().find(|(r, _)| {
            matches!(
                r,
                Role::Look {
                    committed: true,
                    ..
                }
            )
        }) {
            *held_use = true;
        }
        self.holding_use = true;
    }

    /// The controller state the fingers hold right now.
    pub fn state(&self) -> PadState {
        let mut state = PadState::default();
        if self.settings.sprint_toggle && self.settings.sprint_button && self.sprint_latched {
            state.set(Button::LeftStick, true);
        }
        for (id, (role, point)) in &self.fingers {
            match role {
                Role::Move(anchor) => {
                    let radius = self.placement(Control::Move).size / 2.0;
                    let mut left = [
                        (point[0] - anchor[0]) * self.aspect() / radius,
                        (anchor[1] - point[1]) / radius,
                    ];
                    if self.settings.mode == Mode::Dpad {
                        left = left.map(|v| if v.abs() < 1.0 / 3.0 { 0.0 } else { v.signum() });
                    }
                    // Pushing forward past the joystick's rim sprints; coming
                    // back inside stops.
                    let sprint = left[1] > 1.0;
                    let length = left[0].hypot(left[1]).max(1.0);
                    state.left = left.map(|v| v / length);
                    if left[1] > 0.0 && self.dpad_sprint == Some(*id) {
                        state.set(Button::LeftStick, true);
                    }
                    if sprint
                        && self.settings.sprint_with_joystick
                        && self.settings.mode != Mode::Dpad
                    {
                        state.set(Button::LeftStick, true);
                    }
                }
                Role::Button(Button::LeftTrigger) | Role::Shield => state.triggers[0] = 1.0,
                Role::Button(Button::RightTrigger) => state.triggers[1] = 1.0,
                Role::Button(b) => state.set(*b, true),
                Role::Look { .. }
                | Role::Ignore
                | Role::Hotbar { .. }
                | Role::Key(_)
                | Role::Hold(_) => {}
            }
        }
        state
    }
}

#[cfg(test)]
mod tests;
