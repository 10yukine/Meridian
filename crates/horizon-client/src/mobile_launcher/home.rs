//! Home dashboard (`[profile]` + `[news][upd]`), floating Profile Switcher popup,
//! New Profile form, and Rename/Remove profile modals.

use super::widgets::{
    Canvas, HitRect, Tone, count_label, draw_button, draw_input_box, draw_plate,
    effective_profile_state, format_size, palette, profile_badge_parts, profile_stats_line,
    stage_fraction, stage_name, truncate_to_fit, wrap_lines,
};
use super::{ButtonId, MobileLauncher};
use crate::launcher_backend::{ModSearch, Profile, ProfileState, Setup, VANILLA, profile_id};
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
        let sel_is_custom = selected.is_some_and(|p| p.id != VANILLA);
        let sel_name = selected.map_or("Vanilla", |p| p.name.as_str());
        let sel_state = selected.map_or(ProfileState::Available, |p| {
            effective_profile_state(p, setup)
        });

        let card_gap = if compact_v { 10.0 } else { 14.0 };
        let hero_h = if compact_v {
            (avail_h * 0.47).round().clamp(108.0, 134.0).min(avail_h)
        } else {
            (avail_h * 0.43).round().clamp(186.0, 236.0).min(avail_h)
        };

        // 1. Top Hero Card: [profile] — Last Selected Profile
        draw_plate(
            ui,
            x,
            y,
            w,
            hero_h,
            color::with_alpha(palette::accent_star(), 0.48),
        );
        let pad_x = if compact_v { 16.0 } else { 24.0 };
        let pad_y = if compact_v { 10.0 } else { 18.0 };
        let ix = x + pad_x;
        let iw = w - pad_x * 2.0;

        if let Some(p) = ui.paint() {
            // Subtle vertical North Star accent strip on the left edge
            p.rounded(
                x + 4.0,
                y + 14.0,
                3.0,
                (hero_h - 28.0).max(24.0),
                1.5,
                palette::accent_star(),
            );

            // Top row: Eyebrow + right status badge
            let (badge, bcol) = profile_badge_parts(&sel_state, true, col_success, col_danger);
            let bw = p.measure(Face::Mono, ui_theme::small(), &badge) + 16.0;
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
                Face::Mono,
                ui_theme::small(),
                badge_h,
                bx + 8.0,
                y + pad_y,
                bcol,
                &badge,
            );

            let eyebrow_max = (bx - ix - 10.0).max(80.0);
            let eyebrow = truncate_to_fit(
                p,
                Face::Mono,
                11.0,
                "SELECTED PROFILE  |  MINECRAFT 26.3",
                eyebrow_max,
            );
            p.text_line(
                Face::Mono,
                11.0,
                badge_h,
                ix,
                y + pad_y,
                palette::accent_star(),
                &eyebrow,
            );

            // Profile title
            let title_y = y + pad_y + badge_h + if compact_v { 2.0 } else { 8.0 };
            let title_sz = if compact_v { 18.5 } else { 24.0 };
            let title_h = if compact_v { 22.0 } else { 30.0 };
            let fit_name = truncate_to_fit(p, Face::SemiBold, title_sz, sel_name, iw);
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
            let stats = selected.map_or_else(
                || "0 mods  |  0 worlds  |  Base game".to_owned(),
                profile_stats_line,
            );
            let (sub, sub_col) = if compact_v && let Some(note) = &self.status_note {
                (note.clone(), palette::accent_star())
            } else {
                (stats, palette::muted())
            };
            let sub_y = title_y + title_h + if compact_v { 0.0 } else { 4.0 };
            let fit_sub = truncate_to_fit(p, Face::Mono, ui_theme::small(), &sub, iw);
            p.text_line(
                Face::Mono,
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
            } else if !compact_v {
                let (detail, detail_col) = if let Some(note) = &self.status_note {
                    (note.as_str(), palette::accent_star())
                } else {
                    let desc = selected
                        .map(|prof| prof.description.as_str())
                        .filter(|d| !d.is_empty())
                        .unwrap_or("Isolated worlds and mods for this profile.");
                    (desc, palette::body())
                };
                let fit_desc = truncate_to_fit(p, Face::Regular, ui_theme::sizes().ui, detail, iw);
                p.text_line(
                    Face::Regular,
                    ui_theme::sizes().ui,
                    20.0,
                    ix,
                    sub_y + 24.0,
                    detail_col,
                    &fit_desc,
                );
            }
        }

        // Bottom action strip inside the selected profile hero card
        let btn_h: f32 = if compact_v { 30.0 } else { 36.0 };
        let btn_y = y + hero_h - btn_h - if compact_v { 8.0 } else { 16.0 };
        let mut bx_left = ix;
        let mods_w = if compact_v { 76.0 } else { 88.0 };
        draw_button(
            ui,
            hits,
            ButtonId::OpenModsForSelected,
            bx_left,
            btn_y,
            mods_w,
            btn_h,
            Tone::Secondary,
            "Mods",
        );
        bx_left += mods_w + 8.0;

        if sel_is_custom {
            let ren_w = if compact_v { 78.0 } else { 88.0 };
            draw_button(
                ui,
                hits,
                ButtonId::OpenRenameSelected,
                bx_left,
                btn_y,
                ren_w,
                btn_h,
                Tone::Secondary,
                "Rename",
            );
            bx_left += ren_w + 8.0;

            let del_w = if compact_v { 72.0 } else { 82.0 };
            draw_button(
                ui,
                hits,
                ButtonId::AskRemoveSelected,
                bx_left,
                btn_y,
                del_w,
                btn_h,
                Tone::Danger,
                "Delete",
            );
            bx_left += del_w + 8.0;
        }

        if matches!(setup, Setup::Running(_)) {
            let prog_w = if compact_v { 116.0 } else { 132.0 };
            draw_button(
                ui,
                hits,
                ButtonId::ViewInstallScreen,
                bx_left,
                btn_y,
                prog_w,
                btn_h,
                Tone::Banner,
                "View progress",
            );
        }

        let new_w = if compact_v { 124.0 } else { 142.0 };
        draw_button(
            ui,
            hits,
            ButtonId::OpenNewProfile,
            ix + iw - new_w,
            btn_y,
            new_w,
            btn_h,
            Tone::Secondary,
            "+ New profile",
        );

        // 2. Bottom Split Row: [news] (left) + [upd] (right)
        let row_y = y + hero_h + card_gap;
        let row_h = (y + avail_h - row_y).max(0.0);
        if row_h < 64.0 {
            return;
        }
        let left_w = ((w - card_gap) * 0.5).floor();
        let right_x = x + left_w + card_gap;
        let right_w = w - left_w - card_gap;

        self.draw_home_news_card(ui, x, row_y, left_w, row_h, compact_v);
        self.draw_home_updates_card(
            ui,
            right_x,
            row_y,
            right_w,
            row_h,
            compact_v,
            setup,
            profiles,
            col_success,
            col_danger,
        );
    }

    fn draw_home_news_card(
        &self,
        ui: &mut Canvas<'_, '_>,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        compact_v: bool,
    ) {
        draw_plate(ui, x, y, w, h, palette::border());
        let Some(p) = ui.paint() else {
            return;
        };
        let px = if compact_v { 14.0 } else { 20.0 };
        let py = if compact_v { 10.0 } else { 18.0 };
        let ix = x + px;
        let iw = (w - px * 2.0).max(80.0);

        let pill = "SOON";
        let pw = p.measure(Face::Mono, 11.0, pill) + 14.0;
        let ph = 20.0;
        let pill_x = ix + iw - pw;
        p.rounded(
            pill_x,
            y + py + 1.0,
            pw,
            ph,
            6.0,
            color::with_alpha(palette::accent(), 0.16),
        );
        p.border(
            pill_x,
            y + py + 1.0,
            pw,
            ph,
            6.0,
            color::with_alpha(palette::accent_star(), 0.65),
        );
        p.text_line(
            Face::Mono,
            11.0,
            ph,
            pill_x + 7.0,
            y + py + 1.0,
            palette::accent_star(),
            pill,
        );

        let title_w = (pill_x - ix - 8.0).max(60.0);
        let title_sz = if compact_v { 15.5 } else { 18.0 };
        let fit_title = truncate_to_fit(p, Face::SemiBold, title_sz, "GitHub News", title_w);
        p.text_line(
            Face::SemiBold,
            title_sz,
            22.0,
            ix,
            y + py,
            palette::text(),
            &fit_title,
        );

        let mut cy = y + py + if compact_v { 24.0 } else { 32.0 };
        p.rounded(x + 1.0, cy, w - 2.0, 1.0, 0.0, palette::border_subtle());
        cy += if compact_v { 8.0 } else { 14.0 };

        let headline = truncate_to_fit(
            p,
            Face::Medium,
            ui_theme::sizes().ui,
            "Meridian releases & dev notes",
            iw,
        );
        p.text_line(
            Face::Medium,
            ui_theme::sizes().ui,
            18.0,
            ix,
            cy,
            palette::body(),
            &headline,
        );
        cy += if compact_v { 19.0 } else { 24.0 };

        let foot_h = if compact_v { 24.0 } else { 30.0 };
        let foot_pad = if compact_v { 10.0 } else { 14.0 };
        let foot_y = y + h - foot_h - foot_pad;
        let show_footer = foot_y >= cy + 20.0;
        let text_bottom = if show_footer {
            foot_y - 6.0
        } else {
            y + h - foot_pad
        };

        let blurb = "Release notes, launcher updates, and GitHub highlights will show here once the feed is enabled.";
        let line_h = if compact_v { 16.0 } else { 19.0 };
        let max_lines = (((text_bottom - cy) / line_h).floor().max(0.0) as usize).min(4);
        for (i, line) in wrap_lines(p, Face::Regular, ui_theme::small(), blurb, iw)
            .into_iter()
            .take(max_lines)
            .enumerate()
        {
            p.text_line(
                Face::Regular,
                ui_theme::small(),
                line_h,
                ix,
                cy + i as f32 * line_h,
                palette::muted(),
                &line,
            );
        }

        if show_footer {
            p.rounded(ix, foot_y, iw, foot_h, 7.0, palette::surface());
            let repo = truncate_to_fit(
                p,
                Face::Mono,
                11.0,
                "github.com/10yukine/Meridian",
                iw - 20.0,
            );
            p.text_line(
                Face::Mono,
                11.0,
                foot_h,
                ix + 10.0,
                foot_y,
                palette::muted(),
                &repo,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_home_updates_card(
        &self,
        ui: &mut Canvas<'_, '_>,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        compact_v: bool,
        setup: &Setup,
        profiles: &[Profile],
        col_success: u32,
        col_danger: u32,
    ) {
        draw_plate(ui, x, y, w, h, palette::border());
        let Some(p) = ui.paint() else {
            return;
        };
        let px = if compact_v { 14.0 } else { 20.0 };
        let py = if compact_v { 10.0 } else { 18.0 };
        let ix = x + px;
        let iw = (w - px * 2.0).max(80.0);

        let ver = concat!("v", env!("CARGO_PKG_VERSION"));
        let ver_col = match setup {
            Setup::Ready => col_success,
            Setup::Failed { .. } => col_danger,
            Setup::Needed | Setup::Running(_) => palette::accent_star(),
        };
        let pw = p.measure(Face::Mono, 11.0, ver) + 14.0;
        let ph = 20.0;
        let pill_x = ix + iw - pw;
        p.rounded(
            pill_x,
            y + py + 1.0,
            pw,
            ph,
            6.0,
            color::with_alpha(ver_col, 0.16),
        );
        p.border(
            pill_x,
            y + py + 1.0,
            pw,
            ph,
            6.0,
            color::with_alpha(ver_col, 0.70),
        );
        p.text_line(
            Face::Mono,
            11.0,
            ph,
            pill_x + 7.0,
            y + py + 1.0,
            ver_col,
            ver,
        );

        let title_w = (pill_x - ix - 8.0).max(60.0);
        let title_sz = if compact_v { 15.5 } else { 18.0 };
        let fit_title = truncate_to_fit(p, Face::SemiBold, title_sz, "Horizon Updates", title_w);
        p.text_line(
            Face::SemiBold,
            title_sz,
            22.0,
            ix,
            y + py,
            palette::text(),
            &fit_title,
        );

        let mut cy = y + py + if compact_v { 24.0 } else { 32.0 };
        p.rounded(x + 1.0, cy, w - 2.0, 1.0, 0.0, palette::border_subtle());
        cy += if compact_v { 8.0 } else { 14.0 };

        let (runtime_text, runtime_col) = match setup {
            Setup::Needed => ("Not installed".to_owned(), palette::muted()),
            Setup::Running(prog) => (
                format!(
                    "{} {:.0}%",
                    stage_name(prog.stage),
                    stage_fraction(prog) * 100.0
                ),
                palette::accent_star(),
            ),
            Setup::Failed { stage, .. } => (format!("Failed ({})", stage_name(*stage)), col_danger),
            Setup::Ready => ("Installed & ready".to_owned(), col_success),
        };
        let total_bytes: u64 = profiles.iter().map(|prof| prof.size).sum();
        let storage_text = format!(
            "{}  |  {}",
            count_label(profiles.len(), "profile", "profiles"),
            format_size(total_bytes)
        );

        let rows: Vec<(&str, String, u32)> = vec![
            ("Target", "Java Edition 26.3".to_owned(), palette::body()),
            ("Runtime", runtime_text, runtime_col),
            ("Storage", storage_text, palette::body()),
        ];

        let row_h: f32 = if compact_v { 26.0 } else { 36.0 };
        let row_step: f32 = if compact_v { 31.0 } else { 44.0 };
        for (label, value, vcol) in rows {
            if cy + row_h > y + h - 8.0 {
                break;
            }
            p.rounded(ix, cy, iw, row_h, 8.0, palette::surface());
            p.text_line(
                Face::Medium,
                ui_theme::small(),
                row_h,
                ix + 12.0,
                cy,
                palette::muted(),
                label,
            );
            let fit_v = truncate_to_fit(p, Face::Mono, ui_theme::small(), &value, iw * 0.62);
            let vw = p.measure(Face::Mono, ui_theme::small(), &fit_v);
            p.text_line(
                Face::Mono,
                ui_theme::small(),
                row_h,
                ix + iw - 12.0 - vw,
                cy,
                vcol,
                &fit_v,
            );
            cy += row_step;
        }

        if !compact_v && cy + 20.0 <= y + h - 10.0 {
            let note = truncate_to_fit(
                p,
                Face::Regular,
                ui_theme::small(),
                "Tracking upstream Horizon engine & Minecraft 26.3.",
                iw,
            );
            p.text_line(
                Face::Regular,
                ui_theme::small(),
                18.0,
                ix,
                cy + 4.0,
                palette::muted(),
                &note,
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
    pub(super) fn draw_new_profile(
        &self,
        ui: &mut Canvas<'_, '_>,
        hits: &mut Vec<HitRect>,
        x: f32,
        y: f32,
        w: f32,
        avail_h: f32,
        profiles: &[Profile],
        col_danger: u32,
    ) {
        let ultra_v = avail_h < 150.0;
        let compact_v = avail_h < 350.0;
        let create_in_header = avail_h < 245.0;
        draw_plate(ui, x, y, w, avail_h, palette::border());

        let ix = x + 18.0;
        let iw = w - 36.0;
        let pad_y: f32 = if ultra_v {
            6.0
        } else if compact_v {
            8.0
        } else {
            16.0
        };
        let mut cy = y + pad_y;
        let hdr_h: f32 = if ultra_v {
            30.0
        } else if compact_v {
            32.0
        } else {
            38.0
        };

        draw_button(
            ui,
            hits,
            ButtonId::CancelNewProfile,
            ix,
            cy,
            92.0,
            hdr_h,
            Tone::Secondary,
            "<- Cancel",
        );
        if create_in_header {
            let create_w = 148.0_f32.min(iw * 0.40);
            draw_button(
                ui,
                hits,
                ButtonId::CreateProfile,
                ix + iw - create_w,
                cy,
                create_w,
                hdr_h,
                Tone::Primary,
                "Create Profile",
            );
        }
        if let Some(p) = ui.paint() {
            p.text_line(
                Face::SemiBold,
                if compact_v { 18.0 } else { 22.0 },
                hdr_h,
                ix + 106.0,
                cy,
                palette::text(),
                "New Profile",
            );
        }
        cy += hdr_h + if compact_v { 6.0 } else { 14.0 };

        let two_col = w >= 580.0;
        let col_gap = 16.0;
        let left_w = if two_col {
            ((iw - col_gap) * 0.5).round()
        } else {
            iw
        };
        let right_x = if two_col { ix + left_w + col_gap } else { ix };
        let right_w = if two_col { iw - left_w - col_gap } else { iw };

        let mut ly = cy;
        let input_h: f32 = if ultra_v {
            32.0
        } else if compact_v {
            36.0
        } else {
            44.0
        };
        draw_input_box(
            ui,
            hits,
            ButtonId::FocusNewProfileName,
            ix,
            ly,
            left_w,
            input_h,
            "Profile Name",
            &self.new_profile_name,
            false,
            true,
        );
        ly += input_h + if compact_v { 4.0 } else { 6.0 };

        let taken: Vec<String> = profiles.iter().map(|p| p.id.clone()).collect();
        let preview_slug = if self.new_profile_name.trim().is_empty() {
            "profile".to_owned()
        } else {
            profile_id(&self.new_profile_name, &taken)
        };
        // On tight keyboard-lifted layouts, swap the folder preview line for the validation error
        // when one is active so the preset chips and confirm button never lose space.
        let inline_err_on_slug = create_in_header && self.new_profile_error.is_some();
        if let Some(p) = ui.paint() {
            let (meta_text, meta_col) = if let Some(err) = &self.new_profile_error
                && inline_err_on_slug
            {
                (err.clone(), col_danger)
            } else {
                (format!("Folder: profiles/{preview_slug}"), palette::muted())
            };
            let fit_meta = truncate_to_fit(p, Face::Mono, ui_theme::small(), &meta_text, left_w);
            p.text_line(
                Face::Mono,
                ui_theme::small(),
                16.0,
                ix,
                ly,
                meta_col,
                &fit_meta,
            );
        }
        ly += if compact_v { 18.0 } else { 22.0 };

        let bottom_limit = y + avail_h - pad_y;
        let show_preset_label = !create_in_header || (bottom_limit - ly) >= 56.0;
        if show_preset_label {
            if let Some(p) = ui.paint() {
                p.text_line(
                    Face::Medium,
                    ui_theme::small(),
                    16.0,
                    ix,
                    ly,
                    palette::body(),
                    "Start from preset (optional)",
                );
            }
            ly += if compact_v { 18.0 } else { 20.0 };
        }

        let presets = self.backend.presets();
        let chip_h: f32 = if compact_v { 34.0 } else { 40.0 };
        let chip_cols = (presets.len() + 1).max(1);
        let chip_w = (left_w - (chip_cols as f32 - 1.0) * 6.0) / chip_cols as f32;
        if ly + chip_h <= bottom_limit {
            draw_button(
                ui,
                hits,
                ButtonId::SelectPresetNone,
                ix,
                ly,
                chip_w,
                chip_h,
                if self.new_profile_preset.is_none() {
                    Tone::Banner
                } else {
                    Tone::Secondary
                },
                "Empty",
            );
            for (i, preset) in presets.iter().enumerate() {
                let cx = ix + (i + 1) as f32 * (chip_w + 6.0);
                draw_button(
                    ui,
                    hits,
                    ButtonId::SelectPreset(i),
                    cx,
                    ly,
                    chip_w,
                    chip_h,
                    if self.new_profile_preset.as_deref() == Some(&preset.id) {
                        Tone::Banner
                    } else {
                        Tone::Secondary
                    },
                    &preset.name,
                );
            }
            ly += chip_h + 8.0;
        }

        if !inline_err_on_slug && let Some(err) = &self.new_profile_error {
            if let Some(p) = ui.paint() {
                let fit = truncate_to_fit(p, Face::Regular, ui_theme::small(), err, left_w);
                p.text_line(
                    Face::Regular,
                    ui_theme::small(),
                    16.0,
                    ix,
                    ly,
                    col_danger,
                    &fit,
                );
            }
            ly += 18.0;
        }

        if !create_in_header {
            let max_create_h = (bottom_limit - ly).max(34.0);
            let create_h = (if compact_v { 40.0_f32 } else { 48.0 }).min(max_create_h);
            let create_y = ly.min((bottom_limit - create_h).max(ly));
            draw_button(
                ui,
                hits,
                ButtonId::CreateProfile,
                ix,
                create_y,
                left_w,
                create_h,
                Tone::Primary,
                "Create Profile",
            );
        }

        if !two_col {
            return;
        }
        let mut ry = cy;
        if let Some(p) = ui.paint() {
            p.text_line(
                Face::Medium,
                ui_theme::small(),
                18.0,
                right_x,
                ry,
                palette::body(),
                &format!("Extra Mods ({} selected)", self.new_profile_mods.len()),
            );
        }
        ry += 20.0;

        match self.backend.mod_search() {
            ModSearch::Idle | ModSearch::Searching => {
                if let Some(p) = ui.paint() {
                    p.text_line(
                        Face::Regular,
                        ui_theme::small(),
                        22.0,
                        right_x,
                        ry,
                        palette::muted(),
                        "Loading Horizon registry...",
                    );
                }
            }
            ModSearch::Failed(err) => {
                if let Some(p) = ui.paint() {
                    let fit = truncate_to_fit(p, Face::Regular, ui_theme::small(), &err, right_w);
                    p.text_line(
                        Face::Regular,
                        ui_theme::small(),
                        22.0,
                        right_x,
                        ry,
                        col_danger,
                        &fit,
                    );
                }
            }
            ModSearch::Done(list) => {
                let row_h: f32 = if ultra_v {
                    30.0
                } else if compact_v {
                    34.0
                } else {
                    44.0
                };
                let row_gap: f32 = 6.0;
                let total_space = (bottom_limit - ry).max(0.0);
                if total_space < row_h {
                    return;
                }
                let fit_all = ((total_space + row_gap) / (row_h + row_gap))
                    .floor()
                    .max(1.0) as usize;
                let needs_pager = list.len() > fit_all && total_space >= row_h + 34.0;
                let pager_h: f32 = if needs_pager { 34.0 } else { 0.0 };
                let list_space = (total_space - pager_h).max(row_h);
                let per_page = ((list_space + row_gap) / (row_h + row_gap))
                    .floor()
                    .max(1.0) as usize;
                let total_pages = list.len().max(1).div_ceil(per_page);
                let page = self.new_profile_mods_page.min(total_pages - 1);
                let start = page * per_page;
                let end = (start + per_page).min(list.len());
                let rows_limit = bottom_limit - pager_h;

                for (i, m) in list.iter().enumerate().skip(start).take(end - start) {
                    if ry + row_h > rows_limit + 0.5 {
                        break;
                    }
                    let chosen = self.new_profile_mods.contains(&m.id);
                    draw_button(
                        ui,
                        hits,
                        ButtonId::ToggleNewProfileMod(i),
                        right_x,
                        ry,
                        right_w,
                        row_h,
                        if chosen {
                            Tone::Banner
                        } else {
                            Tone::Secondary
                        },
                        &format!(
                            "{} {}  ({})",
                            if chosen { "[x]" } else { "[ ]" },
                            m.name,
                            format_size(m.size)
                        ),
                    );
                    ry += row_h + row_gap;
                }
                if needs_pager && bottom_limit - 28.0 >= rows_limit {
                    let py = bottom_limit - 28.0;
                    if page > 0 {
                        draw_button(
                            ui,
                            hits,
                            ButtonId::NewProfilePrevMods,
                            right_x,
                            py,
                            74.0,
                            28.0,
                            Tone::Secondary,
                            "Prev",
                        );
                    }
                    if page + 1 < total_pages {
                        draw_button(
                            ui,
                            hits,
                            ButtonId::NewProfileNextMods,
                            right_x + right_w - 74.0,
                            py,
                            74.0,
                            28.0,
                            Tone::Secondary,
                            "Next",
                        );
                    }
                }
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
        id: &str,
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
            p.rounded(inner_x, cy + 5.0, 8.0, 8.0, 4.0, col_danger);
            p.text_line(
                Face::Mono,
                ui_theme::small(),
                18.0,
                inner_x + 16.0,
                cy,
                col_danger,
                &format!(
                    "DELETE PROFILE  |  {} WILL BE LOST",
                    world_phrase.to_uppercase()
                ),
            );
            cy += if compact_v { 20.0 } else { 26.0 };
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
            let body = format!(
                "Removing \"{name}\" permanently deletes profiles/{id}, its mods, and {world_phrase}. Existing Vanilla worlds in the main game folder are untouched."
            );
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

    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_rename_modal(
        &self,
        ui: &mut Canvas<'_, '_>,
        hits: &mut Vec<HitRect>,
        x: f32,
        y: f32,
        w: f32,
        avail_h: f32,
        id: &str,
        draft: &str,
        err: Option<&str>,
        col_danger: u32,
    ) {
        let ultra_v = avail_h < 145.0;
        let inner_x_ultra = x + 16.0;
        let inner_w_ultra = (w - 32.0).max(200.0);
        // Ultra-tight keyboard-up layout (e.g. 914x200): title + Cancel on top, input + Save side-by-side.
        if ultra_v && inner_w_ultra >= 360.0 {
            let pad_y: f32 = 6.0;
            let hdr_h: f32 = 30.0;
            let row_h: f32 = 34.0;
            let err_h: f32 = if err.is_some() { 18.0 } else { 0.0 };
            let needed_h = pad_y * 2.0 + hdr_h + 6.0 + row_h + err_h;
            let card_h = needed_h.min(avail_h);
            let cy_top = y + ((avail_h - card_h) * 0.5).max(0.0).round();
            draw_plate(ui, x, cy_top, w, card_h, palette::accent_star());

            let mut cy = cy_top + pad_y;
            let cancel_w = 86.0;
            draw_button(
                ui,
                hits,
                ButtonId::CancelRenameProfile,
                inner_x_ultra + inner_w_ultra - cancel_w,
                cy,
                cancel_w,
                hdr_h,
                Tone::Secondary,
                "Cancel",
            );
            if let Some(p) = ui.paint() {
                p.text_line(
                    Face::SemiBold,
                    17.0,
                    hdr_h,
                    inner_x_ultra,
                    cy,
                    palette::text(),
                    "Rename Profile",
                );
                let tw = p.measure(Face::SemiBold, 17.0, "Rename Profile");
                let folder = format!("profiles/{id}");
                let froom = (inner_w_ultra - cancel_w - tw - 22.0).max(40.0);
                let fit_f = truncate_to_fit(p, Face::Mono, ui_theme::small(), &folder, froom);
                p.text_line(
                    Face::Mono,
                    ui_theme::small(),
                    hdr_h,
                    inner_x_ultra + tw + 12.0,
                    cy,
                    palette::muted(),
                    &fit_f,
                );
            }
            cy += hdr_h + 6.0;

            let save_w = 126.0_f32.min(inner_w_ultra * 0.34);
            let eff_row_h = row_h.min((cy_top + card_h - cy - err_h - pad_y).max(30.0));
            draw_input_box(
                ui,
                hits,
                ButtonId::FocusRenameInput,
                inner_x_ultra,
                cy,
                inner_w_ultra - save_w - 8.0,
                eff_row_h,
                "Display Name",
                draft,
                false,
                true,
            );
            draw_button(
                ui,
                hits,
                ButtonId::SaveRenameProfile,
                inner_x_ultra + inner_w_ultra - save_w,
                cy,
                save_w,
                eff_row_h,
                Tone::Primary,
                "Save Name",
            );
            cy += eff_row_h + 2.0;
            if let Some(e) = err
                && let Some(p) = ui.paint()
            {
                let fit = truncate_to_fit(p, Face::Regular, ui_theme::small(), e, inner_w_ultra);
                p.text_line(
                    Face::Regular,
                    ui_theme::small(),
                    16.0,
                    inner_x_ultra,
                    cy,
                    col_danger,
                    &fit,
                );
            }
            return;
        }

        let tight_v = avail_h < 200.0;
        let compact_v = avail_h < 290.0;
        let pad_y: f32 = if tight_v {
            8.0
        } else if compact_v {
            12.0
        } else {
            20.0
        };
        let hdr_step: f32 = if tight_v {
            28.0
        } else if compact_v {
            48.0
        } else {
            56.0
        };
        let input_h: f32 = if tight_v {
            36.0
        } else if compact_v {
            40.0
        } else {
            44.0
        };
        let input_gap: f32 = if tight_v { 6.0 } else { 8.0 };
        let err_step: f32 = if err.is_some() {
            if tight_v { 18.0 } else { 22.0 }
        } else {
            0.0
        };
        let btn_h: f32 = if tight_v {
            36.0
        } else if compact_v {
            42.0
        } else {
            46.0
        };
        let needed_h = pad_y * 2.0 + hdr_step + input_h + input_gap + err_step + btn_h;
        let card_h = needed_h
            .max(if compact_v { 180.0 } else { 232.0 })
            .min(avail_h);
        let cy_top = y + ((avail_h - card_h) * 0.5).max(0.0).round();
        draw_plate(ui, x, cy_top, w, card_h, palette::accent_star());

        let inner_x = x + if tight_v { 18.0 } else { 24.0 };
        let inner_w = (w - if tight_v { 36.0 } else { 48.0 }).max(200.0);
        let mut cy = cy_top + pad_y;
        if let Some(p) = ui.paint() {
            let folder = format!("Folder stays profiles/{id}");
            if tight_v {
                p.text_line(
                    Face::SemiBold,
                    18.0,
                    24.0,
                    inner_x,
                    cy,
                    palette::text(),
                    "Rename Profile",
                );
                let title_w = p.measure(Face::SemiBold, 18.0, "Rename Profile");
                let folder_room = (inner_w - title_w - 14.0).max(60.0);
                let fit_folder =
                    truncate_to_fit(p, Face::Mono, ui_theme::small(), &folder, folder_room);
                let fw = p.measure(Face::Mono, ui_theme::small(), &fit_folder);
                p.text_line(
                    Face::Mono,
                    ui_theme::small(),
                    24.0,
                    inner_x + inner_w - fw,
                    cy,
                    palette::muted(),
                    &fit_folder,
                );
            } else {
                p.text_line(
                    Face::SemiBold,
                    if compact_v {
                        19.0
                    } else {
                        ui_theme::sizes().title
                    },
                    26.0,
                    inner_x,
                    cy,
                    palette::text(),
                    "Rename Profile",
                );
                let fit_folder =
                    truncate_to_fit(p, Face::Mono, ui_theme::small(), &folder, inner_w);
                p.text_line(
                    Face::Mono,
                    ui_theme::small(),
                    18.0,
                    inner_x,
                    cy + 28.0,
                    palette::muted(),
                    &fit_folder,
                );
            }
        }
        cy += hdr_step;

        draw_input_box(
            ui,
            hits,
            ButtonId::FocusRenameInput,
            inner_x,
            cy,
            inner_w,
            input_h,
            "Display Name",
            draft,
            false,
            true,
        );
        cy += input_h + input_gap;

        if let Some(e) = err {
            if let Some(p) = ui.paint() {
                let fit = truncate_to_fit(p, Face::Regular, ui_theme::small(), e, inner_w);
                p.text_line(
                    Face::Regular,
                    ui_theme::small(),
                    18.0,
                    inner_x,
                    cy,
                    col_danger,
                    &fit,
                );
            }
            cy += err_step;
        }

        let eff_btn_h = btn_h.min((cy_top + card_h - cy - pad_y).max(32.0));
        let btn_y = (cy_top + card_h - eff_btn_h - pad_y).max(cy);
        let half_w = (inner_w - 12.0) * 0.5;
        draw_button(
            ui,
            hits,
            ButtonId::SaveRenameProfile,
            inner_x,
            btn_y,
            half_w,
            eff_btn_h,
            Tone::Primary,
            "Save Name",
        );
        draw_button(
            ui,
            hits,
            ButtonId::CancelRenameProfile,
            inner_x + half_w + 12.0,
            btn_y,
            half_w,
            eff_btn_h,
            Tone::Secondary,
            "Cancel",
        );
    }
}
