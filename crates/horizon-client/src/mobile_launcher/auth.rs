//! Account onboarding (Welcome, Offline name, Ely.by, Microsoft device code) and Account Switcher.

use super::widgets::{
    Canvas, HitRect, Tone, account_provider_tag, account_short, draw_button, draw_input_box,
    draw_plate, draw_player_head, palette, truncate_to_fit, wrap_lines,
};
use super::{ButtonId, ElyField, MobileLauncher};
use crate::launcher_backend::{
    Account, NEEDS_OWNER, NOT_APPROVED, SignIn, i18n::tr, valid_offline_name,
};
use crate::ui_theme::{self, Face, color};

fn draw_card_accent(ui: &mut Canvas<'_, '_>, x: f32, y: f32, w: f32, col: u32) {
    if let Some(p) = ui.paint() {
        p.rounded(x + 14.0, y, (w - 28.0).max(0.0), 2.0, 1.0, col);
    }
}

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
        let allowed = self.backend.game_allowed();
        let compact_v = avail_h < 340.0;
        let pad_top = if compact_v { 14.0_f32 } else { 26.0 };
        let title_block = if compact_v { 44.0 } else { 62.0 };
        let btn_h = if compact_v { 48.0 } else { 52.0 };
        let gap = if compact_v { 8.0 } else { 12.0 };
        let note_h = if compact_v { 22.0 } else { 26.0 };
        let more_h = if self.show_more_auth {
            if compact_v { 42.0 } else { 48.0 }
        } else if compact_v {
            38.0
        } else {
            42.0
        };
        let body_h = if allowed {
            btn_h + gap + btn_h + gap + more_h
        } else {
            btn_h + gap + note_h
        };
        let card_h = (pad_top + title_block + body_h + pad_top).min(avail_h);
        let cy_top = y + ((avail_h - card_h) * 0.5).max(0.0).round();
        draw_plate(ui, x, cy_top, w, card_h, palette::border());

        let inner_x = x + 28.0;
        let inner_w = w - 56.0;
        let mut cy = cy_top + pad_top;

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
                tr("Back"),
            );
        }

        if let Some(p) = ui.paint() {
            // The header's icon is the app's mark; the card shows only its title (user, 2026-11-24).
            let title_x = inner_x;
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
                tr("Sign in to Meridian"),
            );
        }
        cy += title_block;

        draw_button(
            ui,
            hits,
            ButtonId::SignInMicrosoft,
            inner_x,
            cy,
            inner_w,
            btn_h,
            Tone::Primary,
            tr("Sign in with Microsoft"),
        );
        cy += btn_h + gap;

        if !allowed {
            // A release build without a game owner on the phone: show Microsoft sign-in first and
            // say why in one line; offline and Ely.by appear once an owner has signed in.
            if let Some(p) = ui.paint() {
                let reason = tr(NEEDS_OWNER);
                let sz = p.fit(Face::Regular, ui_theme::small(), reason, inner_w);
                let fit = truncate_to_fit(p, Face::Regular, sz, reason, inner_w);
                let rw = p.measure(Face::Regular, sz, &fit);
                p.text_line(
                    Face::Regular,
                    sz,
                    note_h,
                    inner_x + ((inner_w - rw) * 0.5).max(0.0).round(),
                    cy,
                    palette::muted(),
                    &fit,
                );
            }
            return;
        }

        draw_button(
            ui,
            hits,
            ButtonId::OpenOfflineEntry,
            inner_x,
            cy,
            inner_w,
            btn_h,
            Tone::Secondary,
            tr("Play Offline"),
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
                tr("Sign in with Ely.by"),
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
                tr("Hide more options"),
            );
        } else {
            draw_button(
                ui,
                hits,
                ButtonId::ToggleMoreAuth,
                inner_x,
                cy,
                // Under the two ways in, as wide, so its label sits centred below theirs.
                inner_w,
                if compact_v { 38.0 } else { 42.0 },
                Tone::Ghost,
                tr("More options"),
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
        let card_w = w.min(520.0);
        let card_x = x + ((w - card_w) * 0.5).round();
        let s = ((avail_h - 144.0) / 100.0).clamp(0.0, 1.0);
        let pad_x = (22.0 + 6.0 * s).round();
        let pad_y = (8.0 + 16.0 * s).round();
        let title_h = (22.0 + 6.0 * s).round();
        let title_sz = 17.5 + 4.5 * s;
        let sub_h = (16.0 + 4.0 * s).round();
        let gap1 = (6.0 + 6.0 * s).round();
        let field_h = (34.0 + 12.0 * s).round();
        let gap2 = (8.0 + 6.0 * s).round();
        let btn_h = (34.0 + 12.0 * s).round();
        let inner_x = card_x + pad_x;
        let inner_w = (card_w - pad_x * 2.0).max(200.0);

        let needed_h = pad_y * 2.0 + title_h + sub_h + gap1 + field_h + gap2 + btn_h;
        let card_h = needed_h.min(avail_h);
        let cy_top = y + ((avail_h - card_h) * 0.5).max(0.0).round();
        draw_plate(ui, card_x, cy_top, card_w, card_h, palette::border());
        draw_card_accent(ui, card_x, cy_top, card_w, palette::accent());

        let mut cy = cy_top + pad_y;
        if let Some(p) = ui.paint() {
            p.text_line(
                Face::SemiBold,
                title_sz,
                title_h,
                inner_x,
                cy,
                palette::text(),
                tr("Choose Offline Name"),
            );
        }
        cy += title_h;

        let (sub_msg, sub_col) = if let Some(err) = self.offline_error.as_deref() {
            (err, col_danger)
        } else if !valid && !self.offline_name.is_empty() {
            (
                tr("Enter 1-16 letters, digits, or underscores."),
                col_danger,
            )
        } else {
            (
                tr("1 to 16 characters: A-Z, a-z, 0-9, or underscore."),
                palette::muted(),
            )
        };
        if let Some(p) = ui.paint() {
            let fit = truncate_to_fit(p, Face::Regular, ui_theme::small(), sub_msg, inner_w);
            p.text_line(
                Face::Regular,
                ui_theme::small(),
                sub_h,
                inner_x,
                cy,
                sub_col,
                &fit,
            );
        }
        cy += sub_h + gap1;

        draw_input_box(
            ui,
            hits,
            ButtonId::FocusOfflineName,
            inner_x,
            cy,
            inner_w,
            field_h,
            tr("Player Name"),
            &self.offline_name,
            false,
            true,
        );
        cy += field_h + gap2;

        let eff_btn_h = btn_h.min((cy_top + card_h - cy - 4.0).max(32.0));
        let back_w = 120.0_f32.min(inner_w * 0.30);
        draw_button(
            ui,
            hits,
            ButtonId::ConfirmOffline,
            inner_x,
            cy,
            inner_w - back_w - 10.0,
            eff_btn_h,
            Tone::Primary,
            tr("Continue Offline"),
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
            tr("Back"),
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
        let card_w = w.min(540.0);
        let card_x = x + ((w - card_w) * 0.5).round();
        let s = ((avail_h - 150.0) / 110.0).clamp(0.0, 1.0);
        let pad_x = (20.0 + 8.0 * s).round();
        let pad_y = (6.0 + 16.0 * s).round();
        let hdr_h = (20.0 + 8.0 * s).round();
        let gap = (5.0 + 5.0 * s).round();
        let field_h = (32.0 + 12.0 * s).round();
        let btn_h = (32.0 + 14.0 * s).round();
        let inner_x = card_x + pad_x;
        let inner_w = (card_w - pad_x * 2.0).max(200.0);

        let needed_h = pad_y * 2.0 + hdr_h + gap + (field_h + gap) * 2.0 + btn_h;
        let card_h = needed_h.min(avail_h);
        let cy_top = y + ((avail_h - card_h) * 0.5).max(0.0).round();
        draw_plate(ui, card_x, cy_top, card_w, card_h, palette::border());
        draw_card_accent(ui, card_x, cy_top, card_w, palette::accent());

        let mut cy = cy_top + pad_y;
        if let Some(p) = ui.paint() {
            let ely_title = tr("Sign in with Ely.by");
            let title_sz = 17.5 + 4.5 * s;
            p.text_line(
                Face::SemiBold,
                title_sz,
                hdr_h,
                inner_x,
                cy,
                palette::text(),
                ely_title,
            );
            if let Some(err) = &self.ely_error {
                let title_w = p.measure(Face::SemiBold, title_sz, ely_title);
                let err_room = (inner_w - title_w - 14.0).max(80.0);
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

        draw_input_box(
            ui,
            hits,
            ButtonId::FocusElyUsername,
            inner_x,
            cy,
            inner_w,
            field_h,
            tr("Username / Email"),
            &self.ely_username,
            false,
            self.ely_field == ElyField::Username,
        );
        cy += field_h + gap;

        let half_w = (inner_w - 10.0) * 0.5;
        let otp_label = if half_w < 200.0 {
            tr("2FA (Opt)")
        } else {
            tr("2FA Code (Optional)")
        };
        draw_input_box(
            ui,
            hits,
            ButtonId::FocusElyPassword,
            inner_x,
            cy,
            half_w,
            field_h,
            tr("Password"),
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
            otp_label,
            &self.ely_otp,
            false,
            self.ely_field == ElyField::Otp,
        );
        cy += field_h + gap;

        let eff_btn_h = btn_h.min((cy_top + card_h - cy - 4.0).max(32.0));
        let back_w = 116.0_f32.min(inner_w * 0.30);
        draw_button(
            ui,
            hits,
            ButtonId::ConfirmEly,
            inner_x,
            cy,
            inner_w - back_w - 10.0,
            eff_btn_h,
            Tone::Primary,
            tr("Sign In"),
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
            tr("Back"),
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
    ) {
        let compact_v = avail_h < 320.0;
        // Until Mojang approves the app, Microsoft sign-in can't work: the offline account is
        // the way in.
        let not_approved = matches!(sign_in, SignIn::Failed(e) if e == NOT_APPROVED);
        let (title, body) = match sign_in {
            SignIn::Failed(_) if not_approved => (
                tr("Microsoft sign-in isn't available yet"),
                tr(
                    "Mojang hasn't approved Meridian's sign-in yet. Play with an offline account until then.",
                ),
            ),
            SignIn::Failed(err) => (tr("Sign-in failed"), err.as_str()),
            _ => ("", ""),
        };

        match sign_in {
            SignIn::Starting => {
                let card_w = w.min(440.0);
                let card_x = x + ((w - card_w) * 0.5).round();
                let pad = if compact_v { 18.0 } else { 28.0 };
                let inner_x = card_x + pad;
                let inner_w = (card_w - pad * 2.0).max(160.0);
                let dots_h = 16.0;
                let gap1 = if compact_v { 8.0 } else { 10.0 };
                let title_h = 28.0;
                let sub_h = 20.0;
                let gap2 = if compact_v { 14.0 } else { 18.0 };
                let btn_h = if compact_v { 40.0 } else { 44.0 };
                let card_h =
                    (pad * 2.0 + dots_h + gap1 + title_h + sub_h + gap2 + btn_h).min(avail_h);
                let cy_top = y + ((avail_h - card_h) * 0.5).max(0.0).round();
                draw_plate(ui, card_x, cy_top, card_w, card_h, palette::border());
                draw_card_accent(ui, card_x, cy_top, card_w, palette::accent());

                let mut cy = cy_top + pad;
                if let Some(p) = ui.paint() {
                    let phase = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs_f32()
                        * 3.2;
                    let cx = card_x + card_w * 0.5;
                    let dot_r = 4.0;
                    for i in 0..3 {
                        let pulse = ((phase - i as f32 * 0.75).sin() * 0.5 + 0.5).clamp(0.0, 1.0);
                        let dx = cx + (i as f32 - 1.0) * 16.0;
                        let dy = cy + dots_h * 0.5;
                        p.rounded(
                            dx - dot_r,
                            dy - dot_r,
                            dot_r * 2.0,
                            dot_r * 2.0,
                            dot_r,
                            color::with_alpha(palette::accent_star(), 0.28 + 0.72 * pulse),
                        );
                    }
                }
                cy += dots_h + gap1;

                if let Some(p) = ui.paint() {
                    let t_sz = if compact_v { 20.0 } else { 23.0 };
                    let heading = tr("Connecting...");
                    let fit_h = truncate_to_fit(p, Face::SemiBold, t_sz, heading, inner_w);
                    let tw = p.measure(Face::SemiBold, t_sz, &fit_h);
                    p.text_line(
                        Face::SemiBold,
                        t_sz,
                        title_h,
                        inner_x + ((inner_w - tw) * 0.5).max(0.0).round(),
                        cy,
                        palette::text(),
                        &fit_h,
                    );
                    cy += title_h;

                    let sub = tr("Contacting Microsoft sign-in...");
                    let fit_s =
                        truncate_to_fit(p, Face::Regular, ui_theme::small(), sub, inner_w);
                    let sw = p.measure(Face::Regular, ui_theme::small(), &fit_s);
                    p.text_line(
                        Face::Regular,
                        ui_theme::small(),
                        sub_h,
                        inner_x + ((inner_w - sw) * 0.5).max(0.0).round(),
                        cy,
                        palette::muted(),
                        &fit_s,
                    );
                } else {
                    cy += title_h;
                }
                cy += sub_h + gap2;

                let btn_w = 160.0_f32.min(inner_w);
                let btn_x = inner_x + ((inner_w - btn_w) * 0.5).round();
                let eff_btn_h = btn_h.min((cy_top + card_h - cy - 6.0).max(34.0));
                draw_button(
                    ui,
                    hits,
                    ButtonId::CancelSignIn,
                    btn_x,
                    cy,
                    btn_w,
                    eff_btn_h,
                    Tone::Secondary,
                    tr("Cancel"),
                );
            }
            SignIn::Code { url, code } => {
                let card_w = w.min(500.0);
                let card_x = x + ((w - card_w) * 0.5).round();
                let pad = if compact_v { 14.0 } else { 24.0 };
                let inner_x = card_x + pad;
                let inner_w = (card_w - pad * 2.0).max(200.0);
                let eyebrow_h = 16.0;
                let title_h = if compact_v { 24.0 } else { 28.0 };
                let gap1 = if compact_v { 8.0 } else { 12.0 };
                let box_h = if compact_v { 50.0 } else { 58.0 };
                let gap2 = if compact_v { 8.0 } else { 12.0 };
                let url_h = if compact_v { 20.0 } else { 24.0 };
                let gap3 = if compact_v { 10.0 } else { 14.0 };
                let btn_h = if compact_v { 42.0 } else { 46.0 };
                let card_h = (pad * 2.0
                    + eyebrow_h
                    + title_h
                    + gap1
                    + box_h
                    + gap2
                    + url_h
                    + gap3
                    + btn_h)
                    .min(avail_h);
                let cy_top = y + ((avail_h - card_h) * 0.5).max(0.0).round();
                draw_plate(ui, card_x, cy_top, card_w, card_h, palette::border());
                draw_card_accent(ui, card_x, cy_top, card_w, palette::accent());

                let mut cy = cy_top + pad;
                if let Some(p) = ui.paint() {
                    let eyebrow = tr("MICROSOFT SIGN-IN");
                    let ew = p.measure(Face::Medium, 11.0, eyebrow);
                    p.text_line(
                        Face::Medium,
                        11.0,
                        eyebrow_h,
                        inner_x + ((inner_w - ew) * 0.5).max(0.0).round(),
                        cy,
                        palette::accent_star(),
                        eyebrow,
                    );
                    cy += eyebrow_h;

                    let t_sz = if compact_v { 18.5 } else { 21.5 };
                    let heading = tr("Enter this code in your browser");
                    let fit_h = truncate_to_fit(p, Face::SemiBold, t_sz, heading, inner_w);
                    let tw = p.measure(Face::SemiBold, t_sz, &fit_h);
                    p.text_line(
                        Face::SemiBold,
                        t_sz,
                        title_h,
                        inner_x + ((inner_w - tw) * 0.5).max(0.0).round(),
                        cy,
                        palette::text(),
                        &fit_h,
                    );
                } else {
                    cy += eyebrow_h;
                }
                cy += title_h + gap1;

                let copy_w = 82.0_f32.min(inner_w * 0.30);
                let copy_h = (box_h - 14.0).max(32.0);
                let copy_x = inner_x + inner_w - copy_w - 7.0;
                let copy_y = cy + ((box_h - copy_h) * 0.5).round();
                if let Some(p) = ui.paint() {
                    p.rounded(inner_x, cy, inner_w, box_h, 10.0, palette::bg_outer());
                    p.border(
                        inner_x,
                        cy,
                        inner_w,
                        box_h,
                        10.0,
                        color::with_alpha(palette::accent_star(), 0.55),
                    );
                    let formatted = if code.len() == 8 && code.is_ascii() {
                        format!("{} {}", &code[..4], &code[4..])
                    } else {
                        code.to_owned()
                    };
                    let code_room = (copy_x - 8.0 - inner_x).max(40.0);
                    let c_sz = p.fit(
                        Face::Mono,
                        if compact_v { 24.0 } else { 28.0 },
                        &formatted,
                        code_room - 12.0,
                    );
                    let cw = p.measure(Face::Mono, c_sz, &formatted);
                    p.text_line(
                        Face::Mono,
                        c_sz,
                        box_h,
                        inner_x + ((code_room - cw) * 0.5).max(8.0).round(),
                        cy,
                        palette::text(),
                        &formatted,
                    );
                }
                draw_button(
                    ui,
                    hits,
                    ButtonId::CopySignInCode,
                    copy_x,
                    copy_y,
                    copy_w,
                    copy_h,
                    if self.code_copied {
                        Tone::Banner
                    } else {
                        Tone::Secondary
                    },
                    if self.code_copied {
                        tr("Copied")
                    } else {
                        tr("Copy")
                    },
                );
                cy += box_h + gap2;

                if let Some(p) = ui.paint() {
                    let msg = self.status_note.as_deref().unwrap_or(url.as_str());
                    let fit = truncate_to_fit(p, Face::Regular, ui_theme::small(), msg, inner_w);
                    let mw = p.measure(Face::Regular, ui_theme::small(), &fit);
                    p.text_line(
                        Face::Regular,
                        ui_theme::small(),
                        url_h,
                        inner_x + ((inner_w - mw) * 0.5).max(0.0).round(),
                        cy,
                        if self.code_copied {
                            palette::accent_star()
                        } else {
                            palette::muted()
                        },
                        &fit,
                    );
                }
                cy += url_h + gap3;

                let eff_btn_h = btn_h.min((cy_top + card_h - cy - 6.0).max(34.0));
                let cancel_w = 120.0_f32.min(inner_w * 0.32);
                draw_button(
                    ui,
                    hits,
                    ButtonId::OpenBrowserUrl,
                    inner_x,
                    cy,
                    inner_w - cancel_w - 10.0,
                    eff_btn_h,
                    Tone::Primary,
                    tr("Open microsoft.com/link"),
                );
                draw_button(
                    ui,
                    hits,
                    ButtonId::CancelSignIn,
                    inner_x + inner_w - cancel_w,
                    cy,
                    cancel_w,
                    eff_btn_h,
                    Tone::Secondary,
                    tr("Cancel"),
                );
            }
            SignIn::Failed(_) => {
                let card_w = w.min(500.0);
                let card_x = x + ((w - card_w) * 0.5).round();
                let pad = if compact_v { 16.0 } else { 24.0 };
                let inner_x = card_x + pad;
                let inner_w = (card_w - pad * 2.0).max(200.0);
                let lines: Vec<String> =
                    wrap_lines(ui.ro(), Face::Regular, ui_theme::sizes().ui, body, inner_w)
                        .into_iter()
                        .take(3)
                        .collect();
                let btn_h = if compact_v { 42.0 } else { 46.0 };
                let content_h = 32.0 + 6.0 + lines.len() as f32 * 20.0 + 18.0 + btn_h;
                let card_h = (content_h + 2.0 * pad).min(avail_h);
                let cy_top = y + ((avail_h - card_h) * 0.5).max(0.0).round();
                draw_plate(ui, card_x, cy_top, card_w, card_h, palette::border());
                draw_card_accent(
                    ui,
                    card_x,
                    cy_top,
                    card_w,
                    color::hex(crate::menu_style::get().settings.danger, 220),
                );
                let cy = cy_top + pad;

                if let Some(p) = ui.paint() {
                    let t_sz = if compact_v { 19.5 } else { 21.5 };
                    let fit_t = truncate_to_fit(p, Face::SemiBold, t_sz, title, inner_w);
                    p.text_line(
                        Face::SemiBold,
                        t_sz,
                        30.0,
                        inner_x,
                        cy,
                        palette::text(),
                        &fit_t,
                    );
                    for (i, line) in lines.iter().enumerate() {
                        p.text_line(
                            Face::Regular,
                            ui_theme::sizes().ui,
                            20.0,
                            inner_x,
                            cy + 36.0 + i as f32 * 20.0,
                            palette::body(),
                            line,
                        );
                    }
                }
                let btn_y = cy + 36.0 + lines.len() as f32 * 20.0 + 18.0;
                let eff_btn_h = btn_h.min((cy_top + card_h - btn_y - 6.0).max(34.0));
                let half_w = (inner_w - 12.0) * 0.5;
                let (go, go_label) = if not_approved {
                    (ButtonId::OpenOfflineEntry, tr("Use Offline Account"))
                } else {
                    (ButtonId::RetrySignIn, tr("Try Again"))
                };
                draw_button(
                    ui,
                    hits,
                    go,
                    inner_x,
                    btn_y,
                    half_w,
                    eff_btn_h,
                    Tone::Primary,
                    go_label,
                );
                draw_button(
                    ui,
                    hits,
                    ButtonId::CancelSignIn,
                    inner_x + half_w + 12.0,
                    btn_y,
                    half_w,
                    eff_btn_h,
                    Tone::Secondary,
                    tr("Back"),
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
        self.account_list_top.set(None);
        self.account_pages.set(1);

        let inner_x = x;
        let inner_w = w;
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
                tr("Accounts"),
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
            tr("+ Add account"),
        );
        cy += hdr_h + if compact_v { 10.0 } else { 20.0 };

        let item_h = if compact_v { 54.0 } else { 66.0 };
        let gap = 8.0;
        let bottom = y + avail_h;
        self.account_list_top.set(Some(cy));
        if bottom - 8.0 - cy < item_h {
            return;
        }
        let (per_page, total_pages, rows_limit) =
            super::mods::pages(accounts.len(), cy, bottom, item_h + gap);
        self.account_pages.set(total_pages);
        let page = self.account_page.min(total_pages - 1);
        let shown = self.dots_at(2, page);
        let start = page * per_page;
        let end = (start + per_page).min(accounts.len());

        for (idx, acct) in accounts.iter().enumerate().skip(start).take(end - start) {
            if cy + item_h > rows_limit + 0.5 {
                break;
            }
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
                draw_player_head(p, inner_x + 12.0, head_y, head_sz, head.as_ref(), short);
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
                    Face::Regular,
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
                tr("Sign out"),
            );
            if is_active {
                if let Some(p) = ui.paint() {
                    let active_label = tr("Active");
                    let aw = p.measure(Face::Medium, ui_theme::small(), active_label);
                    p.text_line(
                        Face::Medium,
                        ui_theme::small(),
                        act_h,
                        inner_x + inner_w - 22.0 - rem_w - aw,
                        ay,
                        col_success,
                        active_label,
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
                    tr("Use"),
                );
            }
            cy += item_h + gap;
        }

        super::mods::draw_page_dots(
            ui,
            hits,
            ButtonId::AccountsNext,
            inner_x,
            rows_limit,
            bottom,
            inner_w,
            shown,
            total_pages,
        );
    }
}
