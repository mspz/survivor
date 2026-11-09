//! Seal instance exits with their installed AreaTrigger.dbc geometry, never an arena boundary.
use super::{Selection, units::wow_to_bevy};
use avian3d::prelude::{Collider, RigidBody};
use benilla_formats::AreaTriggerRow;
use bevy::prelude::*;

// Teleport source IDs from MaNGOS Zero's areatrigger_teleport table. Geometry stays in the install.
// Includes side/back doors and the Blackrock Depths entrances to Molten Core.
const EXITS: &[u32] = &[
    1470, 1472, 2221, 2568, 2886, 3190, 3191, 3193, 3194, 3195, 3196, 3197, 3528, 3529, 3726,
];

#[derive(Component)]
struct PortalBarrier;

fn barrier(row: &AreaTriggerRow) -> (Collider, Transform) {
    let collider = if row.radius > 0.0 {
        Collider::sphere(row.radius)
    } else {
        Collider::cuboid(row.box_size[1], row.box_size[2], row.box_size[0])
    };
    (
        collider,
        Transform::from_translation(wow_to_bevy(row.position))
            .with_rotation(Quat::from_rotation_y(row.box_yaw)),
    )
}

fn sync(
    mut commands: Commands,
    selection: Res<Selection>,
    triggers: Option<Res<crate::area_trigger::AreaTriggers>>,
    barriers: Query<Entity, With<PortalBarrier>>,
    mut installed: Local<Option<u32>>,
    mut epoch: Option<ResMut<benilla_world::collision::ColliderEpoch>>,
) {
    let map = selection.stage_def();
    let want = map.full_instance().then_some(map.map);
    if *installed == want {
        return;
    }
    // Wait for the startup catalog before marking an instance as installed.
    if want.is_some() && triggers.is_none() {
        return;
    }
    for entity in &barriers {
        commands.entity(entity).despawn();
    }
    if let (Some(map), Some(triggers)) = (want, triggers) {
        for row in triggers
            .0
            .on_map(map)
            .iter()
            .filter(|row| EXITS.contains(&row.id))
        {
            let (collider, transform) = barrier(row);
            commands.spawn((PortalBarrier, RigidBody::Static, collider, transform));
        }
    }
    *installed = want;
    if let Some(epoch) = epoch.as_deref_mut() {
        epoch.bump();
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, sync.after(super::hero::follow_map));
}

#[cfg(test)]
mod tests {
    use super::*;
    use avian3d::prelude::SimpleCollider;
    use crate::survivors::data;

    #[test]
    fn portal_boxes_keep_the_installed_axes_and_position() {
        let row = AreaTriggerRow {
            id: 1472,
            map_id: 230,
            position: [10.0, 20.0, 30.0],
            radius: 0.0,
            box_size: [2.0, 12.0, 8.0],
            box_yaw: 0.7,
        };
        let (collider, transform) = barrier(&row);
        assert_eq!(transform.translation, Vec3::new(-20.0, 30.0, -10.0));
        let aabb = collider.aabb(Vec3::ZERO, Quat::IDENTITY);
        assert!((aabb.max.x - aabb.min.x - 12.0).abs() < 0.001);
        assert!((aabb.max.y - aabb.min.y - 8.0).abs() < 0.001);
        assert!((aabb.max.z - aabb.min.z - 2.0).abs() < 0.001);
        let local_x = transform.rotation * Vec3::NEG_Z;
        let expected = wow_to_bevy([row.box_yaw.cos(), row.box_yaw.sin(), 0.0]);
        assert!(local_x.distance(expected) < 0.001);
    }

    #[test]
    fn installed_dungeons_have_exit_barriers_and_spawn_outside_them() {
        let Some(path) = benilla_formats::wow_data() else {
            return;
        };
        let mut chain = benilla_formats::open_chain(&path).unwrap();
        let catalog = benilla_formats::load_area_trigger_catalog(&mut chain).unwrap();
        for &index in data::DUNGEONS {
            let map = &data::MAPS[index];
            let exits: Vec<_> = catalog
                .on_map(map.map)
                .iter()
                .filter(|row| EXITS.contains(&row.id))
                .collect();
            assert!(!exits.is_empty(), "{} has no exit barriers", map.name);
            assert!(
                exits.iter().all(|row| !row.contains(map.center)),
                "{} starts in a barrier",
                map.name
            );
        }
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(Selection {
                stage: data::DUNGEONS[0],
                in_hub: false,
                ..default()
            })
            .insert_resource(crate::area_trigger::AreaTriggers(catalog))
            .add_systems(Update, sync);
        app.update();
        assert!(
            app.world_mut()
                .query::<&PortalBarrier>()
                .iter(app.world())
                .count()
                > 0
        );
        app.world_mut().resource_mut::<Selection>().in_hub = true;
        app.update();
        assert_eq!(
            app.world_mut()
                .query::<&PortalBarrier>()
                .iter(app.world())
                .count(),
            0
        );
    }
}
