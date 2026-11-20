//! Account onboarding (Welcome, Offline name, Ely.by, Microsoft device code) and Account Switcher.

use super::widgets::{
    Canvas, HitRect, Tone, account_provider_tag, account_short, draw_button, draw_input_box,
    draw_north_star_badge, draw_plate, draw_player_head_with_dot, palette, truncate_to_fit,
    wrap_lines,
};
use super::{ButtonId, ElyField, MobileLauncher};
use crate::launcher_backend::{Account, SignIn, valid_offline_name};
use crate::ui_theme::{self, Face, color};

impl MobileLauncher {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_welcome(
        &self,
        ui: &mut Canvas<'_, '_>,
        hits: &mut Vec<HitRect>,
        x: f32,
        y: f32,
        w: f32,
        avail_h: f32,
        show_cancel_add: bool,
    ) {
        let compact_v = avail_h < 340.0;
        let card_h = 294.0_f32.min(avail_h);
        let cy_top = y + ((avail_h - card_h) * 0.5).max(0.0).round();
        draw_plate(ui, x, cy_top, w, card_h, palette::border());

        let inner_x = x + 28.0;
        let inner_w = w - 56.0;
        let mut cy = cy_top + if compact_v { 14.0 } else { 26.0 };

        if show_cancel_add {
            draw_button(
                ui,
                hits,
                ButtonId::CancelAddAccount,
                inner_x + inner_w - 92.0,
                cy,
                92.0,
                if compact_v { 36.0 } else { 40.0 },
                Tone::Ghost,
                "<- Back",
            );
        }

        if let Some(p) = ui.paint() {
            let logo_sz = if compact_v { 36.0 } else { 44.0 };
            draw_north_star_badge(p, inner_x, cy, logo_sz);
            let title_x = inner_x + logo_sz + 14.0;
            p.text_line(
                Face::SemiBold,
                if compact_v {
                    22.0
                } else {
                    ui_theme::sizes().title
                },
                if compact_v { 24.0 } else { 28.0 },
                title_x,
                cy,
                palette::text(),
                "Sign in to Meridian",
            );
        }
        cy += if compact_v { 44.0 } else { 62.0 };

        let btn_h = if compact_v { 48.0 } else { 52.0 };
        let gap = if compact_v { 8.0 } else { 12.0 };
        draw_button(
            ui,
            hits,
            ButtonId::SignInMicrosoft,
            inner_x,
            cy,
            inner_w,
            btn_h,
            Tone::Primary,
            "Sign in with Microsoft",
        );
        cy += btn_h + gap;

        draw_button(
            ui,
            hits,
            ButtonId::OpenOfflineEntry,
            inner_x,
            cy,
            inner_w,
            btn_h,
            Tone::Secondary,
            "Play Offline",
        );
        cy += btn_h + gap;

        if self.show_more_auth {
            let half_w = (inner_w - 10.0) * 0.5;
            let sub_h = if compact_v { 42.0 } else { 48.0 };
            draw_button(
                ui,
                hits,
                ButtonId::OpenElyEntry,
                inner_x,
                cy,
                half_w,
                sub_h,
                Tone::Secondary,
                "Sign in with Ely.by",
            );
            draw_button(
                ui,
                hits,
                ButtonId::ToggleMoreAuth,
                inner_x + half_w + 10.0,
                cy,
                half_w,
                sub_h,
                Tone::Ghost,
                "Hide more options",
            );
        } else {
            draw_button(
                ui,
                hits,
                ButtonId::ToggleMoreAuth,
                inner_x,
                cy,
                160.0,
                if compact_v { 38.0 } else { 42.0 },
                Tone::Ghost,
                "More options",
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_offline_entry(
        &self,
        ui: &mut Canvas<'_, '_>,
        hits: &mut Vec<HitRect>,
        x: f32,
        y: f32,
        w: f32,
        avail_h: f32,
        col_danger: u32,
    ) {
        let valid = valid_offline_name(self.offline_name.trim());
        let feedback = self.offline_error.as_deref().or(if valid {
            None
        } else {
            Some("Enter 1-16 letters, digits, or underscores.")
        });
        // The rule is a hint until a name is turned down: red only for an error.
        let feedback_col = if self.offline_error.is_some() {
            col_danger
        } else {
            palette::muted()
        };

        let tight_v = avail_h < 185.0;
        let compact_v = avail_h < 320.0;
        let pad_x = if tight_v { 18.0 } else { 28.0 };
        let inner_x = x + pad_x;
        let inner_w = (w - pad_x * 2.0).max(200.0);

        // Ultra-tight keyboard-lifted layout: title + Back in header, input + Continue side-by-side.
        if tight_v && inner_w >= 420.0 {
            let needed_h: f32 = if feedback.is_some() { 118.0 } else { 100.0 };
            let card_h = needed_h.min(avail_h);
            let cy_top = y + ((avail_h - card_h) * 0.5).max(0.0).round();
            draw_plate(ui, x, cy_top, w, card_h, palette::border());

            let hdr_h = 30.0;
            let mut cy = cy_top + 8.0;
            if let Some(p) = ui.paint() {
                p.text_line(
                    Face::SemiBold,
                    18.0,
                    hdr_h,
                    inner_x,
                    cy,
                    palette::text(),
                    "Choose Offline Name",
                );
            }
            draw_button(
                ui,
                hits,
                ButtonId::CancelOffline,
                inner_x + inner_w - 86.0,
                cy,
                86.0,
                hdr_h,
                Tone::Secondary,
                "Back",
            );
            cy += hdr_h + 6.0;

            let row_h = (card_h - (cy - cy_top) - if feedback.is_some() { 24.0 } else { 8.0 })
                .clamp(34.0, 42.0);
            let confirm_w = 176.0_f32.min(inner_w * 0.38);
            draw_input_box(
                ui,
                hits,
                ButtonId::FocusOfflineName,
                inner_x,
                cy,
                inner_w - confirm_w - 10.0,
                row_h,
                "Player Name",
                &self.offline_name,
                false,
                true,
            );
            draw_button(
                ui,
                hits,
                ButtonId::ConfirmOffline,
                inner_x + inner_w - confirm_w,
                cy,
                confirm_w,
                row_h,
                Tone::Primary,
                "Continue Offline",
            );
            cy += row_h + 4.0;
            if let Some(msg) = feedback
                && let Some(p) = ui.paint()
            {
                let fit = truncate_to_fit(p, Face::Regular, ui_theme::small(), msg, inner_w);
                p.text_line(
                    Face::Regular,
                    ui_theme::small(),
                    16.0,
                    inner_x,
                    cy,
                    feedback_col,
                    &fit,
                );
            }
            return;
        }

        let pad_y: f32 = if compact_v { 12.0 } else { 22.0 };
        let hdr_step: f32 = if compact_v { 30.0 } else { 56.0 };
        let field_h: f32 = if compact_v { 40.0 } else { 46.0 };
        let field_gap: f32 = if compact_v { 8.0 } else { 10.0 };
        let fb_step: f32 = if feedback.is_some() {
            if compact_v { 20.0 } else { 24.0 }
        } else {
            0.0
        };
        let btn_h: f32 = if compact_v { 40.0 } else { 48.0 };
        let needed_h = pad_y * 2.0 + hdr_step + field_h + field_gap + fb_step + btn_h;
        let card_h = needed_h.min(avail_h);
        let cy_top = y + ((avail_h - card_h) * 0.5).max(0.0).round();
        draw_plate(ui, x, cy_top, w, card_h, palette::border());

        let mut cy = cy_top + pad_y;
        if let Some(p) = ui.paint() {
            p.text_line(
                Face::SemiBold,
                if compact_v {
                    20.0
                } else {
                    ui_theme::sizes().title
                },
                if compact_v { 24.0 } else { 30.0 },
                inner_x,
                cy,
                palette::text(),
                "Choose Offline Name",
            );
            if !compact_v {
                p.text_line(
                    Face::Regular,
                    ui_theme::small(),
                    18.0,
                    inner_x,
                    cy + 32.0,
                    palette::muted(),
                    "1 to 16 characters: A-Z, a-z, 0-9, or underscore.",
                );
            }
        }
        cy += hdr_step;

        draw_input_box(
            ui,
            hits,
            ButtonId::FocusOfflineName,
            inner_x,
            cy,
            inner_w,
            field_h,
            "Player Name",
            &self.offline_name,
            false,
            true,
        );
        cy += field_h + field_gap;

        if let Some(msg) = feedback {
            if let Some(p) = ui.paint() {
                let fit = truncate_to_fit(p, Face::Regular, ui_theme::small(), msg, inner_w);
                p.text_line(
                    Face::Regular,
                    ui_theme::small(),
                    18.0,
                    inner_x,
                    cy,
                    feedback_col,
                    &fit,
                );
            }
            cy += fb_step;
        }

        let eff_btn_h = btn_h.min((cy_top + card_h - cy - 6.0).max(34.0));
        let back_w = 128.0;
        draw_button(
            ui,
            hits,
            ButtonId::ConfirmOffline,
            inner_x,
            cy,
            inner_w - back_w - 12.0,
            eff_btn_h,
            Tone::Primary,
            "Continue Offline",
        );
        draw_button(
            ui,
            hits,
            ButtonId::CancelOffline,
            inner_x + inner_w - back_w,
            cy,
            back_w,
            eff_btn_h,
            Tone::Secondary,
            "Back",
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_ely_entry(
        &self,
        ui: &mut Canvas<'_, '_>,
        hits: &mut Vec<HitRect>,
        x: f32,
        y: f32,
        w: f32,
        avail_h: f32,
        col_danger: u32,
    ) {
        let compact_v = avail_h < 320.0;
        let pad_x = if avail_h < 220.0 { 18.0 } else { 28.0 };
        let inner_x = x + pad_x;
        let inner_w = (w - pad_x * 2.0).max(200.0);

        // Compact keyboard-lifted landscape: keep Username, Password, 2FA on one horizontal row
        // so every field and Sign In stay visible above the soft keyboard with generous tap targets.
        if avail_h < 280.0 && inner_w >= 540.0 {
            let pad_y: f32 = if avail_h < 180.0 { 8.0 } else { 12.0 };
            let hdr_h: f32 = 24.0;
            let gap: f32 = if avail_h < 180.0 { 6.0 } else { 8.0 };
            let field_h: f32 = if avail_h < 180.0 { 36.0 } else { 42.0 };
            let btn_h: f32 = if avail_h < 180.0 { 36.0 } else { 42.0 };
            let needed_h = pad_y * 2.0 + hdr_h + gap + field_h + gap + btn_h;
            let card_h = needed_h.min(avail_h);
            let cy_top = y + ((avail_h - card_h) * 0.5).max(0.0).round();
            draw_plate(ui, x, cy_top, w, card_h, palette::border());

            let mut cy = cy_top + pad_y;
            if let Some(p) = ui.paint() {
                p.text_line(
                    Face::SemiBold,
                    18.5,
                    hdr_h,
                    inner_x,
                    cy,
                    palette::text(),
                    "Sign in with Ely.by",
                );
                if let Some(err) = &self.ely_error {
                    let title_w = p.measure(Face::SemiBold, 18.5, "Sign in with Ely.by");
                    let err_room = (inner_w - title_w - 16.0).max(80.0);
                    let fit = truncate_to_fit(p, Face::Regular, ui_theme::small(), err, err_room);
                    let ew = p.measure(Face::Regular, ui_theme::small(), &fit);
                    p.text_line(
                        Face::Regular,
                        ui_theme::small(),
                        hdr_h,
                        inner_x + inner_w - ew,
                        cy,
                        col_danger,
                        &fit,
                    );
                }
            }
            cy += hdr_h + gap;

            let col_gap = 8.0;
            let usable = inner_w - col_gap * 2.0;
            let user_w = (usable * 0.40).round();
            let pass_w = (usable * 0.34).round();
            let otp_w = (usable - user_w - pass_w).max(110.0);
            draw_input_box(
                ui,
                hits,
                ButtonId::FocusElyUsername,
                inner_x,
                cy,
                user_w,
                field_h,
                "Username",
                &self.ely_username,
                false,
                self.ely_field == ElyField::Username,
            );
            draw_input_box(
                ui,
                hits,
                ButtonId::FocusElyPassword,
                inner_x + user_w + col_gap,
                cy,
                pass_w,
                field_h,
                "Password",
                &self.ely_password,
                true,
                self.ely_field == ElyField::Password,
            );
            draw_input_box(
                ui,
                hits,
                ButtonId::FocusElyOtp,
                inner_x + user_w + pass_w + col_gap * 2.0,
                cy,
                otp_w,
                field_h,
                "2FA (Opt)",
                &self.ely_otp,
                false,
                self.ely_field == ElyField::Otp,
            );
            cy += field_h + gap;

            let eff_btn_h = btn_h.min((cy_top + card_h - cy - 6.0).max(34.0));
            let back_w = 112.0;
            draw_button(
                ui,
                hits,
                ButtonId::ConfirmEly,
                inner_x,
                cy,
                inner_w - back_w - 10.0,
                eff_btn_h,
                Tone::Primary,
                "Sign In",
            );
            draw_button(
                ui,
                hits,
                ButtonId::CancelEly,
                inner_x + inner_w - back_w,
                cy,
                back_w,
                eff_btn_h,
                Tone::Secondary,
                "Back",
            );
            return;
        }

        let pad_y: f32 = if compact_v { 12.0 } else { 20.0 };
        let hdr_step: f32 = if compact_v { 28.0 } else { 38.0 };
        let field_h: f32 = if compact_v { 38.0 } else { 44.0 };
        let gap: f32 = if compact_v { 8.0 } else { 10.0 };
        let err_step: f32 = if self.ely_error.is_some() {
            if compact_v { 20.0 } else { 24.0 }
        } else {
            0.0
        };
        let btn_h: f32 = if compact_v { 42.0 } else { 48.0 };
        let needed_h = pad_y * 2.0 + hdr_step + (field_h + gap) * 2.0 + err_step + btn_h;
        let card_h = needed_h.min(avail_h);
        let cy_top = y + ((avail_h - card_h) * 0.5).max(0.0).round();
        draw_plate(ui, x, cy_top, w, card_h, palette::border());

        let mut cy = cy_top + pad_y;
        if let Some(p) = ui.paint() {
            p.text_line(
                Face::SemiBold,
                if compact_v {
                    20.0
                } else {
                    ui_theme::sizes().title
                },
                if compact_v { 24.0 } else { 30.0 },
                inner_x,
                cy,
                palette::text(),
                "Sign in with Ely.by",
            );
        }
        cy += hdr_step;

        draw_input_box(
            ui,
            hits,
            ButtonId::FocusElyUsername,
            inner_x,
            cy,
            inner_w,
            field_h,
            "Username / Email",
            &self.ely_username,
            false,
            self.ely_field == ElyField::Username,
        );
        cy += field_h + gap;

        let half_w = (inner_w - 10.0) * 0.5;
        draw_input_box(
            ui,
            hits,
            ButtonId::FocusElyPassword,
            inner_x,
            cy,
            half_w,
            field_h,
            "Password",
            &self.ely_password,
            true,
            self.ely_field == ElyField::Password,
        );
        draw_input_box(
            ui,
            hits,
            ButtonId::FocusElyOtp,
            inner_x + half_w + 10.0,
            cy,
            half_w,
            field_h,
            "2FA Code (Optional)",
            &self.ely_otp,
            false,
            self.ely_field == ElyField::Otp,
        );
        cy += field_h + gap;

        if let Some(err) = &self.ely_error {
            if let Some(p) = ui.paint() {
                let fit = truncate_to_fit(p, Face::Regular, ui_theme::small(), err, inner_w);
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

        let eff_btn_h = btn_h.min((cy_top + card_h - cy - 6.0).max(34.0));
        let back_w = 120.0;
        draw_button(
            ui,
            hits,
            ButtonId::ConfirmEly,
            inner_x,
            cy,
            inner_w - back_w - 10.0,
            eff_btn_h,
            Tone::Primary,
            "Sign In",
        );
        draw_button(
            ui,
            hits,
            ButtonId::CancelEly,
            inner_x + inner_w - back_w,
            cy,
            back_w,
            eff_btn_h,
            Tone::Secondary,
            "Back",
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_signin_progress(
        &self,
        ui: &mut Canvas<'_, '_>,
        hits: &mut Vec<HitRect>,
        x: f32,
        y: f32,
        w: f32,
        avail_h: f32,
        sign_in: &SignIn,
        col_warn: u32,
    ) {
        let compact_v = avail_h < 320.0;
        let card_h = 310.0_f32.min(avail_h);
        let cy_top = y + ((avail_h - card_h) * 0.5).max(0.0).round();
        draw_plate(ui, x, cy_top, w, card_h, palette::border());

        let inner_x = x + 28.0;
        let inner_w = w - 56.0;
        let mut cy = cy_top + if compact_v { 14.0 } else { 24.0 };

        match sign_in {
            SignIn::Starting => {
                if let Some(p) = ui.paint() {
                    p.text_line(
                        Face::SemiBold,
                        if compact_v {
                            22.0
                        } else {
                            ui_theme::sizes().title
                        },
                        32.0,
                        inner_x,
                        cy,
                        palette::text(),
                        "Connecting...",
                    );
                }
                cy += 96.0;
                draw_button(
                    ui,
                    hits,
                    ButtonId::CancelSignIn,
                    inner_x,
                    cy,
                    160.0,
                    48.0,
                    Tone::Secondary,
                    "Cancel",
                );
            }
            SignIn::Code { url, code } => {
                if let Some(p) = ui.paint() {
                    p.text_line(
                        Face::SemiBold,
                        if compact_v {
                            20.0
                        } else {
                            ui_theme::sizes().title
                        },
                        if compact_v { 26.0 } else { 32.0 },
                        inner_x,
                        cy,
                        palette::text(),
                        "Enter this code in your browser",
                    );
                }
                cy += if compact_v { 30.0 } else { 38.0 };

                let box_h = if compact_v { 54.0 } else { 64.0 };
                if let Some(p) = ui.paint() {
                    p.rounded(inner_x, cy, inner_w, box_h, 10.0, palette::surface());
                    p.border(inner_x, cy, inner_w, box_h, 10.0, palette::accent_star());
                    let cw = p.measure(Face::Mono, 28.0, code);
                    p.text_line(
                        Face::Mono,
                        28.0,
                        box_h,
                        inner_x + ((inner_w - cw) * 0.5).round(),
                        cy,
                        palette::text(),
                        code,
                    );
                }
                cy += box_h + if compact_v { 8.0 } else { 12.0 };

                if let Some(p) = ui.paint() {
                    let msg = self.status_note.as_deref().unwrap_or(url.as_str());
                    let fit = truncate_to_fit(p, Face::Mono, ui_theme::small(), msg, inner_w);
                    p.text_line(
                        Face::Mono,
                        ui_theme::small(),
                        18.0,
                        inner_x,
                        cy,
                        col_warn,
                        &fit,
                    );
                }
                cy += if compact_v { 22.0 } else { 26.0 };

                let btn_h = 48.0;
                let cancel_w = 124.0;
                draw_button(
                    ui,
                    hits,
                    ButtonId::OpenBrowserUrl,
                    inner_x,
                    cy,
                    inner_w - cancel_w - 10.0,
                    btn_h,
                    Tone::Primary,
                    "Open microsoft.com/link",
                );
                draw_button(
                    ui,
                    hits,
                    ButtonId::CancelSignIn,
                    inner_x + inner_w - cancel_w,
                    cy,
                    cancel_w,
                    btn_h,
                    Tone::Secondary,
                    "Cancel",
                );
            }
            SignIn::Failed(err) => {
                if let Some(p) = ui.paint() {
                    p.text_line(
                        Face::SemiBold,
                        if compact_v {
                            20.0
                        } else {
                            ui_theme::sizes().title
                        },
                        30.0,
                        inner_x,
                        cy,
                        palette::text(),
                        "Sign-in failed",
                    );
                    cy += 38.0;
                    for (i, line) in
                        wrap_lines(p, Face::Regular, ui_theme::sizes().ui, err, inner_w)
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
                let btn_y = cy_top + card_h - 62.0;
                let half_w = (inner_w - 12.0) * 0.5;
                draw_button(
                    ui,
                    hits,
                    ButtonId::RetrySignIn,
                    inner_x,
                    btn_y,
                    half_w,
                    48.0,
                    Tone::Primary,
                    "Try Again",
                );
                draw_button(
                    ui,
                    hits,
                    ButtonId::CancelSignIn,
                    inner_x + half_w + 12.0,
                    btn_y,
                    half_w,
                    48.0,
                    Tone::Secondary,
                    "Back",
                );
            }
            SignIn::Idle | SignIn::Done(_) => {}
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_account_switcher(
        &self,
        ui: &mut Canvas<'_, '_>,
        hits: &mut Vec<HitRect>,
        x: f32,
        y: f32,
        w: f32,
        avail_h: f32,
        accounts: &[Account],
        active: Option<usize>,
        col_success: u32,
    ) {
        let compact_v = avail_h < 300.0;
        draw_plate(ui, x, y, w, avail_h, palette::border());

        let inner_x = x + 20.0;
        let inner_w = w - 40.0;
        let mut cy = y + if compact_v { 10.0 } else { 18.0 };
        let hdr_h = if compact_v { 34.0 } else { 40.0 };

        if let Some(p) = ui.paint() {
            p.text_line(
                Face::SemiBold,
                if compact_v { 19.0 } else { 22.0 },
                hdr_h,
                inner_x,
                cy,
                palette::text(),
                "Accounts",
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
        draw_button(
            ui,
            hits,
            ButtonId::AddAccount,
            inner_x + inner_w - 156.0,
            cy,
            156.0,
            hdr_h,
            Tone::Primary,
            "+ Add account",
        );
        cy += hdr_h + if compact_v { 10.0 } else { 20.0 };

        let bottom_pad = if compact_v { 10.0 } else { 16.0 };
        let item_h = if compact_v { 54.0 } else { 66.0 };
        let gap = 8.0;
        let space = (y + avail_h - bottom_pad - cy).max(item_h);
        let fit_all = ((space + gap) / (item_h + gap)).floor().max(1.0) as usize;
        let needs_pager = accounts.len() > fit_all;
        let per_page = if needs_pager {
            ((space - 38.0 + gap) / (item_h + gap)).floor().max(1.0) as usize
        } else {
            accounts.len().max(1)
        };
        let total_pages = accounts.len().max(1).div_ceil(per_page);
        let page = self.account_page.min(total_pages - 1);
        let start = page * per_page;
        let end = (start + per_page).min(accounts.len());

        for (idx, acct) in accounts.iter().enumerate().skip(start).take(end - start) {
            let is_active = active == Some(idx);
            if let Some(p) = ui.paint() {
                p.rounded(
                    inner_x,
                    cy,
                    inner_w,
                    item_h,
                    10.0,
                    if is_active {
                        color::with_alpha(palette::accent(), 0.16)
                    } else {
                        palette::surface()
                    },
                );
                p.border(
                    inner_x,
                    cy,
                    inner_w,
                    item_h,
                    10.0,
                    if is_active {
                        palette::accent_star()
                    } else {
                        palette::border()
                    },
                );
                let head_sz = (item_h - 16.0).clamp(32.0, 44.0);
                let head_y = cy + ((item_h - head_sz) * 0.5).round();
                let head = self.backend.player_head(acct);
                let short = account_short(acct);
                let dot_col = match acct {
                    Account::Offline { .. } => palette::accent_star(),
                    Account::Microsoft { .. } | Account::Ely { .. } => col_success,
                };
                draw_player_head_with_dot(
                    p,
                    inner_x + 12.0,
                    head_y,
                    head_sz,
                    head.as_ref(),
                    short,
                    dot_col,
                );
                let tx = inner_x + head_sz + 24.0;
                p.text_line(
                    Face::SemiBold,
                    17.0,
                    22.0,
                    tx,
                    cy + if compact_v { 8.0 } else { 12.0 },
                    palette::text(),
                    short,
                );
                p.text_line(
                    Face::Mono,
                    ui_theme::small(),
                    18.0,
                    tx,
                    cy + if compact_v { 28.0 } else { 36.0 },
                    palette::muted(),
                    account_provider_tag(acct),
                );
            }

            let act_h = if compact_v { 36.0 } else { 42.0 };
            let ay = cy + ((item_h - act_h) * 0.5).round();
            let rem_w = 96.0;
            draw_button(
                ui,
                hits,
                ButtonId::RemoveAccount(idx),
                inner_x + inner_w - 10.0 - rem_w,
                ay,
                rem_w,
                act_h,
                Tone::Danger,
                "Sign out",
            );
            if is_active {
                if let Some(p) = ui.paint() {
                    let aw = p.measure(Face::Mono, ui_theme::small(), "ACTIVE");
                    p.text_line(
                        Face::Mono,
                        ui_theme::small(),
                        act_h,
                        inner_x + inner_w - 22.0 - rem_w - aw,
                        ay,
                        col_success,
                        "ACTIVE",
                    );
                }
            } else {
                draw_button(
                    ui,
                    hits,
                    ButtonId::SwitchAccount(idx),
                    inner_x + inner_w - 18.0 - rem_w - 92.0,
                    ay,
                    92.0,
                    act_h,
                    Tone::Secondary,
                    "Use",
                );
            }
            cy += item_h + gap;
        }

        if needs_pager {
            let py = y + avail_h - bottom_pad - 34.0;
            if page > 0 {
                draw_button(
                    ui,
                    hits,
                    ButtonId::AccountsPrevPage,
                    inner_x,
                    py,
                    82.0,
                    34.0,
                    Tone::Secondary,
                    "Prev",
                );
            }
            if page + 1 < total_pages {
                draw_button(
                    ui,
                    hits,
                    ButtonId::AccountsNextPage,
                    inner_x + inner_w - 82.0,
                    py,
                    82.0,
                    34.0,
                    Tone::Secondary,
                    "Next",
                );
            }
        }
    }
}
