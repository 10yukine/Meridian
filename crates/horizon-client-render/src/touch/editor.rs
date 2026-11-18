// Copyright (c) 2026 Ludovic Marie and the Horizon contributors.
// SPDX-License-Identifier: MIT
//! Touch layout editing adapted from Starling's touch editor. Preview and input
//! share Meridian's existing placements, visibility rules, and layout storage.

use super::{Control, Mode, Settings, TouchControls, layout};
use crate::hud::{HudAtlas, HudBatch, rgba};
use std::path::Path;
use winit::event::TouchPhase;

pub const FILE: &str = "touch-layout.json";
const CANVAS: [f32; 4] = [0.15, 0.16, 0.70, 0.70];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    Standard,
    Dpad,
    Compact,
    Edit,
    Hub,
    Save,
    Cancel,
    Reset,
    Smaller,
    Larger,
    Fainter,
    Brighter,
}

const HUB: [(Action, [f32; 4], &str); 6] = [
    (
        Action::Standard,
        [0.29, 0.27, 0.42, 0.09],
        "Standard joystick",
    ),
    (Action::Dpad, [0.29, 0.38, 0.42, 0.09], "Classic D-pad"),
    (
        Action::Compact,
        [0.29, 0.49, 0.42, 0.09],
        "Compact joystick",
    ),
    (Action::Edit, [0.29, 0.60, 0.42, 0.09], "Edit layout"),
    (Action::Cancel, [0.29, 0.74, 0.20, 0.09], "Cancel"),
    (Action::Save, [0.51, 0.74, 0.20, 0.09], "Save"),
];

const EDITOR: [(Action, [f32; 4], &str); 8] = [
    (Action::Hub, [0.02, 0.04, 0.13, 0.09], "Presets"),
    (Action::Reset, [0.17, 0.04, 0.13, 0.09], "Reset layout"),
    (Action::Cancel, [0.70, 0.04, 0.13, 0.09], "Cancel"),
    (Action::Save, [0.85, 0.04, 0.13, 0.09], "Save"),
    (Action::Smaller, [0.15, 0.89, 0.16, 0.09], "Size −"),
    (Action::Larger, [0.33, 0.89, 0.16, 0.09], "Size +"),
    (Action::Fainter, [0.51, 0.89, 0.16, 0.09], "Opacity −"),
    (Action::Brighter, [0.69, 0.89, 0.16, 0.09], "Opacity +"),
];

#[derive(Clone, Copy)]
enum Gesture {
    Button(Action),
    Move(Control, [f32; 2]),
}

pub(super) struct Editor {
    original: Settings,
    hub: bool,
    selected: Option<Control>,
    finger: Option<(u64, Gesture)>,
    error: Option<String>,
}

impl Editor {
    fn actions(&self) -> &[(Action, [f32; 4], &str)] {
        if self.hub { &HUB } else { &EDITOR }
    }

