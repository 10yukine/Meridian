// Adapted from Starling (https://github.com/starling-client/starling, `android` branch at
// 150a414b09a1, `crates/horizon-client-render/src/touch.rs`), written by Vasysik and the
// Starling contributors. Copyright (c) 2026 Ludovic Marie and the Horizon contributors, under the
// same MIT license as Horizon (LICENSE at the repository root).

//! Drawing the touch controls on the HUD, in GUI pixels. With Bedrock's control artwork downloaded
//! (`meridian_launcher::controls_art`) the gameplay controls take the MCPE look, the default; the
//! sprite choice is adapted from Starling's `crates/horizon-client-render/src/android_controls.rs`
//! at 5432370d5736. Otherwise buttons use the resource pack's own button sprite (`widget/button`,
//! highlighted while held) with a short label, and movement Meridian's geometric tiles.
use super::{Control, Mode, Placement, Role, TouchControls, layout};
use crate::gamepad::{Button, PadState};
use crate::hud::{HudAtlas, HudBatch, rgba};
use crate::lang;

const BUTTON: &str = "gui/sprites/widget/button";
const BUTTON_HELD: &str = "gui/sprites/widget/button_highlighted";
/// A sprite of Bedrock's control artwork: present, the controls take the MCPE look.
const MCPE: &str = "gui/bedrock/ui/jump";

fn mcpe(a: &HudAtlas) -> bool {
    a.sprites.contains_key(MCPE)
}

impl TouchControls {
    /// Draws every visible control, then the tap marker in the tap schemes.
    pub fn draw(&self, b: &mut HudBatch, a: &HudAtlas) {
        let state = self.state();
        for control in Control::ALL {
            if !self.visible(control) {
                continue;
            }
            let p = self.placement(control);
            let color = rgba(255, 255, 255, alpha(p.opacity));
            if control == Control::Move {
                self.draw_move(b, a, p, &state, color);
                continue;
            }
            let bounds = square(b, p.center, p.size);
            let rect = p.rect(self.aspect());
            let pressed = self.held(control, &state)
                || self
                    .fingers
                    .values()
                    .any(|(_, point)| layout::contains(rect, *point));
            self.draw_control(b, a, control, bounds, pressed, color);
        }
        if let Some(point) = self.pointer() {
            let center = [point[0] * b.width, point[1] * b.height];
            let radius = self.settings.circle_size / 2.0;
            ring(b, a, center, radius + 0.7, rgba(0, 0, 0, 110));
            ring(b, a, center, radius, rgba(255, 255, 255, 210));
            // Mining fills the circle outwards from its centre.
            let inner = mining_radius(radius, self.break_progress);
            if inner > 0.0 {
                disc(b, a, center, inner, rgba(255, 255, 255, 90));
                ring(b, a, center, inner, rgba(255, 255, 255, 220));
            }
        }
    }

    /// Draw the action button in both gameplay and the layout editor.
    pub(super) fn draw_control(
        &self,
        b: &mut HudBatch,
        a: &HudAtlas,
        control: Control,
        bounds: [f32; 4],
        pressed: bool,
        color: u32,
    ) {
        if mcpe(a) {
            let drew_sprite = self
                .mcpe_sprite(control)
                .is_some_and(|name| mcpe_sprite(b, a, name, pressed, bounds, color));
            if drew_sprite || mcpe_icon(b, a, control, bounds, pressed, color) {
                return;
            }
        }
        button(b, a, bounds, &self.label(control), pressed, color);
    }

    /// Whether a control shows as held: its button is down, or it latched.
    fn held(&self, control: Control, state: &PadState) -> bool {
        if control == Control::Sneak && self.sneaking {
            return true;
        }
        Self::button(control).is_some_and(|button| match button {
            Button::LeftTrigger => state.triggers[0] > 0.0,
            Button::RightTrigger => state.triggers[1] > 0.0,
            _ => state.held(button),
        })
    }

