//! Mod Manager: profile name tabs, per-mod ON/OFF + Remove controls, and Horizon registry browser.

use super::widgets::{
    Canvas, HitRect, Tone, draw_button, draw_input_box, draw_plate, format_size, palette,
    truncate_to_fit,
};
use super::{ButtonId, MobileLauncher};
use crate::launcher_backend::{ModSearch, Profile, VANILLA};
use crate::ui_theme::{self, Face};

impl MobileLauncher {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_mods_view(
        &self,
        ui: &mut Canvas<'_, '_>,
        hits: &mut Vec<HitRect>,
        x: f32,
        y: f32,
        w: f32,
        avail_h: f32,
        profiles: &[Profile],
        col_success: u32,
        col_danger: u32,
    ) {
        let compact_v = avail_h < 300.0;
        draw_plate(ui, x, y, w, avail_h, palette::border());

        let ix = x + 16.0;
        let iw = w - 32.0;
        let mut cy = y + if compact_v { 8.0 } else { 16.0 };
        let top_btn_h = if compact_v { 32.0 } else { 38.0 };

        let pid = self.effective_mods_profile_id(profiles);
        let mut chip_x = ix;
        for (i, prof) in profiles.iter().enumerate() {
            // Always show the profile's display name (not its folder id).
            let label = prof.name.as_str();
            let cw =
                (ui.ro().measure(Face::Medium, ui_theme::small(), label) + 24.0).clamp(72.0, 180.0);
            if chip_x + cw > ix + iw {
                break;
            }
            draw_button(
                ui,
                hits,
                ButtonId::ModManagerPickProfile(i),
                chip_x,
                cy,
                cw,
                top_btn_h,
                if prof.id == pid {
                    Tone::Banner
                } else {
                    Tone::Secondary
                },
                label,
            );
            chip_x += cw + 6.0;
        }
        cy += top_btn_h + if compact_v { 6.0 } else { 12.0 };

        if pid == VANILLA {
            if let Some(p) = ui.paint() {
                p.text_line(
                    Face::SemiBold,
                    20.0,
                    28.0,
                    ix,
                    cy + 10.0,
                    palette::text(),
                    "Vanilla plays without mods",
                );
                p.text_line(
                    Face::Regular,
                    ui_theme::sizes().ui,
                    22.0,
                    ix,
                    cy + 42.0,
                    palette::muted(),
                    "Your main game folder keeps vanilla worlds safe. Create or pick a modded profile.",
                );
            }
            draw_button(
                ui,
                hits,
                ButtonId::OpenNewProfile,
                ix,
                cy + 80.0,
                210.0,
                46.0,
                Tone::Primary,
                "+ Create Modded Profile",
            );
            return;
        }

        if let Some(err) = &self.mods_error {
            if let Some(p) = ui.paint() {
                let fit = truncate_to_fit(p, Face::Regular, ui_theme::small(), err, iw);
                p.text_line(
                    Face::Regular,
                    ui_theme::small(),
                    18.0,
                    ix,
                    cy,
                    col_danger,
                    &fit,
                );
            }
            cy += 20.0;
        }

        let pane_gap = 12.0;
        let pane_w = ((iw - pane_gap) * 0.5).round();
        let pane_h = (y + avail_h - 12.0 - cy).max(140.0);
        let right_x = ix + pane_w + pane_gap;

        self.draw_profile_mods_pane(ui, hits, ix, cy, pane_w, pane_h, &pid, compact_v);
        self.draw_registry_pane(
            ui,
            hits,
            right_x,
            cy,
            iw - pane_w - pane_gap,
            pane_h,
            &pid,
            compact_v,
            col_success,
            col_danger,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_profile_mods_pane(
        &self,
        ui: &mut Canvas<'_, '_>,
        hits: &mut Vec<HitRect>,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        pid: &str,
        compact_v: bool,
    ) {
        if let Some(p) = ui.paint() {
            p.rounded(x, y, w, h, 10.0, palette::surface());
            p.border(x, y, w, h, 10.0, palette::border());
        }
        let px = x + 10.0;
        let pw = w - 20.0;
        let mut py = y + 8.0;
        let mods = self.backend.mods(pid);
        if let Some(p) = ui.paint() {
            p.text_line(
                Face::SemiBold,
                15.0,
                22.0,
                px,
                py,
                palette::text(),
                &format!("Profile Mods ({})", mods.len()),
            );
        }
        py += 24.0;

        if mods.is_empty() {
            if let Some(p) = ui.paint() {
                p.text_line(
                    Face::Regular,
                    ui_theme::small(),
                    20.0,
                    px,
                    py + 8.0,
                    palette::muted(),
                    "No mods yet. Add from Horizon's registry ->",
                );
            }
            return;
        }

        let row_h = if compact_v { 48.0 } else { 60.0 };
        let row_gap = 6.0;
        let avail = (y + h - 38.0 - py).max(row_h);
        let per_page = ((avail + row_gap) / (row_h + row_gap)).floor().max(1.0) as usize;
        let total_pages = mods.len().max(1).div_ceil(per_page);
        let page = self.mods_list_page.min(total_pages - 1);
        let start = page * per_page;
        let end = (start + per_page).min(mods.len());

        let rem_w = 78.0;
        let tog_w = 50.0;
        for (i, pm) in mods.iter().enumerate().skip(start).take(end - start) {
            if let Some(p) = ui.paint() {
                p.rounded(px, py, pw, row_h, 8.0, palette::card());
                p.border(px, py, pw, row_h, 8.0, palette::border());
                let max_nw = (pw - rem_w - tog_w - 26.0).max(46.0);
                let fit_n = truncate_to_fit(p, Face::Medium, 14.0, &pm.info.name, max_nw);
                p.text_line(
                    Face::Medium,
                    14.0,
                    20.0,
                    px + 10.0,
                    py + if compact_v { 4.0 } else { 7.0 },
                    palette::text(),
                    &fit_n,
                );
                let meta = format!("v{}  |  {}", pm.info.version, format_size(pm.info.size));
                let fit_m = truncate_to_fit(p, Face::Mono, 11.0, &meta, max_nw);
                p.text_line(
                    Face::Mono,
                    11.0,
                    16.0,
                    px + 10.0,
                    py + if compact_v { 24.0 } else { 30.0 },
                    palette::muted(),
                    &fit_m,
                );
            }
            let bh = 32.0;
            let by = py + ((row_h - bh) * 0.5).round();
            draw_button(
                ui,
                hits,
                ButtonId::RemoveProfileMod(i),
                px + pw - 6.0 - rem_w,
                by,
                rem_w,
                bh,
                Tone::Danger,
                "Remove",
            );
            draw_button(
                ui,
                hits,
                ButtonId::ToggleProfileMod(i),
                px + pw - 12.0 - rem_w - tog_w,
                by,
                tog_w,
                bh,
                if pm.enabled {
                    Tone::Banner
                } else {
                    Tone::Secondary
                },
                if pm.enabled { "ON" } else { "OFF" },
            );
            py += row_h + row_gap;
        }

        if total_pages > 1 {
            let fy = y + h - 34.0;
            if page > 0 {
                draw_button(
                    ui,
                    hits,
                    ButtonId::ProfileModsPrev,
                    px,
                    fy,
                    70.0,
                    28.0,
                    Tone::Secondary,
                    "Prev",
                );
            }
            if page + 1 < total_pages {
                draw_button(
                    ui,
                    hits,
                    ButtonId::ProfileModsNext,
                    px + pw - 70.0,
                    fy,
                    70.0,
                    28.0,
                    Tone::Secondary,
                    "Next",
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_registry_pane(
        &self,
        ui: &mut Canvas<'_, '_>,
        hits: &mut Vec<HitRect>,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        pid: &str,
        compact_v: bool,
        col_success: u32,
        col_danger: u32,
    ) {
        if let Some(p) = ui.paint() {
            p.rounded(x, y, w, h, 10.0, palette::surface());
            p.border(x, y, w, h, 10.0, palette::border());
        }
        let px = x + 10.0;
        let pw = w - 20.0;
        let mut py = y + 8.0;

        if let Some(p) = ui.paint() {
            p.text_line(
                Face::SemiBold,
                15.0,
                22.0,
                px,
                py,
                palette::text(),
                "Horizon Registry",
            );
        }
        py += 22.0;

        let search_btn_w = 72.0;
        let search_h = if compact_v { 30.0 } else { 36.0 };
        draw_input_box(
            ui,
            hits,
            ButtonId::FocusModSearch,
            px,
            py,
            pw - search_btn_w - 6.0,
            search_h,
            "",
            if self.mod_search_query.is_empty() && !self.mod_search_focused {
                "Search Horizon mods..."
            } else {
                &self.mod_search_query
            },
            false,
            self.mod_search_focused,
        );
        draw_button(
            ui,
            hits,
            ButtonId::SubmitModSearch,
            px + pw - search_btn_w,
            py,
            search_btn_w,
            search_h,
            Tone::Primary,
            "Find",
        );
        py += search_h + 6.0;

        let installed_ids: Vec<String> = self
            .backend
            .mods(pid)
            .into_iter()
            .map(|m| m.info.id)
            .collect();

        match self.backend.mod_search() {
            ModSearch::Idle | ModSearch::Searching => {
                if let Some(p) = ui.paint() {
                    p.text_line(
                        Face::Regular,
                        ui_theme::small(),
                        22.0,
                        px,
                        py + 6.0,
                        palette::muted(),
                        "Searching Horizon registry...",
                    );
                }
            }
            ModSearch::Failed(err) => {
                if let Some(p) = ui.paint() {
                    let fit = truncate_to_fit(p, Face::Regular, ui_theme::small(), &err, pw);
                    p.text_line(
                        Face::Regular,
                        ui_theme::small(),
                        22.0,
                        px,
                        py + 6.0,
                        col_danger,
                        &fit,
                    );
                }
            }
            ModSearch::Done(results) => {
                if results.is_empty() {
                    if let Some(p) = ui.paint() {
                        p.text_line(
                            Face::Regular,
                            ui_theme::small(),
                            22.0,
                            px,
                            py + 6.0,
                            palette::muted(),
                            "No mods matched.",
                        );
                    }
                    return;
                }
                let row_h = if compact_v { 42.0 } else { 56.0 };
                let row_gap = 6.0;
                let avail = (y + h - 36.0 - py).max(row_h);
                let per_page = ((avail + row_gap) / (row_h + row_gap)).floor().max(1.0) as usize;
                let total_pages = results.len().max(1).div_ceil(per_page);
                let page = self.mod_registry_page.min(total_pages - 1);
                let start = page * per_page;
                let end = (start + per_page).min(results.len());

                for (i, m) in results.iter().enumerate().skip(start).take(end - start) {
                    let already = installed_ids.contains(&m.id);
                    if let Some(p) = ui.paint() {
                        p.rounded(px, py, pw, row_h, 8.0, palette::card());
                        p.border(px, py, pw, row_h, 8.0, palette::border());
                        let max_nw = (pw - 82.0).max(46.0);
                        let fit_n = truncate_to_fit(p, Face::Medium, 14.0, &m.name, max_nw);
                        p.text_line(
                            Face::Medium,
                            14.0,
                            18.0,
                            px + 10.0,
                            py + if compact_v { 3.0 } else { 6.0 },
                            palette::text(),
                            &fit_n,
                        );
                        let sub = format!("{}  |  {}", format_size(m.size), m.description);
                        let fit_s = truncate_to_fit(p, Face::Regular, 11.0, &sub, max_nw);
                        p.text_line(
                            Face::Regular,
                            11.0,
                            16.0,
                            px + 10.0,
                            py + if compact_v { 22.0 } else { 26.0 },
                            palette::muted(),
                            &fit_s,
                        );
                        if already {
                            let aw = p.measure(Face::Mono, 11.0, "Added");
                            p.text_line(
                                Face::Mono,
                                11.0,
                                row_h,
                                px + pw - 10.0 - aw,
                                py,
                                col_success,
                                "Added",
                            );
                        }
                    }
                    if !already {
                        draw_button(
                            ui,
                            hits,
                            ButtonId::AddRegistryMod(i),
                            px + pw - 70.0,
                            py + ((row_h - 30.0) * 0.5).round(),
                            64.0,
                            30.0,
                            Tone::Primary,
                            "+ Add",
                        );
                    }
                    py += row_h + row_gap;
                }

                if total_pages > 1 {
                    let fy = y + h - 34.0;
                    if page > 0 {
                        draw_button(
                            ui,
                            hits,
                            ButtonId::RegistryModsPrev,
                            px,
                            fy,
                            70.0,
                            28.0,
                            Tone::Secondary,
                            "Prev",
                        );
                    }
                    if page + 1 < total_pages {
                        draw_button(
                            ui,
                            hits,
                            ButtonId::RegistryModsNext,
                            px + pw - 70.0,
                            fy,
                            70.0,
                            28.0,
                            Tone::Secondary,
                            "Next",
                        );
                    }
                }
            }
        }
    }
}
