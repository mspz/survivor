//! Stormwind's streets are WMO floors above the ADT heightfield.
use super::{
    data,
    hero::{self, Hero},
    Selection,
};
use benilla_world::{collision::WorldCollision, schedule::WorldStage};
use bevy::prelude::*;

#[derive(Component)]
struct HubFloor {
    height: f32,
    settled: bool,
}
impl Default for HubFloor {
    fn default() -> Self {
        Self {
            height: hero::arena(&data::HUB).y,
            settled: false,
        }
    }
}
pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        place_on_floor
            .in_set(WorldStage::Input)
            .after(hero::drive_hero)
            .before(hero::place_camera),
    );
}
fn floor_height(collision: &WorldCollision, position: Vec3, previous: &HubFloor) -> Option<f32> {
    // Initially stay near the authored street altitude until the city colliders stream in.
    // Afterwards probe from the previous ground, not the jumping body's height or the ADT.
    let lift = if previous.settled { 0.75 } else { 3.0 };
    let origin = Vec3::new(position.x, previous.height + lift, position.z);
    collision
        .ray_body(
            origin,
            Dir3::NEG_Y,
            if previous.settled { 128.0 } else { 6.0 },
        )
        .filter(|hit| hit.normal.y >= 0.5)
        .map(|hit| origin.y - hit.distance)
}
fn place_on_floor(
    mut commands: Commands,
    selection: Res<Selection>,
    collision: WorldCollision,
    mut player: ResMut<crate::player::Player>,
    mut heroes: Query<(Entity, &Hero, &mut Transform, Option<&mut HubFloor>)>,
) {
    for (entity, hero, mut transform, previous) in &mut heroes {
        if !selection.in_hub {
            if previous.is_some() {
                commands.entity(entity).remove::<HubFloor>();
            }
            continue;
        }
        let mut initial = HubFloor::default();
        let mut previous = previous;
        let floor = previous.as_deref_mut().unwrap_or(&mut initial);
        if let Some(height) = floor_height(&collision, transform.translation, floor) {
            floor.height = height;
            floor.settled = true;
        }
        transform.translation.y = floor.height + hero.air;
        player.pos = transform.translation;
        if previous.is_none() {
            commands.entity(entity).insert(initial);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use avian3d::prelude::*;
    fn app() -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::transform::TransformPlugin,
            bevy::asset::AssetPlugin::default(),
            bevy::scene::ScenePlugin,
            PhysicsPlugins::new(PostUpdate),
        ))
        .init_asset::<Mesh>()
        .init_resource::<benilla_world::collision::MoverTraceExclusions>()
        .init_resource::<crate::player::Player>()
        .insert_resource(Selection {
            in_hub: true,
            ..default()
        });
        app.add_systems(Update, place_on_floor);
        let origin = hero::arena(&data::HUB);
        // At this hub coordinate, the installed ADT is 59.457424 while the city street is 94.00618.
        floor(&mut app, 59.457_424);
        let entity = app
            .world_mut()
            .spawn((
                Hero { vy: 0.0, air: 0.0 },
                Transform::from_translation(Vec3::new(origin.x, 59.457_424, origin.z)),
            ))
            .id();
        app.finish();
        app.cleanup();
        (app, entity)
    }
    fn floor(app: &mut App, height: f32) {
        let origin = hero::arena(&data::HUB);
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(20.0, 0.2, 20.0),
            Transform::from_xyz(origin.x, height - 0.1, origin.z),
        ));
    }
    #[test]
    fn stormwind_spawn_waits_above_adt_then_settles_on_city_street_without_picking_the_roof() {
        let (mut app, entity) = app();
        let origin = hero::arena(&data::HUB);
        app.update();
        app.update();
        assert!(
            (app.world().get::<Transform>(entity).unwrap().translation.y - origin.y).abs() < 0.01
        );
        assert!(!app.world().get::<HubFloor>(entity).unwrap().settled);
        floor(&mut app, origin.y);
        floor(&mut app, origin.y + 15.0);
        app.update();
        app.update();
        let position = app.world().get::<Transform>(entity).unwrap().translation;
        assert!((position.y - origin.y).abs() < 0.01);
        assert!(app.world().get::<HubFloor>(entity).unwrap().settled);
        assert_eq!(
            app.world().resource::<crate::player::Player>().pos,
            position
        );
        app.world_mut().get_mut::<Hero>(entity).unwrap().air = 1.5;
        app.update();
        assert!(
            (app.world().get::<Transform>(entity).unwrap().translation.y - origin.y - 1.5).abs()
                < 0.01
        );
    }
    #[test]
    fn expedition_departure_releases_the_hub_height() {
        let (mut app, entity) = app();
        app.update();
        app.world_mut().resource_mut::<Selection>().in_hub = false;
        app.world_mut()
            .get_mut::<Transform>(entity)
            .unwrap()
            .translation
            .y = 12.0;
        app.update();
        assert!(app.world().get::<HubFloor>(entity).is_none());
        assert_eq!(
            app.world().get::<Transform>(entity).unwrap().translation.y,
            12.0
        );
    }
}