    /// The control's Bedrock sprite (the MCPE look), where Bedrock has one: the gameplay
    /// controls. The others keep their labelled buttons.
    fn mcpe_sprite(&self, control: Control) -> Option<&'static str> {
        let dpad = self.settings.mode == Mode::Dpad;
        Some(match control {
            Control::Sprint => "ui/sprint",
            Control::Attack => "ui/attack",
            Control::Use => "ui/interact",
            Control::Jump if self.swimming => "controls/waterascend",
            Control::Sneak if self.swimming => "controls/waterdescend",
            Control::Jump if self.vertical_movement => "controls/flyingascend",
            Control::Sneak if self.vertical_movement => "controls/flyingdescend",
            Control::Sneak if self.mounted => "controls/dismount",
            Control::Jump if dpad => "controls/jump_dpad",
            Control::Sneak if dpad => "controls/sneak_dpad",
            Control::Jump => "ui/jump",
            Control::Sneak => "ui/sneak",
            _ => return None,
        })
    }

    /// The label a control's button shows, in the player's language. Key
    /// names and symbols ("F3", "II") read the same in every language.
    fn label(&self, control: Control) -> String {
        let vertical = self.swimming || self.vertical_movement;
        let key = match control {
            Control::Move => return String::new(),
            Control::Pause => return "II".to_owned(),
            Control::Inventory => return "...".to_owned(),
            Control::Debug => return "F3".to_owned(),
            Control::Perspective => return "F5".to_owned(),
            Control::Sprint => "horizon.touch.sprint",
            Control::Jump if vertical => "horizon.touch.up",
            Control::Jump => "horizon.touch.jump",
            Control::Sneak if vertical => "horizon.touch.down",
            Control::Sneak if self.mounted => "horizon.touch.dismount",
            Control::Sneak => "horizon.touch.sneak",
            Control::Attack => "horizon.touch.attack",
            Control::Use => "horizon.touch.use",
            Control::Chat => "horizon.touch.chat",
            Control::Settings => "horizon.touch.settings",
            Control::Offhand => "horizon.touch.swap",
            Control::Shield => "horizon.touch.shield",
        };
        lang::get(key)
    }

    /// The joystick or the D-pad, with movement highlighted while held.
    fn draw_move(
        &self,
        b: &mut HudBatch,
        a: &HudAtlas,
        p: Placement,
        state: &PadState,
        color: u32,
    ) {
        let anchor = self.fingers.values().find_map(|(role, _)| match role {
            Role::Move(anchor) => Some(*anchor),
            _ => None,
        });
        let center = match anchor {
            Some(anchor) if self.dynamic_joystick() => anchor,
            // A dynamic joystick only shows while a finger holds it.
            None if self.dynamic_joystick() => return,
            _ => p.center,
        };
        let bounds = square(b, center, p.size);
        draw_movement(
            b,
            a,
            bounds,
            self.settings.mode,
            state.left,
            anchor.is_some(),
            color,
        );
    }
}

/// Draws movement for gameplay and the layout preview, with Meridian tiles as the fallback.
pub(super) fn draw_movement(
    b: &mut HudBatch,
    a: &HudAtlas,
    [x, y, size, _]: [f32; 4],
    mode: Mode,
    axis: [f32; 2],
    held: bool,
    color: u32,
) {
    let movement_sprites: &[&str] = if mode == Mode::Dpad {
        &[
            "gui/bedrock/controls/up",
            "gui/bedrock/controls/left",
            "gui/bedrock/controls/right",
            "gui/bedrock/controls/down",
        ]
    } else {
        &["gui/bedrock/ui/joystick_frame", "gui/bedrock/ui/joystick_knob"]
    };
    if mcpe(a)
        && movement_sprites
            .iter()
            .all(|name| a.sprites.contains_key(*name))
    {
        mcpe_movement(b, a, [x, y, size], mode, axis, color);
        return;
    }
    let opacity = color.to_le_bytes()[3];
    if mode == Mode::Dpad {
        let cell = size / 3.0;
        for ([dx, dy], label, pressed) in dpad(axis) {
            let center = [x + cell * (dx + 0.5), y + cell * (dy + 0.5)];
            movement_tile(b, a, center, (cell - 1.0).max(1.0), pressed, opacity);
            arrow(b, a, center, cell, label, color);
        }
        return;
    }
    let middle = [x + size / 2.0, y + size / 2.0];
    movement_tile(b, a, middle, size, false, opacity / 2);
    let knob = knob(middle, axis, size);
    movement_tile(b, a, knob, size * 0.36, held, opacity);
    let r = size * 0.065;
    let [x, y] = knob;
    b.fill_quad(a, [[x, y - r], [x + r, y], [x, y + r], [x - r, y]], color);
}

