//! Rare expedition casters: zone-themed schools, ranged movement and travelling spell hits.
use super::{
    data, enemies, hero,
    spell_engine::Tags,
    units::{Fx, COLOR_HURT},
    Rng, Run,
};
use bevy::prelude::*;

#[derive(Component, Clone, Copy, Debug)]
pub(super) struct Caster {
    pub spell: u32,
    pub tags: Tags,
    pub range: f32,
    pub stand_off: f32,
    pub interval: f32,
    name: &'static str,
    cast: Option<(Entity, f32)>,
}
impl Caster {
    pub fn cancel(&mut self) {
        self.cast = None;
    }
    pub fn casting(&self) -> bool {
        self.cast.is_some()
    }
    /// A new target always requires a full second before the projectile can launch.
    pub fn ready(&mut self, target: Option<Entity>, dt: f32) -> bool {
        let Some(target) = target else {
            self.cancel();
            return false;
        };
        if let Some((previous, left)) = &mut self.cast {
            if *previous == target {
                *left -= dt;
                if *left <= 0.00001 {
                    self.cancel();
                    return true;
                }
                return false;
            }
        }
        self.cast = Some((target, 1.0));
        false
    }
}
pub(super) fn profile(map: usize) -> Option<Caster> {
    let (name, spell, school) = match data::CAMPAIGN.iter().position(|m| *m == map)? {
        1 => ("Riverpaw Mystic", 5176, Tags::NATURE),
        2 => ("Blackrock Summoner", 133, Tags::FIRE),
        3 => ("Skeletal Mage", 116, Tags::COLD),
        4 => ("Hakkari Shadowcaster", 686, Tags::SHADOW),
        5 => ("Shadowsworn Cultist", 133, Tags::FIRE),
        _ => return None,
    };
    Some(Caster {
        name,
        spell,
        tags: school | Tags::SPELL | Tags::PROJECTILE | Tags::DAMAGE,
        range: 20.0,
        stand_off: 16.0,
        interval: 3.0,
        cast: None,
    })
}
pub(super) fn spawn(
    commands: &mut Commands,
    run: &mut Run,
    rng: &mut Rng,
    normal: &'static data::EnemyDef,
    pos: Vec3,
    hero: Vec3,
) -> Entity {
    let caster =
        profile(run.expedition_map).filter(|_| rng.f32() < run.expeditions.ranged_chance());
    let def = caster.map_or(normal, |caster| {
        data::roster(run.expedition_map)
            .enemies
            .iter()
            .find(|d| d.name == caster.name)
            .expect("authored caster belongs to its expedition")
    });
    let entity = enemies::spawn_mob(commands, run, rng, def, pos, hero);
    if let Some(caster) = caster {
        commands.entity(entity).insert(caster);
    }
    entity
}

