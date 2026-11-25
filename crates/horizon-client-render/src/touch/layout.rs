// Adapted from Starling (https://github.com/starling-client/starling, `android` branch at
// 150a414b09a1, `crates/horizon-client-render/src/touch/layout.rs`), written by Vasysik and the
// Starling contributors. Copyright (c) 2026 Ludovic Marie and the Horizon contributors, under the
// same MIT license as Horizon (LICENSE at the repository root).

//! The touch layout: which on-screen controls exist, where each one sits for
//! each control scheme, and the player's saved choices. Coordinates are
//! normalized to the surface; sizes are fractions of its height, so a button
//! stays square on wide phones and on tablets. Nothing here reads input or
//! draws: this is the file the touch controls and their editor share.
use serde::{Deserialize, Serialize};
use std::path::Path;

/// How the player moves and aims.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    /// A joystick moves; attack and use act on the crosshair.
    #[default]
    Crosshair,
    /// A joystick moves; tapping the world attacks or uses what was tapped.
    Tap,
    /// A D-pad moves; tapping the world acts on it (or the crosshair, split).
    Dpad,
}

impl Mode {
    pub const ALL: [Self; 3] = [Self::Crosshair, Self::Tap, Self::Dpad];
    /// The four schemes a settings screen offers: mode, `split_controls`, label.
    pub const SCHEMES: [(Self, bool, &'static str); 4] = [
        (Self::Crosshair, false, "Joystick & aim crosshair"),
        (Self::Tap, false, "Joystick & tap to interact"),
        (Self::Dpad, false, "D-Pad & tap to interact"),
        (Self::Dpad, true, "D-Pad & aim crosshair"),
    ];

    /// This mode's row in [`Settings::layouts`].
    pub fn index(self) -> usize {
        self as usize
    }
}

/// The built-in controls. The order is the index into a layout row and into
/// `button_visibility`, so new controls go at the end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    Move,
    Sprint,
    Jump,
    Sneak,
    Attack,
    Use,
    Chat,
    Pause,
    Settings,
    Inventory,
    Debug,
    Perspective,
    Offhand,
    Shield,
}

/// How many built-in controls a layout row holds.
pub const CONTROLS: usize = Control::ALL.len();

impl Control {
    pub const ALL: [Self; 14] = [
        Self::Move,
        Self::Sprint,
        Self::Jump,
        Self::Sneak,
        Self::Attack,
        Self::Use,
        Self::Chat,
        Self::Pause,
        Self::Settings,
        Self::Inventory,
        Self::Debug,
        Self::Perspective,
        Self::Offhand,
        Self::Shield,
    ];

    /// The name the layout editor shows.
    pub fn name(self) -> &'static str {
        match self {
            Self::Move => "Movement",
            Self::Sprint => "Sprint",
            Self::Jump => "Jump / fly up",
            Self::Sneak => "Sneak / fly down",
            Self::Attack => "Attack / mine",
            Self::Use => "Use / place",
            Self::Chat => "Chat",
            Self::Pause => "Pause",
            Self::Settings => "Touch settings",
            Self::Inventory => "Inventory",
            Self::Debug => "Debug overlay (F3)",
            Self::Perspective => "Camera perspective (F5)",
            Self::Offhand => "Swap hands",
            Self::Shield => "Raise shield",
        }
    }
}

/// Where one control sits: its centre (0..1 of the surface), its size (a
/// fraction of the surface height) and its opacity.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    pub center: [f32; 2],
    pub size: f32,
    pub opacity: f32,
}

impl Placement {
    pub const fn new(x: f32, y: f32, size: f32) -> Self {
        Self {
            center: [x, y],
            size,
            opacity: 0.65,
        }
    }

    /// The square control as `[x, y, w, h]` in normalized surface coordinates,
    /// for a surface `aspect` (width / height) wide.
    pub fn rect(self, aspect: f32) -> [f32; 4] {
        let w = self.size / aspect.max(0.1);
        [
            self.center[0] - w / 2.0,
            self.center[1] - self.size / 2.0,
            w,
            self.size,
        ]
    }

