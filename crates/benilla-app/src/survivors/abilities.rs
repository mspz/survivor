//! Auto-cast: every owned ability fires on its own cooldown at the targets its kind picks, with
//! the spell's own visuals; damage lands when its missile would.

use bevy::prelude::*;

use super::data::{AbilityDef, Effect, Kind};
use super::enemies::{self, Enemy, Loot};
use super::hero::{self, Hero};
use super::spell_engine::targeting::{around, densest, links, nearest, toughest, Target as Foe};
use super::spell_engine::{self as engine, Resolved as Params};
use super::units::{self, flat, Fx, Marks, UnitSpec, COLOR_HEAL, COLOR_MELEE, COLOR_SPELL};
use super::{Rng, Run, RunEntity};
use crate::net::{FieldChanged, Guid, ObjectStore};

/// One blow landing on one enemy.
#[derive(Message, Clone)]
pub(super) struct Hit {
    pub owner: Option<u64>,
    pub skill: &'static str,
    /// Display attribution only; spell rules and visuals still use `skill`.
    pub meter_source: Option<&'static str>,
    /// Triggered hits may benefit from gear, but never trigger another trinket proc.
    pub triggered: bool,
    /// Hidden payloads are resolved once before target copies and periodic snapshots.
    pub proc_resolved: bool,
    /// Milestones already visited by this hit's proc ancestry; copies retain it.
    pub proc_chain: u64,
    pub target: Entity,
    pub damage: f32,
    pub dot: f32,
    pub dot_dur: f32,
    pub effect: Effect,
    pub mark: u32,
    pub mark_dur: f32,
    /// Also hits everything this close to the target, for less.
    pub splash: f32,
    pub crit: bool,
    pub color: u32,
}

