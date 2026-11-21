//! The launcher's motion: a change starts a short eased move over what was already drawn
//! (fade, move, scale of the batch's vertices). Hit areas stay where things end up. Settings'
//! Animations switch turns it off, and then everything changes at once.

use super::widgets::Canvas;
use crate::ui_theme::Ui;
use std::cell::Cell;
use std::ops::Range;
use std::time::{Duration, Instant};

/// A moment far enough back that a motion from it has ended: nothing moves at startup.
pub(super) fn long_ago() -> Instant {
    let now = Instant::now();
    now.checked_sub(Duration::from_secs(60)).unwrap_or(now)
}

/// When `value` last changed, and what it was before: `cell` keeps the value, the one before
/// it and the moment it changed.
pub(super) fn changed<T: Copy + PartialEq>(cell: &Cell<(T, T, Instant)>, value: T) -> (T, Instant) {
    let (now, before, since) = cell.get();
    if now == value {
        return (before, since);
    }
    cell.set((value, now, Instant::now()));
    (now, Instant::now())
}

/// The launcher's motion state, kept across frames.
pub(super) struct Motion {
    /// Settings' Animations switch.
    pub on: Cell<bool>,
    /// Until when something moves: the host draws every frame until then.
    until: Cell<Option<Instant>>,
}

impl Motion {
    pub(super) fn new(on: bool) -> Self {
        Self {
            on: Cell::new(on),
            until: Cell::new(None),
        }
    }

    /// How far a motion begun at `since`, starting `delay` seconds later and lasting `secs`, has
    /// gone, eased out (0 to 1). While it runs, the launcher keeps drawing; with animations off it
    /// is 1 at once.
    pub(super) fn ease(&self, since: Instant, delay: f32, secs: f32) -> f32 {
        if !self.on.get() {
            return 1.0;
        }
        let t = (since.elapsed().as_secs_f32() - delay) / secs;
        if t < 1.0 {
            let end = since + Duration::from_secs_f32(delay + secs);
            self.until
                .set(Some(self.until.get().map_or(end, |until| until.max(end))));
        }
        let t = t.clamp(0.0, 1.0);
        1.0 - (1.0 - t).powi(3)
    }

    /// Whether something still moves.
    pub(super) fn running(&self) -> bool {
        self.until.get().is_some_and(|until| Instant::now() < until)
    }
}

/// Where the batch's vertices end now: what is drawn after this belongs to one piece.
pub(super) fn mark(ui: &mut Canvas<'_, '_>) -> usize {
    ui.paint().map_or(0, |p| p.batch.vertices.len())
}

/// Scales what was drawn since `start` by `scale` about `(cx, cy)`, moves it by `(dx, dy)` (dp)
/// and multiplies its alpha by `alpha`.
#[allow(clippy::too_many_arguments)]
pub(super) fn shape(
    ui: &mut Canvas<'_, '_>,
    start: usize,
    alpha: f32,
    dx: f32,
    dy: f32,
    scale: f32,
    cx: f32,
    cy: f32,
) {
    if let Some(p) = ui.paint() {
        let end = p.batch.vertices.len();
        shape_range(p, start..end, alpha, dx, dy, scale, cx, cy);
    }
}

/// [`shape`] for a range of the batch's vertices.
#[allow(clippy::too_many_arguments)]
pub(super) fn shape_range(
    p: &mut Ui,
    range: Range<usize>,
    alpha: f32,
    dx: f32,
    dy: f32,
    scale: f32,
    cx: f32,
    cy: f32,
) {
    if alpha >= 1.0 && dx == 0.0 && dy == 0.0 && scale == 1.0 {
        return;
    }
    // dp to the batch's GUI pixels.
    let k = p.density / p.batch.scale;
    let (cx, cy, dx, dy) = (cx * k, cy * k, dx * k, dy * k);
    let alpha = alpha.clamp(0.0, 1.0);
    for v in &mut p.batch.vertices[range] {
        v.pos = [
            cx + (v.pos[0] - cx) * scale + dx,
            cy + (v.pos[1] - cy) * scale + dy,
        ];
        if alpha < 1.0 {
            let a = ((v.color >> 24) as f32 * alpha).round() as u32;
            v.color = (v.color & 0x00ff_ffff) | (a << 24);
        }
    }
}
