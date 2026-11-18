//! Shared North Star visual tokens, drawing helpers, and formatting utilities.

use super::ButtonId;
use crate::launcher_backend::{
    Account, Head, Profile, ProfileState, Progress, Setup, Stage, VANILLA,
};
use crate::menu_style;
use crate::ui_theme::{self, Face, Ui, color};

pub(super) mod palette {
    use super::color;

    pub fn bg_outer() -> u32 {
        color::hex(0x08020f, 255)
    }
    pub fn bg_mid() -> u32 {
        color::hex(0x0c0414, 235)
    }
    pub fn bg_inner() -> u32 {
        color::hex(0x100719, 225)
    }
    pub fn card() -> u32 {
        color::hex(0x140b21, 242)
    }
    pub fn surface() -> u32 {
        color::hex(0x1c112d, 255)
    }
    pub fn surface_hover() -> u32 {
        color::hex(0x26183d, 255)
    }
    pub fn border() -> u32 {
        color::hex(0x706c79, 120)
    }
    pub fn accent() -> u32 {
        color::hex(0xa575f6, 255)
    }
    pub fn accent_star() -> u32 {
        color::hex(0xa990ff, 255)
    }
    pub fn on_accent() -> u32 {
        color::hex(0x08020f, 255)
    }
    pub fn text() -> u32 {
        color::hex(0xffffff, 255)
    }
    pub fn body() -> u32 {
        color::hex(0xf9f9fa, 255)
    }
    pub fn muted() -> u32 {
        color::hex(0x9b94a8, 255)
    }
    pub fn pillar_muted() -> u32 {
        color::hex(0x706c79, 255)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tone {
    Primary,
    Secondary,
    Ghost,
    Danger,
    Banner,
}

pub(super) struct HitRect {
    pub id: ButtonId,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl HitRect {
    pub fn contains(&self, (cx, cy): (f32, f32)) -> bool {
        cx >= self.x && cx < self.x + self.w && cy >= self.y && cy < self.y + self.h
    }
}

pub(super) enum Canvas<'r, 'a> {
    Draw(&'r mut Ui<'a>),
    Hit(&'r Ui<'a>),
}

impl<'a> Canvas<'_, 'a> {
    pub fn ro(&self) -> &Ui<'a> {
        match self {
            Self::Draw(ui) => ui,
            Self::Hit(ui) => ui,
        }
    }

    pub fn paint(&mut self) -> Option<&mut Ui<'a>> {
        match self {
            Self::Draw(ui) => Some(ui),
            Self::Hit(_) => None,
        }
    }
}

pub(super) fn draw_backdrop(p: &mut Ui, w: f32, h: f32) {
    p.rounded(0.0, 0.0, w, h, 0.0, palette::bg_outer());
    let (cx, cy) = (w * 0.54, h * 0.46);
    for (scale, col) in [
        (1.06, color::with_alpha(palette::bg_mid(), 0.45)),
        (0.86, color::with_alpha(palette::bg_inner(), 0.45)),
        (0.66, color::with_alpha(palette::bg_inner(), 0.35)),
        (0.46, color::with_alpha(palette::accent(), 10.0 / 255.0)),
        (0.30, color::with_alpha(palette::accent_star(), 8.0 / 255.0)),
    ] {
        let rw = w * scale;
        let rh = h * scale;
        p.rounded(
            (cx - rw * 0.5).round(),
            (cy - rh * 0.5).round(),
            rw,
            rh,
            (rw.min(rh) * 0.48).max(24.0),
            col,
        );
    }
}

pub(super) fn draw_plate(ui: &mut Canvas<'_, '_>, x: f32, y: f32, w: f32, h: f32, border_col: u32) {
    if let Some(p) = ui.paint() {
        p.rounded(x, y, w, h, 14.0, palette::card());
        p.border(x, y, w, h, 14.0, border_col);
    }
}