#[cfg(test)]
mod spell_engine_integration_tests {
    use super::super::{data, Owned};
    use super::*;
    use crate::combat_text::CombatTextSpawn;
    use crate::creature_anim::{CastEvent, PlaySeq, SpellGoTargets, SwingMessage};
    use bevy::ecs::system::RunSystemOnce;
    #[test]
    fn guest_hidden_projectiles_resolve_gear_once_and_keep_their_snapshot_on_links() {
        use super::super::{
            balance,
            multiplayer::{Companion, Companions},
            spell_engine::{Change, Filter, Modifier, Tags},
        };
        let mut world = World::new();
        world.init_resource::<Time>();
        world.init_resource::<Run>();
        world.init_resource::<Rng>();
        world.init_resource::<PendingHits>();
        world.init_resource::<PlaySeq>();
        world.init_resource::<Messages<Hit>>();
        world.init_resource::<Messages<Loot>>();
        world.init_resource::<Messages<FieldChanged>>();
        world.init_resource::<Messages<CastEvent>>();
        world.init_resource::<Messages<SpellGoTargets>>();
        world.init_resource::<Messages<SwingMessage>>();
        world.init_resource::<Messages<CombatTextSpawn>>();
        let mut targets = vec![];
        for i in 0..4 {
            targets.push(
                world
                    .spawn((
                        Guid(100 + i),
                        Transform::from_xyz(i as f32 * 2.0, 0.0, 0.0),
                        Enemy {
                            def: &data::roster(0).enemies[0],
                            hp: 1000.0,
                            max_hp: 1000.0,
                            speed: 0.0,
                            damage: 0.0,
                            radius: 0.5,
                            xp: 1,
                            boss: false,
                            attack_cd: 0.0,
                            slow: 1.0,
                            slow_t: 0.0,
                            root_t: 0.0,
                            stun_t: 0.0,
                            fear_t: 0.0,
                            weaken: 1.0,
                            weaken_t: 0.0,
                            dots: vec![],
                            dead: false,
                        },
                        ObjectStore(benilla_protocol::messages::ObjectFields::from_pairs(&[])),
                        Marks::debuffs(),
                    ))
                    .id(),
            );
        }
        let mut guest = Run {
            owner: Some(7),
            class: &data::CLASSES[7],
            balance: balance::Config::parse("version=1\n[classes]\n[tags]\n").unwrap(),
            ..default()
        };
        guest.replace_spell_modifiers(
            777,
            [
                Change::FlatSpellDamage(6.0),
                Change::IncreasedDamage(1.0),
                Change::AdditionalProjectiles(1),
                Change::AdditionalChains(1),
                Change::CriticalChance(-1.0),
            ]
            .into_iter()
            .map(|change| Modifier {
                source: 777,
                filter: Filter::requiring(Tags::SPELL),
                change,
            }),
        );
        let mut players = Companions::default();
        players.0.insert(
            7,
            Companion {
                entity: Entity::PLACEHOLDER,
                run: guest,
                physics: Hero { vy: 0.0, air: 0.0 },
                offer: default(),
                offer_id: 0,
            },
        );
        world.insert_resource(players);
        let mut hit = Hit::direct("proc_demon_bolt", targets[0], 10.0, false);
        hit.owner = Some(7);
        hit.meter_source = Some("Demonic Pact");
        world.write_message(hit);
        world.run_system_once(apply_hits).unwrap();
        assert_eq!(world.get::<Enemy>(targets[0]).unwrap().hp, 968.0);
        assert_eq!(world.get::<Enemy>(targets[1]).unwrap().hp, 968.0);
        assert_eq!(world.resource::<PendingHits>().0.len(), 2);
        // The next equipment change affects new casts, never already launched links.
        world
            .resource_mut::<Companions>()
            .0
            .get_mut(&7)
            .unwrap()
            .run
            .replace_spell_modifiers(777, []);
        world.resource_mut::<Messages<Hit>>().clear();
        world.resource_mut::<Run>().elapsed = 0.2;
        world.run_system_once(apply_hits).unwrap();
        for target in &targets[2..] {
            assert!((world.get::<Enemy>(*target).unwrap().hp - 972.8).abs() < 0.001);
        }
        assert!(world.resource::<PendingHits>().0.is_empty());
        assert_eq!(world.resource::<Run>().damage_done, 0.0);
        let players = world.resource::<Companions>();
        let guest = &players.0[&7].run;
        assert!((guest.damage_done - 118.4).abs() < 0.001);
        assert_eq!(guest.meter.rows(0.2)[0].key, "Demonic Pact");
        // Melee target cards also apply to hidden companion attacks.
        let previous: Vec<_> = targets
            .iter()
            .map(|e| world.get::<Enemy>(*e).unwrap().hp)
            .collect();
        for (index, entity) in targets.iter().enumerate() {
            world.get_mut::<Transform>(*entity).unwrap().translation.x = index as f32;
        }
        world
            .resource_mut::<Companions>()
            .0
            .get_mut(&7)
            .unwrap()
            .run
            .replace_spell_modifiers(
                777,
                [
                    Change::AdditionalMeleeTargets(2),
                    Change::CriticalChance(-1.0),
                ]
                .into_iter()
                .map(|change| Modifier {
                    source: 777,
                    filter: Filter::default(),
                    change,
                }),
            );
        let mut hit = Hit::direct("proc_pet_bite", targets[0], 10.0, false);
        hit.owner = Some(7);
        world.write_message(hit);
        world.run_system_once(apply_hits).unwrap();
        for (index, entity) in targets[..3].iter().enumerate() {
            assert!(world.get::<Enemy>(*entity).unwrap().hp < previous[index]);
        }
        assert_eq!(world.get::<Enemy>(targets[3]).unwrap().hp, previous[3]);
        // A physical death explosion uses its modified radius and resolves damage once.
        world.spawn((Hero { vy: 0.0, air: 0.0 }, Transform::default()));
        {
            let mut run = world.resource_mut::<Run>();
            run.balance = balance::Config::parse("version=1\n[classes]\n[tags]\n").unwrap();
            run.talent_build[1][12] = 1;
            super::super::talent_runtime::configure(&mut run);
            run.replace_spell_modifiers(
                778,
                [
                    (Tags::AREA, Change::IncreasedArea(3.0)),
                    (Tags::PHYSICAL, Change::IncreasedDamage(1.0)),
                    (Tags::ARCANE, Change::MoreDamage(99.0)),
                    (Tags::DAMAGE, Change::CriticalChance(-1.0)),
                ]
                .into_iter()
                .map(|(tags, change)| Modifier {
                    source: 778,
                    filter: Filter::requiring(tags),
                    change,
                }),
            );
        }
        world
            .get_mut::<Enemy>(targets[0])
            .unwrap()
            .dots
            .push(enemies::Dot {
                owner: None,
                skill: "proc_bleed",
                meter_source: Some("Deep Wounds"),
                dps: 1.0,
                left: 4.0,
                tick: 1.0,
            });
        world.resource_scope(|world, mut run: Mut<Run>| {
            super::super::talent_runtime::on_death(
                &mut run,
                world.get::<Enemy>(targets[0]).unwrap(),
                Vec3::ZERO,
            );
        });
        world.get_mut::<Enemy>(targets[0]).unwrap().dead = true;
        world
            .get_mut::<Transform>(targets[1])
            .unwrap()
            .translation
            .x = 8.0;
        world
            .get_mut::<Transform>(targets[2])
            .unwrap()
            .translation
            .x = 13.0;
        world
            .get_mut::<Transform>(targets[3])
            .unwrap()
            .translation
            .x = 20.0;
        let inside = world.get::<Enemy>(targets[1]).unwrap().hp;
        let outside = world.get::<Enemy>(targets[2]).unwrap().hp;
        world.resource_mut::<Messages<Hit>>().clear();
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(10));
        world
            .run_system_once(super::super::talent_runtime::tick)
            .unwrap();
        world.run_system_once(apply_hits).unwrap();
        let dealt = inside - world.get::<Enemy>(targets[1]).unwrap().hp;
        assert!(
            dealt > 500.0 && dealt < 600.0,
            "Physical explosion damage was {dealt}"
        );
        assert_eq!(world.get::<Enemy>(targets[2]).unwrap().hp, outside);
    }
    #[test]
    fn deep_wounds_keeps_ticking_during_rapid_attacks() {
        let mut world = World::new();
        world.init_resource::<Time>();
        world.init_resource::<Run>();
        world.init_resource::<Rng>();
        world.init_resource::<PendingHits>();
        world.init_resource::<PlaySeq>();
        world.init_resource::<Messages<Hit>>();
        world.init_resource::<Messages<Loot>>();
        world.init_resource::<Messages<FieldChanged>>();
        world.init_resource::<Messages<CastEvent>>();
        world.init_resource::<Messages<SpellGoTargets>>();
        world.init_resource::<Messages<SwingMessage>>();
        world.init_resource::<Messages<CombatTextSpawn>>();
        let mut account = super::super::talents::Account::fresh();
        account.xp = super::super::talents::threshold(60);
        for node in [0, 1] {
            for _ in 0..5 {
                assert!(account.buy(0, 1, node));
            }
        }
        assert!(account.buy(0, 1, 8));
        account.apply(0, &mut world.resource_mut::<Run>());
        let target = world
            .spawn((
                Guid(99),
                Transform::default(),
                Enemy {
                    def: &data::roster(0).enemies[0],
                    hp: 10000.0,
                    max_hp: 10000.0,
                    speed: 0.0,
                    damage: 0.0,
                    radius: 1.0,
                    xp: 1,
                    boss: false,
                    attack_cd: 0.0,
                    slow: 1.0,
                    slow_t: 0.0,
                    root_t: 0.0,
                    stun_t: 0.0,
                    fear_t: 0.0,
                    weaken: 1.0,
                    weaken_t: 0.0,
                    dots: vec![],
                    dead: false,
                },
                ObjectStore(benilla_protocol::messages::ObjectFields::from_pairs(&[])),
                Marks::debuffs(),
            ))
            .id();
        for step in 0..30 {
            world
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_millis(100));
            world.resource_mut::<Run>().elapsed += 0.1;
            world.resource_mut::<Messages<Hit>>().clear();
            let skill = ["cleave", "heroic_strike", "whirlwind"][step % 3];
            world.write_message(Hit::direct(skill, target, 10.0, false));
            world.run_system_once(apply_hits).unwrap();
        }
        let run = world.resource::<Run>();
        let rows = run.meter.rows(run.elapsed);
        assert!(
            rows.iter()
                .any(|row| row.key == "Deep Wounds" && row.total > 0.0),
            "Rapid attacks must not evict all bleeds before their first tick"
        );
        assert!(rows
            .iter()
            .any(|row| row.key == "cleave" && row.total > 0.0));
    }
    #[test]
    fn meter_attributes_hits_and_periodic_damage_without_counting_overkill() {
        let mut world = World::new();
        world.init_resource::<Time>();
        world.init_resource::<Run>();
        world.resource_mut::<Run>().balance =
            super::super::balance::Config::parse("version=1\n[classes]\n[tags]\n").unwrap();
        world.init_resource::<Rng>();
        world.init_resource::<PendingHits>();
        world.init_resource::<PlaySeq>();
        world.init_resource::<Messages<Hit>>();
        world.init_resource::<Messages<Loot>>();
        world.init_resource::<Messages<FieldChanged>>();
        world.init_resource::<Messages<CastEvent>>();
        world.init_resource::<Messages<SpellGoTargets>>();
        world.init_resource::<Messages<SwingMessage>>();
        world.init_resource::<Messages<CombatTextSpawn>>();
        let enemy = |hp, dots| Enemy {
            def: &data::roster(0).enemies[0],
            hp,
            max_hp: hp,
            speed: 0.0,
            damage: 0.0,
            radius: 0.0,
            xp: 1,
            boss: false,
            attack_cd: 0.0,
            slow: 1.0,
            slow_t: 0.0,
            root_t: 0.0,
            stun_t: 0.0,
            fear_t: 0.0,
            weaken: 1.0,
            weaken_t: 0.0,
            dots,
            dead: false,
        };
        let store = || ObjectStore(benilla_protocol::messages::ObjectFields::from_pairs(&[]));
        let target = world
            .spawn((
                Guid(1),
                Transform::default(),
                enemy(50.0, vec![]),
                store(),
                Marks::debuffs(),
            ))
            .id();
        world.spawn((
            Guid(2),
            Transform::default(),
            enemy(
                40.0,
                vec![
                    enemies::Dot {
                        meter_source: None,
                        owner: None,
                        skill: "corruption",
                        dps: 30.0,
                        left: 5.0,
                        tick: 0.0,
                    },
                    enemies::Dot {
                        meter_source: None,
                        owner: None,
                        skill: "serpent_sting",
                        dps: 30.0,
                        left: 5.0,
                        tick: 0.0,
                    },
                ],
            ),
            store(),
            Marks::debuffs(),
        ));
        world
            .resource_mut::<Messages<Hit>>()
            .write(Hit::direct("fireball", target, 100.0, true));
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(100));
        world.run_system_once(apply_hits).unwrap();
        let run = world.resource::<Run>();
        let rows = run.meter.rows(run.elapsed);
        assert_eq!(
            rows.iter().find(|r| r.key == "fireball").unwrap().total,
            50.0
        );
        assert_eq!(
            rows.iter().find(|r| r.key == "corruption").unwrap().total,
            30.0
        );
        assert_eq!(
            rows.iter()
                .find(|r| r.key == "serpent_sting")
                .unwrap()
                .total,
            10.0
        );
        assert_eq!(run.damage_done, 90.0);
        assert_eq!(run.kills, 2);
        // Gear uses the same real hit pipeline: tag gating, ailments, pushback,
        // effective-damage leech and exactly one life-on-kill recovery.
        world.spawn((Hero { vy: 0.0, air: 0.0 }, Transform::default()));
        let geared_target = world
            .spawn((
                Guid(3),
                Transform::from_translation(Vec3::X * 3.0),
                enemy(100.0, vec![]),
                store(),
                Marks::debuffs(),
            ))
            .id();
        {
            let mut run = world.resource_mut::<Run>();
            run.hp = 20.0;
            run.equipment.healing = 0.5;
            run.equipment.life_on_kill = 5.0;
            run.replace_spell_modifiers(
                43,
                [
                    engine::Change::LifeLeech(1.0),
                    engine::Change::BleedChance(1.0),
                    engine::Change::IgniteChance(1.0),
                    engine::Change::ChillChance(1.0),
                    engine::Change::KnockbackChance(1.0),
                ]
                .into_iter()
                .map(|change| engine::Modifier {
                    source: 43,
                    filter: engine::Filter::default(),
                    change,
                }),
            );
        }
        world.resource_mut::<Messages<Hit>>().clear();
        world.resource_mut::<Messages<Hit>>().write(Hit::direct(
            "multi_shot",
            geared_target,
            10.0,
            false,
        ));
        world.run_system_once(apply_hits).unwrap();
        let dealt = 100.0 - world.get::<Enemy>(geared_target).unwrap().hp;
        assert!((world.resource::<Run>().hp - (20.0 + dealt * 1.5)).abs() < 0.001);
        assert_eq!(world.get::<Enemy>(geared_target).unwrap().dots.len(), 1);
        assert_eq!(world.get::<Enemy>(geared_target).unwrap().dots[0].dps, 2.0);
        assert_eq!(
            world.get::<Transform>(geared_target).unwrap().translation.x,
            4.5
        );
        world.resource_mut::<Messages<Hit>>().clear();
        world.resource_mut::<Messages<Hit>>().write(Hit::direct(
            "fireball",
            geared_target,
            1.0,
            false,
        ));
        world.run_system_once(apply_hits).unwrap();
        assert_eq!(world.get::<Enemy>(geared_target).unwrap().dots.len(), 2);
        world.resource_mut::<Messages<Hit>>().clear();
        world.resource_mut::<Messages<Hit>>().write(Hit::direct(
            "frostbolt",
            geared_target,
            1.0,
            false,
        ));
        world.run_system_once(apply_hits).unwrap();
        assert_eq!(world.get::<Enemy>(geared_target).unwrap().slow, 0.75);
        world.resource_mut::<Messages<Hit>>().clear();
        world.resource_mut::<Messages<Hit>>().write(Hit::direct(
            "multi_shot",
            geared_target,
            1000.0,
            false,
        ));
        world.run_system_once(apply_hits).unwrap();
        let run = world.resource::<Run>();
        assert_eq!(run.kills, 3);
        assert!((run.hp - (20.0 + run.stats().max_hp * 0.20 + 7.5)).abs() < 0.001);

        // Attribution must separate milestone damage from the spell used by its visual,
        // including periodic ticks, while still excluding overkill.
        world.insert_resource(Run::default());
        world.resource_mut::<Messages<Hit>>().clear();
        let mut targets = Vec::new();
        for id in 4..=6 {
            targets.push(
                world
                    .spawn((
                        Guid(id),
                        Transform::default(),
                        enemy(100.0, vec![]),
                        store(),
                        Marks::debuffs(),
                    ))
                    .id(),
            );
        }
        let mut burst = Hit::direct("arcane_explosion", targets[0], 500.0, false);
        burst.triggered = true;
        burst.meter_source = Some("Bloodbath");
        world.write_message(burst);
        world.write_message(Hit::direct("arcane_explosion", targets[1], 10.0, false));
        world
            .get_mut::<Enemy>(targets[2])
            .unwrap()
            .dots
            .push(enemies::Dot {
                owner: None,
                skill: "deadly_poison",
                meter_source: Some("Deadly Poison"),
                dps: 5.0,
                left: 5.0,
                tick: 0.0,
            });
        world.run_system_once(apply_hits).unwrap();
        let run = world.resource::<Run>();
        let rows = run.meter.rows(run.elapsed);
        assert_eq!(
            rows.iter().find(|r| r.key == "Bloodbath").unwrap().total,
            100.0
        );
        assert_eq!(
            rows.iter()
                .find(|r| r.key == "arcane_explosion")
                .unwrap()
                .total,
            10.0
        );
        assert_eq!(
            rows.iter()
                .find(|r| r.key == "Deadly Poison")
                .unwrap()
                .total,
            5.0
        );
        assert_eq!(run.damage_done, 115.0);
        assert_eq!(rows.iter().map(|r| r.total).sum::<f64>(), 115.0);
        // Ignite accepts a Fire spell, Fire attack, and a hidden Fire proc alike.
        world.resource_mut::<Messages<Hit>>().clear();
        let mut account = super::super::talents::Account::fresh();
        account.xp = super::super::talents::threshold(60);
        for node in [0, 1] {
            for _ in 0..5 {
                assert!(account.buy(6, 1, node));
            }
        }
        assert!(account.buy(6, 1, 8));
        {
            let mut run = world.resource_mut::<Run>();
            run.class = &data::CLASSES[6];
            run.balance =
                super::super::balance::Config::parse("version=1\n[classes]\n[tags]\n").unwrap();
            account.apply(6, &mut run);
        }
        let mut fire_targets = vec![];
        for (index, skill) in [
            "fireball",
            "explosive_trap",
            "flame_shock",
            "proc_demon_bolt",
            "frostbolt",
        ]
        .into_iter()
        .enumerate()
        {
            let target = world
                .spawn((
                    Guid(100 + index as u64),
                    Transform::from_xyz(index as f32 * 40.0, 0.0, 0.0),
                    enemy(1000.0, vec![]),
                    store(),
                    Marks::debuffs(),
                ))
                .id();
            let mut hit = Hit::direct(skill, target, 10.0, false);
            if skill == "proc_demon_bolt" {
                hit.triggered = true;
                hit.meter_source = Some("Demonic Pact");
            }
            world.write_message(hit);
            fire_targets.push(target);
        }
        world.run_system_once(apply_hits).unwrap();
        for target in &fire_targets[..4] {
            let dots = &world.get::<Enemy>(*target).unwrap().dots;
            assert_eq!(dots.len(), 1);
            assert_eq!(dots[0].skill, "proc_ignite");
            assert_eq!(dots[0].meter_source, Some("Ignite"));
            assert!(dots[0].dps > 0.0);
        }
        assert!(world.get::<Enemy>(fire_targets[4]).unwrap().dots.is_empty());
        // Immortality is not a damage cap: direct hits and DoTs still count in full.
        world.resource_mut::<Messages<Hit>>().clear();
        let mut dummy = enemy(
            100.0,
            vec![enemies::Dot {
                owner: None,
                skill: "proc_bleed",
                meter_source: Some("Deep Wounds"),
                dps: 1_000_000.0,
                left: 4.0,
                tick: 0.0,
            }],
        );
        dummy.def = &super::super::training::DUMMY;
        let target = world
            .spawn((
                Guid(999),
                Transform::default(),
                dummy,
                store(),
                Marks::debuffs(),
            ))
            .id();
        let before = world.resource::<Run>().damage_done;
        let kills = world.resource::<Run>().kills;
        world.write_message(Hit::direct("arcane_missiles", target, 1_000_000.0, false));
        world.run_system_once(apply_hits).unwrap();
        let dummy = world.get::<Enemy>(target).unwrap();
        assert!(!dummy.dead);
        assert_eq!(dummy.hp, dummy.max_hp);
        assert_eq!(world.resource::<Run>().kills, kills);
        assert_eq!(world.resource::<Run>().damage_done - before, 2_000_000.0);
    }

    #[test]
    fn projectile_modifiers_schedule_real_hits_and_multiple_nonrecursive_repeats() {
        for repeats in [1, 2] {
            let mut world = World::new();
            world.init_resource::<Time>();
            world.init_resource::<Rng>();
            world.init_resource::<PendingHits>();
            world.init_resource::<PlaySeq>();
            world.init_resource::<Messages<Hit>>();
            world.init_resource::<Messages<CastEvent>>();
            world.init_resource::<Messages<SpellGoTargets>>();
            world.init_resource::<Messages<SwingMessage>>();
            world.init_resource::<Messages<CombatTextSpawn>>();
            let mut run = Run {
                class: &data::CLASSES[6],
                abilities: vec![Owned {
                    def: data::ability("fireball"),
                    level: 1,
                    cd: 0.0,
                }],
                ..Run::default()
            };
            run.replace_spell_modifiers(
                42,
                [
                    engine::Change::AdditionalProjectiles(2),
                    engine::Change::Fork(1),
                    engine::Change::AdditionalChains(1),
                    engine::Change::RecastChance(repeats as f32),
                ]
                .map(|change| engine::Modifier {
                    source: 42,
                    filter: engine::Filter::default(),
                    change,
                }),
            );
            world.insert_resource(run);
            world.spawn((Transform::default(), Hero { vy: 0.0, air: 0.0 }));
            for i in 1..=8 {
                world.spawn((
                    Transform::from_xyz(i as f32, 0.0, 0.0),
                    Enemy {
                        def: &data::roster(0).enemies[0],
                        hp: 1000.0,
                        max_hp: 1000.0,
                        speed: 0.0,
                        damage: 0.0,
                        radius: 0.0,
                        xp: 1,
                        boss: false,
                        attack_cd: 0.0,
                        slow: 1.0,
                        slow_t: 0.0,
                        root_t: 0.0,
                        stun_t: 0.0,
                        fear_t: 0.0,
                        weaken: 1.0,
                        weaken_t: 0.0,
                        dots: vec![],
                        dead: false,
                    },
                ));
            }
            world
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_millis(100));
            world.run_system_once(cast_abilities).unwrap();
            assert_eq!(world.resource::<Messages<Hit>>().len(), 3);
            assert_eq!(world.resource::<PendingHits>().0.len(), 5);
            assert_eq!(world.resource::<PendingHits>().1.len(), 5);
            assert_eq!(world.resource::<Run>().recasts.len(), 1);
            let cooldown = world.resource::<Run>().abilities[0].cd;
            for repeat in 1..=repeats {
                world.resource_mut::<Run>().elapsed = repeat as f32 * 0.2;
                world
                    .resource_mut::<Time>()
                    .advance_by(std::time::Duration::from_millis(200));
                world.run_system_once(cast_abilities).unwrap();
                assert_eq!(world.resource::<Messages<Hit>>().len(), 3 * (repeat + 1));
                assert!(
                    (world.resource::<Run>().abilities[0].cd - (cooldown - repeat as f32 * 0.2))
                        .abs()
                        < 0.001
                );
            }
            assert!(world.resource::<Run>().recasts.is_empty());
            world.resource_mut::<Run>().elapsed = (repeats + 1) as f32 * 0.2;
            world
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_millis(200));
            world.run_system_once(cast_abilities).unwrap();
            assert_eq!(world.resource::<Messages<Hit>>().len(), 3 * (repeats + 1));
        }
    }
}

