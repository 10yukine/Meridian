// Copyright (c) 2026 Ludovic Marie and the Horizon contributors.
// SPDX-License-Identifier: MIT
//! Touch layout editing adapted from Starling's touch editor. Preview and input
//! share Meridian's existing placements, visibility rules, and layout storage.

use super::draw::key_button;
use super::{Control, Mode, Placement, Settings, TouchControls, layout, profiles};
use crate::hud::{HudAtlas, HudBatch, rgba};
use std::path::Path;
use winit::event::TouchPhase;

pub const FILE: &str = "touch-layout.json";
const CANVAS: [f32; 4] = [0.15, 0.16, 0.70, 0.70];

/// Meridian North Star palette (matching `mobile_launcher::widgets::palette`).
mod theme {
    use crate::hud::rgba;

    pub const SCRIM: u32 = rgba(10, 10, 12, 204);
    pub const MODAL_SCRIM: u32 = rgba(6, 6, 9, 178);
    pub const CARD: u32 = rgba(18, 18, 21, 246);
    pub const ROW_CARD: u32 = rgba(20, 19, 26, 224);
    pub const FIELD_BG: u32 = rgba(12, 12, 16, 255);
    pub const BORDER: u32 = rgba(46, 46, 51, 225);
    pub const BORDER_PURPLE: u32 = rgba(78, 70, 114, 225);
    pub const ACCENT_BAR: u32 = rgba(157, 144, 234, 235);
    pub const PRIMARY: u32 = rgba(109, 94, 209, 248);
    pub const PRIMARY_TOP: u32 = rgba(177, 167, 242, 230);
    pub const SECONDARY: u32 = rgba(28, 27, 36, 245);
    pub const SECONDARY_TOP: u32 = rgba(157, 144, 234, 125);
    pub const DANGER: u32 = rgba(166, 52, 68, 246);
    pub const DANGER_TOP: u32 = rgba(235, 130, 145, 215);
    pub const MUTED_BTN: u32 = rgba(20, 20, 24, 200);
    pub const MUTED_BORDER: u32 = rgba(46, 46, 51, 140);
    pub const MUTED_TEXT: u32 = rgba(161, 161, 170, 235);
    pub const ERROR_TEXT: u32 = rgba(242, 124, 138, 255);
    pub const ACCENT_TEXT: u32 = rgba(192, 184, 246, 255);
    pub const SELECT_OUTLINE: u32 = rgba(157, 144, 234, 245);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    Standard,
    Dpad,
    Edit,
    Save,
    Cancel,
    Presets,
    Reset,
    Smaller,
    Larger,
    Fainter,
    Brighter,
    /// Meridian: opens the key picker; [`Action::Pick`] adds [`layout::KEYS`]'s key `i`.
    Add,
    Pick(usize),
    Back,
    /// Meridian: removes the selected key button.
    Remove,
    /// Meridian: switches the selected key button between hold and toggle.
    Mode,
    /// Meridian: opens the settings page; [`Action::Back`] returns to the hub.
    Settings,
    /// Meridian: opens the layout profiles page; [`Action::PickProfile`] applies one.
    Profiles,
    PickProfile(usize),
    DeleteProfile(usize),
    NewProfile,
    /// Meridian: a settings page switch flips.
    Toggle(Row),
    /// Meridian: a settings page value goes up (true) or down a step.
    Step(Row, bool),
    /// Popup actions.
    PopupClose,
    ConfirmReset,
    ConfirmDiscard,
    SaveAsDefault,
    SaveToProfiles,
    ConfirmOverwrite,
    ConfirmDelete,
    FocusName,
    SubmitNewProfile,
}

const HUB_CARD: [f32; 4] = [0.24, 0.14, 0.52, 0.72];

const HUB: [(Action, [f32; 4], &str); 5] = [
    (Action::Edit, [0.29, 0.27, 0.42, 0.09], "Edit layout"),
    (
        Action::Profiles,
        [0.29, 0.38, 0.42, 0.09],
        "Layout profiles",
    ),
    (Action::Settings, [0.29, 0.49, 0.42, 0.09], "Settings"),
    (Action::Cancel, [0.29, 0.65, 0.20, 0.09], "Cancel"),
    (Action::Save, [0.51, 0.65, 0.20, 0.09], "Save"),
];

/// Meridian: a row of the settings page, a switch or a value with − and +.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Row {
    Sensitivity,
    FloatingJoystick,
    AutoSprint,
    SprintToggle,
    ButtonScale,
    OffhandButton,
    AttackDragLook,
    LeftHanded,
    SwapJumpSneak,
    Vibration,
    ButtonVibration,
    Gyro,
    GyroSpeed,
}

impl Row {
    const ALL: [Row; 13] = [
        Row::Sensitivity,
        Row::FloatingJoystick,
        Row::AutoSprint,
        Row::SprintToggle,
        Row::ButtonScale,
        Row::OffhandButton,
        Row::AttackDragLook,
        Row::LeftHanded,
        Row::SwapJumpSneak,
        Row::Vibration,
        Row::ButtonVibration,
        Row::Gyro,
        Row::GyroSpeed,
    ];

    /// The rows the scheme uses, in order.
    fn shown(s: &Settings) -> Vec<Row> {
        Self::ALL
            .into_iter()
            .filter(|row| match row {
                Row::FloatingJoystick | Row::AutoSprint => s.mode != Mode::Dpad,
                Row::AttackDragLook => s.mode == Mode::Crosshair,
                Row::SwapJumpSneak => s.mode == Mode::Dpad,
                _ => true,
            })
            .collect()
    }

    fn name(self) -> &'static str {
        match self {
            Row::Sensitivity => "Look sensitivity",
            Row::FloatingJoystick => "Floating joystick",
            Row::AutoSprint => "Auto-sprint",
            Row::SprintToggle => "Sprint toggles",
            Row::ButtonScale => "Button scale",
            Row::OffhandButton => "Offhand button",
            Row::AttackDragLook => "Button drags look",
            Row::LeftHanded => "Left-handed",
            Row::SwapJumpSneak => "Swap jump and sneak",
            Row::Vibration => "Vibration",
            Row::ButtonVibration => "Vibrate on press",
            Row::Gyro => "Gyro aiming",
            Row::GyroSpeed => "Gyro speed",
        }
    }

    /// A switch's state; none for a value.
    fn switch(self, s: &Settings) -> Option<bool> {
        match self {
            Row::Sensitivity | Row::ButtonScale | Row::GyroSpeed => None,
            Row::FloatingJoystick => Some(s.dynamic_joystick),
            Row::AutoSprint => Some(s.sprint_with_joystick),
            Row::SprintToggle => Some(s.sprint_toggle),
            Row::OffhandButton => Some(s.offhand_button),
            Row::AttackDragLook => Some(s.attack_button_look),
            Row::LeftHanded => Some(s.left_handed),
            Row::SwapJumpSneak => Some(s.swap_jump_sneak),
            Row::Vibration => Some(s.vibration),
            Row::ButtonVibration => Some(s.button_vibration),
            Row::Gyro => Some(s.gyro),
        }
    }

    fn flip(self, s: &mut Settings) {
        let flag = match self {
            Row::Sensitivity | Row::ButtonScale | Row::GyroSpeed => return,
            Row::FloatingJoystick => &mut s.dynamic_joystick,
            Row::AutoSprint => &mut s.sprint_with_joystick,
            Row::SprintToggle => &mut s.sprint_toggle,
            Row::OffhandButton => &mut s.offhand_button,
            Row::AttackDragLook => &mut s.attack_button_look,
            Row::LeftHanded => &mut s.left_handed,
            Row::SwapJumpSneak => &mut s.swap_jump_sneak,
            Row::Vibration => &mut s.vibration,
            Row::ButtonVibration => &mut s.button_vibration,
            Row::Gyro => &mut s.gyro,
        };
        *flag = !*flag;
    }

    /// A value row's number; none for a switch.
    fn number(self, s: &Settings) -> Option<f32> {
        match self {
            Row::Sensitivity => Some(s.sensitivity),
            Row::ButtonScale => Some(s.button_scale),
            Row::GyroSpeed => Some(s.gyro_sensitivity),
            _ => None,
        }
    }

    fn set_number(self, s: &mut Settings, v: f32) {
        match self {
            Row::Sensitivity => s.sensitivity = v,
            Row::ButtonScale => s.button_scale = v,
            Row::GyroSpeed => s.gyro_sensitivity = v,
            _ => {}
        }
    }

    /// A value row's step and range, as `Settings::sanitize` keeps it.
    fn range(self) -> (f32, [f32; 2]) {
        match self {
            Row::ButtonScale => (0.1, [0.5, 2.0]),
            _ => (0.1, [0.1, 3.0]),
        }
    }

    fn can_step(self, s: &Settings, up: bool) -> bool {
        let (_, [low, high]) = self.range();
        self.number(s).is_some_and(|v| {
            if up {
                v < high - 0.001
            } else {
                v > low + 0.001
            }
        })
    }

    fn step(self, s: &mut Settings, up: bool) {
        let Some(v) = self.number(s) else { return };
        let (step, [low, high]) = self.range();
        // Whole steps, so ten taps up and ten down land where they started.
        let next = ((v / step).round() + if up { 1.0 } else { -1.0 }) * step;
        self.set_number(s, next.clamp(low, high));
    }

    fn text(self, s: &Settings) -> String {
        match (self.switch(s), self.number(s)) {
            (Some(true), _) => "On".into(),
            (Some(false), _) => "Off".into(),
            (None, Some(v)) => format!("{:.0}%", v * 100.0),
            (None, None) => String::new(),
        }
    }
}