/// Bedrock's movement: its D-pad keys (the diagonals while a row's arrow is held, as Bedrock
/// shows them) or its joystick's frame and knob.
fn mcpe_movement(
    b: &mut HudBatch,
    a: &HudAtlas,
    [x, y, size]: [f32; 3],
    mode: Mode,
    axis: [f32; 2],
    color: u32,
) {
    if mode == Mode::Dpad {
        let cell = size / 3.0;
        for ([dx, dy], label, pressed) in dpad(axis) {
            let name = match label {
                "^" => "controls/up",
                "<" => "controls/left",
                ">" => "controls/right",
                _ => "controls/down",
            };
            mcpe_sprite(b, a, name, pressed, [x + cell * dx, y + cell * dy, cell, cell], color);
        }
        if axis[1].abs() > 0.2 {
            let (row, prefix) = if axis[1] > 0.0 { (0.0, "up") } else { (2.0, "down") };
            for (column, side, pressed) in
                [(0.0, "left", axis[0] < -0.2), (2.0, "right", axis[0] > 0.2)]
            {
                let name = format!("controls/{prefix}_{side}");
                let cell_at = [x + cell * column, y + cell * row, cell, cell];
                mcpe_sprite(b, a, &name, pressed, cell_at, color);
            }
        }
        return;
    }
    mcpe_sprite(b, a, "ui/joystick_frame", false, [x, y, size, size], color);
    let k = size * 0.34;
    let [kx, ky] = knob([x + size / 2.0, y + size / 2.0], axis, size);
    mcpe_sprite(b, a, "ui/joystick_knob", false, [kx - k / 2.0, ky - k / 2.0, k, k], color);
}

/// Bedrock's icon buttons (Meridian): the top row as Bedrock's framed icons in its `gui.png`
/// (the figure, as its emote button, for the camera's perspective), the touch settings as a gear
/// in the same frame, and the inventory as Bedrock's hotbar cell with "…". False for a control
/// Bedrock has no icon for.
fn mcpe_icon(
    b: &mut HudBatch,
    a: &HudAtlas,
    control: Control,
    bounds: [f32; 4],
    pressed: bool,
    color: u32,
) -> bool {
    let color = if pressed { dimmed(color) } else { color };
    let [x, y, w, h] = bounds;
    match control {
        Control::Perspective => gui_region(b, a, [236.0, 46.0, 18.0, 18.0], bounds, color),
        Control::Chat => gui_region(b, a, [200.0, 82.0, 18.0, 18.0], bounds, color),
        Control::Pause => gui_region(b, a, [200.0, 64.0, 18.0, 18.0], bounds, color),
        Control::Debug => gui_region(b, a, [236.0, 82.0, 18.0, 18.0], bounds, color),
        Control::Settings => {
            if !gui_region(b, a, [200.0, 64.0, 18.0, 18.0], bounds, color) {
                return false;
            }
            // The pause frame's face over its "≡", then the gear on it.
            let [_, _, _, opacity] = color.to_le_bytes();
            let face = if pressed { 90 } else { 128 };
            let texel = w / 18.0;
            b.fill(a, x + 2.0 * texel, y + 2.0 * texel, 14.0 * texel, 12.0 * texel, rgba(face, face, face, opacity));
            mcpe_sprite(b, a, "ui/gear", false, [x + 3.0 * texel, y + 2.0 * texel, 12.0 * texel, 12.0 * texel], color);
            true
        }
        Control::Inventory => {
            if !gui_region(b, a, [160.0, 0.0, 22.0, 22.0], bounds, color) {
                return false;
            }
            mcpe_sprite(b, a, "ui/more-dots", false, [x + w * 0.2, y + h * 0.3, w * 0.6, h * 0.4], color);
            true
        }
        _ => false,
    }
}

/// Part of Bedrock's `gui.png` (texels `[x, y, w, h]`) drawn into `bounds`; false without it.
fn gui_region(
    b: &mut HudBatch,
    a: &HudAtlas,
    [sx, sy, sw, sh]: [f32; 4],
    [x, y, w, h]: [f32; 4],
    color: u32,
) -> bool {
    let Some(mut r) = a.sprites.get("gui/bedrock/gui").copied() else {
        return false;
    };
    let (u, v) = (r.u0, r.v0);
    let (du, dv) = ((r.u1 - u) / r.w as f32, (r.v1 - v) / r.h as f32);
    // A quarter texel in from each edge: the icons sit edge to edge in the sheet.
    r.u0 = u + (sx + 0.25) * du;
    r.u1 = u + (sx + sw - 0.25) * du;
    r.v0 = v + (sy + 0.25) * dv;
    r.v1 = v + (sy + sh - 0.25) * dv;
    b.rect_uv(x, y, w, h, r, color);
    true
}

