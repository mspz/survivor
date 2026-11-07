//! Host-authoritative proximity revival, independent of either player's learned spells.
use super::{
    hero::Hero,
    multiplayer::{Companions, RemoteOwner},
    units::{self, Fx},
    Run,
};
use crate::net::{FieldChanged, Guid, ObjectStore};
use benilla_protocol::messages::{ObjectFields, ObjectType};
use bevy::prelude::*;

pub(super) const RADIUS: f32 = 5.0;
pub(super) const SECONDS: f32 = 5.0;
#[derive(Default)]
pub(super) struct State {
    pub enabled: bool,
    pub seconds: f32,
    pub assisting: f32,
    rescuer: Option<Entity>,
}
#[derive(Clone, Copy)]
struct Player {
    entity: Entity,
    pos: Vec3,
    alive: bool,
    can_rescue: bool,
}
impl State {
    fn advance(&mut self, corpse: Player, players: &[Player], dt: f32) -> bool {
        if corpse.alive {
            self.seconds = 0.0;
            self.rescuer = None;
            return false;
        }
        let eligible = |p: &&Player| {
            p.entity != corpse.entity
                && p.can_rescue
                && units::flat(p.pos - corpse.pos).length_squared() <= RADIUS * RADIUS
        };
        let rescuer = players
            .iter()
            .filter(eligible)
            .find(|p| Some(p.entity) == self.rescuer)
            .or_else(|| players.iter().find(eligible))
            .map(|p| p.entity);
        if rescuer != self.rescuer {
            self.seconds = 0.0;
            self.rescuer = rescuer;
        }
        if rescuer.is_none() {
            self.seconds = 0.0;
            return false;
        }
        self.seconds = (self.seconds + dt).min(SECONDS);
        self.seconds >= SECONDS - 0.0001
    }
}

/// Reward choices make a player untargetable, so they must not double as a safe revive.
fn can_rescue(run: &Run) -> bool {
    run.dead_t.is_none() && run.hp > 0.0 && run.pending_levels == 0
}

fn revive(run: &mut Run) {
    run.dead_t = None;
    run.hp = run.stats().max_hp * 0.5;
    run.immune_t = 2.0;
    run.shield = 0.0;
    run.shield_t = 0.0;
    run.heal_t = 0.0;
    run.heal_rate = 0.0;
    run.heal_credit = None;
    run.thorns_t = 0.0;
    run.thorns_dot = None;
    run.recasts = Default::default();
    run.revival.seconds = 0.0;
    run.revival.rescuer = None;
    run.version = run.version.wrapping_add(1);
}

pub(super) fn tick(
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut companions: Option<ResMut<Companions>>,
    hero: Query<(Entity, &Transform), With<Hero>>,
    remotes: Query<(Entity, &Transform, &RemoteOwner), Without<Hero>>,
    mut stores: Query<(&Guid, &mut ObjectStore)>,
    mut changes: MessageWriter<FieldChanged>,
    mut fx: Fx,
) {
    let Some(companions) = companions.as_deref_mut() else {
        run.revival = State::default();
        return;
    };
    if companions.0.is_empty() {
        run.revival = State::default();
        return;
    }
    let Ok((host, transform)) = hero.single() else {
        return;
    };
    let dt = time.delta_secs();
    if dt <= 0.0 || run.won {
        return;
    }
    let mut players = vec![Player {
        entity: host,
        pos: transform.translation,
        alive: run.dead_t.is_none() && run.hp > 0.0,
        can_rescue: can_rescue(&run),
    }];
    for (entity, transform, owner) in &remotes {
        if let Some(player) = companions.0.get(&owner.0) {
            players.push(Player {
                entity,
                pos: transform.translation,
                alive: player.run.dead_t.is_none() && player.run.hp > 0.0,
                can_rescue: can_rescue(&player.run),
            });
        }
    }
    run.revival.enabled = true;
    run.revival.assisting = 0.0;
    for player in companions.0.values_mut() {
        player.run.revival.enabled = true;
        player.run.revival.assisting = 0.0;
    }
    let mut assists = vec![];
    let mut completed = vec![];
    fn advance(
        run: &mut Run,
        corpse: Player,
        players: &[Player],
        dt: f32,
        assists: &mut Vec<(Entity, f32)>,
        completed: &mut Vec<(Entity, Entity, f32, f32)>,
    ) {
        let ready = run.revival.advance(corpse, players, dt);
        if let Some(rescuer) = run.revival.rescuer {
            assists.push((rescuer, run.revival.seconds));
            if ready {
                revive(run);
                completed.push((corpse.entity, rescuer, run.hp, run.stats().max_hp));
            }
        }
    }
    advance(
        &mut run,
        players[0],
        &players,
        dt,
        &mut assists,
        &mut completed,
    );
    for corpse in players.iter().copied().skip(1) {
        if let Some(player) = companions
            .0
            .values_mut()
            .find(|p| p.entity == corpse.entity)
        {
            advance(
                &mut player.run,
                corpse,
                &players,
                dt,
                &mut assists,
                &mut completed,
            );
        }
    }
    for (entity, seconds) in assists {
        if entity == host {
            run.revival.assisting = run.revival.assisting.max(seconds);
        } else if let Some(player) = companions.0.values_mut().find(|p| p.entity == entity) {
            player.run.revival.assisting = player.run.revival.assisting.max(seconds);
        }
    }
    for (entity, rescuer, health, maximum) in completed {
        if let Ok((guid, mut store)) = stores.get_mut(entity) {
            store.0.merge_diff(
                ObjectFields::from_pairs(&[
                    (22, health.ceil() as u32),
                    (28, maximum.ceil() as u32),
                ]),
                |index, old, new| {
                    changes.write(FieldChanged {
                        entity,
                        guid: guid.0,
                        kind: ObjectType::Unit,
                        index,
                        old,
                        new,
                    });
                },
            );
        }
        fx.cast(rescuer, 2006, &[entity], None);
        fx.impact(entity, 2006);
    }
}

