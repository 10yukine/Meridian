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
        color::hex(0x0a0a0c, 255)
    }
    pub fn rail() -> u32 {
        color::hex(0x0e0e11, 248)
    }
    pub fn card() -> u32 {
        color::hex(0x121215, 244)
    }
    pub fn surface() -> u32 {
        color::hex(0x19191c, 255)
    }
    pub fn surface_hover() -> u32 {
        color::hex(0x242428, 255)
    }
    pub fn border() -> u32 {
        color::hex(0x2e2e33, 220)
    }
    pub fn border_subtle() -> u32 {
        color::hex(0x222226, 190)
    }
    pub fn accent_fill() -> u32 {
        color::hex(0x6d5ed1, 255)
    }
    pub fn accent_fill_hover() -> u32 {
        color::hex(0x7a6be0, 255)
    }
    pub fn accent() -> u32 {
        color::hex(0x9d90ea, 255)
    }
    pub fn accent_star() -> u32 {
        color::hex(0xb1a7f2, 255)
    }
    pub fn text() -> u32 {
        color::hex(0xfafafa, 255)
    }
    pub fn body() -> u32 {
        color::hex(0xe4e4e7, 255)
    }
    pub fn muted() -> u32 {
        color::hex(0xa1a1aa, 255)
    }
    pub fn logo_gray() -> u32 {
        color::hex(0x71717a, 255)
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

/// Tabler Icons' 24-unit outlines (3.24.0, MIT, Paweł Kuna; `THIRD_PARTY_NOTICES.md`), drawn as
/// the rail's strokes are (`icon_path`, an 18-unit grid): each SVG path's pieces, curves and arcs
/// as short lines ([`svg_path`]).
fn tabler(p: &mut Ui, cx: f32, cy: f32, size: f32, paths: &[&str], col: u32) {
    for d in paths {
        for (points, closed) in svg_path(d) {
            let points: Vec<[f32; 2]> = points.iter().map(|[x, y]| [x * 0.75, y * 0.75]).collect();
            icon_path(p, cx, cy, size, col, &points, closed);
        }
    }
}

/// Tabler's `home`.
pub(super) fn draw_icon_home(p: &mut Ui, cx: f32, cy: f32, sz: f32, col: u32) {
    let paths = [
        "M5 12l-2 0l9 -9l9 9l-2 0",
        "M5 12v7a2 2 0 0 0 2 2h10a2 2 0 0 0 2 -2v-7",
        "M9 21v-6a2 2 0 0 1 2 -2h2a2 2 0 0 1 2 2v6",
    ];
    tabler(p, cx, cy, sz, &paths, col);
}

/// Tabler's `puzzle`.
pub(super) fn draw_icon_mods(p: &mut Ui, cx: f32, cy: f32, sz: f32, col: u32) {
    let piece = "M4 7h3a1 1 0 0 0 1 -1v-1a2 2 0 0 1 4 0v1a1 1 0 0 0 1 1h3a1 1 0 0 1 1 1v3a1 1 0 0 0 1 1h1a2 2 0 0 1 0 4h-1a1 1 0 0 0 -1 1v3a1 1 0 0 1 -1 1h-3a1 1 0 0 1 -1 -1v-1a2 2 0 0 0 -4 0v1a1 1 0 0 1 -1 1h-3a1 1 0 0 1 -1 -1v-3a1 1 0 0 1 1 -1h1a2 2 0 0 0 0 -4h-1a1 1 0 0 1 -1 -1v-3a1 1 0 0 1 1 -1";
    tabler(p, cx, cy, sz, &[piece], col);
}

/// Tabler's `shirt`.
pub(super) fn draw_icon_skins(p: &mut Ui, cx: f32, cy: f32, sz: f32, col: u32) {
    let shirt = "M15 4l6 2v5h-3v8a1 1 0 0 1 -1 1h-10a1 1 0 0 1 -1 -1v-8h-3v-5l6 -2a3 3 0 0 0 6 0";
    tabler(p, cx, cy, sz, &[shirt], col);
}

/// Tabler's `settings`.
pub(super) fn draw_icon_settings(p: &mut Ui, cx: f32, cy: f32, sz: f32, col: u32) {
    let paths = [
        "M10.325 4.317c.426 -1.756 2.924 -1.756 3.35 0a1.724 1.724 0 0 0 2.573 1.066c1.543 -.94 3.31 .826 2.37 2.37a1.724 1.724 0 0 0 1.065 2.572c1.756 .426 1.756 2.924 0 3.35a1.724 1.724 0 0 0 -1.066 2.573c.94 1.543 -.826 3.31 -2.37 2.37a1.724 1.724 0 0 0 -2.572 1.065c-.426 1.756 -2.924 1.756 -3.35 0a1.724 1.724 0 0 0 -2.573 -1.066c-1.543 .94 -3.31 -.826 -2.37 -2.37a1.724 1.724 0 0 0 -1.065 -2.572c-1.756 -.426 -1.756 -2.924 0 -3.35a1.724 1.724 0 0 0 1.066 -2.573c-.94 -1.543 .826 -3.31 2.37 -2.37c1 .608 2.296 .07 2.572 -1.065z",
        "M9 12a3 3 0 1 0 6 0a3 3 0 0 0 -6 0",
    ];
    tabler(p, cx, cy, sz, &paths, col);
}

/// An SVG path's pieces as points, each with whether it closes: the commands Tabler's outlines
/// use (M L H V C A Z, absolute or relative, a command's letter repeated or left out), curves and
/// arcs cut into short lines.
pub(super) fn svg_path(d: &str) -> Vec<(Vec<[f32; 2]>, bool)> {
    // Its letters and numbers, in order ("-.94" and ".826" are numbers; "1.5.5" is two).
    let mut tokens: Vec<Result<char, f32>> = Vec::new();
    let mut chars = d.char_indices().peekable();
    while let Some(&(i, c)) = chars.peek() {
        if c.is_ascii_alphabetic() {
            tokens.push(Ok(c));
            chars.next();
        } else if c == '-' || c == '.' || c.is_ascii_digit() {
            let mut end = i + c.len_utf8();
            let mut dot = c == '.';
            chars.next();
            while let Some(&(j, n)) = chars.peek() {
                if n.is_ascii_digit() || (n == '.' && !dot) {
                    dot |= n == '.';
                    end = j + 1;
                    chars.next();
                } else {
                    break;
                }
            }
            tokens.push(Err(d[i..end].parse().unwrap_or(0.0)));
        } else {
            chars.next();
        }
    }
    let mut pieces: Vec<(Vec<[f32; 2]>, bool)> = Vec::new();
    let (mut at, mut start) = ([0.0f32, 0.0], [0.0f32, 0.0]);
    let mut command = 'M';
    let mut i = 0;
    let take = |i: &mut usize, n: usize| -> Option<Vec<f32>> {
        let v: Vec<f32> = tokens
            .get(*i..*i + n)?
            .iter()
            .map(|t| t.err())
            .collect::<Option<_>>()?;
        *i += n;
        Some(v)
    };
    while i < tokens.len() {
        if let Ok(c) = tokens[i] {
            command = c;
            i += 1;
        }
        let rel = command.is_ascii_lowercase();
        let base = if rel { at } else { [0.0, 0.0] };
        let line = |pieces: &mut Vec<(Vec<[f32; 2]>, bool)>, to: [f32; 2]| {
            if let Some((points, _)) = pieces.last_mut() {
                points.push(to);
            }
        };
        match command.to_ascii_uppercase() {
            'M' => {
                let Some(v) = take(&mut i, 2) else { break };
                at = [base[0] + v[0], base[1] + v[1]];
                start = at;
                pieces.push((vec![at], false));
                // Numbers after a move are lines.
                command = if rel { 'l' } else { 'L' };
            }
            'L' => {
                let Some(v) = take(&mut i, 2) else { break };
                at = [base[0] + v[0], base[1] + v[1]];
                line(&mut pieces, at);
            }
            'H' => {
                let Some(v) = take(&mut i, 1) else { break };
                at[0] = base[0] + v[0];
                line(&mut pieces, at);
            }
            'V' => {
                let Some(v) = take(&mut i, 1) else { break };
                at[1] = base[1] + v[0];
                line(&mut pieces, at);
            }
            'C' => {
                let Some(v) = take(&mut i, 6) else { break };
                let p0 = at;
                let [p1, p2, p3] = [0, 2, 4].map(|k| [base[0] + v[k], base[1] + v[k + 1]]);
                for step in 1..=6 {
                    let t = step as f32 / 6.0;
                    let u = 1.0 - t;
                    let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
                    let q =
                        [0, 1].map(|k| w[0] * p0[k] + w[1] * p1[k] + w[2] * p2[k] + w[3] * p3[k]);
                    line(&mut pieces, q);
                }
                at = p3;
            }
            'A' => {
                let Some(v) = take(&mut i, 7) else { break };
                let to = [base[0] + v[5], base[1] + v[6]];
                for q in arc_points(at, to, v[0], v[1], v[2], v[3] != 0.0, v[4] != 0.0) {
                    line(&mut pieces, q);
                }
                at = to;
            }
            'Z' => {
                if let Some((_, closed)) = pieces.last_mut() {
                    *closed = true;
                }
                at = start;
                // Only a letter may follow.
                if matches!(tokens.get(i), Some(Err(_))) {
                    break;
                }
            }
            // A command these outlines don't use: stop rather than draw it wrong.
            _ => break,
        }
    }
    pieces
}

/// SVG's elliptical arc from `from` to `to` as points after `from` (its endpoint form turned into
/// a centre and angles, SVG 1.1 F.6.5), a point every 22.5 degrees or less.
fn arc_points(
    from: [f32; 2],
    to: [f32; 2],
    rx: f32,
    ry: f32,
    degrees: f32,
    large: bool,
    sweep: bool,
) -> Vec<[f32; 2]> {
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx == 0.0 || ry == 0.0 || from == to {
        return vec![to];
    }
    let (sin, cos) = degrees.to_radians().sin_cos();
    let (hx, hy) = ((from[0] - to[0]) / 2.0, (from[1] - to[1]) / 2.0);
    let (x1, y1) = (cos * hx + sin * hy, -sin * hx + cos * hy);
    let grow = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
    if grow > 1.0 {
        rx *= grow.sqrt();
        ry *= grow.sqrt();
    }
    let num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let k = (num / den).max(0.0).sqrt() * if large == sweep { -1.0 } else { 1.0 };
    let (cx1, cy1) = (k * rx * y1 / ry, -k * ry * x1 / rx);
    let (cx, cy) = (
        cos * cx1 - sin * cy1 + (from[0] + to[0]) / 2.0,
        sin * cx1 + cos * cy1 + (from[1] + to[1]) / 2.0,
    );
    let angle = |ux: f32, uy: f32| uy.atan2(ux);
    let start = angle((x1 - cx1) / rx, (y1 - cy1) / ry);
    let mut delta = angle((-x1 - cx1) / rx, (-y1 - cy1) / ry) - start;
    let tau = std::f32::consts::TAU;
    if sweep && delta < 0.0 {
        delta += tau;
    } else if !sweep && delta > 0.0 {
        delta -= tau;
    }
    let steps = (delta.abs() / (tau / 16.0)).ceil().max(1.0) as usize;
    (1..=steps)
        .map(|n| {
            let (s, c) = (start + delta * n as f32 / steps as f32).sin_cos();
            [
                cx + rx * c * cos - ry * s * sin,
                cy + rx * c * sin + ry * s * cos,
            ]
        })
        .collect()
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
        let texel = |row: usize, col: usize| {
            let i = (row * 8 + col) * 4;
            let rgb =
                (u32::from(rgba[i]) << 16) | (u32::from(rgba[i + 1]) << 8) | u32::from(rgba[i + 2]);
            color::hex(rgb, rgba[i + 3])
        };
        // Rounded like the cells around it: each corner texel is a rounded square two texels
        // wide from its corner, the texels drawn over it leaving only its own rounded corner.
        let radius = step.min(size * 0.2);
        for (row, col) in [(0, 0), (0, 7), (7, 0), (7, 7)] {
            p.rounded(
                x + if col == 0 { 0.0 } else { size - 2.0 * step },
                y + if row == 0 { 0.0 } else { size - 2.0 * step },
                2.0 * step,
                2.0 * step,
                radius,
                texel(row, col),
            );
        }
        for row in 0..8 {
            for col in 0..8 {
                if matches!((row, col), (0 | 7, 0 | 7)) {
                    continue;
                }
                // Rounded up to cover the seams, but never past the head's own edge.
                let (tx, ty) = (x + col as f32 * step, y + row as f32 * step);
                p.rounded(
                    tx,
                    ty,
                    step.ceil().min(x + size - tx),
                    step.ceil().min(y + size - ty),
                    0.0,
                    texel(row, col),
                );
            }
        }
        p.border(x, y, size, size, radius, palette::border());
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
        let room = (x + w - 12.0 - tx).max(20.0);
        let size = ui_theme::sizes().ui;
        let fit = if focused {
            // Typing: the end, where the caret is, stays in sight.
            let mut tail: Vec<char> = format!("{shown}|").chars().collect();
            while tail.len() > 1
                && p.measure(Face::Regular, size, &tail.iter().collect::<String>()) > room
            {
                tail.remove(0);
            }
            tail.into_iter().collect()
        } else {
            truncate_to_fit(p, Face::Regular, size, &shown, room)
        };
        p.text_line(Face::Regular, size, h, tx, y, palette::text(), &fit);
    }
    hits.push(hit);
}

