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
    fn visit(
        &mut self,
        caster: &mut Run,
        me: Entity,
        center: Vec3,
        radius: f32,
        mut apply: impl FnMut(Entity, &mut Run),
    ) {
        let positions = &self.positions;
        let near = |entity| {
            positions
                .get(entity)
                .is_ok_and(|t| super::units::flat(t.translation - center).length() <= radius)
        };
        if near(me) && health(caster).is_some() {
            apply(me, caster);
        }
        if let Some(host) = self.host.as_deref_mut() {
            if host.entity != me && near(host.entity) && health(&host.run).is_some() {
                apply(host.entity, &mut host.run);
            }
        }
        if let Some(players) = self.players.as_deref_mut() {
            for player in players.0.values_mut() {
                if player.entity != me && near(player.entity) && health(&player.run).is_some() {
                    apply(player.entity, &mut player.run);
                }
            }
        }
    }
    pub fn buff(
        &mut self,
        caster: &mut Run,
        me: Entity,
        center: Vec3,
        radius: f32,
        key: &'static str,
        power: f32,
        duration: f32,
    ) {
        let owner = caster.owner;
        self.visit(caster, me, center, radius, |_, run| {
            super::support_spells::grant(run, key, power, duration, owner)
        });
    }
    pub fn process_support(&mut self, caster: &mut Run, me: Entity, commands: &mut Commands) {
        let now = caster.elapsed;
        caster.support.buffs.retain(|b| b.until > now);
        let center = self.positions.get(me).map_or(Vec3::ZERO, |t| t.translation);
        for (owner, amount) in std::mem::take(&mut caster.support.credits) {
            if owner == caster.owner {
                caster
                    .healing_meter
                    .record_hit("prayer_of_mending", now, amount, false, false);
            } else if let Some(host) = self.host.as_deref_mut().filter(|h| h.run.owner == owner) {
                host.run.healing_meter.record_hit(
                    "prayer_of_mending",
                    host.run.elapsed,
                    amount,
                    false,
                    false,
                );
            } else if let Some(source) = self
                .players
                .as_deref_mut()
                .and_then(|p| p.0.values_mut().find(|p| p.run.owner == owner))
            {
                source.run.healing_meter.record_hit(
                    "prayer_of_mending",
                    source.run.elapsed,
                    amount,
                    false,
                    false,
                );
            }
        }
        for buff in std::mem::take(&mut caster.support.transfers) {
            let mut chosen = None;
            let mut lowest = f32::INFINITY;
            self.visit(caster, me, center, 20.0, |entity, run| {
                let fraction = health(run).unwrap_or(f32::INFINITY);
                if entity != me && fraction < lowest {
                    lowest = fraction;
                    chosen = Some(entity);
                }
            });
            if let Some(chosen) = chosen {
                self.visit(caster, me, center, 20.0, |entity, run| {
                    if entity == chosen && buff.until > now {
                        run.support.grant(buff.clone());
                    }
                });
            }
        }
        let mut zones = std::mem::take(&mut caster.support.zones);
        for zone in &mut zones {
            if zone.until <= now {
                commands.entity(zone.entity).despawn();
                continue;
            }
            if caster.dead_t.is_some() || zone.next > now {
                continue;
            }
            if zone.key == "healing_stream_totem" {
                let owner = caster.owner;
                let outgoing = zone.power * (1.0 + caster.equipment.healing).max(0.0);
                let mut total = 0.0;
                self.visit(caster, me, zone.pos, zone.radius, |_, target| {
                    let amount = outgoing / (1.0 + target.equipment.healing).max(0.001);
                    total += apply(target, amount, 0.0, (owner, zone.key), false);
                });
                caster
                    .healing_meter
                    .record_hit(zone.key, now, total, false, true);
                zone.next = now + 2.0;
            } else {
                self.buff(caster, me, zone.pos, zone.radius, zone.key, zone.power, 1.5);
                zone.next = now + 1.0;
            }
        }
        zones.retain(|z| z.until > now);
        caster.support.zones = zones;
    }
    pub fn urgent(&self, caster: &Run, me: Entity, range: f32) -> bool {
        let origin = self.positions.get(me).map_or(Vec3::ZERO, |t| t.translation);
        let urgent = |entity, run: &Run| {
            health(run).is_some_and(|h| h < 0.35)
                && self
                    .positions
                    .get(entity)
                    .is_ok_and(|t| super::units::flat(t.translation - origin).length() <= range)
        };
        urgent(me, caster)
            || self
                .host
                .as_deref()
                .is_some_and(|h| urgent(h.entity, &h.run))
            || self
                .players
                .as_deref()
                .is_some_and(|p| p.0.values().any(|p| urgent(p.entity, &p.run)))
    }
    pub fn area(&mut self, caster: &mut Run, me: Entity, center: Vec3, radius: f32, amount: f32) {
        let credit = (caster.owner, "efflorescence");
        let near = |entity| {
            self.positions
                .get(entity)
                .is_ok_and(|t| super::units::flat(t.translation - center).length() <= radius)
        };
        let mut total = 0.0;
        let outgoing = amount * (1.0 + caster.equipment.healing).max(0.0);
        if near(me) && health(caster).is_some() {
            total += apply(caster, amount, 0.0, credit, false);
        }
        let mut heal = |entity, target: &mut Run| {
            if entity != me && near(entity) && health(target).is_some() {
                let amount = outgoing / (1.0 + target.equipment.healing).max(0.001);
                total += apply(target, amount, 0.0, credit, false);
            }
        };
        if let Some(host) = self.host.as_deref_mut() {
            heal(host.entity, &mut host.run);
        }
        if let Some(players) = self.players.as_deref_mut() {
            for p in players.0.values_mut() {
                heal(p.entity, &mut p.run);
            }
        }
        caster
            .healing_meter
            .record_hit("efflorescence", caster.elapsed, total, false, true);
    }
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
        if def.key == "prayer_of_mending" {
            let amount = amount * (1.0 + caster.equipment.healing).max(0.0);
            if chosen == me {
                super::support_spells::grant(caster, def.key, amount, duration, credit.0);
            } else {
                let target =
                    if let Some(host) = self.host.as_deref_mut().filter(|h| h.entity == chosen) {
                        Some(&mut host.run)
                    } else {
                        self.players
                            .as_deref_mut()
                            .and_then(|p| p.0.values_mut().find(|p| p.entity == chosen))
                            .map(|p| &mut p.run)
                    };
                if let Some(target) = target {
                    super::support_spells::grant(target, def.key, amount, duration, credit.0);
                }
            }
            return chosen;
        }
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
