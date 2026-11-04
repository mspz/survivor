use super::*;
use crate::net::NetEntity;
use benilla_world::{interact::ray_mesh_bounds, view::WorldCamera};
use bevy::{camera::primitives::Aabb, window::PrimaryWindow};

fn clean_click(start: Vec2, end: Vec2) -> bool {
    start.distance_squared(end) <= 36.0
}

#[allow(clippy::too_many_arguments)]
pub(super) fn click(
    mouse: Res<ButtonInput<MouseButton>>,
    phase: Res<State<Phase>>,
    selection: Res<Selection>,
    roster: Res<characters::Roster>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
    units: Query<(Entity, &Status, &Transform, &NetEntity)>,
    children: Query<&Children>,
    meshes: Query<(&Aabb, &GlobalTransform, Option<&InheritedVisibility>)>,
    interactions: Query<&Interaction>,
    mut target: ResMut<Target>,
) {
    if !matches!(*phase.get(), Phase::Playing | Phase::Hub) {
        target.press = None;
        if matches!(*phase.get(), Phase::Menu | Phase::GameOver) {
            target.entity = None;
        }
        return;
    }
    let blocked = selection.sheet_open
        || selection.bags_open
        || selection.talents_open
        || selection.ui_captured
        || roster.expeditions
        || interactions.iter().any(|i| *i != Interaction::None);
    if blocked || mouse.pressed(MouseButton::Right) {
        target.press = None;
        return;
    }
    let Some(cursor) = windows.single().ok().and_then(|w| w.cursor_position()) else {
        target.press = None;
        return;
    };
    if mouse.just_pressed(MouseButton::Left) {
        target.press = Some(cursor);
    }
    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    let Some(_) = target
        .press
        .take()
        .filter(|&start| clean_click(start, cursor))
    else {
        return;
    };
    let Ok((camera, transform)) = cameras.single() else {
        return;
    };
    let Ok(ray) = camera.viewport_to_world(transform, cursor) else {
        return;
    };
    target.entity = nearest(ray, &units, &children, &meshes);
}

#[allow(clippy::type_complexity)]
fn nearest(
    ray: Ray3d,
    units: &Query<(Entity, &Status, &Transform, &NetEntity)>,
    children: &Query<&Children>,
    meshes: &Query<(&Aabb, &GlobalTransform, Option<&InheritedVisibility>)>,
) -> Option<Entity> {
    let mut nearest = None;
    let mut distance = f32::INFINITY;
    for (entity, status, tf, net) in units {
        if status.health <= 0.0 {
            continue;
        }
        let mut has_mesh = false;
        let mut hit = f32::INFINITY;
        for part in std::iter::once(entity).chain(children.iter_descendants(entity)) {
            let Ok((aabb, global, visible)) = meshes.get(part) else {
                continue;
            };
            if visible.is_some_and(|v| !v.get()) {
                continue;
            }
            has_mesh = true;
            if let Some(t) = ray_mesh_bounds(ray.origin, *ray.direction, aabb, global) {
                hit = hit.min(t);
            }
        }
        // Models load asynchronously; a small body box keeps fresh units selectable.
        if !has_mesh {
            let scale = net.scale.clamp(0.4, 5.0);
            let aabb = Aabb::from_min_max(
                Vec3::new(-0.6, 0.0, -0.6) * scale,
                Vec3::new(0.6, 2.0, 0.6) * scale,
            );
            if let Some(t) = ray_mesh_bounds(
                ray.origin,
                *ray.direction,
                &aabb,
                &GlobalTransform::from(*tf),
            ) {
                hit = t;
            }
        }
        if hit < distance {
            distance = hit;
            nearest = Some(entity);
        }
    }
    nearest
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn camera_drags_are_not_target_clicks() {
        assert!(clean_click(Vec2::ZERO, Vec2::new(2.0, 3.0)));
        assert!(!clean_click(Vec2::ZERO, Vec2::new(9.0, 1.0)));
    }

    #[test]
    fn nearest_ally_or_enemy_is_selectable_but_dead_units_are_skipped() {
        use bevy::ecs::system::RunSystemOnce;
        fn pick(
            units: Query<(Entity, &Status, &Transform, &NetEntity)>,
            children: Query<&Children>,
            meshes: Query<(&Aabb, &GlobalTransform, Option<&InheritedVisibility>)>,
        ) -> Option<Entity> {
            nearest(Ray3d::new(Vec3::Y, Dir3::NEG_Z), &units, &children, &meshes)
        }
        let mut world = World::new();
        let mut spawn = |z, friendly| {
            world
                .spawn((
                    Status {
                        name: "Unit".into(),
                        health: 100.0,
                        maximum: 100.0,
                        level: 60,
                        friendly,
                        debuffs: vec![],
                    },
                    Transform::from_xyz(0.0, 0.0, z),
                    NetEntity {
                        kind: benilla_protocol::EntityKind::Unit,
                        display_id: Some(367),
                        scale: 1.0,
                    },
                ))
                .id()
        };
        let ally = spawn(-5.0, true);
        let enemy = spawn(-10.0, false);
        assert_eq!(world.run_system_once(pick).unwrap(), Some(ally));
        world.get_mut::<Status>(ally).unwrap().health = 0.0;
        assert_eq!(world.run_system_once(pick).unwrap(), Some(enemy));
        world.despawn(enemy);
        assert_eq!(world.run_system_once(pick).unwrap(), None);
    }
}