    /// Brings a value read from disk back on screen and into range.
    pub fn sanitize(&mut self) {
        self.size = finite_or(self.size, 0.15).clamp(0.045, 0.45);
        self.opacity = finite_or(self.opacity, 0.65).clamp(0.0, 1.0);
        for c in &mut self.center {
            *c = finite_or(*c, 0.5).clamp(0.025, 0.975);
        }
    }
}

/// `button_visibility` values: follow the scheme, always shown, always hidden.
pub const FOLLOW_SCHEME: u8 = 0;
pub const SHOWN: u8 = 1;
pub const HIDDEN: u8 = 2;

/// The player's touch layout, saved as JSON. Missing fields take their
/// defaults, so later options can be added without resetting a saved file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub mode: Mode,
    /// With [`Mode::Dpad`]: attack and use act on the crosshair.
    pub split_controls: bool,
    /// Mirrors every control left to right.
    pub left_handed: bool,
    /// With [`Mode::Dpad`]: the jump and sneak buttons trade places.
    pub swap_jump_sneak: bool,
    pub sprint_button: bool,
    /// Sprint by pushing the joystick past its rim instead of with a button.
    pub sprint_with_joystick: bool,
    pub offhand_button: bool,
    pub shield_button: bool,
    /// Per control: [`FOLLOW_SCHEME`], [`SHOWN`] or [`HIDDEN`].
    pub button_visibility: [u8; CONTROLS],
    /// One row of placements per [`Mode`], indexed by [`Control`].
    pub layouts: [[Placement; CONTROLS]; 3],
    /// Camera turn speed for look drags; scales both axes.
    pub sensitivity: f32,
    /// Extra scale on the yaw (left and right) turn.
    pub sensitivity_horizontal: f32,
    /// Extra scale on the pitch (up and down) turn.
    pub sensitivity_vertical: f32,
    /// The sprint button toggles sprinting instead of holding it.
    pub sprint_toggle: bool,
    /// In the crosshair schemes, taps and holds on the world also attack and
    /// use (at the crosshair), as in the tap schemes. Meridian: the attack and
    /// use buttons then hide, unless the player shows them.
    pub crosshair_gestures: bool,
    /// In the crosshair schemes, dragging the attack button turns the camera.
    pub attack_button_look: bool,
    /// In the tap schemes, the use button shows while it would act on what
    /// the crosshair points at.
    pub contextual_interact: bool,
    /// The joystick appears where the finger lands, within the dynamic area.
    pub dynamic_joystick: bool,
    /// The dynamic area's width: a fraction of the surface, from the
    /// joystick's edge inwards.
    pub dynamic_area_x: f32,
    /// The dynamic area's height, centred on the joystick (1 is the whole height).
    pub dynamic_area_y: f32,
    /// Scales every control without changing the saved sizes (0.5-2).
    pub button_scale: f32,
    /// The touch marker's diameter in GUI pixels (16-64).
    pub circle_size: f32,
    /// Meridian: the inventory button is a cell at the hotbar's end, as Bedrock's "…" is,
    /// wherever the hotbar sits; moving it in the editor gives it its own place.
    pub inventory_on_hotbar: bool,
    /// Meridian: the version of the factory layout a saved file was made with ([`LAYOUT_VERSION`]);
    /// a file from an older one takes the current factory layout on load.
    #[serde(default)]
    pub layout_version: u32,
    /// Meridian: the key buttons the player added in the editor, per [`Mode`], last drawn on top.
    pub keys: [Vec<KeyButton>; 3],
    /// Meridian: the button that closes an inventory, per [`Mode`] (shown only over one).
    pub close: [Placement; 3],
    /// Meridian: game events (a block broken, a hit taken) buzz the phone.
    pub vibration: bool,
    /// Meridian: a short tick when a touch control is pressed.
    pub button_vibration: bool,
    /// Meridian: turning the phone turns the view too (after Starling's gyroscope aiming).
    pub gyro: bool,
    /// Meridian: how far the view turns for the phone's turn (0.1-3).
    pub gyro_sensitivity: f32,
}

