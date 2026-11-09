//! Auto-cast: every owned ability fires on its own cooldown at the targets its kind picks, with
//! the spell's own visuals; damage lands when its missile would.

use bevy::prelude::*;

use super::data::{self, AbilityDef, Effect, Kind};
use super::enemies::{self, Enemy, Loot};
use super::hero::{self, Hero};
use super::items::legendary::events::{self as legendary_events, Origin};
use super::spell_engine::targeting::{around, densest, links, nearest, weakest, Target as Foe};
use super::spell_engine::{self as engine, Resolved as Params};
use super::units::{self, flat, Fx, Marks, UnitSpec, COLOR_HEAL};
use super::{Rng, Run, RunEntity};
use crate::net::{FieldChanged, Guid, ObjectStore};

/// One blow landing on one enemy.
#[derive(Message, Clone)]
pub(super) struct Hit {
    pub direct_dot: Option<engine::PeriodicSnapshot>,
    pub dot_tags: Option<engine::Tags>,
    pub item_source: Option<u16>,
    pub origin: Origin,
    pub cast_id: u64,
    pub primary: bool,
    pub owner: Option<u64>,
    pub skill: &'static str,
    /// Display attribution only; spell rules and visuals still use `skill`.
    pub meter_source: Option<&'static str>,
    pub tags: Option<engine::Tags>,
    /// Triggered hits may benefit from gear, but never trigger another trinket proc.
    pub triggered: bool,
    /// Hidden payloads are resolved once before target copies and periodic snapshots.
    pub proc_resolved: bool,
    /// Its source has already paid the proc recovery delay; landing never pays it again.
    pub proc_admitted: bool,
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
}
impl Hit {
    pub(super) fn can_proc(&self) -> bool {
        !self.triggered && self.origin == Origin::Skill && super::procs::find(self.skill).is_none()
    }
}

#[cfg(test)]
mod spell_engine_integration_tests {
    #[test]
    fn boss_meter_filters_normal_targets_and_counts_direct_and_periodic_damage() {
        for (boss, rank, dummy, included) in [
            (false, None, false, false),
            (true, None, false, true),
            (false, Some(enemies::Rank::Elite), false, true),
            (false, Some(enemies::Rank::Boss), false, true),
            (false, Some(enemies::Rank::Sprinter), false, false),
            (false, None, true, true),
        ] {
            let (mut world, targets) = legendary_world("fireball", 0, &[3.0]);
            world.get_mut::<Enemy>(targets[0]).unwrap().boss = boss;
            if dummy {
                world.get_mut::<Enemy>(targets[0]).unwrap().def = &super::super::training::DUMMY;
            }
            if let Some(rank) = rank { world.entity_mut(targets[0]).insert(rank); }
            let mut hit = Hit::direct("fireball", targets[0], 40.0, false);
            hit.dot = 10.0;
            hit.dot_dur = 3.0;
            world.resource_mut::<Messages<Hit>>().write(hit);
            world.run_system_once(apply_hits).unwrap();
            let before: f64 = world.resource::<Run>().meter.rows(0.0).iter().map(|r| r.total).sum();
            assert!(before > 0.0);
            world.resource_mut::<Messages<Hit>>().clear();
            world.resource_mut::<Time>().advance_by(std::time::Duration::from_secs(1));
            world.run_system_once(apply_hits).unwrap();
            let run = world.resource::<Run>();
            let total: f64 = run.meter.rows(0.0).iter().map(|r| r.total).sum();
            let filtered: f64 = run.boss_meter.rows(0.0).iter().map(|r| r.total).sum();
            assert!(total > before, "periodic tick must deal damage");
            assert_eq!(filtered, if included { total } else { 0.0 });
            if dummy {
                let enemy = world.get::<Enemy>(targets[0]).unwrap();
                assert!(!enemy.boss);
                assert_eq!(enemy.hp, enemy.max_hp);
            }
            let encoded = serde_json::to_string(&run.boss_meter).unwrap();
            let decoded: super::super::combat_ui::Meter = serde_json::from_str(&encoded).unwrap();
            assert_eq!(decoded.rows(0.0).iter().map(|r| r.total).sum::<f64>(), filtered);
        }
    }

    #[test]
    fn hungry_pack_meter_separates_wolf_attacks_and_bleed_and_both_inherit_the_bow() {
        use super::super::{items::{Config, EquipmentType, Inventory, weapons::DamageRange}, minions, talent_runtime};
        let mut totals = Vec::new();
        for bow_damage in [0.0, 25.0] {
            let (mut world, targets) = legendary_world("arcane_shot", 0, &[3.0]);
            {
                let mut run = world.resource_mut::<Run>();
                run.class = &data::CLASSES[2];
                run.talent_build[0][8] = 1;
                run.talent_build[0][12] = 1;
                talent_runtime::configure(&mut run);
            }
            if bow_damage > 0.0 {
                let config = Config::builtin();
                let mut rng = Rng(123);
                let mut bow = (0..1000).find_map(|_| {
                    let item = config.generate(1, &mut rng).unwrap();
                    (item.equipment_type == EquipmentType::Bow).then_some(item)
                }).unwrap();
                bow.affixes.clear();
                bow.base_damage = Some(DamageRange { min: bow_damage, max: bow_damage });
                let mut inventory = Inventory::fixture(vec![bow]);
                inventory.equip(1, "Ranged").unwrap();
                inventory.apply(&mut world.resource_mut::<Run>());
            }
            world.run_system_once(talent_runtime::tick).unwrap();
            world.resource_mut::<Time>().advance_by(std::time::Duration::from_millis(600));
            world.resource_mut::<Run>().elapsed += 0.6;
            world.run_system_once(talent_runtime::tick).unwrap();
            world.run_system_once(apply_hits).unwrap();
            let bleed = world.get::<Enemy>(targets[0]).unwrap().dots.iter()
                .find(|d| d.meter_source == Some(minions::PACK_BLEED_SOURCE)).unwrap();
            assert!((bleed.dps - (18.0 + bow_damage) * 0.2).abs() < 0.001);
            let run = world.resource::<Run>();
            let direct = run.meter.rows(run.elapsed).into_iter()
                .find(|r| r.key == minions::PACK_ATTACK_SOURCE).unwrap().total;
            world.resource_mut::<Messages<Hit>>().clear();
            world.resource_mut::<Time>().advance_by(std::time::Duration::from_secs(1));
            world.resource_mut::<Run>().elapsed += 1.0;
            world.run_system_once(apply_hits).unwrap();
            let run = world.resource::<Run>();
            let rows = run.meter.rows(run.elapsed);
            assert_eq!(rows.len(), 2);
            assert_eq!(rows.iter().find(|r| r.key == minions::PACK_ATTACK_SOURCE).unwrap().total, direct);
            let periodic = rows.iter().find(|r| r.key == minions::PACK_BLEED_SOURCE).unwrap().total;
            assert!(direct > 0.0 && periodic > 0.0);
            totals.push((direct, periodic));
        }
        assert!(totals[1].0 > totals[0].0);
        assert!(totals[1].1 > totals[0].1);
    }

    #[test]
    fn death_trap_proliferates_explosive_trap_and_copied_dots_trigger_another_explosion() {
        use super::super::talent_runtime;
        let (mut world, targets) = legendary_world("explosive_trap", 0, &[1.0, 3.0, 30.0]);
        {
            let mut run = world.resource_mut::<Run>();
            run.class = &data::CLASSES[2];
            run.talent_build[2][18] = 1;
            talent_runtime::configure(&mut run);
        }
        {
            let mut enemy = world.get_mut::<Enemy>(targets[0]).unwrap();
            enemy.hp = 1.0;
            enemy.dots.push(enemies::Dot {
                owner: None, skill: "explosive_trap", meter_source: None, tags: Some(engine::Tags::FIRE | engine::Tags::DOT),
                item_source: None, origin: Origin::Skill, meta: default(), dps: 10.0, left: 1.2, tick: 0.0,
            });
        }
        world.run_system_once(apply_hits).unwrap();
        assert!(world.get::<Enemy>(targets[0]).unwrap().dead);
        world.resource_mut::<Time>().advance_by(std::time::Duration::from_millis(10));
        world.resource_mut::<Run>().elapsed = 0.01;
        world.run_system_once(talent_runtime::tick).unwrap();
        world.run_system_once(apply_hits).unwrap();
        let copied = &world.get::<Enemy>(targets[1]).unwrap().dots[0];
        assert_eq!(copied.skill, "explosive_trap");
        assert!(copied.left <= 1.2);
        let remaining = copied.left;
        world.get_mut::<Transform>(targets[2]).unwrap().translation.x = 5.0;
        {
            let mut enemy = world.get_mut::<Enemy>(targets[1]).unwrap();
            enemy.hp = 1.0;
            enemy.dots[0].tick = 0.0;
        }
        world.resource_mut::<Run>().elapsed = 0.02;
        world.run_system_once(apply_hits).unwrap();
        assert!(world.get::<Enemy>(targets[1]).unwrap().dead);
        world.resource_mut::<Run>().elapsed = 0.03;
        world.run_system_once(talent_runtime::tick).unwrap();
        let third = world.get::<Enemy>(targets[2]).unwrap();
        assert!(third.dots.iter().any(|d| d.skill == "explosive_trap" && d.left < remaining));
        assert!(world.resource::<PendingHits>().0.iter().any(|(_, h)| h.target == targets[2] && h.meter_source == Some("Death Trap")));
    }

    #[test]
    fn pack_frenzy_explodes_on_pet_and_bleed_kills_and_bleeds_survivors() {
        use super::super::{minions, talent_runtime};
        for periodic_kill in [false, true] {
            let (mut world, targets) = legendary_world("kill_command", 0, &[1.0, 3.0]);
            {
                let mut run = world.resource_mut::<Run>();
                run.class = &data::CLASSES[2];
                for node in [8, 12, 18] { run.talent_build[0][node] = 1; }
                talent_runtime::configure(&mut run);
            }
            {
                let mut enemy = world.get_mut::<Enemy>(targets[0]).unwrap();
                enemy.hp = 1.0;
                enemy.dots.push(enemies::Dot {
                    owner: None, skill: "proc_bleed", meter_source: Some(minions::PACK_BLEED_SOURCE),
                    tags: Some(engine::Tags::PHYSICAL | engine::Tags::MINION | engine::Tags::DOT),
                    item_source: None, origin: Origin::Talent, meta: default(),
                    dps: 10.0, left: 4.0, tick: if periodic_kill { 0.0 } else { 1.0 },
                });
            }
            if !periodic_kill {
                let mut hit = Hit::direct("proc_pet_bite", targets[0], 10.0, false);
                hit.origin = Origin::Talent;
                hit.triggered = true;
                hit.proc_resolved = true;
                world.write_message(hit);
            }
            world.run_system_once(apply_hits).unwrap();
            assert!(world.get::<Enemy>(targets[0]).unwrap().dead);
            let before = world.get::<Enemy>(targets[1]).unwrap().hp;
            world.resource_mut::<Messages<Hit>>().clear();
            world.resource_mut::<Time>().advance_by(std::time::Duration::from_millis(10));
            world.run_system_once(talent_runtime::tick).unwrap();
            assert!(world.resource::<PendingHits>().0.iter().any(|(_, h)| h.target == targets[1] && h.meter_source == Some("Pack Frenzy")));
            world.run_system_once(apply_hits).unwrap();
            let survivor = world.get::<Enemy>(targets[1]).unwrap();
            assert!(survivor.hp < before);
            assert!(survivor.dots.iter().any(|d| d.skill == "proc_bleed" && d.meter_source == Some(minions::PACK_BLEED_SOURCE)));
        }
    }

    #[test]
    fn pack_frenzy_is_a_pet_attack_and_reapplies_hungry_pack_bleed() {
        use super::super::{minions, procs, talent_runtime};
        for pack in [false, true] {
            let (mut world, targets) = legendary_world("arcane_shot", 0, &[3.0]);
            let p = {
                let mut run = world.resource_mut::<Run>();
                run.class = &data::CLASSES[2];
                run.talent_build[0][18] = 1;
                if pack { run.talent_build[0][12] = 1; }
                talent_runtime::configure(&mut run);
                procs::resolve(&run, "proc_bloodburst", 100.0, 0.0, 0.0, 8.5, 64)
            };
            assert!(p.tags.contains(engine::Tags::MINION | engine::Tags::ATTACK));
            assert!(!p.tags.contains(engine::Tags::SPELL));
            let mut hit = Hit::direct("proc_bloodburst", targets[0], p.dmg, false);
            hit.origin = Origin::Talent;
            hit.triggered = true;
            hit.proc_admitted = true;
            hit.proc_resolved = true;
            hit.tags = Some(p.tags);
            hit.meter_source = Some("Pack Frenzy");
            world.write_message(hit);
            world.run_system_once(apply_hits).unwrap();
            let enemy = world.get::<Enemy>(targets[0]).unwrap();
            let bleed = enemy.dots.iter().find(|d| d.meter_source == Some(minions::PACK_BLEED_SOURCE));
            assert_eq!(bleed.is_some(), pack);
            if let Some(bleed) = bleed {
                assert_eq!(bleed.skill, "proc_bleed");
                assert!((bleed.dps - p.dmg * 0.2).abs() < 0.001);
            }
            let count = enemy.dots.len();
            world.resource_mut::<Messages<Hit>>().clear();
            world.resource_mut::<Time>().advance_by(std::time::Duration::from_secs(1));
            world.run_system_once(apply_hits).unwrap();
            assert_eq!(world.get::<Enemy>(targets[0]).unwrap().dots.len(), count);
        }
    }

    #[test]
    fn hungry_pack_triggers_from_minion_skill_damage_without_replacing_native_dots_or_recursing() {
        use super::super::{talent_runtime, items::legendary::periodic};
        for pack in [false, true] {
            for skill in ["stomp", "kill_command", "proc_pet_bite", "proc_demon_bolt", "arcane_shot"] {
                let (mut world, targets) = legendary_world("arcane_shot", 0, &[3.0]);
                if pack {
                    let mut run = world.resource_mut::<Run>();
                    run.class = &data::CLASSES[2];
                    run.talent_build[0][12] = 1;
                    talent_runtime::configure(&mut run);
                }
                let mut hit = Hit::direct(skill, targets[0], 100.0, false);
                hit.tags = Some(engine::tags::for_spell(data::ability(skill)));
                hit.triggered = true;
                hit.proc_resolved = true;
                hit.dot = 3.0;
                hit.dot_dur = 4.0;
                world.write_message(hit);
                world.run_system_once(apply_hits).unwrap();
                let en = world.get::<Enemy>(targets[0]).unwrap();
                let bleed = en.dots.iter().find(|d| d.meter_source == Some(super::super::minions::PACK_BLEED_SOURCE));
                assert_eq!(bleed.is_some(), pack && skill != "arcane_shot", "{skill}");
                assert!(en.dots.iter().any(|d| d.skill == skill && d.dps == 3.0));
                if let Some(bleed) = bleed {
                    assert!((bleed.dps - 20.0).abs() < 0.001);
                    assert!((bleed.left - (4.0 - world.resource::<Time>().delta_secs())).abs() < 0.001);
                    assert!(matches!(periodic::family(bleed), periodic::Family::Bleed));
                }
                let count = en.dots.len();
                world.resource_mut::<Messages<Hit>>().clear();
                world.resource_mut::<Time>().advance_by(std::time::Duration::from_secs(1));
                world.run_system_once(apply_hits).unwrap();
                assert_eq!(world.get::<Enemy>(targets[0]).unwrap().dots.len(), count);
            }
        }
    }

    #[test]
    fn freeze_status_and_fifty_percent_slow_remain_on_bosses_and_elites() {
        for rank in [enemies::Rank::Normal, enemies::Rank::Elite, enemies::Rank::Boss] {
            let (mut world, targets) = legendary_world("frost_nova", 0, &[3.0]);
            world.entity_mut(targets[0]).insert(rank);
            world.get_mut::<Enemy>(targets[0]).unwrap().boss = rank == enemies::Rank::Boss;
            let mut hit = Hit::direct("frost_nova", targets[0], 1.0, false);
            hit.effect = Effect::Root(2.5);
            world.write_message(hit);
            world.run_system_once(apply_hits).unwrap();
            assert!(world.get::<Enemy>(targets[0]).unwrap().root_t > 0.0);
            let mut hit = Hit::direct("frostbolt", targets[0], 1.0, false);
            hit.effect = Effect::Slow(0.1, 3.0);
            world.write_message(hit);
            world.run_system_once(apply_hits).unwrap();
            let en = world.get::<Enemy>(targets[0]).unwrap();
            assert_eq!(en.slow, if rank == enemies::Rank::Normal { 0.1 } else { 0.5 });
            assert!(en.root_t > 0.0);
        }
    }

    #[test]
    fn pet_damage_leeches_life_for_its_owner_without_triggering_ailments() {
        use super::super::multiplayer::{Companion, Companions};
        for guest in [false, true] {
            for skill in ["proc_pet_bite", "proc_demon_bolt", "kill_command", "stomp", "proc_fireburst"] {
                let (mut world, targets) = legendary_world("arcane_shot", 0, &[3.0]);
                let mut player = Run::default();
                player.hp = 10.0;
                player.replace_spell_modifiers(903, [engine::Change::LifeLeech(0.5), engine::Change::BleedChance(1.0)]
                    .into_iter().map(|change| engine::Modifier {
                        source: 903, filter: engine::Filter::requiring(engine::Tags::ATTACK), change,
                    }));
                if guest {
                    player.owner = Some(7);
                    let mut players = Companions::default();
                    players.0.insert(7, Companion {
                        entity: Entity::PLACEHOLDER, run: player,
                        physics: Hero { vy: 0.0, air: 0.0 }, offer: default(), offer_id: 0,
                    });
                    world.insert_resource(players);
                } else {
                    let mut host = world.resource_mut::<Run>();
                    host.hp = player.hp;
                    host.replace_spell_modifiers(903, player.spell_modifiers.entries().iter().copied());
                }
                // Prepared hits cover Tame Beast, repeats and commands; hidden hits cover wolves/demons.
                let mut hit = Hit::direct(skill, targets[0], 10.0, false);
                hit.triggered = true;
                hit.owner = guest.then_some(7);
                if matches!(skill, "kill_command" | "stomp") {
                    hit.tags = Some(engine::tags::for_spell(data::ability(skill)));
                    hit.proc_resolved = true;
                }
                world.write_message(hit);
                world.run_system_once(apply_hits).unwrap();
                let recovered = if guest { world.resource::<Companions>().0[&7].run.hp - 10.0 }
                    else { world.resource::<Run>().hp - 10.0 };
                let dealt = 1000.0 - world.get::<Enemy>(targets[0]).unwrap().hp;
                if skill == "proc_fireburst" { assert_eq!(recovered, 0.0); }
                else { assert!(dealt > 0.0); assert!((recovered - dealt * 0.5).abs() < 0.001); }
                assert!(world.get::<Enemy>(targets[0]).unwrap().dots.is_empty());
                if guest { assert_eq!(world.resource::<Run>().damage_done, 0.0); }
            }
        }
    }

