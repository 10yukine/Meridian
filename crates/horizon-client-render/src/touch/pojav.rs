// Copyright (c) 2026 Ludovic Marie and the Horizon contributors.
// SPDX-License-Identifier: MIT
//! Importer for Amethyst / PojavLauncher `CustomControls` JSON layouts (`version`, `mControlDataList`,
//! `mDrawerDataList`, `mJoystickDataList`) into Meridian's touch [`Settings`].

use super::layout::{self, Control, KeyButton, Mode, Placement, Settings};
use serde::Deserialize;
use std::path::Path;

const SCREEN_W: f32 = 1920.0;
const SCREEN_H: f32 = 1080.0;
const DENSITY: f32 = 3.0;
const MARGIN: f32 = 6.0;
const MAX_IMPORT_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Deserialize, Default)]
#[serde(default)]
struct CustomControls {
    version: i32,
    #[serde(rename = "scaledAt")]
    scaled_at: Option<f32>,
    #[serde(rename = "mControlDataList")]
    controls: Vec<ControlData>,
    #[serde(rename = "mDrawerDataList")]
    drawers: Vec<ControlDrawerData>,
    #[serde(rename = "mJoystickDataList")]
    joysticks: Vec<ControlJoystickData>,
}

#[derive(Clone, Deserialize, Default)]
#[serde(default)]
struct ControlData {
    name: String,
    #[serde(rename = "dynamicX")]
    dynamic_x: String,
    #[serde(rename = "dynamicY")]
    dynamic_y: String,
    #[serde(rename = "isToggle")]
    is_toggle: bool,
    keycodes: Vec<i32>,
    opacity: Option<f32>,
    width: Option<f32>,
    height: Option<f32>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ControlDrawerData {
    #[serde(rename = "buttonProperties")]
    button_properties: Vec<ControlData>,
    properties: ControlData,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ControlJoystickData {
    #[serde(flatten)]
    base: ControlData,
    #[serde(rename = "forwardLock")]
    forward_lock: bool,
    absolute: bool,
}

/// What [`import_json`] converted from a Pojav/Amethyst layout file.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportResult {
    pub settings: Settings,
    pub mapped_controls: usize,
    pub mapped_keys: usize,
}

/// Parses an Amethyst / Pojav `CustomControls` JSON document into Meridian [`Settings`].
pub fn import_json(json: &str) -> Result<ImportResult, String> {
    let parsed: CustomControls =
        serde_json::from_str(json).map_err(|e| format!("Invalid Pojav layout JSON: {e}"))?;
    if parsed.controls.is_empty()
        && parsed.drawers.is_empty()
        && parsed.joysticks.is_empty()
    {
        return Err("That file has no Pojav controls to import.".into());
    }

    let mut settings = Settings::default();
    let mut flat: Vec<ControlData> = parsed.controls;
    for drawer in parsed.drawers {
        flat.extend(drawer.button_properties);
    }

    let mut mapped_controls = 0usize;
    let mut mapped_keys = 0usize;

    let mut wasd_places = Vec::new();
    for ctrl in &flat {
        if ctrl.keycodes.iter().any(|&k| matches!(k, 87 | 65 | 83 | 68))
            && let Some(place) = control_placement(ctrl)
        {
            wasd_places.push(place);
        }
    }

    if let Some(joy) = parsed.joysticks.first() {
        settings.mode = Mode::Crosshair;
        settings.split_controls = false;
        settings.crosshair_gestures = false;
        settings.dynamic_joystick = !joy.absolute;
        if let Some(place) = control_placement(&joy.base) {
            let mode = Mode::Crosshair.index();
            settings.layouts[mode][Control::Move as usize] = place;
            mapped_controls += 1;
        }
    } else if !wasd_places.is_empty() {
        settings.mode = Mode::Dpad;
        settings.split_controls = true;
        settings.crosshair_gestures = true;
        let count = wasd_places.len() as f32;
        let cx = wasd_places.iter().map(|p| p.center[0]).sum::<f32>() / count;
        let cy = wasd_places.iter().map(|p| p.center[1]).sum::<f32>() / count;
        let avg_size = wasd_places.iter().map(|p| p.size).sum::<f32>() / count;
        let avg_opacity = wasd_places.iter().map(|p| p.opacity).sum::<f32>() / count;
        let mut dpad_place = Placement {
            center: [cx, cy],
            size: (avg_size * 2.6).clamp(0.25, 0.45),
            opacity: avg_opacity,
        };
        dpad_place.sanitize();
        settings.layouts[Mode::Dpad.index()][Control::Move as usize] = dpad_place;
        mapped_controls += 1;
    }

    let mode = settings.mode.index();
    let mut explicit_attack_or_use = false;
    for ctrl in &flat {
        let Some(place) = control_placement(ctrl) else {
            continue;
        };
        for &code in &ctrl.keycodes {
            if let Some(control) = map_builtin_control(code) {
                settings.layouts[mode][control as usize] = place;
                match control {
                    Control::Sprint => settings.sprint_toggle = ctrl.is_toggle,
                    Control::Inventory => settings.inventory_on_hotbar = false,
                    Control::Offhand => settings.offhand_button = true,
                    Control::Attack | Control::Use => explicit_attack_or_use = true,
                    _ => {}
                }
                mapped_controls += 1;
            } else if let Some(key) = map_key_button(code) {
                if !settings.keys[mode].iter().any(|existing| existing.key == key) {
                    settings.keys[mode].push(KeyButton {
                        key: key.to_owned(),
                        place,
                        toggle: ctrl.is_toggle,
                    });
                    mapped_keys += 1;
                }
            }
        }
    }
    if settings.mode == Mode::Dpad && explicit_attack_or_use {
        settings.crosshair_gestures = false;
    }

    settings.sanitize();
    Ok(ImportResult {
        settings,
        mapped_controls,
        mapped_keys,
    })
}

/// Reads a Pojav `CustomControls` JSON file (`<= 2 MiB`), saves it into
/// `<profile_dir>/touch-layouts/<slug>.json` and activates `<profile_dir>/touch-layout.json`.
pub fn import_file(path: &Path, profile_dir: &Path, display_name: &str) -> Result<String, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("Could not read layout file: {e}"))?;
    if meta.len() > MAX_IMPORT_BYTES {
        return Err("Touch layout file is larger than 2 MiB.".into());
    }
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("Touch layout file is not valid UTF-8 text: {e}"))?;
    let imported = import_json(&text)?;

    let stem = display_name
        .strip_suffix(".json")
        .or_else(|| display_name.strip_suffix(".JSON"))
        .unwrap_or(display_name);
    let slug = slugify(stem);
    let profiles_dir = profile_dir.join("touch-layouts");
    std::fs::create_dir_all(&profiles_dir)
        .map_err(|e| format!("Could not create touch-layouts folder: {e}"))?;
    let saved_profile = profiles_dir.join(format!("{slug}.json"));
    imported
        .settings
        .save(&saved_profile)
        .map_err(|e| format!("Could not save touch layout profile: {e}"))?;
    imported
        .settings
        .save(&profile_dir.join("touch-layout.json"))
        .map_err(|e| format!("Could not activate imported touch layout: {e}"))?;
    Ok(format!(
        "Imported {slug} ({} controls, {} keys)",
        imported.mapped_controls, imported.mapped_keys
    ))
}

