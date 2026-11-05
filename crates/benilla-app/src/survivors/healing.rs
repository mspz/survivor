//! Automatic heals select the living party member with the lowest health percentage in range.
use super::{
    data::AbilityDef,
    items::legendary,
    multiplayer::{Companions, HostPlayer},
    Run,
};
use bevy::{ecs::system::SystemParam, prelude::*};

#[derive(SystemParam)]
pub(super) struct PartyHealing<'w, 's> {
    players: Option<ResMut<'w, Companions>>,
    host: Option<ResMut<'w, HostPlayer>>,
    positions: Query<'w, 's, &'static Transform>,
}

fn health(run: &Run) -> Option<f32> {
    (run.hp > 0.0 && run.dead_t.is_none()).then(|| run.hp / run.stats().max_hp.max(1.0))
}

impl PartyHealing<'_, '_> {
    #[allow(clippy::too_many_arguments)]
    pub fn cast(
        &mut self,
        caster: &mut Run,
        me: Entity,
        def: &AbilityDef,
        amount: f32,
        duration: f32,
        range: f32,
    ) -> Entity {
        let origin = self.positions.get(me).map_or(Vec3::ZERO, |t| t.translation);
        let mut chosen = me;
        let mut lowest = health(caster).unwrap_or(f32::INFINITY);
        let mut consider = |entity, run: &Run| {
            let Some(fraction) = health(run) else { return };
            if self
                .positions
                .get(entity)
                .is_ok_and(|t| super::units::flat(t.translation - origin).length() <= range)
                && fraction < lowest
            {
                lowest = fraction;
                chosen = entity;
            }
        };
        if let Some(host) = self.host.as_deref() {
            consider(host.entity, &host.run);
        }
        if let Some(players) = self.players.as_deref() {
            for player in players.0.values() {
                consider(player.entity, &player.run);
            }
        }
        if chosen == me {
            if caster.legendary.has(305) && def.key == "regrowth" {
                legendary::effects::seed(caster, amount, duration);
            } else if caster.legendary.has(152) || caster.legendary.has(237) {
                caster
                    .legendary
                    .effects
                    .active_heals
                    .push((amount, duration));
            } else {
                apply(caster, amount, duration);
            }
        } else {
            let outgoing = amount * (1.0 + caster.equipment.healing).max(0.0);
            let target = if let Some(host) = self.host.as_deref_mut().filter(|h| h.entity == chosen)
            {
                Some(&mut host.run)
            } else {
                self.players
                    .as_deref_mut()
                    .and_then(|p| p.0.values_mut().find(|p| p.entity == chosen))
                    .map(|p| &mut p.run)
            };
            if let Some(target) = target {
                // Run::heal applies the recipient's equipment multiplier; only the caster's
                // healing power belongs to this cast, including its periodic snapshot.
                let incoming = outgoing / (1.0 + target.equipment.healing).max(0.001);
                apply(target, incoming, duration);
            }
        }
        chosen
    }
}

fn apply(target: &mut Run, amount: f32, duration: f32) {
    if duration <= 0.0 {
        legendary::events::active_heal(target, amount);
    } else {
        target.heal_rate = amount / duration.max(1.0);
        target.heal_t = duration;
    }
}