/// Meridian: the settings / profiles row `i` of `n`: one centred column up to six, else two.
fn row_rect(i: usize, n: usize) -> [f32; 4] {
    let (column, row) = ((i / 6) as f32, (i % 6) as f32);
    let x = if n > 6 { 0.06 + column * 0.45 } else { 0.285 };
    [x, 0.13 + row * 0.112, 0.43, 0.098]
}
fn row_switch([x, y, w, h]: [f32; 4]) -> [f32; 4] {
    [x + w - 0.14, y + 0.015, 0.13, h - 0.03]
}
fn row_minus([x, y, w, h]: [f32; 4]) -> [f32; 4] {
    [x + w - 0.20, y + 0.015, 0.05, h - 0.03]
}
fn row_number([x, y, w, h]: [f32; 4]) -> [f32; 4] {
    [x + w - 0.15, y, 0.09, h]
}
fn row_plus([x, y, w, h]: [f32; 4]) -> [f32; 4] {
    [x + w - 0.06, y + 0.015, 0.05, h - 0.03]
}
fn profile_select_rect([x, y, w, h]: [f32; 4]) -> [f32; 4] {
    [x, y, w - 0.068, h]
}
fn profile_delete_rect([x, y, w, h]: [f32; 4]) -> [f32; 4] {
    [x + w - 0.058, y + 0.012, 0.050, h - 0.024]
}
/// Meridian: the settings / profiles page's way back.
const PAGE_BACK: [f32; 4] = [0.42, 0.86, 0.16, 0.09];

/// Top bar (4 buttons flanking centered title) and center-justified bottom bar (7 buttons, total
/// width `7 * 0.115 + 6 * 0.010 = 0.865`, margins `0.0675` each side).
const EDITOR: [(Action, [f32; 4], &str); 11] = [
    (Action::Presets, [0.02, 0.04, 0.13, 0.09], "Presets"),
    (Action::Reset, [0.165, 0.04, 0.145, 0.09], "Reset layout"),
    (Action::Cancel, [0.69, 0.04, 0.13, 0.09], "Cancel"),
    (Action::Save, [0.835, 0.04, 0.145, 0.09], "Save"),
    (Action::Smaller, [0.0675, 0.89, 0.115, 0.09], "Size −"),
    (Action::Larger, [0.1925, 0.89, 0.115, 0.09], "Size +"),
    (Action::Fainter, [0.3175, 0.89, 0.115, 0.09], "Opacity −"),
    (Action::Brighter, [0.4425, 0.89, 0.115, 0.09], "Opacity +"),
    (Action::Add, [0.5675, 0.89, 0.115, 0.09], "Add button"),
    (Action::Remove, [0.6925, 0.89, 0.115, 0.09], "Remove"),
    (Action::Mode, [0.8175, 0.89, 0.115, 0.09], "Hold"),
];

/// Centered modal dialog boxes & buttons.
const DIALOG_PRESETS: [f32; 4] = [0.28, 0.24, 0.44, 0.52];
const PRESET_STD: [f32; 4] = [0.32, 0.37, 0.36, 0.09];
const PRESET_DPAD: [f32; 4] = [0.32, 0.485, 0.36, 0.09];
const PRESET_CLOSE: [f32; 4] = [0.40, 0.63, 0.20, 0.085];

const DIALOG_CONFIRM: [f32; 4] = [0.24, 0.29, 0.52, 0.42];
const CONFIRM_LEFT: [f32; 4] = [0.27, 0.57, 0.21, 0.09];
const CONFIRM_RIGHT: [f32; 4] = [0.52, 0.57, 0.21, 0.09];

const DIALOG_NEW_PROFILE: [f32; 4] = [0.24, 0.04, 0.52, 0.40];
const NEW_PROFILE_FIELD: [f32; 4] = [0.29, 0.18, 0.42, 0.09];
const NEW_PROFILE_CANCEL: [f32; 4] = [0.29, 0.305, 0.19, 0.09];
const NEW_PROFILE_SAVE: [f32; 4] = [0.52, 0.305, 0.19, 0.09];

/// Meridian: the key picker's cell for [`layout::KEYS`]'s key `i`, ten to a row.
fn pick_cell(i: usize) -> [f32; 4] {
    let (column, row) = ((i % 10) as f32, (i / 10) as f32);
    [0.155 + column * 0.07, 0.26 + row * 0.12, 0.06, 0.10]
}
const BACK: [f32; 4] = [0.42, 0.66, 0.16, 0.09];

#[derive(Clone, Debug, PartialEq, Eq)]
enum Popup {
    Presets,
    ConfirmReset,
    ConfirmDiscard,
    SavePrompt,
    ConfirmOverwrite { name: String, is_default: bool },
    ConfirmDelete { index: usize, name: String },
    NewProfile { name: String, close_after: bool },
}

/// Meridian: what the editor selects and drags: a built-in control, a key button (its place in the
/// scheme's keys) or the close button.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Item {
    Control(Control),
    Key(usize),
    Close,
}

#[derive(Clone, Copy)]
enum Gesture {
    Button(Action),
    Move(Item, [f32; 2]),
}

pub(super) struct Editor {
    original: Settings,
    hub: bool,
    /// Meridian: the key picker is open (after Add button).
    picking: bool,
    /// Meridian: the settings page is open over the hub.
    page: bool,
    /// Meridian: the layout profiles page is open (browse mode or save-to-profile mode).
    profiles_page: bool,
    save_to_profile: bool,
    profiles: Vec<profiles::LayoutProfile>,
    popup: Option<Popup>,
    selected: Option<Item>,
    finger: Option<(u64, Gesture)>,
    error: Option<String>,
}