fn slugify(name: &str) -> String {
    let mut slug = String::new();
    for c in name.trim().chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    let trimmed = slug.trim_end_matches('-');
    if trimmed.is_empty() {
        "pojav-layout".into()
    } else {
        trimmed.chars().take(48).collect()
    }
}

fn map_builtin_control(code: i32) -> Option<Control> {
    match code {
        32 => Some(Control::Jump),
        340 | 344 => Some(Control::Sneak),
        341 => Some(Control::Sprint),
        69 => Some(Control::Inventory),
        84 => Some(Control::Chat),
        256 | -9 => Some(Control::Pause),
        292 => Some(Control::Debug),
        294 => Some(Control::Perspective),
        70 => Some(Control::Offhand),
        -3 => Some(Control::Attack),
        -4 => Some(Control::Use),
        _ => None,
    }
}

fn map_key_button(code: i32) -> Option<&'static str> {
    let key = match code {
        -6 => "Pick",
        66 => "B",
        67 => "C",
        71 => "G",
        72 => "H",
        73 => "I",
        74 => "J",
        75 => "K",
        76 => "L",
        77 => "M",
        78 => "N",
        79 => "O",
        80 => "P",
        81 => "Q",
        82 => "R",
        85 => "U",
        86 => "V",
        88 => "X",
        89 => "Y",
        90 => "Z",
        291 => "F2",
        293 => "F4",
        295 => "F6",
        296 => "F7",
        297 => "F8",
        298 => "F9",
        299 => "F10",
        300 => "F11",
        301 => "F12",
        _ => return None,
    };
    layout::KEYS.contains(&key).then_some(key)
}