/// Meridian: the keys the editor can add as buttons: the letters and F keys no built-in control
/// already gives (W, A, S, D, E, T, F3 and F5 have theirs). F1 is left out: it hides the HUD, and
/// the touch controls with it, so its own button couldn't bring them back. Pick isn't a key: it
/// sends the pick block action ([`super::PICK_BLOCK`]), which is bound to the middle mouse button.
pub const KEYS: [&str; 30] = [
    "B", "C", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "U", "V", "X", "Y",
    "Z", "F2", "F4", "F6", "F7", "F8", "F9", "F10", "F11", "F12", "Pick",
];

/// Meridian: a key button the player added: one of [`KEYS`] and where it is.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KeyButton {
    pub key: String,
    pub place: Placement,
    /// A tap presses the key and the next tap lets it go; false: held while the finger is.
    #[serde(default)]
    pub toggle: bool,
}

impl KeyButton {
    /// The key's name in [`KEYS`], for a button that holds one.
    pub fn name(&self) -> Option<&'static str> {
        KEYS.iter().copied().find(|k| *k == self.key)
    }
}

/// Meridian: where a new key button lands (the middle) and where the close button starts (the
/// top right corner, clear of the top row).
pub const NEW_KEY: Placement = Placement::new(0.5, 0.5, 0.12);
pub const CLOSE: Placement = Placement::new(0.955, 0.08, 0.11);

/// Meridian's factory layout. 2: Bedrock's (its two-column action cluster, its D-pad, the top
/// row as its icons, the inventory on the hotbar). 3: F3 shows in the top row, centred again.
/// 4: Swap hands joins the top row, centred around 0.50.
pub const LAYOUT_VERSION: u32 = 4;

/// The top row's factory x before F3 showed in it (layout version 2), and in version 3.
const TOP_ROW: [(Control, f32, f32); 5] = [
    (Control::Debug, 0.40, 0.42),
    (Control::Perspective, 0.44, 0.46),
    (Control::Chat, 0.48, 0.50),
    (Control::Pause, 0.52, 0.54),
    (Control::Settings, 0.56, 0.58),
];

/// The top row's factory x from version 3 to version 4 (centred around 0.50 with 0.05 spacing
/// so 6 buttons fit cleanly up to 16:10).
const TOP_ROW_V4: [(Control, f32, f32); 5] = [
    (Control::Debug, 0.42, 0.375),
    (Control::Perspective, 0.46, 0.425),
    (Control::Chat, 0.50, 0.475),
    (Control::Pause, 0.54, 0.525),
    (Control::Settings, 0.58, 0.575),
];

// Starling's default D-pad already supplies the movement, sneak and jump slots.
// Space the crosshair action squares out so they also fit narrower surfaces.
const DPAD_ACTION_POSITIONS: [(Control, Placement, Placement); 2] = [
    (
        Control::Attack,
        Placement::new(0.80, 0.64, 0.16),
        Placement::new(0.80, 0.60, 0.14),
    ),
    (
        Control::Use,
        Placement::new(0.92, 0.52, 0.16),
        Placement::new(0.92, 0.42, 0.14),
    ),
];

