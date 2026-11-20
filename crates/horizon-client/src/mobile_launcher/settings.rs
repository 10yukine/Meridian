//! Skins preview and Launcher Settings views.

use super::widgets::{
    Canvas, HitRect, Tone, account_label, account_provider_tag, account_short, count_label,
    draw_button, draw_input_box, draw_plate, draw_player_head, draw_skin_body, format_size,
    palette, truncate_to_fit, wrap_lines,
};
use super::{ButtonId, MobileLauncher};
use crate::launcher_backend::{Account, Profile, Setup, SkinUpload};
use crate::ui_theme::{self, Face};

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
        draw_plate(ui, x, y, w, avail_h, palette::border());

        let ix = x + 22.0;
        let mut cy = y + if compact_v { 12.0 } else { 22.0 };

        if let Some(p) = ui.paint() {
            p.text_line(
                Face::SemiBold,
                if compact_v { 19.0 } else { 22.0 },
                28.0,
                ix,
                cy,
                palette::text(),
                "Skin",
            );
            p.rounded(
                x + 1.0,
                cy + if compact_v { 30.0 } else { 38.0 },
                w - 2.0,
                1.0,
                0.0,
                palette::border_subtle(),
            );
        }
        cy += if compact_v { 38.0 } else { 52.0 };

        // The whole figure, head to feet: the chosen file until it is put on, else the account's
        // own (its face alone until that has loaded).
        let tile_h = (y + avail_h - cy - if compact_v { 10.0 } else { 18.0 }).max(80.0);
        let texel = ((tile_h - 20.0) / 32.0).floor().max(2.0);
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

        if let Some(p) = ui.paint() {
            p.rounded(ix, cy, tile_w, tile_h, 14.0, palette::surface());
            p.border(ix, cy, tile_w, tile_h, 14.0, palette::border());
            if let Some((skin, slim)) = &skin {
                let figure_w = if *slim { 14.0 } else { 16.0 } * texel;
                draw_skin_body(
                    p,
                    ix + ((tile_w - figure_w) * 0.5).round(),
                    cy + ((tile_h - 32.0 * texel) * 0.5).round(),
                    texel,
                    skin,
                    *slim,
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
        account: &Option<Account>,
        setup: &Setup,
        profiles: &[Profile],
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
        draw_plate(ui, x, cy_top, w, card_h, palette::border());

        let inner_x = x + 22.0;
        let inner_w = w - 44.0;
        let mut cy = cy_top + if compact_v { 10.0 } else { 20.0 };
        let hdr_h = if compact_v { 32.0 } else { 38.0 };

        if onboarding {
            draw_button(
                ui,
                hits,
                ButtonId::SettingsBack,
                inner_x,
                cy,
                96.0,
                hdr_h,
                Tone::Secondary,
                "<- Back",
            );
        } else {
            draw_button(
                ui,
                hits,
                ButtonId::Quit,
                inner_x + inner_w - 136.0,
                cy,
                136.0,
                hdr_h,
                Tone::Secondary,
                "Quit Launcher",
            );
        }
        if let Some(p) = ui.paint() {
            let title_x = if onboarding { inner_x + 112.0 } else { inner_x };
            p.text_line(
                Face::SemiBold,
                if compact_v { 19.0 } else { 22.0 },
                hdr_h,
                title_x,
                cy,
                palette::text(),
                "Launcher Settings",
            );
            p.rounded(
                x + 1.0,
                cy + hdr_h + if compact_v { 4.0 } else { 8.0 },
                w - 2.0,
                1.0,
                0.0,
                palette::border_subtle(),
            );
        }
        cy += hdr_h + if compact_v { 10.0 } else { 18.0 };

        let row_h: f32 = if compact_v { 32.0 } else { 46.0 };
        let row_step: f32 = if compact_v { 36.0 } else { 54.0 };
        let gap = 8.0;
        let half_w = ((inner_w - gap) * 0.5).floor();
        let ctl_h = (row_h - 6.0).max(26.0);
        let ctl_w = (half_w * 0.46).clamp(110.0, 184.0);
        let card_end = cy_top + card_h - 6.0;
        // Typing the game's arguments with the keyboard up: that field is all that shows.
        let typing_args = self.game_args_focused && avail_h < 220.0;

        // A cell's plate and name; its control or value goes on its right.
        let plate = |ui: &mut Canvas<'_, '_>, cx: f32, cy: f32, cw: f32, name: &str| {
            if let Some(p) = ui.paint() {
                p.rounded(cx, cy, cw, row_h, 8.0, palette::surface());
                p.text_line(
                    Face::Medium,
                    ui_theme::sizes().ui,
                    row_h,
                    cx + 14.0,
                    cy,
                    palette::text(),
                    name,
                );
            }
        };
        let value = |ui: &mut Canvas<'_, '_>, cx: f32, cy: f32, cw: f32, text: &str, ink: u32| {
            if let Some(p) = ui.paint() {
                let fit = truncate_to_fit(p, Face::Regular, ui_theme::small(), text, cw * 0.58);
                let tw = p.measure(Face::Regular, ui_theme::small(), &fit);
                p.text_line(
                    Face::Regular,
                    ui_theme::small(),
                    row_h,
                    cx + cw - 14.0 - tw,
                    cy,
                    ink,
                    &fit,
                );
            }
        };

        // The launcher's state, two cells to a row.
        let reclaim = self.backend.reclaimable().filter(|&bytes| bytes > 0);
        let offer = matches!(setup, Setup::Ready).then_some(reclaim).flatten();
        let free_shown = !onboarding && (offer.is_some() || self.freed.is_some());
        #[derive(Clone, Copy)]
        enum Cell {
            Font,
            FreeSpace,
            Account,
            Profiles,
        }
        let cells: Vec<Cell> = [
            Some(Cell::Font),
            free_shown.then_some(Cell::FreeSpace),
            Some(Cell::Account),
            Some(Cell::Profiles),
        ]
        .into_iter()
        .flatten()
        .collect();
        if !typing_args {
            for (i, cell) in cells.iter().enumerate() {
                let cx = inner_x + if i % 2 == 0 { 0.0 } else { half_w + gap };
                let ccy = cy + (i / 2) as f32 * row_step;
                if ccy + row_h > card_end {
                    break;
                }
                let ctl_x = cx + half_w - 6.0 - ctl_w;
                let ctl_y = ccy + ((row_h - ctl_h) * 0.5).round();
                match cell {
                    Cell::Font => {
                        plate(ui, cx, ccy, half_w, "Launcher Font");
                        // Modern (Geist) by default; the Minecraft pixel font once 26.3's art is
                        // in (persisted in horizon.txt via ui_theme::preference).
                        let pixel_font = ui_theme::pixel_font();
                        let label = if pixel_font && ui.ro().pack_font_available() {
                            "Pixel (Minecraft)"
                        } else if pixel_font {
                            "Pixel (after 26.3)"
                        } else {
                            "Modern (Geist)"
                        };
                        let tone = if pixel_font {
                            Tone::Banner
                        } else {
                            Tone::Secondary
                        };
                        let id = ButtonId::ToggleLauncherFont;
                        draw_button(ui, hits, id, ctl_x, ctl_y, ctl_w, ctl_h, tone, label);
                    }
                    // What only installing uses, offered once the game is installed and the
                    // cache measured; after freeing, what happened.
                    Cell::FreeSpace => {
                        plate(ui, cx, ccy, half_w, "Free Up Space");
                        match (&self.freed, offer) {
                            (Some(Err(err)), _) => value(ui, cx, ccy, half_w, err, col_danger),
                            (_, Some(bytes)) => draw_button(
                                ui,
                                hits,
                                ButtonId::FreeSpace,
                                ctl_x,
                                ctl_y,
                                ctl_w,
                                ctl_h,
                                Tone::Secondary,
                                &format!("Free {}", format_size(bytes)),
                            ),
                            (Some(Ok(bytes)), None) => {
                                let text = format!("Freed {}", format_size(*bytes));
                                value(ui, cx, ccy, half_w, &text, col_success);
                            }
                            (None, None) => {}
                        }
                    }
                    Cell::Account => {
                        plate(ui, cx, ccy, half_w, "Account");
                        let text = account
                            .as_ref()
                            .map_or_else(|| "Not signed in".to_owned(), account_label);
                        value(ui, cx, ccy, half_w, &text, palette::body());
                    }
                    Cell::Profiles => {
                        plate(ui, cx, ccy, half_w, "Profiles");
                        let bytes: u64 = profiles.iter().map(|p| p.size).sum();
                        let text = format!(
                            "{}, {}",
                            count_label(profiles.len(), "profile", "profiles"),
                            format_size(bytes)
                        );
                        value(ui, cx, ccy, half_w, &text, palette::body());
                    }
                }
            }
            cy += cells.len().div_ceil(2) as f32 * row_step;
        }

        // The game's arguments, below everything: words, "quoted words" as one.
        if !onboarding && cy + row_h <= card_end {
            let name = "Game Arguments";
            plate(ui, inner_x, cy, inner_w, name);
            let name_w = ui.ro().measure(Face::Medium, ui_theme::sizes().ui, name) + 30.0;
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
                inner_x + name_w,
                cy + 3.0,
                inner_w - name_w - 4.0,
                row_h - 6.0,
                if text.is_empty() { "None" } else { "" },
                text,
                false,
                self.game_args_focused,
            );
            cy += row_step;
            if let Some(err) = &self.game_args_error {
                if let Some(p) = ui.paint() {
                    let fit = truncate_to_fit(p, Face::Regular, ui_theme::small(), err, inner_w);
                    p.text_line(
                        Face::Regular,
                        ui_theme::small(),
                        18.0,
                        inner_x + 14.0,
                        cy - 4.0,
                        col_danger,
                        &fit,
                    );
                }
                cy += 16.0;
            }
        }

        // About, underneath.
        if !onboarding && !typing_args && cy + row_h <= card_end {
            plate(ui, inner_x, cy, inner_w, "About");
            let links_w = 2.0 * ctl_w.min(140.0) + 8.0;
            let link_w = ctl_w.min(140.0);
            let link_y = cy + ((row_h - ctl_h) * 0.5).round();
            let right = inner_x + inner_w - 6.0;
            draw_button(
                ui,
                hits,
                ButtonId::OpenLicenses,
                right - link_w,
                link_y,
                link_w,
                ctl_h,
                Tone::Secondary,
                "Licenses",
            );
            draw_button(
                ui,
                hits,
                ButtonId::OpenSourceCode,
                right - links_w,
                link_y,
                link_w,
                ctl_h,
                Tone::Secondary,
                "Source Code",
            );
            if let Some(p) = ui.paint() {
                let name_w = p.measure(Face::Medium, ui_theme::sizes().ui, "About") + 30.0;
                let about = concat!(
                    "Meridian ",
                    env!("CARGO_PKG_VERSION"),
                    "  ·  Minecraft 26.3  ·  Horizon engine"
                );
                let room = (inner_w - name_w - links_w - 20.0).max(0.0);
                let fit = truncate_to_fit(p, Face::Regular, ui_theme::small(), about, room);
                p.text_line(
                    Face::Regular,
                    ui_theme::small(),
                    row_h,
                    inner_x + name_w,
                    cy,
                    palette::body(),
                    &fit,
                );
                if cy + row_step + 16.0 <= card_end {
                    let note = "Not an official Minecraft product. Not approved by or associated \
                                with Mojang or Microsoft.";
                    let fit = truncate_to_fit(p, Face::Regular, ui_theme::small(), note, inner_w);
                    p.text_line(
                        Face::Regular,
                        ui_theme::small(),
                        18.0,
                        inner_x + 14.0,
                        cy + row_step - 2.0,
                        palette::muted(),
                        &fit,
                    );
                }
            }
        }
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
