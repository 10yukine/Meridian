//! Four-stage game installation screen (`Check -> Download -> Verify -> Prepare`) and failure retry.

use super::widgets::{
    Canvas, HitRect, Tone, draw_button, draw_plate, format_progress_stats, palette, stage_fraction,
    stage_heading, stage_index, stage_name, truncate_to_fit, wrap_lines,
};
use super::{ButtonId, MobileLauncher};
use crate::launcher_backend::{Progress, Stage};
use crate::ui_theme::{self, Face};

impl MobileLauncher {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_setup_running(
        &self,
        ui: &mut Canvas<'_, '_>,
        hits: &mut Vec<HitRect>,
        x: f32,
        y: f32,
        w: f32,
        avail_h: f32,
        progress: &Progress,
        col_success: u32,
    ) {
        let compact_v = avail_h < 300.0;
        let card_h = 340.0_f32.min(avail_h);
        let cy_top = y + ((avail_h - card_h) * 0.5).max(0.0).round();
        draw_plate(ui, x, cy_top, w, card_h, palette::border());

        let inner_x = x + 24.0;
        let inner_w = w - 48.0;
        let mut cy = cy_top + if compact_v { 10.0 } else { 22.0 };

        if let Some(p) = ui.paint() {
            p.text_line(
                Face::SemiBold,
                if compact_v {
                    19.0
                } else {
                    ui_theme::sizes().title
                },
                if compact_v { 24.0 } else { 30.0 },
                inner_x,
                cy,
                palette::text(),
                stage_heading(progress.stage),
            );
        }
        cy += if compact_v { 32.0 } else { 44.0 };

        let current_idx = stage_index(progress.stage);
        let pill_gap = 8.0;
        let pill_w = (inner_w - pill_gap * 3.0) / 4.0;
        let pill_h = if compact_v { 28.0 } else { 36.0 };
        if let Some(p) = ui.paint() {
            for (i, label) in ["Check", "Download", "Verify", "Prepare"]
                .iter()
                .enumerate()
            {
                let px = inner_x + i as f32 * (pill_w + pill_gap);
                // Done in green, the current one on a plate, the rest muted.
                let (bg, bdr, ink) = if i < current_idx {
                    (0, palette::border_subtle(), col_success)
                } else if i == current_idx {
                    (
                        palette::surface_hover(),
                        palette::accent_star(),
                        palette::text(),
                    )
                } else {
                    (0, palette::border_subtle(), palette::muted())
                };
                if bg != 0 {
                    p.rounded(px, cy, pill_w, pill_h, 8.0, bg);
                }
                p.border(px, cy, pill_w, pill_h, 8.0, bdr);
                let tw = p.measure(Face::Medium, ui_theme::small(), label);
                p.text_line(
                    Face::Medium,
                    ui_theme::small(),
                    pill_h,
                    px + ((pill_w - tw) * 0.5).round(),
                    cy,
                    ink,
                    label,
                );
            }
        }
        cy += pill_h + if compact_v { 10.0 } else { 16.0 };

        let frac = stage_fraction(progress);
        let bar_h = if compact_v { 12.0 } else { 14.0 };
        if let Some(p) = ui.paint() {
            p.rounded(inner_x, cy, inner_w, bar_h, 6.0, palette::bg_outer());
            p.border(inner_x, cy, inner_w, bar_h, 6.0, palette::border());
            if frac > 0.0 {
                p.rounded(
                    inner_x,
                    cy,
                    (inner_w * frac).max(12.0),
                    bar_h,
                    6.0,
                    palette::accent(),
                );
            }
            cy += bar_h + 8.0;
            let (left_stat, right_stat) = format_progress_stats(progress, frac);
            p.text_line(
                Face::Mono,
                ui_theme::small(),
                18.0,
                inner_x,
                cy,
                palette::body(),
                &left_stat,
            );
            let rw = p.measure(Face::Mono, ui_theme::small(), &right_stat);
            p.text_line(
                Face::Mono,
                ui_theme::small(),
                18.0,
                inner_x + inner_w - rw,
                cy,
                palette::accent_star(),
                &right_stat,
            );
            cy += 20.0;
            if let Some(note) = &progress.note {
                let fit = truncate_to_fit(p, Face::Regular, ui_theme::small(), note, inner_w);
                p.text_line(
                    Face::Regular,
                    ui_theme::small(),
                    18.0,
                    inner_x,
                    cy,
                    palette::muted(),
                    &fit,
                );
            }
        }

        let btn_h = if compact_v { 42.0 } else { 46.0 };
        let btn_y = cy_top + card_h - btn_h - if compact_v { 10.0 } else { 16.0 };
        let half_w = (inner_w - 12.0) * 0.5;
        draw_button(
            ui,
            hits,
            ButtonId::GoHome,
            inner_x,
            btn_y,
            half_w,
            btn_h,
            Tone::Secondary,
            "Hide to Home",
        );
        draw_button(
            ui,
            hits,
            ButtonId::CancelSetup,
            inner_x + half_w + 12.0,
            btn_y,
            half_w,
            btn_h,
            Tone::Danger,
            "Cancel",
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_setup_failed(
        &self,
        ui: &mut Canvas<'_, '_>,
        hits: &mut Vec<HitRect>,
        x: f32,
        y: f32,
        w: f32,
        avail_h: f32,
        stage: Stage,
        error: &str,
        col_danger: u32,
    ) {
        let compact_v = avail_h < 300.0;
        let card_h = 290.0_f32.min(avail_h);
        let cy_top = y + ((avail_h - card_h) * 0.5).max(0.0).round();
        draw_plate(ui, x, cy_top, w, card_h, col_danger);

        let inner_x = x + 24.0;
        let inner_w = w - 48.0;
        let mut cy = cy_top + if compact_v { 12.0 } else { 22.0 };

        if let Some(p) = ui.paint() {
            p.text_line(
                Face::SemiBold,
                if compact_v {
                    19.0
                } else {
                    ui_theme::sizes().title
                },
                28.0,
                inner_x,
                cy,
                palette::text(),
                &format!("Install stopped at {}", stage_name(stage)),
            );
            cy += if compact_v { 30.0 } else { 36.0 };
            for (i, line) in wrap_lines(p, Face::Regular, ui_theme::sizes().ui, error, inner_w)
                .into_iter()
                .take(3)
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
            ButtonId::RetrySetup,
            inner_x,
            btn_y,
            half_w,
            btn_h,
            Tone::Primary,
            "Retry Install",
        );
        draw_button(
            ui,
            hits,
            ButtonId::GoHome,
            inner_x + half_w + 12.0,
            btn_y,
            half_w,
            btn_h,
            Tone::Secondary,
            "Back to Home",
        );
    }
}