impl Default for Settings {
    fn default() -> Self {
        // Bedrock's layout (Meridian): the joystick on the left, and on the right its two columns,
        // jump over sneak over use beside sprint over attack; the top row is F3, perspective,
        // chat, pause, touch settings and swap hands, as Bedrock's top-bar icons.
        let row = [
            Placement::new(0.15, 0.58, 0.33),    // Move
            Placement::new(0.81, 0.52, 0.14),    // Sprint
            Placement::new(0.925, 0.42, 0.14),   // Jump
            Placement::new(0.925, 0.62, 0.14),   // Sneak
            Placement::new(0.81, 0.72, 0.14),    // Attack
            Placement::new(0.925, 0.82, 0.14),   // Use
            Placement::new(0.475, 0.055, 0.075), // Chat
            Placement::new(0.525, 0.055, 0.075), // Pause
            Placement::new(0.575, 0.055, 0.075), // Settings
            Placement::new(0.73, 0.95, 0.075),   // Inventory, unless on the hotbar
            Placement::new(0.375, 0.055, 0.075), // Debug
            Placement::new(0.425, 0.055, 0.075), // Perspective
            Placement::new(0.625, 0.055, 0.075), // Offhand
            Placement::new(0.81, 0.32, 0.12),    // Shield
        ];
        let mut layouts = [row; 3];
        // Bedrock's D-pad: large, where the joystick is, with sneak as its centre; jump alone on
        // the right.
        layouts[Mode::Dpad.index()][Control::Move as usize] = Placement::new(0.13, 0.74, 0.45);
        layouts[Mode::Dpad.index()][Control::Sneak as usize] = Placement::new(0.13, 0.74, 0.15);
        layouts[Mode::Dpad.index()][Control::Jump as usize] = Placement::new(0.90, 0.77, 0.165);
        for (control, _, placement) in DPAD_ACTION_POSITIONS {
            layouts[Mode::Dpad.index()][control as usize] = placement;
        }
        let button_visibility = [FOLLOW_SCHEME; CONTROLS];
        Self {
            mode: Mode::Crosshair,
            split_controls: false,
            left_handed: false,
            swap_jump_sneak: false,
            sprint_button: true,
            sprint_with_joystick: false,
            offhand_button: true,
            // Only in the tap scheme (with no use button) and while holding a shield.
            shield_button: true,
            button_visibility,
            layouts,
            sensitivity: 1.0,
            sensitivity_horizontal: 1.0,
            sensitivity_vertical: 1.0,
            sprint_toggle: false,
            crosshair_gestures: false,
            attack_button_look: true,
            contextual_interact: false,
            dynamic_joystick: false,
            dynamic_area_x: 0.25,
            dynamic_area_y: 1.0,
            button_scale: 1.0,
            circle_size: 32.0,
            inventory_on_hotbar: true,
            layout_version: LAYOUT_VERSION,
            keys: Default::default(),
            close: [CLOSE; 3],
            vibration: true,
            button_vibration: false,
            gyro: false,
            gyro_sensitivity: 1.0,
        }
    }
}

impl Settings {
    /// Reads `path`; a missing or unreadable file gives the defaults.
    pub fn load(path: &Path) -> Self {
        let mut settings: Self = std::fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        // A layout made before Bedrock's (Meridian) takes it: the placements, which buttons show
        // and the inventory on the hotbar. The player's other choices stay.
        // ponytail: a layout the player arranged before version 2 is replaced too; keep moved
        // controls if players ask for it.
        // Version 3: F3 shows (a player who hid it hides it again), and the factory top row
        // moves to stay centred. Moved buttons and the player's other choices stay.
        if settings.layout_version == 2 {
            let debug = &mut settings.button_visibility[Control::Debug as usize];
            if *debug == HIDDEN {
                *debug = FOLLOW_SCHEME;
            }
            for row in &mut settings.layouts {
                for (control, old, new) in TOP_ROW {
                    let p = &mut row[control as usize];
                    if p.center == [old, 0.055] {
                        p.center[0] = new;
                    }
                }
            }
            settings.layout_version = 3;
        }
        if settings.layout_version == 3 {
            for row in &mut settings.layouts {
                for (control, old, new) in TOP_ROW_V4 {
                    let p = &mut row[control as usize];
                    if p.center == [old, 0.055] {
                        p.center[0] = new;
                    }
                }
                let offhand = &mut row[Control::Offhand as usize];
                if offhand.center == [0.80, 0.96] && offhand.size == 0.075 {
                    offhand.center = [0.625, 0.055];
                }
            }
            if settings.button_visibility[Control::Offhand as usize] != HIDDEN {
                settings.offhand_button = true;
            }
            settings.layout_version = 4;
        }
        if settings.layout_version < LAYOUT_VERSION {
            let factory = Self::default();
            settings.layouts = factory.layouts;
            settings.button_visibility = factory.button_visibility;
            settings.shield_button = factory.shield_button;
            settings.inventory_on_hotbar = true;
            settings.layout_version = LAYOUT_VERSION;
        }
        // Update the old factory geometry on load, even if other buttons were edited.
        // A moved or resized action keeps its saved geometry and opacity.
        for (control, old, new) in DPAD_ACTION_POSITIONS {
            let place = &mut settings.layouts[Mode::Dpad.index()][control as usize];
            if place.center == old.center && place.size == old.size {
                place.center = new.center;
                place.size = new.size;
            }
        }
        settings.sanitize();
        settings
    }

