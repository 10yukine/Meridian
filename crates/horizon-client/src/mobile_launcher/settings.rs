//! Skins preview and Launcher Settings views.

use super::widgets::{
    Canvas, HitRect, Tone, account_provider_tag, account_short, blank_skin, draw_button,
    draw_input_box, draw_plate, draw_player_head, draw_skin_3d, draw_switch, format_size, palette,
    truncate_to_fit, wrap_lines,
};
use super::{ButtonId, MobileLauncher, SettingsTab};
use crate::launcher_backend::{
    Account, ImportKind, Language, LanguageDownload, Setup, SkinUpload,
    i18n::{self, tr, trf},
};
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
        let pad_v = if compact_v { 4.0 } else { 8.0 };
        let cy = y + pad_v;

        // The whole figure, head to feet: the chosen file until it is put on, else the selected
        // slot's skin, or the account's own (its face alone until that has loaded).
        let tile_h = (y + avail_h - cy - pad_v).max(80.0);
        // Room for the figure turned and tilted: about 34 pixels tall, 18 wide.
        let max_texel = (((w * 0.28) - 32.0) / 16.0).floor().max(2.0);
        let texel = ((tile_h - 20.0) / 34.0).floor().clamp(2.0, max_texel);
        let tile_w = 16.0 * texel + 32.0;

        let slots = account
            .as_ref()
            .map(|a| self.backend.skin_slots(a))
            .unwrap_or_default();
        let selected_idx = self.skin_slot.min(3);
        let selected_slot = slots[selected_idx].as_ref();
        let effective_slim = match (
            self.skin_pick.is_some() || self.skin_slim_edited,
            selected_slot,
        ) {
            (false, Some(slot)) => slot.slim,
            _ => self.skin_slim,
        };
        let skin = self
            .skin_pick
            .as_ref()
            .map(|pick| (pick.skin.clone(), effective_slim))
            .or_else(|| selected_slot.map(|slot| (slot.skin.clone(), effective_slim)))
            .or_else(|| {
                account
                    .as_ref()
                    .and_then(|a| self.backend.player_skin(a))
                    .map(|(s, slim)| {
                        (
                            s,
                            if self.skin_slim_edited {
                                self.skin_slim
                            } else {
                                slim
                            },
                        )
                    })
            });
        let name = account.as_ref().map_or(tr("Player"), account_short);
        let provider = account
            .as_ref()
            .map_or(tr("Not signed in"), account_provider_tag);
        let col_gap = if w < 760.0 { 18.0 } else { 24.0 };
        let tx = ix + tile_w + col_gap;
        let ctl_w = (w * 0.25).clamp(168.0, 210.0);
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
            let fit_name = truncate_to_fit(p, Face::SemiBold, 20.0, name, ctl_w);
            p.text_line(
                Face::SemiBold,
                20.0,
                26.0,
                tx,
                cy + 2.0,
                palette::text(),
                &fit_name,
            );
            p.text_line(
                Face::Regular,
                ui_theme::small(),
                18.0,
                tx,
                cy + 28.0,
                palette::muted(),
                provider,
            );
        }

        let head_h = if compact_v { 50.0 } else { 58.0 };
        let mut by = cy + head_h;
        let btn_gap = if compact_v { 8.0 } else { 10.0 };
        let max_btn_h = if compact_v { 38.0 } else { 42.0 };
        let btn_h = ((tile_h - head_h - 24.0 - 2.0 * btn_gap) / 3.0)
            .floor()
            .clamp(28.0, max_btn_h);
        let upload = self.backend.skin_upload();
        let mut status_room = (x + w - 22.0 - tx).max(0.0);
        let status = match account {
            Some(Account::Microsoft { .. } | Account::Offline { .. }) => {
                let gx = tx + ctl_w + col_gap;
                status_room = (gx - 8.0 - tx).max(ctl_w);

                draw_button(
                    ui,
                    hits,
                    ButtonId::PickSkin,
                    tx,
                    by,
                    ctl_w,
                    btn_h,
                    Tone::Secondary,
                    tr("Choose Skin File"),
                );
                by += btn_h + btn_gap;

                draw_switch(
                    ui,
                    hits,
                    [
                        Some(ButtonId::SkinArms(false)),
                        Some(ButtonId::SkinArms(true)),
                    ],
                    tx,
                    by,
                    ctl_w,
                    btn_h,
                    [tr("Classic"), tr("Slim")],
                    effective_slim,
                );
                by += btn_h + btn_gap;

                let slot_dirty = self.skin_pick.is_some()
                    || selected_slot.is_some_and(|s| s.slim != effective_slim);
                let dirty = slot_dirty || self.skin_retry || selected_slot.is_some_and(|s| !s.worn);
                let has_png = self.skin_pick.is_some() || selected_slot.is_some();
                let can_save = !self.skin_uploading && dirty && has_png;

                if can_save {
                    draw_button(
                        ui,
                        hits,
                        ButtonId::UseSkin,
                        tx,
                        by,
                        ctl_w,
                        btn_h,
                        Tone::Primary,
                        tr("Save"),
                    );
                } else if let Some(p) = ui.paint() {
                    p.rounded(tx, by, ctl_w, btn_h, 10.0, palette::surface());
                    p.border(tx, by, ctl_w, btn_h, 10.0, palette::border_subtle());
                    let label = if self.skin_uploading {
                        tr("Saving…")
                    } else {
                        tr("Save")
                    };
                    let tw = p.measure(Face::Medium, ui_theme::sizes().ui, label);
                    let lx = tx + ((ctl_w - tw) * 0.5).round();
                    p.text_line(
                        Face::Medium,
                        ui_theme::sizes().ui,
                        btn_h,
                        lx,
                        by,
                        palette::muted(),
                        label,
                    );
                }

                // Four saved-skin slots (2x2) on the right.
                let gw = (x + w - gx).max(120.0);
                let slot_gap = if compact_v { 8.0 } else { 12.0 };
                let slot_w = ((gw - slot_gap) * 0.5).floor().max(54.0);
                let slot_h = ((tile_h - slot_gap) * 0.5).floor().max(44.0);
                let slot_texel = ((slot_h - 24.0) / 34.0).floor().max(1.5);
                for (i, slot) in slots.iter().enumerate() {
                    let sx = gx + (i % 2) as f32 * (slot_w + slot_gap);
                    let sy = cy + (i / 2) as f32 * (slot_h + slot_gap);
                    let hit = HitRect {
                        id: ButtonId::SkinSlot(i),
                        x: sx,
                        y: sy,
                        w: slot_w,
                        h: slot_h,
                    };
                    if let Some(p) = ui.paint() {
                        let selected = i == selected_idx;
                        let hovered = hit.contains(p.cursor);
                        let (fill, border_col) = match (selected, hovered) {
                            (true, true) => (
                                color::with_alpha(palette::accent(), 0.26),
                                palette::accent_star(),
                            ),
                            (true, false) => (
                                color::with_alpha(palette::accent(), 0.18),
                                palette::accent_star(),
                            ),
                            (false, true) => (palette::surface_hover(), palette::accent()),
                            (false, false) => (palette::card(), palette::border()),
                        };
                        p.rounded(sx, sy, slot_w, slot_h, 12.0, fill);
                        p.border(sx, sy, slot_w, slot_h, 12.0, border_col);

                        let fcx = sx + (slot_w * 0.5).round();
                        let fcy = sy + (slot_h * 0.5 + 4.0).round();
                        if let Some(saved) = slot {
                            draw_skin_3d(
                                p,
                                fcx,
                                fcy,
                                slot_texel,
                                &saved.skin,
                                saved.slim,
                                0.45,
                                0.18,
                            );
                        } else {
                            draw_skin_3d(p, fcx, fcy, slot_texel, blank_skin(), false, 0.45, 0.18);
                        }

                        let num = ["1", "2", "3", "4"][i];
                        let num_ink = if selected {
                            palette::text()
                        } else {
                            palette::muted()
                        };
                        p.text_line(
                            Face::Medium,
                            ui_theme::small(),
                            18.0,
                            sx + 10.0,
                            sy + 5.0,
                            num_ink,
                            num,
                        );
                        // Unsaved changes show on Save (primary), not as a dot here.
                        if slot.as_ref().is_some_and(|s| s.worn) {
                            let tag = tr("Worn");
                            let tw = p.measure(Face::Medium, ui_theme::small(), tag);
                            p.text_line(
                                Face::Medium,
                                ui_theme::small(),
                                18.0,
                                sx + slot_w - tw - 10.0,
                                sy + 5.0,
                                col_success,
                                tag,
                            );
                        } else if slot.is_none() {
                            let plus = "+";
                            let pw = p.measure(Face::SemiBold, 16.0, plus);
                            p.text_line(
                                Face::SemiBold,
                                16.0,
                                18.0,
                                sx + slot_w - pw - 10.0,
                                sy + 5.0,
                                num_ink,
                                plus,
                            );
                        }
                    }
                    hits.push(hit);
                }

                if self.skin_uploading {
                    Some((tr("Putting it on…").to_owned(), palette::muted()))
                } else {
                    match upload {
                        SkinUpload::Failed(why) => Some((why, col_danger)),
                        SkinUpload::Done if !slot_dirty => {
                            Some((tr("Skin updated.").to_owned(), col_success))
                        }
                        _ => None,
                    }
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
                    tr("Change Skin on ely.by"),
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
            let fit = truncate_to_fit(p, Face::Regular, ui_theme::small(), &text, status_room);
            p.text_line(
                Face::Regular,
                ui_theme::small(),
                18.0,
                tx,
                by + btn_h + 6.0,
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
                tr("Back"),
            );
            if let Some(p) = ui.paint() {
                p.text_line(
                    Face::SemiBold,
                    if compact_v { 19.0 } else { 22.0 },
                    hdr_h,
                    inner_x + 112.0,
                    cy,
                    palette::text(),
                    tr("Launcher Settings"),
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

        // Signed-in Settings splits into Launcher, Game, and Files tabs so phone screens reach
        // every switch without scrolling through twenty rows.
        let active_tab = if onboarding || typing_args {
            SettingsTab::Launcher
        } else {
            let tab_h = if compact_v { 28.0 } else { 32.0 };
            let mut tx = inner_x;
            for (tab, label) in [
                (SettingsTab::Launcher, tr("Launcher")),
                (SettingsTab::Game, tr("Game")),
                (SettingsTab::Files, tr("Files")),
            ] {
                let tw = (ui.ro().measure(Face::Medium, ui_size, label) + 24.0).max(74.0);
                draw_button(
                    ui,
                    hits,
                    ButtonId::SelectSettingsTab(tab),
                    tx,
                    cy,
                    tw,
                    tab_h,
                    if self.settings_tab == tab {
                        Tone::Banner
                    } else {
                        Tone::Ghost
                    },
                    label,
                );
                tx += tw + 6.0;
            }
            cy += tab_h + 6.0;
            self.settings_tab
        };

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

        // Two choices side by side, `right` chosen when true: the one not chosen takes the tap.
        let switch = |ui: &mut Canvas<'_, '_>,
                      hits: &mut Vec<HitRect>,
                      cy: f32,
                      id: ButtonId,
                      labels: [&str; 2],
                      right: bool| {
            let ids = if right {
                [Some(id), None]
            } else {
                [None, Some(id)]
            };
            draw_switch(ui, hits, ids, col_x, ctl_y(cy), col_w, ctl_h, labels, right);
        };

        if active_tab == SettingsTab::Game {
            let profile = self.backend.selected_profile();
            let opts = self.backend.game_options(&profile);
            let items: [(u8, &str, &str, bool); 5] = [
                (
                    0,
                    tr("Dynamic Lights"),
                    tr("Held torches and glowing items light nearby blocks."),
                    opts.dynamic_lights,
                ),
                (
                    1,
                    tr("Coordinates HUD"),
                    tr("Shows your X, Y, Z position in the corner."),
                    opts.coords,
                ),
                (
                    2,
                    tr("Inventory Sort"),
                    tr("Adds a one-tap sort button to inventories and chests."),
                    opts.sort,
                ),
                (
                    3,
                    tr("Far Terrain (LOD)"),
                    tr("Renders distant terrain beyond your render distance."),
                    opts.far_terrain,
                ),
                (
                    4,
                    tr("Auto-Jump"),
                    tr("Automatically jumps over one-block ledges while walking."),
                    opts.auto_jump,
                ),
            ];
            let g_row_h: f32 = if compact_v { 44.0 } else { 48.0 };
            let total = items.len() + usize::from(self.game_options_error.is_some());
            let mut rows = Rows {
                skip: self.page_offset().min(total.saturating_sub(1)),
                seen: 0,
                below: 0,
                y: cy,
                end: card_end,
            };
            let text_w = (col_x - 12.0 - inner_x).max(80.0);
            for (idx, title, desc, on) in items {
                if let Some(ry) = rows.next(g_row_h, g_row_h) {
                    if let Some(p) = ui.paint() {
                        p.rounded(
                            inner_x,
                            ry + g_row_h - 1.0,
                            inner_w,
                            1.0,
                            0.0,
                            palette::border_subtle(),
                        );
                        let fit_t = truncate_to_fit(p, Face::Medium, ui_size, title, text_w);
                        p.text_line(
                            Face::Medium,
                            ui_size,
                            20.0,
                            inner_x,
                            ry + if compact_v { 3.0 } else { 5.0 },
                            palette::text(),
                            &fit_t,
                        );
                        let fit_d =
                            truncate_to_fit(p, Face::Regular, ui_theme::small(), desc, text_w);
                        p.text_line(
                            Face::Regular,
                            ui_theme::small(),
                            16.0,
                            inner_x,
                            ry + if compact_v { 23.0 } else { 26.0 },
                            palette::muted(),
                            &fit_d,
                        );
                    }
                    let sy = ry + ((g_row_h - ctl_h) * 0.5).round();
                    let id = ButtonId::ToggleGameOption(idx);
                    let ids = if !on { [Some(id), None] } else { [None, Some(id)] };
                    draw_switch(
                        ui,
                        hits,
                        ids,
                        col_x,
                        sy,
                        col_w,
                        ctl_h,
                        [tr("On"), tr("Off")],
                        !on,
                    );
                }
            }
            if let Some(err) = &self.game_options_error
                && let Some(ry) = rows.next(22.0, 20.0)
                && let Some(p) = ui.paint()
            {
                let fit = truncate_to_fit(p, Face::Regular, ui_theme::small(), err, inner_w);
                p.text_line(
                    Face::Regular,
                    ui_theme::small(),
                    20.0,
                    inner_x,
                    ry + 2.0,
                    col_danger,
                    &fit,
                );
            }
            self.scroll_max.set(rows.skip + rows.below);
            return;
        }

        if active_tab == SettingsTab::Files {
            let profile = self.backend.selected_profile();
            let worlds = self.backend.profile_worlds(&profile);
            let f_row_h: f32 = if compact_v { 40.0 } else { 44.0 };
            let status_shown = self.file_busy || self.file_status.is_some();
            let total = 5 + usize::from(status_shown);
            let mut rows = Rows {
                skip: self.page_offset().min(total.saturating_sub(1)),
                seen: 0,
                below: 0,
                y: cy,
                end: card_end,
            };
            for (title, btn_label, kind) in [
                (tr("Import World"), tr("Import .zip"), ImportKind::World),
                (
                    tr("Import Resource Pack"),
                    tr("Import .zip"),
                    ImportKind::ResourcePack,
                ),
                (
                    tr("Import Shader Pack"),
                    tr("Import .zip"),
                    ImportKind::ShaderPack,
                ),
                (
                    tr("Import Touch Layout"),
                    tr("Pojav / Amethyst"),
                    ImportKind::TouchLayout,
                ),
            ] {
                if let Some(ry) = rows.next(f_row_h, f_row_h) {
                    name(ui, ry, f_row_h, title);
                    if self.file_busy {
                        value(ui, ry, tr("Working…"), palette::muted());
                    } else {
                        buttons(ui, hits, ry, &[(ButtonId::ImportFile(kind), btn_label)]);
                    }
                }
            }
            if let Some(ry) = rows.next(f_row_h, f_row_h) {
                name(ui, ry, f_row_h, tr("Export World"));
                if self.file_busy {
                    value(ui, ry, tr("Exporting…"), palette::muted());
                } else if worlds.is_empty() {
                    value(ui, ry, tr("No worlds in this profile"), palette::muted());
                } else {
                    let selected = &worlds[self.export_world_idx % worlds.len()];
                    let btn_x = if worlds.len() > 1 {
                        buttons(
                            ui,
                            hits,
                            ry,
                            &[
                                (ButtonId::CycleExportWorld, tr("Next")),
                                (ButtonId::ExportWorld, tr("Export .zip")),
                            ],
                        )
                    } else {
                        buttons(ui, hits, ry, &[(ButtonId::ExportWorld, tr("Export .zip"))])
                    };
                    if let Some(p) = ui.paint() {
                        let left =
                            inner_x + p.measure(Face::Medium, ui_size, tr("Export World")) + 16.0;
                        let room = (btn_x - 12.0 - left).max(0.0);
                        let fit =
                            truncate_to_fit(p, Face::Regular, ui_theme::small(), selected, room);
                        let tw = p.measure(Face::Regular, ui_theme::small(), &fit);
                        p.text_line(
                            Face::Regular,
                            ui_theme::small(),
                            f_row_h,
                            btn_x - 12.0 - tw,
                            ry,
                            palette::body(),
                            &fit,
                        );
                    }
                }
            }
            if status_shown
                && let Some(ry) = rows.next(24.0, 22.0)
                && let Some(p) = ui.paint()
            {
                let (text, ink) = if self.file_busy {
                    (tr("Working…"), palette::accent_star())
                } else if let Some(Ok(msg)) = &self.file_status {
                    (msg.as_str(), col_success)
                } else if let Some(Err(err)) = &self.file_status {
                    (err.as_str(), col_danger)
                } else {
                    ("", palette::muted())
                };
                let fit = truncate_to_fit(p, Face::Regular, ui_theme::small(), text, inner_w);
                p.text_line(
                    Face::Regular,
                    ui_theme::small(),
                    22.0,
                    inner_x,
                    ry + 2.0,
                    ink,
                    &fit,
                );
            }
            self.scroll_max.set(rows.skip + rows.below);
            return;
        }

        let reclaim = self.backend.reclaimable().filter(|&bytes| bytes > 0);
        let offer = matches!(setup, Setup::Ready).then_some(reclaim).flatten();
        let full = !onboarding && !typing_args;
        let free_shown = full && (offer.is_some() || self.freed.is_some());
        let lang_status = self.backend.language_download();
        let lang_err_h = if matches!(lang_status, LanguageDownload::Failed(_)) {
            18.0
        } else {
            0.0
        };
        let error_h = if self.game_args_error.is_some() {
            18.0
        } else {
            0.0
        };
        // The rows under the title, as a drag scrolled them: rows scrolled past and rows past the
        // card's end aren't drawn, and how many are below sets how far a drag goes.
        let total = 3 * usize::from(!typing_args)
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
        if !typing_args && let Some(cy) = rows.next(row_h + lang_err_h, row_h + lang_err_h) {
            name(ui, cy, row_h + lang_err_h, tr("Language"));
            let chosen = self.backend.language();
            let pack_ready = self.pack_font_ready.get();
            let display_lang = |lang: Language| {
                if lang.bundled_font_supported() || pack_ready {
                    lang.name()
                } else {
                    lang.english_name()
                }
            };
            let label = match &lang_status {
                LanguageDownload::Downloading(lang) => format!("{}…", display_lang(*lang)),
                _ => display_lang(chosen).to_owned(),
            };
            buttons(
                ui,
                hits,
                cy,
                &[(ButtonId::ToggleLanguagePicker, label.as_str())],
            );
            if let LanguageDownload::Failed(err) = &lang_status
                && let Some(p) = ui.paint()
            {
                let fit = truncate_to_fit(p, Face::Regular, ui_theme::small(), err, inner_w);
                let tw = p.measure(Face::Regular, ui_theme::small(), &fit);
                let tx = inner_x + inner_w - tw;
                p.text_line(
                    Face::Regular,
                    ui_theme::small(),
                    lang_err_h,
                    tx,
                    cy + row_h - 4.0,
                    col_danger,
                    &fit,
                );
            }
        }
        if !typing_args && let Some(cy) = rows.next(row_h, row_h) {
            name(ui, cy, row_h, tr("Launcher Font"));
            let pixel = ui_theme::pixel_font();
            switch(
                ui,
                hits,
                cy,
                ButtonId::ToggleLauncherFont,
                [tr("Modern"), tr("Pixel")],
                pixel,
            );
        }
        // Moving pages, pop-ups and lists (`motion.rs`); off, everything changes at once.
        if !typing_args && let Some(cy) = rows.next(row_h, row_h) {
            name(ui, cy, row_h, tr("Animations"));
            let off = !self.motion.on.get();
            switch(
                ui,
                hits,
                cy,
                ButtonId::ToggleAnimations,
                [tr("On"), tr("Off")],
                off,
            );
        }

        // The game's arguments: words, "quoted words" as one; why they were refused, under them.
        if !onboarding && let Some(cy) = rows.next(row_h + error_h, row_h + error_h) {
            name(ui, cy, row_h + error_h, tr("Game Arguments"));
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
                if text.is_empty() { tr("None") } else { "" },
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
            name(ui, cy, row_h, tr("Free Up Space"));
            match (&self.freed, offer) {
                (Some(Err(err)), _) => value(ui, cy, err, col_danger),
                (_, Some(bytes)) => {
                    let label = trf("Free {size}", &[("size", &format_size(bytes))]);
                    buttons(ui, hits, cy, &[(ButtonId::FreeSpace, label.as_str())]);
                }
                (Some(Ok(bytes)), None) => {
                    let text = trf("Freed {size}", &[("size", &format_size(*bytes))]);
                    value(ui, cy, &text, col_success);
                }
                (None, None) => {}
            }
        }

        if full {
            if let Some(cy) = rows.next(row_h, row_h) {
                name(ui, cy, row_h, tr("Version"));
                let version = concat!(
                    "Meridian ",
                    env!("CARGO_PKG_VERSION"),
                    "  ·  Minecraft 26.3"
                );
                value(ui, cy, version, palette::body());
            }
            if let Some(cy) = rows.next(row_h, row_h) {
                name(ui, cy, row_h, tr("Open Source"));
                let links = [
                    (ButtonId::OpenSourceCode, tr("Source Code")),
                    (ButtonId::OpenLicenses, tr("Licenses")),
                ];
                buttons(ui, hits, cy, &links);
            }
            if let Some(cy) = rows.next(row_h, row_h) {
                name(ui, cy, row_h, tr("Quit Launcher"));
                buttons(ui, hits, cy, &[(ButtonId::Quit, tr("Quit"))]);
            }
            if let Some(cy) = rows.next(row_h, row_h) {
                name(ui, cy, row_h, tr("Diagnostics"));
                if self.diagnostics_exporting {
                    value(ui, cy, tr("Exporting…"), palette::muted());
                } else {
                    let btn_x =
                        buttons(ui, hits, cy, &[(ButtonId::ExportDiagnostics, tr("Export"))]);
                    if let Some(result) = &self.diagnostics
                        && let Some(p) = ui.paint()
                    {
                        let (text, ok) = diagnostics_status(result);
                        let ink = if ok { col_success } else { col_danger };
                        let left =
                            inner_x + p.measure(Face::Medium, ui_size, tr("Diagnostics")) + 16.0;
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
                let note = tr(
                    "Not an official Minecraft product. Not approved by or associated \
                            with Mojang or Microsoft.",
                );
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
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_free_space_confirm(
        &self,
        ui: &mut Canvas<'_, '_>,
        hits: &mut Vec<HitRect>,
        x: f32,
        y: f32,
        w: f32,
        avail_h: f32,
        reclaimable: u64,
    ) {
        let size = format_size(reclaimable);
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
                &trf("Free up {size}?", &[("size", &size)]),
            );
            cy += if compact_v { 30.0 } else { 40.0 };
            let body = tr(
                "Deletes the download cache and Java, which only installing uses. Your worlds and profiles stay. An update downloads them again.",
            );
            let lines = ((btn_y - 8.0 - cy) / 20.0).floor().max(0.0) as usize;
            for (i, line) in wrap_lines(p, Face::Regular, ui_theme::sizes().ui, body, inner_w)
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
            tr("Keep Files"),
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
            &trf("Free Up {size}", &[("size", &size)]),
        );
    }

    /// Language picker modal: the 87 launcher languages in `Language::ALL` order, each named in
    /// itself, with its download size (or `Installed` / `Active`) on the right.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_language_popup(
        &self,
        ui: &mut Canvas<'_, '_>,
        hits: &mut Vec<HitRect>,
        x: f32,
        y: f32,
        w: f32,
        avail_h: f32,
        col_success: u32,
    ) {
        let compact_v = avail_h < 300.0;
        let pad = if compact_v { 12.0 } else { 18.0 };
        let hdr_h = if compact_v { 30.0 } else { 36.0 };
        let row_h = if compact_v { 32.0 } else { 36.0 };
        let row_gap = 4.0;
        let step = row_h + row_gap;
        let max_card_h = (avail_h - 16.0).clamp(112.0, 356.0);
        let list_avail = (max_card_h - pad * 2.0 - hdr_h - 10.0).max(row_h);
        let max_rows = ((list_avail + row_gap) / step).floor().max(1.0) as usize;
        let shown_rows = max_rows.min(Language::ALL.len());
        let card_h = (pad * 2.0 + hdr_h + 10.0 + shown_rows as f32 * step - row_gap).min(avail_h);
        let cy_top = y + ((avail_h - card_h) * 0.5).max(0.0).round();
        draw_plate(ui, x, cy_top, w, card_h, palette::border());

        let inner_x = x + pad;
        let inner_w = (w - pad * 2.0).max(160.0);
        let hdr_y = cy_top + pad;
        if let Some(p) = ui.paint() {
            p.text_line(
                Face::SemiBold,
                if compact_v { 18.0 } else { 20.0 },
                hdr_h,
                inner_x,
                hdr_y,
                palette::text(),
                tr("Language"),
            );
        }

        let done_w = 78.0;
        let done_x = inner_x + inner_w - done_w;
        draw_button(
            ui,
            hits,
            ButtonId::CloseLanguagePicker,
            done_x,
            hdr_y,
            done_w,
            hdr_h,
            Tone::Secondary,
            tr("Done"),
        );
        if Language::ALL.len() > shown_rows {
            let nav_w = 60.0;
            let down_x = done_x - 6.0 - nav_w;
            let up_x = down_x - 6.0 - nav_w;
            draw_button(
                ui,
                hits,
                ButtonId::LanguageScrollUp,
                up_x,
                hdr_y,
                nav_w,
                hdr_h,
                Tone::Ghost,
                tr("Up"),
            );
            draw_button(
                ui,
                hits,
                ButtonId::LanguageScrollDown,
                down_x,
                hdr_y,
                nav_w,
                hdr_h,
                Tone::Ghost,
                tr("Down"),
            );
        }

        let max_scroll = Language::ALL.len().saturating_sub(shown_rows);
        self.scroll_max.set(max_scroll);
        let start = self.language_scroll.min(max_scroll);
        let chosen = self.backend.language();
        let dl_status = self.backend.language_download();
        let pack_ready = self.pack_font_ready.get();
        let mut ry = hdr_y + hdr_h + 10.0;

        for (idx, lang) in Language::ALL
            .into_iter()
            .enumerate()
            .skip(start)
            .take(shown_rows)
        {
            let supported = lang.bundled_font_supported() || pack_ready;
            let hit = HitRect {
                id: ButtonId::SelectLanguage(idx),
                x: inner_x,
                y: ry,
                w: inner_w,
                h: row_h,
            };
            let is_chosen = lang == chosen;
            if let Some(p) = ui.paint() {
                let hovered = supported && hit.contains(p.cursor);
                let fill = if is_chosen {
                    color::with_alpha(palette::accent(), 0.20)
                } else if hovered {
                    palette::surface_hover()
                } else {
                    palette::surface()
                };
                let bdr = if is_chosen {
                    palette::accent_star()
                } else if hovered {
                    palette::border()
                } else {
                    palette::border_subtle()
                };
                p.rounded(inner_x, ry, inner_w, row_h, 9.0, fill);
                p.border(inner_x, ry, inner_w, row_h, 9.0, bdr);

                // Supported languages (all 87 once the prepared pack's Unifont sheets are loaded)
                // show their native name; scripts Geist lacks show their plain Latin name until
                // the game is installed so no missing-glyph boxes are drawn.
                let display_name = if supported {
                    lang.name()
                } else {
                    lang.english_name()
                };
                let name_fit = truncate_to_fit(
                    p,
                    Face::Medium,
                    ui_theme::sizes().ui,
                    display_name,
                    inner_w * 0.62,
                );
                p.text_line(
                    Face::Medium,
                    ui_theme::sizes().ui,
                    row_h,
                    inner_x + 14.0,
                    ry,
                    if supported {
                        palette::text()
                    } else {
                        palette::muted()
                    },
                    &name_fit,
                );

                let (tag, tag_col) = if !supported {
                    (tr("Install game first").to_owned(), palette::muted())
                } else {
                    match &dl_status {
                        LanguageDownload::Downloading(dl) if *dl == lang => {
                            (tr("Downloading").to_owned(), palette::accent())
                        }
                        _ if is_chosen => (tr("Active").to_owned(), col_success),
                        _ if lang == Language::English || self.backend.language_cached(lang) => {
                            (tr("Installed").to_owned(), palette::muted())
                        }
                        _ => (format_size(i18n::download_size(lang)), palette::muted()),
                    }
                };
                let tw = p.measure(Face::Regular, ui_theme::small(), &tag);
                p.text_line(
                    Face::Regular,
                    ui_theme::small(),
                    row_h,
                    inner_x + inner_w - 14.0 - tw,
                    ry,
                    tag_col,
                    &tag,
                );
            }
            if supported {
                hits.push(hit);
            }
            ry += step;
        }
    }
}

/// Status text for Settings' Diagnostics row: `(text, ok)` where `ok` uses the success colour
/// and `false` uses the danger colour.
pub(super) fn diagnostics_status(result: &Result<String, String>) -> (String, bool) {
    match result {
        Ok(file) => (trf("Shared {file}", &[("file", file)]), true),
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