/// The player model's six parts and their outer layers, in skin pixels with y up and the figure
/// facing +z: where each sits in the skin (u, v), its size (w, h, d), its centre, and how far
/// the outer layer stands off it (hat, jacket, sleeves, trousers).
fn player_cuboids(slim: bool) -> Vec<((usize, usize), (usize, usize, usize), [f32; 3], f32)> {
    let arm = if slim { 3 } else { 4 };
    let half_arm = arm as f32 / 2.0;
    let parts = [
        ((0, 0), (32, 0), (8, 8, 8), [0.0, 28.0, 0.0], 0.5),
        ((16, 16), (16, 32), (8, 12, 4), [0.0, 18.0, 0.0], 0.25),
        (
            (40, 16),
            (40, 32),
            (arm, 12, 4),
            [-4.0 - half_arm, 18.0, 0.0],
            0.25,
        ),
        (
            (32, 48),
            (48, 48),
            (arm, 12, 4),
            [4.0 + half_arm, 18.0, 0.0],
            0.25,
        ),
        ((0, 16), (0, 32), (4, 12, 4), [-2.0, 6.0, 0.0], 0.25),
        ((16, 48), (0, 48), (4, 12, 4), [2.0, 6.0, 0.0], 0.25),
    ];
    parts
        .into_iter()
        .flat_map(|(base, outer, size, centre, grow)| {
            [(base, size, centre, 0.0), (outer, size, centre, grow)]
        })
        .collect()
}

