//! Home (the selected profile and its game log), floating Profile Switcher popup, and the
//! Remove profile modal.

use super::widgets::{
    Canvas, HitRect, Tone, count_label, draw_button, draw_plate, effective_profile_state,
    log_levels, palette, profile_badge_parts, profile_stats_line, stage_fraction, truncate_to_fit,
    wrap_lines,
};
use super::{ButtonId, MobileLauncher};
use crate::launcher_backend::{Profile, ProfileState, Setup, VANILLA};
use crate::ui_theme::{self, Face, color};

impl MobileLauncher {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_home(
        &self,
        ui: &mut Canvas<'_, '_>,
        hits: &mut Vec<HitRect>,
        x: f32,
        y: f32,
        w: f32,
        avail_h: f32,
        setup: &Setup,
        profiles: &[Profile],
        col_success: u32,
        col_danger: u32,
    ) {
        let compact_v = avail_h < 320.0;
        let selected_id = self.backend.selected_profile();
        let selected = profiles
            .iter()
            .find(|p| p.id == selected_id)
            .or_else(|| profiles.first());
        let sel_name = selected.map_or("Vanilla", |p| p.name.as_str());
        let sel_state = selected.map_or(ProfileState::Available, |p| {
            effective_profile_state(p, setup)
        });

        // The selected profile, the whole page.
        let hero_h = avail_h;
        draw_plate(ui, x, y, w, hero_h, palette::border());
        let pad_x = if compact_v { 16.0 } else { 24.0 };
        let pad_y = if compact_v { 10.0 } else { 18.0 };
        let ix = x + pad_x;
        let iw = w - pad_x * 2.0;

        if let Some(p) = ui.paint() {
            // Top row: the profile's name, its status on the right.
            let (badge, bcol) = profile_badge_parts(&sel_state, true, col_success, col_danger);
            let bw = p.measure(Face::Medium, ui_theme::small(), &badge) + 16.0;
            let bx = ix + iw - bw;
            let badge_h = if compact_v { 20.0 } else { 22.0 };
            p.rounded(
                bx,
                y + pad_y,
                bw,
                badge_h,
                6.0,
                color::with_alpha(bcol, 0.16),
            );
            p.border(
                bx,
                y + pad_y,
                bw,
                badge_h,
                6.0,
                color::with_alpha(bcol, 0.75),
            );
            p.text_line(
                Face::Medium,
                ui_theme::small(),
                badge_h,
                bx + 8.0,
                y + pad_y,
                bcol,
                &badge,
            );

            // Profile title
            let title_y = y + pad_y;
            let title_sz = if compact_v { 18.5 } else { 24.0 };
            let title_h = if compact_v { 22.0 } else { 30.0 };
            let fit_name = truncate_to_fit(p, Face::SemiBold, title_sz, sel_name, bx - ix - 12.0);
            p.text_line(
                Face::SemiBold,
                title_sz,
                title_h,
                ix,
                title_y,
                palette::text(),
                &fit_name,
            );

            // Single-line stats right under profile name (on compact phones, status note takes priority).
            let stats = selected.map_or_else(|| "...".to_owned(), profile_stats_line);
            let (sub, sub_col) = if compact_v && let Some(note) = &self.status_note {
                (note.clone(), palette::accent_star())
            } else {
                (stats, palette::muted())
            };
            let sub_y = title_y + title_h + if compact_v { 0.0 } else { 4.0 };
            let fit_sub = truncate_to_fit(p, Face::Regular, ui_theme::small(), &sub, iw);
            p.text_line(
                Face::Regular,
                ui_theme::small(),
                18.0,
                ix,
                sub_y,
                sub_col,
                &fit_sub,
            );

            if let ProfileState::Installing(prog) = &sel_state {
                let frac = stage_fraction(prog);
                let bar_y = sub_y + if compact_v { 18.0 } else { 26.0 };
                let bar_h = if compact_v { 4.0 } else { 6.0 };
                p.rounded(ix, bar_y, iw, bar_h, bar_h * 0.5, palette::bg_outer());
                if frac > 0.0 {
                    p.rounded(
                        ix,
                        bar_y,
                        (iw * frac).max(8.0),
                        bar_h,
                        bar_h * 0.5,
                        palette::accent(),
                    );
                }
            } else if !compact_v && let Some(note) = &self.status_note {
                let fit = truncate_to_fit(p, Face::Regular, ui_theme::sizes().ui, note, iw);
                p.text_line(
                    Face::Regular,
                    ui_theme::sizes().ui,
                    20.0,
                    ix,
                    sub_y + 24.0,
                    palette::accent_star(),
                    &fit,
                );
            }
        }
        let title_h = if compact_v { 22.0 } else { 30.0 };
        let below_stats = y + pad_y + title_h + if compact_v { 20.0 } else { 26.0 };
        let log_y = below_stats
            + if matches!(sel_state, ProfileState::Installing(_)) {
                if compact_v { 10.0 } else { 18.0 }
            } else if !compact_v && self.status_note.is_some() {
                30.0
            } else {
                6.0
            };

        // The install's progress, while it runs.
        let btn_h: f32 = if compact_v { 30.0 } else { 36.0 };
        let btn_y = y + hero_h - btn_h - if compact_v { 8.0 } else { 16.0 };
        let installing = matches!(setup, Setup::Running(_));
        if installing {
            let prog_w = if compact_v { 116.0 } else { 132.0 };
            draw_button(
                ui,
                hits,
                ButtonId::ViewInstallScreen,
                ix,
                btn_y,
                prog_w,
                btn_h,
                Tone::Banner,
                "View progress",
            );
        }
        let log_end = if installing {
            btn_y - 10.0
        } else {
            y + hero_h - pad_y
        };
        let profile = selected.map_or(VANILLA, |p| p.id.as_str());
        self.draw_game_log(
            ui,
            hits,
            ix,
            log_y,
            iw,
            log_end - log_y,
            profile,
            col_danger,
        );
    }