fn dir_of(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

impl Editor {
    fn actions(&self) -> &[(Action, [f32; 4], &str)] {
        if self.page || self.profiles_page {
            &[]
        } else if self.hub {
            &HUB
        } else {
            &EDITOR
        }
    }

    fn popup_action_at(&self, popup: &Popup, point: [f32; 2]) -> Option<Action> {
        match popup {
            Popup::Presets => {
                if layout::contains(PRESET_STD, point) {
                    Some(Action::Standard)
                } else if layout::contains(PRESET_DPAD, point) {
                    Some(Action::Dpad)
                } else if layout::contains(PRESET_CLOSE, point) {
                    Some(Action::PopupClose)
                } else {
                    None
                }
            }
            Popup::ConfirmReset => {
                if layout::contains(CONFIRM_LEFT, point) {
                    Some(Action::PopupClose)
                } else if layout::contains(CONFIRM_RIGHT, point) {
                    Some(Action::ConfirmReset)
                } else {
                    None
                }
            }
            Popup::ConfirmDiscard => {
                if layout::contains(CONFIRM_LEFT, point) {
                    Some(Action::PopupClose)
                } else if layout::contains(CONFIRM_RIGHT, point) {
                    Some(Action::ConfirmDiscard)
                } else {
                    None
                }
            }
            Popup::SavePrompt => {
                if layout::contains(CONFIRM_LEFT, point) {
                    Some(Action::SaveAsDefault)
                } else if layout::contains(CONFIRM_RIGHT, point) {
                    Some(Action::SaveToProfiles)
                } else {
                    None
                }
            }
            Popup::ConfirmOverwrite { .. } => {
                if layout::contains(CONFIRM_LEFT, point) {
                    Some(Action::PopupClose)
                } else if layout::contains(CONFIRM_RIGHT, point) {
                    Some(Action::ConfirmOverwrite)
                } else {
                    None
                }
            }
            Popup::ConfirmDelete { .. } => {
                if layout::contains(CONFIRM_LEFT, point) {
                    Some(Action::PopupClose)
                } else if layout::contains(CONFIRM_RIGHT, point) {
                    Some(Action::ConfirmDelete)
                } else {
                    None
                }
            }
            Popup::NewProfile { .. } => {
                if layout::contains(NEW_PROFILE_FIELD, point) {
                    Some(Action::FocusName)
                } else if layout::contains(NEW_PROFILE_CANCEL, point) {
                    Some(Action::PopupClose)
                } else if layout::contains(NEW_PROFILE_SAVE, point) {
                    Some(Action::SubmitNewProfile)
                } else {
                    None
                }
            }
        }
    }

    fn action_at(&self, point: [f32; 2], settings: &Settings) -> Option<Action> {
        if let Some(popup) = &self.popup {
            return self.popup_action_at(popup, point);
        }
        if self.profiles_page {
            if layout::contains(PAGE_BACK, point) {
                return Some(Action::Back);
            }
            let shown = self.profiles.len().min(11);
            let total = shown + 1;
            for i in 0..shown {
                let rect = row_rect(i, total);
                if !self.save_to_profile
                    && !self.profiles[i].builtin
                    && layout::contains(profile_delete_rect(rect), point)
                {
                    return Some(Action::DeleteProfile(i));
                }
                let hit = if !self.save_to_profile && !self.profiles[i].builtin {
                    profile_select_rect(rect)
                } else {
                    rect
                };
                if layout::contains(hit, point) {
                    return Some(Action::PickProfile(i));
                }
            }
            if layout::contains(row_rect(shown, total), point) {
                return Some(Action::NewProfile);
            }
            return None;
        }
        if self.page {
            if layout::contains(PAGE_BACK, point) {
                return Some(Action::Back);
            }
            let rows = Row::shown(settings);
            let (i, &row) = rows
                .iter()
                .enumerate()
                .find(|&(i, _)| layout::contains(row_rect(i, rows.len()), point))?;
            let rect = row_rect(i, rows.len());
            // A switch's whole row flips it; a value takes its − and +.
            return if row.switch(settings).is_some() {
                Some(Action::Toggle(row))
            } else if layout::contains(row_minus(rect), point) {
                Some(Action::Step(row, false))
            } else if layout::contains(row_plus(rect), point) {
                Some(Action::Step(row, true))
            } else {
                None
            };
        }
        if self.picking {
            if layout::contains(BACK, point) {
                return Some(Action::Back);
            }
            return (0..layout::KEYS.len())
                .find(|&i| layout::contains(pick_cell(i), point))
                .map(Action::Pick);
        }
        self.actions()
            .iter()
            .find(|(_, rect, _)| layout::contains(*rect, point))
            .map(|(action, _, _)| *action)
    }
}

fn preview([x, y, w, h]: [f32; 4]) -> [f32; 4] {
    [
        CANVAS[0] + x * CANVAS[2],
        CANVAS[1] + y * CANVAS[3],
        w * CANVAS[2],
        h * CANVAS[3],
    ]
}

fn world_point(point: [f32; 2]) -> [f32; 2] {
    [
        (point[0] - CANVAS[0]) / CANVAS[2],
        (point[1] - CANVAS[1]) / CANVAS[3],
    ]
}

impl TouchControls {
    pub fn with_saved_layout() -> Self {
        Self {
            settings: Settings::load(Path::new(FILE)),
            ..Self::default()
        }
    }

    pub fn open_editor(&mut self) {
        self.clear();
        self.editor = Some(Editor {
            original: self.settings.clone(),
            hub: true,
            picking: false,
            page: false,
            profiles_page: false,
            save_to_profile: false,
            profiles: Vec::new(),
            popup: None,
            selected: None,
            finger: None,
            error: None,
        });
    }

    pub fn editing(&self) -> bool {
        self.editor.is_some()
    }

    pub fn editor_typing(&self) -> bool {
        self.editor
            .as_ref()
            .is_some_and(|e| matches!(e.popup, Some(Popup::NewProfile { .. })))
    }

    pub fn editor_text(&mut self, text: &str) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        let Some(Popup::NewProfile { name, .. }) = &mut editor.popup else {
            return;
        };
        editor.error = None;
        for ch in text.chars() {
            if ch.is_control()
                || matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
            {
                continue;
            }
            if name.chars().count() < 28 {
                name.push(ch);
            }
        }
    }

    pub fn editor_backspace(&mut self) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        if let Some(Popup::NewProfile { name, .. }) = &mut editor.popup {
            editor.error = None;
            name.pop();
        }
    }

    pub fn editor_enter(&mut self) {
        self.editor_enter_at(Path::new(FILE));
    }

    fn editor_enter_at(&mut self, path: &Path) {
        let Some(mut editor) = self.editor.take() else {
            return;
        };
        if !self.editor_action(&mut editor, Action::SubmitNewProfile, path) {
            self.editor = Some(editor);
        }
    }

    pub fn editor_escape(&mut self) {
        let Some(mut editor) = self.editor.take() else {
            return;
        };
        editor.error = None;
        if editor.popup.take().is_some() {
            self.editor = Some(editor);
            return;
        }
        if editor.picking || editor.page || editor.profiles_page {
            editor.picking = false;
            editor.page = false;
            editor.profiles_page = false;
            editor.save_to_profile = false;
            self.editor = Some(editor);
            return;
        }
        if self.settings != editor.original {
            editor.popup = Some(Popup::ConfirmDiscard);
            self.editor = Some(editor);
            return;
        }
        self.settings = editor.original;
        self.clear();
    }

    pub fn cancel_editor(&mut self) {
        if let Some(editor) = self.editor.take() {
            self.settings = editor.original;
            self.clear();
        }
    }

    /// Editor touches never reach the gameplay controls.
    pub fn editor_touch(&mut self, id: u64, point: [f32; 2], phase: TouchPhase) {
        self.edit_at(id, point, phase, Path::new(FILE));
    }

    #[cfg(test)]
    pub(super) fn edit_at_test(&mut self, point: [f32; 2], path: &Path) {
        self.edit_at(1, point, TouchPhase::Started, path);
        self.edit_at(1, point, TouchPhase::Ended, path);
    }

    fn edit_at(&mut self, id: u64, point: [f32; 2], phase: TouchPhase, path: &Path) {
        let Some(mut editor) = self.editor.take() else {
            return;
        };
        if phase == TouchPhase::Started && editor.finger.is_none() {
            if let Some(action) = editor.action_at(point, &self.settings) {
                editor.finger = Some((id, Gesture::Button(action)));
            } else if !editor.hub
                && !editor.picking
                && !editor.page
                && !editor.profiles_page
                && editor.popup.is_none()
            {
                editor.selected = self.item_at(point);
                if let Some(item) = editor.selected
                    && let Some(place) = self.item_place(item)
                {
                    let [x, y] = world_point(point);
                    let offset = [x - place.center[0], y - place.center[1]];
                    editor.finger = Some((id, Gesture::Move(item, offset)));
                }
            }
        } else if let Some((owner, gesture)) = editor.finger
            && owner == id
        {
            match gesture {
                Gesture::Move(item, [dx, dy]) if phase != TouchPhase::Cancelled => {
                    let [x, y] = world_point(point);
                    let size = self.item_saved(item).map_or(0.0, |p| p.size);
                    let p = self.fit(Placement {
                        center: [x - dx, y - dy],
                        size,
                        opacity: 1.0,
                    });
                    let left_handed = self.settings.left_handed;
                    if let Some(place) = self.item_saved(item) {
                        place.center = p.center;
                        if left_handed {
                            place.center[0] = 1.0 - place.center[0];
                        }
                    }
                    // Meridian: moved, the inventory leaves the hotbar for its own place.
                    if item == Item::Control(Control::Inventory) {
                        self.settings.inventory_on_hotbar = false;
                    }
                }
                Gesture::Button(action)
                    if phase == TouchPhase::Ended
                        && editor.action_at(point, &self.settings) == Some(action) =>
                {
                    if self.editor_action(&mut editor, action, path) {
                        return;
                    }
                }
                _ => {}
            }
            if matches!(phase, TouchPhase::Ended | TouchPhase::Cancelled) {
                editor.finger = None;
            }
        }
        self.editor = Some(editor);
    }

    /// Meridian: where an editor item is on this surface.
    fn item_place(&self, item: Item) -> Option<Placement> {
        match item {
            Item::Control(control) => Some(self.placement(control)),
            Item::Key(i) => self.key_place(i),
            Item::Close => Some(self.close_place()),
        }
    }

    /// Meridian: the saved placement an editor item changes (as saved: not mirrored or scaled).
    fn item_saved(&mut self, item: Item) -> Option<&mut Placement> {
        let mode = self.settings.mode.index();
        match item {
            Item::Control(control) => {
                let slot = self.settings.slot(control);
                Some(&mut self.settings.layouts[mode][slot])
            }
            Item::Key(i) => self.settings.keys[mode].get_mut(i).map(|k| &mut k.place),
            Item::Close => Some(&mut self.settings.close[mode]),
        }
    }

    /// Meridian: the editor item under `point`, the top one first: key buttons, the close button,
    /// then the visible built-in controls.
    fn item_at(&self, point: [f32; 2]) -> Option<Item> {
        let keys = (0..self.settings.mode_keys().len()).rev().map(Item::Key);
        let controls = Control::ALL
            .into_iter()
            .rev()
            .filter(|&control| self.visible(control))
            .map(Item::Control);
        keys.chain([Item::Close]).chain(controls).find(|&item| {
            self.item_place(item)
                .is_some_and(|p| layout::contains(preview(p.rect(self.aspect())), point))
        })
    }

    fn overlapping_dpad_actions(&self) -> bool {
        if self.settings.mode != Mode::Dpad {
            return false;
        }
        let actions = [
            Control::Jump,
            Control::Sneak,
            Control::Attack,
            Control::Use,
            Control::Shield,
            Control::Inventory,
            Control::Offhand,
            Control::Sprint,
        ];
        actions.iter().enumerate().any(|(i, &a)| {
            if !self.visible(a) {
                return false;
            }
            let [x, y, w, h] = self.placement(a).rect(self.aspect());
            actions[i + 1..]
                .iter()
                .copied()
                .filter(|&b| self.visible(b))
                .any(|b| {
                    let [bx, by, bw, bh] = self.placement(b).rect(self.aspect());
                    x < bx + bw && bx < x + w && y < by + bh && by < y + h
                })
        })
    }

    fn save_active(&mut self, editor: &mut Editor, path: &Path) -> bool {
        self.settings.sanitize();
        match self.settings.save(path) {
            Ok(()) => {
                self.clear();
                true
            }
            Err(_) => {
                editor.error = Some("Could not save the touch layout.".into());
                false
            }
        }
    }

    fn editor_action(&mut self, editor: &mut Editor, action: Action, path: &Path) -> bool {
        editor.error = None;
        let dir = dir_of(path).to_path_buf();
        match action {
            Action::Save => {
                self.settings.sanitize();
                if self.overlapping_dpad_actions() {
                    editor.error = Some("Move overlapping buttons apart.".into());
                    return false;
                }
                editor.popup = Some(Popup::SavePrompt);
            }
            Action::SaveAsDefault => {
                editor.popup = None;
                return self.save_active(editor, path);
            }
            Action::SaveToProfiles => {
                editor.popup = None;
                editor.profiles = profiles::list_profiles(&dir);
                editor.profiles_page = true;
                editor.save_to_profile = true;
            }
            Action::Cancel => {
                if self.settings != editor.original {
                    editor.popup = Some(Popup::ConfirmDiscard);
                    return false;
                }
                self.settings = editor.original.clone();
                self.clear();
                return true;
            }
            Action::ConfirmDiscard => {
                self.settings = editor.original.clone();
                self.clear();
                return true;
            }
            Action::Presets => editor.popup = Some(Popup::Presets),
            Action::Reset => editor.popup = Some(Popup::ConfirmReset),
            Action::ConfirmReset => {
                let mode = self.settings.mode.index();
                self.settings.layouts[mode] = Settings::default().layouts[mode];
                self.settings.close[mode] = layout::CLOSE;
                self.settings.inventory_on_hotbar = true;
                editor.selected = None;
                editor.popup = None;
            }
            Action::PopupClose => editor.popup = None,
            Action::Edit => editor.hub = false,
            Action::Standard | Action::Dpad => {
                self.settings.mode = if action == Action::Dpad {
                    Mode::Dpad
                } else {
                    Mode::Crosshair
                };
                // Meridian: the classic D-pad is MCPE's split controls: the crosshair picks the
                // block, a tap anywhere places or attacks there and a hold breaks or uses it,
                // with no attack and use buttons.
                let dpad = action == Action::Dpad;
                self.settings.split_controls = dpad;
                self.settings.crosshair_gestures = dpad;
                let mode = self.settings.mode.index();
                self.settings.layouts[mode] = Settings::default().layouts[mode];
                self.settings.inventory_on_hotbar = true;
                editor.hub = false;
                editor.popup = None;
                editor.selected = None;
            }
            Action::Add => editor.picking = true,
            Action::Back => {
                editor.picking = false;
                editor.page = false;
                editor.profiles_page = false;
                editor.save_to_profile = false;
            }
            Action::Profiles => {
                editor.profiles = profiles::list_profiles(&dir);
                editor.profiles_page = true;
                editor.save_to_profile = false;
            }
            Action::PickProfile(i) => {
                if let Some(profile) = editor.profiles.get(i).cloned() {
                    if editor.save_to_profile {
                        editor.popup = Some(Popup::ConfirmOverwrite {
                            name: profile.name,
                            is_default: profile.builtin,
                        });
                    } else {
                        self.settings = profile.settings;
                        editor.selected = None;
                        editor.profiles_page = false;
                    }
                }
            }
            Action::ConfirmOverwrite => {
                let Some(Popup::ConfirmOverwrite { name, is_default }) = editor.popup.clone()
                else {
                    return false;
                };
                self.settings.sanitize();
                if !is_default && let Err(err) = profiles::save_profile(&dir, &name, &self.settings)
                {
                    editor.error = Some(err);
                    return false;
                }
                editor.popup = None;
                editor.profiles_page = false;
                editor.save_to_profile = false;
                return self.save_active(editor, path);
            }
            Action::DeleteProfile(i) => {
                if let Some(profile) = editor.profiles.get(i)
                    && !profile.builtin
                {
                    editor.popup = Some(Popup::ConfirmDelete {
                        index: i,
                        name: profile.name.clone(),
                    });
                }
            }
            Action::ConfirmDelete => {
                let Some(Popup::ConfirmDelete { index, .. }) = editor.popup.take() else {
                    return false;
                };
                if let Some(profile) = editor.profiles.get(index)
                    && let Some(p) = &profile.path
                    && let Err(err) = profiles::delete_profile(p)
                {
                    editor.error = Some(err);
                }
                editor.profiles = profiles::list_profiles(&dir);
            }
            Action::NewProfile => {
                let name = profiles::next_profile_name(&editor.profiles);
                editor.popup = Some(Popup::NewProfile {
                    name,
                    close_after: editor.save_to_profile,
                });
            }
            Action::FocusName => {}
            Action::SubmitNewProfile => {
                let Some(Popup::NewProfile { name, close_after }) = editor.popup.clone() else {
                    return false;
                };
                let Some(clean) = profiles::sanitize_profile_name(&name) else {
                    editor.error = Some("Enter a valid profile name (not Default).".into());
                    return false;
                };
                if editor
                    .profiles
                    .iter()
                    .any(|p| !p.builtin && p.name.eq_ignore_ascii_case(&clean))
                {
                    editor.popup = Some(Popup::ConfirmOverwrite {
                        name: clean,
                        is_default: false,
                    });
                    return false;
                }
                self.settings.sanitize();
                if let Err(err) = profiles::save_profile(&dir, &clean, &self.settings) {
                    editor.error = Some(err);
                    return false;
                }
                editor.profiles = profiles::list_profiles(&dir);
                editor.popup = None;
                if close_after {
                    editor.profiles_page = false;
                    editor.save_to_profile = false;
                    return self.save_active(editor, path);
                }
            }
            Action::Settings => editor.page = true,
            Action::Toggle(row) => row.flip(&mut self.settings),
            Action::Step(row, up) => row.step(&mut self.settings, up),
            Action::Pick(i) => {
                let key = layout::KEYS[i];
                let keys = &mut self.settings.keys[self.settings.mode.index()];
                // A key already added stays as it is: one button per key.
                if !keys.iter().any(|k| k.key == key) {
                    keys.push(layout::KeyButton {
                        key: key.into(),
                        place: layout::NEW_KEY,
                        toggle: false,
                    });
                    editor.selected = Some(Item::Key(keys.len() - 1));
                }
                editor.picking = false;
            }
            Action::Remove => {
                if let Some(Item::Key(i)) = editor.selected {
                    let keys = &mut self.settings.keys[self.settings.mode.index()];
                    if i < keys.len() {
                        keys.remove(i);
                    }
                    editor.selected = None;
                }
            }
            Action::Mode => {
                if let Some(Item::Key(i)) = editor.selected
                    && let Some(key) = self.settings.keys[self.settings.mode.index()].get_mut(i)
                {
                    key.toggle = !key.toggle;
                }
            }
            Action::Smaller | Action::Larger | Action::Fainter | Action::Brighter => {
                if let Some(item) = editor.selected
                    && let Some(p) = self.item_saved(item)
                {
                    match action {
                        Action::Smaller => p.size -= 0.015,
                        Action::Larger => p.size += 0.015,
                        Action::Fainter => p.opacity = (p.opacity - 0.10).max(0.15),
                        Action::Brighter => p.opacity += 0.10,
                        _ => unreachable!(),
                    }
                    p.sanitize();
                }
            }
        }
        false
    }

    pub fn draw_editor(&self, b: &mut HudBatch, a: &HudAtlas) {
        let Some(editor) = &self.editor else { return };
        b.fill(a, 0.0, 0.0, b.width, b.height, theme::SCRIM);

        let error_shown = editor.popup.is_none().then_some(()).and(editor.error.as_deref());
        let title = error_shown.unwrap_or(if editor.picking {
            "Pick a key to add"
        } else if editor.profiles_page {
            if editor.save_to_profile {
                "Save to profile"
            } else {
                "Layout profiles"
            }
        } else if editor.page {
            "Settings"
        } else if editor.hub {
            "Controls"
        } else {
            "Drag a control"
        });
        let title_color = if error_shown.is_some() {
            theme::ERROR_TEXT
        } else {
            u32::MAX
        };

        if editor.picking {
            label(b, a, [0.30, 0.04, 0.40, 0.09], title, title_color);
            let added = self.settings.mode_keys();
            for (i, key) in layout::KEYS.iter().enumerate() {
                let taken = added.iter().any(|k| k.key == *key);
                if taken {
                    plate_disabled(b, a, pick_cell(i), key);
                } else {
                    plate_primary(b, a, pick_cell(i), key);
                }
            }
            plate_secondary(b, a, BACK, "Back");
            return;
        }

        if editor.profiles_page {
            label(b, a, [0.30, 0.03, 0.40, 0.08], title, title_color);
            let shown = editor.profiles.len().min(11);
            let total = shown + 1;
            for i in 0..shown {
                let profile = &editor.profiles[i];
                let rect = row_rect(i, total);
                if !editor.save_to_profile && !profile.builtin {
                    plate_secondary(b, a, profile_select_rect(rect), &profile.name);
                    plate_danger(b, a, profile_delete_rect(rect), "×");
                } else {
                    plate_secondary(b, a, rect, &profile.name);
                }
            }
            plate_primary(b, a, row_rect(shown, total), "+");
            plate_secondary(b, a, PAGE_BACK, "Back");
            draw_popup(b, a, editor);
            return;
        }

        // Settings page
        if editor.page {
            label(b, a, [0.30, 0.03, 0.40, 0.08], title, title_color);
            let rows = Row::shown(&self.settings);
            for (i, &row) in rows.iter().enumerate() {
                let rect = row_rect(i, rows.len());
                panel_box(b, a, rect, theme::ROW_CARD, theme::BORDER, false);
                let [x, y, w, h] = rect;
                label_left(b, a, [x + 0.015, y, w - 0.23, h], row.name(), u32::MAX);
                match row.switch(&self.settings) {
                    Some(true) => plate_primary(b, a, row_switch(rect), &row.text(&self.settings)),
                    Some(false) => plate_disabled(b, a, row_switch(rect), &row.text(&self.settings)),
                    None => {
                        if row.can_step(&self.settings, false) {
                            plate_secondary(b, a, row_minus(rect), "−");
                        } else {
                            plate_disabled(b, a, row_minus(rect), "−");
                        }
                        label(b, a, row_number(rect), &row.text(&self.settings), u32::MAX);
                        if row.can_step(&self.settings, true) {
                            plate_secondary(b, a, row_plus(rect), "+");
                        } else {
                            plate_disabled(b, a, row_plus(rect), "+");
                        }
                    }
                }
            }
            plate_secondary(b, a, PAGE_BACK, "Back");
            draw_popup(b, a, editor);
            return;
        }

        if editor.hub {
            panel_box(b, a, HUB_CARD, theme::CARD, theme::BORDER, true);
            label(b, a, [0.29, 0.16, 0.42, 0.08], title, title_color);
        } else {
            label(b, a, [0.32, 0.04, 0.36, 0.09], title, title_color);
            panel_box(b, a, CANVAS, theme::CARD, theme::BORDER, false);
            let (width, height) = (b.width, b.height);
            let bounds_of = |p: Placement| {
                let rect = preview(p.rect(self.aspect()));
                [
                    rect[0] * width,
                    rect[1] * height,
                    rect[2] * width,
                    rect[3] * height,
                ]
            };
            let tint = |p: Placement| rgba(255, 255, 255, (p.opacity * 255.0) as u8);
            for control in Control::ALL {
                if self.visible(control) {
                    let p = self.placement(control);
                    if control == Control::Move {
                        super::draw::draw_movement(
                            b,
                            a,
                            bounds_of(p),
                            self.settings.mode,
                            [0.0; 2],
                            false,
                            tint(p),
                        );
                    } else {
                        self.draw_control(b, a, control, bounds_of(p), false, tint(p));
                    }
                }
            }
            let close = self.close_place();
            key_button(b, a, bounds_of(close), "x", false, false, tint(close));
            for (i, key) in self.settings.mode_keys().iter().enumerate() {
                if let Some(p) = self.key_place(i) {
                    key_button(b, a, bounds_of(p), &key.key, true, false, tint(p));
                }
            }
            if let Some(item) = editor.selected
                && let Some(p) = self.item_place(item)
            {
                let [x, y, w, h] = bounds_of(p);
                stroke_rect(b, a, x, y, w, h, 2.0, theme::SELECT_OUTLINE);
            }
        }

        for &(action, rect, text) in editor.actions() {
            let enabled = match action {
                Action::Smaller | Action::Larger | Action::Fainter | Action::Brighter => {
                    editor.selected.is_some()
                }
                Action::Remove | Action::Mode => matches!(editor.selected, Some(Item::Key(_))),
                _ => true,
            };
            let text = match (action, editor.selected) {
                (Action::Mode, Some(Item::Key(i)))
                    if self.settings.mode_keys().get(i).is_some_and(|k| k.toggle) =>
                {
                    "Toggle"
                }
                _ => text,
            };
            if !enabled {
                plate_disabled(b, a, rect, text);
            } else if action == Action::Save {
                plate_primary(b, a, rect, text);
            } else {
                plate_secondary(b, a, rect, text);
            }
        }

        draw_popup(b, a, editor);
    }
}

