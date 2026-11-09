//! Survivors uses the streamed world's walk hulls and camera hulls, including WMO interiors.
use avian3d::character_controller::move_and_slide::{MoveAndSlideConfig, MoveAndSlideHitResponse};
use avian3d::prelude::Collider;
use benilla_world::{collision::WorldCollision, world_point::WorldPoint};
use bevy::{ecs::system::SystemParam, prelude::*};

#[derive(SystemParam)]
pub(super) struct Surface<'w, 's> {
    terrain: WorldPoint<'w, 's>,
    collision: WorldCollision<'w, 's>,
    body: Local<'s, BodyShape>,
}
struct BodyShape(Collider);
impl Default for BodyShape {
    fn default() -> Self {
        Self(Collider::capsule(0.35, 1.0))
    }
}
impl Surface<'_, '_> {
    pub(super) fn terrain_height_under(&self, feet: Vec3) -> Option<f32> {
        // Start just above the feet, never above a room's ceiling or the next storey.
        let origin = feet + Vec3::Y * 0.6;
        self.collision
            .ray_body(origin, Dir3::NEG_Y, 8.6)
            .filter(|hit| hit.normal.y > 0.4)
            .map(|hit| origin.y - hit.distance)
            .or_else(|| {
                self.terrain
                    .terrain_height_under(feet)
                    .filter(|height| (height - feet.y).abs() <= 8.0)
            })
    }

    pub(super) fn constrain(&self, from: Vec3, to: Vec3) -> Vec3 {
        let movement = to - from;
        if movement.length_squared() < 1e-10 {
            return to;
        }
        let offset = Vec3::Y;
        let slide = |start: Vec3, delta: Vec3| {
            self.collision
                .slide_body(
                    &self.body.0,
                    start + offset,
                    delta,
                    std::time::Duration::from_secs(1),
                    &MoveAndSlideConfig::default(),
                    |_| MoveAndSlideHitResponse::Accept,
                )
                .position
                - offset
        };
        let direct = slide(from, movement);
        let horizontal = Vec3::new(movement.x, 0.0, movement.z);
        // Let grounded walkers climb small stairs, but never step through a low ceiling.
        if horizontal.length_squared() > 1e-6
            && movement.y.abs() <= 0.6
            && self
                .terrain_height_under(from)
                .is_some_and(|floor| (floor - from.y).abs() < 0.2)
            && Vec3::new(direct.x - to.x, 0.0, direct.z - to.z).length_squared() > 0.0025
        {
            let lift = Vec3::Y * 0.6;
            if self
                .collision
                .cast_body(&self.body.0, from + offset, lift, 0.01)
                .is_none()
            {
                let raised = slide(from + lift, horizontal);
                if let Some(floor) = self.terrain_height_under(raised) {
                    let stepped = Vec3::new(raised.x, floor, raised.z);
                    let progress =
                        |p: Vec3| Vec3::new(p.x - from.x, 0.0, p.z - from.z).dot(horizontal);
                    if floor > from.y + 0.02
                        && floor <= from.y + 0.61
                        && progress(stepped) > progress(direct)
                    {
                        return stepped;
                    }
                }
            }
        }
        direct
    }

    pub(super) fn camera_eye(&self, look: Vec3, desired: Vec3) -> Vec3 {
        let boom = desired - look;
        let length = boom.length();
        match self.collision.cast_camera(look, boom, false) {
            Some(distance) if length > 0.0 => {
                look + boom / length * (distance - 0.05).max(0.05).min(length)
            }
            _ => desired,
        }
    }
}

#[cfg(test)]
pub(super) fn init_test_world(world: &mut World) {
    benilla_world::world_point::init_world_point_resources(world);
    world.init_resource::<avian3d::collider_tree::ColliderTrees>();
    world.init_resource::<avian3d::prelude::PhysicsLengthUnit>();
    world.init_resource::<benilla_world::collision::MoverTraceExclusions>();
}

#[cfg(test)]
mod tests {
    use super::*;
    use avian3d::prelude::*;
    use bevy::ecs::system::RunSystemOnce;

