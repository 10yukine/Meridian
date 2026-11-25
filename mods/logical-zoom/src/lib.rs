//! Smooth optical hold-to-zoom for Horizon & Meridian (inspired by Logical Zoom).
use mod_api::{
    CalculatePlayerTurnEvent, ClientHost, ClientTickPreEvent, ComputeFovEvent, Flow, Hooks,
    KeyConflictContext, KeyRegistration, Registrar, RegistrationContext, RenderHandEvent,
};
use std::sync::atomic::{AtomicU32, Ordering};

const ID: &str = "logical_zoom";
const KEY: &str = "zoom";
const PROJECTION_SCALE: f32 = 0.23;
const SMOOTH_FACTOR: f32 = 0.42;
const SNAP_EPSILON: f32 = 0.002;

#[derive(Default)]
struct LogicalZoom {
    prev_amount: AtomicU32,
    curr_amount: AtomicU32,
}

fn active(host: &dyn ClientHost) -> bool {
    host.playing() && host.on(ID) && host.is_down(ID, KEY)
}

fn step_zoom(curr: f32, zooming: bool) -> f32 {
    let target = if zooming { 1.0 } else { 0.0 };
    let next = curr + (target - curr) * SMOOTH_FACTOR;
    if (target - next).abs() < SNAP_EPSILON {
        target
    } else {
        next
    }
}

fn zoom_fov_at(degrees: f32, amount: f32) -> f32 {
    let amount = amount.clamp(0.0, 1.0);
    if amount <= 0.0 || !degrees.is_finite() || !(1.0..=179.0).contains(&degrees) {
        return degrees;
    }
    let scale = PROJECTION_SCALE.powf(amount);
    // The host refuses angles below one degree, so clamp exceptionally narrow views.
    (2.0 * (scale * (degrees.to_radians() / 2.0).tan()).atan())
        .to_degrees()
        .max(1.0)
}

#[mod_api::horizon_mod]
impl Hooks for LogicalZoom {
    fn register(&self, registrar: &mut dyn Registrar, _: &RegistrationContext) {
        registrar.register_key_mapping(
            KeyRegistration::new(KEY, "key.keyboard.c")
                .label("key.logical_zoom.zoom")
                .context(KeyConflictContext::InGame),
        );
    }

    fn client_tick_pre(&self, host: &mut dyn ClientHost, _: &mut ClientTickPreEvent) {
        let curr = f32::from_bits(self.curr_amount.load(Ordering::Relaxed));
        let next = step_zoom(curr, active(host));
        self.prev_amount.store(curr.to_bits(), Ordering::Relaxed);
        self.curr_amount.store(next.to_bits(), Ordering::Relaxed);
    }

    fn compute_fov(&self, _: &dyn ClientHost, event: &mut ComputeFovEvent) {
        let prev = f32::from_bits(self.prev_amount.load(Ordering::Relaxed));
        let curr = f32::from_bits(self.curr_amount.load(Ordering::Relaxed));
        let partial = if event.partial.is_finite() {
            event.partial.clamp(0.0, 1.0)
        } else {
            1.0
        };
        let amount = prev + (curr - prev) * partial;
        if amount > 0.0 {
            event.degrees = zoom_fov_at(event.degrees, amount);
        }
    }

    fn render_hand(&self, host: &dyn ClientHost, _: &mut RenderHandEvent<'_, '_>) -> Flow {
        let amount = f32::from_bits(self.curr_amount.load(Ordering::Relaxed));
        if active(host) || amount > 0.15 {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }

    fn calculate_player_turn(&self, host: &dyn ClientHost, event: &mut CalculatePlayerTurnEvent) {
        if active(host) {
            event.cinematic_camera_enabled = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_scales_and_obeys_host_floor() {
        let actual = zoom_fov_at(70.0, 1.0);
        assert!((actual - 18.3007).abs() < 0.01, "{actual}");
        let ratio = (actual.to_radians() / 2.0).tan() / (70_f32.to_radians() / 2.0).tan();
        assert!((ratio - 0.23).abs() < 1e-6);
        assert_eq!(zoom_fov_at(70.0, 0.0), 70.0);
        let mid = zoom_fov_at(70.0, 0.5);
        assert!(mid > actual && mid < 70.0, "{mid}");
        assert_eq!(zoom_fov_at(1.0, 1.0), 1.0);
        assert!(zoom_fov_at(f32::NAN, 1.0).is_nan());
        assert_eq!(zoom_fov_at(180.0, 1.0), 180.0);
    }

    #[test]
    fn zoom_eases_smoothly_in_and_out_to_exact_endpoints() {
        let mut amount = 0.0;
        let first = step_zoom(amount, true);
        assert!(first > 0.0 && first < 1.0, "smooth ramp in: {first}");
        amount = first;
        for _ in 0..20 {
            amount = step_zoom(amount, true);
        }
        assert_eq!(amount, 1.0);

        let out = step_zoom(amount, false);
        assert!(out < 1.0 && out > 0.0, "smooth ramp out: {out}");
        amount = out;
        for _ in 0..20 {
            amount = step_zoom(amount, false);
        }
        assert_eq!(amount, 0.0);
    }
}