/// A held icon, darker.
fn dimmed(color: u32) -> u32 {
    let [r, g, blue, opacity] = color.to_le_bytes();
    let dim = |c: u8| (u16::from(c) * 7 / 10) as u8;
    rgba(dim(r), dim(g), dim(blue), opacity)
}

/// A Bedrock sprite fitted into `bounds`, its pressed look while held where it has one.
fn mcpe_sprite(
    b: &mut HudBatch,
    a: &HudAtlas,
    name: &str,
    pressed: bool,
    [x, y, w, h]: [f32; 4],
    color: u32,
) -> bool {
    let held = format!("gui/bedrock/{name}_pressed");
    let sprite = if pressed && a.sprites.contains_key(&held) {
        held
    } else {
        format!("gui/bedrock/{name}")
    };
    let Some(r) = a.sprites.get(&sprite).copied() else {
        return false;
    };
    let scale = (w / r.w as f32).min(h / r.h as f32);
    let (sw, sh) = (r.w as f32 * scale, r.h as f32 * scale);
    b.sprite_scaled(a, &sprite, x + (w - sw) / 2.0, y + (h - sh) / 2.0, sw, sh, color);
    true
}

/// A frosted square with stepped corners and a short highlight on its top edge.
fn movement_tile(
    b: &mut HudBatch,
    a: &HudAtlas,
    [x, y]: [f32; 2],
    size: f32,
    pressed: bool,
    opacity: u8,
) {
    let edge = (size / 16.0).max(1.0).min(size / 6.0);
    let x = x - size / 2.0;
    let y = y - size / 2.0;
    let outline = rgba(17, 24, 39, opacity);
    let face = if pressed {
        rgba(83, 115, 147, opacity)
    } else {
        rgba(37, 49, 68, opacity / 2)
    };
    b.fill(a, x + edge, y, size - 2.0 * edge, size, outline);
    b.fill(a, x, y + edge, edge, size - 2.0 * edge, outline);
    b.fill(
        a,
        x + size - edge,
        y + edge,
        edge,
        size - 2.0 * edge,
        outline,
    );
    b.fill(
        a,
        x + edge,
        y + edge,
        size - 2.0 * edge,
        size - 2.0 * edge,
        face,
    );
    b.fill(
        a,
        x + 3.0 * edge,
        y + edge,
        size - 6.0 * edge,
        edge,
        rgba(169, 207, 225, opacity),
    );
}

fn arrow(b: &mut HudBatch, a: &HudAtlas, [x, y]: [f32; 2], cell: f32, direction: &str, color: u32) {
    let step = (cell / 10.0).max(1.0);
    let at = |u: f32, v: f32| {
        let [u, v] = match direction {
            "<" => [v, -u],
            ">" => [-v, u],
            "v" => [-u, -v],
            _ => [u, v],
        };
        [x + u * step, y + v * step]
    };
    // Chevron formed from two stepped arms, with its tip filled only once.
    for [u, v] in [
        [-2.0, 1.0],
        [-1.0, 0.0],
        [0.0, -1.0],
        [1.0, 0.0],
        [2.0, 1.0],
    ] {
        let [left, top] = at(u, v);
        b.fill(a, left - step / 2.0, top - step / 2.0, step, step, color);
    }
}