/// Vector-free pixel construction of `03 North Star.svg`:
/// muted left pillar, white right pillar, violet diagonal sweep, and 4-point star at upper-right.
pub(super) fn draw_north_star_logo(p: &mut Ui, x: f32, y: f32, size: f32) {
    let s = size / 26.0;
    p.rounded(
        x + 3.0 * s,
        y + 6.0 * s,
        4.5 * s,
        14.0 * s,
        1.0,
        palette::pillar_muted(),
    );
    for step in 0..5 {
        let f = step as f32;
        p.rounded(
            x + (2.0 + f * 4.0) * s,
            y + (16.5 - f * 1.4) * s,
            6.0 * s,
            2.5 * s,
            1.0,
            palette::accent(),
        );
    }
    p.rounded(
        x + 17.5 * s,
        y + 5.0 * s,
        4.5 * s,
        15.0 * s,
        1.0,
        palette::text(),
    );
    let sx = x + 13.0 * s;
    let sy = y + 5.5 * s;
    p.rounded(
        sx - 4.0 * s,
        sy - 1.0 * s,
        8.0 * s,
        2.0 * s,
        0.5,
        palette::accent_star(),
    );
    p.rounded(
        sx - 1.0 * s,
        sy - 4.0 * s,
        2.0 * s,
        8.0 * s,
        0.5,
        palette::accent_star(),
    );
    p.rounded(
        sx - 2.0 * s,
        sy - 2.0 * s,
        4.0 * s,
        4.0 * s,
        1.0,
        palette::accent_star(),
    );
}

