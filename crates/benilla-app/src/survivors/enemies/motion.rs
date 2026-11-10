use bevy::prelude::*;

/// Pursuit ends at contact/stand-off range even when a slow frame permits a very long step.
pub(super) fn travel(
    velocity: Vec3,
    dt: f32,
    distance: f32,
    stand_off: f32,
    fleeing: bool,
) -> Vec3 {
    let movement = velocity * dt;
    if fleeing {
        movement
    } else {
        movement.clamp_length_max((distance - stand_off).max(0.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pursuit_does_not_overshoot_and_normal_motion_and_fleeing_keep_their_speed() {
        assert_eq!(
            travel(Vec3::X * 5.0, 0.01, 20.0, 2.0, false),
            Vec3::X * 5.0 * 0.01
        );
        assert!((travel(Vec3::X * 5000.0, 0.5, 20.0, 2.0, false).x - 18.0).abs() < 0.001);
        assert!((travel(Vec3::X * 5000.0, 0.5, 20.0, 8.0, false).x - 12.0).abs() < 0.001);
        assert_eq!(travel(Vec3::X * 5.0, 1.0, 1.0, 2.0, false), Vec3::ZERO);
        assert_eq!(travel(-Vec3::X * 5.0, 1.0, 1.0, 2.0, true), -Vec3::X * 5.0);
        assert_eq!(travel(Vec3::ZERO, 1.0, 20.0, 2.0, false), Vec3::ZERO);
    }
}