fn control_placement(ctrl: &ControlData) -> Option<Placement> {
    let width_px = ctrl.width.filter(|w| w.is_finite() && *w > 0.0).unwrap_or(50.0) * DENSITY;
    let height_px = ctrl
        .height
        .filter(|h| h.is_finite() && *h > 0.0)
        .unwrap_or(50.0)
        * DENSITY;
    let x_px = eval_dynamic(&ctrl.dynamic_x, width_px, height_px)?;
    let y_px = eval_dynamic(&ctrl.dynamic_y, width_px, height_px)?;
    let mut place = Placement {
        center: [(x_px + width_px * 0.5) / SCREEN_W, (y_px + height_px * 0.5) / SCREEN_H],
        size: width_px.max(height_px) / SCREEN_H,
        opacity: ctrl.opacity.filter(|o| o.is_finite()).unwrap_or(0.65),
    };
    place.sanitize();
    Some(place)
}

fn eval_dynamic(expr: &str, width_px: f32, height_px: f32) -> Option<f32> {
    let trimmed = expr.trim();
    if trimmed.is_empty() {
        return None;
    }
    let vars = [
        ("${top}", 0.0),
        ("${left}", 0.0),
        ("${right}", SCREEN_W - width_px),
        ("${bottom}", SCREEN_H - height_px),
        ("${width}", width_px),
        ("${height}", height_px),
        ("${screen_width}", SCREEN_W),
        ("${screen_height}", SCREEN_H),
        ("${margin}", MARGIN),
        ("${preferred_scale}", 1.0),
    ];
    let mut expanded = trimmed.to_owned();
    for (name, value) in vars {
        expanded = expanded.replace(name, &format!("{value}"));
    }
    let mut parser = ExprParser {
        bytes: expanded.as_bytes(),
        pos: 0,
    };
    let val = parser.parse_expr()?;
    parser.skip_ws();
    (parser.pos == parser.bytes.len() && val.is_finite()).then_some(val)
}