    /// The profile's game log as the game last wrote it: its last lines that fit, from the level
    /// chosen up, and Copy.
    #[allow(clippy::too_many_arguments)]
    fn draw_game_log(
        &self,
        ui: &mut Canvas<'_, '_>,
        hits: &mut Vec<HitRect>,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        profile: &str,
        col_danger: u32,
    ) {
        let head_h = 30.0;
        if h < head_h + 48.0 {
            return;
        }
        let col_warn = color::hex(crate::menu_style::get().settings.warn, 255);
        let lines = self.backend.game_log(profile);
        let label_w = ui
            .ro()
            .measure(Face::SemiBold, ui_theme::small(), "Game log")
            + 16.0;
        if let Some(p) = ui.paint() {
            p.text_line(
                Face::SemiBold,
                ui_theme::small(),
                head_h,
                x,
                y,
                palette::text(),
                "Game log",
            );
        }
        let mut cx = x + label_w;
        for (level, name) in [(0, "All"), (1, "Warnings"), (2, "Errors")] {
            let cw = ui.ro().measure(Face::Medium, ui_theme::small(), name) + 22.0;
            draw_button(
                ui,
                hits,
                ButtonId::LogLevel(level),
                cx,
                y,
                cw,
                head_h,
                if self.log_level == level {
                    Tone::Banner
                } else {
                    Tone::Ghost
                },
                name,
            );
            cx += cw + 6.0;
        }
        if !lines.is_empty() {
            draw_button(
                ui,
                hits,
                ButtonId::CopyLog,
                x + w - 88.0,
                y,
                88.0,
                head_h,
                Tone::Secondary,
                if self.log_copied { "Copied" } else { "Copy" },
            );
        }
        let box_y = y + head_h + 8.0;
        let box_h = h - head_h - 8.0;
        let Some(p) = ui.paint() else {
            return;
        };
        p.rounded(x, box_y, w, box_h, 10.0, palette::bg_outer());
        p.border(x, box_y, w, box_h, 10.0, palette::border_subtle());
        let shown: Vec<(&String, u8)> = lines
            .iter()
            .zip(log_levels(&lines))
            .filter(|(_, level)| *level >= self.log_level)
            .collect();
        if shown.is_empty() {
            let empty = if lines.is_empty() {
                "The game's log shows here once you play."
            } else {
                "Nothing at this level."
            };
            p.text_line(
                Face::Regular,
                ui_theme::small(),
                20.0,
                x + 12.0,
                box_y + 10.0,
                palette::muted(),
                empty,
            );
            return;
        }
        let (size, line_h) = (11.0, 16.0);
        let rows = ((box_h - 16.0) / line_h).floor().max(0.0) as usize;
        for (i, (line, level)) in shown[shown.len().saturating_sub(rows)..].iter().enumerate() {
            let ink = match level {
                2 => col_danger,
                1 => col_warn,
                _ => palette::body(),
            };
            let fit = truncate_to_fit(p, Face::Mono, size, line, w - 24.0);
            p.text_line(
                Face::Mono,
                size,
                line_h,
                x + 12.0,
                box_y + 8.0 + i as f32 * line_h,
                ink,
                &fit,
            );
        }
    }