/// A skin on the player model turned `yaw` about its height and tilted `pitch` toward the
/// viewer from above: one quad per visible skin pixel, as (corners in skin pixels about the
/// figure's middle with y down, depth toward the viewer, colour), farthest first. Bottoms are
/// left out: a viewer level with the figure or above it never sees them.
pub(super) fn skin_quads(
    skin: &[u8],
    slim: bool,
    yaw: f32,
    pitch: f32,
) -> Vec<([[f32; 2]; 4], f32, u32)> {
    let mut quads = Vec::new();
    if skin.len() < 64 * 64 * 4 {
        return quads;
    }
    let (sin_y, cos_y) = yaw.sin_cos();
    let (sin_p, cos_p) = pitch.sin_cos();
    // Into the view: screen x, screen y (down), depth (toward the viewer).
    let turn = |[x, y, z]: [f32; 3]| {
        let xr = x * cos_y + z * sin_y;
        let zr = -x * sin_y + z * cos_y;
        [xr, -(y * cos_p - zr * sin_p), y * sin_p + zr * cos_p]
    };
    let add =
        |a: [f32; 3], b: [f32; 3], t: f32| [a[0] + b[0] * t, a[1] + b[1] * t, a[2] + b[2] * t];
    for ((u, v), (w, h, d), c, grow) in player_cuboids(slim) {
        let (hx, hy, hz) = (
            w as f32 / 2.0 + grow,
            h as f32 / 2.0 + grow,
            d as f32 / 2.0 + grow,
        );
        // About the figure's middle (it is 32 pixels tall).
        let (x0, x1, y0, y1, z0, z1) = (
            c[0] - hx,
            c[0] + hx,
            c[1] - 16.0 - hy,
            c[1] - 16.0 + hy,
            -hz,
            hz,
        );
        let (sx, sy, sz) = (x1 - x0, y1 - y0, z1 - z0);
        // Each face: where it sits in the skin, its size there, its top-left corner, the
        // directions of its pixel columns and rows, and its normal.
        let faces = [
            (
                (u + d, v + d),
                (w, h),
                [x0, y1, z1],
                [sx, 0.0, 0.0],
                [0.0, -sy, 0.0],
                [0.0, 0.0, 1.0],
            ),
            (
                (u + 2 * d + w, v + d),
                (w, h),
                [x1, y1, z0],
                [-sx, 0.0, 0.0],
                [0.0, -sy, 0.0],
                [0.0, 0.0, -1.0],
            ),
            (
                (u, v + d),
                (d, h),
                [x0, y1, z0],
                [0.0, 0.0, sz],
                [0.0, -sy, 0.0],
                [-1.0, 0.0, 0.0],
            ),
            (
                (u + d + w, v + d),
                (d, h),
                [x1, y1, z1],
                [0.0, 0.0, -sz],
                [0.0, -sy, 0.0],
                [1.0, 0.0, 0.0],
            ),
            (
                (u + d, v),
                (w, d),
                [x0, y1, z0],
                [sx, 0.0, 0.0],
                [0.0, 0.0, sz],
                [0.0, 1.0, 0.0],
            ),
        ];
        for ((tu, tv), (tw, th), origin, cols, rows, normal) in faces {
            let n = turn(normal);
            if n[2] <= 1e-3 {
                continue;
            }
            // Lit from the viewer's upper side, so the faces stay apart as it turns.
            let shade = 0.5 + 0.5 * (0.6 * -n[1] + 0.8 * n[2]).clamp(0.0, 1.0);
            for j in 0..th {
                for i in 0..tw {
                    let at = ((tv + j) * 64 + tu + i) * 4;
                    let a = skin[at + 3];
                    if a == 0 {
                        continue;
                    }
                    let lit = |c: u8| (f32::from(c) * shade).round() as u32;
                    let rgb = (lit(skin[at]) << 16) | (lit(skin[at + 1]) << 8) | lit(skin[at + 2]);
                    let (fi, fj) = (i as f32 / tw as f32, j as f32 / th as f32);
                    let (di, dj) = (1.0 / tw as f32, 1.0 / th as f32);
                    let corner = |a: f32, b: f32| turn(add(add(origin, cols, a), rows, b));
                    let c = [
                        corner(fi, fj),
                        corner(fi + di, fj),
                        corner(fi + di, fj + dj),
                        corner(fi, fj + dj),
                    ];
                    let depth = c.iter().map(|p| p[2]).sum::<f32>() / 4.0;
                    quads.push((c.map(|p| [p[0], p[1]]), depth, color::hex(rgb, a)));
                }
            }
        }
    }
    quads.sort_by(|a, b| a.1.total_cmp(&b.1));
    quads
}