impl Hit {
    pub fn direct(skill: &'static str, target: Entity, damage: f32, crit: bool) -> Self {
        Self {
            owner: None,
            skill,
            meter_source: None,
            triggered: false,
            proc_resolved: false,
            proc_chain: 0,
            target,
            damage,
            dot: 0.0,
            dot_dur: 0.0,
            effect: Effect::None,
            mark: 0,
            mark_dur: 0.0,
            splash: 0.0,
            crit,
            color: COLOR_MELEE,
        }
    }
}

/// Hits in flight, by the run time they land.
#[derive(Resource, Default)]
pub(super) struct PendingHits(pub Vec<(f32, Hit)>, pub Vec<(f32, Entity, Entity, u32)>);

/// A ground spell's area, ticking on whatever stands in it.
#[derive(Component)]
pub(super) struct GroundArea {
    owner: Option<u64>,
    skill: &'static str,
    crit: f32,
    crit_multiplier: f32,
    damage: f32,
    dot: f32,
    dot_dur: f32,
    radius: f32,
    left: f32,
    every: f32,
    next_tick: f32,
    effect: Effect,
    mark: u32,
    /// The spell whose impact plays on each enemy a tick hits, 0 for none.
    impact: u32,
}

#[derive(Component)]
pub(super) struct Totem {
    owner: Option<u64>,
    skill: &'static str,
    crit: f32,
    crit_multiplier: f32,
    cast: u32,
    damage: f32,
    range: f32,
    radius: f32,
    left: f32,
    tick: f32,
    pulse: bool,
}

