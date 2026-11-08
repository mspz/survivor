//! Hub services share mesh picking on the host and on replicated guest entities.
use super::{bank, gambling, hero::Hero, shared_chest, Phase, Selection};
use crate::net::{NetEntity, ObjectStore};
use benilla_protocol::EntityKind;
use benilla_world::{interact::ray_mesh_bounds, view::WorldCamera};
use bevy::{camera::primitives::Aabb, prelude::*, window::PrimaryWindow};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Service {
    Bank,
    Shared,
    Gambling,
    TrainingSmith,
    TrainingGold,
}

fn service(position: Vec3, net: &NetEntity, store: Option<&ObjectStore>) -> Option<Service> {
    if net.kind == EntityKind::Unit {
        match store.and_then(|s| s.0.object_entry()) {
            Some(gambling::TEST_SMITH_ENTRY) => return Some(Service::TrainingSmith),
            Some(gambling::TEST_GOLD_ENTRY) => return Some(Service::TrainingGold),
            _ => {}
        }
    }
    if net.kind == EntityKind::Unit
        && store.and_then(|s| s.0.object_entry()) == Some(gambling::NPC_ENTRY)
    {
        return Some(Service::Gambling);
    }
    if net.kind == EntityKind::GameObject && net.display_id == Some(10) {
        if position.distance_squared(bank::position()) < 0.25 {
            return Some(Service::Bank);
        }
        if position.distance_squared(shared_chest::position()) < 0.25 {
            return Some(Service::Shared);
        }
    }
    None
}

fn accessible(service: Service, player: Vec3) -> bool {
    match service {
        Service::Bank => bank::near(player),
        Service::Shared => shared_chest::near(player),
        Service::Gambling => gambling::near(player),
        Service::TrainingSmith => gambling::near_training(player, false),
        Service::TrainingGold => gambling::near_training(player, true),
    }
}

#[allow(clippy::type_complexity)]
fn nearest(
    ray: Ray3d,
    player: Vec3,
    units: &Query<(Entity, &Transform, &NetEntity, Option<&ObjectStore>)>,
    children: &Query<&Children>,
    meshes: &Query<(&Aabb, &GlobalTransform, Option<&InheritedVisibility>)>,
) -> Option<Service> {
    let mut nearest = None;
    let mut distance = f32::INFINITY;
    for (entity, transform, net, store) in units {
        let Some(service) = service(transform.translation, net, store) else {
            continue;
        };
        if !accessible(service, player) {
            continue;
        }
        let mut has_mesh = false;
        let mut hit = f32::INFINITY;
        for part in std::iter::once(entity).chain(children.iter_descendants(entity)) {
            let Ok((bounds, global, visible)) = meshes.get(part) else {
                continue;
            };
            has_mesh = true;
            if visible.is_some_and(|v| !v.get()) {
                continue;
            }
            if let Some(t) = ray_mesh_bounds(ray.origin, *ray.direction, bounds, global) {
                hit = hit.min(t);
            }
        }
        if !has_mesh || (!hit.is_finite() && net.kind == EntityKind::Unit) {
            let scale = net.scale.clamp(0.4, 5.0);
            let bounds = Aabb::from_min_max(
                Vec3::new(-0.6, 0.0, -0.6) * scale,
                Vec3::new(0.6, 2.0, 0.6) * scale,
            );
            if let Some(t) = ray_mesh_bounds(
                ray.origin,
                *ray.direction,
                &bounds,
                &GlobalTransform::from(*transform),
            ) {
                hit = t;
            }
        }
        if hit < distance {
            distance = hit;
            nearest = Some(service);
        }
    }
    nearest
}

