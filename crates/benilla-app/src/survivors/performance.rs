//! Survivors owns its aiming, interactions and UI. The classic WoW target picker otherwise
//! skins and ray-tests the horde's models every frame despite having no consumer in this mode.
use bevy::prelude::*;

pub(super) fn plugin(app: &mut App) {
    // Opt back in only for an interactive A/B measurement on the same build and terrain.
    let native_picking = crate::webenv::var("WOW_SURVIVORS_NATIVE_PICKING").as_deref() == Some("1");
    app.configure_sets(Update, crate::target::TargetUpdate.run_if(move || native_picking));
}
