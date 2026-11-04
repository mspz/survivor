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

#[derive(Resource, Default)]
pub(super) struct GuestActions {
    pub pick: Option<usize>,
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
        .init_resource::<GuestActions>()
        .init_resource::<Companions>();
    app.add_plugins(lobby::plugin);
    #[cfg(not(target_arch = "wasm32"))]
    native::plugin(app);
}