/// The skin on the player model ([`skin_quads`]), its middle at (`cx`, `cy`), `texel` pixels
/// to a skin pixel.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_skin_3d(
    p: &mut Ui,
    cx: f32,
    cy: f32,
    texel: f32,
    skin: &[u8],
    slim: bool,
    yaw: f32,
    pitch: f32,
) {
    let k = p.density / p.batch.scale;
    for (corners, _, col) in skin_quads(skin, slim, yaw, pitch) {
        let at = corners.map(|[x, y]| [(cx + x * texel) * k, (cy + y * texel) * k]);
        p.batch.fill_quad(p.atlas, at, col);
    }
}

/// A flat 64x64 RGBA skin whose six body parts are filled and outer layers are empty, for an
/// empty skin slot's silhouette.
pub(super) fn blank_skin() -> &'static [u8; 64 * 64 * 4] {
    static SKIN: std::sync::OnceLock<[u8; 64 * 64 * 4]> = std::sync::OnceLock::new();
    SKIN.get_or_init(|| {
        let mut rgba = [0u8; 64 * 64 * 4];
        for (u0, v0, w, h) in [(0, 0, 32, 16), (0, 16, 56, 16), (16, 48, 32, 16)] {
            for y in v0..v0 + h {
                for x in u0..u0 + w {
                    rgba[(y * 64 + x) * 4..][..4].copy_from_slice(&[113, 113, 122, 88]);
                }
            }
        }
        rgba
    })
}

