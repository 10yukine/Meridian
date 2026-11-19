//! Skins preview and Launcher Settings views.

use super::widgets::{
    Canvas, HitRect, Tone, account_label, account_provider_tag, account_short, count_label,
    draw_button, draw_plate, draw_player_head, format_size, palette, stage_fraction, stage_name,
    truncate_to_fit, wrap_lines,
};
use super::{ButtonId, MobileLauncher};
use crate::launcher_backend::{Account, Profile, Setup};
use crate::ui_theme::{self, Face, color};

impl MobileLauncher {
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
    ) {
        let compact_v = avail_h < 300.0;
        draw_plate(ui, x, y, w, avail_h, palette::border());

        let ix = x + 22.0;
        let iw = w - 44.0;
        let mut cy = y + if compact_v { 12.0 } else { 22.0 };

        if let Some(p) = ui.paint() {
            p.text_line(
                Face::SemiBold,
                if compact_v { 19.0 } else { 22.0 },
                28.0,
                ix,
                cy,
                palette::text(),
                "Skins & Capes",
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

        let preview_sz = if compact_v { 120.0 } else { 180.0 };
        let head_sz = if compact_v { 72.0 } else { 104.0 };
        let head = account.as_ref().and_then(|a| self.backend.player_head(a));
        let name = account.as_ref().map_or("Player", account_short);
        let provider = account
            .as_ref()
            .map_or("Not signed in", account_provider_tag);

        if let Some(p) = ui.paint() {
            p.rounded(
                ix - 3.0,
                cy - 3.0,
                preview_sz + 6.0,
                preview_sz + 6.0,
                17.0,
                color::with_alpha(palette::accent(), 0.10),
            );
            p.rounded(ix, cy, preview_sz, preview_sz, 14.0, palette::surface());
            p.border(
                ix,
                cy,
                preview_sz,
                preview_sz,
                14.0,
                color::with_alpha(palette::accent_star(), 0.55),
            );
            draw_player_head(
                p,
                ix + ((preview_sz - head_sz) * 0.5).round(),
                cy + ((preview_sz - head_sz) * 0.5).round(),
                head_sz,
                head.as_ref(),
                name,
            );

            let tx = ix + preview_sz + 24.0;
            let tw = (iw - preview_sz - 24.0).max(120.0);
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
                Face::Mono,
                ui_theme::small(),
                18.0,
                tx,
                cy + 32.0,
                palette::accent_star(),
                provider,
            );
            let note = "Your active account face is loaded automatically. Custom Steve/Alex skin uploads and cape selection are coming in a future update.";
            let max_lines = if compact_v { 2 } else { 4 };
            for (i, line) in wrap_lines(p, Face::Regular, ui_theme::sizes().ui, note, tw)
                .into_iter()
                .take(max_lines)
                .enumerate()
            {
                p.text_line(
                    Face::Regular,
                    ui_theme::sizes().ui,
                    20.0,
                    tx,
                    cy + 58.0 + i as f32 * 20.0,
                    palette::muted(),
                    &line,
                );
            }
        }

        let btn_x = ix + preview_sz + 24.0;
        let btn_y = if compact_v {
            (cy + 106.0).min(y + avail_h - 48.0)
        } else {
            (cy + preview_sz - 44.0).min(y + avail_h - 56.0)
        };
        draw_button(
            ui,
            hits,
            ButtonId::OpenAccountSwitcher,
            btn_x,
            btn_y,
            170.0,
            if compact_v { 38.0 } else { 44.0 },
            Tone::Secondary,
            "Switch Account",
        );
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
                Tone::Ghost,
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

        // Interactive Launcher Font setting (Modern Geist by default; switchable to Minecraft
        // pixel font and persisted in horizon.txt via ui_theme::preference).
        let pixel_font = ui_theme::pixel_font();
        let pack_font_ready = ui.ro().pack_font_available();
        let font_btn_w = if compact_v { 156.0 } else { 184.0 };
        let font_btn_h = (row_h - 6.0).max(26.0);
        let font_btn_y = cy + ((row_h - font_btn_h) * 0.5).round();
        let font_btn_x = inner_x + inner_w - 8.0 - font_btn_w;
        if let Some(p) = ui.paint() {
            p.rounded(inner_x, cy, inner_w, row_h, 8.0, palette::surface());
            p.text_line(
                Face::Medium,
                ui_theme::sizes().ui,
                row_h,
                inner_x + 14.0,
                cy,
                palette::text(),
                "Launcher Font",
            );
            if inner_w >= 500.0 {
                let hint = if pixel_font && pack_font_ready {
                    "Minecraft bitmap font"
                } else if pixel_font {
                    "Uses Geist until Minecraft 26.3 is installed"
                } else if pack_font_ready {
                    "Clean modern UI font"
                } else {
                    "Modern UI font (Pixel unlocks after 26.3 install)"
                };
                let hw = p.measure(Face::Regular, ui_theme::small(), hint);
                p.text_line(
                    Face::Regular,
                    ui_theme::small(),
                    row_h,
                    font_btn_x - 14.0 - hw,
                    cy,
                    palette::muted(),
                    hint,
                );
            }
        }
        let font_btn_label = if pixel_font && pack_font_ready {
            "Pixel (Minecraft)"
        } else if pixel_font {
            "Pixel (after 26.3)"
        } else {
            "Modern (Geist)"
        };
        draw_button(
            ui,
            hits,
            ButtonId::ToggleLauncherFont,
            font_btn_x,
            font_btn_y,
            font_btn_w,
            font_btn_h,
            if pixel_font {
                Tone::Banner
            } else {
                Tone::Secondary
            },
            font_btn_label,
        );
        cy += row_step;

        let total_profile_bytes: u64 = profiles.iter().map(|p| p.size).sum();
        let rows = [
            (
                "Account",
                account
                    .as_ref()
                    .map_or_else(|| "Not signed in".to_owned(), account_label),
                palette::body(),
            ),
            (
                "Minecraft 26.3",
                match setup {
                    Setup::Needed => "Not installed".to_owned(),
                    Setup::Running(p) => {
                        format!(
                            "{} ({:.0}%)",
                            stage_name(p.stage),
                            stage_fraction(p) * 100.0
                        )
                    }
                    Setup::Failed { stage, .. } => format!("Failed at {}", stage_name(*stage)),
                    Setup::Ready => "Installed & ready".to_owned(),
                },
                match setup {
                    Setup::Ready => col_success,
                    Setup::Failed { .. } => col_danger,
                    Setup::Running(_) => palette::accent_star(),
                    Setup::Needed => palette::muted(),
                },
            ),
            (
                "Profiles Storage",
                format!(
                    "{}  |  {}",
                    count_label(profiles.len(), "profile", "profiles"),
                    format_size(total_profile_bytes)
                ),
                palette::body(),
            ),
            (
                "Touch Controls",
                "MCPE default (Meridian art fallback)".to_owned(),
                palette::muted(),
            ),
        ];

        for (label, value, val_col) in rows {
            if cy + row_h > cy_top + card_h - 6.0 {
                break;
            }
            if let Some(p) = ui.paint() {
                p.rounded(inner_x, cy, inner_w, row_h, 8.0, palette::surface());
                p.text_line(
                    Face::Medium,
                    ui_theme::sizes().ui,
                    row_h,
                    inner_x + 14.0,
                    cy,
                    palette::text(),
                    label,
                );
                let fit_val =
                    truncate_to_fit(p, Face::Mono, ui_theme::small(), &value, inner_w * 0.65);
                let vw = p.measure(Face::Mono, ui_theme::small(), &fit_val);
                p.text_line(
                    Face::Mono,
                    ui_theme::small(),
                    row_h,
                    inner_x + inner_w - 14.0 - vw,
                    cy,
                    val_col,
                    &fit_val,
                );
            }
            cy += row_step;
        }
    }
}