pub(super) struct Missile {
    pub attacker: Entity,
    pub target: Entity,
    pub owner: Option<u64>,
    pub caster: Caster,
    pub damage: f32,
    left: f32,
}
#[derive(Default)]
pub(super) struct Pending {
    missiles: Vec<Missile>,
    elapsed: f32,
}
impl Pending {
    pub fn launch(
        &mut self,
        attacker: Entity,
        target: Entity,
        owner: Option<u64>,
        caster: Caster,
        damage: f32,
        distance: f32,
        speed: f32,
    ) {
        self.missiles.push(Missile {
            attacker,
            target,
            owner,
            caster,
            damage,
            left: distance / speed.max(1.0),
        });
    }
    pub fn advance(&mut self, elapsed: f32, dt: f32) -> Vec<Missile> {
        if elapsed < self.elapsed {
            self.missiles.clear();
        }
        self.elapsed = elapsed;
        let mut due = vec![];
        for mut missile in std::mem::take(&mut self.missiles) {
            missile.left -= dt;
            if missile.left <= 0.0 {
                due.push(missile);
            } else {
                self.missiles.push(missile);
            }
        }
        due
    }
}
pub(super) fn impact(missile: Missile, run: &mut Run, fx: &mut Fx) {
    if run.dead_t.is_some() || run.pending_levels > 0 {
        return;
    }
    run.legendary.events.attacker = Some(missile.attacker);
    run.taken_source = missile.caster.name;
    let raw = missile.damage * (1.0 - run.support.power(run.elapsed, "smoke_bomb"));
    let damage = hero::hurt_hero_tagged(run, raw, missile.caster.tags);
    run.legendary.events.attacker = None;
    fx.impact(missile.target, missile.caster.spell);
    if damage > 0.0 {
        fx.number(run.owner, missile.target, damage, false, COLOR_HURT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    #[test]
    fn cast_requires_one_full_second_and_restarts_after_interruption_or_target_change() {
        let mut caster = profile(data::CAMPAIGN[1]).unwrap();
        let first = Entity::from_raw_u32(1).unwrap();
        let second = Entity::from_raw_u32(2).unwrap();
        assert!(!caster.ready(Some(first), 0.5));
        assert!(!caster.ready(Some(first), 0.9));
        assert!(caster.ready(Some(first), 0.1));
        assert!(!caster.ready(Some(first), 0.1));
        assert!(!caster.ready(None, 1.0));
        assert!(!caster.ready(Some(first), 1.0));
        assert!(!caster.ready(Some(second), 1.0));
        assert!(!caster.ready(Some(second), 0.9));
        assert!(caster.ready(Some(second), 0.1));
    }

    fn combat() -> (World, Entity, Entity) {
        use crate::{
            combat_text::CombatTextSpawn,
            creature_anim::{CastEvent, PlaySeq, SpellGoTargets, SwingMessage},
            net::FieldChanged,
        };
        let mut world = World::new();
        benilla_world::world_point::init_world_point_resources(&mut world);
        world.init_resource::<Time>();
        world.insert_resource(Rng(95));
        world.init_resource::<PlaySeq>();
        world.init_resource::<Messages<CastEvent>>();
        world.init_resource::<Messages<SpellGoTargets>>();
        world.init_resource::<Messages<SwingMessage>>();
        world.init_resource::<Messages<CombatTextSpawn>>();
        world.init_resource::<Messages<FieldChanged>>();
        world.init_resource::<Messages<super::super::abilities::Hit>>();
        let mut run = Run {
            hp: 100.0,
            equipment_armour: 1.0e6,
            expedition_map: data::CAMPAIGN[1],
            elapsed: 1.0,
            ..default()
        };
        run.equipment.resistances.nature = 50.0;
        world.insert_resource(run);
        let hero = world
            .spawn((hero::Hero { vy: 0.0, air: 0.0 }, Transform::default()))
            .id();
        let enemy = world
            .run_system_once(|mut commands: Commands, mut run: ResMut<Run>| {
                enemies::spawn_enemy(
                    &mut commands,
                    &mut run,
                    &data::roster(data::CAMPAIGN[1]).enemies[0],
                    Vec3::X * 16.0,
                    Vec3::ZERO,
                    false,
                )
            })
            .unwrap();
        world
            .entity_mut(enemy)
            .insert(profile(data::CAMPAIGN[1]).unwrap());
        {
            let mut e = world.get_mut::<enemies::Enemy>(enemy).unwrap();
            e.damage = 20.0;
            e.attack_cd = 0.0;
        }
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(50));
        (world, hero, enemy)
    }
    #[test]
    fn bosses_and_elites_move_while_frozen_and_slow_is_capped_at_fifty_percent() {
        for rank in [
            enemies::Rank::Normal,
            enemies::Rank::Elite,
            enemies::Rank::Boss,
        ] {
            let (mut world, _, enemy) = combat();
            world.entity_mut(enemy).remove::<Caster>();
            world.entity_mut(enemy).insert(rank);
            {
                let mut en = world.get_mut::<enemies::Enemy>(enemy).unwrap();
                en.boss = rank == enemies::Rank::Boss;
                en.speed = 10.0;
                en.root_t = 2.0;
                en.slow = 0.1;
                en.slow_t = 2.0;
            }
            world.run_system_once(enemies::move_enemies).unwrap();
            let en = world.get::<enemies::Enemy>(enemy).unwrap();
            assert!(en.root_t > 0.0, "freeze state must remain available");
            let resistant = rank != enemies::Rank::Normal;
            assert!(
                (world.get::<Transform>(enemy).unwrap().translation.x
                    - if resistant { 15.75 } else { 16.0 })
                .abs()
                    < 0.001
            );
            assert_eq!(en.slow, if resistant { 0.5 } else { 0.1 });
            world.get_mut::<enemies::Enemy>(enemy).unwrap().root_t = 0.0;
            let before = world.get::<Transform>(enemy).unwrap().translation.x;
            world.run_system_once(enemies::move_enemies).unwrap();
            let after = world.get::<Transform>(enemy).unwrap().translation.x;
            assert!((before - after - if resistant { 0.25 } else { 0.05 }).abs() < 0.001);
        }
    }

    #[test]
    fn distant_monsters_walk_towards_host_or_guest_at_their_speed_and_respect_stuns() {
        use super::super::multiplayer::{Companion, Companions, RemoteOwner};
        for guest_target in [false, true] {
            for ranged in [false, true] {
                for stunned in [false, true] {
                    for distance in [60.0, 1000.0] {
                        let (mut world, host, enemy) = combat();
                        if !ranged {
                            world.entity_mut(enemy).remove::<Caster>();
                        }
                        world.get_mut::<Transform>(enemy).unwrap().translation = Vec3::X * distance;
                        if guest_target {
                            world.get_mut::<Transform>(host).unwrap().translation =
                                -Vec3::X * distance;
                            let entity = world.spawn((RemoteOwner(42), Transform::default())).id();
                            let mut companions = Companions::default();
                            companions.0.insert(
                                42,
                                Companion {
                                    entity,
                                    run: Run {
                                        owner: Some(42),
                                        ..default()
                                    },
                                    physics: hero::Hero { vy: 0.0, air: 0.0 },
                                    offer: default(),
                                    offer_id: 0,
                                },
                            );
                            world.insert_resource(companions);
                        }
                        if stunned {
                            world.get_mut::<enemies::Enemy>(enemy).unwrap().stun_t = 2.0;
                        }
                        let speed = world.get::<enemies::Enemy>(enemy).unwrap().speed;
                        world.run_system_once(enemies::move_enemies).unwrap();
                        let position = world.get::<Transform>(enemy).unwrap().translation;
                        let expected = distance - if stunned || (ranged && distance <= 20.0) { 0.0 } else { speed * 0.05 };
                        assert!((position.x - expected).abs() < 0.001, "guest={guest_target}, ranged={ranged}, stunned={stunned}, distance={distance}: {position:?}");
                        assert_eq!(position.z, 0.0);
                    }
                }
            }
        }
    }
    #[test]
    fn reward_selection_hides_host_and_guests_from_melee_casters_bosses_and_taunts() {
        use super::super::{
            multiplayer::{Companion, Companions, RemoteOwner},
            talent_runtime::Taunted,
        };
        use crate::creature_anim::{MovementState, SpellGoTargets, SwingMessage};
        for protect_guest in [false, true] {
            for boss in [false, true] {
                for ranged in [false, true] {
                    let (mut world, host, enemy) = combat();
                    let guest = world
                        .spawn((RemoteOwner(42), Transform::from_xyz(12.0, 0.0, 0.0)))
                        .id();
                    let mut guest_run = Run {
                        owner: Some(42),
                        ..default()
                    };
                    guest_run.pending_levels = u32::from(protect_guest);
                    world.resource_mut::<Run>().pending_levels = u32::from(!protect_guest);
                    let mut players = Companions::default();
                    players.0.insert(
                        42,
                        Companion {
                            entity: guest,
                            run: guest_run,
                            physics: hero::Hero { vy: 0.0, air: 0.0 },
                            offer: default(),
                            offer_id: 0,
                        },
                    );
                    world.insert_resource(players);
                    world.get_mut::<enemies::Enemy>(enemy).unwrap().boss = boss;
                    world.entity_mut(enemy).insert(Taunted {
                        owner: protect_guest.then_some(42),
                        until: 10.0,
                    });
                    if !ranged {
                        world.entity_mut(enemy).remove::<Caster>();
                        world.get_mut::<Transform>(enemy).unwrap().translation.x =
                            if protect_guest { 0.1 } else { 11.9 };
                    }
                    let system = world.register_system(enemies::move_enemies);
                    world.run_system(system).unwrap();
                    let expected = if protect_guest { host } else { guest };
                    if ranged {
                        assert!(world.resource::<Messages<SpellGoTargets>>().is_empty());
                        world.resource_mut::<Time>().advance_by(std::time::Duration::from_secs(1));
                        world.resource_mut::<Run>().elapsed += 1.0;
                        world.run_system(system).unwrap();
                        let cast = world
                            .resource_mut::<Messages<SpellGoTargets>>()
                            .drain()
                            .next()
                            .unwrap();
                        assert_eq!(cast.hits, vec![expected]);
                    } else {
                        let swing = world
                            .resource_mut::<Messages<SwingMessage>>()
                            .drain()
                            .next()
                            .unwrap();
                        assert_eq!(swing.victim, Some(expected));
                    }
                    world.resource_mut::<Run>().pending_levels = 1;
                    world
                        .resource_mut::<Companions>()
                        .0
                        .get_mut(&42)
                        .unwrap()
                        .run
                        .pending_levels = 1;
                    world.get_mut::<enemies::Enemy>(enemy).unwrap().attack_cd = 0.0;
                    let before = world.get::<Transform>(enemy).unwrap().translation;
                    world.run_system(system).unwrap();
                    assert!(world.resource::<Messages<SpellGoTargets>>().is_empty());
                    assert!(world.resource::<Messages<SwingMessage>>().is_empty());
                    assert_eq!(world.get::<MovementState>(enemy).unwrap().speed, 0.0);
                    assert_eq!(world.get::<Transform>(enemy).unwrap().translation, before);
                    world.resource_mut::<Run>().pending_levels = 0;
                    world
                        .resource_mut::<Companions>()
                        .0
                        .get_mut(&42)
                        .unwrap()
                        .run
                        .pending_levels = 0;
                    world.run_system(system).unwrap();
                    assert!(
                        !world.resource::<Messages<SpellGoTargets>>().is_empty()
                            || !world.resource::<Messages<SwingMessage>>().is_empty()
                            || world.get::<MovementState>(enemy).unwrap().speed > 0.0
                            || world.get::<Caster>(enemy).is_some_and(Caster::casting)
                    );
                }
            }
        }
    }
    #[test]
    fn casters_hold_range_and_projectiles_use_resistance_instead_of_armour_or_melee_block() {
        let (mut world, _, enemy) = combat();
        world.resource_mut::<Run>().equipment.block = 1.0;
        let system = world.register_system(enemies::move_enemies);
        world.run_system(system).unwrap();
        assert_eq!(
            world.resource::<Run>().hp,
            100.0,
            "missile must travel before damage"
        );
        assert_eq!(
            world
                .get::<crate::creature_anim::MovementState>(enemy)
                .unwrap()
                .speed,
            0.0
        );
        let casts = world.resource::<Messages<crate::creature_anim::SpellGoTargets>>();
        assert_eq!(casts.len(), 0, "caster must wind up before launching");
        world.resource_mut::<Time>().advance_by(std::time::Duration::from_secs(1));
        world.resource_mut::<Run>().elapsed += 1.0;
        world.run_system(system).unwrap();
        assert_eq!(world.resource::<Messages<crate::creature_anim::SpellGoTargets>>().len(), 1);
        assert_eq!(world.resource::<Run>().hp, 100.0);
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(1));
        world.resource_mut::<Run>().elapsed += 1.0;
        world.run_system(system).unwrap();
        assert!((world.resource::<Run>().hp - 90.0).abs() < 0.001);
        assert!(world
            .resource::<Messages<crate::creature_anim::SwingMessage>>()
            .is_empty());
    }
    #[test]
    fn ranged_missiles_hit_the_selected_guest_and_stuns_stop_new_casts() {
        use super::super::multiplayer::{Companion, Companions, RemoteOwner};
        let (mut world, _, enemy) = combat();
        let entity = world
            .spawn((RemoteOwner(42), Transform::from_xyz(12.0, 0.0, 0.0)))
            .id();
        let mut guest = Run {
            owner: Some(42),
            hp: 100.0,
            ..default()
        };
        guest.equipment.resistances.nature = 25.0;
        let mut players = Companions::default();
        players.0.insert(
            42,
            Companion {
                entity,
                run: guest,
                physics: hero::Hero { vy: 0.0, air: 0.0 },
                offer: default(),
                offer_id: 0,
            },
        );
        world.insert_resource(players);
        let system = world.register_system(enemies::move_enemies);
        world.get_mut::<enemies::Enemy>(enemy).unwrap().stun_t = 2.0;
        world.run_system(system).unwrap();
        assert!(world
            .resource::<Messages<crate::creature_anim::SpellGoTargets>>()
            .is_empty());
        world.get_mut::<enemies::Enemy>(enemy).unwrap().stun_t = 0.0;
        world.run_system(system).unwrap();
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(1));
        world.resource_mut::<Run>().elapsed += 1.0;
        world.run_system(system).unwrap();
        assert_eq!(world.resource::<Companions>().0[&42].run.hp, 100.0, "only launched after the cast time");
        world.resource_mut::<Run>().elapsed += 1.0;
        world.run_system(system).unwrap();
        assert_eq!(world.resource::<Run>().hp, 100.0);
        assert!((world.resource::<Companions>().0[&42].run.hp - 85.0).abs() < 0.001);
    }
    #[test]
    fn first_zone_and_non_campaign_maps_have_no_casters_and_other_zones_use_real_roster_casters() {
        for (tier, map) in data::CAMPAIGN.iter().enumerate() {
            let caster = profile(*map);
            if tier == 0 {
                assert!(caster.is_none());
                continue;
            }
            let caster = caster.unwrap();
            assert!(!caster.tags.contains(Tags::PHYSICAL));
            assert!(data::roster(*map)
                .enemies
                .iter()
                .any(|d| d.name == caster.name));
            assert_eq!(
                super::super::spell_engine::tags::school(
                    data::ABILITIES
                        .iter()
                        .find(|d| d.spell == caster.spell)
                        .unwrap()
                        .key
                )
                .unwrap(),
                Tags(caster.tags.0 & (Tags::NATURE | Tags::FIRE | Tags::COLD | Tags::SHADOW).0)
            );
        }
        assert!(profile(data::TRAINING_MAP).is_none());
    }
    #[test]
    fn caster_selection_is_two_percent_and_never_adds_extra_monsters() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        world.insert_resource(Run {
            expedition_map: data::CAMPAIGN[1],
            ..default()
        });
        world.insert_resource(Rng(784));
        world
            .run_system_once(
                |mut commands: Commands, mut run: ResMut<Run>, mut rng: ResMut<Rng>| {
                    for _ in 0..10000 {
                        spawn(
                            &mut commands,
                            &mut run,
                            &mut rng,
                            &data::roster(data::CAMPAIGN[1]).enemies[0],
                            Vec3::ZERO,
                            Vec3::ZERO,
                        );
                    }
                },
            )
            .unwrap();
        let casters = world.query::<&Caster>().iter(&world).count();
        assert!((150..=250).contains(&casters), "{casters}/10000");
        assert_eq!(world.query::<&enemies::Enemy>().iter(&world).count(), 10000);
    }
    #[test]
    fn missiles_travel_once_and_are_cleared_when_the_expedition_clock_resets() {
        let mut pending = Pending::default();
        let entity = Entity::from_raw_u32(42).unwrap();
        let caster = profile(data::CAMPAIGN[1]).unwrap();
        pending.launch(entity, entity, None, caster, 100.0, 20.0, 20.0);
        assert!(pending.advance(0.5, 0.5).is_empty());
        assert_eq!(pending.advance(1.0, 0.5).len(), 1);
        assert!(pending.advance(2.0, 1.0).is_empty());
        pending.launch(entity, entity, None, caster, 100.0, 20.0, 20.0);
        assert!(pending.advance(0.0, 0.1).is_empty());
        assert!(pending.missiles.is_empty());
    }
}
