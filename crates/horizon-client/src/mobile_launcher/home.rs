//! Home overview, Profile picker/cards, New Profile form, and Rename/Remove profile modals.

use super::widgets::{
    Canvas, HitRect, Tone, count_label, draw_button, draw_input_box, draw_plate,
    effective_profile_state, format_size, palette, profile_badge_parts, profile_stats_line,
    stage_fraction, truncate_to_fit, wrap_lines,
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
        picker_title: Option<&str>,
    ) {
        let compact_v = avail_h < 320.0;
        let selected_id = self.backend.selected_profile();
        let selected = profiles
            .iter()
            .find(|p| p.id == selected_id)
            .or_else(|| profiles.first());
        let sel_is_custom = selected.is_some_and(|p| p.id != VANILLA);
        let sel_name = selected.map_or("Vanilla", |p| p.name.as_str());

        // Single calm unified workspace panel
        draw_plate(ui, x, y, w, avail_h, palette::border());
        let lx = x + if compact_v { 16.0 } else { 22.0 };
        let lw = w - if compact_v { 32.0 } else { 44.0 };
        let mut ly = y + if compact_v { 10.0 } else { 18.0 };
        let hdr_h: f32 = if compact_v { 36.0 } else { 54.0 };
        let btn_h: f32 = if compact_v { 34.0 } else { 38.0 };
        let btn_y = ly + ((hdr_h - btn_h) * 0.5).round();

        let new_w = if compact_v { 126.0 } else { 138.0 };
        let mut right_edge = lx + lw;
        draw_button(
            ui,
            hits,
            ButtonId::OpenNewProfile,
            right_edge - new_w,
            btn_y,
            new_w,
            btn_h,
            Tone::Secondary,
            "+ New profile",
        );
        right_edge -= new_w + 8.0;

        if sel_is_custom {
            let del_w = if compact_v { 72.0 } else { 80.0 };
            draw_button(
                ui,
                hits,
                ButtonId::AskRemoveSelected,
                right_edge - del_w,
                btn_y,
                del_w,
                btn_h,
                Tone::Danger,
                "Delete",
            );
            right_edge -= del_w + 6.0;
            let ren_w = if compact_v { 76.0 } else { 84.0 };
            draw_button(
                ui,
                hits,
                ButtonId::OpenRenameSelected,
                right_edge - ren_w,
                btn_y,
                ren_w,
                btn_h,
                Tone::Secondary,
                "Rename",
            );
            right_edge -= ren_w + 6.0;
        }

        if !compact_v && picker_title.is_none() {
            let mods_w = 78.0;
            draw_button(
                ui,
                hits,
                ButtonId::OpenModsForSelected,
                right_edge - mods_w,
                btn_y,
                mods_w,
                btn_h,
                Tone::Ghost,
                "Mods",
            );
            right_edge -= mods_w + 8.0;
        }

        if let Some(p) = ui.paint() {
            let title_max = (right_edge - lx - 10.0).max(80.0);
            let heading = picker_title.unwrap_or(sel_name);
            let fit_heading = truncate_to_fit(
                p,
                Face::SemiBold,
                if compact_v { 18.0 } else { 22.0 },
                heading,
                title_max,
            );
            if compact_v {
                p.text_line(
                    Face::SemiBold,
                    18.0,
                    hdr_h,
                    lx,
                    ly,
                    palette::text(),
                    &fit_heading,
                );
            } else {
                p.text_line(
                    Face::SemiBold,
                    22.0,
                    28.0,
                    lx,
                    ly + 2.0,
                    palette::text(),
                    &fit_heading,
                );
                let sub = if let Some(note) = &self.status_note {
                    note.clone()
                } else if picker_title.is_some() {
                    "Worlds stay separate per profile. Vanilla keeps your main game folder."
                        .to_owned()
                } else if let Some(prof) = selected {
                    profile_stats_line(prof)
                } else {
                    "0 mods  |  0 worlds".to_owned()
                };
                let sub_col = if self.status_note.is_some() {
                    palette::accent_star()
                } else {
                    palette::muted()
                };
                let fit_sub = truncate_to_fit(p, Face::Regular, ui_theme::small(), &sub, title_max);
                p.text_line(
                    Face::Regular,
                    ui_theme::small(),
                    18.0,
                    lx,
                    ly + 32.0,
                    sub_col,
                    &fit_sub,
                );
            }
            let div_y = ly + hdr_h + if compact_v { 6.0 } else { 12.0 };
            p.rounded(x + 1.0, div_y, w - 2.0, 1.0, 0.0, palette::border_subtle());
        }
        ly += hdr_h + if compact_v { 14.0 } else { 26.0 };

        if !compact_v && picker_title.is_none() {
            if let Some(p) = ui.paint() {
                p.text_line(
                    Face::Mono,
                    11.0,
                    16.0,
                    lx,
                    ly,
                    palette::muted(),
                    "PROFILES  -  WORLDS STAY SEPARATE PER PROFILE",
                );
            }
            ly += 24.0;
        }

        let bottom_pad = if compact_v { 8.0 } else { 16.0 };
        let item_h = if compact_v { 66.0 } else { 78.0 };
        let item_gap = if compact_v { 6.0 } else { 10.0 };
        let space = (y + avail_h - bottom_pad - ly).max(item_h);
        let fit_all = ((space + item_gap) / (item_h + item_gap)).floor().max(1.0) as usize;
        let needs_pager = profiles.len() > fit_all;
        let pager_h = if needs_pager { 38.0 } else { 0.0 };
        let per_page = ((space - pager_h + item_gap) / (item_h + item_gap))
            .floor()
            .max(1.0) as usize;
        let total_pages = profiles.len().max(1).div_ceil(per_page);
        let page = self.profile_page.min(total_pages - 1);
        let start = page * per_page;
        let end = (start + per_page).min(profiles.len());

        for (idx, profile) in profiles.iter().enumerate().skip(start).take(end - start) {
            let is_selected = profile.id == selected_id;
            let state = effective_profile_state(profile, setup);
            let card_hit = HitRect {
                id: ButtonId::SelectProfile(idx),
                x: lx,
                y: ly,
                w: lw,
                h: item_h,
            };
            if let Some(p) = ui.paint() {
                let fill = if is_selected {
                    color::with_alpha(palette::accent(), 0.14)
                } else if card_hit.contains(p.cursor) {
                    palette::surface_hover()
                } else {
                    palette::surface()
                };
                let bdr = if is_selected {
                    color::with_alpha(palette::accent_star(), 0.72)
                } else {
                    palette::border()
                };
                p.rounded(lx, ly, lw, item_h, 11.0, fill);
                p.border(lx, ly, lw, item_h, 11.0, bdr);
                if is_selected {
                    p.rounded(
                        lx + 4.0,
                        ly + 14.0,
                        3.0,
                        item_h - 28.0,
                        1.5,
                        palette::accent_star(),
                    );
                }

                let (badge, bcol) =
                    profile_badge_parts(&state, is_selected, col_success, col_danger);
                let bw = p.measure(Face::Mono, ui_theme::small(), &badge) + 16.0;
                let bx = lx + lw - 14.0 - bw;
                let by = ly + if compact_v { 8.0 } else { 14.0 };
                p.rounded(bx, by, bw, 22.0, 6.0, color::with_alpha(bcol, 0.16));
                p.border(bx, by, bw, 22.0, 6.0, color::with_alpha(bcol, 0.75));
                p.text_line(
                    Face::Mono,
                    ui_theme::small(),
                    22.0,
                    bx + 8.0,
                    by,
                    bcol,
                    &badge,
                );

                let text_x = lx + if is_selected { 18.0 } else { 14.0 };
                let title_w = (bx - text_x - 12.0).max(60.0);
                let fit_name = truncate_to_fit(
                    p,
                    Face::SemiBold,
                    if compact_v { 15.5 } else { 17.0 },
                    &profile.name,
                    title_w,
                );
                p.text_line(
                    Face::SemiBold,
                    if compact_v { 15.5 } else { 17.0 },
                    22.0,
                    text_x,
                    by - 1.0,
                    palette::text(),
                    &fit_name,
                );

                let stats = profile_stats_line(profile);
                let fit_stats =
                    truncate_to_fit(p, Face::Mono, ui_theme::small(), &stats, lw - 32.0);
                let sy = ly + if compact_v { 32.0 } else { 40.0 };
                p.text_line(
                    Face::Mono,
                    ui_theme::small(),
                    18.0,
                    text_x,
                    sy,
                    palette::muted(),
                    &fit_stats,
                );

                if let ProfileState::Installing(prog) = &state {
                    let frac = stage_fraction(prog);
                    let bar_y = ly + item_h - 11.0;
                    p.rounded(text_x, bar_y, lw - 32.0, 5.0, 2.5, palette::bg_outer());
                    if frac > 0.0 {
                        p.rounded(
                            text_x,
                            bar_y,
                            ((lw - 32.0) * frac).max(8.0),
                            5.0,
                            2.5,
                            palette::accent(),
                        );
                    }
                }
            }
            hits.push(card_hit);
            ly += item_h + item_gap;
        }

        if needs_pager {
            let py = y + avail_h - bottom_pad - 32.0;
            if page > 0 {
                draw_button(
                    ui,
                    hits,
                    ButtonId::ProfilesPrevPage,
                    lx,
                    py,
                    80.0,
                    32.0,
                    Tone::Secondary,
                    "Prev",
                );
            }
            if let Some(p) = ui.paint() {
                let txt = format!("{} / {total_pages}", page + 1);
                let tw = p.measure(Face::Mono, ui_theme::small(), &txt);
                p.text_line(
                    Face::Mono,
                    ui_theme::small(),
                    32.0,
                    lx + ((lw - tw) * 0.5).round(),
                    py,
                    palette::muted(),
                    &txt,
                );
            }
            if page + 1 < total_pages {
                draw_button(
                    ui,
                    hits,
                    ButtonId::ProfilesNextPage,
                    lx + lw - 80.0,
                    py,
                    80.0,
                    32.0,
                    Tone::Secondary,
                    "Next",
                );
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
        let compact_v = avail_h < 300.0;
        draw_plate(ui, x, y, w, avail_h, palette::border());

        let ix = x + 18.0;
        let iw = w - 36.0;
        let mut cy = y + if compact_v { 8.0 } else { 16.0 };
        let hdr_h = if compact_v { 32.0 } else { 38.0 };

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
        draw_input_box(
            ui,
            hits,
            ButtonId::FocusNewProfileName,
            ix,
            ly,
            left_w,
            if compact_v { 36.0 } else { 44.0 },
            "Profile Name",
            &self.new_profile_name,
            false,
            true,
        );
        ly += if compact_v { 40.0 } else { 50.0 };

        let taken: Vec<String> = profiles.iter().map(|p| p.id.clone()).collect();
        let preview_slug = if self.new_profile_name.trim().is_empty() {
            "profile".to_owned()
        } else {
            profile_id(&self.new_profile_name, &taken)
        };
        if let Some(p) = ui.paint() {
            p.text_line(
                Face::Mono,
                ui_theme::small(),
                16.0,
                ix,
                ly,
                palette::muted(),
                &format!("Folder: profiles/{preview_slug}"),
            );
        }
        ly += if compact_v { 18.0 } else { 22.0 };

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

        let presets = self.backend.presets();
        let chip_h = if compact_v { 34.0 } else { 40.0 };
        let chip_cols = (presets.len() + 1).max(1);
        let chip_w = (left_w - (chip_cols as f32 - 1.0) * 6.0) / chip_cols as f32;
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

        if let Some(err) = &self.new_profile_error {
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

        let create_h = if compact_v { 42.0 } else { 48.0 };
        let create_y = (y + avail_h - create_h - 10.0).min(ly.max(cy + 122.0));
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
                let row_h = if compact_v { 36.0 } else { 44.0 };
                let row_gap = 6.0;
                let avail = (y + avail_h - 46.0 - ry).max(row_h);
                let per_page = ((avail + row_gap) / (row_h + row_gap)).floor().max(1.0) as usize;
                let total_pages = list.len().max(1).div_ceil(per_page);
                let page = self.new_profile_mods_page.min(total_pages - 1);
                let start = page * per_page;
                let end = (start + per_page).min(list.len());

                for (i, m) in list.iter().enumerate().skip(start).take(end - start) {
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
                if list.len() > per_page {
                    let py = y + avail_h - 40.0;
                    if page > 0 {
                        draw_button(
                            ui,
                            hits,
                            ButtonId::NewProfilePrevMods,
                            right_x,
                            py,
                            74.0,
                            30.0,
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
                            30.0,
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
        let compact_v = avail_h < 280.0;
        let card_h = 248.0_f32.min(avail_h);
        let cy_top = y + ((avail_h - card_h) * 0.5).max(0.0).round();
        draw_plate(ui, x, cy_top, w, card_h, palette::accent_star());

        let inner_x = x + 24.0;
        let inner_w = w - 48.0;
        let mut cy = cy_top + if compact_v { 12.0 } else { 20.0 };
        if let Some(p) = ui.paint() {
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
            p.text_line(
                Face::Mono,
                ui_theme::small(),
                18.0,
                inner_x,
                cy + 28.0,
                palette::muted(),
                &format!("Folder stays profiles/{id}"),
            );
        }
        cy += if compact_v { 48.0 } else { 56.0 };

        draw_input_box(
            ui,
            hits,
            ButtonId::FocusRenameInput,
            inner_x,
            cy,
            inner_w,
            if compact_v { 40.0 } else { 44.0 },
            "Display Name",
            draft,
            false,
            true,
        );
        cy += if compact_v { 46.0 } else { 52.0 };

        if let Some(e) = err
            && let Some(p) = ui.paint()
        {
            p.text_line(
                Face::Regular,
                ui_theme::small(),
                18.0,
                inner_x,
                cy,
                col_danger,
                e,
            );
        }

        let btn_h = if compact_v { 42.0 } else { 46.0 };
        let btn_y = cy_top + card_h - btn_h - if compact_v { 10.0 } else { 16.0 };
        let half_w = (inner_w - 12.0) * 0.5;
        draw_button(
            ui,
            hits,
            ButtonId::SaveRenameProfile,
            inner_x,
            btn_y,
            half_w,
            btn_h,
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
            btn_h,
            Tone::Secondary,
            "Cancel",
        );
    }
}