fn draw_popup(b: &mut HudBatch, a: &HudAtlas, editor: &Editor) {
    let Some(popup) = &editor.popup else {
        return;
    };
    b.fill(a, 0.0, 0.0, b.width, b.height, theme::MODAL_SCRIM);
    match popup {
        Popup::Presets => {
            panel_box(b, a, DIALOG_PRESETS, theme::CARD, theme::BORDER, true);
            label(b, a, [0.31, 0.265, 0.38, 0.08], "Control presets", u32::MAX);
            plate_secondary(b, a, PRESET_STD, "Standard joystick");
            plate_secondary(b, a, PRESET_DPAD, "Classic D-pad");
            plate_primary(b, a, PRESET_CLOSE, "Close");
        }
        Popup::ConfirmReset => {
            panel_box(b, a, DIALOG_CONFIRM, theme::CARD, theme::BORDER, true);
            label(b, a, [0.27, 0.315, 0.46, 0.08], "Reset layout?", u32::MAX);
            label(
                b,
                a,
                [0.26, 0.405, 0.48, 0.07],
                "Restore default positions for this scheme?",
                theme::MUTED_TEXT,
            );
            plate_secondary(b, a, CONFIRM_LEFT, "Cancel");
            plate_danger(b, a, CONFIRM_RIGHT, "Reset");
        }
        Popup::ConfirmDiscard => {
            panel_box(b, a, DIALOG_CONFIRM, theme::CARD, theme::BORDER, true);
            label(b, a, [0.27, 0.315, 0.46, 0.08], "Discard changes?", u32::MAX);
            label(
                b,
                a,
                [0.26, 0.405, 0.48, 0.07],
                "You have unsaved layout edits.",
                theme::MUTED_TEXT,
            );
            plate_secondary(b, a, CONFIRM_LEFT, "Keep editing");
            plate_danger(b, a, CONFIRM_RIGHT, "Discard");
        }
        Popup::SavePrompt => {
            panel_box(b, a, DIALOG_CONFIRM, theme::CARD, theme::BORDER, true);
            label(b, a, [0.27, 0.315, 0.46, 0.08], "Save to a profile?", u32::MAX);
            label(
                b,
                a,
                [0.26, 0.405, 0.48, 0.07],
                "Save into a profile or override Default.",
                theme::MUTED_TEXT,
            );
            plate_secondary(b, a, CONFIRM_LEFT, "No (Default)");
            plate_primary(b, a, CONFIRM_RIGHT, "Yes (Profiles)");
        }
        Popup::ConfirmOverwrite { name, .. } => {
            panel_box(b, a, DIALOG_CONFIRM, theme::CARD, theme::BORDER, true);
            label(b, a, [0.27, 0.315, 0.46, 0.08], "Overwrite profile?", u32::MAX);
            let msg = format!("Replace '{name}' with this layout?");
            label(b, a, [0.26, 0.405, 0.48, 0.07], &msg, theme::MUTED_TEXT);
            plate_secondary(b, a, CONFIRM_LEFT, "Cancel");
            plate_danger(b, a, CONFIRM_RIGHT, "Overwrite");
        }
        Popup::ConfirmDelete { name, .. } => {
            panel_box(b, a, DIALOG_CONFIRM, theme::CARD, theme::BORDER, true);
            label(b, a, [0.27, 0.315, 0.46, 0.08], "Delete profile?", u32::MAX);
            let msg = format!("Remove '{name}' permanently?");
            label(b, a, [0.26, 0.405, 0.48, 0.07], &msg, theme::MUTED_TEXT);
            plate_secondary(b, a, CONFIRM_LEFT, "Cancel");
            plate_danger(b, a, CONFIRM_RIGHT, "Delete");
        }
        Popup::NewProfile { name, .. } => {
            panel_box(b, a, DIALOG_NEW_PROFILE, theme::CARD, theme::BORDER, true);
            label(b, a, [0.27, 0.055, 0.46, 0.065], "New layout profile", u32::MAX);
            let (sub, sub_color) = match editor.error.as_deref() {
                Some(err) => (err, theme::ERROR_TEXT),
                None => ("Tap name below to edit", theme::ACCENT_TEXT),
            };
            label(b, a, [0.27, 0.120, 0.46, 0.045], sub, sub_color);
            panel_box(
                b,
                a,
                NEW_PROFILE_FIELD,
                theme::FIELD_BG,
                theme::ACCENT_BAR,
                false,
            );
            let shown = format!("{name}|");
            label(b, a, NEW_PROFILE_FIELD, &shown, u32::MAX);
            plate_secondary(b, a, NEW_PROFILE_CANCEL, "Cancel");
            plate_primary(b, a, NEW_PROFILE_SAVE, "Save");
        }
    }
}