    #[test]
    fn contact_threats_override_ranged_boss_priority_until_they_leave_the_danger_zone() {
        let (mut world, targets) = legendary_world("frostbolt", 0, &[1.0, 4.0, 6.0]);
        world.resource_mut::<Run>().legendary.equipped.clear();
        world.get_mut::<Enemy>(targets[1]).unwrap().hp = 1.0;
        world.get_mut::<Enemy>(targets[2]).unwrap().boss = true;
        world.run_system_once(cast_abilities).unwrap();
        let first: Vec<_> = world
            .resource_mut::<Messages<SpellGoTargets>>()
            .drain()
            .collect();
        assert_eq!(first[0].hits[0], targets[0]);
        world
            .get_mut::<Transform>(targets[0])
            .unwrap()
            .translation
            .x = 3.0;
        world.resource_mut::<Run>().abilities[0].cd = 0.0;
        world.run_system_once(cast_abilities).unwrap();
        let next: Vec<_> = world
            .resource_mut::<Messages<SpellGoTargets>>()
            .drain()
            .collect();
        assert_eq!(next[0].hits[0], targets[2]);
    }
    #[test]
    fn minion_stomp_waits_for_pet_range_and_casts_and_slows_from_the_pet() {
        let (mut world, targets) = legendary_world("tame_beast", 0, &[1.0]);
        world.resource_mut::<Run>().class = &data::CLASSES[2];
        world.run_system_once(cast_abilities).unwrap();
        assert!(world.resource::<Run>().talent_runtime.minions.tame_active);
        world
            .run_system_once(super::super::talent_runtime::tick)
            .unwrap();
        let pet = world
            .query_filtered::<Entity, With<super::super::talent_runtime::Summon>>()
            .single(&world)
            .unwrap();
        world.get_mut::<Transform>(pet).unwrap().translation.x = 20.0;
        world.resource_mut::<Run>().abilities.push(Owned {
            def: data::ability("stomp"),
            level: 1,
            cd: 0.0,
        });
        world.resource_mut::<Messages<CastEvent>>().clear();
        world.run_system_once(cast_abilities).unwrap();
        assert!(world
            .resource::<Run>()
            .talent_runtime
            .minions
            .stomp
            .is_none());
        world
            .get_mut::<Transform>(targets[0])
            .unwrap()
            .translation
            .x = 21.0;
        world.resource_mut::<Run>().abilities[1].cd = 0.0;
        world.run_system_once(cast_abilities).unwrap();
        assert!(world
            .resource::<Run>()
            .talent_runtime
            .minions
            .stomp
            .is_some());
        assert!(world.resource::<Messages<CastEvent>>().is_empty());
        world
            .run_system_once(super::super::talent_runtime::tick)
            .unwrap();
        let casts: Vec<_> = world
            .resource_mut::<Messages<CastEvent>>()
            .drain()
            .filter(|event| event.spell_id == 20549)
            .collect();
        assert_eq!(casts.len(), 1);
        assert_eq!(casts[0].entity, pet);
        world.run_system_once(apply_hits).unwrap();
        let enemy = world.get::<Enemy>(targets[0]).unwrap();
        assert_eq!(enemy.slow, 0.5);
        assert!(enemy.slow_t > 0.0);
        assert!(enemy.hp < 1000.0);
    }
    #[test]
    fn totems_replace_oldest_at_each_spells_limit() {
        for key in [
            "searing_totem",
            "magma_totem",
            "earthbind_totem",
            "healing_stream_totem",
            "strength_of_earth_totem",
        ] {
            for extra in [0, 1, 3] {
                let (mut world, _) = legendary_world(key, 0, &[1.0]);
                {
                    let mut run = world.resource_mut::<Run>();
                    run.abilities.retain(|o| o.def.key == key);
                    run.replace_spell_modifiers(
                        901,
                        [engine::Modifier {
                            source: 901,
                            filter: engine::Filter::requiring(engine::Tags::TOTEM),
                            change: engine::Change::AdditionalTotems(extra),
                        }],
                    );
                }
                for _ in 0..6 {
                    world.resource_mut::<Run>().abilities[0].cd = 0.0;
                    world.run_system_once(cast_abilities).unwrap();
                }
                let count = world
                    .query::<&Totem>()
                    .iter(&world)
                    .filter(|t| t.skill == key)
                    .count()
                    + world
                        .resource::<Run>()
                        .support
                        .zones
                        .iter()
                        .filter(|z| z.key == key)
                        .count();
                assert_eq!(count, 1 + extra as usize, "{key}");
            }
        }
    }