struct ExprParser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl ExprParser<'_> {
    fn skip_ws(&mut self) {
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.skip_ws();
        self.bytes.get(self.pos).copied()
    }

    fn parse_expr(&mut self) -> Option<f32> {
        let mut lhs = self.parse_term()?;
        while let Some(op @ (b'+' | b'-')) = self.peek() {
            self.pos += 1;
            let rhs = self.parse_term()?;
            lhs = if op == b'+' { lhs + rhs } else { lhs - rhs };
        }
        Some(lhs)
    }

    fn parse_term(&mut self) -> Option<f32> {
        let mut lhs = self.parse_unary()?;
        while let Some(op @ (b'*' | b'/')) = self.peek() {
            self.pos += 1;
            let rhs = self.parse_unary()?;
            lhs = if op == b'*' {
                lhs * rhs
            } else if rhs.abs() > f32::EPSILON {
                lhs / rhs
            } else {
                return None;
            };
        }
        Some(lhs)
    }

    fn parse_unary(&mut self) -> Option<f32> {
        match self.peek() {
            Some(b'-') => {
                self.pos += 1;
                Some(-self.parse_unary()?)
            }
            Some(b'+') => {
                self.pos += 1;
                self.parse_unary()
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Option<f32> {
        self.skip_ws();
        match self.bytes.get(self.pos).copied() {
            Some(b'(') => {
                self.pos += 1;
                let inner = self.parse_expr()?;
                if self.peek() != Some(b')') {
                    return None;
                }
                self.pos += 1;
                Some(inner)
            }
            Some(b'a'..=b'z' | b'A'..=b'Z') => {
                let start = self.pos;
                while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_alphanumeric() {
                    self.pos += 1;
                }
                let ident = std::str::from_utf8(&self.bytes[start..self.pos]).ok()?;
                if self.peek() != Some(b'(') {
                    return None;
                }
                self.pos += 1;
                let arg = self.parse_expr()?;
                if self.peek() != Some(b')') {
                    return None;
                }
                self.pos += 1;
                match ident {
                    "dp" => Some(arg / DENSITY),
                    "px" => Some(arg * DENSITY),
                    _ => None,
                }
            }
            Some(b'0'..=b'9' | b'.') => {
                let start = self.pos;
                let mut seen_dot = false;
                while self.pos < self.bytes.len() {
                    let c = self.bytes[self.pos];
                    if c.is_ascii_digit() {
                        self.pos += 1;
                    } else if c == b'.' && !seen_dot {
                        seen_dot = true;
                        self.pos += 1;
                    } else {
                        break;
                    }
                }
                std::str::from_utf8(&self.bytes[start..self.pos])
                    .ok()?
                    .parse::<f32>()
                    .ok()
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_dynamic_math_dp_px_and_variables() {
        let w = 50.0 * DENSITY;
        let h = 50.0 * DENSITY;
        let x = eval_dynamic("${right} - ${margin} * 2 - ${width}", w, h).unwrap();
        assert!((x - ((1920.0 - 150.0) - 12.0 - 150.0)).abs() < 0.01);
        let y = eval_dynamic("(${screen_height} - ${height}) / 2 + px(10) - dp(30)", w, h).unwrap();
        assert!((y - ((1080.0 - 150.0) / 2.0 + 30.0 - 10.0)).abs() < 0.01);
        assert_eq!(eval_dynamic("-5 + (2 * 3)", 100.0, 100.0), Some(1.0));
        assert_eq!(eval_dynamic("10 / 0", 100.0, 100.0), None);
        assert_eq!(eval_dynamic("${unknown}", 100.0, 100.0), None);
    }

    #[test]
    fn imports_pojav_default_v8_wasd_layout() {
        let json = r#"{
            "version": 8,
            "scaledAt": 100.0,
            "mControlDataList": [
                {"name": "F3", "keycodes": [292], "dynamicX": "${margin}", "dynamicY": "${margin}", "width": 80, "height": 30},
                {"name": "Chat", "keycodes": [84], "dynamicX": "${margin} * 2 + ${width}", "dynamicY": "${margin}", "width": 80, "height": 30},
                {"name": "Third", "keycodes": [294], "dynamicX": "${margin}", "dynamicY": "${height} + ${margin}", "width": 80, "height": 30},
                {"name": "W", "keycodes": [87], "dynamicX": "${margin} * 2 + ${width}", "dynamicY": "${bottom} - ${margin} * 3 - ${height} * 2"},
                {"name": "A", "keycodes": [65], "dynamicX": "${margin}", "dynamicY": "${bottom} - ${margin} * 2 - ${height}"},
                {"name": "S", "keycodes": [83], "dynamicX": "${margin} * 2 + ${width}", "dynamicY": "${bottom} - ${margin}"},
                {"name": "D", "keycodes": [68], "dynamicX": "${margin} * 3 + ${width} * 2", "dynamicY": "${bottom} - ${margin} * 2 - ${height}"},
                {"name": "Inv", "keycodes": [69], "dynamicX": "${margin} * 3 + ${width} * 2", "dynamicY": "${bottom} - ${margin}"},
                {"name": "Shift", "keycodes": [340], "dynamicX": "${margin} * 2 + ${width}", "dynamicY": "${screen_height} - ${margin} * 2 - ${height} * 2", "isToggle": true},
                {"name": "Jump", "keycodes": [32], "dynamicX": "${right} - ${margin} * 2 - ${width}", "dynamicY": "${bottom} - ${margin} * 2 - ${height}"}
            ]
        }"#;
        let res = import_json(json).unwrap();
        assert_eq!(res.settings.mode, Mode::Dpad);
        assert!(res.settings.split_controls);
        assert!(!res.settings.inventory_on_hotbar);
        assert!(res.mapped_controls >= 6);
    }

    #[test]
    fn imports_joystick_drawers_toggles_offhand_and_pick() {
        let json = r#"{
            "version": 8,
            "mJoystickDataList": [
                {"name": "Stick", "dynamicX": "200", "dynamicY": "600", "width": 120, "height": 120, "absolute": false}
            ],
            "mControlDataList": [
                {"name": "PRI", "keycodes": [-3], "dynamicX": "${right} - 100", "dynamicY": "${bottom} - 200", "opacity": 0.8},
                {"name": "SEC", "keycodes": [-4], "dynamicX": "${right} - 250", "dynamicY": "${bottom} - 200"},
                {"name": "Sprint", "keycodes": [341], "dynamicX": "${right} - 100", "dynamicY": "${bottom} - 400", "isToggle": true},
                {"name": "Swap", "keycodes": [70], "dynamicX": "${right} - 400", "dynamicY": "${bottom} - 100"},
                {"name": "Menu", "keycodes": [-9], "dynamicX": "900", "dynamicY": "20"}
            ],
            "mDrawerDataList": [
                {
                    "buttonProperties": [
                        {"name": "Mid", "keycodes": [-6], "dynamicX": "1400", "dynamicY": "400"},
                        {"name": "Zoom", "keycodes": [67], "dynamicX": "1550", "dynamicY": "400", "isToggle": true},
                        {"name": "F12", "keycodes": [301], "dynamicX": "1700", "dynamicY": "400"}
                    ]
                }
            ]
        }"#;
        let res = import_json(json).unwrap();
        assert_eq!(res.settings.mode, Mode::Crosshair);
        assert!(res.settings.dynamic_joystick);
        assert!(res.settings.sprint_toggle);
        assert!(res.settings.offhand_button);
        assert_eq!(res.mapped_keys, 3);
        let keys = res.settings.mode_keys();
        assert_eq!(keys[0].key, "Pick");
        assert!(!keys[0].toggle);
        assert_eq!(keys[1].key, "C");
        assert!(keys[1].toggle);
        assert_eq!(keys[2].key, "F12");
    }

    #[test]
    fn import_file_writes_profile_and_rejects_bad_or_empty_json() {
        assert!(import_json("{not json").is_err());
        assert!(import_json("{}").is_err());
        let dir = std::env::temp_dir().join(format!("meridian-pojav-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("My PvP.json");
        std::fs::write(
            &src,
            r#"{"mControlDataList":[{"name":"Jump","keycodes":[32],"dynamicX":"1500","dynamicY":"800"}]}"#,
        )
        .unwrap();
        let summary = import_file(&src, &dir, "My PvP.json").unwrap();
        assert!(summary.contains("my-pvp"));
        assert!(dir.join("touch-layouts/my-pvp.json").is_file());
        assert!(dir.join("touch-layout.json").is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