/// The D-pad's arrows: each one's cell in a 3x3 grid, its label and whether
/// the stick (`left`, up positive) presses it.
fn dpad(left: [f32; 2]) -> [([f32; 2], &'static str, bool); 4] {
    [
        ([1.0, 0.0], "^", left[1] > 0.2),
        ([0.0, 1.0], "<", left[0] < -0.2),
        ([2.0, 1.0], ">", left[0] > 0.2),
        ([1.0, 2.0], "v", left[1] < -0.2),
    ]
}

/// Where the joystick's knob sits: the stick is up positive, the screen down.
fn knob(middle: [f32; 2], left: [f32; 2], size: f32) -> [f32; 2] {
    [
        middle[0] + left[0] * size / 4.0,
        middle[1] - left[1] * size / 4.0,
    ]
}

/// A placement's square in GUI pixels, `[x, y, size, size]`.
fn square(b: &HudBatch, center: [f32; 2], size: f32) -> [f32; 4] {
    let size = size * b.height;
    [
        center[0] * b.width - size / 2.0,
        center[1] * b.height - size / 2.0,
        size,
        size,
    ]
}

fn alpha(opacity: f32) -> u8 {
    (opacity.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// A labelled button: the pack's button sprite, or a plain frame when the
/// pack has none. A held button darkens and its label drops a pixel.
fn button(
    b: &mut HudBatch,
    a: &HudAtlas,
    bounds: [f32; 4],
    label: &str,
    pressed: bool,
    color: u32,
) {
    let [x, y, w, h] = bounds;
    let [r, g, blue, opacity] = color.to_le_bytes();
    if a.sprites.contains_key(BUTTON) {
        let sprite = if pressed && a.sprites.contains_key(BUTTON_HELD) {
            BUTTON_HELD
        } else {
            BUTTON
        };
        crate::ui::nine_slice(b, a, sprite, x, y, w, h, color);
    } else {
        let fill = if pressed { 150 } else { 90 };
        b.fill(
            a,
            x,
            y,
            w,
            h,
            rgba(0, 0, 0, (u16::from(opacity) * fill / 255) as u8),
        );
        for edge in [
            [x, y, w, 1.0],
            [x, y + h - 1.0, w, 1.0],
            [x, y, 1.0, h],
            [x + w - 1.0, y, 1.0, h],
        ] {
            b.fill(a, edge[0], edge[1], edge[2], edge[3], color);
        }
    }
    if label.is_empty() {
        return;
    }
    let text = if pressed {
        rgba(r / 2 + 127, g / 2 + 127, blue / 2, opacity)
    } else {
        color
    };
    let drop = if pressed { (h / 18.0).min(2.0) } else { 0.0 };
    let width = HudBatch::text_width(a, label);
    let start = b.vertices.len();
    b.text(
        a,
        label,
        x + (w - width) / 2.0,
        y + (h - 8.0) / 2.0 + drop,
        text,
        true,
    );
    // Shrink a long label to fit, never grow a short one: the width leaves 3
    // pixels each side, and the height cap keeps the 8-pixel line inside the
    // same margin, never above full size.
    let fit_width = (w - 6.0).max(1.0) / width.max(1.0);
    let height_cap = ((h - 6.0).max(1.0) / 8.0).min(1.0);
    let scale = fit_width.min(height_cap);
    b.scale_since(start, x + w / 2.0, y + h / 2.0 + drop, scale, scale);
}

/// How far the mining disc reaches inside a marker of `radius`, by progress.
fn mining_radius(radius: f32, progress: f32) -> f32 {
    if progress.is_finite() {
        (radius - 1.5).max(0.0) * progress.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// 65 points round the unit circle, the last one equal to the first.
fn circle_points() -> &'static [[f32; 2]; 65] {
    static POINTS: std::sync::OnceLock<[[f32; 2]; 65]> = std::sync::OnceLock::new();
    POINTS.get_or_init(|| {
        std::array::from_fn(|i| {
            let angle = i as f32 / 64.0 * std::f32::consts::TAU;
            [angle.cos(), angle.sin()]
        })
    })
}

/// A 1.4-pixel circle line, one quad per segment so nothing blends twice.
fn ring(b: &mut HudBatch, a: &HudAtlas, center: [f32; 2], radius: f32, color: u32) {
    let at = |p: [f32; 2], r: f32| [center[0] + p[0] * r, center[1] + p[1] * r];
    let (outer, inner) = (radius + 0.7, (radius - 0.7).max(0.0));
    for s in circle_points().windows(2) {
        b.fill_quad(
            a,
            [
                at(s[0], outer),
                at(s[1], outer),
                at(s[1], inner),
                at(s[0], inner),
            ],
            color,
        );
    }
}

/// A filled circle.
fn disc(b: &mut HudBatch, a: &HudAtlas, center: [f32; 2], radius: f32, color: u32) {
    let at = |p: [f32; 2]| [center[0] + p[0] * radius, center[1] + p[1] * radius];
    for s in circle_points().windows(2) {
        b.fill_quad(a, [center, at(s[0]), at(s[1]), center], color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atlas() -> HudAtlas {
        HudAtlas::build(&horizon_assets::resource_source::ResourceSource::directory(
            std::path::PathBuf::from("/nonexistent/horizon-touch-draw"),
        ))
        .unwrap()
    }

    fn vertices(t: &TouchControls, a: &HudAtlas) -> usize {
        let mut b = HudBatch::new(
            [1920, 1080],
            crate::hud::GuiScale {
                requested: 0,
                unicode: false,
            },
        );
        t.draw(&mut b, a);
        b.vertices.len()
    }

    #[test]
    fn mining_disc_grows_outwards_within_the_marker() {
        assert_eq!(mining_radius(16.0, 0.0), 0.0);
        assert_eq!(mining_radius(16.0, 0.5), 7.25);
        assert_eq!(mining_radius(16.0, 1.0), 14.5);
        assert_eq!(mining_radius(16.0, 2.0), 14.5);
        assert_eq!(mining_radius(16.0, -1.0), 0.0);
        assert_eq!(mining_radius(16.0, f32::NAN), 0.0);
    }

    #[test]
    fn hidden_controls_draw_nothing() {
        let a = atlas();
        let mut t = TouchControls::default();
        assert!(vertices(&t, &a) > 0);
        t.settings.button_visibility = [layout::HIDDEN; layout::CONTROLS];
        assert_eq!(vertices(&t, &a), 0);
    }

    #[test]
    fn a_dynamic_joystick_shows_only_while_held() {
        let a = atlas();
        let mut t = TouchControls::default();
        t.settings.dynamic_joystick = true;
        let idle = vertices(&t, &a);
        t.update(1, [0.1, 0.6], true, false);
        assert!(vertices(&t, &a) > idle);
        t.update(1, [0.1, 0.6], false, true);
        assert_eq!(vertices(&t, &a), idle);
    }

    #[test]
    fn the_marker_shows_in_tap_schemes_and_fills_while_mining() {
        let a = atlas();
        let mut t = TouchControls::default();
        let idle = vertices(&t, &a);
        t.update(1, [0.6, 0.4], true, false);
        assert_eq!(vertices(&t, &a), idle, "no marker with a crosshair");
        t.clear();
        t.settings.mode = Mode::Tap;
        let idle = vertices(&t, &a);
        t.update(1, [0.6, 0.4], true, false);
        let marker = vertices(&t, &a);
        assert!(marker > idle);
        t.break_progress = 0.5;
        assert!(vertices(&t, &a) > marker);
    }

    fn english<R>(f: impl FnOnce() -> R) -> R {
        let (result, missed) = lang::scoped(lang::Language::load(&[], "en_us"), f);
        assert!(missed.is_empty(), "no English text for {missed:?}");
        result
    }

    #[test]
    fn labels_and_held_buttons_follow_the_player() {
        let mut t = TouchControls::default();
        let all = |t: &TouchControls| english(|| Control::ALL.map(|c| t.label(c)));
        assert_eq!(all(&t)[..3], ["", "Sprint", "Jump"]);
        assert!(all(&t).iter().all(|l| !l.starts_with("horizon.")));
        assert_eq!(t.label(Control::Debug), "F3");
        t.swimming = true;
        assert_eq!(english(|| t.label(Control::Jump)), "Up");
        assert_eq!(english(|| t.label(Control::Sneak)), "Down");
        t.swimming = false;
        t.mounted = true;
        assert_eq!(english(|| t.label(Control::Sneak)), "Dismount");
        let state = t.state();
        assert!(!t.held(Control::Sneak, &state));
        t.sneaking = true;
        assert!(t.held(Control::Sneak, &state));
        t.update(1, t.placement(Control::Attack).center, true, false);
        assert!(t.held(Control::Attack, &t.state()));
        assert!(!t.held(Control::Use, &t.state()));
    }

    #[test]
    fn colours_split_into_rgba_in_order() {
        let half = rgba(255, 255, 255, alpha(0.5));
        assert_eq!(half.to_le_bytes(), [255, 255, 255, 128]);
        assert_eq!(rgba(10, 20, 30, 40).to_le_bytes(), [10, 20, 30, 40]);
    }

    /// One button drawn alone, as `button` draws it in `draw`.
    fn one(a: &HudAtlas, bounds: [f32; 4], label: &str, pressed: bool) -> HudBatch {
        let mut b = HudBatch::new(
            [1920, 1080],
            crate::hud::GuiScale {
                requested: 0,
                unicode: false,
            },
        );
        button(&mut b, a, bounds, label, pressed, rgba(255, 255, 255, 128));
        b
    }

    #[test]
    fn a_held_button_uses_the_packs_highlighted_sprite() {
        let mut a = atlas();
        let rect = |u0: f32| crate::hud::Rect {
            u0,
            v0: 0.0,
            u1: u0 + 0.1,
            v1: 0.1,
            w: 200,
            h: 20,
        };
        a.sprites.insert(BUTTON.to_owned(), rect(0.2));
        a.sprites.insert(BUTTON_HELD.to_owned(), rect(0.5));
        let uses = |b: &HudBatch, u0: f32| {
            b.vertices
                .iter()
                .any(|v| (u0..=u0 + 0.1).contains(&v.uv[0]) && v.uv[1] <= 0.1)
        };
        let idle = one(&a, [0.0, 0.0, 60.0, 20.0], "", false);
        assert!(uses(&idle, 0.2) && !uses(&idle, 0.5));
        let held = one(&a, [0.0, 0.0, 60.0, 20.0], "", true);
        assert!(uses(&held, 0.5) && !uses(&held, 0.2));
        assert!(
            held.vertices
                .iter()
                .all(|v| v.color == rgba(255, 255, 255, 128))
        );
    }

    #[test]
    fn without_the_sprite_a_plain_frame_darkens_while_held() {
        let a = atlas();
        assert!(!a.sprites.contains_key(BUTTON));
        let colours = |pressed| {
            let b = one(&a, [10.0, 10.0, 60.0, 20.0], "", pressed);
            b.vertices.iter().map(|v| v.color).collect::<Vec<_>>()
        };
        let (idle, held) = (colours(false), colours(true));
        let mut fill = HudBatch::new(
            [1920, 1080],
            crate::hud::GuiScale {
                requested: 0,
                unicode: false,
            },
        );
        fill.fill(&a, 0.0, 0.0, 1.0, 1.0, 0);
        let quad = fill.vertices.len();
        // The backdrop, then the four one-pixel edges in the button's colour.
        assert_eq!(idle.len(), 5 * quad);
        assert!(idle[..quad].iter().all(|&c| c == rgba(0, 0, 0, 45)));
        assert!(held[..quad].iter().all(|&c| c == rgba(0, 0, 0, 75)));
        assert!(idle[quad..].iter().all(|&c| c == rgba(255, 255, 255, 128)));
    }

    #[test]
    fn a_long_label_shrinks_to_fit_and_a_short_one_keeps_its_size() {
        let a = atlas();
        let frame = one(&a, [0.0, 0.0, 40.0, 20.0], "", false).vertices.len();
        let span = |label: &str| {
            let b = one(&a, [0.0, 0.0, 40.0, 20.0], label, false);
            assert!(b.vertices.len() > frame, "{label} drew no text");
            let xs = b.vertices[frame..].iter().map(|v| v.pos[0]);
            let (lo, hi) = xs.fold((f32::MAX, f32::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)));
            (lo, hi)
        };
        let long = "a label much wider than its button";
        assert!(HudBatch::text_width(&a, long) > 40.0);
        let (lo, hi) = span(long);
        assert!(lo >= 3.0 - 1e-3 && hi <= 37.0 + 1e-3, "{lo}..{hi}");
        let width = HudBatch::text_width(&a, "Up");
        let (lo, hi) = span("Up");
        assert!(hi - lo >= width - 1e-3, "{} < {width}", hi - lo);
    }

    #[test]
    fn the_dpad_and_knob_follow_the_stick_up_positive() {
        let only = |left: [f32; 2]| {
            let on: Vec<_> = dpad(left).into_iter().filter(|p| p.2).collect();
            assert_eq!(on.len(), 1, "{left:?}");
            (on[0].0, on[0].1)
        };
        assert_eq!(only([0.0, 0.5]), ([1.0, 0.0], "^"));
        assert_eq!(only([0.0, -0.5]), ([1.0, 2.0], "v"));
        assert_eq!(only([-0.5, 0.0]), ([0.0, 1.0], "<"));
        assert_eq!(only([0.5, 0.0]), ([2.0, 1.0], ">"));
        assert!(dpad([0.1, -0.1]).iter().all(|p| !p.2));
        // Stick up draws the knob above the middle, on screen.
        assert_eq!(knob([50.0, 50.0], [0.0, 1.0], 40.0), [50.0, 40.0]);
        assert_eq!(knob([50.0, 50.0], [1.0, 0.0], 40.0), [60.0, 50.0]);
    }
}
