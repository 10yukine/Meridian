// Adapted from Starling (https://github.com/starling-client/starling, 5432370d5736,
// `crates/horizon-client/src/touch.rs`), written by Vasysik and the Starling contributors.
// Copyright (c) 2026 Ludovic Marie and the Horizon contributors, under the same MIT license as
// Horizon (LICENSE at the repository root).

//! Aiming by turning the phone: what the gyroscope turned becomes the view's turn.

/// The view's turn, in degrees right and down, for what the gyroscope turned (radians about
/// the device's own x and y, those of its natural portrait) on a display at `rotation`
/// (Surface.ROTATION_0..3: 1 with the phone's top to the left). The phone is a window on the
/// world: turned left about the screen's up, the view turns left; its top tilted toward the
/// player (its back to the sky), the view looks up.
pub fn gyro_turn(turned: [f32; 2], rotation: i32) -> [f32; 2] {
    let [x, y] = turned;
    // Rotation about the screen's right and up axes.
    let (right, up) = match rotation.rem_euclid(4) {
        0 => (x, y),
        1 => (-y, x),
        2 => (-x, -y),
        _ => (y, -x),
    };
    [-up.to_degrees(), -right.to_degrees()]
}

#[cfg(test)]
mod tests {
    use super::gyro_turn;

    #[test]
    fn the_gyroscope_turns_the_view_as_the_phone_turns_on_any_side() {
        let close =
            |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).abs() < 1e-3 && (a[1] - b[1]).abs() < 1e-3;
        let quarter = std::f32::consts::FRAC_PI_2;
        // Landscape, top to the left (ROTATION_90): the screen's up is the device's x. Turned
        // left a quarter about it: the view turns 90 degrees left.
        assert!(close(gyro_turn([quarter, 0.0], 1), [-90.0, 0.0]));
        // The other landscape (ROTATION_270): the screen's up is the device's -x.
        assert!(close(gyro_turn([-quarter, 0.0], 3), [-90.0, 0.0]));
        // Top tilted toward the player about the screen's right (the device's -y at 90):
        // looking up.
        assert!(close(gyro_turn([0.0, -quarter], 1), [0.0, -90.0]));
        assert!(close(gyro_turn([0.0, quarter], 3), [0.0, -90.0]));
        // Portrait as it is.
        assert!(close(gyro_turn([0.0, quarter], 0), [-90.0, 0.0]));
    }
}