fn stroke_rect(
    b: &mut HudBatch,
    a: &HudAtlas,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    thickness: f32,
    color: u32,
) {
    b.fill(a, x, y, w, thickness, color);
    b.fill(a, x, y + h - thickness, w, thickness, color);
    b.fill(a, x, y, thickness, h, color);
    b.fill(a, x + w - thickness, y, thickness, h, color);
}

fn panel_box(
    b: &mut HudBatch,
    a: &HudAtlas,
    rect: [f32; 4],
    bg: u32,
    border: u32,
    accent_bar: bool,
) {
    let [x, y, w, h] = [
        rect[0] * b.width,
        rect[1] * b.height,
        rect[2] * b.width,
        rect[3] * b.height,
    ];
    b.fill(a, x, y, w, h, bg);
    stroke_rect(b, a, x, y, w, h, 1.0, border);
    if accent_bar {
        b.fill(a, x, y, w, 2.0, theme::ACCENT_BAR);
    }
}

fn plate_styled(
    b: &mut HudBatch,
    a: &HudAtlas,
    rect: [f32; 4],
    text: &str,
    bg: u32,
    border: u32,
    top: u32,
    text_color: u32,
) {
    let [x, y, w, h] = [
        rect[0] * b.width,
        rect[1] * b.height,
        rect[2] * b.width,
        rect[3] * b.height,
    ];
    b.fill(a, x, y, w, h, bg);
    stroke_rect(b, a, x, y, w, h, 1.0, border);
    b.fill(a, x, y, w, 1.5, top);
    label(b, a, rect, text, text_color);
}