    /// Writes `path` through a temporary file, so a crash never leaves half a layout.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let temp = path.with_extension("json.tmp");
        std::fs::write(&temp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(temp, path)
    }

    pub fn sanitize(&mut self) {
        // Only the D-pad has a split variant (see `Mode::SCHEMES`).
        if self.mode != Mode::Dpad {
            self.split_controls = false;
        }
        for v in &mut self.button_visibility {
            if *v > HIDDEN {
                *v = FOLLOW_SCHEME;
            }
        }
        for p in self.layouts.iter_mut().flatten() {
            p.sanitize();
        }
        // Meridian: only known keys, each once per scheme.
        for keys in &mut self.keys {
            let mut seen = Vec::new();
            keys.retain(|k| match k.name() {
                Some(name) if !seen.contains(&name) => {
                    seen.push(name);
                    true
                }
                _ => false,
            });
            for k in keys.iter_mut() {
                k.place.sanitize();
            }
        }
        for p in &mut self.close {
            p.sanitize();
        }
        for v in [
            &mut self.sensitivity,
            &mut self.sensitivity_horizontal,
            &mut self.sensitivity_vertical,
            &mut self.gyro_sensitivity,
        ] {
            *v = finite_or(*v, 1.0).clamp(0.1, 3.0);
        }
        self.button_scale = finite_or(self.button_scale, 1.0).clamp(0.5, 2.0);
        self.circle_size = finite_or(self.circle_size, 32.0).clamp(16.0, 64.0);
        self.dynamic_area_x = finite_or(self.dynamic_area_x, 0.25).clamp(0.0, 1.0);
        self.dynamic_area_y = finite_or(self.dynamic_area_y, 1.0).clamp(0.0, 1.0);
    }

    /// Whether attack and use act on the crosshair rather than on a tap.
    pub fn crosshair(&self) -> bool {
        self.mode == Mode::Crosshair || self.split_controls
    }

    pub fn visible(&self, c: Control) -> bool {
        match self.button_visibility[c as usize] {
            SHOWN => return true,
            HIDDEN => return false,
            _ => {}
        }
        match c {
            Control::Offhand => self.offhand_button,
            Control::Shield => self.shield_button && !self.visible(Control::Use),
            Control::Attack | Control::Use => self.crosshair() && !self.crosshair_gestures,
            Control::Sprint => {
                self.sprint_button && self.mode != Mode::Dpad && !self.sprint_with_joystick
            }
            _ => true,
        }
    }

    /// The layout slot a control is drawn in (jump and sneak may trade places).
    pub fn slot(&self, c: Control) -> usize {
        let swap = self.mode == Mode::Dpad && self.swap_jump_sneak;
        match c {
            Control::Jump if swap => Control::Sneak as usize,
            Control::Sneak if swap => Control::Jump as usize,
            _ => c as usize,
        }
    }

    /// Where a control is in the current scheme, mirrored for left hands.
    pub fn placement(&self, c: Control) -> Placement {
        self.mirrored(self.layouts[self.mode.index()][self.slot(c)])
    }

