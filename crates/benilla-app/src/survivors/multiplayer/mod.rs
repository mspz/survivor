//! Native cooperative host/join; the browser retains the in-process solo server.
use bevy::prelude::*;
use std::collections::BTreeMap;

#[derive(Component, Clone, Copy)]
pub(super) struct RemoteOwner(pub u64);

#[derive(Component)]
pub(super) struct OwnedEffect(pub u64);

pub(super) struct Companion {
    pub entity: Entity,
    pub run: super::Run,
    pub physics: super::hero::Hero,
    pub offer: super::hud::Offer,
    pub offer_id: u64,
}

#[derive(Resource, Default)]
pub(super) struct Companions(pub BTreeMap<u64, Companion>);

/// Keeps the host available as an ally while registered rules simulate a guest's run.
#[derive(Resource)]
pub(super) struct HostPlayer {
    pub entity: Entity,
    pub run: super::Run,
}

pub(super) fn player_run<'a>(
    host: &'a mut super::Run,
    players: Option<&'a mut Companions>,
    owner: Option<u64>,
) -> Option<&'a mut super::Run> {
    match owner {
        Some(owner) => Some(&mut players?.0.get_mut(&owner)?.run),
        None => Some(host),
    }
}

#[derive(Resource, Default, Clone, Copy, PartialEq, Eq)]
pub(super) enum Role {
    #[default]
    Solo,
    Host,
    Guest,
}

pub(super) fn authoritative(role: Option<Res<Role>>) -> bool {
    role.is_none_or(|r| *r != Role::Guest)
}

/// Menus are local overlays in co-op; the expedition keeps running underneath them.
pub(super) fn runs_in(phase: super::Phase, role: Role) -> bool {
    phase == super::Phase::Playing
        || (role != Role::Solo
            && matches!(
                phase,
                super::Phase::Paused | super::Phase::CharacterSheet | super::Phase::LevelUp
            ))
}

pub(super) fn simulating(
    phase: Res<State<super::Phase>>,
    role: Option<Res<Role>>,
    selection: Res<super::Selection>,
) -> bool {
    !selection.in_hub && runs_in(*phase.get(), role.as_deref().copied().unwrap_or_default())
}

#[derive(Resource, Default)]
pub(super) struct GuestActions {
    pub pick: Option<usize>,
    pub reset_meter: bool,
}

pub(super) fn reset_meters(run: &mut super::Run, players: Option<&mut Companions>) {
    fn clear(run: &mut super::Run) {
        run.effect_meter = default();
        run.meter = default();
        run.boss_meter = default();
        run.healing_meter = default();
        run.taken_meter = default();
    }
    clear(run);
    if let Some(players) = players {
        for player in players.0.values_mut() {
            clear(&mut player.run);
        }
    }
}

pub(super) mod lobby;
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
mod protocol;
#[cfg(not(target_arch = "wasm32"))]
mod transport;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Role>()
        .init_resource::<super::combat_ui::PartyDamage>()
        .init_resource::<GuestActions>()
        .init_resource::<Companions>();
    app.add_plugins(lobby::plugin);
    #[cfg(not(target_arch = "wasm32"))]
    native::plugin(app);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::survivors::{server, Phase};
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn coop_menus_keep_the_clock_and_simulation_running() {
        for role in [Role::Solo, Role::Host, Role::Guest] {
            let mut world = World::new();
            world.init_resource::<Time<Virtual>>();
            world.insert_resource(role);
            world.run_system_once(server::pause).unwrap();
            assert_eq!(
                world.resource::<Time<Virtual>>().is_paused(),
                role == Role::Solo
            );
            for phase in [Phase::Paused, Phase::LevelUp, Phase::CharacterSheet] {
                assert_eq!(runs_in(phase, role), role != Role::Solo);
            }
            assert!(!runs_in(Phase::GameOver, role));
            assert!(!runs_in(Phase::Hub, role));
        }
    }
}