fn plate_primary(b: &mut HudBatch, a: &HudAtlas, rect: [f32; 4], text: &str) {
    plate_styled(
        b,
        a,
        rect,
        text,
        theme::PRIMARY,
        theme::ACCENT_BAR,
        theme::PRIMARY_TOP,
        u32::MAX,
    );
}

fn plate_secondary(b: &mut HudBatch, a: &HudAtlas, rect: [f32; 4], text: &str) {
    plate_styled(
        b,
        a,
        rect,
        text,
        theme::SECONDARY,
        theme::BORDER_PURPLE,
        theme::SECONDARY_TOP,
        u32::MAX,
    );
}

fn plate_danger(b: &mut HudBatch, a: &HudAtlas, rect: [f32; 4], text: &str) {
    plate_styled(
        b,
        a,
        rect,
        text,
        theme::DANGER,
        theme::DANGER_TOP,
        theme::DANGER_TOP,
        u32::MAX,
    );
}

fn plate_disabled(b: &mut HudBatch, a: &HudAtlas, rect: [f32; 4], text: &str) {
    plate_styled(
        b,
        a,
        rect,
        text,
        theme::MUTED_BTN,
        theme::MUTED_BORDER,
        theme::MUTED_BORDER,
        theme::MUTED_TEXT,
    );
}

fn label(b: &mut HudBatch, a: &HudAtlas, [x, y, w, h]: [f32; 4], text: &str, color: u32) {
    let width = HudBatch::text_width(a, text);
    let [cx, cy] = [(x + w / 2.0) * b.width, (y + h / 2.0) * b.height];
    let scale = ((w * b.width - 4.0) / width.max(1.0)).clamp(0.1, 1.0);
    let start = b.vertices.len();
    b.text(a, text, cx - width / 2.0, cy - 4.0, color, true);
    b.scale_since(start, cx, cy, scale, scale);
}