fn open(
    service: Service,
    selection: &mut Selection,
    bank: &mut bank::View,
    shared: &mut shared_chest::View,
    gambling: &mut gambling::View,
) {
    bank.open = service == Service::Bank;
    shared.open = service == Service::Shared;
    gambling.open = matches!(service, Service::Gambling | Service::TrainingSmith);
    selection.bags_open = true;
    selection.sheet_open = false;
    selection.talents_open = false;
    selection.ui_captured = true;
    selection.pointer_captured = true;
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn click(
    mouse: Res<ButtonInput<MouseButton>>,
    phase: Res<State<Phase>>,
    mut selection: ResMut<Selection>,
    roster: Res<super::characters::Roster>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
    units: Query<(Entity, &Transform, &NetEntity, Option<&ObjectStore>)>,
    children: Query<&Children>,
    meshes: Query<(&Aabb, &GlobalTransform, Option<&InheritedVisibility>)>,
    interactions: Query<&Interaction>,
    heroes: Query<&Transform, With<Hero>>,
    mut bank: ResMut<bank::View>,
    mut shared: ResMut<shared_chest::View>,
    mut gambling: ResMut<gambling::View>,
    mut inventory: ResMut<super::items::Inventory>,
    mut press: Local<Option<Vec2>>,
) {
    let training = super::training::active(&selection);
    if !(selection.in_hub || training)
        || !(matches!(*phase.get(), Phase::Hub | Phase::CharacterSheet)
            || (training && *phase.get() == Phase::Playing))
        || selection.talents_open
        || selection.pointer_captured
        || roster.expeditions
        || interactions.iter().any(|i| *i != Interaction::None)
    {
        *press = None;
        return;
    }
    let Some(cursor) = windows.single().ok().and_then(|w| w.cursor_position()) else {
        *press = None;
        return;
    };
    if mouse.just_pressed(MouseButton::Right) {
        *press = Some(cursor);
    }
    if !mouse.just_released(MouseButton::Right) {
        return;
    }
    if press
        .take()
        .is_none_or(|start| start.distance_squared(cursor) > 36.0)
    {
        return;
    }
    let (Ok(player), Ok((camera, transform))) = (heroes.single(), cameras.single()) else {
        return;
    };
    let Ok(ray) = camera.viewport_to_world(transform, cursor) else {
        return;
    };
    if let Some(service) = nearest(ray, player.translation, &units, &children, &meshes) {
        if service == Service::TrainingGold {
            if let Err(error) = gambling::give_training_gold(&selection, &mut inventory) {
                inventory.notice = error;
            } else {
                inventory.notice = "+10,000 gold".into();
            }
            selection.pointer_captured = true;
            selection.ui_captured = true;
            return;
        }
        open(
            service,
            &mut selection,
            &mut bank,
            &mut shared,
            &mut gambling,
        );
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        click
            .after(super::combat_ui::input)
            .before(super::client::input)
            .before(super::hud::keys),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn treasurer_body_remains_clickable_when_model_bounds_miss_the_body() {
        let mut world = World::new();
        let position = gambling::training_position(true);
        world.spawn((
            Transform::from_translation(position),
            NetEntity {
                kind: EntityKind::Unit,
                display_id: Some(49),
                scale: 1.0,
            },
            ObjectStore(benilla_protocol::messages::ObjectFields::from_pairs(&[(
                3,
                gambling::TEST_GOLD_ENTRY,
            )])),
            Aabb::from_min_max(-Vec3::ONE, Vec3::ONE),
            GlobalTransform::from_translation(position + Vec3::X * 100.0),
        ));
        world.run_system_once(move |units: Query<(Entity, &Transform, &NetEntity, Option<&ObjectStore>)>, children: Query<&Children>, meshes: Query<(&Aabb, &GlobalTransform, Option<&InheritedVisibility>)>| {
            let ray = Ray3d::new(position + Vec3::Y * 10.0, Dir3::NEG_Y);
            assert_eq!(nearest(ray, position, &units, &children, &meshes), Some(Service::TrainingGold));
            assert_eq!(nearest(ray, position + Vec3::X * 10.0, &units, &children, &meshes), None);
            let miss = Ray3d::new(position + Vec3::X * 3.0 + Vec3::Y * 10.0, Dir3::NEG_Y);
            assert_eq!(nearest(miss, position, &units, &children, &meshes), None);
        }).unwrap();
    }

    #[test]
    fn replicated_services_are_pickable_without_host_only_components_and_require_proximity() {
        for (expected, position, kind, display, entry) in [
            (
                Service::Bank,
                bank::position(),
                EntityKind::GameObject,
                10,
                0,
            ),
            (
                Service::Shared,
                shared_chest::position(),
                EntityKind::GameObject,
                10,
                0,
            ),
            (
                Service::TrainingSmith,
                gambling::training_position(false),
                EntityKind::Unit,
                49,
                gambling::TEST_SMITH_ENTRY,
            ),
            (
                Service::TrainingGold,
                gambling::training_position(true),
                EntityKind::Unit,
                49,
                gambling::TEST_GOLD_ENTRY,
            ),
            (
                Service::Gambling,
                gambling::position(),
                EntityKind::Unit,
                49,
                gambling::NPC_ENTRY,
            ),
        ] {
            let mut world = World::new();
            world.spawn((
                Transform::from_translation(position),
                NetEntity {
                    kind,
                    display_id: Some(display),
                    scale: 1.0,
                },
                ObjectStore(benilla_protocol::messages::ObjectFields::from_pairs(&[(
                    3, entry,
                )])),
            ));
            world
                .run_system_once(
                    move |units: Query<(Entity, &Transform, &NetEntity, Option<&ObjectStore>)>,
                          children: Query<&Children>,
                          meshes: Query<(
                        &Aabb,
                        &GlobalTransform,
                        Option<&InheritedVisibility>,
                    )>| {
                        let ray = Ray3d::new(position + Vec3::Y * 10.0, Dir3::NEG_Y);
                        assert_eq!(
                            nearest(ray, position, &units, &children, &meshes),
                            Some(expected)
                        );
                        assert_eq!(
                            nearest(ray, position + Vec3::X * 10.0, &units, &children, &meshes),
                            None
                        );
                        let miss =
                            Ray3d::new(position + Vec3::X * 3.0 + Vec3::Y * 10.0, Dir3::NEG_Y);
                        assert_eq!(nearest(miss, position, &units, &children, &meshes), None);
                    },
                )
                .unwrap();
        }
    }
    #[test]
    fn opening_each_service_closes_the_other_two_and_opens_the_bag() {
        let mut selection = Selection::default();
        let mut bank = bank::View::default();
        let mut shared = shared_chest::View::default();
        let mut gambling = gambling::View::default();
        for service in [Service::Bank, Service::Shared, Service::Gambling] {
            open(
                service,
                &mut selection,
                &mut bank,
                &mut shared,
                &mut gambling,
            );
            assert_eq!(
                (bank.open, shared.open, gambling.open),
                (
                    service == Service::Bank,
                    service == Service::Shared,
                    service == Service::Gambling
                )
            );
            assert!(selection.bags_open && selection.ui_captured && selection.pointer_captured);
        }
    }
}
