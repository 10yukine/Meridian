//! Shared North Star visual tokens, geometric icons, and formatting helpers.

use super::ButtonId;
use crate::launcher_backend::{
    Account, Head, Profile, ProfileState, Progress, Setup, Stage, VANILLA,
};
use crate::menu_style;
use crate::ui_theme::{self, Face, Ui, color};

pub(super) mod palette {
    use super::color;

    pub fn bg_outer() -> u32 {
        color::hex(0x080310, 255)
    }
    pub fn rail() -> u32 {
        color::hex(0x0b0514, 248)
    }
    pub fn card() -> u32 {
        color::hex(0x0f081c, 244)
    }
    pub fn surface() -> u32 {
        color::hex(0x170e29, 255)
    }
    pub fn surface_hover() -> u32 {
        color::hex(0x211538, 255)
    }
    pub fn border() -> u32 {
        color::hex(0x2a1e42, 220)
    }
    pub fn border_subtle() -> u32 {
        color::hex(0x211735, 190)
    }
    pub fn accent_fill() -> u32 {
        color::hex(0x7c45ec, 255)
    }
    pub fn accent_fill_hover() -> u32 {
        color::hex(0x8f5cff, 255)
    }
    pub fn accent() -> u32 {
        color::hex(0xa575f6, 255)
    }
    pub fn accent_star() -> u32 {
        color::hex(0xa990ff, 255)
    }
    pub fn text() -> u32 {
        color::hex(0xffffff, 255)
    }
    pub fn body() -> u32 {
        color::hex(0xebe8f2, 255)
    }
    pub fn muted() -> u32 {
        color::hex(0x8c859e, 255)
    }
    pub fn logo_gray() -> u32 {
        color::hex(0x706c79, 255)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tone {
    Primary,
    Secondary,
    Ghost,
    Danger,
    Banner,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RailIcon {
    Home,
    Mods,
    Skins,
    Settings,
}

pub(super) struct HitRect {
    pub id: ButtonId,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl HitRect {
    pub fn contains(&self, (cx, cy): (f32, f32)) -> bool {
        cx >= self.x && cx < self.x + self.w && cy >= self.y && cy < self.y + self.h
    }
}

pub(super) enum Canvas<'r, 'a> {
    Draw(&'r mut Ui<'a>),
    Hit(&'r Ui<'a>),
}

impl<'a> Canvas<'_, 'a> {
    pub fn ro(&self) -> &Ui<'a> {
        match self {
            Self::Draw(ui) => ui,
            Self::Hit(ui) => ui,
        }
    }

    pub fn paint(&mut self) -> Option<&mut Ui<'a>> {
        match self {
            Self::Draw(ui) => Some(ui),
            Self::Hit(_) => None,
        }
    }
}

pub(super) fn draw_backdrop(p: &mut Ui, w: f32, h: f32) {
    p.rounded(0.0, 0.0, w, h, 0.0, palette::bg_outer());
    // Ultra-subtle ambient violet tint anchored gently off-center (no visible ring banding).
    for (cx_f, cy_f, scale, alpha) in [
        (0.18, 0.14, 0.72, 0.024),
        (0.54, 0.44, 0.92, 0.018),
        (0.84, 0.86, 0.68, 0.020),
    ] {
        let rw = w * scale;
        let rh = h * scale;
        p.rounded(
            (w * cx_f - rw * 0.5).round(),
            (h * cy_f - rh * 0.5).round(),
            rw,
            rh,
            (rw.min(rh) * 0.5).max(40.0),
            color::with_alpha(palette::accent(), alpha),
        );
    }
}

pub(super) fn draw_plate(ui: &mut Canvas<'_, '_>, x: f32, y: f32, w: f32, h: f32, border_col: u32) {
    if let Some(p) = ui.paint() {
        p.rounded(x, y, w, h, 14.0, palette::card());
        p.border(x, y, w, h, 14.0, border_col);
    }
}

fn cubic_pt(p0: [f32; 2], p1: [f32; 2], p2: [f32; 2], p3: [f32; 2], t: f32) -> [f32; 2] {
    let u = 1.0 - t;
    let uu = u * u;
    let tt = t * t;
    let uuu = uu * u;
    let ttt = tt * t;
    [
        uuu * p0[0] + 3.0 * uu * t * p1[0] + 3.0 * u * tt * p2[0] + ttt * p3[0],
        uuu * p0[1] + 3.0 * uu * t * p1[1] + 3.0 * u * tt * p2[1] + ttt * p3[1],
    ]
}

/// Exact vector construction of `Meridian Logo/03 North Star.svg` rendered via GPU quads:
/// white slanted pillar -> sweeping violet cubic-Bezier orbit -> gray slanted pillar -> 4-point star.
pub(super) fn draw_meridian_north_star(p: &mut Ui, x: f32, y: f32, size: f32) {
    let k = p.density / p.batch.scale;
    // Tightened crop around the SVG artwork inside viewBox (308.1, 313.05, 640, 640).
    let (vx, vy, vw) = (358.1_f32, 353.05_f32, 540.0_f32);
    let to_gui = |sx: f32, sy: f32| -> [f32; 2] {
        [
            (x + (sx - vx) / vw * size) * k,
            (y + (sy - vy) / vw * size) * k,
        ]
    };
    let bar_pt = |px: f32, py: f32| -> [f32; 2] {
        let sx = 628.1 + (px - 628.1) * 0.86;
        let sy = 633.05 + (py - 633.05) * 0.86;
        to_gui(sx, sy)
    };

    // 1. Back white slanted bar (#ffffff)
    p.batch.fill_quad(
        p.atlas,
        [
            bar_pt(708.56, 511.685),
            bar_pt(789.38, 454.625),
            bar_pt(789.38, 754.415),
            bar_pt(708.56, 811.475),
        ],
        palette::text(),
    );

    // 2. Sweeping violet orbital crescent (#a575f6) — exact SVG cubic Beziers
    let rad = 52.5_f32.to_radians();
    let (sin_a, cos_a) = rad.sin_cos();
    let sweep_pt = |pt: [f32; 2]| -> [f32; 2] {
        let dx = pt[0] - 628.1;
        let dy = pt[1] - 609.0;
        let rx = 628.1 + dx * cos_a - dy * sin_a;
        let ry = 609.0 + dx * sin_a + dy * cos_a;
        to_gui(1256.2 - rx, ry + 18.0)
    };
    let outer_segs: [([f32; 2], [f32; 2], [f32; 2], [f32; 2]); 4] = [
        (
            [567.62, 526.37],
            [476.06, 510.41],
            [382.82, 506.21],
            [368.54, 537.29],
        ),
        (
            [368.54, 537.29],
            [350.90, 575.09],
            [474.38, 633.05],
            [612.98, 667.49],
        ),
        (
            [612.98, 667.49],
            [745.70, 700.25],
            [885.14, 689.33],
            [888.50, 660.77],
        ),
        (
            [888.50, 660.77],
            [891.86, 630.53],
            [785.18, 586.85],
            [684.38, 561.65],
        ),
    ];
    // Return path reversed so outer[seg](t) pairs cleanly with inner[seg](t).
    let inner_segs: [([f32; 2], [f32; 2], [f32; 2], [f32; 2]); 4] = [
        (
            [567.62, 526.37],
            [480.26, 512.93],
            [416.42, 520.49],
            [405.50, 544.85],
        ),
        (
            [405.50, 544.85],
            [393.74, 570.89],
            [481.94, 617.09],
            [609.62, 645.65],
        ),
        (
            [609.62, 645.65],
            [733.10, 672.53],
            [860.78, 678.41],
            [861.62, 648.17],
        ),
        (
            [861.62, 648.17],
            [862.46, 629.69],
            [771.74, 595.25],
            [684.38, 561.65],
        ),
    ];
    let steps = 8usize;
    for seg in 0..4 {
        let (o0, o1, o2, o3) = outer_segs[seg];
        let (i0, i1, i2, i3) = inner_segs[seg];
        for s in 0..steps {
            let t0 = s as f32 / steps as f32;
            let t1 = (s + 1) as f32 / steps as f32;
            let a = sweep_pt(cubic_pt(o0, o1, o2, o3, t0));
            let b = sweep_pt(cubic_pt(o0, o1, o2, o3, t1));
            let c = sweep_pt(cubic_pt(i0, i1, i2, i3, t1));
            let d = sweep_pt(cubic_pt(i0, i1, i2, i3, t0));
            p.batch.fill_quad(p.atlas, [a, b, c, d], palette::accent());
        }
    }

    // 3. Front gray slanted bar (#706c79)
    p.batch.fill_quad(
        p.atlas,
        [
            bar_pt(467.18, 454.625),
            bar_pt(547.64, 511.685),
            bar_pt(547.64, 811.475),
            bar_pt(467.18, 754.415),
        ],
        palette::logo_gray(),
    );

    // 4. Concave 4-point North Star sparkle (#a990ff)
    let star_pt =
        |pt: [f32; 2]| -> [f32; 2] { to_gui(308.1 + pt[0] * 1.25, 313.05 + pt[1] * 1.25) };
    let center = star_pt([253.0, 214.0]);
    let star_arcs: [([f32; 2], [f32; 2], [f32; 2], [f32; 2]); 4] = [
        (
            [253.0, 178.0],
            [256.0, 204.0],
            [263.0, 210.0],
            [289.0, 214.0],
        ),
        (
            [289.0, 214.0],
            [263.0, 218.0],
            [256.0, 225.0],
            [253.0, 250.0],
        ),
        (
            [253.0, 250.0],
            [250.0, 225.0],
            [242.0, 218.0],
            [217.0, 214.0],
        ),
        (
            [217.0, 214.0],
            [242.0, 210.0],
            [250.0, 204.0],
            [253.0, 178.0],
        ),
    ];
    let star_steps = 6usize;
    for (p0, p1, p2, p3) in star_arcs {
        for s in 0..star_steps {
            let t0 = s as f32 / star_steps as f32;
            let t1 = (s + 1) as f32 / star_steps as f32;
            let a = star_pt(cubic_pt(p0, p1, p2, p3, t0));
            let b = star_pt(cubic_pt(p0, p1, p2, p3, t1));
            p.batch
                .fill_quad(p.atlas, [center, a, b, center], palette::accent_star());
        }
    }
}

/// Framed Meridian North Star logo tile anchored at the top-left of the launcher rail.
pub(super) fn draw_north_star_badge(p: &mut Ui, x: f32, y: f32, size: f32) {
    // Soft ambient violet halo
    p.rounded(
        x - 2.0,
        y - 2.0,
        size + 4.0,
        size + 4.0,
        13.0,
        color::with_alpha(palette::accent(), 0.14),
    );
    p.rounded(x, y, size, size, 11.0, palette::card());
    p.border(
        x,
        y,
        size,
        size,
        11.0,
        color::with_alpha(palette::accent_star(), 0.48),
    );
    let pad = (size * 0.08).round();
    draw_meridian_north_star(p, x + pad, y + pad, size - pad * 2.0);
}

/// The rail icons' line, `t` thick from `a` to `b` (UI units), with round ends and joins.
fn stroke(p: &mut Ui, a: [f32; 2], b: [f32; 2], t: f32, col: u32) {
    let k = p.density / p.batch.scale;
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len = dx.hypot(dy).max(f32::EPSILON);
    let (nx, ny) = (-dy / len * t * 0.5, dx / len * t * 0.5);
    p.batch.fill_quad(
        p.atlas,
        [
            [(a[0] + nx) * k, (a[1] + ny) * k],
            [(b[0] + nx) * k, (b[1] + ny) * k],
            [(b[0] - nx) * k, (b[1] - ny) * k],
            [(a[0] - nx) * k, (a[1] - ny) * k],
        ],
        col,
    );
    for [x, y] in [a, b] {
        p.rounded(x - t * 0.5, y - t * 0.5, t, t, t * 0.5, col);
    }
}

/// An outline on the icons' 18-unit grid, centred on `(cx, cy)`: one stroke weight for all.
fn icon_path(p: &mut Ui, cx: f32, cy: f32, sz: f32, col: u32, points: &[[f32; 2]], closed: bool) {
    let s = sz / 18.0;
    let at = |[x, y]: [f32; 2]| [cx + (x - 9.0) * s, cy + (y - 9.0) * s];
    let ends = closed.then(|| [points[points.len() - 1], points[0]]);
    for pair in points.windows(2).map(|w| [w[0], w[1]]).chain(ends) {
        stroke(p, at(pair[0]), at(pair[1]), 1.7 * s, col);
    }
}

/// Points on a circle around `c` from `from` to `to` degrees (y down: 270 is up).
fn arc(c: [f32; 2], r: f32, from: f32, to: f32) -> impl Iterator<Item = [f32; 2]> {
    let steps = ((to - from).abs() / 22.5).ceil().max(1.0) as usize;
    (0..=steps).map(move |i| {
        let a = (from + (to - from) * i as f32 / steps as f32).to_radians();
        [c[0] + r * a.cos(), c[1] + r * a.sin()]
    })
}

pub(super) fn draw_icon_home(p: &mut Ui, cx: f32, cy: f32, sz: f32, col: u32) {
    let roof = [[2.0, 8.5], [9.0, 2.5], [16.0, 8.5]];
    icon_path(p, cx, cy, sz, col, &roof, false);
    let walls = [[4.0, 7.0], [4.0, 15.5], [14.0, 15.5], [14.0, 7.0]];
    icon_path(p, cx, cy, sz, col, &walls, false);
    let door = [[7.3, 15.5], [7.3, 11.0], [10.7, 11.0], [10.7, 15.5]];
    icon_path(p, cx, cy, sz, col, &door, false);
}

/// A puzzle piece: a square with a knob on its top and right sides.
pub(super) fn draw_icon_mods(p: &mut Ui, cx: f32, cy: f32, sz: f32, col: u32) {
    let mut piece = vec![[3.0, 15.0], [3.0, 5.0], [6.3, 5.0]];
    piece.extend(arc([8.0, 3.77], 2.1, 144.0, 396.0));
    piece.extend([[9.7, 5.0], [13.0, 5.0], [13.0, 8.3]]);
    piece.extend(arc([14.23, 10.0], 2.1, 234.0, 486.0));
    piece.extend([[13.0, 11.7], [13.0, 15.0]]);
    // Centred: the knobs reach past the square up and to the right.
    let piece: Vec<_> = piece
        .into_iter()
        .map(|[x, y]| [x - 0.65, y + 0.65])
        .collect();
    icon_path(p, cx, cy, sz, col, &piece, true);
}

pub(super) fn draw_icon_skins(p: &mut Ui, cx: f32, cy: f32, sz: f32, col: u32) {
    let shirt = [
        [6.5, 2.5],
        [2.0, 4.8],
        [3.0, 8.6],
        [5.0, 8.0],
        [5.0, 15.5],
        [13.0, 15.5],
        [13.0, 8.0],
        [15.0, 8.6],
        [16.0, 4.8],
        [11.5, 2.5],
        [9.0, 4.0],
    ];
    icon_path(p, cx, cy, sz, col, &shirt, true);
}

/// A gear: a ring with eight teeth around a hub.
pub(super) fn draw_icon_settings(p: &mut Ui, cx: f32, cy: f32, sz: f32, col: u32) {
    let ring: Vec<_> = arc([9.0, 9.0], 5.0, 0.0, 360.0).collect();
    icon_path(p, cx, cy, sz, col, &ring, false);
    let hub: Vec<_> = arc([9.0, 9.0], 1.9, 0.0, 360.0).collect();
    icon_path(p, cx, cy, sz, col, &hub, false);
    let s = sz / 18.0;
    for tooth in 0..8 {
        let a = (tooth as f32 * 45.0).to_radians();
        let at = |r: f32| [cx + r * a.cos() * s, cy + r * a.sin() * s];
        stroke(p, at(5.0), at(7.2), 2.4 * s, col);
    }
}

pub(super) fn draw_icon_play(p: &mut Ui, cx: f32, cy: f32, sz: f32, col: u32) {
    let k = p.density / p.batch.scale;
    let left = cx - sz * 0.40;
    let right = cx + sz * 0.52;
    let half_h = sz * 0.52;
    p.batch.fill_quad(
        p.atlas,
        [
            [left * k, (cy - half_h) * k],
            [right * k, cy * k],
            [left * k, (cy + half_h) * k],
            [left * k, (cy - half_h) * k],
        ],
        col,
    );
}

pub(super) fn draw_icon_box(p: &mut Ui, cx: f32, cy: f32, sz: f32, col: u32) {
    let s = sz / 24.0;
    let x = cx - sz * 0.5;
    let y = cy - sz * 0.5;
    // Lid
    p.border(x + 2.0 * s, y + 3.5 * s, 20.0 * s, 4.8 * s, 2.0, col);
    // Body
    p.border(x + 3.5 * s, y + 8.0 * s, 17.0 * s, 12.5 * s, 2.4, col);
    // Handle slot
    p.rounded(x + 9.0 * s, y + 12.0 * s, 6.0 * s, 2.2 * s, 1.0, col);
}

pub(super) fn draw_icon_chevron(p: &mut Ui, cx: f32, cy: f32, sz: f32, col: u32, up: bool) {
    let half_w = sz * 0.55;
    let half_h = sz * 0.30;
    let pts = if up {
        [
            [cx - half_w, cy + half_h],
            [cx, cy - half_h],
            [cx + half_w, cy + half_h],
        ]
    } else {
        [
            [cx - half_w, cy - half_h],
            [cx, cy + half_h],
            [cx + half_w, cy - half_h],
        ]
    };
    let t = (sz * 0.20).clamp(1.3, 1.8);
    stroke(p, pts[0], pts[1], t, col);
    stroke(p, pts[1], pts[2], t, col);
}

pub(super) fn draw_player_head(
    p: &mut Ui,
    x: f32,
    y: f32,
    size: f32,
    head: Option<&Head>,
    name: &str,
) {
    if let Some(rgba) = head {
        let step = size / 8.0;
        for row in 0..8 {
            for col in 0..8 {
                let i = (row * 8 + col) * 4;
                let a = rgba[i + 3];
                if a == 0 {
                    continue;
                }
                let rgb = (u32::from(rgba[i]) << 16)
                    | (u32::from(rgba[i + 1]) << 8)
                    | u32::from(rgba[i + 2]);
                p.rounded(
                    x + col as f32 * step,
                    y + row as f32 * step,
                    step.ceil(),
                    step.ceil(),
                    0.0,
                    color::hex(rgb, a),
                );
            }
        }
        p.border(x, y, size, size, 3.0, palette::border());
    } else {
        p.rounded(x, y, size, size, 6.0, palette::surface_hover());
        p.border(x, y, size, size, 6.0, palette::accent_star());
        let initial: String = name
            .chars()
            .next()
            .map_or('?', |c| c.to_ascii_uppercase())
            .to_string();
        let iw = p.measure(Face::SemiBold, size * 0.52, &initial);
        p.text_line(
            Face::SemiBold,
            size * 0.52,
            size,
            x + ((size - iw) * 0.5).round(),
            y,
            palette::text(),
            &initial,
        );
    }
}

pub(super) fn draw_player_head_with_dot(
    p: &mut Ui,
    x: f32,
    y: f32,
    size: f32,
    head: Option<&Head>,
    name: &str,
    dot_col: u32,
) {
    draw_player_head(p, x, y, size, head, name);
    let dx = x + size - 7.0;
    let dy = y + size - 7.0;
    p.rounded(dx - 2.0, dy - 2.0, 11.0, 11.0, 5.5, palette::rail());
    p.rounded(dx, dy, 7.0, 7.0, 3.5, dot_col);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_input_box(
    ui: &mut Canvas<'_, '_>,
    hits: &mut Vec<HitRect>,
    id: ButtonId,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    label: &str,
    value: &str,
    masked: bool,
    focused: bool,
) {
    let hit = HitRect { id, x, y, w, h };
    if let Some(p) = ui.paint() {
        let bdr = if focused {
            palette::accent_star()
        } else {
            palette::border()
        };
        p.rounded(x, y, w, h, 9.0, palette::surface());
        p.border(x, y, w, h, 9.0, bdr);
        let mut tx = x + 12.0;
        if !label.is_empty() {
            p.text_line(
                Face::Medium,
                ui_theme::small(),
                h,
                tx,
                y,
                palette::muted(),
                label,
            );
            tx += p.measure(Face::Medium, ui_theme::small(), label) + 14.0;
        }
        let shown = if masked {
            "*".repeat(value.chars().count())
        } else {
            value.to_owned()
        };
        let display = if focused { format!("{shown}|") } else { shown };
        let fit = truncate_to_fit(
            p,
            Face::Mono,
            ui_theme::sizes().ui,
            &display,
            (x + w - 12.0 - tx).max(20.0),
        );
        p.text_line(
            Face::Mono,
            ui_theme::sizes().ui,
            h,
            tx,
            y,
            palette::text(),
            &fit,
        );
    }
    hits.push(hit);
}

/// A row that toggles: a checkbox, the name left-aligned beside it, and `detail` (a size)
/// right-aligned and muted.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_check_row(
    ui: &mut Canvas<'_, '_>,
    hits: &mut Vec<HitRect>,
    id: ButtonId,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    checked: bool,
    name: &str,
    detail: &str,
) {
    hits.push(HitRect { id, x, y, w, h });
    let Some(p) = ui.paint() else { return };
    let (plate, edge) = if checked {
        (
            color::with_alpha(palette::accent(), 0.16),
            color::with_alpha(palette::accent_star(), 0.6),
        )
    } else {
        (palette::surface(), palette::border())
    };
    p.rounded(x, y, w, h, 8.0, plate);
    p.border(x, y, w, h, 8.0, edge);
    let bs = (h * 0.5).clamp(14.0, 20.0);
    let (bx, by) = (x + 12.0, (y + (h - bs) * 0.5).round());
    if checked {
        p.rounded(bx, by, bs, bs, 4.0, palette::accent_fill());
        let t = (bs * 0.13).max(1.8);
        let mid = [bx + bs * 0.42, by + bs * 0.70];
        stroke(p, [bx + bs * 0.22, by + bs * 0.50], mid, t, palette::text());
        stroke(p, mid, [bx + bs * 0.78, by + bs * 0.30], t, palette::text());
    } else {
        p.border(bx, by, bs, bs, 4.0, palette::muted());
    }
    let size = ui_theme::small();
    let dw = p.measure(Face::Mono, size, detail);
    let dx = x + w - 12.0 - dw;
    p.text_line(Face::Mono, size, h, dx, y, palette::muted(), detail);
    let nx = bx + bs + 10.0;
    let fit = truncate_to_fit(
        p,
        Face::Medium,
        ui_theme::body(),
        name,
        (dx - 12.0 - nx).max(0.0),
    );
    p.text_line(
        Face::Medium,
        ui_theme::body(),
        h,
        nx,
        y,
        palette::text(),
        &fit,
    );
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_button(
    ui: &mut Canvas<'_, '_>,
    hits: &mut Vec<HitRect>,
    id: ButtonId,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    tone: Tone,
    label: &str,
) {
    let hit = HitRect { id, x, y, w, h };
    if let Some(p) = ui.paint() {
        let hovered = hit.contains(p.cursor);
        let danger = color::hex(menu_style::get().settings.danger, 255);
        let (plate, border, ink) = match (tone, hovered) {
            (Tone::Primary, false) => (
                palette::accent_fill(),
                Some(color::with_alpha(palette::accent_star(), 0.72)),
                palette::text(),
            ),
            (Tone::Primary, true) => (
                palette::accent_fill_hover(),
                Some(palette::accent_star()),
                palette::text(),
            ),
            (Tone::Banner, false) => (
                color::with_alpha(palette::accent(), 0.22),
                Some(palette::accent_star()),
                palette::text(),
            ),
            (Tone::Banner, true) => (
                color::with_alpha(palette::accent(), 0.34),
                Some(palette::text()),
                palette::text(),
            ),
            (Tone::Secondary, false) => {
                (palette::surface(), Some(palette::border()), palette::body())
            }
            (Tone::Secondary, true) => (
                palette::surface_hover(),
                Some(palette::accent_star()),
                palette::text(),
            ),
            (Tone::Ghost, false) => (0x0000_0000, None, palette::muted()),
            (Tone::Ghost, true) => (palette::surface(), Some(palette::border()), palette::text()),
            (Tone::Danger, false) => (color::with_alpha(danger, 0.14), Some(danger), danger),
            (Tone::Danger, true) => (danger, Some(danger), palette::text()),
        };
        let radius = 10.0;
        if plate >> 24 != 0 {
            p.rounded(x, y, w, h, radius, plate);
        }
        if let Some(b) = border {
            p.border(x, y, w, h, radius, b);
        }
        let room = (w - 16.0).max(16.0);
        let font_size = p.fit(Face::Medium, ui_theme::sizes().ui, label, room);
        let tw = p.measure(Face::Medium, font_size, label);
        let lx = x + ((w - tw) * 0.5).round();
        p.text_line(Face::Medium, font_size, h, lx, y, ink, label);
    }
    hits.push(hit);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_play_button(
    ui: &mut Canvas<'_, '_>,
    hits: &mut Vec<HitRect>,
    id: ButtonId,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    label: &str,
    with_play_icon: bool,
) {
    let hit = HitRect { id, x, y, w, h };
    if let Some(p) = ui.paint() {
        let hovered = hit.contains(p.cursor);
        // Subtle purple glow behind the primary launch CTA
        p.rounded(
            x - 2.0,
            y - 2.0,
            w + 4.0,
            h + 4.0,
            13.0,
            color::with_alpha(palette::accent(), if hovered { 0.24 } else { 0.14 }),
        );
        let fill = if hovered {
            palette::accent_fill_hover()
        } else {
            palette::accent_fill()
        };
        p.rounded(x, y, w, h, 11.0, fill);
        p.border(
            x,
            y,
            w,
            h,
            11.0,
            color::with_alpha(palette::accent_star(), if hovered { 0.95 } else { 0.72 }),
        );
        let icon_span = if with_play_icon { 18.0 } else { 0.0 };
        let room = (w - 22.0 - icon_span).max(24.0);
        let font_size = p.fit(Face::SemiBold, 15.0, label, room);
        let tw = p.measure(Face::SemiBold, font_size, label);
        let total_w = tw + icon_span;
        let start_x = x + ((w - total_w) * 0.5).round();
        if with_play_icon {
            draw_icon_play(p, start_x + 5.0, y + h * 0.5, 11.0, palette::text());
        }
        p.text_line(
            Face::SemiBold,
            font_size,
            h,
            start_x + icon_span,
            y,
            palette::text(),
            label,
        );
    }
    hits.push(hit);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_rail_item(
    ui: &mut Canvas<'_, '_>,
    hits: &mut Vec<HitRect>,
    id: ButtonId,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    active: bool,
    icon: RailIcon,
    label: &str,
) {
    let hit = HitRect { id, x, y, w, h };
    if let Some(p) = ui.paint() {
        let hovered = hit.contains(p.cursor);
        let (fill, bdr, ink) = if active {
            (
                color::with_alpha(palette::accent(), 0.20),
                Some(color::with_alpha(palette::accent_star(), 0.75)),
                palette::text(),
            )
        } else if hovered {
            (palette::surface(), Some(palette::border()), palette::body())
        } else {
            (0x0000_0000, None, palette::muted())
        };
        if active {
            p.rounded(
                x - 2.0,
                y - 2.0,
                w + 4.0,
                h + 4.0,
                14.0,
                color::with_alpha(palette::accent(), 0.12),
            );
        }
        if fill >> 24 != 0 {
            p.rounded(x, y, w, h, 12.0, fill);
        }
        if let Some(b) = bdr {
            p.border(x, y, w, h, 12.0, b);
        }
        if active {
            let bar_inset = (h * 0.22).clamp(4.0, 12.0);
            p.rounded(
                x - 5.0,
                y + bar_inset,
                3.0,
                (h - bar_inset * 2.0).max(8.0),
                1.5,
                palette::accent_star(),
            );
        }
        let icon_only = h < 36.0;
        let compact = h < 48.0;
        let icon_sz = if icon_only {
            (h - 10.0).clamp(14.0, 18.0)
        } else if compact {
            17.0
        } else {
            19.0
        };
        let icon_cy = if icon_only {
            y + h * 0.5
        } else {
            y + if compact { 15.5 } else { 19.0 }
        };
        let icon_cx = x + w * 0.5;
        let icon_col = if active { palette::accent_star() } else { ink };
        match icon {
            RailIcon::Home => draw_icon_home(p, icon_cx, icon_cy, icon_sz, icon_col),
            RailIcon::Mods => draw_icon_mods(p, icon_cx, icon_cy, icon_sz, icon_col),
            RailIcon::Skins => draw_icon_skins(p, icon_cx, icon_cy, icon_sz, icon_col),
            RailIcon::Settings => draw_icon_settings(p, icon_cx, icon_cy, icon_sz, icon_col),
        }
        if !icon_only {
            let lbl_sz = if compact { 9.0 } else { 9.5 };
            let lw = p.measure(Face::Medium, lbl_sz, label);
            let ly = y + h - if compact { 14.0 } else { 16.0 };
            p.text_line(
                Face::Medium,
                lbl_sz,
                13.0,
                x + ((w - lw) * 0.5).round(),
                ly,
                ink,
                label,
            );
        }
    }
    hits.push(hit);
}

pub(super) fn count_label(n: usize, singular: &str, plural: &str) -> String {
    if n == 1 {
        format!("1 {singular}")
    } else {
        format!("{n} {plural}")
    }
}

pub(super) fn profile_stats_line(profile: &Profile) -> String {
    format!(
        "{}  |  {}  |  {}",
        count_label(profile.mods, "mod", "mods"),
        count_label(profile.worlds, "world", "worlds"),
        format_size(profile.size)
    )
}

pub(super) fn effective_profile_state(profile: &Profile, setup: &Setup) -> ProfileState {
    if profile.id == VANILLA {
        match setup {
            Setup::Needed => ProfileState::Available,
            Setup::Running(p) => ProfileState::Installing(p.clone()),
            Setup::Failed { error, .. } => ProfileState::Failed(error.clone()),
            Setup::Ready => ProfileState::Installed,
        }
    } else {
        profile.state.clone()
    }
}

pub(super) fn profile_badge_parts(
    state: &ProfileState,
    selected: bool,
    col_success: u32,
    col_danger: u32,
) -> (String, u32) {
    match state {
        ProfileState::Available => ("NEEDS INSTALL".to_owned(), palette::accent_star()),
        ProfileState::Installing(p) => {
            let pct = (stage_fraction(p) * 100.0).round() as u32;
            (format!("INSTALLING {pct}%"), palette::accent())
        }
        ProfileState::Installed if selected => ("READY".to_owned(), col_success),
        ProfileState::Installed => ("INSTALLED".to_owned(), palette::muted()),
        ProfileState::Failed(_) => ("FAILED".to_owned(), col_danger),
    }
}

pub(super) fn account_short(account: &Account) -> &str {
    match account {
        Account::Microsoft { name } | Account::Ely { name } | Account::Offline { name } => {
            name.as_str()
        }
    }
}

pub(super) fn account_provider_tag(account: &Account) -> &'static str {
    match account {
        Account::Microsoft { .. } => "Microsoft account",
        Account::Ely { .. } => "Ely.by account",
        Account::Offline { .. } => "Offline profile",
    }
}

pub(super) fn account_label(account: &Account) -> String {
    match account {
        Account::Microsoft { name } => format!("Microsoft ({name})"),
        Account::Ely { name } => format!("Ely.by ({name})"),
        Account::Offline { name } => format!("Offline ({name})"),
    }
}

pub(super) fn format_size(bytes: u64) -> String {
    if bytes == 0 {
        "Base game".to_owned()
    } else if bytes >= 1_000_000 {
        format!("{:.1} MB", bytes as f64 / 1_000_000.0)
    } else {
        format!("{:.0} KB", (bytes as f64 / 1_000.0).max(1.0))
    }
}

pub(super) fn stage_index(stage: Stage) -> usize {
    match stage {
        Stage::Check => 0,
        Stage::Download => 1,
        Stage::Verify => 2,
        Stage::Prepare => 3,
    }
}

pub(super) fn stage_name(stage: Stage) -> &'static str {
    match stage {
        Stage::Check => "Check",
        Stage::Download => "Download",
        Stage::Verify => "Verify",
        Stage::Prepare => "Prepare",
    }
}

pub(super) fn stage_heading(stage: Stage) -> &'static str {
    match stage {
        Stage::Check => "Checking installed game files",
        Stage::Download => "Downloading Minecraft 26.3 resources",
        Stage::Verify => "Verifying SHA-1 file integrity",
        Stage::Prepare => "Preparing runtime resources",
    }
}

pub(super) fn stage_fraction(progress: &Progress) -> f32 {
    if progress.total == 0 {
        return 0.0;
    }
    (progress.done as f64 / progress.total as f64).clamp(0.0, 1.0) as f32
}

pub(super) fn stage_pill_text(progress: &Progress) -> String {
    let pct = (stage_fraction(progress) * 100.0).round() as u32;
    if progress.stage == Stage::Download && progress.bytes_per_second > 0 {
        let mbps = progress.bytes_per_second as f64 / 1_000_000.0;
        format!("Downloading {pct}% ({mbps:.1} MB/s)")
    } else {
        format!("{} {pct}%", stage_name(progress.stage))
    }
}

pub(super) fn format_progress_stats(progress: &Progress, frac: f32) -> (String, String) {
    let pct = (frac * 100.0).round() as u32;
    match progress.stage {
        Stage::Download => {
            let done_mb = progress.done as f64 / 1_000_000.0;
            let total_mb = progress.total as f64 / 1_000_000.0;
            let left = if progress.total > 0 {
                format!("{done_mb:.1} / {total_mb:.1} MB")
            } else {
                format!("{done_mb:.1} MB")
            };
            let right = if progress.bytes_per_second > 0 && progress.total > progress.done {
                let speed = progress.bytes_per_second as f64 / 1_000_000.0;
                let eta = (progress.total - progress.done) / progress.bytes_per_second.max(1);
                format!("{pct}%  |  {speed:.1} MB/s  |  ETA {eta}s")
            } else {
                format!("{pct}%")
            };
            (left, right)
        }
        Stage::Prepare => {
            let left = if progress.total > 0 {
                format!("Step {} of {}", progress.done, progress.total)
            } else {
                "Running preparer...".to_owned()
            };
            (left, format!("{pct}%"))
        }
        Stage::Check | Stage::Verify => {
            let left = if progress.total > 0 {
                format!("{} / {} files", progress.done, progress.total)
            } else {
                "Scanning files...".to_owned()
            };
            (left, format!("{pct}%"))
        }
    }
}

pub(super) fn truncate_to_fit(ui: &Ui, face: Face, size: f32, text: &str, max_w: f32) -> String {
    if max_w <= 12.0 {
        return String::new();
    }
    if ui.measure(face, size, text) <= max_w {
        return text.to_owned();
    }
    let mut out = String::new();
    for ch in text.chars() {
        let candidate = format!("{out}{ch}...");
        if ui.measure(face, size, &candidate) > max_w {
            break;
        }
        out.push(ch);
    }
    format!("{out}...")
}

pub(super) fn wrap_lines(ui: &Ui, face: Face, size: f32, text: &str, max_w: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split(' ') {
        let candidate = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        if !line.is_empty() && ui.measure(face, size, &candidate) > max_w {
            lines.push(std::mem::replace(&mut line, word.to_owned()));
        } else {
            line = candidate;
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}