    use super::super::{data, Owned};
    use super::*;
    use crate::combat_text::CombatTextSpawn;
    use crate::creature_anim::{CastEvent, PlaySeq, SpellGoTargets, SwingMessage};
    use bevy::ecs::system::RunSystemOnce;
    #[test]
    fn specialization_signature_adds_two_real_projectiles_at_rank_six() {
        let (mut world, _) = legendary_world("fireball", 0, &[1.0, 2.0, 3.0]);
        world.resource_mut::<Run>().abilities[0].level = 5;
        world.run_system_once(cast_abilities).unwrap();
        assert_eq!(world.resource::<Messages<Hit>>().len(), 1);
        world.resource_mut::<Messages<Hit>>().clear();
        world.resource_mut::<Run>().abilities[0].level = 6;
        world.resource_mut::<Run>().abilities[0].cd = 0.0;
        world.run_system_once(cast_abilities).unwrap();
        assert_eq!(world.resource::<Messages<Hit>>().len(), 3);
    }
    #[test]
    fn searing_totem_projectile_modifiers_apply_to_its_real_volley() {
        let (mut world, _) = legendary_world("searing_totem", 0, &[1.0, 2.0, 3.0]);
        let base = world
            .resource::<Run>()
            .resolve_spell(data::ability("searing_totem"), 1)
            .dmg;
        world.resource_mut::<Run>().replace_spell_modifiers(
            125,
            [
                engine::Modifier {
                    source: 125,
                    filter: engine::Filter::requiring(engine::Tags::PROJECTILE),
                    change: engine::Change::AdditionalProjectiles(2),
                },
                engine::Modifier {
                    source: 125,
                    filter: engine::Filter::requiring(engine::Tags::PROJECTILE),
                    change: engine::Change::IncreasedDamage(1.0),
                },
            ],
        );
        let p = world
            .resource::<Run>()
            .resolve_spell(data::ability("searing_totem"), 1);
        assert!(p
            .tags
            .contains(engine::Tags::PROJECTILE | engine::Tags::TOTEM | engine::Tags::FIRE));
        assert_eq!(p.count, 3);
        assert!(!engine::tags::for_spell(data::ability("magma_totem"))
            .contains(engine::Tags::PROJECTILE));
        world.run_system_once(cast_abilities).unwrap();
        assert_eq!(world.query::<&Totem>().iter(&world).count(), 1);
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(0.4));
        world.run_system_once(tick_areas).unwrap();
        let hits: Vec<_> = world.resource_mut::<Messages<Hit>>().drain().collect();
        assert_eq!(hits.len(), 3);
        assert_eq!(
            hits.iter()
                .map(|h| h.target)
                .collect::<std::collections::HashSet<_>>()
                .len(),
            3
        );
        assert!(hits.iter().all(|h| (h.damage - base * 2.0).abs() < 0.001
            && h.tags.unwrap().contains(engine::Tags::PROJECTILE)));
    }

    #[test]
    fn rogue_knives_and_smoke_emit_visible_casts_and_a_lifetime_bound_cloud() {
        for (key, visual) in [("fan_of_knives", 1680), ("smoke_bomb", 10389)] {
            let (mut world, _) = legendary_world(key, 0, &[1.0, 2.0]);
            world.init_resource::<crate::entities::dest_fx::AreaVisualScales>();
            world.run_system_once(cast_abilities).unwrap();
            assert!(world.resource::<Messages<CastEvent>>().iter_current_update_messages().any(|e| e.spell_id == visual));
            assert!(world.resource::<Messages<SpellGoTargets>>().iter_current_update_messages().any(|e| e.spell_id == visual));
            if key == "fan_of_knives" {
                let hits: Vec<_> = world.resource::<Messages<Hit>>().iter_current_update_messages().collect();
                assert_eq!(hits.len(), 2);
                assert!(hits.iter().all(|h| h.skill == key && h.damage > 0.0));
            } else {
                let zone = &world.resource::<Run>().support.zones[0];
                let anchor = zone.entity;
                let until = zone.until;
                assert_eq!(world.get::<ObjectStore>(anchor).unwrap().0.dynamicobject_spell_id(), Some(10389));
                assert_eq!(world.get::<ObjectStore>(anchor).unwrap().0.object_scale_x(), Some(zone.radius / 2.0));
                assert!(world.resource::<Messages<Hit>>().is_empty());
                world.resource_mut::<Run>().elapsed = until + 0.1;
                world.run_system_once(cast_abilities).unwrap();
                assert!(world.get_entity(anchor).is_err());
                assert!(world.resource::<Run>().support.zones.iter().all(|z| z.entity != anchor));
            }
        }
    }

    #[test]
    fn support_all_new_skills_cast_and_buffs_never_emit_damage_hits() {
        for &(_, key, _, _) in super::super::support_spells::SPELLS {
            let (mut world, _) = legendary_world(key, 0, &[1.0, 2.0]);
            world.resource_mut::<Run>().support.attacks = 3;
            world.resource_mut::<Run>().support.melee = 3;
            world.run_system_once(cast_abilities).unwrap();
            assert!(world.resource::<Run>().abilities[0].cd > 0.2, "{key}");
            if data::ability(key).kind == Kind::Buff {
                assert!(world.resource::<Messages<Hit>>().is_empty(), "{key}");
            }
        }
    }

    fn support_party(world: &mut World) -> [Entity; 3] {
        let mut party = super::super::multiplayer::Companions::default();
        let mut entities = Vec::new();
        for (owner, x, hp) in [(42, 2.0, 50.0), (43, 40.0, 50.0), (44, 2.0, 0.0)] {
            let entity = world.spawn(Transform::from_xyz(x, 0.0, 0.0)).id();
            party.0.insert(
                owner,
                super::super::multiplayer::Companion {
                    entity,
                    run: Run {
                        owner: Some(owner),
                        hp,
                        ..default()
                    },
                    physics: Hero { vy: 0.0, air: 0.0 },
                    offer: default(),
                    offer_id: 0,
                },
            );
            entities.push(entity);
        }
        world.insert_resource(party);
        entities.try_into().unwrap()
    }

    #[test]
    fn support_buff_effect_card_increases_the_snapshot_given_to_party_members() {
        let (mut world, _) = legendary_world("battle_shout", 0, &[1.0]);
        support_party(&mut world);
        {
            let mut run = world.resource_mut::<Run>();
            let passive = data::PASSIVES
                .iter()
                .find(|p| p.key == "buff_effect")
                .unwrap();
            run.passives.push((passive, 4));
            super::super::passives::sync(&mut run);
        }
        world.run_system_once(cast_abilities).unwrap();
        assert!((world.resource::<Run>().stats().attack_power - 0.12 * 1.2).abs() < 0.001);
        let party = world.resource::<super::super::multiplayer::Companions>();
        assert!((party.0[&42].run.stats().attack_power - 0.12 * 1.2).abs() < 0.001);
        assert_eq!(
            super::super::support_spells::effective_magnitude("barkskin", 8, 1000.0),
            0.75
        );
        assert_eq!(
            super::super::support_spells::effective_magnitude("smoke_bomb", 8, 1000.0),
            0.75
        );
    }
    #[test]
    fn support_party_buffs_respect_range_life_and_do_not_stack() {
        let (mut world, _) = legendary_world("battle_shout", 0, &[1.0]);
        support_party(&mut world);
        world.run_system_once(cast_abilities).unwrap();
        let party = world.resource::<super::super::multiplayer::Companions>();
        assert!((party.0[&42].run.stats().attack_power - 0.12).abs() < 0.001);
        assert_eq!(party.0[&43].run.stats().attack_power, 0.0);
        assert_eq!(party.0[&44].run.stats().attack_power, 0.0);
        world.resource_mut::<Run>().abilities[0].cd = 0.0;
        world.run_system_once(cast_abilities).unwrap();
        let party = world.resource::<super::super::multiplayer::Companions>();
        assert!((party.0[&42].run.stats().attack_power - 0.12).abs() < 0.001);
    }

    #[test]
    fn support_prayer_bounces_to_a_living_ally_and_credits_original_caster() {
        let (mut world, _) = legendary_world("prayer_of_mending", 0, &[1.0]);
        support_party(&mut world);
        world.resource_mut::<Run>().hp = 10.0;
        world.run_system_once(cast_abilities).unwrap();
        assert!(
            world
                .resource::<Run>()
                .support
                .power(0.0, "prayer_of_mending")
                > 0.0
        );
        hero::hurt_hero(&mut world.resource_mut::<Run>(), 5.0);
        world.run_system_once(cast_abilities).unwrap();
        let party = world.resource::<super::super::multiplayer::Companions>();
        let prayer = party.0[&42]
            .run
            .support
            .buffs
            .iter()
            .find(|b| b.key == "prayer_of_mending")
            .unwrap();
        assert_eq!(prayer.jumps, 4);
        assert_eq!(prayer.caster, None);
        assert!(party.0[&43].run.support.buffs.is_empty());
        assert!(!world.resource::<Run>().healing_meter.rows(0.0).is_empty());
    }

    #[test]
    fn support_healing_totem_is_stationary_and_heals_nearby_allies_only() {
        let (mut world, _) = legendary_world("healing_stream_totem", 0, &[1.0]);
        support_party(&mut world);
        world.resource_mut::<Run>().hp = 10.0;
        world.run_system_once(cast_abilities).unwrap();
        world.run_system_once(cast_abilities).unwrap();
        assert!(world.resource::<Run>().hp > 10.0);
        let party = world.resource::<super::super::multiplayer::Companions>();
        assert!(party.0[&42].run.hp > 50.0);
        assert_eq!(party.0[&43].run.hp, 50.0);
        assert_eq!(party.0[&44].run.hp, 0.0);
        let hp = world.resource::<Run>().hp;
        let hero = world
            .query_filtered::<Entity, With<Hero>>()
            .single(&world)
            .unwrap();
        world.get_mut::<Transform>(hero).unwrap().translation.x = 30.0;
        world.resource_mut::<Run>().elapsed = 2.0;
        world.run_system_once(cast_abilities).unwrap();
        assert_eq!(world.resource::<Run>().hp, hp);
    }

    #[test]
    fn totemic_recall_moves_all_owned_totem_types_without_refreshing_or_stealing_them() {
        for owner in [None, Some(42)] {
            for exclude_others in [false, true] {
                let (mut world, _) = legendary_world("searing_totem", 0, &[1.0]);
                {
                    let mut run = world.resource_mut::<Run>();
                    run.legendary.equipped.clear();
                    run.owner = owner;
                    run.abilities = [
                        "searing_totem",
                        "magma_totem",
                        "earthbind_totem",
                        "strength_of_earth_totem",
                        "healing_stream_totem",
                    ]
                    .into_iter()
                    .map(|key| Owned {
                        def: data::ability(key),
                        level: 1,
                        cd: 0.0,
                    })
                    .collect();
                }
                world.run_system_once(cast_abilities).unwrap();
                let mut generic: Vec<_> = world
                    .query::<(Entity, &Totem, &Transform)>()
                    .iter(&world)
                    .map(|(e, t, tf)| (e, t.left, t.tick, tf.translation))
                    .collect();
                generic.sort_by_key(|(e, ..)| e.to_bits());
                assert_eq!(generic.len(), 3);
                if exclude_others {
                    world.get_mut::<Totem>(generic[0].0).unwrap().owner = Some(99);
                    world.get_mut::<Totem>(generic[1].0).unwrap().left = 0.0;
                }
                for zone in &mut world.resource_mut::<Run>().support.zones {
                    zone.next = 1.0;
                }
                let zones: Vec<_> = world
                    .resource::<Run>()
                    .support
                    .zones
                    .iter()
                    .map(|z| (z.entity, z.until, z.power, z.next))
                    .collect();
                assert_eq!(zones.len(), 2);
                let hero = world
                    .query_filtered::<Entity, With<Hero>>()
                    .single(&world)
                    .unwrap();
                world.get_mut::<Transform>(hero).unwrap().translation.x = 30.0;
                world.resource_mut::<Run>().abilities = vec![Owned {
                    def: data::ability("totemic_recall"),
                    level: 1,
                    cd: 0.0,
                }];
                world.run_system_once(cast_abilities).unwrap();
                for (index, (entity, left, tick, original)) in generic.iter().enumerate() {
                    let totem = world.get::<Totem>(*entity).unwrap();
                    let position = world.get::<Transform>(*entity).unwrap().translation;
                    if exclude_others && index < 2 {
                        assert_eq!(position, *original);
                    } else {
                        assert!((position - Vec3::X * 30.0).length() <= 3.01);
                        assert_eq!(totem.left, *left);
                    }
                    assert_eq!(totem.tick, *tick);
                }
                for (entity, until, power, next) in zones {
                    let zone = world
                        .resource::<Run>()
                        .support
                        .zones
                        .iter()
                        .find(|z| z.entity == entity)
                        .unwrap();
                    assert!((zone.pos - Vec3::X * 30.0).length() <= 3.01);
                    assert_eq!(
                        world.get::<Transform>(entity).unwrap().translation,
                        zone.pos
                    );
                    assert_eq!((zone.until, zone.power, zone.next), (until, power, next));
                }
                assert!(world.resource::<Run>().abilities[0].cd > 1.0);
                world.get_mut::<Transform>(hero).unwrap().translation.x = 60.0;
                world.run_system_once(cast_abilities).unwrap();
                assert!(world
                    .resource::<Run>()
                    .support
                    .zones
                    .iter()
                    .all(|z| z.pos.x < 35.0));
            }
        }
    }

    #[test]
    fn totemic_recall_is_a_learnable_utility_and_waits_for_distant_totems() {
        let (mut world, _) = legendary_world("totemic_recall", 0, &[]);
        world.run_system_once(cast_abilities).unwrap();
        assert_eq!(world.resource::<Run>().abilities[0].cd, 0.2);
        let run = world.resource::<Run>();
        let def = data::ability("totemic_recall");
        assert!(data::CLASSES
            .iter()
            .find(|c| c.id == 7)
            .unwrap()
            .pool
            .contains(&def.key));
        let low = run.resolve_spell(def, 1);
        let gold = run.resolve_spell(def, 6);
        assert!(low
            .tags
            .contains(engine::Tags::UTILITY | engine::Tags::TOTEM | engine::Tags::SPELL));
        assert!(!low
            .tags
            .intersects(engine::Tags::DAMAGE | engine::Tags::BUFF | engine::Tags::DURATION));
        assert_eq!(gold.dmg, 0.0);
        assert!(gold.cooldown < low.cooldown * 0.75);
        let text =
            super::super::hud::describe_card(super::super::hud::Choice::NewAbility(def), run, None);
        assert_eq!(text.name, "Totemic Recall");
        assert!(text.desc.contains("remaining duration"));
    }

    #[test]
    fn added_skills_are_learnable_classified_and_cast_in_the_real_system() {
        let rewards: toml::Value =
            toml::from_str(super::super::reward_config::DEFAULT_FILE).unwrap();
        for &(class_id, first, second) in super::super::extra_spells::ADDITIONS {
            let class = data::CLASSES.iter().find(|c| c.id == class_id).unwrap();
            for key in [first, second] {
                assert_eq!(class.pool.iter().filter(|k| **k == key).count(), 1, "{key}");
                assert_eq!(rewards["spells"][key].as_integer(), Some(1));
                assert!(engine::tags::school(key).is_some(), "{key}");
                let def = data::ability(key);
                assert_eq!(
                    data::ABILITIES
                        .iter()
                        .filter(|other| other.spell == def.spell)
                        .count(),
                    1,
                    "{key} needs its own visual identity"
                );
                let (mut world, _) = legendary_world(key, 0, &[1.0, 2.0, 6.0]);
                world.resource_mut::<Run>().extra_spells.revenge_until = 5.0;
                world.run_system_once(cast_abilities).unwrap();
                assert!(world.resource::<Run>().abilities[0].cd > 0.2, "{key}");
            }
        }
    }

    #[test]
    fn added_direct_synergies_and_soul_burn_use_only_the_casters_dots() {
        for (key, expected_damage, expected_dot) in
            [("garrote", 30.0, 15.0), ("lava_lash", 30.0, 0.0)]
        {
            let (mut world, targets) = legendary_world(key, 0, &[1.0]);
            if key == "lava_lash" {
                let mut dot = Hit::direct("flame_shock", targets[0], 0.0, false);
                dot.dot = 10.0;
                dot.dot_dur = 8.0;
                world.write_message(dot);
                world.run_system_once(apply_hits).unwrap();
                world.resource_mut::<Messages<Hit>>().clear();
            }
            let before = world.get::<Enemy>(targets[0]).unwrap().hp;
            let expected_damage = {
                let run = world.resource::<Run>();
                let en = world.get::<Enemy>(targets[0]).unwrap();
                let (armour, reduction) =
                    run.expeditions
                        .defenses(run.expedition_map, en.def.level, en.boss, false);
                let tags = engine::tags::for_spell(data::ability(key));
                let mods =
                    engine::on_hit::modifiers_tagged(key, tags, run.spell_modifiers.entries());
                engine::on_hit::damage(expected_damage, en.hp, en.max_hp, en.boss, armour, mods)
                    * (1.0 - reduction)
            };
            let mut h = Hit::direct(key, targets[0], 20.0, false);
            if key == "garrote" {
                h.dot = 10.0;
                h.dot_dur = 8.0;
            }
            world.write_message(h);
            world.run_system_once(apply_hits).unwrap();
            let en = world.get::<Enemy>(targets[0]).unwrap();
            assert!((before - en.hp - expected_damage).abs() < 0.001, "{key}");
            if expected_dot > 0.0 {
                assert_eq!(en.dots[0].dps, expected_dot);
            }
        }
        let (mut world, targets) = legendary_world("soul_burn", 0, &[1.0]);
        let mut dot = Hit::direct("corruption", targets[0], 0.0, false);
        dot.dot = 10.0;
        dot.dot_dur = 10.0;
        world.write_message(dot);
        world.run_system_once(apply_hits).unwrap();
        world.resource_mut::<Messages<Hit>>().clear();
        let before = world.get::<Enemy>(targets[0]).unwrap().hp;
        let mut foreign = world.get::<Enemy>(targets[0]).unwrap().dots[0].clone();
        foreign.owner = Some(42);
        foreign.left = 8.0;
        world
            .get_mut::<Enemy>(targets[0])
            .unwrap()
            .dots
            .push(foreign);
        world.write_message(Hit::direct("soul_burn", targets[0], 10.0, false));
        world.run_system_once(apply_hits).unwrap();
        let en = world.get::<Enemy>(targets[0]).unwrap();
        assert!(en.dots[0].left < 5.0);
        assert!(en.dots[1].left > 7.5);
        assert!(before - en.hp >= 50.0);
    }

    #[test]
    fn added_penance_schedules_three_hits_and_utility_fields_apply_slow() {
        let (mut world, _) = legendary_world("penance", 0, &[1.0]);
        world.run_system_once(cast_abilities).unwrap();
        assert_eq!(world.resource::<Messages<Hit>>().len(), 1);
        assert_eq!(world.resource::<PendingHits>().0.len(), 2);
        for key in ["frost_trap", "earthbind_totem"] {
            let (mut world, targets) = legendary_world(key, 0, &[1.0]);
            world.run_system_once(cast_abilities).unwrap();
            let physical_trap = if key == "frost_trap" {
                let entity = world.query_filtered::<Entity, With<GroundArea>>().single(&world).unwrap();
                assert_eq!(world.get::<crate::net::NetEntity>(entity).unwrap().display_id, Some(3073));
                assert_eq!(world.get::<ObjectStore>(entity).unwrap().0.gameobject_state(), Some(1));
                Some(entity)
            } else { None };
            world
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_secs(1));
            world.run_system_once(tick_areas).unwrap();
            if let Some(entity) = physical_trap {
                assert_eq!(world.get::<ObjectStore>(entity).unwrap().0.gameobject_state(), Some(0));
            }
            world.run_system_once(apply_hits).unwrap();
            let en = world.get::<Enemy>(targets[0]).unwrap();
            assert!(en.slow_t > 0.0, "{key}");
            assert_eq!(en.slow, 0.5, "{key}");
            assert_eq!(en.hp, en.max_hp, "{key}");
        }
    }

    #[test]
    fn added_disease_ticks_heal_and_ice_armor_reduces_incoming_damage() {
        let (mut world, targets) = legendary_world("devouring_plague", 0, &[1.0]);
        world.resource_mut::<Run>().hp = 10.0;
        world.run_system_once(cast_abilities).unwrap();
        world.run_system_once(apply_hits).unwrap();
        world.resource_mut::<Messages<Hit>>().clear();
        world.get_mut::<Enemy>(targets[0]).unwrap().dots[0].tick = 0.0;
        world.run_system_once(apply_hits).unwrap();
        assert!(world.resource::<Run>().hp > 10.0);
        let mut run = Run::default();
        run.extra_spells.ice_until = 6.0;
        assert!((hero::hurt_hero(&mut run, 20.0) - 17.0).abs() < 0.001);
    }

    #[test]
    fn added_revenge_waits_for_a_block_and_holy_shield_retalitates_only_on_block() {
        let (mut world, targets) = legendary_world("revenge", 0, &[1.0]);
        world.run_system_once(cast_abilities).unwrap();
        assert!(world.resource::<Messages<Hit>>().is_empty());
        world.resource_mut::<Run>().equipment.block = 0.5;
        hero::hurt_hero_with_block(&mut world.resource_mut::<Run>(), 10.0, 0.1);
        world.run_system_once(cast_abilities).unwrap();
        assert!(!world.resource::<Messages<Hit>>().is_empty());
        assert_eq!(world.resource::<Run>().extra_spells.revenge_until, 0.0);

        let (mut world, _) = legendary_world("holy_shield", 0, &[1.0]);
        world.run_system_once(cast_abilities).unwrap();
        {
            let mut run = world.resource_mut::<Run>();
            run.legendary.events.attacker = Some(targets[0]);
            assert_eq!(hero::hurt_hero_with_block(&mut run, 10.0, 0.1), 0.0);
        }
        world.run_system_once(tick_areas).unwrap();
        assert_eq!(world.resource::<Messages<Hit>>().len(), 1);
    }

    #[test]
    fn added_bombs_explode_without_reapplying_their_dot() {
        for key in ["living_bomb", "seed_of_corruption"] {
            let (mut world, targets) = legendary_world(key, 0, &[1.0, 2.0]);
            world.run_system_once(cast_abilities).unwrap();
            world.run_system_once(apply_hits).unwrap();
            world.resource_mut::<Messages<Hit>>().clear();
            if key == "living_bomb" {
                world.resource_mut::<Run>().elapsed = 7.0;
            } else {
                world.write_message(Hit::direct("shadow_bolt", targets[0], 200.0, false));
                world.run_system_once(apply_hits).unwrap();
                world.resource_mut::<Messages<Hit>>().clear();
            }
            let before = world.get::<Enemy>(targets[1]).unwrap().hp;
            world.run_system_once(tick_areas).unwrap();
            assert!(!world.resource::<Messages<Hit>>().is_empty(), "{key}");
            world.run_system_once(apply_hits).unwrap();
            let secondary = world.get::<Enemy>(targets[1]).unwrap();
            assert!(secondary.hp < before, "{key}");
            assert!(secondary.dots.is_empty(), "{key}");
        }
    }

    #[test]
    fn added_penance_heals_critical_health_and_efflorescence_stays_at_its_origin() {
        for key in ["penance", "efflorescence"] {
            let (mut world, _) = legendary_world(key, 0, &[1.0]);
            world.resource_mut::<Run>().hp = 10.0;
            world.run_system_once(cast_abilities).unwrap();
            if key == "penance" {
                assert!(world.resource::<Run>().hp > 10.0);
                assert!(world.resource::<Messages<Hit>>().is_empty());
            } else {
                world.resource_mut::<Run>().elapsed = 1.0;
                world.run_system_once(cast_abilities).unwrap();
                let hp = world.resource::<Run>().hp;
                assert!(hp > 10.0);
                let hero = world
                    .query_filtered::<Entity, With<Hero>>()
                    .single(&world)
                    .unwrap();
                world.get_mut::<Transform>(hero).unwrap().translation.x = 30.0;
                world.resource_mut::<Run>().elapsed = 2.0;
                world.run_system_once(cast_abilities).unwrap();
                assert_eq!(world.resource::<Run>().hp, hp);
            }
        }
    }

    #[test]
    fn proc_rules_only_original_skill_kills_trigger_death_explosions() {
        for proc_kill in [false, true] {
            let (mut world, targets) = legendary_world("fireball", 24, &[1.0]);
            {
                let mut en = world.get_mut::<Enemy>(targets[0]).unwrap();
                en.hp = 1.0;
                en.dots.push(enemies::Dot {
                    owner: None,
                    skill: "proc_ignite",
                    meter_source: None,
                    item_source: None,
                    origin: Origin::Talent,
                    tags: Some(engine::Tags::FIRE | engine::Tags::DOT),
                    meta: default(),
                    dps: 10.0,
                    left: 5.0,
                    tick: 5.0,
                });
            }
            let mut hit = Hit::direct("fireball", targets[0], 100.0, false);
            hit.cast_id = 100;
            if proc_kill {
                hit.origin = Origin::Talent;
                hit.triggered = true;
            }
            world.write_message(hit);
            world.run_system_once(apply_hits).unwrap();
            assert!(world.get::<Enemy>(targets[0]).unwrap().dead);
            assert_eq!(
                world.resource::<Run>().legendary.events.queue.is_empty(),
                proc_kill
            );
        }
    }

    #[test]
    fn proc_rules_periodic_area_keeps_all_victims_in_one_activation() {
        use super::super::items::legendary::periodic;
        let (mut world, _) = legendary_world("fireball", 69, &[1.0, 2.0, 3.0]);
        periodic::field(
            &mut world.resource_mut::<Run>(),
            69,
            Vec3::ZERO,
            120.0,
            1.0,
            engine::Tags::PHYSICAL,
            false,
        );
        world.resource_mut::<Run>().elapsed = 1.0;
        world.run_system_once(periodic::tick_world).unwrap();
        assert_eq!(world.resource::<Run>().legendary.events.queue.len(), 1);
        world.run_system_once(legendary_events::tick).unwrap();
        let hits: Vec<_> = world.resource_mut::<Messages<Hit>>().drain().collect();
        assert_eq!(hits.len(), 3);
        assert!(hits.iter().all(|hit| hit.damage == 40.0 && !hit.can_proc()));
    }

    #[test]
    fn proc_rules_thorns_are_non_recursive_and_share_a_recovery_delay() {
        let (mut world, targets) = legendary_world("thorns", 1, &[0.1, 0.2]);
        for target in targets {
            world
                .entity_mut(target)
                .insert(crate::creature_anim::MovementState::default());
        }
        benilla_world::world_point::init_world_point_resources(&mut world);
        {
            let mut run = world.resource_mut::<Run>();
            run.legendary.equipped.clear();
            run.thorns = 10.0;
            run.thorns_t = 5.0;
            run.thorns_spell = data::ability("thorns").spell;
        }
        world.run_system_once(enemies::move_enemies).unwrap();
        let hits: Vec<_> = world.resource_mut::<Messages<Hit>>().drain().collect();
        assert_eq!(hits.len(), 1);
        assert!(!hits[0].can_proc());
        assert_eq!(hits[0].origin, Origin::Talent);
    }

    #[test]
    fn proc_rules_overlapping_explosions_hit_each_enemy_once_per_parent() {
        let (mut world, targets) = legendary_world("fireball", 1, &[1.0, 2.0, 3.0]);
        world.resource_mut::<Run>().legendary.equipped.clear();
        let emit = |world: &mut World, cast: u64| {
            for _ in 0..5 {
                for &target in &targets {
                    let mut h = Hit::direct("fireball", target, 10.0, false);
                    h.origin = Origin::Legendary;
                    h.triggered = true;
                    h.proc_resolved = true;
                    h.item_source = Some(24);
                    h.cast_id = cast;
                    h.tags = Some(engine::Tags::SPELL | engine::Tags::FIRE | engine::Tags::AREA);
                    world.write_message(h);
                }
            }
        };
        emit(&mut world, 101);
        world.run_system_once(apply_hits).unwrap();
        let first = world.resource::<Run>().damage_done;
        assert!(first > 0.0);
        assert_eq!(world.resource::<Run>().meter.rows(0.0).len(), 1);
        for target in &targets {
            assert!(world.get::<Enemy>(*target).unwrap().hp >= 990.0);
        }
        world.resource_mut::<Messages<Hit>>().clear();
        world.resource_mut::<Run>().elapsed = 1.0;
        emit(&mut world, 101);
        world.run_system_once(apply_hits).unwrap();
        assert_eq!(world.resource::<Run>().damage_done, first);
        world.resource_mut::<Messages<Hit>>().clear();
        emit(&mut world, 102);
        world.run_system_once(apply_hits).unwrap();
        assert_eq!(world.resource::<Run>().damage_done, first * 2.0);
    }

    #[test]
    fn survival_traps_target_one_then_two_enemy_positions() {
        for skill in ["explosive_trap", "frost_trap"] {
            for (second, count) in [(false, 1), (true, 2)] {
                let (mut world, _) = legendary_world(skill, 0, &[8.0, 12.0, 30.0]);
                {
                    let mut run = world.resource_mut::<Run>();
                    run.class = &data::CLASSES[2];
                    run.talent_build[2][8] = 1;
                    if second {
                        run.talent_build[2][12] = 1;
                    }
                    super::super::talent_runtime::configure(&mut run);
                    run.abilities.retain(|a| a.def.key == skill);
                }
                world.run_system_once(cast_abilities).unwrap();
                let centers: Vec<_> = world
                    .query::<(&Transform, &GroundArea)>()
                    .iter(&world)
                    .map(|(tf, _)| tf.translation.x)
                    .collect();
                assert_eq!(centers.len(), count, "{skill}");
                assert!(centers.iter().all(|x| *x == 8.0 || *x == 12.0));
            }
        }
    }

    fn legendary_world(
        skill: &'static str,
        unique: u16,
        positions: &[f32],
    ) -> (World, Vec<Entity>) {
        let mut world = World::new();
        world.init_resource::<super::super::support_spells::Debuffs>();
        world.init_resource::<Time>();
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
        let mut run = Run {
            expeditions: super::super::expedition_config::Config::default().without_resistances(),
            balance: super::super::balance::Config::parse("version=1\n[classes]\n[tags]\n")
                .unwrap(),
            abilities: vec![Owned {
                def: data::ability(skill),
                level: 1,
                cd: 0.0,
            }],
            ..default()
        };
        run.legendary.equipped.insert(
            unique,
            super::super::items::legendary::Roll {
                unique,
                milestone: None,
            },
        );
        run.replace_spell_modifiers(
            42,
            [engine::Modifier {
                source: 42,
                filter: engine::Filter::default(),
                change: engine::Change::CriticalChance(-1.0),
            }],
        );
        world.insert_resource(run);
        world.spawn((Transform::default(), Hero { vy: 0.0, air: 0.0 }));
        let targets = positions
            .iter()
            .enumerate()
            .map(|(index, x)| {
                world
                    .spawn((
                        Guid(100 + index as u64),
                        Transform::from_xyz(*x, 0.0, 0.0),
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
                        ObjectStore(benilla_protocol::messages::ObjectFields::from_pairs(&[])),
                        Marks::debuffs(),
                    ))
                    .id()
            })
            .collect();
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(100));
        (world, targets)
    }

    #[test]
    fn ranged_spells_prefer_lowest_health_in_range_and_melee_keeps_nearest_target() {
        for key in [
            "frostbolt",
            "pyroblast",
            "chain_lightning",
            "shadow_word_pain",
            "heroic_strike",
        ] {
            let positions = if key == "heroic_strike" {
                [1.0, 3.0, 200.0]
            } else {
                [4.0, 6.0, 200.0]
            };
            let (mut world, targets) = legendary_world(key, 286, &positions);
            world.resource_mut::<Run>().legendary.equipped.clear();
            world.get_mut::<Enemy>(targets[1]).unwrap().hp = 50.0;
            world.get_mut::<Enemy>(targets[2]).unwrap().hp = 1.0;
            world.run_system_once(cast_abilities).unwrap();
            let gos = world
                .resource_mut::<Messages<SpellGoTargets>>()
                .drain()
                .collect::<Vec<_>>();
            if key == "heroic_strike" {
                let swing = world
                    .resource_mut::<Messages<SwingMessage>>()
                    .drain()
                    .next()
                    .unwrap();
                assert_eq!(swing.victim, Some(targets[0]));
            } else {
                assert_eq!(
                    gos.first().unwrap().hits.first(),
                    Some(&targets[1]),
                    "{key}"
                );
            }
        }
    }
    #[test]
    fn ranged_spells_prioritize_bosses_over_wounded_mobs_but_ignore_distant_bosses() {
        for key in [
            "frostbolt",
            "pyroblast",
            "chain_lightning",
            "shadow_word_pain",
        ] {
            let (mut world, targets) = legendary_world(key, 286, &[4.0, 6.0, 8.0, 200.0]);
            world.resource_mut::<Run>().legendary.equipped.clear();
            world.get_mut::<Enemy>(targets[1]).unwrap().hp = 1.0;
            world.get_mut::<Enemy>(targets[2]).unwrap().boss = true;
            {
                let mut distant = world.get_mut::<Enemy>(targets[3]).unwrap();
                distant.boss = true;
                distant.hp = 0.5;
            }
            world.run_system_once(cast_abilities).unwrap();
            let gos = world
                .resource_mut::<Messages<SpellGoTargets>>()
                .drain()
                .collect::<Vec<_>>();
            assert_eq!(
                gos.first().unwrap().hits.first(),
                Some(&targets[2]),
                "{key}"
            );
        }
    }
    #[test]
    fn legendary_attack_range_reaches_targets_outside_native_melee_and_projectile_range() {
        for key in ["heroic_strike", "arcane_shot", "serpent_sting"] {
            for bonus in [false, true] {
                let (mut world, targets) =
                    legendary_world(key, 286, &[data::ability(key).range + 1.5]);
                world.resource_mut::<Run>().legendary.equipped.clear();
                if bonus {
                    let p = data::PASSIVES
                        .iter()
                        .find(|p| p.key == "attack_range")
                        .unwrap();
                    let mut run = world.resource_mut::<Run>();
                    run.passives.push((p, 1));
                    super::super::passives::sync(&mut run);
                }
                world.run_system_once(cast_abilities).unwrap();
                let cast = if key == "heroic_strike" {
                    world
                        .resource_mut::<Messages<SwingMessage>>()
                        .drain()
                        .any(|s| s.victim == Some(targets[0]))
                } else {
                    world
                        .resource_mut::<Messages<SpellGoTargets>>()
                        .drain()
                        .any(|s| s.hits.contains(&targets[0]))
                };
                assert_eq!(cast, bonus, "{key}");
            }
        }
    }
    #[test]
    fn demoralizing_shout_ranks_apply_the_scaled_weakness_to_real_enemies() {
        for (rank, expected) in [(1, 0.5), (3, 0.45), (5, 0.35), (6, 0.25)] {
            let (mut world, targets) = legendary_world("demoralizing_shout", 286, &[1.0]);
            {
                let mut run = world.resource_mut::<Run>();
                run.legendary.equipped.clear();
                run.abilities[0].level = rank;
            }
            world.run_system_once(cast_abilities).unwrap();
            world.run_system_once(apply_hits).unwrap();
            let enemy = world.get::<Enemy>(targets[0]).unwrap();
            assert!((enemy.weaken - expected).abs() < 0.001, "rank {rank}");
            assert_eq!(
                enemy.weaken_t,
                5.0 + 0.25 * super::super::specialization::steps(rank)
            );
        }
    }
    #[test]
    fn healing_casts_above_eighty_five_percent_and_selects_allies_including_the_host() {
        use super::super::multiplayer::{Companion, Companions, HostPlayer};
        for guest in [false, true] {
            for skill in ["flash_heal", "rejuvenation"] {
                let (mut world, _) = legendary_world(skill, 286, &[]);
                {
                    let mut caster = world.resource_mut::<Run>();
                    caster.legendary.equipped.clear();
                    caster.class = &data::CLASSES[4];
                    caster.talent_build[0][0] = 1; // Discipline has no blessing exception.
                    caster.owner = guest.then_some(42);
                    caster.equipment.healing = 0.5;
                    caster.hp = caster.stats().max_hp;
                }
                let mut players = Companions::default();
                let mut chosen = Entity::PLACEHOLDER;
                for (id, distance, fraction, dead) in [
                    (10, 4.0, 0.9, false),
                    (11, 50.0, 0.1, false),
                    (12, 2.0, 0.0, true),
                    (13, 6.0, 0.95, false),
                ] {
                    let entity = world.spawn(Transform::from_xyz(distance, 0.0, 0.0)).id();
                    let mut run = Run {
                        owner: Some(id),
                        ..default()
                    };
                    run.equipment.healing = 0.8;
                    run.hp = run.stats().max_hp * fraction;
                    run.dead_t = dead.then_some(1.0);
                    if id == 10 {
                        chosen = entity;
                        if guest {
                            run.owner = None;
                            world.insert_resource(HostPlayer { entity, run });
                            continue;
                        }
                    }
                    players.0.insert(
                        id,
                        Companion {
                            entity,
                            run,
                            physics: Hero { vy: 0.0, air: 0.0 },
                            offer: default(),
                            offer_id: 0,
                        },
                    );
                }
                world.insert_resource(players);
                let power = world
                    .resource::<Run>()
                    .resolve_spell(data::ability(skill), 1);
                world.run_system_once(cast_abilities).unwrap();
                assert!(world.resource::<Run>().abilities[0].cd > 0.0);
                let go = world
                    .resource_mut::<Messages<SpellGoTargets>>()
                    .drain()
                    .next()
                    .unwrap();
                assert_eq!(go.hits, vec![chosen]);
                let target = if guest {
                    &world.resource::<HostPlayer>().run
                } else {
                    &world.resource::<Companions>().0[&10].run
                };
                if power.duration > 0.0 {
                    assert_eq!(world.resource::<Run>().heal_t, 0.0);
                    assert_eq!(target.heal_t, power.duration);
                    assert!(
                        (target.heal_rate * power.duration * 1.8 - power.dmg * 1.5).abs() < 0.001
                    );
                } else {
                    assert!(target.hp > target.stats().max_hp * 0.9);
                    assert!(
                        target.healing_meter.rows(0.0).is_empty(),
                        "the recipient is not the healer"
                    );
                    assert!(world.resource::<Run>().healing_meter.rows(0.0)[0].total > 0.0);
                }
            }
        }
    }
    #[test]
    fn reward_selection_stops_casts_hits_and_dots_for_host_and_guests() {
        use super::super::multiplayer::{Companion, Companions};
        for owner in [None, Some(42)] {
            let (mut world, targets) = legendary_world("frostbolt", 286, &[2.0]);
            world.resource_mut::<Run>().legendary.equipped.clear();
            world.resource_mut::<Run>().pending_levels = 1;
            world.run_system_once(cast_abilities).unwrap();
            assert!(world.resource::<Messages<SpellGoTargets>>().is_empty());
            assert!(world.resource::<Messages<Hit>>().is_empty());
            if let Some(id) = owner {
                let mut guest_run = world.remove_resource::<Run>().unwrap();
                guest_run.owner = owner;
                world.insert_resource(Run::default());
                let entity = world.spawn(Transform::default()).id();
                let mut players = Companions::default();
                players.0.insert(
                    id,
                    Companion {
                        entity,
                        run: guest_run,
                        physics: Hero { vy: 0.0, air: 0.0 },
                        offer: default(),
                        offer_id: 0,
                    },
                );
                world.insert_resource(players);
            }
            let target = targets[0];
            world
                .get_mut::<Enemy>(target)
                .unwrap()
                .dots
                .push(enemies::Dot {
                    owner,
                    skill: "fireball",
                    meter_source: None,
                    tags: Some(engine::Tags::FIRE | engine::Tags::DOT),
                    item_source: None,
                    origin: Origin::Skill,
                    meta: default(),
                    dps: 100.0,
                    left: 10.0,
                    tick: 0.0,
                });
            let mut hit = Hit::direct("fireball", target, 100.0, false);
            hit.owner = owner;
            world.resource_mut::<Messages<Hit>>().write(hit.clone());
            world.run_system_once(apply_hits).unwrap();
            assert_eq!(world.get::<Enemy>(target).unwrap().hp, 1000.0);
            assert!(world.get::<Enemy>(target).unwrap().dots[0].left < 10.0);
            if let Some(id) = owner {
                world
                    .resource_mut::<Companions>()
                    .0
                    .get_mut(&id)
                    .unwrap()
                    .run
                    .pending_levels = 0;
            } else {
                world.resource_mut::<Run>().pending_levels = 0;
            }
            world.resource_mut::<Messages<Hit>>().write(hit);
            world.run_system_once(apply_hits).unwrap();
            assert!(world.get::<Enemy>(target).unwrap().hp < 1000.0);
        }
    }
    #[test]
    fn periodic_floating_numbers_are_grouped_by_player_instead_of_combining_the_party() {
        use super::super::multiplayer::{Companion, Companions};
        let (mut world, targets) = legendary_world("fireball", 286, &[2.0]);
        world.resource_mut::<Run>().legendary.equipped.clear();
        world.init_resource::<Messages<units::CombatNumber>>();
        let entity = world.spawn(Transform::default()).id();
        let mut players = Companions::default();
        players.0.insert(
            42,
            Companion {
                entity,
                run: Run {
                    owner: Some(42),
                    ..default()
                },
                physics: Hero { vy: 0.0, air: 0.0 },
                offer: default(),
                offer_id: 0,
            },
        );
        world.insert_resource(players);
        for (owner, dps) in [(None, 10.0), (Some(42), 20.0)] {
            world
                .get_mut::<Enemy>(targets[0])
                .unwrap()
                .dots
                .push(enemies::Dot {
                    owner,
                    skill: "fireball",
                    meter_source: None,
                    tags: Some(engine::Tags::FIRE | engine::Tags::DOT),
                    item_source: None,
                    origin: Origin::Skill,
                    meta: default(),
                    dps,
                    left: 5.0,
                    tick: 0.0,
                });
        }
        world.run_system_once(apply_hits).unwrap();
        let numbers: Vec<_> = world
            .resource_mut::<Messages<units::CombatNumber>>()
            .drain()
            .collect();
        assert_eq!(numbers.len(), 2);
        assert_eq!(numbers[0].owner, None);
        assert_eq!(numbers[1].owner, Some(42));
        assert!((numbers[1].amount - numbers[0].amount * 2.0).abs() < 0.001);
        let total: f32 = numbers.iter().map(|n| n.amount).sum();
        assert!((1000.0 - world.get::<Enemy>(targets[0]).unwrap().hp - total).abs() < 0.001);
        let local: Vec<_> = world
            .resource_mut::<Messages<CombatTextSpawn>>()
            .drain()
            .collect();
        assert_eq!(local.len(), 1);
        assert_eq!(
            local[0].text,
            (numbers[0].amount.round() as i64).to_string()
        );
    }
    #[test]
    fn healer_blessings_cast_at_full_health_buff_allies_and_expire_without_recasting() {
        use super::super::{
            healer_buffs,
            multiplayer::{Companion, Companions, RemoteOwner},
            talent_runtime,
        };
        let (mut world, _) = legendary_world("flash_heal", 286, &[]);
        {
            let mut run = world.resource_mut::<Run>();
            run.class = &data::CLASSES[4];
            run.hp = run.stats().max_hp;
            run.legendary.equipped.clear();
        }
        let entity = world
            .spawn((RemoteOwner(42), Transform::from_xyz(5.0, 0.0, 0.0)))
            .id();
        let mut players = Companions::default();
        players.0.insert(
            42,
            Companion {
                entity,
                run: Run {
                    owner: Some(42),
                    ..default()
                },
                physics: Hero { vy: 0.0, air: 0.0 },
                offer: default(),
                offer_id: 0,
            },
        );
        world.insert_resource(players);
        world.run_system_once(cast_abilities).unwrap();
        assert!(world.resource::<Run>().abilities[0].cd > 0.0);
        world.run_system_once(talent_runtime::support).unwrap();
        assert!((healer_buffs::snapshot(world.resource::<Run>())[0] - 0.2).abs() < 0.0001);
        assert!(
            (healer_buffs::snapshot(&world.resource::<Companions>().0[&42].run)[0] - 0.2).abs()
                < 0.0001
        );
        world.resource_mut::<Run>().abilities[0].cd = 100.0;
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(9));
        world.run_system_once(talent_runtime::support).unwrap();
        assert_eq!(healer_buffs::snapshot(world.resource::<Run>())[0], 0.0);
        assert_eq!(
            healer_buffs::snapshot(&world.resource::<Companions>().0[&42].run)[0],
            0.0
        );
    }
    #[test]
    fn legendary_nova_selects_targets_around_a_distant_enemy() {
        let (mut world, _) = legendary_world("arcane_explosion", 6, &[15.0, 17.0, 30.0]);
        world.run_system_once(cast_abilities).unwrap();
        assert_eq!(world.resource::<Messages<Hit>>().len(), 2);
    }
    #[test]
    fn copied_projectiles_keep_their_native_splash_without_gaining_an_extra_area() {
        for (key, splash) in [("fireball", true), ("frostbolt", false)] {
            let (mut world, targets) = legendary_world(key, 286, &[2.0, 3.0]);
            {
                let mut run = world.resource_mut::<Run>();
                super::super::items::legendary::casting::copy(
                    &mut run,
                    286,
                    key,
                    Vec3::ZERO,
                    Some(targets[0]),
                    0.0,
                );
            }
            world
                .run_system_once(super::super::items::legendary::casting::tick)
                .unwrap();
            world.run_system_once(apply_hits).unwrap();
            assert!(world.get::<Enemy>(targets[0]).unwrap().hp < 1000.0);
            assert_eq!(
                world.get::<Enemy>(targets[1]).unwrap().hp < 1000.0,
                splash,
                "{key}"
            );
        }
    }
    #[test]
    fn splash_does_not_replace_the_projectiles_primary_target_for_legendary_counters() {
        let (mut world, targets) =
            legendary_world("fireball", 317, &[2.0, 2.5, 3.0, 3.5, 4.0, 4.5]);
        for (index, target) in targets.iter().skip(1).enumerate() {
            world.resource_mut::<Messages<Hit>>().clear();
            let mut hit = Hit::direct("fireball", *target, 1.0, false);
            hit.primary = true;
            hit.cast_id = index as u64 + 1;
            hit.splash = 10.0;
            world.resource_mut::<Messages<Hit>>().write(hit);
            world.run_system_once(apply_hits).unwrap();
        }
        assert_eq!(world.resource::<Run>().legendary.events.queue.len(), 1);
    }
    #[test]
    fn legendary_runtime_systems_execute_transformed_spells_without_query_conflicts() {
        use super::super::items::legendary::{casting, effects, events, periodic};
        for (id, key) in [
            (3, "arcane_explosion"),
            (13, "chain_lightning"),
            (17, "blizzard"),
            (42, "thunder_clap"),
            (43, "cleave"),
            (52, "consecration"),
            (57, "hellfire"),
            (70, "fireball"),
            (71, "holy_nova"),
            (98, "arcane_explosion"),
            (115, "fireball"),
            (124, "arcane_explosion"),
            (126, "cone_of_cold"),
            (167, "sinister_strike"),
            (169, "sinister_strike"),
            (194, "searing_totem"),
            (263, "fireball"),
            (264, "fireball"),
            (265, "corruption"),
            (303, "entangling_roots"),
            (306, "hurricane"),
            (309, "hellfire"),
            (310, "death_coil"),
            (311, "heroic_strike"),
        ] {
            let (mut world, targets) = legendary_world(key, id, &[2.0, 4.0, 6.0, 8.0]);
            let mut schedule = Schedule::default();
            schedule.add_systems(
                (
                    events::tick,
                    periodic::tick_world,
                    effects::tick,
                    cast_abilities,
                    tick_areas,
                    casting::tick,
                    apply_hits,
                )
                    .chain(),
            );
            for _ in 0..32 {
                world.resource_mut::<Run>().elapsed += 0.25;
                world
                    .resource_mut::<Time>()
                    .advance_by(std::time::Duration::from_millis(250));
                schedule.run(&mut world);
            }
            let run = world.resource::<Run>();
            assert!(run.damage_done.is_finite(), "unique {id}");
            assert!(
                targets
                    .iter()
                    .all(|e| world.get::<Enemy>(*e).unwrap().hp.is_finite()),
                "unique {id}"
            );
        }
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn arcane_talents_repeat_real_casts_once_without_resetting_cooldown() {
        use super::super::talents;
        let (mut world, _) = legendary_world("arcane_missiles", 6, &[3.0]);
        let mut build = [[0; talents::NODES]; 3];
        build[0][8] = 1;
        {
            let mut run = world.resource_mut::<Run>();
            run.class = &data::CLASSES[6];
            run.legendary.equipped.clear();
            talents::Account::from_shared(6, 0, build).apply(6, &mut run);
            run.replace_spell_modifiers(
                900,
                [engine::Modifier {
                    source: 900,
                    filter: engine::Filter::requiring(engine::Tags::SPELL),
                    change: engine::Change::RecastChance(0.5),
                }],
            );
        }
        world.run_system_once(cast_abilities).unwrap();
        let count =
            world.resource::<PendingHits>().0.len() + world.resource::<Messages<Hit>>().len();
        assert!(count > 0);
        assert_eq!(world.resource::<Run>().recasts.len(), 1);
        let cooldown = world.resource::<Run>().abilities[0].cd;
        world.resource_mut::<Run>().elapsed = 0.2;
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(200));
        world.run_system_once(cast_abilities).unwrap();
        assert_eq!(
            world.resource::<PendingHits>().0.len() + world.resource::<Messages<Hit>>().len(),
            count * 2
        );
        assert!(world.resource::<Run>().recasts.is_empty());
        assert!((world.resource::<Run>().abilities[0].cd - (cooldown - 0.2)).abs() < 0.001);
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn arcane_talents_four_planned_repeats_escalate_and_trigger_nonrecursive_learned_spells() {
        use super::super::talents;
        let (mut world, _) = legendary_world("arcane_missiles", 6, &[3.0]);
        let mut build = [[0; talents::NODES]; 3];
        for node in [8, 12, 18] {
            build[0][node] = 1;
        }
        {
            let mut run = world.resource_mut::<Run>();
            run.class = &data::CLASSES[6];
            run.legendary.equipped.clear();
            talents::Account::from_shared(6, 0, build).apply(6, &mut run);
            run.abilities.retain(|o| o.def.key == "arcane_missiles");
            run.abilities.push(Owned {
                def: data::ability("fireball"),
                level: 1,
                cd: 999.0,
            });
            run.replace_spell_modifiers(
                900,
                [engine::Modifier {
                    source: 900,
                    filter: engine::Filter::requiring(engine::Tags::SPELL),
                    change: engine::Change::RecastChance(3.5),
                }],
            );
        }
        fn drain(world: &mut World) -> Vec<Hit> {
            let mut result: Vec<_> = std::mem::take(&mut world.resource_mut::<PendingHits>().0)
                .into_iter()
                .map(|(_, h)| h)
                .collect();
            result.extend(world.resource_mut::<Messages<Hit>>().drain());
            result
        }
        world.run_system_once(cast_abilities).unwrap();
        let original = drain(&mut world);
        let base = original
            .iter()
            .find(|h| h.skill == "arcane_missiles")
            .unwrap()
            .damage;
        let extra_base = world
            .resource::<Run>()
            .resolve_spell(data::ability("fireball"), 1)
            .dmg;
        let cooldown = world.resource::<Run>().abilities[0].cd;
        let extra_cd = world.resource::<Run>().abilities[1].cd;
        for ordinal in 1..=4 {
            world.resource_mut::<Run>().elapsed = ordinal as f32 * 0.2;
            world
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_millis(200));
            world.run_system_once(cast_abilities).unwrap();
            let wave = drain(&mut world);
            let copies: Vec<_> = wave
                .iter()
                .filter(|h| h.skill == "arcane_missiles")
                .collect();
            let extras: Vec<_> = wave.iter().filter(|h| h.skill == "fireball").collect();
            let meter = serde_json::to_value(&world.resource::<Run>().meter).unwrap();
            assert_eq!(meter["entries"]["fireball"]["casts"], 1);
            assert_eq!(meter["entries"]["fireball"]["debug"]["extra_casts"], 1);
            assert_eq!(meter["entries"]["fireball"]["debug"]["casts"], 0);
            assert!(!copies.is_empty());
            assert_eq!(
                !extras.is_empty(),
                ordinal == 1,
                "companion proc ICD at repeat {ordinal}"
            );
            for hit in copies {
                assert!((hit.damage - base * 1.2_f32.powi(ordinal)).abs() < 0.001);
            }
            for hit in extras {
                assert!(!hit.can_proc());
                assert!((hit.damage - extra_base).abs() < 0.001);
            }
            assert_eq!(
                world.resource::<Run>().recasts.len(),
                usize::from(ordinal < 4)
            );
        }
        assert!((world.resource::<Run>().abilities[0].cd - (cooldown - 0.8)).abs() < 0.001);
        assert!((world.resource::<Run>().abilities[1].cd - (extra_cd - 0.8)).abs() < 0.001);
        world.resource_mut::<Run>().elapsed = 2.0;
        world.resource_mut::<Run>().abilities[0].cd = 0.0;
        world.run_system_once(cast_abilities).unwrap();
        drain(&mut world);
        world.resource_mut::<Run>().elapsed = 2.2;
        world.run_system_once(cast_abilities).unwrap();
        let next_wave = drain(&mut world);
        assert!(next_wave.iter().any(|h| h.skill == "arcane_missiles"));
        for hit in next_wave.iter().filter(|h| h.skill == "arcane_missiles") {
            assert!(
                (hit.damage - base * 1.2).abs() < 0.001,
                "New original cast did not reset the escalation"
            );
        }
    }

    #[test]
    fn legendary_chain_reselects_living_targets_without_resetting_cooldown() {
        let (mut world, targets) = legendary_world("chain_lightning", 11, &[3.0, 5.0, 8.0]);
        world.run_system_once(cast_abilities).unwrap();
        assert_eq!(world.resource::<Run>().recasts.len(), 1);
        let cooldown = world.resource::<Run>().abilities[0].cd;
        world.resource_mut::<Messages<Hit>>().clear();
        world.resource_mut::<PendingHits>().0.clear();
        world.resource_mut::<PendingHits>().1.clear();
        world.despawn(targets[0]);
        world.resource_mut::<Run>().elapsed = 0.2;
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(200));
        world.run_system_once(cast_abilities).unwrap();
        assert!(world.resource::<Run>().recasts.is_empty());
        assert!((world.resource::<Run>().abilities[0].cd - (cooldown - 0.2)).abs() < 0.001);
        let hits: Vec<_> = world.resource_mut::<Messages<Hit>>().drain().collect();
        assert!(!hits.is_empty());
        assert_eq!(hits[0].target, targets[1]);
    }

    #[test]
    fn legendary_fireball_echo_reevaluates_splash_and_cancels_on_dead_primary() {
        for life in [1000.0, 5.0] {
            let (mut world, targets) = legendary_world("fireball", 1, &[3.0, 4.0, 10.0]);
            world.get_mut::<Enemy>(targets[0]).unwrap().hp = life;
            let mut hit = Hit::direct("fireball", targets[0], 10.0, false);
            hit.splash = 3.5;
            world.write_message(hit);
            world.run_system_once(apply_hits).unwrap();
            assert_eq!(world.resource::<PendingHits>().0.len(), 1);
            assert_eq!(world.get::<Enemy>(targets[1]).unwrap().hp, 993.0);
            world.resource_mut::<Messages<Hit>>().clear();
            world
                .get_mut::<Transform>(targets[1])
                .unwrap()
                .translation
                .x = 10.0;
            world.resource_mut::<Run>().elapsed = 0.2;
            world.run_system_once(apply_hits).unwrap();
            assert!(world.resource::<PendingHits>().0.is_empty());
            assert_eq!(world.get::<Enemy>(targets[1]).unwrap().hp, 993.0);
            assert_eq!(world.get::<Enemy>(targets[2]).unwrap().hp, 1000.0);
            assert_eq!(
                world.get::<Enemy>(targets[0]).unwrap().hp,
                if life > 5.0 { 980.0 } else { 0.0 }
            );
        }
    }

    #[test]
    fn elemental_hits_and_dots_use_resistance_and_keep_final_expedition_reduction() {
        let (mut world, targets) = legendary_world("fireball", 6, &[3.0]);
        let config = include_str!("../../../../config/survivors-expeditions.toml");
        let mut config: toml::Value = toml::from_str(config).unwrap();
        for key in ["monster_resistance_per_level", "boss_resistance_per_level"] {
            config[key]["fire"] = toml::Value::Float(10.0);
        }
        config["zones"].as_array_mut().unwrap()[5]["damage_reduction"] = toml::Value::Float(0.5);
        {
            let mut run = world.resource_mut::<Run>();
            run.legendary.equipped.clear();
            run.expeditions =
                super::super::expedition_config::Config::parse(&toml::to_string(&config).unwrap())
                    .unwrap();
            run.expedition_map = data::CAMPAIGN[5];
        }
        let level = world.get::<Enemy>(targets[0]).unwrap().def.level;
        let mut hit = Hit::direct("fireball", targets[0], 100.0, false);
        hit.tags = Some(engine::Tags::FIRE | engine::Tags::DAMAGE);
        world.write_message(hit);
        world
            .get_mut::<Enemy>(targets[0])
            .unwrap()
            .dots
            .push(enemies::Dot {
                item_source: None,
                meta: Default::default(),
                origin: Origin::Skill,
                owner: None,
                skill: "fireball",
                meter_source: None,
                tags: Some(engine::Tags::FIRE | engine::Tags::DOT | engine::Tags::DAMAGE),
                dps: 100.0,
                left: 5.0,
                tick: 0.0,
            });
        world.run_system_once(apply_hits).unwrap();
        let expected = 200.0 * (1.0 - (level as f32 * 10.0).min(75.0) / 100.0) * 0.5;
        let actual = 1000.0 - world.get::<Enemy>(targets[0]).unwrap().hp;
        assert!((actual - expected).abs() < 0.001, "{actual} != {expected}");
    }

    #[test]
    fn elites_reduce_direct_and_periodic_damage_after_zone_and_elemental_defenses() {
        let (mut world, targets) = legendary_world("fireball", 6, &[3.0]);
        let config = include_str!("../../../../config/survivors-expeditions.toml");
        let mut config: toml::Value = toml::from_str(config).unwrap();
        for key in ["monster_resistance_per_level", "boss_resistance_per_level"] {
            config[key]["fire"] = toml::Value::Float(10.0);
        }
        config["zones"].as_array_mut().unwrap()[5]["damage_reduction"] = toml::Value::Float(0.5);
        {
            let mut run = world.resource_mut::<Run>();
            run.legendary.equipped.clear();
            run.expeditions =
                super::super::expedition_config::Config::parse(&toml::to_string(&config).unwrap())
                    .unwrap();
            run.expedition_map = data::CAMPAIGN[5];
        }
        world.entity_mut(targets[0]).insert(enemies::Rank::Elite);
        let level = world.get::<Enemy>(targets[0]).unwrap().def.level;
        let mut hit = Hit::direct("fireball", targets[0], 100.0, false);
        hit.tags = Some(engine::Tags::FIRE | engine::Tags::DAMAGE);
        world.write_message(hit);
        world
            .get_mut::<Enemy>(targets[0])
            .unwrap()
            .dots
            .push(enemies::Dot {
                item_source: None,
                meta: Default::default(),
                origin: Origin::Skill,
                owner: None,
                skill: "fireball",
                meter_source: None,
                tags: Some(engine::Tags::FIRE | engine::Tags::DOT | engine::Tags::DAMAGE),
                dps: 100.0,
                left: 5.0,
                tick: 0.0,
            });
        world.run_system_once(apply_hits).unwrap();
        let expected = 200.0 * (1.0 - (level as f32 * 10.0).min(75.0) / 100.0) * 0.5 * 0.8;
        let actual = 1000.0 - world.get::<Enemy>(targets[0]).unwrap().hp;
        assert!((actual - expected).abs() < 0.001, "{actual} != {expected}");
    }

    #[test]
    fn protection_reduces_direct_and_periodic_damage_multiplicatively_with_elite_defenses() {
        let (mut world, targets) = legendary_world("fireball", 6, &[3.0]);
        let config = include_str!("../../../../config/survivors-expeditions.toml");
        let mut config: toml::Value = toml::from_str(config).unwrap();
        for key in ["monster_resistance_per_level", "boss_resistance_per_level"] {
            config[key]["fire"] = toml::Value::Float(10.0);
        }
        config["zones"].as_array_mut().unwrap()[5]["damage_reduction"] = toml::Value::Float(0.5);
        {
            let mut run = world.resource_mut::<Run>();
            run.legendary.equipped.clear();
            run.expeditions =
                super::super::expedition_config::Config::parse(&toml::to_string(&config).unwrap())
                    .unwrap();
            run.expedition_map = data::CAMPAIGN[5];
        }
        world.entity_mut(targets[0]).insert((
            enemies::Rank::Elite,
            super::super::special_mobs::Protection(0.25),
        ));
        let level = world.get::<Enemy>(targets[0]).unwrap().def.level;
        let mut hit = Hit::direct("fireball", targets[0], 100.0, false);
        hit.tags = Some(engine::Tags::FIRE | engine::Tags::DAMAGE);
        world.write_message(hit);
        world
            .get_mut::<Enemy>(targets[0])
            .unwrap()
            .dots
            .push(enemies::Dot {
                item_source: None,
                meta: Default::default(),
                origin: Origin::Skill,
                owner: None,
                skill: "fireball",
                meter_source: None,
                tags: Some(engine::Tags::FIRE | engine::Tags::DOT | engine::Tags::DAMAGE),
                dps: 100.0,
                left: 5.0,
                tick: 0.0,
            });
        world.run_system_once(apply_hits).unwrap();
        let expected = 200.0 * (1.0 - (level as f32 * 10.0).min(75.0) / 100.0) * 0.5 * 0.8 * 0.75;
        let actual = 1000.0 - world.get::<Enemy>(targets[0]).unwrap().hp;
        assert!((actual - expected).abs() < 0.001, "{actual} != {expected}");
    }

    #[test]
    fn legendary_projectile_keeps_its_physical_school_after_unequipping_conversion() {
        let (mut world, targets) = legendary_world("fireball", 96, &[3.0]);
        world.run_system_once(cast_abilities).unwrap();
        world.resource_mut::<Run>().legendary.equipped.clear();
        world.run_system_once(apply_hits).unwrap();
        let enemy = world.get::<Enemy>(targets[0]).unwrap();
        let run = world.resource::<Run>();
        let (armour, reduction) =
            run.expeditions
                .defenses(run.expedition_map, enemy.def.level, enemy.boss, false);
        let expected = 1000.0
            - data::ability("fireball").damage
                * (1.0 - super::super::items::armour::reduction(armour))
                * (1.0 - reduction);
        assert!((enemy.hp - expected).abs() < 0.001);
    }
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
        world.resource_mut::<Run>().expeditions =
            super::super::expedition_config::Config::default().without_resistances();
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
        // Milestone wolves hit three nearby enemies without extra-target cards.
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
                tags: None,
                item_source: None,
                origin: Origin::Skill,
                meta: Default::default(),
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
        let enemy = world.get::<Enemy>(targets[1]).unwrap();
        let run = world.resource::<Run>();
        let (armour, reduction) =
            run.expeditions
                .defenses(run.expedition_map, enemy.def.level, enemy.boss, false);
        let expected =
            600.0 * (1.0 - super::super::items::armour::reduction(armour)) * (1.0 - reduction);
        assert!(
            (dealt - expected).abs() < 0.001,
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
        world.resource_mut::<Run>().expeditions =
            super::super::expedition_config::Config::default().without_resistances();
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
                        tags: None,
                        owner: None,
                        skill: "corruption",
                        item_source: None,
                        origin: Origin::Skill,
                        meta: Default::default(),
                        dps: 30.0,
                        left: 5.0,
                        tick: 0.0,
                    },
                    enemies::Dot {
                        meter_source: None,
                        tags: None,
                        owner: None,
                        skill: "serpent_sting",
                        item_source: None,
                        origin: Origin::Skill,
                        meta: Default::default(),
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
        world.insert_resource(Run {
            expeditions: super::super::expedition_config::Config::default().without_resistances(),
            ..default()
        });
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
                tags: None,
                item_source: None,
                origin: Origin::Skill,
                meta: Default::default(),
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
        // Ignite accepts original Fire damage, with an ICD, but never a hidden Fire proc.
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
        for target in &fire_targets[..1] {
            let dots = &world.get::<Enemy>(*target).unwrap().dots;
            assert_eq!(dots.len(), 1);
            assert_eq!(dots[0].skill, "proc_ignite");
            assert_eq!(dots[0].meter_source, Some("Ignite"));
            assert!(dots[0].dps > 0.0);
        }
        for target in &fire_targets[1..] {
            assert!(world.get::<Enemy>(*target).unwrap().dots.is_empty());
        }
        // Immortality is not a damage cap: direct hits and DoTs still count in full.
        world.resource_mut::<Messages<Hit>>().clear();
        let mut dummy = enemy(
            100.0,
            vec![enemies::Dot {
                owner: None,
                skill: "proc_bleed",
                meter_source: Some("Deep Wounds"),
                tags: None,
                item_source: None,
                origin: Origin::Skill,
                meta: Default::default(),
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
    fn a_disabled_spell_does_not_cast_until_it_is_enabled_again() {
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
        run.disabled_spells.insert("fireball");
        world.insert_resource(run);
        world.spawn((Transform::default(), Hero { vy: 0.0, air: 0.0 }));
        world.spawn((
            Transform::from_xyz(1.0, 0.0, 0.0),
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
        let cast = |world: &mut World| {
            world
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_millis(100));
            world.run_system_once(cast_abilities).unwrap();
            world.resource::<Messages<CastEvent>>().len()
        };
        assert_eq!(cast(&mut world), 0);
        world.resource_mut::<Run>().disabled_spells.clear();
        assert!(cast(&mut world) > 0);
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
            tags: None,
            triggered: false,
            proc_resolved: false,
            proc_admitted: false,
            direct_dot: None,
            dot_tags: None,
            proc_chain: 0,
            item_source: None,
            origin: Origin::Skill,
            cast_id: 0,
            primary: false,
            target,
            damage,
            dot: 0.0,
            dot_dur: 0.0,
            effect: Effect::None,
            mark: 0,
            mark_dur: 0.0,
            splash: 0.0,
            crit,
        }
    }
}

/// Hits in flight, by the run time they land.
#[derive(Resource, Default)]
pub(super) struct PendingHits(pub Vec<(f32, Hit)>, pub Vec<(f32, Entity, Entity, u32)>);

/// A ground spell's area, ticking on whatever stands in it.
#[derive(Component)]
pub(super) struct GroundArea {
    triggered: bool,
    direct_dot: Option<engine::PeriodicSnapshot>,
    cast_id: u64,
    held: Option<f32>,
    root_once: bool,
    trail: bool,
    tags: engine::Tags,
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
    spawned_at: f32,
    projectiles: Params,
    effect: Effect,
    triggered: bool,
    cast_id: u64,
    direct_dot: Option<engine::PeriodicSnapshot>,
    tags: engine::Tags,
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

fn recall_totems(
    run: &mut Run,
    position: Vec3,
    totems: &Query<(Entity, &Transform, &Totem)>,
    summons: &Query<(Entity, &Transform, &super::talent_runtime::Summon)>,
    commands: &mut Commands,
) -> bool {
    let mut owned: Vec<_> = totems
        .iter()
        .filter(|(_, _, t)| t.owner == run.owner && t.left > 0.0)
        .map(|(e, tf, _)| (e, tf.translation))
        .collect();
    owned.extend(
        summons
            .iter()
            .filter(|(_, _, s)| s.owned_totem(run.owner))
            .map(|(e, tf, _)| (e, tf.translation)),
    );
    owned.extend(
        run.support
            .zones
            .iter()
            .filter(|z| {
                z.until > run.elapsed
                    && matches!(z.key, "strength_of_earth_totem" | "healing_stream_totem")
            })
            .map(|z| (z.entity, z.pos)),
    );
    if !owned
        .iter()
        .any(|(_, pos)| flat(*pos - position).length() > 4.0)
    {
        return false;
    }
    owned.sort_by_key(|(e, _)| e.to_bits());
    for (index, (entity, _)) in owned.into_iter().enumerate() {
        let angle = index as f32 * std::f32::consts::FRAC_PI_2;
        let radius = 2.0 + (index / 4) as f32;
        let destination = position + Vec3::new(angle.cos(), 0.0, angle.sin()) * radius;
        if let Some(zone) = run.support.zones.iter_mut().find(|z| z.entity == entity) {
            zone.pos = destination;
        }
        commands.queue(move |world: &mut World| {
            if let Some(mut tf) = world.get_mut::<Transform>(entity) {
                tf.translation = destination;
            }
        });
    }
    true
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
        tags: Some(p.tags),
        triggered: p.triggered,
        proc_resolved: false,
        proc_admitted: p.triggered,
        direct_dot: p.direct_dot,
        dot_tags: p.dot_tags,
        proc_chain: 0,
        item_source: None,
        origin: if p.triggered {
            Origin::Talent
        } else {
            Origin::Skill
        },
        cast_id: p.cast_id,
        primary: false,
        target,
        damage,
        dot: p.dot,
        dot_dur: p.duration,
        effect: p.effect,
        mark: def.mark,
        mark_dur: engine::mark_duration(p.effect, p.duration),
        splash: if matches!(def.kind, Kind::Bolt | Kind::Strike)
            || def.key == "hammer_of_the_righteous"
        {
            p.radius
        } else {
            0.0
        },
        crit,
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

/// The client spell that draws `skill`'s impact: a hidden proc's, else the ability's own.
fn visual_spell(skill: &str) -> Option<u32> {
    super::procs::find(skill)
        .map(|proc| proc.def.spell)
        .or_else(|| {
            super::data::ABILITIES
                .iter()
                .find(|def| def.key == skill)
                .map(|def| def.spell)
        })
        .filter(|&spell| spell != 0)
}

#[cfg(test)]
mod echo_visual_tests {
    use super::*;

    #[test]
    fn echoes_find_the_impact_of_abilities_and_hidden_procs() {
        assert_eq!(visual_spell("fireball"), Some(133));
        let bleed = super::super::procs::find("proc_bleed").unwrap();
        assert_eq!(
            visual_spell("proc_bleed"),
            Some(bleed.def.spell).filter(|&s| s != 0)
        );
        assert_eq!(visual_spell("no_such_skill"), None);
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
    for (index, link) in links(roots, foes, p, speed, native_chain)
        .into_iter()
        .enumerate()
    {
        let (damage, crit) = roll(
            rng,
            if index == 0 && p.primary_critical {
                1.0
            } else {
                p.crit
            },
            p.crit_multiplier,
            p.dmg * link.scale,
        );
        let mut hit = hit_for(def, p, link.target.e, damage, crit);
        hit.primary = index == 0;
        hit.dot *= link.scale;
        if let Some(dot) = hit.direct_dot.as_mut() {
            dot.dps *= link.scale;
        }
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
    mut healing: super::healing::PartyHealing,
    mut debuffs: Option<ResMut<super::support_spells::Debuffs>>,
    totems: Query<(Entity, &Transform, &Totem)>,
    summons: Query<(Entity, &Transform, &super::talent_runtime::Summon)>,
) {
    let dt = time.delta_secs();
    if run.pending_levels > 0 || dt <= 0.0 {
        return;
    }
    let Ok((me, htf)) = hero.single() else { return };
    let hpos = htf.translation;
    healing.process_support(&mut run, me, &mut commands);
    if run.dead_t.is_some() {
        return;
    }
    let now = run.elapsed;
    if let Some(debuffs) = debuffs.as_deref_mut() {
        debuffs.prune(now);
    }
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
    let mut blooms = std::mem::take(&mut run.extra_spells.blooms);
    for bloom in &mut blooms {
        while bloom.left > 0 && bloom.next <= now {
            healing.area(&mut run, me, bloom.center, bloom.radius, bloom.amount);
            bloom.left -= 1;
            bloom.next += 1.0;
        }
        if bloom.left == 0 {
            commands.entity(bloom.entity).despawn();
        }
    }
    blooms.retain(|b| b.left > 0);
    run.extra_spells.blooms = blooms;
    let speed_of = |spell: u32| {
        spells
            .as_deref()
            .and_then(|s| s.catalog.get(spell))
            .map_or(0.0, |d| d.speed)
    };

    let mut totem_slots = super::totem_limits::Slots::default();
    let mut active_totems: Vec<_> = totems
        .iter()
        .filter(|(_, _, t)| t.owner == run.owner && t.left > 0.0)
        .map(|(e, _, t)| (t.skill, e, t.spawned_at))
        .collect();
    active_totems.sort_by(|a, b| a.2.total_cmp(&b.2));
    for (key, entity, _) in active_totems {
        totem_slots.track(key, entity);
    }
    for zone in &run.support.zones {
        if zone.until > now && data::ability(zone.key).totem.0 != 0 {
            totem_slots.track(zone.key, zone.entity);
        }
    }
    let mut casts = std::collections::VecDeque::new();
    for i in 0..run.abilities.len() {
        if run.disabled_spells.contains(run.abilities[i].def.key) {
            continue;
        }
        run.abilities[i].cd -= dt;
        let ordinal = run.recasts.take_due_ordinal(i, now);
        if run.abilities[i].cd <= 0.0 || ordinal.is_some() {
            casts.push_back((i, ordinal, false));
        }
    }
    while let Some((i, ordinal, extra)) = casts.pop_front() {
        let repeated = ordinal.is_some() || extra;
        let o = &run.abilities[i];
        let original_def = o.def;
        let skill_level = o.level;
        if !super::support_spells::cast_ready(&run, original_def.key) {
            continue;
        }
        if original_def.key == "revenge" && !repeated && run.extra_spells.revenge_until <= now {
            continue;
        }
        let profile = super::items::legendary::profile(&run, original_def);
        let def = &profile.0;
        if run.legendary.has(308) && def.key == "shadow_bolt" {
            let oldest = enemies
                .iter()
                .flat_map(|(e, _, en)| {
                    en.dots
                        .iter()
                        .filter(|d| d.owner == run.owner && d.skill == "curse_of_agony")
                        .map(move |d| (e, d.meta.started))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|p| p.0);
            foes.sort_by(|a, b| {
                (Some(a.e) != oldest)
                    .cmp(&(Some(b.e) != oldest))
                    .then(a.dist.total_cmp(&b.dist))
            });
        } else if run.legendary.has(306) && def.key == "hurricane" {
            foes.sort_by(|a, b| {
                let rooted = |e| {
                    enemies.get(e).is_ok_and(|(_, _, en)| {
                        en.root_t > 0.0
                            && en
                                .dots
                                .iter()
                                .any(|d| d.owner == run.owner && d.skill == "entangling_roots")
                    })
                };
                rooted(b.e)
                    .cmp(&rooted(a.e))
                    .then(a.dist.total_cmp(&b.dist))
            });
        } else if matches!(
            def.kind,
            Kind::Bolt | Kind::Strike | Kind::Chain | Kind::Dot
        ) {
            foes.sort_by(|a, b| a.hp.total_cmp(&b.hp).then(a.dist.total_cmp(&b.dist)));
        } else {
            foes.sort_by(|a, b| a.dist.total_cmp(&b.dist));
        }
        foes.sort_by_key(|foe| !engine::targeting::threatens_owner(foe.dist, foe.radius));
        let mut p = run.resolve_spell_for_cast(original_def, o.level, &mut rng);
        if def.key == "mongoose_bite" && run.extra_spells.dodge_until > now {
            p.dmg *= 1.75;
        }
        p.triggered = extra;
        if extra && !super::procs::ready(&mut run, "proc_arcane_companion", p.tags) {
            continue;
        }
        if let Some(ordinal) = ordinal {
            super::arcane_repeats::empower(&run, &mut p, ordinal);
        }
        p.cast_id = if repeated {
            0
        } else {
            run.legendary.events.serial.wrapping_add(1).max(1)
        };
        if extra {
            p.cast_id = super::procs::parent(&mut run, 0);
        }
        let mut primary = None;
        if run.legendary.has(11) && def.key == "chain_lightning" {
            p.recast += 1.0;
        }
        let speed = speed_of(def.spell);
        let area_scale = if def.radius > 0.0 {
            (p.radius / def.radius).clamp(0.25, 8.0)
        } else {
            1.0
        };
        fx.area_scale(me, def.spell, area_scale);
        if repeated {
            fx.echo(me, def.spell);
        }
        let intercepted =
            super::items::legendary::casting::intercept(&mut run, def, p, &foes, hpos, repeated);
        let minion_ready = match def.key {
            "stomp" | "kill_command" => summons.iter().any(|(_, tf, pet)| {
                pet.owned_pet(run.owner)
                    && foes.iter().any(|foe| {
                        flat(foe.pos - tf.translation).length()
                            <= if def.key == "stomp" {
                                p.radius
                            } else {
                                p.range
                            }
                    })
            }),
            "bestial_wrath" => summons.iter().any(|(_, _, pet)| pet.owned_pet(run.owner)),
            _ => !foes.is_empty(),
        };
        let fired = if let Some(fired) = super::minions::cast(&mut run, def, p, minion_ready) {
            if fired && def.key == "tame_beast" {
                fx.cast(me, def.spell, &[me], None);
            }
            if fired && def.key == "bestial_wrath" {
                for (entity, _, pet) in &summons {
                    if pet.owned_pet(run.owner) {
                        fx.cast(entity, def.spell, &[entity], None);
                    }
                }
            }
            fired
        } else if let Some((fired, target)) = intercepted {
            primary = target;
            fired
        } else {
            match def.kind {
                Kind::Buff => {
                    let power = super::support_spells::effective_magnitude(
                        def.key,
                        skill_level,
                        p.buff_effect,
                    );
                    if def.key == "totemic_recall" {
                        let moved = recall_totems(&mut run, hpos, &totems, &summons, &mut commands);
                        if moved {
                            fx.cast(me, def.spell, &[me], Some(hpos));
                        }
                        moved
                    } else if matches!(def.key, "hunters_mark" | "curse_of_elements") {
                        if let Some(target) = foes
                            .iter()
                            .filter(|f| f.dist <= p.range)
                            .max_by(|a, b| a.hp.total_cmp(&b.hp))
                        {
                            if let Some(state) = debuffs.as_deref_mut() {
                                let boss = enemies.get(target.e).is_ok_and(|(_, _, en)| en.boss);
                                state.apply_from(
                                    target.e,
                                    def.key,
                                    power * if boss { 0.5 } else { 1.0 },
                                    now,
                                    p.duration,
                                    run.owner,
                                );
                            }
                            primary = Some((target.e, target.pos));
                            fx.cast(me, def.spell, &[target.e], None);
                            true
                        } else {
                            false
                        }
                    } else if def.key == "prayer_of_mending" {
                        let target = healing.cast(&mut run, me, def, power, p.duration, p.range);
                        fx.cast(me, def.spell, &[target], None);
                        true
                    } else if matches!(
                        def.key,
                        "smoke_bomb" | "strength_of_earth_totem" | "healing_stream_totem"
                    ) {
                        let serial = run.guid();
                        let entity = if def.totem.0 != 0 {
                            units::spawn_unit(
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
                                hpos,
                                0.0,
                            )
                        } else {
                            units::spawn_area_object(
                                &mut commands,
                                serial,
                                def.spell,
                                p.radius,
                                area_scale,
                                hpos,
                            )
                        };
                        if let Some(owner) = run.owner {
                            commands
                                .entity(entity)
                                .insert(super::multiplayer::OwnedEffect(owner));
                        }
                        if p.max_totems > 0 {
                            for retired in totem_slots.insert(def.key, entity, p.max_totems) {
                                run.support.zones.retain(|z| z.entity != retired);
                                commands.entity(retired).despawn();
                            }
                        }
                        run.support.zones.push(super::support_spells::Zone {
                            entity,
                            key: def.key,
                            pos: hpos,
                            radius: p.radius,
                            until: now + p.duration,
                            next: now,
                            power,
                        });
                        fx.cast(me, def.spell, &[me], Some(hpos));
                        true
                    } else {
                        if matches!(
                            def.key,
                            "savage_roar" | "slice_and_dice" | "barkskin" | "demon_armor"
                        ) {
                            let owner = run.owner;
                            super::support_spells::grant(
                                &mut run, def.key, power, p.duration, owner,
                            );
                        } else {
                            healing.buff(&mut run, me, hpos, p.radius, def.key, power, p.duration);
                        }
                        fx.cast(me, def.spell, &[me], None);
                        true
                    }
                }
                Kind::Bolt | Kind::Strike | Kind::Melee => {
                    if def.key == "penance" && healing.urgent(&run, me, p.range) {
                        let target = healing.cast(&mut run, me, def, p.dmg * 3.0, 0.0, p.range);
                        fx.cast(me, def.spell, &[target], None);
                        true
                    } else {
                        let reach = if def.kind == Kind::Melee {
                            p.range + 0.5
                        } else {
                            p.range
                        };
                        let targets = if def.kind == Kind::Melee {
                            nearest(&foes, reach, p.count)
                        } else {
                            weakest(&foes, reach, p.count, |e| {
                                enemies.get(e).is_ok_and(|(_, _, en)| en.boss)
                            })
                        };
                        if targets.is_empty() {
                            false
                        } else {
                            primary = Some((targets[0].e, targets[0].pos));
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
                            if def.key == "penance" {
                                for ordinal in 1..=2 {
                                    for target in &targets {
                                        let (damage, crit) =
                                            roll(&mut rng, p.crit, p.crit_multiplier, p.dmg);
                                        schedule(
                                            &mut pending,
                                            &mut hits,
                                            now,
                                            ordinal as f32 * 0.3,
                                            hit_for(def, &p, target.e, damage, crit),
                                        );
                                    }
                                }
                            }
                            true
                        }
                    }
                }
                Kind::Nova | Kind::Cone => {
                    let center = if def.kind == Kind::Cone && run.legendary.has(198) {
                        Some(
                            super::items::legendary::effects::pack_center(&mut run).unwrap_or(hpos),
                        )
                    } else if let Some(mid) = super::items::legendary::effects::area_center(&run) {
                        Some(mid)
                    } else if run.legendary.has(6) && def.kind == Kind::Nova {
                        foes.first().filter(|f| f.dist <= 24.0).map(|f| f.pos)
                    } else {
                        Some(hpos)
                    };
                    let mut targets =
                        center.map_or_else(Vec::new, |center| around(&foes, center, p.radius));
                    if def.kind == Kind::Cone {
                        if let Some(first) = targets.first().copied() {
                            let aim = if run.legendary.has(257)
                                && run.legendary.casting.last_heading.length_squared() > 0.0
                            {
                                run.legendary.casting.last_heading
                            } else {
                                flat(first.pos - hpos).normalize_or_zero()
                            };
                            targets.retain(|f| {
                                let d = flat(f.pos - center.unwrap_or(hpos));
                                d.length() < 2.0 || d.normalize_or_zero().dot(aim) >= 0.8
                            });
                        }
                    }
                    if targets.is_empty() {
                        false
                    } else {
                        let main = targets.iter().max_by(|a, b| a.hp.total_cmp(&b.hp)).unwrap();
                        primary = Some((main.e, main.pos));
                        fx.cast(me, def.spell, &entities(&targets), None);
                        for f in &targets {
                            let (dmg, crit) = roll(&mut rng, p.crit, p.crit_multiplier, p.dmg);
                            let delay = if speed > 0.0 { f.dist / speed } else { 0.0 };
                            let mut hit = hit_for(def, &p, f.e, dmg, crit);
                            hit.primary = Some(f.e) == primary.map(|p| p.0);
                            schedule(&mut pending, &mut hits, now, delay, hit);
                        }
                        true
                    }
                }
                Kind::Ground | Kind::GroundSelf => {
                    let targeted = p.tags.contains(engine::Tags::TRAP)
                        && (super::talent_runtime::active(&run, super::talent_design::Mechanic::TrapLaunch)
                            || super::talent_runtime::active(&run, super::talent_design::Mechanic::TrapVolley));
                    let centers: Vec<Vec3> = if targeted {
                        let count = if super::talent_runtime::active(
                            &run,
                            super::talent_design::Mechanic::TrapVolley,
                        ) {
                            2
                        } else {
                            1
                        };
                        weakest(&foes, p.range.max(14.0), count, |e| {
                            enemies.get(e).is_ok_and(|(_, _, en)| en.boss)
                        })
                        .into_iter()
                        .map(|f| f.pos)
                        .collect()
                    } else {
                        let center = if def.kind == Kind::GroundSelf {
                            foes.first()
                                .filter(|f| f.dist <= p.radius + 4.0)
                                .map(|_| hpos)
                        } else {
                            densest(&foes, p.range, p.radius)
                        };
                        center.into_iter().collect()
                    };
                    let cast = !centers.is_empty();
                    for center in centers {
                        {
                            run.legendary
                                .casting
                                .ground
                                .push((center, now + p.duration));
                            if def.key == "rain_of_fire" {
                                run.legendary.casting.rains.push((center, now + p.duration));
                            }
                            primary = around(&foes, center, p.radius)
                                .into_iter()
                                .max_by(|a, b| a.hp.total_cmp(&b.hp))
                                .map(|f| (f.e, f.pos));
                            let trap = def.key == "explosive_trap";
                            fx.cast(me, if trap { 13813 } else { def.spell }, &[], Some(center));
                            let area = if trap {
                                units::spawn_explosive_trap(&mut commands, run.guid(), center)
                            } else if def.key == "frost_trap" {
                                units::spawn_frost_trap(
                                    &mut commands,
                                    run.guid(),
                                    p.radius,
                                    area_scale,
                                    center,
                                )
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
                                fx.area_scale(area, def.spell, area_scale);
                            }
                            let burst = def.duration <= 0.0;
                            commands.entity(area).insert(GroundArea {
                                triggered: p.triggered,
                                direct_dot: p.direct_dot,
                                cast_id: p.cast_id,
                                held: if run.legendary.has(17)
                                    && p.tags.contains(engine::Tags::COLD)
                                    && def.duration > 0.0
                                {
                                    Some(
                                        p.dmg
                                            * if p.duration >= 0.4 {
                                                (p.duration - 0.4).floor() + 1.0
                                            } else {
                                                0.0
                                            },
                                    )
                                } else {
                                    None
                                },
                                root_once: run.legendary.has(303) && def.key == "entangling_roots",
                                trail: run.legendary.has(52) && def.key == "consecration",
                                tags: p.tags,
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
                                effect: p.effect,
                                mark: def.mark,
                                impact: if def.overhead || trap { def.spell } else { 0 },
                            });
                            if let Some(owner) = run.owner {
                                commands
                                    .entity(area)
                                    .insert(super::multiplayer::OwnedEffect(owner));
                            }
                        }
                    }
                    cast
                }
                Kind::Chain => {
                    let targets = weakest(&foes, p.range, 1, |e| {
                        enemies.get(e).is_ok_and(|(_, _, en)| en.boss)
                    });
                    if targets.is_empty() {
                        false
                    } else {
                        primary = Some((targets[0].e, targets[0].pos));
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
                    let targets = weakest(&foes, p.range, p.count, |e| {
                        enemies.get(e).is_ok_and(|(_, _, en)| en.boss)
                    });
                    if targets.is_empty() {
                        false
                    } else {
                        primary = Some((targets[0].e, targets[0].pos));
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
                    run.legendary
                        .casting
                        .rains
                        .retain(|(_, until)| *until > now);
                    let mut center = if run.legendary.has(309) && def.key == "hellfire" {
                        run.legendary.casting.rains.first().map_or(hpos, |p| p.0)
                    } else {
                        hpos
                    };
                    if run.legendary.has(57) {
                        if let Some((entity, old, oldp, until)) =
                            run.legendary.casting.auras.get(def.key).copied()
                        {
                            if until > now {
                                center = old;
                            } else {
                                for f in around(&foes, old, oldp.radius) {
                                    hits.write(hit_for(def, &oldp, f.e, oldp.dmg, false));
                                }
                                commands.entity(entity).despawn();
                                run.legendary.casting.auras.remove(def.key);
                            }
                        }
                        if !run.legendary.casting.auras.contains_key(def.key)
                            && foes
                                .iter()
                                .any(|f| flat(f.pos - center).length() <= p.radius)
                        {
                            let serial = run.guid();
                            let entity = units::spawn_area_object(
                                &mut commands,
                                serial,
                                def.spell,
                                p.radius,
                                area_scale,
                                center,
                            );
                            run.legendary
                                .casting
                                .auras
                                .insert(def.key, (entity, center, p, now + 4.0));
                        }
                    }
                    let targets = around(&foes, center, p.radius);
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
                    if run.legendary.has(212) && run.legendary.effects.shield_lock > now {
                        continue;
                    }
                    if !extra {
                        super::talent_runtime::refreshed_shield(&mut run, p.dmg);
                    }
                    run.legendary.events.shield_origin =
                        if extra { Origin::Talent } else { Origin::Skill };
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
                        if def.key == "holy_shield" {
                            run.extra_spells.holy_until = now + p.duration;
                        } else if def.key == "ice_armor" {
                            run.extra_spells.ice_until = now + p.duration;
                        } else {
                            run.thorns = p.dmg;
                            run.thorns_dot = p.direct_dot;
                            run.thorns_t = p.duration;
                            run.thorns_spell = def.spell;
                        }
                        fx.cast(me, def.spell, &[me], None);
                        true
                    } else {
                        false
                    }
                }
                Kind::Heal => {
                    if def.key == "efflorescence" {
                        let ticks = p.duration.floor().max(1.0) as u32;
                        let serial = run.guid();
                        let entity = units::spawn_area_object(
                            &mut commands,
                            serial,
                            def.spell,
                            p.radius,
                            area_scale,
                            hpos,
                        );
                        if let Some(owner) = run.owner {
                            commands
                                .entity(entity)
                                .insert(super::multiplayer::OwnedEffect(owner));
                        }
                        run.extra_spells.blooms.push(super::extra_spells::Bloom {
                            entity,
                            center: hpos,
                            radius: p.radius,
                            amount: p.dmg / ticks as f32,
                            next: now + 1.0,
                            left: ticks,
                        });
                        fx.cast(me, def.spell, &[me], Some(hpos));
                        true
                    } else {
                        let level = run.abilities[i].level;
                        if !extra {
                            super::healer_buffs::on_heal_cast(&mut run, original_def, level);
                        }
                        let target = if extra {
                            healing.cast_with_origin(
                                &mut run, me, def, p.dmg, p.duration, p.range, true,
                            )
                        } else {
                            healing.cast(&mut run, me, def, p.dmg, p.duration, p.range)
                        };
                        fx.cast(me, def.spell, &[target], None);
                        fx.number(run.owner, target, p.dmg, false, COLOR_HEAL);
                        true
                    }
                }
                Kind::Totem => {
                    if foes
                        .first()
                        .is_some_and(|f| f.dist <= p.range.max(p.radius) + 6.0)
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
                        for retired in totem_slots.insert(def.key, totem, p.max_totems) {
                            commands.entity(retired).despawn();
                        }
                        commands.entity(totem).insert(Totem {
                            spawned_at: now,
                            projectiles: p,
                            effect: p.effect,
                            triggered: p.triggered,
                            cast_id: p.cast_id,
                            direct_dot: p.direct_dot,
                            tags: p.tags,
                            owner: run.owner,
                            skill: def.key,
                            crit: p.crit,
                            crit_multiplier: p.crit_multiplier,
                            cast: def.totem.1,
                            damage: p.dmg,
                            range: p.range,
                            radius: p.radius,
                            left: p.duration,
                            tick: 0.3,
                            pulse: matches!(def.key, "magma_totem" | "earthbind_totem"),
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
            }
        };
        if fired {
            if run.class.id == 3 {
                units::hunter_weapon(&mut commands, me, p.tags);
            }
            if !repeated {
                super::support_spells::activated(&mut run, def.key, p.tags);
            }
            run.meter.cast(def.key);
            run.boss_meter.cast(def.key);
            run.meter.debug_cast(def.key, ordinal.is_some(), extra);
            run.boss_meter.debug_cast(def.key, ordinal.is_some(), extra);
            run.legendary.events.position = hpos;
            if !repeated {
                run.legendary.events.serial = p.cast_id;
            }
            legendary_events::cast(&mut run, def, &p, primary, repeated);
            run.proc_runtime.parent = p.cast_id;
            super::items::legendary::casting::after_cast(&mut run, def, &p, primary, repeated);
            run.proc_runtime.parent = 0;
            if ordinal.is_some() {
                if let Some(other) = super::arcane_repeats::extra_spell(&run, i, p.tags, &mut rng) {
                    casts.push_back((other, None, true));
                }
            }
        }
        if !repeated {
            if fired && def.key == "revenge" {
                run.extra_spells.revenge_until = 0.0;
            }
            if fired && def.key == "mongoose_bite" {
                run.extra_spells.dodge_until = 0.0;
            }
            run.abilities[i].cd = if fired { p.cooldown } else { 0.2 };
            let bit = 1u64 << i.min(63);
            if fired {
                run.retrying &= !bit;
            } else {
                run.retrying |= bit;
            }
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
    mut run: ResMut<Run>,
    mut players: Option<ResMut<super::multiplayer::Companions>>,
    mut rng: ResMut<Rng>,
    spells: Option<Res<crate::ui_action::Spells>>,
    mut areas: Query<
        (
            Entity,
            &Transform,
            &mut GroundArea,
            Option<&crate::net::Guid>,
            Option<&mut crate::net::ObjectStore>,
        ),
        Without<Totem>,
    >,
    mut totems: Query<(Entity, &mut Transform, &mut Totem), Without<GroundArea>>,
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
    super::extra_spells::tick(&mut run, &live, &mut hits);
    if let Some(players) = players.as_deref_mut() {
        for player in players.0.values_mut() {
            super::extra_spells::tick(&mut player.run, &live, &mut hits);
        }
    }

    for (e, tf, mut area, guid, store) in &mut areas {
        area.left -= dt;
        area.next_tick -= dt;
        if let Some(held) = area.held {
            if area.left > 0.0 {
                continue;
            }
            area.damage = held;
            area.held = None;
            area.next_tick = 0.0;
        }
        if area.trail && area.next_tick <= 0.0 && area.left > 0.0 {
            area.next_tick += 1.0;
            let pos = super::multiplayer::player_run(&mut run, players.as_deref_mut(), area.owner)
                .map_or(tf.translation, |owner| owner.legendary.events.position);
            let serial = run.guid();
            let tile = units::spawn_area_object(
                &mut commands,
                serial,
                data::ability(area.skill).spell,
                area.radius * 0.4,
                (area.radius * 0.4 / data::ability(area.skill).radius.max(0.001)).clamp(0.25, 8.0),
                pos,
            );
            commands.entity(tile).insert(GroundArea {
                triggered: area.triggered,
                direct_dot: area.direct_dot,
                cast_id: 0,
                held: None,
                root_once: false,
                trail: false,
                tags: area.tags,
                owner: area.owner,
                skill: area.skill,
                crit: area.crit,
                crit_multiplier: area.crit_multiplier,
                damage: area.damage * 0.5,
                dot: 0.0,
                dot_dur: 0.0,
                radius: area.radius * 0.4,
                left: 2.0,
                every: 1.0,
                next_tick: 0.0,
                effect: Effect::None,
                mark: 0,
                impact: data::ability(area.skill).spell,
            });
            if let Some(owner) = area.owner {
                commands
                    .entity(tile)
                    .insert(super::multiplayer::OwnedEffect(owner));
            }
        }
        if area.trail {
            if area.left <= 0.0 {
                commands.entity(e).despawn();
            }
            continue;
        }
        if area.next_tick <= 0.0 {
            area.next_tick += area.every;
            if matches!(area.skill, "explosive_trap" | "frost_trap") {
                if let (Some(guid), Some(mut store)) = (guid, store) {
                    fx.trigger_trap(e, guid.0, &mut store, tf.translation, &mut fields);
                }
            }
            let primary = around(&live, tf.translation, area.radius)
                .into_iter()
                .max_by(|a, b| a.hp.total_cmp(&b.hp))
                .map(|f| f.e);
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
                    tags: Some(area.tags),
                    triggered: area.triggered,
                    proc_resolved: false,
                    proc_admitted: area.triggered,
                    direct_dot: area.direct_dot,
                    dot_tags: None,
                    proc_chain: 0,
                    item_source: None,
                    origin: if area.triggered {
                        Origin::Talent
                    } else {
                        Origin::Skill
                    },
                    cast_id: area.cast_id,
                    primary: Some(f.e) == primary && area.cast_id != 0,
                    target: f.e,
                    damage: dmg,
                    dot: area.dot,
                    dot_dur: area.dot_dur,
                    effect: area.effect,
                    mark: area.mark,
                    mark_dur: 1.5,
                    splash: 0.0,
                    crit,
                });
            }
            if !area.triggered {
                area.cast_id = 0;
            }
            if area.root_once {
                area.effect = Effect::None;
                area.root_once = false;
            }
        }
        if area.left <= 0.0 {
            commands.entity(e).despawn();
        }
    }

    for (e, mut tf, mut totem) in &mut totems {
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
            if let Some(owner) =
                super::multiplayer::player_run(&mut run, players.as_deref_mut(), totem.owner)
            {
                if owner.legendary.has(82)
                    && !live.iter().any(|f| {
                        flat(f.pos - tf.translation).length() <= totem.range.max(totem.radius)
                    })
                {
                    let delta = flat(owner.legendary.events.position - tf.translation);
                    tf.translation +=
                        delta.normalize_or_zero() * (dt * 1.5).min((delta.length() - 2.0).max(0.0));
                }
            }
            continue;
        }
        let pos = tf.translation;
        if let Some(owner) =
            super::multiplayer::player_run(&mut run, players.as_deref_mut(), totem.owner)
        {
            if owner.legendary.has(194) {
                let every = if totem.pulse { 2.0 } else { 1.0 };
                totem.tick = every;
                let percent = super::procs::resolve(
                    owner,
                    "proc_totem_heal",
                    0.02 * every,
                    0.0,
                    0.0,
                    12.0,
                    1,
                )
                .dmg;
                owner
                    .legendary
                    .effects
                    .heals
                    .push((pos, percent, 12.0, true, false));
                continue;
            }
        }
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
                    effect: totem.effect,
                    direct_dot: totem.direct_dot,
                    tags: Some(totem.tags),
                    triggered: totem.triggered,
                    proc_resolved: false,
                    proc_admitted: totem.triggered,
                    dot_tags: None,
                    proc_chain: 0,
                    item_source: None,
                    origin: if totem.triggered {
                        Origin::Talent
                    } else {
                        Origin::Skill
                    },
                    cast_id: if totem.triggered { totem.cast_id } else { 0 },
                    primary: false,
                    ..Hit::direct(totem.skill, f.e, dmg, crit)
                });
            }
        } else {
            totem.tick = 1.0;
            let mut foes: Vec<_> = live
                .iter()
                .copied()
                .map(|mut f| {
                    f.dist = flat(f.pos - pos).length();
                    f
                })
                .collect();
            foes.sort_by(|a, b| a.dist.total_cmp(&b.dist));
            let roots: Vec<_> = foes
                .iter()
                .filter(|f| f.dist <= totem.range)
                .take(totem.projectiles.count)
                .copied()
                .collect();
            if roots.is_empty() {
                continue;
            }
            fx.cast(e, totem.cast, &entities(&roots), None);
            let speed = spells
                .as_deref()
                .and_then(|s| s.catalog.get(totem.cast))
                .map_or(0.0, |d| d.speed);
            for link in links(&roots, &foes, &totem.projectiles, speed, false) {
                let (dmg, crit) = roll(
                    &mut rng,
                    totem.crit,
                    totem.crit_multiplier,
                    totem.damage * link.scale,
                );
                if let Some(from) = link.from {
                    pending
                        .1
                        .push((now + link.launch, from, link.target.e, totem.cast));
                }
                let mut direct_dot = totem.direct_dot;
                if let Some(dot) = direct_dot.as_mut() {
                    dot.dps *= link.scale;
                }
                schedule(
                    &mut pending,
                    &mut hits,
                    now,
                    link.arrival,
                    Hit {
                        owner: totem.owner,
                        skill: totem.skill,
                        effect: totem.effect,
                        direct_dot,
                        triggered: totem.triggered,
                        proc_admitted: totem.triggered,
                        origin: if totem.triggered {
                            Origin::Talent
                        } else {
                            Origin::Skill
                        },
                        cast_id: if totem.triggered { totem.cast_id } else { 0 },
                        tags: Some(totem.tags),
                        ..Hit::direct(totem.skill, link.target.e, dmg, crit)
                    },
                );
            }
        }
    }
}

/// Land every hit due: damage and its number, the over-time part, crowd control, the marks, and
/// death with its loot.
pub(super) fn apply_hits(
    ranks: Query<&super::enemies::Rank>,
    protection: Query<&super::special_mobs::Protection>,
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut rng: ResMut<Rng>,
    mut pending: ResMut<PendingHits>,
    mut incoming: MessageReader<Hit>,
    mut debuffs: Option<ResMut<super::support_spells::Debuffs>>,
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
    let mut slowed = Vec::new();
    if let Some(target) = run.support.slow_attacker.take() {
        slowed.push(target);
    }
    if let Some(players) = players.as_deref_mut() {
        for player in players.0.values_mut() {
            if let Some(target) = player.run.support.slow_attacker.take() {
                slowed.push(target);
            }
        }
    }
    for target in slowed {
        if let Ok((_, _, _, mut en, ..)) = enemies.get_mut(target) {
            en.slow = en.slow.min(0.6);
            en.slow_t = en.slow_t.max(2.0);
        }
    }
    // Equipment/buff sources stay fixed during this system, even across thousands of targets.
    let mut item_mods = std::collections::HashMap::new();
    pending.1.retain(|&(at, from, to, spell)| {
        if at <= now {
            if from == to {
                // A proc echo: no caster to animate, only the landing on the target.
                fx.echo(to, spell);
                fx.impact(to, spell);
            } else {
                fx.cast(from, spell, &[to], None);
            }
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
        if super::multiplayer::player_run(&mut run, players.as_deref_mut(), h.owner)
            .is_none_or(|player| player.pending_levels > 0)
        {
            continue;
        }
        if h.origin != Origin::Legendary && h.triggered && super::procs::find(h.skill).is_none() {
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
            let mut p = super::procs::resolve(
                player,
                h.skill,
                h.damage,
                h.dot,
                h.dot_dur,
                h.splash,
                if matches!(h.skill, "proc_spirit_bolt" | "proc_pet_bite") { 3 } else { 1 },
            );
            p.roll_extras(std::array::from_fn(|i| {
                if p.extra_chances[i] > 0.0 {
                    rng.f32()
                } else {
                    1.0
                }
            }));
            (h.damage, h.crit) =
                engine::critical_with_multiplier(p.dmg, p.crit, p.crit_multiplier, rng.f32());
            h.dot = p.dot;
            h.direct_dot = p.direct_dot;
            if h.direct_dot.is_some() {
                h.damage = 0.0;
            }
            h.tags = Some(p.tags);
            h.dot_dur = p.duration;
            h.splash = p.radius;
            h.triggered = true;
            if h.origin == Origin::Skill {
                h.origin = Origin::Talent;
            }
            h.proc_resolved = true;
            let repeats = engine::recast_count(p.recast, rng.f32(), false).min(64);
            let echo_spell = visual_spell(h.skill);
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
                        if let Some(dot) = copy.direct_dot.as_mut() {
                            dot.dps *= link.scale;
                        }
                        for repeat in 0..repeats {
                            let at =
                                now + link.arrival + engine::RECAST_DELAY * (repeat + 1) as f32;
                            pending.0.push((at, copy.clone()));
                            if let Some(spell) = echo_spell {
                                pending.1.push((at, copy.target, copy.target, spell));
                            }
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
                let at = now + engine::RECAST_DELAY * (repeat + 1) as f32;
                pending.0.push((at, h.clone()));
                if let Some(spell) = echo_spell {
                    pending.1.push((at, h.target, h.target, spell));
                }
            }
        }
        if h.direct_dot.is_some() {
            h.damage = 0.0;
            if matches!(h.effect, Effect::Execute(_)) {
                h.effect = Effect::None;
            }
        }
        prepared.push(h);
    }
    // Splash reaches the target's neighbours at seventy percent.
    let mut all = Vec::with_capacity(prepared.len());
    for mut h in prepared {
        // The echo keeps the original target and splash profile, rather than recording
        // separate echoes on the neighbours hit by the first explosion.
        if h.proc_chain == u64::MAX
            && h.skill == "fireball"
            && enemies.get(h.target).is_ok_and(|(_, _, _, en, ..)| en.dead)
        {
            continue;
        }
        if h.can_proc() && (h.damage > 0.0 || h.direct_dot.is_some()) && h.skill == "fireball" {
            if let Some(player) =
                super::multiplayer::player_run(&mut run, players.as_deref_mut(), h.owner)
            {
                if player.legendary.has(1) {
                    let mut echo = h.clone();
                    echo.triggered = true;
                    echo.origin = Origin::Legendary;
                    echo.proc_resolved = true;
                    echo.proc_chain = u64::MAX;
                    pending.0.push((now + engine::RECAST_DELAY, echo));
                }
            }
        }
        if h.splash > 0.0 {
            h.tags = Some(
                h.tags
                    .unwrap_or_else(|| engine::tags::for_spell(data::ability(h.skill)))
                    | engine::Tags::AREA,
            );
            if let Ok((_, _, center, ..)) = enemies.get(h.target) {
                let c = center.translation;
                for (e, _, tf, en, ..) in enemies.iter() {
                    if e != h.target && !en.dead && flat(tf.translation - c).length() <= h.splash {
                        all.push(Hit {
                            primary: false,
                            target: e,
                            damage: h.damage * 0.7,
                            splash: 0.0,
                            mark: 0,
                            direct_dot: h.direct_dot.map(|mut dot| {
                                dot.dps *= 0.7;
                                dot
                            }),
                            ..h.clone()
                        });
                    }
                }
            }
        }
        all.push(Hit { splash: 0.0, ..h });
    }

    // Character XP is shared: every kill is credited to each player once the hits are resolved.
    let mut party_kills: Vec<u32> = Vec::new();
    let mut kill = |commands: &mut Commands,
                    run: &mut Run,
                    e: Entity,
                    pos: Vec3,
                    en: &mut Enemy,
                    origin: Origin| {
        if origin == Origin::Skill {
            super::items::legendary::periodic::death(run, e, en, pos);
        }
        // Pack Frenzy also follows deaths caused by pet attacks, their bleeds and explosions.
        let pack_death = origin == Origin::Talent
            && super::talent_runtime::active(run, super::talent_design::Mechanic::Bloodburst)
            && super::items::legendary::milestones::source_class(run, super::talent_design::Mechanic::Bloodburst) == 3;
        let trap_death = origin == Origin::Talent
            && super::talent_runtime::active(run, super::talent_design::Mechanic::DeathTrap);
        if origin == Origin::Skill || pack_death || trap_death {
            super::talent_runtime::on_death(run, en, pos);
        }
        en.dead = true;
        en.hp = 0.0;
        en.dots.clear();
        enemies::mark_dying(commands, e);
        loot.write(Loot {
            item_level: en.def.level,
            pos,
            xp: en.xp,
            boss: en.boss,
        });
        run.kills += 1;
        party_kills.push(en.def.level);
        if origin == Origin::Skill {
            run.heal(run.equipment.life_on_kill);
        }
        if en.boss {
            run.banner = Some((format!("{} is slain!", en.def.name), 3.0));
        }
    };

    let expedition_defenses = run.expeditions.clone();
    let expedition_map = run.expedition_map;
    for mut h in all {
        let Some(run) = super::multiplayer::player_run(&mut run, players.as_deref_mut(), h.owner)
        else {
            continue;
        };
        if !h.can_proc() {
            let tags = h
                .tags
                .unwrap_or_else(|| engine::tags::for_spell(data::ability(h.skill)));
            if !super::procs::admit(
                run,
                &h,
                tags.intersects(engine::Tags::AREA | engine::Tags::CHAIN),
            ) {
                continue;
            }
            // Even legacy producers that only set `triggered` retain a proc origin in DoTs/kills.
            if h.origin == Origin::Skill {
                h.origin = Origin::Talent;
            }
        }
        if h.crit {
            if run.specialty.multi_unlocked {
                let crit = run.stats().crit;
                h.damage *= super::specialty::multi_crit(&run.specialty, crit, &mut rng);
            }
        }
        run.record.biggest_hit = run.record.biggest_hit.max(h.damage);
        super::procs::parent(run, h.cast_id);
        let Ok((e, guid, mut tf, mut en, mut store, mut marks)) = enemies.get_mut(h.target) else {
            continue;
        };
        if en.dead {
            continue;
        }
        let full = en.hp >= en.max_hp;
        let controls_before = [en.slow_t, en.root_t, en.stun_t, en.fear_t, en.weaken_t];
        let was_rooted = en.root_t > 0.0;
        let was_feared = en.fear_t > 0.0;
        let mut dealt_effective = 0.0;
        let tags = h.tags.unwrap_or_else(|| {
            super::items::legendary::profile(run, super::data::ability(h.skill)).1
        });
        super::items::legendary::periodic::before_hit(run, &h, &mut en, tf.translation, tags);
        super::items::legendary::effects::before_hit(run, &mut h, &mut en, tf.translation, tags);
        super::extra_spells::before_hit(run, &mut h, &mut en, tf.translation);
        if let Some(state) = debuffs.as_deref_mut() {
            if h.skill == "mangle" && h.can_proc() {
                let level = run
                    .abilities
                    .iter()
                    .find(|o| o.def.key == "mangle")
                    .map_or(1, |o| o.level);
                state.apply_from(
                    e,
                    "mangle",
                    super::support_spells::magnitude("mangle", level),
                    run.elapsed,
                    h.dot_dur,
                    run.owner,
                );
            }
            h.damage *= state.multiplier(e, run.elapsed, tags, false);
        }
        let mut mods = *item_mods
            .entry((h.owner, h.skill, tags.0))
            .or_insert_with(|| {
                engine::on_hit::modifiers_tagged(h.skill, tags, run.spell_modifiers.entries())
            });
        let (armour, final_reduction) = expedition_defenses.defenses(
            expedition_map,
            en.def.level,
            en.boss,
            super::training::actor(en.def),
        );
        if !h.can_proc() {
            if !tags.contains(engine::Tags::MINION | engine::Tags::DAMAGE) {
                mods.leech = 0.0;
            }
            mods.bleed = 0.0;
            mods.ignite = 0.0;
            mods.chill = 0.0;
            mods.knockback = 0.0;
        }
        let talent_multiplier = if h.origin == Origin::Legendary {
            1.0
        } else {
            super::talent_runtime::before_hit(run, &h, &en)
        };
        let raw = engine::on_hit::damage(
            h.damage * talent_multiplier,
            en.hp,
            en.max_hp,
            en.boss,
            armour,
            mods,
        );
        let resistance =
            expedition_defenses.resistances(en.def.level, en.boss, super::training::actor(en.def));
        let raw = raw * (1.0 - resistance.reduction(tags));
        let protection_multiplier = 1.0 - protection.get(e).map_or(0.0, |p| p.0);
        let elite_multiplier = if ranks
            .get(e)
            .is_ok_and(|r| *r == super::enemies::Rank::Elite)
        {
            1.0 - expedition_defenses.elites.damage_reduction
        } else {
            1.0
        };
        let dmg = engine::damage_on_target(raw, en.hp, en.max_hp, en.boss, h.effect)
            * (1.0 - final_reduction)
            * elite_multiplier
            * protection_multiplier;
        if dmg > 0.0 {
            let immortal = super::training::dummy(&en);
            let effective = if immortal {
                dmg
            } else {
                dmg.min(en.hp.max(0.0))
            };
            dealt_effective = effective;
            if h.can_proc() {
                super::extra_spells::damage(run, e, tf.translation, effective);
            }
            if h.can_proc() {
                super::talent_runtime::periodic_leech(run, tags, effective);
            }
            let now = run.elapsed;
            let major = en.boss
                || ranks
                    .get(e)
                    .is_ok_and(|r| matches!(r, super::enemies::Rank::Elite | super::enemies::Rank::Boss));
            let target_kind = if en.boss {
                "Boss"
            } else if major {
                "Elite"
            } else if immortal {
                "Training dummy"
            } else {
                "Normal"
            };
            for meter in std::iter::once(&mut run.meter).chain((major || immortal).then_some(&mut run.boss_meter)) {
                meter.record_damage(
                    h.meter_source.unwrap_or(h.skill),
                    now,
                    super::combat_ui::DamageEvent {
                        skill: h.skill,
                        tags,
                        origin: h.origin,
                        item: h.item_source,
                        cast: h.cast_id,
                        amount: dmg,
                        effective,
                        critical: h.crit,
                        periodic: false,
                        target: target_kind,
                        family: "Direct",
                    },
                );
            }
            if immortal {
                en.hp = en.max_hp;
            } else {
                en.hp -= dmg;
            }
            run.damage_done += f64::from(effective);
            fx.hit_number(h.owner, e, dmg, h.crit, tags);
            if h.can_proc() {
                if let Effect::Drain(frac) = h.effect {
                    if !(run.legendary.has(310) && h.skill == "death_coil") || h.primary {
                        run.heal(effective * frac);
                    }
                }
            }
            super::items::combat::leech(run, effective * mods.leech);
            if h.can_proc() {
                super::items::combat::record_hit_from(run, h.skill, mods.tags, h.cast_id);
            }
            for (chance, factor, seconds, spell) in
                [(mods.bleed, 0.20, 4.0, 1943), (mods.ignite, 0.25, 3.0, 133)]
            {
                if h.can_proc() && en.hp > 0.0 && chance > 0.0 {
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
                    if !super::procs::ready(run, key, tags) {
                        continue;
                    }
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
                    super::items::legendary::periodic::install(
                        run,
                        &mut en,
                        e,
                        tf.translation,
                        enemies::Dot {
                            owner: h.owner,
                            skill: key,
                            meter_source,
                            tags: Some(p.tags),
                            item_source: None,
                            origin: if h.origin == Origin::Item {
                                Origin::Item
                            } else {
                                Origin::Talent
                            },
                            meta: Default::default(),
                            dps: p.dot,
                            left: p.duration,
                            tick,
                        },
                    );
                    marks.add(spell, p.duration);
                    fx.impact(e, spell);
                }
            }
            if mods.chill > 0.0
                && rng.f32() < mods.chill.clamp(0.0, 1.0)
                && super::procs::ready(run, "proc_chill", engine::Tags::COLD | engine::Tags::SPELL)
            {
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
                && super::procs::ready(run, "proc_knockback", tags)
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
        if dealt_effective > 0.0 {
            super::minions::on_hit(run, &h, tags, &mut en, e, tf.translation);
        }
        if h.dot > 0.0 && h.dot_dur > 0.0 {
            super::items::legendary::periodic::install(
                run,
                &mut en,
                e,
                tf.translation,
                enemies::Dot {
                    owner: h.owner,
                    skill: h.skill,
                    meter_source: h.meter_source,
                    tags: Some(h.dot_tags.unwrap_or(tags)),
                    item_source: h.item_source,
                    origin: h.origin,
                    meta: Default::default(),
                    dps: h.dot,
                    left: h.dot_dur,
                    tick: 1.0,
                },
            );
        }
        if let Some(dot) = h.direct_dot {
            super::items::legendary::periodic::install(
                run,
                &mut en,
                e,
                tf.translation,
                enemies::Dot {
                    owner: h.owner,
                    skill: h.skill,
                    meter_source: h.meter_source,
                    tags: Some(dot.tags),
                    origin: h.origin,
                    item_source: h.item_source,
                    meta: default(),
                    dps: dot.dps,
                    left: dot.duration,
                    tick: 1.0,
                },
            );
        }
        if h.origin != Origin::Legendary {
            super::talent_runtime::on_hit(run, &h, &mut en, tf.translation, &mut rng, &mut pending);
        }
        let boss = en.boss;
        if run.legendary.has(159) && h.origin != Origin::Legendary {
            if let Effect::Fear(duration) = h.effect {
                h.effect = Effect::None;
                if boss {
                    super::items::legendary::effects::fear_boss(run, e, tf.translation);
                } else if run.legendary.effects.allies.len() > 1 {
                    commands.entity(e).insert(super::talent_runtime::Taunted {
                        owner: run.owner,
                        until: run.elapsed + duration,
                    });
                } else {
                    en.slow = en.slow.min(0.5);
                    en.slow_t = en.slow_t.max(duration);
                }
            }
        }
        match h.effect {
            Effect::Slow(m, t) => {
                let m = if en.ignores_freeze(ranks.get(e).ok()) { m.max(0.5) } else { m };
                en.slow = if en.slow_t > 0.0 { en.slow.min(m) } else { m };
                en.slow_t = en.slow_t.max(t);
            }
            Effect::Root(t) => en.root_t = en.root_t.max(if boss { t * 0.3 } else { t }),
            Effect::Stun(t) => en.stun_t = en.stun_t.max(if boss { t * 0.3 } else { t }),
            Effect::Fear(t) if !boss => en.fear_t = en.fear_t.max(t),
            Effect::Weaken(m, t) => {
                en.weaken = if en.weaken_t > 0.0 {
                    en.weaken.min(m)
                } else {
                    m
                };
                en.weaken_t = en.weaken_t.max(t);
            }
            _ => {}
        }
        if h.mark != 0 {
            marks.add(h.mark, h.mark_dur);
        }
        let rooted_by_skill = !was_rooted && en.root_t > 0.0;
        super::items::legendary::effects::hit(
            run,
            &h,
            &mut en,
            tf.translation,
            tags,
            dealt_effective,
            was_feared,
            &mut rng,
        );
        en.cap_slow(ranks.get(e).ok());
        super::combat_ui::track_control_changes(
            &mut commands, e, h.owner, h.skill, controls_before, &en,
        );
        legendary_events::hit(
            run,
            &h,
            tags,
            tf.translation,
            dealt_effective,
            full,
            rooted_by_skill,
            &mut rng,
        );
        if en.hp <= 0.0 {
            if h.origin == Origin::Skill && !h.triggered {
                super::items::legendary::effects::death(
                    run,
                    e,
                    tf.translation,
                    h.skill,
                    dealt_effective,
                );
            }
            legendary_events::killed(run, e, tf.translation, h.origin, &mut rng);
            run.record.spell_kill(h.skill);
            kill(&mut commands, run, e, tf.translation, &mut en, h.origin);
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
        let mut periodic_numbers = std::collections::BTreeMap::<(Option<u64>, u32), f32>::new();
        let mut last_owner = None;
        let mut last_origin = Origin::Skill;
        let mut last_skill = "";
        let mut last_parent = 0;
        let (max_hp, boss) = (en.max_hp, en.boss);
        let rooted = en.root_t > 0.0;
        let resistance =
            expedition_defenses.resistances(en.def.level, boss, super::training::actor(en.def));
        let (armour, final_reduction) = expedition_defenses.defenses(
            expedition_map,
            en.def.level,
            boss,
            super::training::actor(en.def),
        );
        let protection_multiplier = 1.0 - protection.get(e).map_or(0.0, |p| p.0);
        let elite_multiplier = if ranks
            .get(e)
            .is_ok_and(|r| *r == super::enemies::Rank::Elite)
        {
            1.0 - expedition_defenses.elites.damage_reduction
        } else {
            1.0
        };
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
                if run.pending_levels > 0 {
                    continue;
                }
                d.meta.parent = super::procs::parent(run, d.meta.parent);
                let tick_damage = super::items::legendary::periodic::tick(
                    run,
                    e,
                    d,
                    tf.translation,
                    rooted,
                    boss,
                );
                let tags = d.tags.unwrap_or_else(|| {
                    super::items::legendary::profile(run, super::data::ability(d.skill)).1
                });
                let mods = *item_mods
                    .entry((d.owner, d.skill, tags.0))
                    .or_insert_with(|| {
                        engine::on_hit::modifiers_tagged(
                            d.skill,
                            tags,
                            run.spell_modifiers.entries(),
                        )
                    });
                let amount =
                    engine::on_hit::damage(tick_damage, remaining_hp, max_hp, boss, armour, mods)
                        * debuffs.as_deref().map_or(1.0, |state| {
                            state.multiplier(
                                e,
                                run.elapsed,
                                tags,
                                super::items::legendary::periodic::family(d)
                                    == super::items::legendary::periodic::Family::Bleed,
                            )
                        })
                        * (1.0 - resistance.reduction(tags))
                        * (1.0 - final_reduction)
                        * elite_multiplier
                        * protection_multiplier;
                dealt += amount;
                *periodic_numbers
                    .entry((d.owner, fx.periodic_color(tags)))
                    .or_default() += amount;
                let effective = if immortal {
                    amount.max(0.0)
                } else {
                    amount.min(remaining_hp).max(0.0)
                };
                let now = run.elapsed;
                let major = boss
                    || ranks
                        .get(e)
                        .is_ok_and(|r| matches!(r, super::enemies::Rank::Elite | super::enemies::Rank::Boss));
                let target_kind = if boss {
                    "Boss"
                } else if major {
                    "Elite"
                } else if immortal {
                    "Training dummy"
                } else {
                    "Normal"
                };
                let family = match super::items::legendary::periodic::family(d) {
                    super::items::legendary::periodic::Family::Bleed => "Bleed",
                    super::items::legendary::periodic::Family::Poison => "Poison",
                    super::items::legendary::periodic::Family::Burn => "Ignite / burn",
                    super::items::legendary::periodic::Family::Disease => "Disease",
                    super::items::legendary::periodic::Family::Curse => "Curse",
                    _ => "Other periodic",
                };
                for meter in std::iter::once(&mut run.meter).chain((major || immortal).then_some(&mut run.boss_meter)) {
                    meter.record_damage(
                        d.meter_source.unwrap_or(d.skill),
                        now,
                        super::combat_ui::DamageEvent {
                            skill: d.skill,
                            tags,
                            origin: d.origin,
                            item: d.item_source,
                            cast: d.meta.parent,
                            amount,
                            effective,
                            critical: false,
                            periodic: true,
                            target: target_kind,
                            family,
                        },
                    );
                }
                if !immortal {
                    remaining_hp = (remaining_hp - effective).max(0.0);
                }
                run.damage_done += f64::from(effective);
                if d.origin == Origin::Skill {
                    super::talent_runtime::periodic_leech(run, tags, effective);
                    super::extra_spells::damage(run, e, tf.translation, effective);
                    if d.skill == "devouring_plague" {
                        run.heal(effective * 0.15);
                    }
                }
                if d.origin == Origin::Skill {
                    super::items::legendary::effects::periodic_hit(
                        run,
                        e,
                        tf.translation,
                        d.skill,
                        tags,
                        effective,
                        d.left,
                    );
                }
                let mut periodic = Hit::direct(d.skill, e, effective, false);
                periodic.origin = d.origin;
                legendary_events::hit(
                    run,
                    &periodic,
                    tags,
                    tf.translation,
                    effective,
                    false,
                    false,
                    &mut rng,
                );
                if effective > 0.0 {
                    last_owner = d.owner;
                    last_origin = d.origin;
                    last_skill = d.skill;
                    last_parent = d.meta.parent;
                }
            }
        }

        if dealt > 0.0 {
            if immortal {
                en.hp = en.max_hp;
            } else {
                en.hp -= dealt;
            }
            for ((owner, color), damage) in periodic_numbers {
                if damage > 0.0 {
                    fx.number(owner, e, damage, false, color);
                }
            }
            if en.hp <= 0.0 {
                if let Some(run) =
                    super::multiplayer::player_run(&mut run, players.as_deref_mut(), last_owner)
                {
                    super::procs::parent(run, last_parent);
                    legendary_events::killed(run, e, tf.translation, last_origin, &mut rng);
                    run.record.spell_kill(last_skill);
                    kill(&mut commands, run, e, tf.translation, &mut en, last_origin);
                }
                units::kill_unit(&mut store);
                marks.clear();
                marks.flush(e, guid.0, &mut store, &mut fields);
            }
        }
        if !en.dead {
            for d in en.dots.iter().filter(|d| d.left <= 0.0) {
                if let Some(player) =
                    super::multiplayer::player_run(&mut run, players.as_deref_mut(), d.owner)
                {
                    if !d.meta.transferred {
                        super::procs::parent(player, d.meta.parent);
                        legendary_events::expired(player, tf.translation, d.origin);
                        super::items::legendary::periodic::expired(player, e, d, tf.translation);
                    }
                }
            }
        }
        en.dots.retain(|d| d.left > 0.0);
    }
    share_kills(&mut run, players.as_deref_mut(), &party_kills);
    run.proc_runtime.parent = 0;
    if let Some(players) = players.as_deref_mut() {
        for player in players.0.values_mut() {
            player.run.proc_runtime.parent = 0;
        }
    }
}

/// Every player is credited every kill, against their own character level, and the bosses the host
/// has seen, since only the host's run spawns them.
fn share_kills(
    host: &mut Run,
    players: Option<&mut super::multiplayer::Companions>,
    levels: &[u32],
) {
    for &level in levels {
        super::talents::credit_kill(host, level);
    }
    for player in players.into_iter().flat_map(|players| players.0.values_mut()) {
        for &level in levels {
            super::talents::credit_kill(&mut player.run, level);
        }
        player.run.bosses = player.run.bosses.max(host.bosses);
    }
}