/// Tabler's `pencil`.
pub(super) fn draw_icon_pencil(p: &mut Ui, cx: f32, cy: f32, size: f32, col: u32) {
    let paths = [
        "M4 20h4l10.5 -10.5a2.828 2.828 0 1 0 -4 -4l-10.5 10.5v4",
        "M13.5 6.5l4 4",
    ];
    tabler(p, cx, cy, size, &paths, col);
}

/// Tabler's `trash`.
pub(super) fn draw_icon_trash(p: &mut Ui, cx: f32, cy: f32, size: f32, col: u32) {
    let paths = [
        "M4 7l16 0",
        "M10 11l0 6",
        "M14 11l0 6",
        "M5 7l1 12a2 2 0 0 0 2 2h8a2 2 0 0 0 2 -2l1 -12",
        "M9 7v-3a1 1 0 0 1 1 -1h4a1 1 0 0 1 1 1v3",
    ];
    tabler(p, cx, cy, size, &paths, col);
}

/// The shown modded profile's tab: one cell with its name, then its pencil (rename) and bin
/// (delete), each its own tap.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_tab_with_tools(
    ui: &mut Canvas<'_, '_>,
    hits: &mut Vec<HitRect>,
    name_id: ButtonId,
    rename_id: ButtonId,
    delete_id: ButtonId,
    x: f32,
    y: f32,
    name_w: f32,
    tool_w: f32,
    h: f32,
    label: &str,
) {
    let w = name_w + 2.0 * tool_w;
    if let Some(p) = ui.paint() {
        p.rounded(x, y, w, h, 10.0, color::with_alpha(palette::accent(), 0.22));
        p.border(x, y, w, h, 10.0, palette::accent_star());
        let room = (name_w - 16.0).max(16.0);
        let size = p.fit(Face::Medium, ui_theme::sizes().ui, label, room);
        let label = truncate_to_fit(p, Face::Medium, size, label, room);
        let tw = p.measure(Face::Medium, size, &label);
        p.text_line(
            Face::Medium,
            size,
            h,
            x + ((name_w - tw) * 0.5).round(),
            y,
            palette::text(),
            &label,
        );
        // A hairline between the name and its tools.
        p.rounded(
            x + name_w,
            y + h * 0.25,
            1.0,
            h * 0.5,
            0.0,
            color::with_alpha(palette::accent_star(), 0.45),
        );
        let icon = (h * 0.46).min(18.0);
        draw_icon_pencil(
            p,
            x + name_w + tool_w * 0.5,
            y + h * 0.5,
            icon,
            palette::body(),
        );
        let danger = color::hex(menu_style::get().settings.danger, 255);
        draw_icon_trash(p, x + name_w + tool_w * 1.5, y + h * 0.5, icon, danger);
    }
    hits.push(HitRect {
        id: name_id,
        x,
        y,
        w: name_w,
        h,
    });
    hits.push(HitRect {
        id: rename_id,
        x: x + name_w,
        y,
        w: tool_w,
        h,
    });
    hits.push(HitRect {
        id: delete_id,
        x: x + name_w + tool_w,
        y,
        w: tool_w,
        h,
    });
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
            // One step lighter than the surface rows it sits on, with an edge that shows.
            (Tone::Secondary, false) => (
                palette::surface_hover(),
                Some(color::with_alpha(palette::text(), 0.18)),
                palette::body(),
            ),
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
        let label = truncate_to_fit(p, Face::Medium, font_size, label, room);
        let tw = p.measure(Face::Medium, font_size, &label);
        let lx = x + ((w - tw) * 0.5).round();
        p.text_line(Face::Medium, font_size, h, lx, y, ink, &label);
    }
    hits.push(hit);
}

