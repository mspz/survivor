//! A local-profile vault shared by its characters; its contents never enter chest snapshots.
#[cfg(test)]
use super::shared_chest::Entry;
use super::shared_chest::{Replica, Request};
use super::{
    hero, hud,
    items::{Inventory, Item},
    multiplayer::Role,
    units, Phase, Run, RunEntity, Selection,
};
use bevy::prelude::*;
#[path = "shared_chest/ui.rs"]
mod ui;
type OtherView = super::shared_chest::View;
const TITLE: &str = "Personal bank";
const OPEN_LABEL: &str = "Open personal bank";
const PROMPT_OFFSET: f32 = 130.0;
#[cfg(test)]
use super::shared_chest::fixture_item;

#[derive(Resource)]
struct Store(super::shared_chest::Store);
impl Default for Store {
    fn default() -> Self {
        Self(super::shared_chest::Store::personal())
    }
}
#[derive(Resource, Default)]
pub(super) struct View {
    pub open: bool,
    pub replica: Replica,
    pub requests: Vec<Request>,
    pub notice: String,
    pub revision: u64,
    stamp: Option<(u64, u64, Option<u64>)>,
    page: usize,
}
#[derive(Component)]
struct Chest;
pub(super) fn position() -> Vec3 {
    units::wow_to_bevy([-9471.0, 58.0, 57.034_08])
}
pub(super) fn near(pos: Vec3) -> bool {
    units::flat(pos - position()).length() <= 5.0
}
fn receive(replica: Replica, inventory: &mut Inventory, view: &mut View) {
    for token in &replica.accepted {
        if let Err(error) = inventory.finish_deposit(token) {
            view.notice = error;
        }
    }
    for delivery in &replica.deliveries {
        match inventory.receive_trade(delivery) {
            Ok(()) => view.requests.push(Request::Ack(delivery.token.clone())),
            Err(error) => view.notice = error,
        }
    }
    if !replica.notice.is_empty() {
        view.notice = replica.notice.clone();
    }
    view.replica = replica;
    view.revision = view.revision.wrapping_add(1);
}
fn local_sync(
    selection: Res<Selection>,
    heroes: Query<&Transform, With<hero::Hero>>,
    mut store: ResMut<Store>,
    mut inventory: ResMut<Inventory>,
    mut view: ResMut<View>,
) {
    if !store.0.loaded() {
        store.0.load();
    }
    let accessible = selection.in_hub && heroes.single().is_ok_and(|t| near(t.translation));
    let mut requests = std::mem::take(&mut view.requests);
    requests.extend(
        inventory
            .pending_deposits()
            .iter()
            .filter(|t| t.vault == store.0.identity())
            .map(|t| Request::Deposit(t.token.clone())),
    );
    for request in requests {
        if let Err(error) = store.0.request(&inventory, request, accessible) {
            view.notice = error;
        }
    }
    let stamp = (
        store.0.revision(),
        inventory.revision,
        inventory.money_owner(),
    );
    if view.stamp != Some(stamp) {
        receive(store.0.replica(&inventory), &mut inventory, &mut view);
        view.stamp = Some((
            store.0.revision(),
            inventory.revision,
            inventory.money_owner(),
        ));
    }
}
fn spawn(
    mut commands: Commands,
    selection: Res<Selection>,
    mut run: ResMut<Run>,
    role: Res<Role>,
    chests: Query<Entity, With<Chest>>,
    heroes: Query<&Transform, With<hero::Hero>>,
) {
    if !selection.in_hub {
        for entity in &chests {
            commands.entity(entity).despawn();
        }
        return;
    }
    if *role == Role::Guest || heroes.is_empty() {
        return;
    }
    if chests.is_empty() {
        let entity = units::spawn_object(&mut commands, run.guid(), 10, 3.0, position());
        commands.entity(entity).insert((Chest, RunEntity));
    }
}
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Store>()
        .init_resource::<View>()
        .add_systems(Update, spawn.after(super::characters::arrive))
        .add_systems(
            Update,
            local_sync.after(super::items::sync).before(ui::render),
        )
        .add_systems(
            Update,
            ui::input
                .after(super::combat_ui::input)
                .after(super::hub_interact::click)
                .before(super::client::input)
                .before(hud::keys),
        )
        .add_systems(Update, ui::gestures.after(ui::input).before(local_sync))
        .add_systems(Update, ui::render.after(ui::gestures))
        .add_systems(Update, ui::tooltip.after(ui::render));
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    #[test]
    fn characters_share_the_personal_bank_in_guest_mode_without_using_the_shared_chest() {
        let mut world = World::new();
        world.init_resource::<Store>();
        world.init_resource::<View>();
        world.insert_resource(Role::Guest);
        world.insert_resource(Selection {
            in_hub: true,
            ..default()
        });
        world.insert_resource(Inventory::fixture_for_character(1, vec![fixture_item()]));
        world.spawn((
            hero::Hero { vy: 0.0, air: 0.0 },
            Transform::from_translation(position()),
        ));
        world.run_system_once(local_sync).unwrap();
        let vault = world.resource::<View>().replica.identity.clone();
        world
            .resource_mut::<Inventory>()
            .begin_deposit(1, &vault)
            .unwrap();
        world.run_system_once(local_sync).unwrap();
        assert!(world.resource::<Inventory>().all().is_empty());
        assert!(world.resource::<Inventory>().pending_deposits().is_empty());
        let entries = world.resource::<View>().replica.entries.clone();
        assert_eq!(entries.len(), 1);
        let shared = super::super::shared_chest::Store::default();
        assert!(shared
            .replica(world.resource::<Inventory>())
            .entries
            .is_empty());
        assert_ne!(shared.identity(), vault);
        // Switching characters replaces the inventory, while the account bank remains the same.
        let mut other = Inventory::fixture_for_character(2, vec![]);
        other.ensure_trade_identity().unwrap();
        world.insert_resource(other);
        world
            .resource_mut::<View>()
            .requests
            .push(Request::Withdraw {
                vault,
                entry: entries[0].id,
            });
        world.run_system_once(local_sync).unwrap();
        world.run_system_once(local_sync).unwrap();
        assert_eq!(world.resource::<Inventory>().all().len(), 1);
        assert!(world.resource::<View>().replica.entries.is_empty());
        world.run_system_once(local_sync).unwrap();
        assert_eq!(world.resource::<Inventory>().all().len(), 1);
    }
}