/// Meridian: [`label`] from the box's left edge, shrunk the same way to fit.
fn label_left(b: &mut HudBatch, a: &HudAtlas, [x, y, w, h]: [f32; 4], text: &str, color: u32) {
    let width = HudBatch::text_width(a, text);
    let [left, cy] = [x * b.width, (y + h / 2.0) * b.height];
    let scale = (w * b.width / width.max(1.0)).clamp(0.1, 1.0);
    let start = b.vertices.len();
    b.text(a, text, left, cy - 4.0, color, true);
    b.scale_since(start, left, cy, scale, scale);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tap(t: &mut TouchControls, path: &Path, [x, y]: [f32; 2]) {
        t.edit_at(1, [x, y], TouchPhase::Started, path);
        t.edit_at(1, [x, y], TouchPhase::Ended, path);
    }

    fn center([x, y, w, h]: [f32; 4]) -> [f32; 2] {
        [x + w / 2.0, y + h / 2.0]
    }

    fn hub_center(action: Action) -> [f32; 2] {
        let rect = HUB
            .iter()
            .find(|(a, _, _)| *a == action)
            .map(|(_, rect, _)| *rect)
            .unwrap();
        center(rect)
    }

    fn editor_center(action: Action) -> [f32; 2] {
        let rect = EDITOR
            .iter()
            .find(|(a, _, _)| *a == action)
            .map(|(_, rect, _)| *rect)
            .unwrap();
        center(rect)
    }

    #[test]
    fn the_editor_uses_bedrock_buttons_and_falls_back_for_missing_sprites() {
        let mut atlas =
            HudAtlas::build(&horizon_assets::resource_source::ResourceSource::directory(
                std::path::PathBuf::from("/nonexistent/horizon-touch-preview"),
            ))
            .unwrap();
        let sprite = |u0| crate::hud::Rect {
            u0,
            u1: u0 + 0.04,
            v0: 9.0,
            v1: 9.04,
            w: 16,
            h: 16,
        };
        atlas
            .sprites
            .insert("gui/bedrock/ui/jump".into(), sprite(9.0));
        atlas
            .sprites
            .insert("gui/sprites/widget/button".into(), sprite(8.0));
        let mut t = TouchControls::default();
        t.viewport(2400, 1080);
        t.open_editor();
        tap(&mut t, Path::new(FILE), hub_center(Action::Edit));
        let mut batch = HudBatch::new(
            [2400, 1080],
            crate::hud::GuiScale {
                requested: 0,
                unicode: false,
            },
        );
        t.draw_editor(&mut batch, &atlas);
        assert!(batch.vertices.iter().any(|v| v.uv[0] == 9.0));
        assert!(batch.vertices.iter().any(|v| v.uv[0] == 8.0));
    }

    /// Meridian: a key button's frame is Bedrock's jump button; its label sprite takes the text's
    /// place when the key has one (not the close button's x, not Pick), else the text stays.
    #[test]
    fn key_buttons_draw_their_icon_and_others_keep_their_text() {
        let mut atlas =
            HudAtlas::build(&horizon_assets::resource_source::ResourceSource::directory(
                std::path::PathBuf::from("/nonexistent/horizon-touch-preview"),
            ))
            .unwrap();
        let sprite = |u0| crate::hud::Rect {
            u0,
            u1: u0 + 0.22,
            v0: 9.0,
            v1: 9.22,
            w: 22,
            h: 22,
        };
        for (name, u0) in [
            ("gui/bedrock/ui/jump", 9.0),
            ("gui/bedrock/controls/jump", 9.0),
            ("gui/bedrock/controls/jump_pressed", 9.0),
            ("gui/meridian/keys/b", 20.0),
            ("gui/meridian/keys/b_pressed", 30.0),
            ("gui/meridian/keys/x", 40.0),
        ] {
            atlas.sprites.insert(name.into(), sprite(u0));
        }
        let draw = |label: &str, icon, pressed| {
            let mut batch = HudBatch::new(
                [2400, 1080],
                crate::hud::GuiScale {
                    requested: 0,
                    unicode: false,
                },
            );
            key_button(
                &mut batch,
                &atlas,
                [100.0, 100.0, 110.0, 110.0],
                label,
                icon,
                pressed,
                u32::MAX,
            );
            let from = |u: f32| batch.vertices.iter().any(|v| (u..u + 1.0).contains(&v.uv[0]));
            (from(20.0), from(30.0), from(40.0), batch.vertices.len())
        };
        let (normal, pressed, close, _) = draw("B", true, false);
        assert!(normal && !pressed && !close, "B shows its icon");
        let (normal, pressed, _, _) = draw("B", true, true);
        assert!(!normal && pressed, "pressed B shows the pressed icon");
        let (_, _, icon, text_vertices) = draw("x", false, false);
        assert!(!icon);
        let (_, _, icon, icon_vertices) = draw("X", true, false);
        assert!(icon);
        assert!(text_vertices > icon_vertices);
        let (normal, pressed, close, _) = draw("G", true, false);
        assert!(!normal && !pressed && !close);
    }

    #[test]
    fn settings_page_switches_steps_and_cancels() {
        let path = std::env::temp_dir().join(format!(
            "meridian-touch-settings-{}.json",
            std::process::id()
        ));
        let mut t = TouchControls::default();
        t.viewport(2400, 1080);
        t.open_editor();
        tap(&mut t, &path, hub_center(Action::Settings));
        let rows = Row::shown(&t.settings);
        let row = |i| row_rect(i, rows.len());
        tap(&mut t, &path, center(row_plus(row(0))));
        assert!((t.settings.sensitivity - 1.1).abs() < 1e-4);
        for _ in 0..40 {
            tap(&mut t, &path, center(row_plus(row(0))));
        }
        assert_eq!(t.settings.sensitivity, 3.0);
        assert!(!Row::Sensitivity.can_step(&t.settings, true));
        for _ in 0..40 {
            tap(&mut t, &path, center(row_minus(row(0))));
        }
        assert_eq!(t.settings.sensitivity, 0.1);
        let [x, y, _, h] = row(3);
        tap(&mut t, &path, [x + 0.03, y + h / 2.0]);
        assert!(t.settings.sprint_toggle);
        tap(&mut t, &path, center(PAGE_BACK));
        // Edited settings -> Cancel opens Discard confirmation, ConfirmDiscard reverts & closes.
        tap(&mut t, &path, hub_center(Action::Cancel));
        assert!(t.editing());
        tap(&mut t, &path, center(CONFIRM_RIGHT));
        assert!(!t.editing());
        assert_eq!(t.settings.sensitivity, 1.0);
        assert!(!t.settings.sprint_toggle);
    }

    #[test]
    fn settings_page_rows_follow_the_scheme() {
        let mut s = Settings::default();
        s.mode = Mode::Dpad;
        assert!(Row::shown(&s).contains(&Row::SwapJumpSneak));
        assert!(!Row::shown(&s).contains(&Row::FloatingJoystick));
        assert_eq!(Row::LeftHanded.text(&s), "Off");
        assert_eq!(Row::Sensitivity.text(&s), "100%");
        assert_eq!(Row::Vibration.text(&s), "On");
        assert_eq!(Row::Gyro.text(&s), "Off");
        assert_eq!(Row::GyroSpeed.text(&s), "100%");
    }

    #[test]
    fn dpad_preset_drags_and_saves_without_gameplay_input() {
        let path =
            std::env::temp_dir().join(format!("meridian-touch-save-{}.json", std::process::id()));
        let mut t = TouchControls::default();
        t.viewport(2400, 1080);
        t.open_editor();
        tap(&mut t, &path, hub_center(Action::Edit));
        tap(&mut t, &path, editor_center(Action::Presets));
        tap(&mut t, &path, center(PRESET_DPAD));
        assert_eq!(t.settings.mode, Mode::Dpad);
        assert!(t.settings.split_controls && t.settings.crosshair_gestures);
        assert!(!t.visible(Control::Attack) && !t.visible(Control::Use));

        let control = Control::Jump;
        let [x, y, w, h] = preview(t.placement(control).rect(t.aspect()));
        let start = [x + w / 2.0, y + h / 2.0];
        t.edit_at(1, start, TouchPhase::Started, &path);
        t.edit_at(2, [0.6, 0.5], TouchPhase::Moved, &path);
        let original = t.placement(control);
        t.edit_at(1, [start[0] - 0.03, start[1]], TouchPhase::Moved, &path);
        t.edit_at(1, [start[0] - 0.03, start[1]], TouchPhase::Ended, &path);
        assert!(t.placement(control).center[0] < original.center[0]);
        assert!(t.state().left.iter().all(|v| *v == 0.0));
        tap(&mut t, &path, editor_center(Action::Larger));
        assert!(t.placement(control).size > original.size);
        // Save -> SavePrompt -> No (Default) overrides touch-layout.json directly.
        tap(&mut t, &path, editor_center(Action::Save));
        assert!(t.editing());
        tap(&mut t, &path, center(CONFIRM_LEFT));
        assert!(!t.editing());
        assert_eq!(Settings::load(&path), t.settings);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn add_puts_a_picked_key_in_the_middle_and_remove_takes_it_away() {
        let path =
            std::env::temp_dir().join(format!("meridian-touch-keys-{}.json", std::process::id()));
        let mut t = TouchControls::default();
        t.viewport(2400, 1080);
        t.open_editor();
        tap(&mut t, &path, hub_center(Action::Edit));
        let (add, remove, mode_btn) = (
            editor_center(Action::Add),
            editor_center(Action::Remove),
            editor_center(Action::Mode),
        );
        let pick = |key: &str| {
            let i = layout::KEYS.iter().position(|k| *k == key).unwrap();
            center(pick_cell(i))
        };
        tap(&mut t, &path, remove);
        assert!(t.settings.mode_keys().is_empty());
        tap(&mut t, &path, add);
        tap(&mut t, &path, pick("G"));
        assert_eq!(t.settings.mode_keys().len(), 1);
        assert_eq!(t.settings.mode_keys()[0].key, "G");
        assert_eq!(t.settings.mode_keys()[0].place, layout::NEW_KEY);
        tap(&mut t, &path, add);
        tap(&mut t, &path, pick("G"));
        assert_eq!(t.settings.mode_keys().len(), 1);
        assert!(!t.settings.mode_keys()[0].toggle);
        tap(&mut t, &path, mode_btn);
        assert!(t.settings.mode_keys()[0].toggle);
        tap(&mut t, &path, mode_btn);
        assert!(!t.settings.mode_keys()[0].toggle);
        tap(&mut t, &path, remove);
        assert!(t.settings.mode_keys().is_empty());
        tap(&mut t, &path, add);
        tap(&mut t, &path, pick("F12"));
        tap(&mut t, &path, editor_center(Action::Save));
        tap(&mut t, &path, center(CONFIRM_LEFT));
        assert!(!t.editing());
        assert_eq!(Settings::load(&path).mode_keys()[0].key, "F12");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn reset_discard_and_save_to_profile_flows_confirm_and_persist() {
        let dir = std::env::temp_dir().join(format!(
            "meridian-touch-popups-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(FILE);

        let mut t = TouchControls::default();
        t.viewport(2400, 1080);
        let original = t.settings.clone();
        t.open_editor();
        tap(&mut t, &path, hub_center(Action::Edit));
        // Move Jump slightly so layout is dirty.
        t.settings.layouts[0][Control::Jump as usize].center = [0.55, 0.55];
        // Reset layout requires confirmation; Cancel closes popup, Confirm resets.
        tap(&mut t, &path, editor_center(Action::Reset));
        tap(&mut t, &path, center(CONFIRM_LEFT));
        assert_eq!(t.settings.layouts[0][Control::Jump as usize].center, [0.55, 0.55]);
        tap(&mut t, &path, editor_center(Action::Reset));
        tap(&mut t, &path, center(CONFIRM_RIGHT));
        assert_eq!(
            t.settings.layouts[0][Control::Jump as usize].center,
            original.layouts[0][Control::Jump as usize].center
        );

        // Make dirty -> Cancel opens Discard confirmation; Keep editing keeps editor open.
        t.settings.sensitivity = 2.0;
        tap(&mut t, &path, editor_center(Action::Cancel));
        assert!(t.editing());
        tap(&mut t, &path, center(CONFIRM_LEFT));
        assert!(t.editing());

        // Save -> Yes (Profiles) -> '+' -> NewProfile ("Profile 1") -> Save.
        tap(&mut t, &path, editor_center(Action::Save));
        tap(&mut t, &path, center(CONFIRM_RIGHT));
        // Shown: [Default, +] -> row 1 of 2 is '+'
        tap(&mut t, &path, center(row_rect(1, 2)));
        assert!(t.editor_typing());
        t.editor_backspace();
        t.editor_text("9");
        tap(&mut t, &path, center(NEW_PROFILE_SAVE));
        assert!(!t.editing());
        assert!(dir.join("touch-layouts/Profile 9.json").is_file());
        assert_eq!(Settings::load(&path).sensitivity, 2.0);

        // Open again, edit -> Hub Save -> Yes (Profiles) -> tap 'Profile 9' -> Overwrite confirm!
        t.open_editor();
        t.settings.sensitivity = 1.5;
        tap(&mut t, &path, hub_center(Action::Save));
        tap(&mut t, &path, center(CONFIRM_RIGHT));
        // Shown: [Default, Profile 9, +] -> row 1 of 3 is Profile 9
        tap(&mut t, &path, center(row_rect(1, 3)));
        assert!(matches!(
            t.editor.as_ref().unwrap().popup,
            Some(Popup::ConfirmOverwrite { .. })
        ));
        tap(&mut t, &path, center(CONFIRM_RIGHT));
        assert!(!t.editing());
        let saved_p9 = profiles::list_profiles(&dir)
            .into_iter()
            .find(|p| p.name == "Profile 9")
            .unwrap();
        assert_eq!(saved_p9.settings.sensitivity, 1.5);

        // Browse Layout profiles -> delete Profile 9 via '×' + confirm!
        t.open_editor();
        tap(&mut t, &path, hub_center(Action::Profiles));
        tap(&mut t, &path, center(profile_delete_rect(row_rect(1, 3))));
        tap(&mut t, &path, center(CONFIRM_RIGHT));
        assert_eq!(profiles::list_profiles(&dir).len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dpad_load_repairs_factory_actions_and_save_rejects_overlap() {
        let path = std::env::temp_dir().join(format!(
            "meridian-touch-dpad-spacing-{}.json",
            std::process::id()
        ));
        let mut t = TouchControls::default();
        t.viewport(2320, 1080);
        t.settings.mode = Mode::Dpad;
        t.settings.split_controls = true;
        let row = &mut t.settings.layouts[Mode::Dpad.index()];
        row[Control::Attack as usize] = layout::Placement::new(0.80, 0.64, 0.16);
        row[Control::Use as usize] = layout::Placement::new(0.92, 0.52, 0.16);
        let moved_jump = layout::Placement::new(0.8384, 0.7549, 0.155);
        row[Control::Jump as usize] = moved_jump;
        assert!(t.overlapping_dpad_actions());
        t.settings.save(&path).unwrap();

        t.settings = Settings::load(&path);
        assert_eq!(t.settings.placement(Control::Jump), moved_jump);
        assert!(!t.overlapping_dpad_actions());
        t.open_editor();
        tap(&mut t, &path, hub_center(Action::Edit));
        t.settings.layouts[Mode::Dpad.index()][Control::Jump as usize] =
            t.settings.placement(Control::Attack);
        let before = std::fs::read(&path).unwrap();
        tap(&mut t, &path, editor_center(Action::Save));
        assert!(t.editing());
        assert_eq!(std::fs::read(&path).unwrap(), before);

        tap(&mut t, &path, editor_center(Action::Reset));
        tap(&mut t, &path, center(CONFIRM_RIGHT));
        tap(&mut t, &path, editor_center(Action::Save));
        tap(&mut t, &path, center(CONFIRM_LEFT));
        assert!(!t.editing());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn cancelled_save_and_cancel_restore_the_previous_settings() {
        let path =
            std::env::temp_dir().join(format!("meridian-touch-cancel-{}.json", std::process::id()));
        let mut t = TouchControls::default();
        let original = t.settings.clone();
        t.open_editor();
        tap(&mut t, &path, hub_center(Action::Edit));
        tap(&mut t, &path, editor_center(Action::Presets));
        tap(&mut t, &path, center(PRESET_DPAD));
        assert_ne!(t.settings, original);
        let save_pt = editor_center(Action::Save);
        t.edit_at(1, save_pt, TouchPhase::Started, &path);
        t.edit_at(1, save_pt, TouchPhase::Cancelled, &path);
        assert!(t.editing() && !path.exists());
        tap(&mut t, &path, editor_center(Action::Cancel));
        tap(&mut t, &path, center(CONFIRM_RIGHT));
        assert!(!t.editing());
        assert_eq!(t.settings, original);
    }
}