/// Two choices side by side (`right` chosen when true); each half with an id in `ids` takes taps.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_switch(
    ui: &mut Canvas<'_, '_>,
    hits: &mut Vec<HitRect>,
    ids: [Option<ButtonId>; 2],
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    labels: [&str; 2],
    right: bool,
) {
    let half = (w * 0.5).round();
    if let Some(p) = ui.paint() {
        p.rounded(x, y, w, h, 10.0, palette::surface());
        p.border(x, y, w, h, 10.0, palette::border());
        let (chosen_x, chosen_w) = if right { (x + half, w - half) } else { (x, half) };
        let chosen = color::with_alpha(palette::accent(), 0.26);
        p.rounded(
            chosen_x + 3.0,
            y + 3.0,
            chosen_w - 6.0,
            h - 6.0,
            7.0,
            chosen,
        );
        let ui_size = ui_theme::sizes().ui;
        for (i, label) in labels.into_iter().enumerate() {
            let (hx, hw) = if i == 0 { (x, half) } else { (x + half, w - half) };
            let ink = if (i == 1) == right {
                palette::text()
            } else {
                palette::muted()
            };
            let room = (hw - 10.0).max(16.0);
            let size = p.fit(Face::Medium, ui_size, label, room);
            let fit = truncate_to_fit(p, Face::Medium, size, label, room);
            let tw = p.measure(Face::Medium, size, &fit);
            let lx = hx + ((hw - tw) * 0.5).round();
            p.text_line(Face::Medium, size, h, lx, y, ink, &fit);
        }
    }
    for (i, id) in ids.into_iter().enumerate() {
        if let Some(id) = id {
            let (hx, hw) = if i == 0 { (x, half) } else { (x + half, w - half) };
            hits.push(HitRect {
                id,
                x: hx,
                y,
                w: hw,
                h,
            });
        }
    }
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
) -> Option<std::ops::Range<usize>> {
    let hit = HitRect { id, x, y, w, h };
    let mut piece = None;
    if let Some(p) = ui.paint() {
        let piece_start = p.batch.vertices.len();
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
            // The selection (glow, plate, edge and mark), which slides between items.
            piece = Some(piece_start..p.batch.vertices.len());
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
    piece
}

pub(super) fn count_label(n: usize, singular: &str, plural: &str) -> String {
    if n == 1 {
        format!("1 {singular}")
    } else {
        format!("{n} {plural}")
    }
}

pub(super) fn profile_stats_line(profile: &Profile) -> String {
    let counts = format!(
        "{}, {}",
        count_label(profile.mods, "mod", "mods"),
        count_label(profile.worlds, "world", "worlds")
    );
    match profile.size {
        0 => counts,
        bytes => format!("{counts}, {}", format_size(bytes)),
    }
}

/// Each game log line's level: 0 information, 1 a warning, 2 an error, from its `[thread/LEVEL]`
/// tag as the desktop launcher reads it (launcher/src/console_page.rs); a line without a tag
/// goes on with the entry above (a stack trace, a long message) and takes its level.
pub(super) fn log_levels(lines: &[String]) -> Vec<u8> {
    let mut level = 0;
    lines
        .iter()
        .map(|line| {
            if line.starts_with('[') {
                level = if line.contains("/ERROR]") || line.contains("/FATAL]") {
                    2
                } else {
                    u8::from(line.contains("/WARN]"))
                };
            }
            level
        })
        .collect()
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
        ProfileState::Available => ("Needs install".to_owned(), palette::accent_star()),
        ProfileState::Installing(p) => {
            let pct = (stage_fraction(p) * 100.0).round() as u32;
            (format!("Installing {pct}%"), palette::accent())
        }
        ProfileState::Installed if selected => ("Ready".to_owned(), col_success),
        ProfileState::Installed => ("Installed".to_owned(), palette::muted()),
        ProfileState::Failed(_) => ("Failed".to_owned(), col_danger),
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
        Account::Microsoft { .. } => "Microsoft",
        Account::Ely { .. } => "Ely.by",
        Account::Offline { .. } => "Offline",
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
                format!("{pct}%  ·  {speed:.1} MB/s  ·  {eta} s left")
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
