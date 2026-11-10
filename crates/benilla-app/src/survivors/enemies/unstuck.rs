//! A blocked chase gets one second without obstacle collision after one second without progress.
use super::*;

const BLOCKED_SECONDS: f32 = 1.0;
const PHASE_SECONDS: f32 = 1.0;
const PROGRESS_DISTANCE: f32 = 0.05;

#[derive(Default)]
pub(in crate::survivors) struct Tracker(HashMap<Entity, State>);

struct State {
    anchor: Vec3,
    blocked: f32,
    phase_left: f32,
}

impl Tracker {
    pub fn retain(&mut self, mut exists: impl FnMut(Entity) -> bool) {
        self.0.retain(|entity, _| exists(*entity));
    }

    pub fn reset(&mut self, entity: Entity) {
        self.0.remove(&entity);
    }

    pub fn resolve(
        &mut self,
        entity: Entity,
        from: Vec3,
        desired: Vec3,
        trying_to_walk: bool,
        dt: f32,
        mut collide: impl FnMut(Vec3, Vec3) -> Vec3,
    ) -> Vec3 {
        if !trying_to_walk {
            self.reset(entity);
            return collide(from, desired);
        }
        let state = self.0.entry(entity).or_insert(State {
            anchor: from,
            blocked: 0.0,
            phase_left: 0.0,
        });
        if state.phase_left > 0.0 {
            let phase_time = state.phase_left.min(dt);
            let phase_end = from.lerp(desired, phase_time / dt);
            state.phase_left = (state.phase_left - phase_time).max(0.0);
            let next = if phase_time < dt {
                collide(phase_end, desired)
            } else {
                desired
            };
            state.anchor = next;
            state.blocked = 0.0;
            return next;
        }
        let next = collide(from, desired);
        if flat(next - state.anchor).length_squared() > PROGRESS_DISTANCE * PROGRESS_DISTANCE {
            state.anchor = next;
            state.blocked = 0.0;
        } else {
            state.blocked += dt;
            if state.blocked + 1.0e-5 >= BLOCKED_SECONDS {
                state.blocked = 0.0;
                state.phase_left = PHASE_SECONDS;
            }
        }
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blocked_chase_waits_one_second_then_phases_for_exactly_one_second() {
        let mut tracker = Tracker::default();
        let entity = Entity::from_raw_u32(1).unwrap();
        let mut position = Vec3::ZERO;
        for _ in 0..10 {
            position = tracker.resolve(
                entity,
                position,
                position + Vec3::X * 0.1,
                true,
                0.1,
                |from, _| from,
            );
            assert_eq!(position, Vec3::ZERO);
        }
        position = tracker.resolve(
            entity,
            position,
            position + Vec3::X * 0.75,
            true,
            0.75,
            |from, _| from,
        );
        assert!((position.x - 0.75).abs() < 1.0e-5);
        position = tracker.resolve(
            entity,
            position,
            position + Vec3::X * 0.75,
            true,
            0.75,
            |from, _| from,
        );
        assert!(
            (position.x - 1.0).abs() < 1.0e-5,
            "collision resumes inside a long frame"
        );
        let stopped = tracker.resolve(
            entity,
            position,
            position + Vec3::X,
            true,
            0.1,
            |from, _| from,
        );
        assert_eq!(stopped, position);
    }

    #[test]
    fn walking_and_intentional_stops_do_not_accumulate_a_blocked_chase() {
        let mut tracker = Tracker::default();
        let entity = Entity::from_raw_u32(1).unwrap();
        let mut position = Vec3::ZERO;
        for _ in 0..200 {
            position = tracker.resolve(
                entity,
                position,
                position + Vec3::X * 0.02,
                true,
                0.1,
                |_, to| to,
            );
            assert_eq!(tracker.0[&entity].phase_left, 0.0);
        }
        for _ in 0..30 {
            tracker.resolve(entity, position, position, false, 0.1, |from, _| from);
            assert!(!tracker.0.contains_key(&entity));
        }
    }

    #[test]
    fn casting_or_crowd_control_cancels_both_detection_and_active_phasing() {
        let mut tracker = Tracker::default();
        let entity = Entity::from_raw_u32(1).unwrap();
        tracker.resolve(entity, Vec3::ZERO, Vec3::X, true, 1.0, |from, _| from);
        assert_eq!(tracker.0[&entity].phase_left, 1.0);
        assert_eq!(
            tracker.resolve(entity, Vec3::ZERO, Vec3::X, false, 0.1, |from, _| from),
            Vec3::ZERO
        );
        tracker.resolve(entity, Vec3::ZERO, Vec3::X, true, 0.9, |from, _| from);
        assert_eq!(tracker.0[&entity].phase_left, 0.0);
    }
    #[test]
    fn crosses_a_real_wall_keeps_ground_support_and_restores_collision_afterwards() {
        use avian3d::prelude::*;
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::transform::TransformPlugin,
            bevy::asset::AssetPlugin::default(),
            bevy::scene::ScenePlugin,
            PhysicsPlugins::new(PostUpdate),
        ));
        app.init_asset::<Mesh>();
        super::super::super::geometry::init_test_world(app.world_mut());
        for (size, position) in [
            (Vec3::new(40.0, 1.0, 40.0), Vec3::new(0.0, -0.5, 0.0)),
            (Vec3::new(1.0, 4.0, 10.0), Vec3::new(2.0, 2.0, 0.0)),
            (Vec3::new(1.0, 4.0, 10.0), Vec3::new(8.0, 2.0, 0.0)),
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
        app.world_mut()
            .run_system_once(|surface: super::super::super::geometry::Surface| {
                let mut tracker = Tracker::default();
                let entity = Entity::from_raw_u32(1).unwrap();
                let mut position = Vec3::ZERO;
                for _ in 0..30 {
                    let desired = ground(&surface, position + Vec3::X * 0.4, position.y);
                    position = tracker.resolve(entity, position, desired, true, 0.1, |from, to| {
                        surface.constrain(from, to)
                    });
                    if tracker.0[&entity].phase_left > 0.0 {
                        break;
                    }
                }
                assert_eq!(tracker.0[&entity].phase_left, 1.0);
                assert!(
                    position.x < 1.5,
                    "the first wall blocked the chase: {position:?}"
                );
                for _ in 0..10 {
                    let desired = ground(&surface, position + Vec3::X * 0.4, position.y);
                    position = tracker.resolve(entity, position, desired, true, 0.1, |from, to| {
                        surface.constrain(from, to)
                    });
                }
                assert!(
                    position.x > 3.0 && position.x < 7.0,
                    "escaped the first wall: {position:?}"
                );
                assert!(position.y.abs() < 0.01, "the floor remains solid");
                let desired = ground(&surface, position + Vec3::X * 10.0, position.y);
                position = tracker.resolve(entity, position, desired, true, 0.1, |from, to| {
                    surface.constrain(from, to)
                });
                assert!(
                    position.x > 6.0 && position.x < 7.5,
                    "the second wall blocks movement after the one-second escape: {position:?}"
                );
            })
            .unwrap();
    }
}
