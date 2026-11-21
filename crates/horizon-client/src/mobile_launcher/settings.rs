//! Skins preview and Launcher Settings views.

use super::widgets::{
    Canvas, HitRect, Tone, account_provider_tag, account_short, draw_button, draw_input_box,
    draw_plate, draw_player_head, draw_skin_3d, format_size, palette, truncate_to_fit, wrap_lines,
};
use super::{ButtonId, MobileLauncher};
use crate::launcher_backend::{Account, Setup, SkinUpload};
use crate::ui_theme::{self, Face, color};

impl MobileLauncher {
    /// The account's skin: a chosen PNG goes to a Microsoft account's profile, or stays on this
    /// phone for an offline account's game; an Ely.by account changes its own on ely.by.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_skins_view(
        &self,
        ui: &mut Canvas<'_, '_>,
        hits: &mut Vec<HitRect>,
        x: f32,
        y: f32,
        w: f32,
        avail_h: f32,
        account: &Option<Account>,
        col_success: u32,
        col_danger: u32,
    ) {
        let compact_v = avail_h < 300.0;

        let ix = x;
        let cy = y + if compact_v { 4.0 } else { 8.0 };

        // The whole figure, head to feet: the chosen file until it is put on, else the account's
        // own (its face alone until that has loaded).
        let tile_h = (y + avail_h - cy - 8.0).max(80.0);
        // Room for the figure turned and tilted: about 34 pixels tall, 18 wide.
        let texel = ((tile_h - 20.0) / 34.0).floor().max(2.0);
        let tile_w = 16.0 * texel + 32.0;
        let skin = self
            .skin_pick
            .as_ref()
            .map(|pick| (pick.skin.clone(), self.skin_slim))
            .or_else(|| account.as_ref().and_then(|a| self.backend.player_skin(a)));
        let name = account.as_ref().map_or("Player", account_short);
        let provider = account
            .as_ref()
            .map_or("Not signed in", account_provider_tag);
        let tx = ix + tile_w + 24.0;
        // Opening Skins turns the figure in to where it rests (0.9 s).
        let (page, _, since) = self.page_motion.get();
        let turned = if page == super::Route::Skins {
            self.motion.ease(since, 0.0, 0.9)
        } else {
            1.0
        };

        if let Some(p) = ui.paint() {
            if let Some((skin, slim)) = &skin {
                draw_skin_3d(
                    p,
                    ix + (tile_w * 0.5).round(),
                    cy + (tile_h * 0.5).round(),
                    texel,
                    skin,
                    *slim,
                    self.skin_yaw - 1.4 * (1.0 - turned),
                    self.skin_pitch,
                );
            } else {
                let head = account.as_ref().and_then(|a| self.backend.player_head(a));
                let head_sz = (tile_w - 40.0).min(104.0);
                draw_player_head(
                    p,
                    ix + ((tile_w - head_sz) * 0.5).round(),
                    cy + ((tile_h - head_sz) * 0.5).round(),
                    head_sz,
                    head.as_ref(),
                    name,
                );
            }
            p.text_line(
                Face::SemiBold,
                20.0,
                26.0,
                tx,
                cy + 4.0,
                palette::text(),
                name,
            );
            p.text_line(
                Face::Regular,
                ui_theme::small(),
                18.0,
                tx,
                cy + 32.0,
                palette::muted(),
                provider,
            );
        }

        let btn_h = if compact_v { 38.0 } else { 44.0 };
        let mut by = cy + if compact_v { 58.0 } else { 66.0 };
        let upload = self.backend.skin_upload();
        let status = match account {
            Some(Account::Microsoft { .. } | Account::Offline { .. }) => {
                draw_button(
                    ui,
                    hits,
                    ButtonId::PickSkin,
                    tx,
                    by,
                    170.0,
                    btn_h,
                    Tone::Secondary,
                    "Choose Skin File",
                );
                if self.skin_pick.is_some() {
                    for (i, (slim, label)) in [(false, "Classic Arms"), (true, "Slim Arms")]
                        .into_iter()
                        .enumerate()
                    {
                        draw_button(
                            ui,
                            hits,
                            ButtonId::SkinArms(slim),
                            tx + 182.0 + i as f32 * 136.0,
                            by,
                            128.0,
                            btn_h,
                            if self.skin_slim == slim {
                                Tone::Banner
                            } else {
                                Tone::Ghost
                            },
                            label,
                        );
                    }
                    by += btn_h + 10.0;
                    if self.skin_uploading {
                        Some(("Putting it on...".to_owned(), palette::muted()))
                    } else {
                        draw_button(
                            ui,
                            hits,
                            ButtonId::UseSkin,
                            tx,
                            by,
                            170.0,
                            btn_h,
                            Tone::Primary,
                            "Use This Skin",
                        );
                        match upload {
                            SkinUpload::Failed(why) => Some((why, col_danger)),
                            _ => None,
                        }
                    }
                } else if upload == SkinUpload::Done {
                    Some(("Skin updated.".to_owned(), col_success))
                } else {
                    None
                }
            }
            Some(Account::Ely { .. }) => {
                draw_button(
                    ui,
                    hits,
                    ButtonId::OpenElySkins,
                    tx,
                    by,
                    210.0,
                    btn_h,
                    Tone::Secondary,
                    "Change Skin on ely.by",
                );
                None
            }
            None => None,
        };
        let status = self
            .skin_error
            .clone()
            .map(|why| (why, col_danger))
            .or(status);
        if let (Some((text, col)), Some(p)) = (status, ui.paint()) {
            let room = (x + w - 22.0 - tx).max(0.0);
            let fit = truncate_to_fit(p, Face::Regular, ui_theme::small(), &text, room);
            p.text_line(
                Face::Regular,
                ui_theme::small(),
                18.0,
                tx,
                by + btn_h + 10.0,
                col,
                &fit,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_settings_view(
        &self,
        ui: &mut Canvas<'_, '_>,
        hits: &mut Vec<HitRect>,
        x: f32,
        y: f32,
        w: f32,
        avail_h: f32,
        onboarding: bool,
        setup: &Setup,
        col_success: u32,
        col_danger: u32,
    ) {
        let compact_v = avail_h < 300.0;
        let card_h = if onboarding {
            318.0_f32.min(avail_h)
        } else {
            avail_h
        };
        let cy_top = if onboarding {
            y + ((avail_h - card_h) * 0.5).max(0.0).round()
        } else {
            y
        };
        // Before sign-in there is no rail, so Settings stays one card. After it, the page is the backdrop.
        if onboarding {
            draw_plate(ui, x, cy_top, w, card_h, palette::border());
        }

        let inset = if onboarding { 22.0 } else { 0.0 };
        let inner_x = x + inset;
        let inner_w = w - inset * 2.0;
        let mut cy = cy_top
            + if onboarding {
                if compact_v { 10.0 } else { 20.0 }
            } else {
                0.0
            };
        if onboarding {
            let hdr_h = if compact_v { 32.0 } else { 38.0 };
            draw_button(
                ui,
                hits,
                ButtonId::SettingsBack,
                inner_x,
                cy,
                96.0,
                hdr_h,
                Tone::Secondary,
                "Back",
            );
            if let Some(p) = ui.paint() {
                p.text_line(
                    Face::SemiBold,
                    if compact_v { 19.0 } else { 22.0 },
                    hdr_h,
                    inner_x + 112.0,
                    cy,
                    palette::text(),
                    "Launcher Settings",
                );
            }
            cy += hdr_h + if compact_v { 10.0 } else { 18.0 };
        }

        // One list: each row's name on the left, its control in one column on the right that
        // every row shares.
        let row_h: f32 = if compact_v { 38.0 } else { 42.0 };
        let ctl_h: f32 = if compact_v { 30.0 } else { 34.0 };
        let col_w = (inner_w * 0.42).clamp(200.0, 320.0);
        let col_x = inner_x + inner_w - col_w;
        let card_end = cy_top + card_h - 6.0;
        let ctl_y = |cy: f32| cy + ((row_h - ctl_h) * 0.5).round();
        // Typing the game's arguments with the keyboard up: that field is all that shows.
        let typing_args = self.game_args_focused && avail_h < 220.0;
        let ui_size = ui_theme::sizes().ui;

        // A row's name, and the hairline under the row (`h` tall).
        let name = |ui: &mut Canvas<'_, '_>, cy: f32, h: f32, text: &str| {
            if let Some(p) = ui.paint() {
                let line = palette::border_subtle();
                p.rounded(inner_x, cy + h - 1.0, inner_w, 1.0, 0.0, line);
                p.text_line(
                    Face::Medium,
                    ui_size,
                    row_h,
                    inner_x,
                    cy,
                    palette::text(),
                    text,
                );
            }
        };
        // A fact at the column's right end.
        let value = |ui: &mut Canvas<'_, '_>, cy: f32, text: &str, ink: u32| {
            if let Some(p) = ui.paint() {
                let fit = truncate_to_fit(p, Face::Regular, ui_theme::small(), text, col_w);
                let tw = p.measure(Face::Regular, ui_theme::small(), &fit);
                let tx = inner_x + inner_w - tw;
                p.text_line(Face::Regular, ui_theme::small(), row_h, tx, cy, ink, &fit);
            }
        };
        // Buttons at the column's right end, each as wide as its label; returns the leftmost
        // button's left edge.
        let buttons = |ui: &mut Canvas<'_, '_>,
                       hits: &mut Vec<HitRect>,
                       cy: f32,
                       items: &[(ButtonId, &str)]|
         -> f32 {
            let mut bx = inner_x + inner_w;
            for &(id, label) in items.iter().rev() {
                let bw = (ui.ro().measure(Face::Medium, ui_size, label) + 32.0).max(84.0);
                bx -= bw;
                draw_button(
                    ui,
                    hits,
                    id,
                    bx,
                    ctl_y(cy),
                    bw,
                    ctl_h,
                    Tone::Secondary,
                    label,
                );
                bx -= 8.0;
            }
            bx + 8.0
        };

        let reclaim = self.backend.reclaimable().filter(|&bytes| bytes > 0);
        let offer = matches!(setup, Setup::Ready).then_some(reclaim).flatten();
        let full = !onboarding && !typing_args;
        let free_shown = full && (offer.is_some() || self.freed.is_some());
        let error_h = if self.game_args_error.is_some() {
            18.0
        } else {
            0.0
        };
        // The rows under the title, as a drag scrolled them: rows scrolled past and rows past the
        // card's end aren't drawn, and how many are below sets how far a drag goes.
        let total = 2 * usize::from(!typing_args)
            + usize::from(!onboarding)
            + usize::from(free_shown)
            + 5 * usize::from(full);
        let mut rows = Rows {
            skip: if typing_args {
                0
            } else {
                self.page_offset().min(total.saturating_sub(1))
            },
            seen: 0,
            below: 0,
            y: cy,
            end: card_end,
        };

        // Two choices side by side, `right` chosen when true: the one not chosen takes the tap.
        let switch = |ui: &mut Canvas<'_, '_>,
                      hits: &mut Vec<HitRect>,
                      cy: f32,
                      id: ButtonId,
                      labels: [&str; 2],
                      right: bool| {
            let (by, half) = (ctl_y(cy), col_w * 0.5);
            if let Some(p) = ui.paint() {
                p.rounded(col_x, by, col_w, ctl_h, 10.0, palette::surface());
                p.border(col_x, by, col_w, ctl_h, 10.0, palette::border());
                let chosen_x = col_x + if right { half } else { 0.0 };
                let chosen = color::with_alpha(palette::accent(), 0.26);
                p.rounded(
                    chosen_x + 3.0,
                    by + 3.0,
                    half - 6.0,
                    ctl_h - 6.0,
                    7.0,
                    chosen,
                );
                for (i, label) in labels.into_iter().enumerate() {
                    let ink = if (i == 1) == right {
                        palette::text()
                    } else {
                        palette::muted()
                    };
                    let tw = p.measure(Face::Medium, ui_size, label);
                    let lx = col_x + i as f32 * half + ((half - tw) * 0.5).round();
                    p.text_line(Face::Medium, ui_size, ctl_h, lx, by, ink, label);
                }
            }
            let x = col_x + if right { 0.0 } else { half };
            hits.push(HitRect {
                id,
                x,
                y: by,
                w: half,
                h: ctl_h,
            });
        };
        if !typing_args && let Some(cy) = rows.next(row_h, row_h) {
            name(ui, cy, row_h, "Launcher Font");
            let pixel = ui_theme::pixel_font();
            switch(
                ui,
                hits,
                cy,
                ButtonId::ToggleLauncherFont,
                ["Modern", "Pixel"],
                pixel,
            );
        }
        // Moving pages, pop-ups and lists (`motion.rs`); off, everything changes at once.
        if !typing_args && let Some(cy) = rows.next(row_h, row_h) {
            name(ui, cy, row_h, "Animations");
            let off = !self.motion.on.get();
            switch(ui, hits, cy, ButtonId::ToggleAnimations, ["On", "Off"], off);
        }

        // The game's arguments: words, "quoted words" as one; why they were refused, under them.
        if !onboarding && let Some(cy) = rows.next(row_h + error_h, row_h + error_h) {
            name(ui, cy, row_h + error_h, "Game Arguments");
            let saved = self.backend.game_args();
            let text = if self.game_args_focused {
                self.game_args_draft.as_str()
            } else {
                saved.as_str()
            };
            draw_input_box(
                ui,
                hits,
                ButtonId::FocusGameArgs,
                col_x,
                ctl_y(cy),
                col_w,
                ctl_h,
                if text.is_empty() { "None" } else { "" },
                text,
                false,
                self.game_args_focused,
            );
            if let Some(err) = &self.game_args_error
                && let Some(p) = ui.paint()
            {
                let fit = truncate_to_fit(p, Face::Regular, ui_theme::small(), err, inner_w);
                let tw = p.measure(Face::Regular, ui_theme::small(), &fit);
                let tx = inner_x + inner_w - tw;
                p.text_line(
                    Face::Regular,
                    ui_theme::small(),
                    error_h,
                    tx,
                    cy + row_h - 4.0,
                    col_danger,
                    &fit,
                );
            }
        }

        // What only installing uses, offered once the game is installed and the cache measured;
        // after freeing, what happened.
        if free_shown && let Some(cy) = rows.next(row_h, row_h) {
            name(ui, cy, row_h, "Free Up Space");
            match (&self.freed, offer) {
                (Some(Err(err)), _) => value(ui, cy, err, col_danger),
                (_, Some(bytes)) => {
                    let label = format!("Free {}", format_size(bytes));
                    buttons(ui, hits, cy, &[(ButtonId::FreeSpace, label.as_str())]);
                }
                (Some(Ok(bytes)), None) => {
                    let text = format!("Freed {}", format_size(*bytes));
                    value(ui, cy, &text, col_success);
                }
                (None, None) => {}
            }
        }

        if full {
            if let Some(cy) = rows.next(row_h, row_h) {
                name(ui, cy, row_h, "Version");
                let version = concat!(
                    "Meridian ",
                    env!("CARGO_PKG_VERSION"),
                    "  ·  Minecraft 26.3"
                );
                value(ui, cy, version, palette::body());
            }
            if let Some(cy) = rows.next(row_h, row_h) {
                name(ui, cy, row_h, "Open Source");
                let links = [
                    (ButtonId::OpenSourceCode, "Source Code"),
                    (ButtonId::OpenLicenses, "Licenses"),
                ];
                buttons(ui, hits, cy, &links);
            }
            if let Some(cy) = rows.next(row_h, row_h) {
                name(ui, cy, row_h, "Quit Launcher");
                buttons(ui, hits, cy, &[(ButtonId::Quit, "Quit")]);
            }
            if let Some(cy) = rows.next(row_h, row_h) {
                name(ui, cy, row_h, "Diagnostics");
                if self.diagnostics_exporting {
                    value(ui, cy, "Exporting...", palette::muted());
                } else {
                    let btn_x = buttons(ui, hits, cy, &[(ButtonId::ExportDiagnostics, "Export")]);
                    if let Some(result) = &self.diagnostics
                        && let Some(p) = ui.paint()
                    {
                        let (text, ok) = diagnostics_status(result);
                        let ink = if ok { col_success } else { col_danger };
                        let left = inner_x + p.measure(Face::Medium, ui_size, "Diagnostics") + 16.0;
                        let room = (btn_x - 12.0 - left).max(0.0);
                        let fit = truncate_to_fit(p, Face::Regular, ui_theme::small(), &text, room);
                        let tw = p.measure(Face::Regular, ui_theme::small(), &fit);
                        p.text_line(
                            Face::Regular,
                            ui_theme::small(),
                            row_h,
                            btn_x - 12.0 - tw,
                            cy,
                            ink,
                            &fit,
                        );
                    }
                }
            }
            if let (Some(cy), Some(p)) = (rows.next(22.0, 20.0), ui.paint()) {
                let note = "Not an official Minecraft product. Not approved by or associated \
                            with Mojang or Microsoft.";
                let fit = truncate_to_fit(p, Face::Regular, ui_theme::small(), note, inner_w);
                p.text_line(
                    Face::Regular,
                    ui_theme::small(),
                    22.0,
                    inner_x,
                    cy + 4.0,
                    palette::muted(),
                    &fit,
                );
            }
        }
        self.scroll_max.set(rows.skip + rows.below);
    }
    /// "Free up space?": what goes, what stays, and what it costs later.
    pub(super) fn draw_free_space_confirm(
        &self,
        ui: &mut Canvas<'_, '_>,
        hits: &mut Vec<HitRect>,
        x: f32,
        y: f32,
        w: f32,
        avail_h: f32,
    ) {
        let size = format_size(self.backend.reclaimable().unwrap_or(0));
        let compact_v = avail_h < 280.0;
        let card_h = (if compact_v { 196.0_f32 } else { 224.0 }).min(avail_h);
        let cy_top = y + ((avail_h - card_h) * 0.5).max(0.0).round();
        draw_plate(ui, x, cy_top, w, card_h, palette::border());
        let inner_x = x + 24.0;
        let inner_w = w - 48.0;
        let btn_h = if compact_v { 40.0 } else { 46.0 };
        let btn_y = cy_top + card_h - btn_h - if compact_v { 10.0 } else { 16.0 };
        if let Some(p) = ui.paint() {
            let mut cy = cy_top + if compact_v { 12.0 } else { 20.0 };
            p.text_line(
                Face::SemiBold,
                if compact_v { 19.0 } else { 22.0 },
                if compact_v { 26.0 } else { 32.0 },
                inner_x,
                cy,
                palette::text(),
                &format!("Free up {size}?"),
            );
            cy += if compact_v { 30.0 } else { 40.0 };
            let body = "Deletes the download cache and Java, which only installing uses. Your worlds and profiles stay. An update downloads them again."
                .to_owned();
            let lines = ((btn_y - 8.0 - cy) / 20.0).floor().max(0.0) as usize;
            for (i, line) in wrap_lines(p, Face::Regular, ui_theme::sizes().ui, &body, inner_w)
                .into_iter()
                .take(lines)
                .enumerate()
            {
                p.text_line(
                    Face::Regular,
                    ui_theme::sizes().ui,
                    20.0,
                    inner_x,
                    cy + i as f32 * 20.0,
                    palette::body(),
                    &line,
                );
            }
        }
        let half_w = (inner_w - 12.0) * 0.5;
        draw_button(
            ui,
            hits,
            ButtonId::CancelFreeSpace,
            inner_x,
            btn_y,
            half_w,
            btn_h,
            Tone::Secondary,
            "Keep Files",
        );
        draw_button(
            ui,
            hits,
            ButtonId::ConfirmFreeSpace,
            inner_x + half_w + 12.0,
            btn_y,
            half_w,
            btn_h,
            Tone::Primary,
            &format!("Free Up {size}"),
        );
    }
}

/// Status text for Settings' Diagnostics row: `(text, ok)` where `ok` uses the success colour
/// and `false` uses the danger colour.
pub(super) fn diagnostics_status(result: &Result<String, String>) -> (String, bool) {
    match result {
        Ok(file) => (format!("Shared {file}"), true),
        Err(err) => (err.clone(), false),
    }
}

/// Rows down a page, a drag scrolled: the first `skip` aren't drawn, nor those past `end`.
struct Rows {
    skip: usize,
    seen: usize,
    /// Rows past `end`.
    below: usize,
    y: f32,
    end: f32,
}

impl Rows {
    /// The next row's top, `step` above the one after it and `h` tall; `None` when scrolled past
    /// or below the end.
    fn next(&mut self, step: f32, h: f32) -> Option<f32> {
        self.seen += 1;
        if self.seen <= self.skip {
            return None;
        }
        if self.below > 0 || self.y + h > self.end {
            self.below += 1;
            return None;
        }
        let y = self.y;
        self.y += step;
        Some(y)
    }
}