    fn room() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::transform::TransformPlugin,
            bevy::asset::AssetPlugin::default(),
            bevy::scene::ScenePlugin,
            PhysicsPlugins::new(PostUpdate),
        ));
        app.init_asset::<Mesh>();
        init_test_world(app.world_mut());
        for (size, position) in [
            (Vec3::new(40.0, 1.0, 40.0), Vec3::new(0.0, -0.5, 0.0)),
            (Vec3::new(40.0, 1.0, 40.0), Vec3::new(0.0, 4.5, 0.0)),
            (Vec3::new(1.0, 4.0, 40.0), Vec3::new(5.0, 2.0, 0.0)),
            (Vec3::new(2.0, 0.4, 4.0), Vec3::new(2.0, 0.2, -10.0)),
            // A solid trunk outside the room, on the upper storey.
            (Vec3::new(1.5, 8.0, 1.5), Vec3::new(-5.0, 9.0, 0.0)),
        ] {
            app.world_mut().spawn((
                RigidBody::Static,
                Collider::cuboid(size.x, size.y, size.z),
                Transform::from_translation(position),
            ));
        }
        app.finish();
        app.cleanup();
        app.update();
        app.update();
        app
    }

    #[test]
    fn jump_arc_on_instance_floors_matches_terrain_even_when_render_clamp_reseats_the_body() {
        use crate::survivors::{data, hero, session};
        let mut app = room();
        app.world_mut().run_system_once(|surface: Surface| {
            for floor in [0.0, 5.0] {
                for reseat in [false, true] {
                    for walking in [false, true] {
                        let mut actual = Transform::from_translation(Vec3::Y * floor);
                        let mut expected = actual;
                        let mut physics = hero::Hero { vy: 0.0, air: 0.0 };
                        let mut expected_physics = physics;
                        let mut movement = crate::creature_anim::MovementState::default();
                        let mut expected_movement = movement.clone();
                        let mut apex = 0.0_f32;
                        for frame in 0..60 {
                            if reseat { actual.translation.y = floor; }
                            let command = session::Command {
                                active: true, jump: frame == 0,
                                direction: if walking { Vec3::X } else { Vec3::ZERO }, ..default()
                            };
                            hero::step_motion(&mut expected, &mut expected_movement, &mut expected_physics,
                                command, 1.0 / 60.0, 2.0, &data::MAPS[data::DUNGEONS[0]], |_| Some(floor));
                            hero::step_world_motion(&mut actual, &mut movement, &mut physics,
                                command, 1.0 / 60.0, 2.0, &data::MAPS[data::DUNGEONS[0]], &surface);
                            assert!(actual.translation.distance(expected.translation) < 0.003,
                                "floor={floor}, reseat={reseat}, walking={walking}, frame={frame}: actual={:?}, expected={:?}",
                                actual.translation, expected.translation);
                            apex = apex.max(physics.air);
                        }
                        assert!(apex > 1.4);
                        assert!(physics.air < 0.003 && physics.vy == 0.0);
                    }
                }
            }
        }).unwrap();
    }

    #[test]
    fn a_real_low_ceiling_still_limits_the_jump_and_allows_a_normal_landing() {
        use crate::survivors::{data, hero, session};
        let mut app = room();
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(4.0, 0.5, 4.0),
            Transform::from_xyz(0.0, 2.65, 0.0),
        ));
        app.update();
        app.update();
        app.world_mut()
            .run_system_once(|surface: Surface| {
                let mut transform = Transform::default();
                let mut physics = hero::Hero { vy: 0.0, air: 0.0 };
                let mut movement = crate::creature_anim::MovementState::default();
                let mut apex = 0.0_f32;
                for frame in 0..60 {
                    transform.translation.y = 0.0;
                    hero::step_world_motion(
                        &mut transform,
                        &mut movement,
                        &mut physics,
                        session::Command {
                            active: true,
                            jump: frame == 0,
                            ..default()
                        },
                        1.0 / 60.0,
                        2.0,
                        &data::MAPS[data::DUNGEONS[0]],
                        &surface,
                    );
                    assert!(transform.translation.y >= -0.003);
                    apex = apex.max(physics.air);
                }
                assert!(apex > 0.3 && apex < 0.6, "apex={apex}");
                assert!(physics.air < 0.003 && physics.vy == 0.0);
            })
            .unwrap();
    }

    #[test]
    fn interior_floor_queries_stay_on_the_current_storey() {
        let mut app = room();
        app.world_mut()
            .run_system_once(|surface: Surface| {
                assert!((surface.terrain_height_under(Vec3::ZERO).unwrap()).abs() < 0.01);
                assert!((surface.terrain_height_under(Vec3::Y * 5.0).unwrap() - 5.0).abs() < 0.01);
                assert!(
                    surface
                        .terrain_height_under(Vec3::new(50.0, 0.0, 0.0))
                        .is_none()
                );
            })
            .unwrap();
    }

    #[test]
    fn bodies_stop_at_walls_and_trunks_and_slide_along_them() {
        let mut app = room();
        app.world_mut()
            .run_system_once(|surface: Surface| {
                let stopped = surface.constrain(Vec3::ZERO, Vec3::X * 10.0);
                assert!(stopped.x > 3.0 && stopped.x < 4.3, "{stopped:?}");
                let sliding = surface.constrain(Vec3::ZERO, Vec3::new(10.0, 0.0, 5.0));
                assert!(sliding.x < 4.3 && sliding.z > 2.0, "{sliding:?}");
                let trunk = surface.constrain(Vec3::new(-10.0, 5.0, 0.0), Vec3::new(0.0, 5.0, 0.0));
                assert!(trunk.x < -5.8, "{trunk:?}");
                let step =
                    surface.constrain(Vec3::new(0.0, 0.0, -10.0), Vec3::new(2.0, 0.4, -10.0));
                assert!(step.x > 1.8 && (step.y - 0.4).abs() < 0.03, "{step:?}");
            })
            .unwrap();
    }

    #[test]
    fn camera_boom_stays_inside_room_and_cannot_cross_tree_trunks() {
        let mut app = room();
        app.world_mut()
            .run_system_once(|surface: Surface| {
                let look = Vec3::Y * 1.4;
                let ceiling = surface.camera_eye(look, Vec3::Y * 20.0);
                assert!(ceiling.y > 2.0 && ceiling.y < 3.8, "{ceiling:?}");
                let wall = surface.camera_eye(look, look + Vec3::X * 20.0);
                assert!(wall.x < 4.3, "{wall:?}");
                let outside = Vec3::new(-10.0, 7.0, 0.0);
                assert!(surface.camera_eye(outside, outside + Vec3::X * 10.0).x < -5.8);
            })
            .unwrap();
    }
}
