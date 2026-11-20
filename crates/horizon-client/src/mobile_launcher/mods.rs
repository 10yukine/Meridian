//! Mod Manager: profile name tabs, per-mod ON/OFF + Remove controls, and Horizon registry browser.

use super::widgets::{
    Canvas, HitRect, Tone, draw_button, draw_icon_box, draw_input_box, draw_plate,
    draw_tab_with_tools, format_size, palette, truncate_to_fit, wrap_lines,
};
use super::{ButtonId, MobileLauncher};
use crate::launcher_backend::{ModSearch, Profile, VANILLA};
use crate::ui_theme::{self, Face, color};

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
        let ultra_v = avail_h < 150.0;
        let compact_v = avail_h < 300.0;
        draw_plate(ui, x, y, w, avail_h, palette::border());

        let ix = x + 16.0;
        let iw = w - 32.0;
        let mut cy = y + if ultra_v {
            6.0
        } else if compact_v {
            8.0
        } else {
            14.0
        };
        let top_btn_h = if ultra_v {
            26.0
        } else if compact_v {
            32.0
        } else {
            36.0
        };

        let new_w = if compact_v { 116.0 } else { 128.0 };
        draw_button(
            ui,
            hits,
            ButtonId::OpenNewProfile,
            ix + iw - new_w,
            cy,
            new_w,
            top_btn_h,
            Tone::Secondary,
            "New Profile",
        );

        let pid = self.effective_mods_profile_id(profiles);
        let mut chip_x = ix;
        // Tabs that don't fit go on the next row: the first ends before New Profile.
        let mut row_end = ix + iw - new_w - 10.0;
        for (i, prof) in profiles.iter().enumerate() {
            let renaming = self
                .profile_rename
                .as_ref()
                .filter(|(id, _)| *id == prof.id);
            // Always show the profile's display name (not its folder id).
            let label = prof.name.as_str();
            let cw =
                (ui.ro().measure(Face::Medium, ui_theme::small(), label) + 26.0).clamp(72.0, 180.0);
            // The modded profile shown carries its own tools in its cell: rename and delete.
            let tools = prof.id == pid && prof.id != VANILLA;
            let tool_w = top_btn_h;
            let done_w = 72.0;
            let want = match (renaming, tools) {
                (Some(_), _) => 100.0 + 6.0 + done_w,
                (None, true) => cw + 2.0 * tool_w,
                (None, false) => cw,
            };
            if chip_x > ix && chip_x + want > row_end {
                chip_x = ix;
                cy += top_btn_h + 6.0;
                row_end = ix + iw;
            }
            // Its name typed in place, and Done: with the keyboard up, this row is what shows.
            if let Some((_, typed)) = renaming {
                let field_w = (row_end - chip_x - 6.0 - done_w).min(260.0);
                draw_input_box(
                    ui,
                    hits,
                    ButtonId::FocusRenameInput,
                    chip_x,
                    cy,
                    field_w,
                    top_btn_h,
                    // The old name until a new one is typed: it stays if none is.
                    if typed.is_empty() { &prof.name } else { "" },
                    typed,
                    false,
                    true,
                );
                draw_button(
                    ui,
                    hits,
                    ButtonId::SaveRenameProfile,
                    chip_x + field_w + 6.0,
                    cy,
                    done_w,
                    top_btn_h,
                    Tone::Primary,
                    "Done",
                );
                chip_x += field_w + done_w + 12.0;
                continue;
            }
            if tools {
                draw_tab_with_tools(
                    ui,
                    hits,
                    ButtonId::ModManagerPickProfile(i),
                    ButtonId::RenameTabProfile,
                    ButtonId::DeleteTabProfile,
                    chip_x,
                    cy,
                    cw,
                    tool_w,
                    top_btn_h,
                    label,
                );
                chip_x += cw + 2.0 * tool_w + 6.0;
                continue;
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
                    Tone::Ghost
                },
                label,
            );
            chip_x += cw + 6.0;
        }
        cy += top_btn_h
            + if ultra_v {
                4.0
            } else if compact_v {
                6.0
            } else {
                12.0
            };
        if let Some(p) = ui.paint() {
            p.rounded(x + 1.0, cy, w - 2.0, 1.0, 0.0, palette::border_subtle());
        }
        cy += if ultra_v {
            4.0
        } else if compact_v {
            8.0
        } else {
            14.0
        };

        if pid == VANILLA {
            let rem_h = (y + avail_h - cy).max(0.0);
            let center_x = x + w * 0.5;
            let badge_sz = if compact_v { 44.0 } else { 58.0 };
            let sub = "Mods go in their own profiles, so Vanilla worlds stay untouched.";
            let sub_max = (iw - 40.0).min(460.0);
            let sub_lines: Vec<String> =
                wrap_lines(ui.ro(), Face::Regular, ui_theme::small(), sub, sub_max)
                    .into_iter()
                    .take(2)
                    .collect();
            let cta_w = 216.0;
            let cta_h = if compact_v { 38.0 } else { 44.0 };
            let badge_gap = if compact_v { 10.0 } else { 18.0 };
            let heading_h = if compact_v { 24.0 } else { 30.0 };
            let text_h = badge_sz + badge_gap + heading_h + sub_lines.len() as f32 * 18.0;
            let block_h = text_h + 14.0 + cta_h;
            let mut ey = cy + ((rem_h - block_h) * 0.5).max(4.0).round();
            // The button follows the text (it ends inside the plate on every phone layout).
            let cta_y = (ey + text_h + 14.0).min(y + avail_h - cta_h - 10.0);

            if let Some(p) = ui.paint() {
                draw_icon_box(
                    p,
                    center_x,
                    ey + badge_sz * 0.5,
                    badge_sz * 0.46,
                    palette::muted(),
                );
                ey += badge_sz + badge_gap;

                let heading = "Vanilla plays without mods";
                let h_sz = if compact_v { 17.0 } else { 19.0 };
                let hw = p.measure(Face::SemiBold, h_sz, heading);
                p.text_line(
                    Face::SemiBold,
                    h_sz,
                    24.0,
                    (center_x - hw * 0.5).round(),
                    ey,
                    palette::text(),
                    heading,
                );
                ey += heading_h;

                for (li, line) in sub_lines.iter().enumerate() {
                    let lw = p.measure(Face::Regular, ui_theme::small(), line);
                    p.text_line(
                        Face::Regular,
                        ui_theme::small(),
                        18.0,
                        (center_x - lw * 0.5).round(),
                        ey + li as f32 * 18.0,
                        palette::muted(),
                        line,
                    );
                }
            }
            draw_button(
                ui,
                hits,
                ButtonId::OpenNewProfile,
                (center_x - cta_w * 0.5).round(),
                cta_y,
                cta_w,
                cta_h,
                Tone::Primary,
                "Create Modded Profile",
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
        let pane_h = (y + avail_h - 12.0 - cy).max(0.0);
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
                let cx = x + w * 0.5;
                let mid_y = py + ((y + h - py) * 0.5).max(52.0);
                let sz = if compact_v { 36.0 } else { 44.0 };
                let bx = (cx - sz * 0.5).round();
                let by = (mid_y - sz - 10.0).max(py + 6.0).round();
                p.rounded(
                    bx,
                    by,
                    sz,
                    sz,
                    11.0,
                    color::with_alpha(palette::accent(), 0.16),
                );
                p.border(
                    bx,
                    by,
                    sz,
                    sz,
                    11.0,
                    color::with_alpha(palette::accent_star(), 0.45),
                );
                draw_icon_box(p, cx, by + sz * 0.5, sz * 0.48, palette::accent_star());
                let title = "No mods in this profile yet";
                let tw = p.measure(Face::Medium, 13.5, title);
                p.text_line(
                    Face::Medium,
                    13.5,
                    18.0,
                    (cx - tw * 0.5).round(),
                    by + sz + 10.0,
                    palette::text(),
                    title,
                );
                let sub = "Add mods from Horizon's registry ->";
                let sw = p.measure(Face::Regular, ui_theme::small(), sub);
                p.text_line(
                    Face::Regular,
                    ui_theme::small(),
                    16.0,
                    (cx - sw * 0.5).round(),
                    by + sz + 30.0,
                    palette::muted(),
                    sub,
                );
            }
            return;
        }

        let row_h: f32 = if compact_v { 48.0 } else { 60.0 };
        let row_gap: f32 = 6.0;
        let bottom_limit = y + h - 8.0;
        let total_space = (bottom_limit - py).max(0.0);
        if total_space < row_h {
            return;
        }
        let fit_all = ((total_space + row_gap) / (row_h + row_gap))
            .floor()
            .max(1.0) as usize;
        let needs_pager = mods.len() > fit_all && total_space >= row_h + 34.0;
        let pager_h: f32 = if needs_pager { 34.0 } else { 0.0 };
        let avail = (total_space - pager_h).max(row_h);
        let per_page = ((avail + row_gap) / (row_h + row_gap)).floor().max(1.0) as usize;
        let total_pages = mods.len().max(1).div_ceil(per_page);
        let page = self.mods_list_page.min(total_pages - 1);
        let start = page * per_page;
        let end = (start + per_page).min(mods.len());
        let rows_limit = bottom_limit - pager_h;

        let rem_w = 78.0;
        let tog_w = 50.0;
        for (i, pm) in mods.iter().enumerate().skip(start).take(end - start) {
            if py + row_h > rows_limit + 0.5 {
                break;
            }
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
                let meta = format!("v{}  ·  {}", pm.info.version, format_size(pm.info.size));
                let fit_m = truncate_to_fit(p, Face::Regular, 11.0, &meta, max_nw);
                p.text_line(
                    Face::Regular,
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

        if needs_pager && total_pages > 1 {
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
        let ultra_pane = h < 84.0;
        let mut py = y + if ultra_pane { 6.0 } else { 8.0 };

        if !ultra_pane {
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
        }

        let search_btn_w = 72.0;
        let search_h = if ultra_pane {
            32.0_f32.min((h - 12.0).max(30.0))
        } else if compact_v {
            30.0
        } else {
            36.0
        };
        draw_input_box(
            ui,
            hits,
            ButtonId::FocusModSearch,
            px,
            py,
            pw - search_btn_w - 6.0,
            search_h,
            if self.mod_search_query.is_empty() && !self.mod_search_focused {
                "Search Horizon mods"
            } else {
                ""
            },
            &self.mod_search_query,
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
                let row_h: f32 = if compact_v { 42.0 } else { 56.0 };
                let row_gap: f32 = 6.0;
                let bottom_limit = y + h - 8.0;
                let total_space = (bottom_limit - py).max(0.0);
                if total_space < row_h {
                    return;
                }
                let fit_all = ((total_space + row_gap) / (row_h + row_gap))
                    .floor()
                    .max(1.0) as usize;
                let needs_pager = results.len() > fit_all && total_space >= row_h + 34.0;
                let pager_h: f32 = if needs_pager { 34.0 } else { 0.0 };
                let avail = (total_space - pager_h).max(row_h);
                let per_page = ((avail + row_gap) / (row_h + row_gap)).floor().max(1.0) as usize;
                let total_pages = results.len().max(1).div_ceil(per_page);
                let page = self.mod_registry_page.min(total_pages - 1);
                let start = page * per_page;
                let end = (start + per_page).min(results.len());
                let rows_limit = bottom_limit - pager_h;

                for (i, m) in results.iter().enumerate().skip(start).take(end - start) {
                    if py + row_h > rows_limit + 0.5 {
                        break;
                    }
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
                        let sub = format!("{}  ·  {}", format_size(m.size), m.description);
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
                            let aw = p.measure(Face::Regular, 11.0, "Added");
                            p.text_line(
                                Face::Regular,
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

                if needs_pager && total_pages > 1 {
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