pub(super) fn draw_player_head(
    p: &mut Ui,
    x: f32,
    y: f32,
    size: f32,
    head: Option<&Head>,
    name: &str,
) {
    if let Some(rgba) = head {
        let step = size / 8.0;
        for row in 0..8 {
            for col in 0..8 {
                let i = (row * 8 + col) * 4;
                let a = rgba[i + 3];
                if a == 0 {
                    continue;
                }
                let rgb = (u32::from(rgba[i]) << 16)
                    | (u32::from(rgba[i + 1]) << 8)
                    | u32::from(rgba[i + 2]);
                p.rounded(
                    x + col as f32 * step,
                    y + row as f32 * step,
                    step.ceil(),
                    step.ceil(),
                    0.0,
                    color::hex(rgb, a),
                );
            }
        }
        p.border(x, y, size, size, 2.0, palette::border());
    } else {
        p.rounded(x, y, size, size, 4.0, palette::surface_hover());
        p.border(x, y, size, size, 4.0, palette::accent_star());
        let initial: String = name
            .chars()
            .next()
            .map_or('?', |c| c.to_ascii_uppercase())
            .to_string();
        let iw = p.measure(Face::SemiBold, size * 0.55, &initial);
        p.text_line(
            Face::SemiBold,
            size * 0.55,
            size,
            x + ((size - iw) * 0.5).round(),
            y,
            palette::text(),
            &initial,
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_input_box(
    ui: &mut Canvas<'_, '_>,
    hits: &mut Vec<HitRect>,
    id: ButtonId,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    label: &str,
    value: &str,
    masked: bool,
    focused: bool,
) {
    let hit = HitRect { id, x, y, w, h };
    if let Some(p) = ui.paint() {
        let bdr = if focused {
            palette::accent_star()
        } else {
            palette::border()
        };
        p.rounded(x, y, w, h, 8.0, palette::surface());
        p.border(x, y, w, h, 8.0, bdr);
        let mut tx = x + 12.0;
        if !label.is_empty() {
            p.text_line(
                Face::Medium,
                ui_theme::small(),
                h,
                tx,
                y,
                palette::muted(),
                label,
            );
            tx += p.measure(Face::Medium, ui_theme::small(), label) + 14.0;
        }
        let shown = if masked {
            "*".repeat(value.chars().count())
        } else {
            value.to_owned()
        };
        let display = if focused { format!("{shown}|") } else { shown };
        let fit = truncate_to_fit(
            p,
            Face::Mono,
            ui_theme::sizes().ui,
            &display,
            (x + w - 12.0 - tx).max(20.0),
        );
        p.text_line(
            Face::Mono,
            ui_theme::sizes().ui,
            h,
            tx,
            y,
            palette::text(),
            &fit,
        );
    }
    hits.push(hit);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_button(
    ui: &mut Canvas<'_, '_>,
    hits: &mut Vec<HitRect>,
    id: ButtonId,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    tone: Tone,
    label: &str,
) {
    let hit = HitRect { id, x, y, w, h };
    if let Some(p) = ui.paint() {
        let hovered = hit.contains(p.cursor);
        let danger = color::hex(menu_style::get().settings.danger, 255);
        let (plate, border, ink) = match (tone, hovered) {
            (Tone::Primary, false) => (palette::accent(), None, palette::on_accent()),
            (Tone::Primary, true) => (
                palette::accent_star(),
                Some(palette::text()),
                palette::on_accent(),
            ),
            (Tone::Banner, false) => (
                color::with_alpha(palette::accent(), 0.24),
                Some(palette::accent_star()),
                palette::text(),
            ),
            (Tone::Banner, true) => (
                color::with_alpha(palette::accent(), 0.36),
                Some(palette::text()),
                palette::text(),
            ),
            (Tone::Secondary, false) => {
                (palette::surface(), Some(palette::border()), palette::body())
            }
            (Tone::Secondary, true) => (
                palette::surface_hover(),
                Some(palette::accent_star()),
                palette::text(),
            ),
            (Tone::Ghost, false) => (0x0000_0000, None, palette::muted()),
            (Tone::Ghost, true) => (palette::surface(), None, palette::text()),
            (Tone::Danger, false) => (color::with_alpha(danger, 0.16), Some(danger), danger),
            (Tone::Danger, true) => (danger, Some(danger), palette::text()),
        };
        let radius = 10.0;
        if plate >> 24 != 0 {
            p.rounded(x, y, w, h, radius, plate);
        }
        if let Some(b) = border {
            p.border(x, y, w, h, radius, b);
        }
        let room = (w - 16.0).max(16.0);
        let font_size = p.fit(Face::Medium, ui_theme::sizes().ui, label, room);
        let tw = p.measure(Face::Medium, font_size, label);
        let lx = x + ((w - tw) * 0.5).round();
        p.text_line(Face::Medium, font_size, h, lx, y, ink, label);
    }
    hits.push(hit);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_sidebar_item(
    ui: &mut Canvas<'_, '_>,
    hits: &mut Vec<HitRect>,
    id: ButtonId,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    active: bool,
    label: &str,
) {
    let hit = HitRect { id, x, y, w, h };
    if let Some(p) = ui.paint() {
        let hovered = hit.contains(p.cursor);
        let (fill, bdr, ink) = if active {
            (
                color::with_alpha(palette::accent(), 0.22),
                Some(palette::accent_star()),
                palette::text(),
            )
        } else if hovered {
            (
                palette::surface_hover(),
                Some(palette::border()),
                palette::text(),
            )
        } else {
            (0x0000_0000, None, palette::muted())
        };
        if fill >> 24 != 0 {
            p.rounded(x, y, w, h, 10.0, fill);
        }
        if let Some(b) = bdr {
            p.border(x, y, w, h, 10.0, b);
        }
        if active {
            p.rounded(x + 3.0, y + 10.0, 3.0, h - 20.0, 1.5, palette::accent());
        }
        let font_size = p.fit(
            Face::Medium,
            ui_theme::sizes().ui,
            label,
            (w - 28.0).max(20.0),
        );
        p.text_line(Face::Medium, font_size, h, x + 16.0, y, ink, label);
    }
    hits.push(hit);
}

pub(super) fn count_label(n: usize, singular: &str, plural: &str) -> String {
    if n == 1 {
        format!("1 {singular}")
    } else {
        format!("{n} {plural}")
    }
}

pub(super) fn profile_stats_line(profile: &Profile) -> String {
    format!(
        "{}  |  {}  |  {}",
        count_label(profile.mods, "mod", "mods"),
        count_label(profile.worlds, "world", "worlds"),
        format_size(profile.size)
    )
}

pub(super) fn effective_profile_state(profile: &Profile, setup: &Setup) -> ProfileState {
    if profile.id == VANILLA {
        match setup {
            Setup::Needed => ProfileState::Available,
            Setup::Running(p) => ProfileState::Installing(p.clone()),
            Setup::Failed { error, .. } => ProfileState::Failed(error.clone()),
            Setup::Ready => ProfileState::Installed,
        }
    } else {
        profile.state.clone()
    }
}

pub(super) fn profile_badge_parts(
    state: &ProfileState,
    selected: bool,
    col_success: u32,
    col_danger: u32,
) -> (String, u32) {
    match state {
        ProfileState::Available => ("NEEDS INSTALL".to_owned(), palette::accent_star()),
        ProfileState::Installing(p) => {
            let pct = (stage_fraction(p) * 100.0).round() as u32;
            (format!("INSTALLING {pct}%"), palette::accent())
        }
        ProfileState::Installed if selected => ("READY".to_owned(), col_success),
        ProfileState::Installed => ("INSTALLED".to_owned(), palette::muted()),
        ProfileState::Failed(_) => ("FAILED".to_owned(), col_danger),
    }
}

pub(super) fn account_short(account: &Account) -> &str {
    match account {
        Account::Microsoft { name } | Account::Ely { name } | Account::Offline { name } => {
            name.as_str()
        }
    }
}

pub(super) fn account_provider_tag(account: &Account) -> &'static str {
    match account {
        Account::Microsoft { .. } => "Microsoft account",
        Account::Ely { .. } => "Ely.by account",
        Account::Offline { .. } => "Offline profile",
    }
}

pub(super) fn account_label(account: &Account) -> String {
    match account {
        Account::Microsoft { name } => format!("Microsoft ({name})"),
        Account::Ely { name } => format!("Ely.by ({name})"),
        Account::Offline { name } => format!("Offline ({name})"),
    }
}

pub(super) fn format_size(bytes: u64) -> String {
    if bytes == 0 {
        "Base game".to_owned()
    } else if bytes >= 1_000_000 {
        format!("{:.1} MB", bytes as f64 / 1_000_000.0)
    } else {
        format!("{:.0} KB", (bytes as f64 / 1_000.0).max(1.0))
    }
}

pub(super) fn stage_index(stage: Stage) -> usize {
    match stage {
        Stage::Check => 0,
        Stage::Download => 1,
        Stage::Verify => 2,
        Stage::Prepare => 3,
    }
}

pub(super) fn stage_name(stage: Stage) -> &'static str {
    match stage {
        Stage::Check => "Check",
        Stage::Download => "Download",
        Stage::Verify => "Verify",
        Stage::Prepare => "Prepare",
    }
}

pub(super) fn stage_heading(stage: Stage) -> &'static str {
    match stage {
        Stage::Check => "Checking installed game files",
        Stage::Download => "Downloading Minecraft 26.3 resources",
        Stage::Verify => "Verifying SHA-1 file integrity",
        Stage::Prepare => "Preparing runtime resources",
    }
}

pub(super) fn stage_fraction(progress: &Progress) -> f32 {
    if progress.total == 0 {
        return 0.0;
    }
    (progress.done as f64 / progress.total as f64).clamp(0.0, 1.0) as f32
}

pub(super) fn stage_pill_text(progress: &Progress) -> String {
    let pct = (stage_fraction(progress) * 100.0).round() as u32;
    if progress.stage == Stage::Download && progress.bytes_per_second > 0 {
        let mbps = progress.bytes_per_second as f64 / 1_000_000.0;
        format!("Downloading {pct}% ({mbps:.1} MB/s)")
    } else {
        format!("{} {pct}%", stage_name(progress.stage))
    }
}

pub(super) fn format_progress_stats(progress: &Progress, frac: f32) -> (String, String) {
    let pct = (frac * 100.0).round() as u32;
    match progress.stage {
        Stage::Download => {
            let done_mb = progress.done as f64 / 1_000_000.0;
            let total_mb = progress.total as f64 / 1_000_000.0;
            let left = if progress.total > 0 {
                format!("{done_mb:.1} / {total_mb:.1} MB")
            } else {
                format!("{done_mb:.1} MB")
            };
            let right = if progress.bytes_per_second > 0 && progress.total > progress.done {
                let speed = progress.bytes_per_second as f64 / 1_000_000.0;
                let eta = (progress.total - progress.done) / progress.bytes_per_second.max(1);
                format!("{pct}%  |  {speed:.1} MB/s  |  ETA {eta}s")
            } else {
                format!("{pct}%")
            };
            (left, right)
        }
        Stage::Prepare => {
            let left = if progress.total > 0 {
                format!("Step {} of {}", progress.done, progress.total)
            } else {
                "Running preparer...".to_owned()
            };
            (left, format!("{pct}%"))
        }
        Stage::Check | Stage::Verify => {
            let left = if progress.total > 0 {
                format!("{} / {} files", progress.done, progress.total)
            } else {
                "Scanning files...".to_owned()
            };
            (left, format!("{pct}%"))
        }
    }
}

pub(super) fn truncate_to_fit(ui: &Ui, face: Face, size: f32, text: &str, max_w: f32) -> String {
    if max_w <= 12.0 {
        return String::new();
    }
    if ui.measure(face, size, text) <= max_w {
        return text.to_owned();
    }
    let mut out = String::new();
    for ch in text.chars() {
        let candidate = format!("{out}{ch}...");
        if ui.measure(face, size, &candidate) > max_w {
            break;
        }
        out.push(ch);
    }
    format!("{out}...")
}

pub(super) fn wrap_lines(ui: &Ui, face: Face, size: f32, text: &str, max_w: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split(' ') {
        let candidate = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        if !line.is_empty() && ui.measure(face, size, &candidate) > max_w {
            lines.push(std::mem::replace(&mut line, word.to_owned()));
        } else {
            line = candidate;
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}