pub(super) fn hit_for(
    def: &AbilityDef,
    p: &Params,
    target: Entity,
    damage: f32,
    crit: bool,
) -> Hit {
    Hit {
        owner: None,
        skill: def.key,
        meter_source: None,
        triggered: false,
        proc_resolved: false,
        proc_chain: 0,
        target,
        damage,
        dot: p.dot,
        dot_dur: p.duration,
        effect: def.effect,
        mark: def.mark,
        mark_dur: engine::mark_duration(def.effect, p.duration),
        splash: if matches!(def.kind, Kind::Bolt | Kind::Strike) {
            p.radius
        } else {
            0.0
        },
        crit,
        color: if def.kind == Kind::Melee {
            COLOR_MELEE
        } else {
            COLOR_SPELL
        },
    }
}

fn schedule(
    pending: &mut PendingHits,
    hits: &mut MessageWriter<Hit>,
    now: f32,
    delay: f32,
    hit: Hit,
) {
    if delay <= 0.0 {
        hits.write(hit);
    } else {
        pending.0.push((now + delay, hit));
    }
}

fn roll(rng: &mut Rng, crit: f32, multiplier: f32, dmg: f32) -> (f32, bool) {
    engine::critical_with_multiplier(dmg, crit, multiplier, rng.f32())
}

#[allow(clippy::too_many_arguments)]
fn emit_links(
    roots: &[Foe],
    foes: &[Foe],
    p: &Params,
    def: &AbilityDef,
    native_chain: bool,
    speed: f32,
    rng: &mut Rng,
    pending: &mut PendingHits,
    hits: &mut MessageWriter<Hit>,
    now: f32,
) {
    for link in links(roots, foes, p, speed, native_chain) {
        let (damage, crit) = roll(rng, p.crit, p.crit_multiplier, p.dmg * link.scale);
        let mut hit = hit_for(def, p, link.target.e, damage, crit);
        hit.dot *= link.scale;
        if let Some(from) = link.from {
            pending
                .1
                .push((now + link.launch, from, link.target.e, def.spell));
        }
        schedule(pending, hits, now, link.arrival, hit);
    }
}

