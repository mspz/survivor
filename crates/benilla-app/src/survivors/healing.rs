//! Automatic heals select the living party member with the lowest health percentage in range.
use super::{
    data::AbilityDef,
    items::legendary,
    multiplayer::{Companions, HostPlayer},
    Run,
};
use bevy::{ecs::system::SystemParam, prelude::*};

#[derive(Message)]
pub(super) struct Credit {
    pub owner: Option<u64>,
    pub skill: &'static str,
    pub amount: f32,
}

pub(super) fn credit(
    mut credits: MessageReader<Credit>,
    mut run: ResMut<Run>,
    mut players: ResMut<Companions>,
) {
    for credit in credits.read() {
        if let Some(source) =
            super::multiplayer::player_run(&mut run, Some(&mut players), credit.owner)
        {
            source.healing_meter.record_hit(
                credit.skill,
                source.elapsed,
                credit.amount,
                false,
                true,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    #[test]
    fn proc_rules_triggered_heals_do_not_trigger_healing_shields() {
        for triggered in [false, true] {
            let mut run = Run {
                hp: 1.0,
                ..default()
            };
            run.legendary.equipped.insert(
                217,
                legendary::Roll {
                    unique: 217,
                    milestone: None,
                },
            );
            run.immune_t = 5.0;
            apply(&mut run, 10.0, 0.0, (None, "flash_heal"), triggered);
            if triggered {
                assert!(run.hp > 1.0);
                assert_eq!(run.shield, 0.0);
            } else {
                assert_eq!(run.hp, 1.0);
                assert!(run.shield > 0.0);
            }
            apply(&mut run, 10.0, 5.0, (None, "renew"), triggered);
            assert_eq!(run.heal_triggered, triggered);
        }
    }
    #[test]
    fn periodic_healing_is_credited_to_the_guest_caster() {
        let mut world = World::new();
        world.init_resource::<Run>();
        world.init_resource::<Messages<Credit>>();
        let entity = world.spawn_empty().id();
        let mut players = Companions::default();
        players.0.insert(
            42,
            super::super::multiplayer::Companion {
                entity,
                run: Run {
                    elapsed: 5.0,
                    owner: Some(42),
                    ..default()
                },
                physics: super::super::hero::Hero { vy: 0.0, air: 0.0 },
                offer: default(),
                offer_id: 0,
            },
        );
        world.insert_resource(players);
        world.resource_mut::<Messages<Credit>>().write(Credit {
            owner: Some(42),
            skill: "renew",
            amount: 17.0,
        });
        world.run_system_once(credit).unwrap();
        assert!(world.resource::<Run>().healing_meter.rows(5.0).is_empty());
        assert_eq!(
            world.resource::<Companions>().0[&42]
                .run
                .healing_meter
                .rows(5.0)[0]
                .total,
            17.0
        );
    }
}

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
        self.cast_with_origin(caster, me, def, amount, duration, range, false)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn cast_with_origin(
        &mut self,
        caster: &mut Run,
        me: Entity,
        def: &AbilityDef,
        amount: f32,
        duration: f32,
        range: f32,
        triggered: bool,
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
        let credit = (caster.owner, def.key);
        let mut healed = 0.0;
        if chosen == me {
            if !triggered && caster.legendary.has(305) && def.key == "regrowth" {
                legendary::effects::seed(caster, amount, duration);
            } else if !triggered && (caster.legendary.has(152) || caster.legendary.has(237)) {
                caster
                    .legendary
                    .effects
                    .active_heals
                    .push((amount, duration));
            } else {
                healed = apply(caster, amount, duration, credit, triggered);
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
                healed = apply(target, incoming, duration, credit, triggered);
            }
        }
        caster
            .healing_meter
            .record_hit(def.key, caster.elapsed, healed, false, false);
        chosen
    }
}

fn apply(
    target: &mut Run,
    amount: f32,
    duration: f32,
    credit: (Option<u64>, &'static str),
    triggered: bool,
) -> f32 {
    if duration <= 0.0 {
        if triggered {
            target.recover(amount)
        } else {
            legendary::events::receive_active_heal(target, amount)
        }
    } else {
        target.heal_rate = amount / duration.max(1.0);
        target.heal_t = duration;
        target.heal_credit = Some(credit);
        target.heal_triggered = triggered;
        0.0
    }
}