pub(super) fn hint(run: &Run) -> Option<String> {
    if !run.revival.enabled {
        return None;
    }
    if run.dead_t.is_some() {
        Some(if run.revival.seconds > 0.0 {
            format!(
                "An ally is reviving you: {:.1} / 5 sec",
                run.revival.seconds
            )
        } else {
            "Fallen: an ally can revive you by staying within 5 yd for 5 sec.".into()
        })
    } else if run.revival.assisting > 0.0 {
        Some(format!(
            "Reviving ally: {:.1} / 5 sec — stay within 5 yd",
            run.revival.assisting
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::super::{multiplayer::Companion, units::Marks};
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    fn player(id: u32, x: f32, alive: bool) -> Player {
        Player {
            entity: Entity::from_raw_u32(id).unwrap(),
            pos: Vec3::X * x,
            alive,
            can_rescue: alive,
        }
    }
    #[test]
    fn revive_needs_five_continuous_seconds_inside_the_five_yard_radius() {
        let corpse = player(1, 0.0, false);
        let rescuers = [player(2, 5.0, true), player(3, 1.0, true)];
        let mut state = State::default();
        for _ in 0..49 {
            assert!(!state.advance(corpse, &rescuers, 0.1));
        }
        assert!(state.advance(corpse, &rescuers, 0.1));
        assert!(
            (state.seconds - 5.0).abs() < 0.001,
            "multiple allies must not accelerate revival"
        );
        state = State::default();
        assert!(!state.advance(corpse, &[player(2, 5.01, true)], 6.0));
        assert_eq!(state.seconds, 0.0);
    }
    #[test]
    fn leaving_dying_disconnecting_or_changing_rescuer_resets_the_channel() {
        let corpse = player(1, 0.0, false);
        for interrupted in [
            vec![player(2, 6.0, true)],
            vec![player(2, 0.0, false)],
            vec![],
        ] {
            let mut state = State::default();
            assert!(!state.advance(corpse, &[player(2, 0.0, true)], 3.0));
            assert!(!state.advance(corpse, &interrupted, 1.0));
            assert_eq!(state.seconds, 0.0);
            assert!(!state.advance(corpse, &[player(3, 0.0, true)], 4.0));
            assert!(state.advance(corpse, &[player(3, 0.0, true)], 1.0));
        }
        let mut state = State::default();
        state.advance(corpse, &[player(2, 0.0, true)], 3.0);
        assert!(!state.advance(corpse, &[player(3, 0.0, true)], 2.0));
        assert_eq!(state.seconds, 2.0);
    }
    #[test]
    fn dead_players_cannot_revive_themselves_or_each_other() {
        let corpse = player(1, 0.0, false);
        let mut state = State::default();
        assert!(!state.advance(corpse, &[corpse, player(2, 0.0, false)], 10.0));
        assert_eq!(state.seconds, 0.0);
    }
    fn world(host_dead: bool, guest_dead: bool) -> (World, Entity, Entity) {
        use crate::{
            combat_text::CombatTextSpawn,
            creature_anim::{CastEvent, PlaySeq, SpellGoTargets, SwingMessage},
        };
        let mut world = World::new();
        world.init_resource::<Time>();
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(1));
        world.init_resource::<PlaySeq>();
        world.init_resource::<Messages<CastEvent>>();
        world.init_resource::<Messages<SpellGoTargets>>();
        world.init_resource::<Messages<SwingMessage>>();
        world.init_resource::<Messages<CombatTextSpawn>>();
        world.init_resource::<Messages<FieldChanged>>();
        world.insert_resource(Run {
            hp: if host_dead { 0.0 } else { 120.0 },
            dead_t: host_dead.then_some(4.0),
            ..default()
        });
        let host = world
            .spawn((
                Hero { vy: 0.0, air: 0.0 },
                Transform::default(),
                Guid(10),
                ObjectStore(ObjectFields::from_pairs(&[(
                    22,
                    if host_dead { 0 } else { 120 },
                )])),
                Marks::buffs(),
            ))
            .id();
        let guest = world
            .spawn((
                RemoteOwner(42),
                Transform::from_xyz(5.0, 0.0, 0.0),
                Guid(20),
                ObjectStore(ObjectFields::from_pairs(&[(
                    22,
                    if guest_dead { 0 } else { 120 },
                )])),
                Marks::buffs(),
            ))
            .id();
        let mut players = Companions::default();
        players.0.insert(
            42,
            Companion {
                entity: guest,
                run: Run {
                    owner: Some(42),
                    hp: if guest_dead { 0.0 } else { 120.0 },
                    dead_t: guest_dead.then_some(4.0),
                    ..default()
                },
                physics: Hero { vy: 0.0, air: 0.0 },
                offer: default(),
                offer_id: 0,
            },
        );
        world.insert_resource(players);
        (world, host, guest)
    }
    #[test]
    fn host_and_guests_can_revive_each_other_with_live_health_fields_and_grace_immunity() {
        for host_dead in [false, true] {
            let (mut world, host, guest) = world(host_dead, !host_dead);
            for _ in 0..4 {
                world.run_system_once(tick).unwrap();
            }
            let fallen = if host_dead {
                &*world.resource::<Run>()
            } else {
                &world.resource::<Companions>().0[&42].run
            };
            assert!(fallen.dead_t.is_some());
            assert_eq!(fallen.revival.seconds, 4.0);
            world.run_system_once(tick).unwrap();
            let fallen = if host_dead {
                &*world.resource::<Run>()
            } else {
                &world.resource::<Companions>().0[&42].run
            };
            assert!(fallen.dead_t.is_none());
            assert_eq!(fallen.hp, fallen.stats().max_hp * 0.5);
            assert_eq!(fallen.immune_t, 2.0);
            let entity = if host_dead { host } else { guest };
            assert_eq!(
                world.get::<ObjectStore>(entity).unwrap().0.unit_health(),
                Some(fallen.hp.ceil() as u32)
            );
            assert!(!world.resource::<Messages<FieldChanged>>().is_empty());
            assert!(!world
                .resource::<Messages<crate::creature_anim::SpellGoTargets>>()
                .is_empty());
        }
    }
    #[test]
    fn players_choosing_rewards_cannot_revive_but_can_be_revived() {
        for host_rescues in [true, false] {
            let (mut world, _, _) = world(!host_rescues, host_rescues);
            let choosing = |world: &mut World, levels| {
                if host_rescues {
                    world.resource_mut::<Run>().pending_levels = levels;
                } else {
                    world
                        .resource_mut::<Companions>()
                        .0
                        .get_mut(&42)
                        .unwrap()
                        .run
                        .pending_levels = levels;
                }
            };
            let fallen = |world: &World| {
                let run = if host_rescues {
                    &world.resource::<Companions>().0[&42].run
                } else {
                    world.resource::<Run>()
                };
                (run.dead_t.is_some(), run.revival.seconds)
            };
            choosing(&mut world, 1);
            for _ in 0..6 {
                world.run_system_once(tick).unwrap();
            }
            assert_eq!(fallen(&world), (true, 0.0));
            choosing(&mut world, 0);
            for _ in 0..3 {
                world.run_system_once(tick).unwrap();
            }
            assert_eq!(fallen(&world), (true, 3.0));
            choosing(&mut world, 1);
            world.run_system_once(tick).unwrap();
            assert_eq!(
                fallen(&world),
                (true, 0.0),
                "choosing interrupts the channel"
            );
        }
        let (mut world, _, _) = world(true, false);
        world.resource_mut::<Run>().pending_levels = 1;
        for _ in 0..5 {
            world.run_system_once(tick).unwrap();
        }
        assert!(world.resource::<Run>().dead_t.is_none());
    }
    #[test]
    fn full_party_wipes_cannot_resurrect_and_solo_remains_disabled() {
        let (mut world, _, _) = world(true, true);
        for _ in 0..6 {
            world.run_system_once(tick).unwrap();
        }
        assert!(world.resource::<Run>().dead_t.is_some());
        assert!(world.resource::<Companions>().0[&42].run.dead_t.is_some());
        world.resource_mut::<Companions>().0.clear();
        world.run_system_once(tick).unwrap();
        assert!(!world.resource::<Run>().revival.enabled);
        assert!(hint(world.resource::<Run>()).is_none());
    }
}