fn entities(foes: &[Foe]) -> Vec<Entity> {
    foes.iter().map(|f| f.e).collect()
}

pub(super) fn cast_abilities(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut rng: ResMut<Rng>,
    spells: Option<Res<crate::ui_action::Spells>>,
    hero: Query<(Entity, &Transform), With<Hero>>,
    enemies: Query<(Entity, &Transform, &Enemy)>,
    mut fx: Fx,
    mut pending: ResMut<PendingHits>,
    mut hits: MessageWriter<Hit>,
) {
    let dt = time.delta_secs();
    if run.dead_t.is_some() || dt <= 0.0 {
        return;
    }
    let Ok((me, htf)) = hero.single() else { return };
    let hpos = htf.translation;
    let now = run.elapsed;
    let mut foes: Vec<Foe> = enemies
        .iter()
        .filter(|(_, _, e)| !e.dead)
        .map(|(e, tf, en)| Foe {
            e,
            pos: tf.translation,
            hp: en.hp,
            dist: hero::reach(hpos, tf.translation),
            radius: en.radius,
        })
        .collect();
    foes.sort_by(|a, b| a.dist.total_cmp(&b.dist));
    let speed_of = |spell: u32| {
        spells
            .as_deref()
            .and_then(|s| s.catalog.get(spell))
            .map_or(0.0, |d| d.speed)
    };

    for i in 0..run.abilities.len() {
        run.abilities[i].cd -= dt;
        let repeated = run.recasts.take_due(i, now);
        if run.abilities[i].cd > 0.0 && !repeated {
            continue;
        }
        let o = &run.abilities[i];
        let def = o.def;
        let stats = run.stats();
        let p = run.resolve_spell_for_cast(def, o.level, &mut rng);
        let speed = speed_of(def.spell);
        // The client's models are authored for the base radius: grow them with the area.
        let area_scale = if def.radius > 0.0 {
            (p.radius / def.radius).clamp(0.25, 8.0)
        } else {
            1.0
        };
        fx.area_scale(me, def.spell, area_scale);
        let fired = match def.kind {
            Kind::Bolt | Kind::Strike | Kind::Melee => {
                let reach = if def.kind == Kind::Melee {
                    def.range + 0.5
                } else {
                    def.range
                };
                let targets = if matches!(
                    def.key,
                    "pyroblast" | "aimed_shot" | "exorcism" | "starfire"
                ) {
                    toughest(&foes, reach, p.count)
                } else {
                    nearest(&foes, reach, p.count)
                };
                if targets.is_empty() {
                    false
                } else {
                    if def.kind == Kind::Melee {
                        fx.swing(me, targets[0].e, p.dmg);
                        for f in &targets {
                            fx.impact(f.e, def.spell);
                        }
                    } else {
                        fx.cast(me, def.spell, &entities(&targets), None);
                    }
                    emit_links(
                        &targets,
                        &foes,
                        &p,
                        def,
                        false,
                        speed,
                        &mut rng,
                        &mut pending,
                        &mut hits,
                        now,
                    );
                    true
                }
            }
            Kind::Nova | Kind::Cone => {
                let mut targets = around(&foes, hpos, p.radius);
                if def.kind == Kind::Cone {
                    if let Some(first) = targets.first().copied() {
                        let aim = flat(first.pos - hpos).normalize_or_zero();
                        targets.retain(|f| {
                            let d = flat(f.pos - hpos);
                            d.length() < 2.0 || d.normalize_or_zero().dot(aim) >= 0.8
                        });
                    }
                }
                if targets.is_empty() {
                    false
                } else {
                    fx.cast(me, def.spell, &entities(&targets), None);
                    for f in &targets {
                        let (dmg, crit) = roll(&mut rng, p.crit, p.crit_multiplier, p.dmg);
                        let delay = if speed > 0.0 { f.dist / speed } else { 0.0 };
                        schedule(
                            &mut pending,
                            &mut hits,
                            now,
                            delay,
                            hit_for(def, &p, f.e, dmg, crit),
                        );
                    }
                    true
                }
            }
            Kind::Ground | Kind::GroundSelf => {
                let center = if def.kind == Kind::GroundSelf {
                    foes.first()
                        .filter(|f| f.dist <= p.radius + 4.0)
                        .map(|_| hpos)
                } else {
                    densest(&foes, def.range, p.radius)
                };
                match center {
                    None => false,
                    Some(center) => {
                        let trap = def.key == "explosive_trap";
                        fx.cast(me, if trap { 13813 } else { def.spell }, &[], Some(center));
                        let area = if trap {
                            units::spawn_explosive_trap(&mut commands, run.guid(), center)
                        } else if def.overhead {
                            commands
                                .spawn((Transform::from_translation(center), RunEntity))
                                .id()
                        } else {
                            let serial = run.guid();
                            units::spawn_area_object(
                                &mut commands,
                                serial,
                                def.spell,
                                p.radius,
                                area_scale,
                                center,
                            )
                        };
                        if trap {
                            // The trap itself casts the explosion.
                            fx.area_scale(area, def.spell, area_scale);
                        }
                        let burst = def.duration <= 0.0;
                        commands.entity(area).insert(GroundArea {
                            owner: run.owner,
                            skill: def.key,
                            crit: p.crit,
                            crit_multiplier: p.crit_multiplier,
                            damage: p.dmg,
                            dot: p.dot,
                            dot_dur: if burst { 5.0 } else { 0.0 },
                            radius: p.radius,
                            left: if trap {
                                1.9
                            } else if burst {
                                1.5
                            } else {
                                p.duration
                            },
                            every: if burst { f32::INFINITY } else { 1.0 },
                            next_tick: if burst && !trap { 0.0 } else { 0.4 },
                            effect: def.effect,
                            mark: def.mark,
                            impact: if def.overhead || trap { def.spell } else { 0 },
                        });
                        if let Some(owner) = run.owner {
                            commands
                                .entity(area)
                                .insert(super::multiplayer::OwnedEffect(owner));
                        }
                        true
                    }
                }
            }
            Kind::Chain => {
                let targets = nearest(&foes, p.range, 1);
                if targets.is_empty() {
                    false
                } else {
                    fx.cast(me, def.spell, &entities(&targets), None);
                    emit_links(
                        &targets,
                        &foes,
                        &p,
                        def,
                        true,
                        speed,
                        &mut rng,
                        &mut pending,
                        &mut hits,
                        now,
                    );
                    true
                }
            }
            Kind::Dot => {
                let mut pool: Vec<Foe> = foes
                    .iter()
                    .filter(|f| f.dist <= def.range)
                    .take(p.count * 3)
                    .copied()
                    .collect();
                if pool.is_empty() {
                    false
                } else {
                    let mut targets = Vec::with_capacity(p.count);
                    while targets.len() < p.count && !pool.is_empty() {
                        targets.push(pool.swap_remove(rng.index(pool.len())));
                    }
                    fx.cast(me, def.spell, &entities(&targets), None);
                    for f in &targets {
                        let delay = if speed > 0.0 { f.dist / speed } else { 0.0 };
                        schedule(
                            &mut pending,
                            &mut hits,
                            now,
                            delay,
                            hit_for(def, &p, f.e, 0.0, false),
                        );
                    }
                    true
                }
            }
            Kind::Aura => {
                let targets = around(&foes, hpos, p.radius);
                if targets.is_empty() {
                    false
                } else {
                    if def.key == "seal_of_righteousness" {
                        fx.cast(me, def.spell, &[me], None);
                    }
                    for f in targets.iter().take(6) {
                        fx.impact(f.e, def.spell);
                    }
                    for f in &targets {
                        let (dmg, crit) = roll(&mut rng, p.crit, p.crit_multiplier, p.dmg);
                        hits.write(hit_for(def, &p, f.e, dmg, crit));
                    }
                    true
                }
            }
            Kind::Orbit => {
                let targets = nearest(&foes, p.radius, p.count);
                if targets.is_empty() {
                    false
                } else {
                    for f in &targets {
                        fx.impact(f.e, 403);
                        let (dmg, crit) = roll(&mut rng, p.crit, p.crit_multiplier, p.dmg);
                        hits.write(hit_for(def, &p, f.e, dmg, crit));
                    }
                    true
                }
            }
            Kind::Shield => {
                super::talent_runtime::refreshed_shield(&mut run, p.dmg);
                run.shield = p.dmg;
                run.shield_t = p.duration;
                fx.cast(me, def.spell, &[me], None);
                true
            }
            Kind::Immune => {
                if foes.first().is_some_and(|f| f.dist <= 5.0) {
                    run.immune_t = p.duration;
                    fx.cast(me, def.spell, &[me], None);
                    true
                } else {
                    false
                }
            }
            Kind::Thorns => {
                if foes.first().is_some_and(|f| f.dist <= 8.0) {
                    run.thorns = p.dmg;
                    run.thorns_t = p.duration;
                    run.thorns_spell = def.spell;
                    fx.cast(me, def.spell, &[me], None);
                    true
                } else {
                    false
                }
            }
            Kind::Heal => {
                if run.hp < stats.max_hp * 0.85 {
                    if p.duration <= 0.0 {
                        run.heal(p.dmg);
                    } else {
                        run.heal_rate = p.dmg / p.duration.max(1.0);
                        run.heal_t = p.duration;
                    }
                    fx.cast(me, def.spell, &[me], None);
                    fx.number(me, p.dmg, false, COLOR_HEAL);
                    true
                } else {
                    false
                }
            }
            Kind::Totem => {
                if foes
                    .first()
                    .is_some_and(|f| f.dist <= def.range.max(p.radius) + 6.0)
                {
                    fx.cast(me, def.spell, &[], None);
                    let a = rng.range(0.0, std::f32::consts::TAU);
                    let pos = hpos + Vec3::new(a.cos(), 0.0, a.sin()) * 2.0;
                    let serial = run.guid();
                    let totem = units::spawn_unit(
                        &mut commands,
                        &UnitSpec {
                            serial,
                            display: def.totem.0,
                            scale: 1.0,
                            level: 60,
                            faction: units::FACTION_FRIENDLY,
                            bytes0: 0,
                            weapons: &[],
                            sheath: 0,
                        },
                        pos,
                        0.0,
                    );
                    commands.entity(totem).insert(Totem {
                        owner: run.owner,
                        skill: def.key,
                        crit: p.crit,
                        crit_multiplier: p.crit_multiplier,
                        cast: def.totem.1,
                        damage: p.dmg,
                        range: def.range,
                        radius: p.radius,
                        left: p.duration,
                        tick: 0.3,
                        pulse: def.key == "magma_totem",
                    });
                    if let Some(owner) = run.owner {
                        commands
                            .entity(totem)
                            .insert(super::multiplayer::OwnedEffect(owner));
                    }
                    true
                } else {
                    false
                }
            }
        };
        if !repeated {
            run.abilities[i].cd = if fired { p.cooldown } else { 0.2 };
            if fired && p.recast > 0.0 {
                let count = engine::recast_count(p.recast, rng.f32(), false);
                run.recasts.schedule(i, now, count);
            }
        }
    }
}