    fn action_at(&self, point: [f32; 2]) -> Option<Action> {
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
            selected: None,
            finger: None,
            error: None,
        });
    }

    pub fn editing(&self) -> bool {
        self.editor.is_some()
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

    fn edit_at(&mut self, id: u64, point: [f32; 2], phase: TouchPhase, path: &Path) {
        let Some(mut editor) = self.editor.take() else {
            return;
        };
        if phase == TouchPhase::Started && editor.finger.is_none() {
            if let Some(action) = editor.action_at(point) {
                editor.finger = Some((id, Gesture::Button(action)));
            } else if !editor.hub {
                editor.selected = Control::ALL.into_iter().rev().find(|&control| {
                    self.visible(control)
                        && layout::contains(
                            preview(self.placement(control).rect(self.aspect())),
                            point,
                        )
                });
                if let Some(control) = editor.selected {
                    let center = self.placement(control).center;
                    let [x, y] = world_point(point);
                    editor.finger =
                        Some((id, Gesture::Move(control, [x - center[0], y - center[1]])));
                }
            }
        } else if let Some((owner, gesture)) = editor.finger
            && owner == id
        {
            match gesture {
                Gesture::Move(control, [dx, dy]) if phase != TouchPhase::Cancelled => {
                    let [x, y] = world_point(point);
                    let p = self.fit(layout::Placement {
                        center: [x - dx, y - dy],
                        size: self.settings.placement(control).size,
                        opacity: 1.0,
                    });
                    let slot = self.settings.slot(control);
                    let place = &mut self.settings.layouts[self.settings.mode.index()][slot];
                    place.center = p.center;
                    if self.settings.left_handed {
                        place.center[0] = 1.0 - place.center[0];
                    }
                }
                Gesture::Button(action)
                    if phase == TouchPhase::Ended && editor.action_at(point) == Some(action) =>
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

    fn editor_action(&mut self, editor: &mut Editor, action: Action, path: &Path) -> bool {
        editor.error = None;
        match action {
            Action::Save => {
                self.settings.sanitize();
                match self.settings.save(path) {
                    Ok(()) => {
                        self.clear();
                        return true;
                    }
                    Err(_) => editor.error = Some("Could not save the touch layout.".into()),
                }
            }
            Action::Cancel => {
                self.settings = editor.original.clone();
                self.clear();
                return true;
            }
            Action::Hub => editor.hub = true,
            Action::Edit => editor.hub = false,
            Action::Standard | Action::Dpad | Action::Compact => {
                self.settings.mode = if action == Action::Dpad {
                    Mode::Dpad
                } else {
                    Mode::Crosshair
                };
                self.settings.split_controls = action == Action::Dpad;
                let mode = self.settings.mode.index();
                let mut row = Settings::default().layouts[mode];
                if action == Action::Compact {
                    for control in [
                        Control::Move,
                        Control::Attack,
                        Control::Use,
                        Control::Jump,
                        Control::Sneak,
                        Control::Sprint,
                    ] {
                        row[control as usize].size *= 0.80;
                    }
                    row[Control::Jump as usize].center = [0.90, 0.83];
                    row[Control::Use as usize].center = [0.90, 0.62];
                }
                self.settings.layouts[mode] = row;
                editor.hub = false;
                editor.selected = None;
            }
            Action::Reset => {
                let mode = self.settings.mode.index();
                self.settings.layouts[mode] = Settings::default().layouts[mode];
            }
            Action::Smaller | Action::Larger | Action::Fainter | Action::Brighter => {
                if let Some(control) = editor.selected {
                    let slot = self.settings.slot(control);
                    let p = &mut self.settings.layouts[self.settings.mode.index()][slot];
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
        b.fill(a, 0.0, 0.0, b.width, b.height, rgba(7, 12, 19, 180));
        let title = editor.error.as_deref().unwrap_or(if editor.hub {
            "Touch controls"
        } else {
            "Drag a control"
        });
        label(b, a, [0.31, 0.04, 0.38, 0.09], title, u32::MAX);
        if !editor.hub {
            plate(b, a, CANVAS, "", rgba(20, 31, 45, 160));
            for control in Control::ALL {
                if self.visible(control) {
                    let p = self.placement(control);
                    let color = if editor.selected == Some(control) {
                        rgba(65, 130, 150, 230)
                    } else {
                        rgba(43, 62, 81, (p.opacity.max(0.3) * 255.0) as u8)
                    };
                    let rect = preview(p.rect(self.aspect()));
                    if control == Control::Move {
                        let bounds = [
                            rect[0] * b.width,
                            rect[1] * b.height,
                            rect[3] * b.height,
                            rect[3] * b.height,
                        ];
                        let opacity = if editor.selected == Some(control) {
                            1.0
                        } else {
                            p.opacity
                        };
                        super::draw::draw_movement(
                            b,
                            a,
                            bounds,
                            self.settings.mode,
                            [0.0; 2],
                            editor.selected == Some(control),
                            rgba(225, 243, 252, (opacity * 255.0) as u8),
                        );
                        continue;
                    }
                    let name = match control {
                        Control::Debug => "F3",
                        Control::Perspective => "F5",
                        Control::Inventory => "...",
                        Control::Settings => "Touch",
                        _ => control.name().split(" / ").next().unwrap_or(control.name()),
                    };
                    plate(b, a, rect, name, color);
                }
            }
        }
        for &(action, rect, text) in editor.actions() {
            let enabled = editor.selected.is_some()
                || !matches!(
                    action,
                    Action::Smaller | Action::Larger | Action::Fainter | Action::Brighter
                );
            plate(
                b,
                a,
                rect,
                text,
                if enabled {
                    rgba(45, 70, 93, 240)
                } else {
                    rgba(36, 44, 53, 190)
                },
            );
        }
    }
}

fn plate(b: &mut HudBatch, a: &HudAtlas, rect: [f32; 4], text: &str, color: u32) {
    let [x, y, w, h] = [
        rect[0] * b.width,
        rect[1] * b.height,
        rect[2] * b.width,
        rect[3] * b.height,
    ];
    b.fill(a, x, y, w, h, color);
    b.fill(a, x, y, w, 1.0, rgba(150, 200, 216, 170));
    label(b, a, rect, text, u32::MAX);
}

fn label(b: &mut HudBatch, a: &HudAtlas, [x, y, w, h]: [f32; 4], text: &str, color: u32) {
    let width = HudBatch::text_width(a, text);
    let [cx, cy] = [(x + w / 2.0) * b.width, (y + h / 2.0) * b.height];
    let scale = ((w * b.width - 4.0) / width.max(1.0)).clamp(0.1, 1.0);
    let start = b.vertices.len();
    b.text(a, text, cx - width / 2.0, cy - 4.0, color, true);
    b.scale_since(start, cx, cy, scale, scale);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tap(t: &mut TouchControls, path: &Path, [x, y]: [f32; 2]) {
        t.edit_at(1, [x, y], TouchPhase::Started, path);
        t.edit_at(1, [x, y], TouchPhase::Ended, path);
    }

    #[test]
    fn dpad_preset_drags_and_saves_without_gameplay_input() {
        let path =
            std::env::temp_dir().join(format!("meridian-touch-save-{}.json", std::process::id()));
        let mut t = TouchControls::default();
        t.viewport(2400, 1080);
        t.open_editor();
        tap(&mut t, &path, [0.50, 0.42]);
        assert_eq!(t.settings.mode, Mode::Dpad);
        assert!(t.settings.split_controls);
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
        tap(&mut t, &path, [0.41, 0.93]);
        assert!(t.placement(control).size > original.size);
        tap(&mut t, &path, [0.91, 0.08]);
        assert!(!t.editing());
        assert_eq!(Settings::load(&path), t.settings);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn cancelled_save_and_cancel_restore_the_previous_settings() {
        let path =
            std::env::temp_dir().join(format!("meridian-touch-cancel-{}.json", std::process::id()));
        let mut t = TouchControls::default();
        let original = t.settings.clone();
        t.open_editor();
        tap(&mut t, &path, [0.5, 0.53]);
        assert_ne!(t.settings, original);
        t.edit_at(1, [0.91, 0.08], TouchPhase::Started, &path);
        t.edit_at(1, [0.91, 0.08], TouchPhase::Cancelled, &path);
        assert!(t.editing() && !path.exists());
        tap(&mut t, &path, [0.76, 0.08]);
        assert!(!t.editing());
        assert_eq!(t.settings, original);
    }
}