    /// Meridian: the current scheme's key buttons.
    pub fn mode_keys(&self) -> &[KeyButton] {
        &self.keys[self.mode.index()]
    }

    /// Meridian: where key button `i` of the current scheme is, mirrored for left hands.
    pub fn key_placement(&self, i: usize) -> Option<Placement> {
        self.mode_keys().get(i).map(|k| self.mirrored(k.place))
    }

    /// Meridian: where the close button is in the current scheme, mirrored for left hands.
    pub fn close_placement(&self) -> Placement {
        self.mirrored(self.close[self.mode.index()])
    }

    fn mirrored(&self, mut p: Placement) -> Placement {
        if self.left_handed {
            p.center[0] = 1.0 - p.center[0];
        }
        p
    }
}

/// Whether `p` lies in the rectangle `[x, y, w, h]` (edges included).
pub fn contains([x, y, w, h]: [f32; 4], p: [f32; 2]) -> bool {
    (x..=x + w).contains(&p[0]) && (y..=y + h).contains(&p[1])
}

fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("horizon-touch-{name}-{}.json", std::process::id()))
    }

    #[test]
    fn mode_indices_match_layout_rows_and_schemes() {
        for (i, mode) in Mode::ALL.into_iter().enumerate() {
            assert_eq!(mode.index(), i);
        }
        for (mode, split, _) in Mode::SCHEMES {
            assert!(!split || mode == Mode::Dpad);
        }
        for (i, c) in Control::ALL.into_iter().enumerate() {
            assert_eq!(c as usize, i);
        }
    }

    #[test]
    fn scheme_decides_attack_use_sprint_and_shield() {
        let mut s = Settings::default();
        assert!(s.visible(Control::Attack) && s.visible(Control::Use));
        assert!(s.visible(Control::Sprint));
        // The shield button only stands in when Use is hidden.
        assert!(!s.visible(Control::Shield));
        s.mode = Mode::Tap;
        assert!(!s.visible(Control::Attack) && !s.visible(Control::Use));
        assert!(s.visible(Control::Shield));
        s.mode = Mode::Dpad;
        assert!(!s.visible(Control::Sprint));
        s.split_controls = true;
        assert!(s.crosshair() && s.visible(Control::Attack));
        s.mode = Mode::Crosshair;
        s.sprint_with_joystick = true;
        assert!(!s.visible(Control::Sprint));
        assert!(s.visible(Control::Offhand));
        s.offhand_button = false;
        assert!(!s.visible(Control::Offhand));
    }

    #[test]
    fn player_visibility_overrides_the_scheme_and_survives_a_save() {
        let mut s = Settings {
            mode: Mode::Tap,
            ..Settings::default()
        };
        s.button_visibility[Control::Attack as usize] = SHOWN;
        s.button_visibility[Control::Chat as usize] = HIDDEN;
        assert!(s.visible(Control::Attack));
        assert!(!s.visible(Control::Chat));
        let file = temp_file("visibility");
        s.save(&file).unwrap();
        let read = Settings::load(&file);
        std::fs::remove_file(&file).unwrap();
        assert_eq!(read, s);
    }

    #[test]
    fn missing_bad_or_partial_files_fall_back_to_defaults() {
        assert_eq!(Settings::load(&temp_file("absent")), Settings::default());
        let file = temp_file("bad");
        std::fs::write(&file, b"{ not json").unwrap();
        assert_eq!(Settings::load(&file), Settings::default());
        std::fs::write(&file, br#"{"mode":"dpad","left_handed":true}"#).unwrap();
        let read = Settings::load(&file);
        std::fs::remove_file(&file).unwrap();
        assert_eq!(read.mode, Mode::Dpad);
        assert!(read.left_handed);
        assert_eq!(read.layouts, Settings::default().layouts);
        // A file from before key buttons and the close button: none, and the close button's corner.
        assert!(read.keys.iter().all(Vec::is_empty));
        assert_eq!(read.close, [CLOSE; 3]);
    }

    #[test]
    fn a_version_2_layout_shows_f3_and_centres_the_top_row_but_keeps_moved_buttons() {
        let mut old = Settings {
            mode: Mode::Tap,
            layout_version: 2,
            ..Settings::default()
        };
        old.button_visibility[Control::Debug as usize] = HIDDEN;
        for row in &mut old.layouts {
            for (control, x, _) in TOP_ROW {
                row[control as usize].center[0] = x;
            }
        }
        old.layouts[Mode::Tap.index()][Control::Chat as usize].center = [0.30, 0.2];
        let file = temp_file("version-2");
        old.save(&file).unwrap();
        let read = Settings::load(&file);
        std::fs::remove_file(&file).unwrap();
        assert_eq!(read.layout_version, LAYOUT_VERSION);
        assert!(read.visible(Control::Debug));
        assert_eq!(read.mode, Mode::Tap, "the player's scheme stays");
        let factory = Settings::default();
        for (control, _, new_x) in TOP_ROW_V4 {
            assert_eq!(factory.layouts[0][control as usize].center, [new_x, 0.055]);
            if control != Control::Chat {
                assert_eq!(read.placement(control).center, [new_x, 0.055], "{control:?}");
            }
        }
        assert_eq!(read.placement(Control::Offhand).center, [0.625, 0.055]);
        assert!(read.visible(Control::Offhand));
        assert_eq!(
            read.placement(Control::Chat).center,
            [0.30, 0.2],
            "a moved button stays"
        );
        // Version 3 -> 4 keeps a manually moved Offhand button where it is.
        let mut v3_moved = Settings {
            layout_version: 3,
            ..Settings::default()
        };
        v3_moved.layouts[0][Control::Offhand as usize].center = [0.22, 0.77];
        v3_moved.save(&file).unwrap();
        let read_v3 = Settings::load(&file);
        std::fs::remove_file(&file).unwrap();
        assert_eq!(read_v3.layout_version, LAYOUT_VERSION);
        assert_eq!(read_v3.placement(Control::Offhand).center, [0.22, 0.77]);
    }

    #[test]
    fn sanitize_brings_controls_back_on_screen() {
        let mut s = Settings {
            mode: Mode::Tap,
            split_controls: true,
            ..Settings::default()
        };
        s.button_visibility[0] = 9;
        s.layouts[0][0] = Placement {
            center: [f32::NAN, 7.0],
            size: 0.0,
            opacity: f32::INFINITY,
        };
        // A layout saved before toggle buttons: the key holds.
        let old: KeyButton =
            serde_json::from_str(r#"{"key":"G","place":{"center":[0.5,0.5],"size":0.12,"opacity":1.0}}"#)
                .unwrap();
        assert!(!old.toggle);
        let key = |key: &str| KeyButton {
            key: key.into(),
            place: NEW_KEY,
            toggle: false,
        };
        s.keys[0] = vec![key("G"), key("W"), key("G"), key("F12")];
        s.sanitize();
        let names: Vec<_> = s.keys[0].iter().map(|k| k.key.as_str()).collect();
        assert_eq!(names, ["G", "F12"], "known keys, once each");
        assert!(!s.split_controls);
        assert_eq!(s.button_visibility[0], FOLLOW_SCHEME);
        let p = s.layouts[0][0];
        assert_eq!(p.center, [0.5, 0.975]);
        assert_eq!(p.size, 0.045);
        assert_eq!(p.opacity, 0.65);
    }

    #[test]
    fn sensitivity_stays_finite_and_in_range() {
        let mut s = Settings {
            sensitivity: f32::INFINITY,
            sensitivity_horizontal: 9.0,
            sensitivity_vertical: 0.0,
            ..Settings::default()
        };
        s.sanitize();
        assert_eq!(
            [
                s.sensitivity,
                s.sensitivity_horizontal,
                s.sensitivity_vertical
            ],
            [1.0, 3.0, 0.1]
        );
        // Files saved before these options existed keep the defaults.
        let read: Settings = serde_json::from_str(r#"{"mode":"tap"}"#).unwrap();
        assert_eq!(read.sensitivity, 1.0);
    }

    #[test]
    fn phone_feedback_options_default_safely_and_stay_in_range() {
        // A file saved before these options existed keeps the defaults.
        let read: Settings = serde_json::from_str(r#"{"mode":"tap"}"#).unwrap();
        assert!(read.vibration && !read.button_vibration && !read.gyro);
        assert_eq!(read.gyro_sensitivity, 1.0);
        for (bad, fixed) in [(f32::NAN, 1.0), (0.0, 0.1), (9.0, 3.0), (2.0, 2.0)] {
            let mut s = Settings {
                gyro_sensitivity: bad,
                ..Settings::default()
            };
            s.sanitize();
            assert_eq!(s.gyro_sensitivity, fixed);
        }
    }

    #[test]
    fn control_options_default_safely_and_stay_in_range() {
        let read: Settings = serde_json::from_str(r#"{"mode":"tap"}"#).unwrap();
        assert!(read.attack_button_look && !read.sprint_toggle && !read.dynamic_joystick);
        assert_eq!([read.dynamic_area_x, read.dynamic_area_y], [0.25, 1.0]);
        let mut s = Settings {
            button_scale: f32::NAN,
            dynamic_area_x: -1.0,
            dynamic_area_y: f32::INFINITY,
            ..Settings::default()
        };
        s.sanitize();
        assert_eq!(
            [s.button_scale, s.dynamic_area_x, s.dynamic_area_y],
            [1.0, 0.0, 1.0]
        );
        s.button_scale = 9.0;
        s.sanitize();
        assert_eq!(s.button_scale, 2.0);
    }

    #[test]
    fn the_marker_size_defaults_and_stays_in_range() {
        let mut s: Settings = serde_json::from_str(r#"{"mode":"tap"}"#).unwrap();
        assert_eq!(s.circle_size, 32.0);
        for (bad, fixed) in [(f32::NAN, 32.0), (1.0, 16.0), (500.0, 64.0)] {
            s.circle_size = bad;
            s.sanitize();
            assert_eq!(s.circle_size, fixed);
        }
    }

    #[test]
    fn left_hands_mirror_and_dpad_can_swap_jump_and_sneak() {
        let mut s = Settings::default();
        let right = s.placement(Control::Jump).center;
        s.left_handed = true;
        assert_eq!(
            s.placement(Control::Jump).center,
            [1.0 - right[0], right[1]]
        );
        s.left_handed = false;
        s.mode = Mode::Dpad;
        s.swap_jump_sneak = true;
        let row = s.layouts[Mode::Dpad.index()];
        assert_eq!(s.placement(Control::Jump), row[Control::Sneak as usize]);
        assert_eq!(s.placement(Control::Sneak), row[Control::Jump as usize]);
        // Only the D-pad swaps.
        s.mode = Mode::Crosshair;
        assert_eq!(s.slot(Control::Jump), Control::Jump as usize);
    }

    #[test]
    fn controls_stay_square_on_wide_surfaces() {
        let p = Placement::new(0.5, 0.5, 0.2);
        let [x, y, w, h] = p.rect(2.0);
        assert_eq!([w, h], [0.1, 0.2]);
        assert!(contains([x, y, w, h], [0.5, 0.5]));
        assert!(contains([x, y, w, h], [x + w, y + h]));
        assert!(!contains([x, y, w, h], [0.56, 0.5]));
    }

    #[test]
    fn factory_controls_are_on_screen_in_every_mode() {
        let s = Settings::default();
        for row in s.layouts {
            for p in row {
                let mut clamped = p;
                clamped.sanitize();
                assert_eq!(clamped, p);
            }
        }
    }
}