/// Ground areas tick, and totems fight, until they run out.
#[allow(clippy::type_complexity)]
pub(super) fn tick_areas(
    mut commands: Commands,
    time: Res<Time>,
    run: Res<Run>,
    mut rng: ResMut<Rng>,
    spells: Option<Res<crate::ui_action::Spells>>,
    mut areas: Query<(
        Entity,
        &Transform,
        &mut GroundArea,
        Option<&crate::net::Guid>,
        Option<&mut crate::net::ObjectStore>,
    )>,
    mut totems: Query<(Entity, &Transform, &mut Totem), Without<GroundArea>>,
    enemies: Query<(Entity, &Transform, &Enemy), (Without<GroundArea>, Without<Totem>)>,
    mut fx: Fx,
    mut pending: ResMut<PendingHits>,
    mut hits: MessageWriter<Hit>,
    mut fields: MessageWriter<crate::net::FieldChanged>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let now = run.elapsed;
    let live: Vec<Foe> = enemies
        .iter()
        .filter(|(_, _, e)| !e.dead)
        .map(|(e, tf, en)| Foe {
            e,
            pos: tf.translation,
            hp: en.hp,
            dist: 0.0,
            radius: en.radius,
        })
        .collect();

    for (e, tf, mut area, guid, store) in &mut areas {
        area.left -= dt;
        area.next_tick -= dt;
        if area.next_tick <= 0.0 {
            area.next_tick += area.every;
            if area.skill == "explosive_trap" {
                if let (Some(guid), Some(mut store)) = (guid, store) {
                    fx.trigger_trap(e, guid.0, &mut store, tf.translation, &mut fields);
                }
            }
            for (i, f) in around(&live, tf.translation, area.radius)
                .into_iter()
                .enumerate()
            {
                if area.impact != 0 && i < 12 {
                    fx.impact(f.e, area.impact);
                }
                let (dmg, crit) = roll(&mut rng, area.crit, area.crit_multiplier, area.damage);
                hits.write(Hit {
                    owner: area.owner,
                    skill: area.skill,
                    meter_source: None,
                    triggered: false,
                    proc_resolved: false,
                    proc_chain: 0,
                    target: f.e,
                    damage: dmg,
                    dot: area.dot,
                    dot_dur: area.dot_dur,
                    effect: area.effect,
                    mark: area.mark,
                    mark_dur: 1.5,
                    splash: 0.0,
                    crit,
                    color: COLOR_SPELL,
                });
            }
        }
        if area.left <= 0.0 {
            commands.entity(e).despawn();
        }
    }

    for (e, tf, mut totem) in &mut totems {
        totem.left -= dt;
        if totem.left <= 0.0 {
            commands
                .entity(e)
                .remove::<Totem>()
                .insert(benilla_world::model_fade::DespawnFade::default());
            continue;
        }
        totem.tick -= dt;
        if totem.tick > 0.0 {
            continue;
        }
        let pos = tf.translation;
        if totem.pulse {
            let targets = around(&live, pos, totem.radius);
            totem.tick = 2.0;
            if targets.is_empty() {
                continue;
            }
            fx.cast(e, totem.cast, &[], None);
            for f in &targets {
                let (dmg, crit) = roll(&mut rng, totem.crit, totem.crit_multiplier, totem.damage);
                hits.write(Hit {
                    owner: totem.owner,
                    skill: totem.skill,
                    triggered: false,
                    proc_resolved: false,
                    proc_chain: 0,
                    color: COLOR_SPELL,
                    ..Hit::direct(totem.skill, f.e, dmg, crit)
                });
            }
        } else {
            totem.tick = 1.0;
            let target = live
                .iter()
                .map(|f| (flat(f.pos - pos).length(), f))
                .filter(|(d, _)| *d <= totem.range)
                .min_by(|a, b| a.0.total_cmp(&b.0));
            let Some((dist, f)) = target else { continue };
            fx.cast(e, totem.cast, &[f.e], None);
            let speed = spells
                .as_deref()
                .and_then(|s| s.catalog.get(totem.cast))
                .map_or(0.0, |d| d.speed);
            let delay = if speed > 0.0 { dist / speed } else { 0.0 };
            let (dmg, crit) = roll(&mut rng, totem.crit, totem.crit_multiplier, totem.damage);
            schedule(
                &mut pending,
                &mut hits,
                now,
                delay,
                Hit {
                    owner: totem.owner,
                    skill: totem.skill,
                    color: COLOR_SPELL,
                    ..Hit::direct(totem.skill, f.e, dmg, crit)
                },
            );
        }
    }
}