    /// Upward profile dropdown anchored directly above the bottom Profile Bar, matching its width,
    /// highlighting the selected profile row, and showing a slim right-edge scrollbar when long.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_profile_popup(
        &self,
        ui: &mut Canvas<'_, '_>,
        hits: &mut Vec<HitRect>,
        ui_w: f32,
        top_h: f32,
        dock_y: f32,
        bar_x: f32,
        bar_w: f32,
        setup: &Setup,
        profiles: &[Profile],
        col_success: u32,
        col_danger: u32,
    ) {
        let compact_v = dock_y < 380.0;
        let selected_id = self.backend.selected_profile();

        let popup_x = bar_x;
        let popup_w = bar_w;
        let pad_x: f32 = 4.0;
        let pad_y: f32 = 4.0;
        let row_h: f32 = if compact_v { 32.0 } else { 36.0 };
        let row_gap: f32 = 2.0;

        let max_h_cap: f32 = if compact_v { 256.0 } else { 316.0 };
        let max_popup_h = (dock_y - top_h - 12.0)
            .clamp(row_h + pad_y * 2.0, max_h_cap)
            .min((dock_y - 6.0).max(row_h + pad_y * 2.0));
        let avail_for_rows = (max_popup_h - pad_y * 2.0).max(row_h);
        let visible_cap = ((avail_for_rows + row_gap) / (row_h + row_gap))
            .floor()
            .max(1.0) as usize;
        let overflows = profiles.len() > visible_cap;
        let visible = profiles.len().min(visible_cap).max(1);
        let max_scroll = profiles.len().saturating_sub(visible);
        let scroll = self.profile_scroll.min(max_scroll);

        let rows_h = visible as f32 * row_h + visible.saturating_sub(1) as f32 * row_gap;
        let popup_h = (pad_y * 2.0 + rows_h).min(max_popup_h);
        let popup_y = (dock_y - 4.0 - popup_h).max(top_h + 4.0);

        // Dim scrim over the viewport above the bottom dock + 4 non-overlapping dismiss bands.
        if let Some(p) = ui.paint() {
            p.rounded(
                0.0,
                0.0,
                ui_w,
                dock_y,
                0.0,
                color::with_alpha(palette::bg_outer(), 0.58),
            );
            // Soft ambient drop glow behind the upward dropdown.
            p.rounded(
                popup_x - 2.0,
                popup_y - 2.0,
                popup_w + 4.0,
                popup_h + 4.0,
                12.0,
                color::with_alpha(palette::accent(), 0.14),
            );
        }
        if popup_y > 0.0 {
            hits.push(HitRect {
                id: ButtonId::CloseProfilePopup,
                x: 0.0,
                y: 0.0,
                w: ui_w,
                h: popup_y,
            });
        }
        let bottom_band_h = dock_y - (popup_y + popup_h);
        if bottom_band_h > 0.0 {
            hits.push(HitRect {
                id: ButtonId::CloseProfilePopup,
                x: 0.0,
                y: popup_y + popup_h,
                w: ui_w,
                h: bottom_band_h,
            });
        }
        if popup_x > 0.0 {
            hits.push(HitRect {
                id: ButtonId::CloseProfilePopup,
                x: 0.0,
                y: popup_y,
                w: popup_x,
                h: popup_h,
            });
        }
        let right_band_w = ui_w - (popup_x + popup_w);
        if right_band_w > 0.0 {
            hits.push(HitRect {
                id: ButtonId::CloseProfilePopup,
                x: popup_x + popup_w,
                y: popup_y,
                w: right_band_w,
                h: popup_h,
            });
        }

        // Solid opaque dropdown container so dimmed cards underneath never bleed through.
        if let Some(p) = ui.paint() {
            p.rounded(
                popup_x,
                popup_y,
                popup_w,
                popup_h,
                10.0,
                color::hex(0x0f081c, 255),
            );
            p.border(
                popup_x,
                popup_y,
                popup_w,
                popup_h,
                10.0,
                color::with_alpha(palette::accent_star(), 0.62),
            );
        }

        let sb_gutter: f32 = if overflows { 16.0 } else { 0.0 };
        let ix = popup_x + pad_x;
        let iw = (popup_w - pad_x * 2.0 - sb_gutter).max(80.0);

        let mut ly = popup_y + pad_y;
        for (idx, profile) in profiles.iter().enumerate().skip(scroll).take(visible) {
            let is_selected = profile.id == selected_id;
            let state = effective_profile_state(profile, setup);
            let row_hit = HitRect {
                id: ButtonId::SelectProfile(idx),
                x: ix,
                y: ly,
                w: iw,
                h: row_h,
            };
            if let Some(p) = ui.paint() {
                let hovered = row_hit.contains(p.cursor);
                if is_selected {
                    p.rounded(ix, ly, iw, row_h, 6.0, palette::accent_fill());
                } else if hovered {
                    p.rounded(ix, ly, iw, row_h, 6.0, palette::surface_hover());
                }

                let dot_col = if is_selected {
                    palette::text()
                } else {
                    match &state {
                        ProfileState::Installed => col_success,
                        ProfileState::Installing(_) | ProfileState::Available => {
                            palette::accent_star()
                        }
                        ProfileState::Failed(_) => col_danger,
                    }
                };
                p.rounded(
                    ix + 10.0,
                    ly + ((row_h - 6.0) * 0.5).round(),
                    6.0,
                    6.0,
                    3.0,
                    dot_col,
                );

                let right_meta = match &state {
                    ProfileState::Installing(prog) => format!(
                        "Installing {}%",
                        (stage_fraction(prog) * 100.0).round() as u32
                    ),
                    ProfileState::Failed(_) => "Failed".to_owned(),
                    ProfileState::Available => "Needs install".to_owned(),
                    ProfileState::Installed => profile_stats_line(profile),
                };
                let meta_col = if is_selected {
                    color::hex(0xebe4ff, 255)
                } else {
                    match &state {
                        ProfileState::Failed(_) => col_danger,
                        ProfileState::Installing(_) | ProfileState::Available => {
                            palette::accent_star()
                        }
                        ProfileState::Installed => palette::muted(),
                    }
                };

                let tx = ix + 24.0;
                let max_row_w = (iw - 34.0).max(60.0);
                let name_face = if is_selected {
                    Face::SemiBold
                } else {
                    Face::Medium
                };
                let name_col = if is_selected || hovered {
                    palette::text()
                } else {
                    palette::body()
                };
                let fit_meta = truncate_to_fit(
                    p,
                    Face::Mono,
                    ui_theme::small(),
                    &right_meta,
                    max_row_w * 0.56,
                );
                let mw = p.measure(Face::Mono, ui_theme::small(), &fit_meta);
                let name_room = (max_row_w - mw - 12.0).max(52.0);
                let fit_name = truncate_to_fit(p, name_face, 13.5, &profile.name, name_room);

                p.text_line(name_face, 13.5, row_h, tx, ly, name_col, &fit_name);
                p.text_line(
                    Face::Mono,
                    ui_theme::small(),
                    row_h,
                    ix + iw - 10.0 - mw,
                    ly,
                    meta_col,
                    &fit_meta,
                );

                if let ProfileState::Installing(prog) = &state {
                    let frac = stage_fraction(prog);
                    let bar_y = ly + row_h - 3.0;
                    p.rounded(tx, bar_y, iw - 34.0, 2.5, 1.2, palette::bg_outer());
                    if frac > 0.0 {
                        p.rounded(
                            tx,
                            bar_y,
                            ((iw - 34.0) * frac).max(8.0),
                            2.5,
                            1.2,
                            if is_selected {
                                palette::text()
                            } else {
                                palette::accent()
                            },
                        );
                    }
                }
            }
            hits.push(row_hit);
            ly += row_h + row_gap;
        }

        // Right-edge slim scrollbar track + rounded thumb + tappable up/down regions when long.
        if overflows {
            let gutter_x = popup_x + popup_w - sb_gutter;
            let track_y = popup_y + pad_y;
            let track_h = rows_h.max(24.0);
            let half_h = (track_h * 0.5).floor();
            let up_hit = HitRect {
                id: ButtonId::ProfileScrollUp,
                x: gutter_x,
                y: track_y,
                w: sb_gutter - 2.0,
                h: half_h,
            };
            let down_hit = HitRect {
                id: ButtonId::ProfileScrollDown,
                x: gutter_x,
                y: track_y + half_h,
                w: sb_gutter - 2.0,
                h: track_h - half_h,
            };
            if let Some(p) = ui.paint() {
                let bar_w: f32 = 4.0;
                let bar_x = gutter_x + ((sb_gutter - 2.0 - bar_w) * 0.5).round();
                p.rounded(
                    bar_x,
                    track_y + 2.0,
                    bar_w,
                    (track_h - 4.0).max(16.0),
                    2.0,
                    color::with_alpha(palette::border(), 0.48),
                );
                let usable_h = (track_h - 4.0).max(16.0);
                let thumb_h =
                    (visible as f32 / profiles.len() as f32 * usable_h).clamp(20.0, usable_h);
                let frac = if max_scroll == 0 {
                    0.0
                } else {
                    scroll as f32 / max_scroll as f32
                };
                let thumb_y = track_y + 2.0 + (frac * (usable_h - thumb_h)).round();
                let hovered = up_hit.contains(p.cursor) || down_hit.contains(p.cursor);
                p.rounded(
                    bar_x,
                    thumb_y,
                    bar_w,
                    thumb_h,
                    2.0,
                    if hovered {
                        palette::accent_star()
                    } else {
                        color::with_alpha(palette::muted(), 0.88)
                    },
                );
            }
            if scroll > 0 {
                hits.push(up_hit);
            }
            if scroll < max_scroll {
                hits.push(down_hit);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_remove_confirm(
        &self,
        ui: &mut Canvas<'_, '_>,
        hits: &mut Vec<HitRect>,
        x: f32,
        y: f32,
        w: f32,
        avail_h: f32,
        name: &str,
        worlds: usize,
        col_danger: u32,
    ) {
        let compact_v = avail_h < 280.0;
        let card_h = 286.0_f32.min(avail_h);
        let cy_top = y + ((avail_h - card_h) * 0.5).max(0.0).round();
        draw_plate(ui, x, cy_top, w, card_h, col_danger);

        let inner_x = x + 24.0;
        let inner_w = w - 48.0;
        let mut cy = cy_top + if compact_v { 12.0 } else { 22.0 };
        let world_phrase = count_label(worlds, "saved world", "saved worlds");

        if let Some(p) = ui.paint() {
            p.text_line(
                Face::SemiBold,
                if compact_v {
                    19.0
                } else {
                    ui_theme::sizes().title
                },
                if compact_v { 24.0 } else { 32.0 },
                inner_x,
                cy,
                palette::text(),
                &format!("Delete \"{name}\"?"),
            );
            cy += if compact_v { 28.0 } else { 38.0 };
            let body =
                format!("Its mods and {world_phrase} are deleted for good. Vanilla worlds stay.");
            for (i, line) in wrap_lines(p, Face::Regular, ui_theme::sizes().ui, &body, inner_w)
                .into_iter()
                .take(4)
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

        let btn_h = if compact_v { 42.0 } else { 48.0 };
        let btn_y = cy_top + card_h - btn_h - if compact_v { 10.0 } else { 16.0 };
        let half_w = (inner_w - 12.0) * 0.5;
        draw_button(
            ui,
            hits,
            ButtonId::CancelRemoveProfile,
            inner_x,
            btn_y,
            half_w,
            btn_h,
            Tone::Secondary,
            "Keep Profile",
        );
        draw_button(
            ui,
            hits,
            ButtonId::ConfirmRemoveProfile,
            inner_x + half_w + 12.0,
            btn_y,
            half_w,
            btn_h,
            Tone::Danger,
            "Delete Profile & Worlds",
        );
    }
}