/// Land every hit due: damage and its number, the over-time part, crowd control, the marks, and
/// death with its loot.
pub(super) fn apply_hits(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut rng: ResMut<Rng>,
    mut pending: ResMut<PendingHits>,
    mut incoming: MessageReader<Hit>,
    mut enemies: Query<(
        Entity,
        &Guid,
        &mut Transform,
        &mut Enemy,
        &mut ObjectStore,
        &mut Marks,
    )>,
    mut fx: Fx,
    mut loot: MessageWriter<Loot>,
    mut fields: MessageWriter<FieldChanged>,
    hero: Query<
        (
            &Transform,
            Option<&super::multiplayer::RemoteOwner>,
            Has<Hero>,
        ),
        (
            Without<Enemy>,
            Or<(With<Hero>, With<super::multiplayer::RemoteOwner>)>,
        ),
    >,
    mut players: Option<ResMut<super::multiplayer::Companions>>,
) {
    let dt = time.delta_secs();
    let now = run.elapsed;
    let mut due: Vec<Hit> = incoming.read().cloned().collect();
    // Equipment/buff sources stay fixed during this system, even across thousands of targets.
    let mut item_mods = std::collections::HashMap::new();
    pending.1.retain(|&(at, from, to, spell)| {
        if at <= now {
            fx.cast(from, spell, &[to], None);
            false
        } else {
            true
        }
    });
    pending.0.retain(|(t, h)| {
        if *t <= now {
            due.push(h.clone());
            false
        } else {
            true
        }
    });

    // Hidden spells use the same stat/modifier resolver as visible spells. Target copies and
    // repeats keep this snapshot, so they cannot apply bonuses or schedule repeats twice.
    let mut prepared = Vec::with_capacity(due.len());
    for mut h in due {
        if h.triggered && super::procs::find(h.skill).is_none() {
            if let Some(player) =
                super::multiplayer::player_run(&mut run, players.as_deref_mut(), h.owner)
            {
                if let Some(key) = super::procs::source_key(player, h.meter_source) {
                    h.skill = key;
                }
            }
        }
        if super::procs::find(h.skill).is_some() && !h.proc_resolved {
            let Some(player) =
                super::multiplayer::player_run(&mut run, players.as_deref_mut(), h.owner)
            else {
                continue;
            };
            let p = super::procs::resolve(
                player,
                h.skill,
                h.damage,
                h.dot,
                h.dot_dur,
                h.splash,
                if h.skill == "proc_spirit_bolt" { 3 } else { 1 },
            );
            (h.damage, h.crit) =
                engine::critical_with_multiplier(p.dmg, p.crit, p.crit_multiplier, rng.f32());
            h.dot = p.dot;
            h.dot_dur = p.duration;
            h.splash = p.radius;
            h.triggered = true;
            h.proc_resolved = true;
            let repeats = engine::recast_count(p.recast, rng.f32(), false).min(64);
            if p.tags
                .intersects(engine::Tags::PROJECTILE | engine::Tags::CHAIN | engine::Tags::MELEE)
            {
                if let Ok((_, _, tf, ..)) = enemies.get(h.target) {
                    let center = tf.translation;
                    let mut foes: Vec<_> = enemies
                        .iter()
                        .filter(|(_, _, _, en, ..)| !en.dead)
                        .map(|(e, _, tf, en, ..)| Foe {
                            e,
                            pos: tf.translation,
                            hp: en.hp,
                            dist: flat(tf.translation - center).length(),
                            radius: en.radius,
                        })
                        .collect();
                    foes.sort_by(|a, b| {
                        (a.e != h.target)
                            .cmp(&(b.e != h.target))
                            .then_with(|| a.dist.total_cmp(&b.dist))
                            .then_with(|| a.e.to_bits().cmp(&b.e.to_bits()))
                    });
                    let roots: Vec<_> = foes
                        .iter()
                        .filter(|f| f.dist <= p.range)
                        .take(
                            if p.tags
                                .intersects(engine::Tags::PROJECTILE | engine::Tags::MELEE)
                            {
                                p.count
                            } else {
                                1
                            },
                        )
                        .copied()
                        .collect();
                    for link in links(&roots, &foes, &p, 0.0, p.tags.contains(engine::Tags::CHAIN))
                    {
                        let mut copy = h.clone();
                        copy.target = link.target.e;
                        copy.damage *= link.scale;
                        copy.dot *= link.scale;
                        for repeat in 0..repeats {
                            pending.0.push((
                                now + link.arrival + engine::RECAST_DELAY * (repeat + 1) as f32,
                                copy.clone(),
                            ));
                        }
                        if link.arrival > 0.0 {
                            pending.0.push((now + link.arrival, copy));
                        } else {
                            prepared.push(copy);
                        }
                    }
                    continue;
                }
            }
            for repeat in 0..repeats {
                pending
                    .0
                    .push((now + engine::RECAST_DELAY * (repeat + 1) as f32, h.clone()));
            }
        }
        prepared.push(h);
    }
    // Splash reaches the target's neighbours at seventy percent.
    let mut all = Vec::with_capacity(prepared.len());
    for h in prepared {
        if h.splash > 0.0 {
            if let Ok((_, _, center, ..)) = enemies.get(h.target) {
                let c = center.translation;
                for (e, _, tf, en, ..) in enemies.iter() {
                    if e != h.target && !en.dead && flat(tf.translation - c).length() <= h.splash {
                        all.push(Hit {
                            target: e,
                            damage: h.damage * 0.7,
                            splash: 0.0,
                            mark: 0,
                            ..h.clone()
                        });
                    }
                }
            }
        }
        all.push(Hit { splash: 0.0, ..h });
    }

    let mut kill =
        |commands: &mut Commands, run: &mut Run, e: Entity, pos: Vec3, en: &mut Enemy| {
            super::talent_runtime::on_death(run, en, pos);
            en.dead = true;
            en.hp = 0.0;
            en.dots.clear();
            enemies::mark_dying(commands, e);
            loot.write(Loot {
                pos,
                xp: en.xp,
                boss: en.boss,
            });
            run.kills += 1;
            run.heal(run.equipment.life_on_kill);
            if en.boss {
                run.banner = Some((format!("{} is slain!", en.def.name), 3.0));
            }
        };

    for h in all {
        let Some(run) = super::multiplayer::player_run(&mut run, players.as_deref_mut(), h.owner)
        else {
            continue;
        };
        let Ok((e, guid, mut tf, mut en, mut store, mut marks)) = enemies.get_mut(h.target) else {
            continue;
        };
        if en.dead {
            continue;
        }
        let mods = *item_mods
            .entry((h.owner, h.skill))
            .or_insert_with(|| engine::on_hit::modifiers(h.skill, run.spell_modifiers.entries()));
        let armour = en.def.level as f32 * if en.boss { 1.0 } else { 0.5 };
        let talent_multiplier = super::talent_runtime::before_hit(run, &h, &en);
        let raw = engine::on_hit::damage(
            h.damage * talent_multiplier,
            en.hp,
            en.max_hp,
            en.boss,
            armour,
            mods,
        );
        let dmg = engine::damage_on_target(raw, en.hp, en.max_hp, en.boss, h.effect);
        if dmg > 0.0 {
            let immortal = super::training::dummy(&en);
            let effective = if immortal {
                dmg
            } else {
                dmg.min(en.hp.max(0.0))
            };
            super::talent_runtime::periodic_leech(run, h.skill, effective);
            let now = run.elapsed;
            run.meter
                .record(h.meter_source.unwrap_or(h.skill), now, effective);
            if immortal {
                en.hp = en.max_hp;
            } else {
                en.hp -= dmg;
            }
            run.damage_done += f64::from(effective);
            fx.number(e, dmg, h.crit, h.color);
            if let Effect::Drain(frac) = h.effect {
                run.heal(effective * frac);
            }
            super::items::combat::leech(run, effective * mods.leech);
            if !h.triggered {
                super::items::combat::record_hit(run, h.skill, mods.tags);
            }
            for (chance, factor, seconds, spell) in
                [(mods.bleed, 0.20, 4.0, 1943), (mods.ignite, 0.25, 3.0, 133)]
            {
                if en.hp > 0.0 && chance > 0.0 {
                    let roll = rng.f32();
                    if roll >= chance.clamp(0.0, 1.0) {
                        continue;
                    }
                    let meter_source = super::talent_runtime::ailment_source(run, spell, roll)
                        .or(h.meter_source)
                        .or(Some(h.skill));
                    let key = if spell == 1943 {
                        "proc_bleed"
                    } else {
                        "proc_ignite"
                    };
                    let p =
                        super::procs::resolve(run, key, 0.0, h.damage * factor, seconds, 0.0, 1);
                    // New stacks share their effect's next tick, even when the oldest
                    // stack is evicted; rapid hits must not restart the entire bleed clock.
                    let tick = en
                        .dots
                        .iter()
                        .filter(|d| {
                            d.owner == h.owner && d.meter_source == meter_source && d.skill == key
                        })
                        .map(|d| d.tick)
                        .fold(1.0_f32, f32::min);
                    if en.dots.len() >= 6 {
                        en.dots.remove(0);
                    }
                    en.dots.push(enemies::Dot {
                        owner: h.owner,
                        skill: key,
                        meter_source,
                        dps: p.dot,
                        left: p.duration,
                        tick,
                    });
                    marks.add(spell, p.duration);
                    fx.impact(e, spell);
                }
            }
            if mods.chill > 0.0 && rng.f32() < mods.chill.clamp(0.0, 1.0) {
                en.slow = if en.slow_t > 0.0 {
                    en.slow.min(0.75)
                } else {
                    0.75
                };
                en.slow_t = en.slow_t.max(2.0);
            }
            if en.hp > 0.0
                && !super::training::dummy(&en)
                && !en.boss
                && mods.knockback > 0.0
                && rng.f32() < mods.knockback.clamp(0.0, 1.0)
            {
                if let Some((hero, _, _)) = hero.iter().find(|(_, owner, local)| {
                    h.owner
                        .map_or(*local, |id| owner.is_some_and(|o| o.0 == id))
                }) {
                    let away = flat(tf.translation - hero.translation).normalize_or_zero();
                    tf.translation += away * 1.5;
                }
            }
        }
        if h.dot > 0.0 && h.dot_dur > 0.0 {
            if en.dots.len() >= 6 {
                en.dots.remove(0);
            }
            en.dots.push(enemies::Dot {
                owner: h.owner,
                skill: h.skill,
                meter_source: h.meter_source,
                dps: h.dot,
                left: h.dot_dur,
                tick: 1.0,
            });
        }
        super::talent_runtime::on_hit(run, &h, &mut en, tf.translation, &mut rng, &mut pending);
        let boss = en.boss;
        match h.effect {
            Effect::Slow(m, t) => {
                let m = if boss { m.max(0.75) } else { m };
                en.slow = if en.slow_t > 0.0 { en.slow.min(m) } else { m };
                en.slow_t = en.slow_t.max(t);
            }
            Effect::Root(t) => en.root_t = en.root_t.max(if boss { t * 0.3 } else { t }),
            Effect::Stun(t) => en.stun_t = en.stun_t.max(if boss { t * 0.3 } else { t }),
            Effect::Fear(t) if !boss => en.fear_t = en.fear_t.max(t),
            Effect::Weaken(m, t) => {
                en.weaken = m;
                en.weaken_t = t;
            }
            _ => {}
        }
        if h.mark != 0 {
            marks.add(h.mark, h.mark_dur);
        }
        if en.hp <= 0.0 {
            kill(&mut commands, run, e, tf.translation, &mut en);
            units::kill_unit(&mut store);
            marks.clear();
        }
        marks.flush(e, guid.0, &mut store, &mut fields);
    }

    // The over-time ticks, once a second each.
    for (e, guid, tf, mut en, mut store, mut marks) in &mut enemies {
        if en.dead || en.dots.is_empty() {
            continue;
        }
        let mut dealt = 0.0;
        let immortal = super::training::dummy(&en);
        let mut remaining_hp = en.hp.max(0.0);
        let mut last_owner = None;
        let (max_hp, boss) = (en.max_hp, en.boss);
        for d in &mut en.dots {
            d.left -= dt;
            d.tick -= dt;
            if d.tick <= 0.0 {
                let Some(run) =
                    super::multiplayer::player_run(&mut run, players.as_deref_mut(), d.owner)
                else {
                    d.left = 0.0;
                    continue;
                };
                d.tick += 1.0;
                let mods = *item_mods.entry((d.owner, d.skill)).or_insert_with(|| {
                    engine::on_hit::modifiers(d.skill, run.spell_modifiers.entries())
                });
                let amount = engine::on_hit::damage(d.dps, remaining_hp, max_hp, boss, 0.0, mods);
                dealt += amount;
                let effective = if immortal {
                    amount.max(0.0)
                } else {
                    amount.min(remaining_hp).max(0.0)
                };
                let now = run.elapsed;
                run.meter
                    .record(d.meter_source.unwrap_or(d.skill), now, effective);
                if !immortal {
                    remaining_hp = (remaining_hp - effective).max(0.0);
                }
                run.damage_done += f64::from(effective);
                super::talent_runtime::periodic_leech(run, d.skill, effective);
                if effective > 0.0 {
                    last_owner = d.owner;
                }
            }
        }

        if dealt > 0.0 {
            if immortal {
                en.hp = en.max_hp;
            } else {
                en.hp -= dealt;
            }
            fx.number(e, dealt, false, COLOR_SPELL);
            if en.hp <= 0.0 {
                if let Some(run) =
                    super::multiplayer::player_run(&mut run, players.as_deref_mut(), last_owner)
                {
                    kill(&mut commands, run, e, tf.translation, &mut en);
                }
                units::kill_unit(&mut store);
                marks.clear();
                marks.flush(e, guid.0, &mut store, &mut fields);
            }
        }
        en.dots.retain(|d| d.left > 0.0);
    }
}
