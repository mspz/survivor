use super::super::{data, Run};
use super::*;
use bevy::prelude::*;

fn base() -> Stats {
    Run::default().stats()
}
fn change(change: Change) -> Modifier {
    Modifier {
        source: 1,
        filter: Filter::default(),
        change,
    }
}
fn spell(key: &str, mods: &[Modifier]) -> Resolved {
    resolve(data::ability(key), 1, &base(), mods)
}
fn target(n: u32, x: f32) -> targeting::Target {
    targeting::Target {
        e: Entity::from_raw_u32(n).unwrap(),
        pos: Vec3::new(x, 0.0, 0.0),
        hp: 100.0,
        dist: x,
        radius: 0.0,
    }
}

#[test]
fn every_authored_spell_has_a_school_and_exactly_one_attack_or_spell_tag() {
    for def in data::ABILITIES {
        assert!(
            tags::school(def.key).is_some(),
            "Missing school: {}",
            def.key
        );
        let t = tags::for_spell(def);
        assert_ne!(
            t.contains(Tags::ATTACK),
            t.contains(Tags::SPELL),
            "{}",
            def.key
        );
        assert!(
            !(t.contains(Tags::MELEE) && t.contains(Tags::PROJECTILE)),
            "{}",
            def.key
        );
    }
}
#[test]
fn unmodified_values_preserve_existing_rank_scaling_for_every_spell() {
    let stats = base();
    for def in data::ABILITIES {
        for level in 1..=data::MAX_ABILITY_LEVEL {
            let p = resolve(def, level, &stats, &[]);
            let rank = f32::from(level - 1);
            assert!(
                (p.dmg - def.damage * (1.0 + 0.3 * rank) * stats.might).abs() < 0.001,
                "{}",
                def.key
            );
            assert!((p.dot - def.dot * (1.0 + 0.3 * rank) * stats.might).abs() < 0.001);
            assert!((p.radius - def.radius * (1.0 + 0.08 * rank) * stats.area).abs() < 0.001);
            assert!(
                (p.cooldown
                    - (def.cooldown
                        * 0.93f32.powi(i32::from(level) - 1)
                        * (1.0 - stats.haste).max(0.35))
                    .max(0.2))
                .abs()
                    < 0.001
            );
            assert_eq!(
                p.count,
                def.count as usize + [3, 5, 7].into_iter().filter(|&n| level >= n).count()
            );
        }
    }
}
#[test]
fn projectiles_cannot_affect_melee_even_with_an_unfiltered_modifier() {
    let mods = [change(Change::AdditionalProjectiles(4))];
    assert_eq!(
        spell("heroic_strike", &mods).count,
        spell("heroic_strike", &[]).count
    );
    assert_eq!(spell("arcane_shot", &mods).count, 5);
    let attack_only = [Modifier {
        filter: Filter {
            all: Tags::ATTACK | Tags::PROJECTILE,
            ..default()
        },
        ..mods[0]
    }];
    assert_eq!(spell("fireball", &attack_only).count, 1);
    assert_eq!(spell("arcane_shot", &attack_only).count, 5);
}
#[test]
fn elemental_filters_and_increased_more_have_distinct_stacking() {
    let fire = Modifier {
        filter: Filter {
            all: Tags::FIRE,
            ..default()
        },
        ..change(Change::IncreasedDamage(1.0))
    };
    assert_eq!(spell("fireball", &[fire]).dmg, 40.0);
    assert_eq!(spell("frostbolt", &[fire]).dmg, 12.0);
    let mods = [
        change(Change::FlatDamage(10.0)),
        change(Change::IncreasedDamage(0.2)),
        change(Change::IncreasedDamage(0.3)),
        change(Change::MoreDamage(0.4)),
        change(Change::MoreDamage(-0.2)),
    ];
    assert!((spell("fireball", &mods).dmg - 50.4).abs() < 0.001);
    assert_eq!(
        spell("power_word_shield", &mods).dmg,
        spell("power_word_shield", &[]).dmg
    );
}
#[test]
fn selectors_support_exclusions_any_tags_and_one_specific_skill() {
    let filter = Filter {
        any: Tags::FIRE | Tags::COLD,
        exclude: Tags::AREA,
        ..default()
    };
    assert!(filter.matches("frostbolt", tags::for_spell(data::ability("frostbolt"))));
    assert!(!filter.matches("fireball", tags::for_spell(data::ability("fireball"))));
    let single = Filter {
        skill: Some("cleave"),
        ..default()
    };
    assert!(single.matches("cleave", Tags::ATTACK));
    assert!(!single.matches("heroic_strike", Tags::ATTACK));
}
#[test]
fn melee_targets_and_area_are_compatible_only_with_their_mechanics() {
    assert_eq!(
        spell(
            "heroic_strike",
            &[change(Change::AdditionalMeleeTargets(2))]
        )
        .count,
        3
    );
    assert_eq!(
        spell("fireball", &[change(Change::AdditionalMeleeTargets(2))]).count,
        1
    );
    let area = [change(Change::IncreasedArea(1.0))];
    assert!(
        (spell("frost_nova", &area).radius / spell("frost_nova", &[]).radius - 2.0f32.sqrt()).abs()
            < 0.001
    );
    assert_eq!(
        spell("chain_lightning", &area).link_range,
        spell("chain_lightning", &[]).link_range
    );
}
#[test]
fn source_replacement_and_removal_do_not_accumulate_stale_item_rolls() {
    let mut set = Modifiers::default();
    set.replace_source(12, [change(Change::AdditionalProjectiles(2))]);
    set.replace_source(12, [change(Change::AdditionalProjectiles(3))]);
    assert_eq!(spell("fireball", set.entries()).count, 4);
    set.remove_source(12);
    assert_eq!(spell("fireball", set.entries()).count, 1);
}
#[test]
fn native_chain_and_projectile_fork_chain_routes_have_timing_and_no_cycles() {
    let foes: Vec<_> = (0..10).map(|n| target(n, n as f32 + 1.0)).collect();
    let native = spell("chain_lightning", &[]);
    let route = targeting::links(&foes[..1], &foes, &native, 20.0, true);
    assert_eq!(route.len(), 4);
    let extra = spell("chain_lightning", &[change(Change::AdditionalChains(2))]);
    assert_eq!(extra.count, 5);
    assert_eq!(
        targeting::links(&foes[..1], &foes, &extra, 20.0, true).len(),
        6
    );
    assert!((route[2].arrival - 0.16).abs() < 0.001);
    let p = spell(
        "fireball",
        &[change(Change::Fork(1)), change(Change::AdditionalChains(1))],
    );
    let route = targeting::links(&foes[..1], &foes, &p, 10.0, false);
    assert_eq!(route.len(), 5);
    assert_eq!(
        route
            .iter()
            .map(|h| h.target.e)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        5
    );
    assert!(route[1].launch >= route[0].arrival);
    assert!(route[1].arrival > route[1].launch);
    assert!((route[1].scale - 0.7).abs() < 0.001);
}
#[test]
fn graph_budget_limits_hostile_modifiers_and_reserves_primary_targets() {
    let foes: Vec<_> = (0..250).map(|n| target(n, n as f32 * 0.01)).collect();
    let p = spell(
        "fireball",
        &[
            change(Change::Fork(u32::MAX)),
            change(Change::AdditionalChains(u32::MAX)),
            change(Change::AdditionalProjectiles(u32::MAX)),
        ],
    );
    let route = targeting::links(&foes[..20], &foes, &p, 0.0, false);
    assert!(route.len() <= MAX_TARGETS);
    assert_eq!(
        route
            .iter()
            .map(|h| h.target.e)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        route.len()
    );
}
#[test]
fn recasts_do_not_trigger_recasts_and_utility_skills_do_not_repeat() {
    assert_eq!(recast_count(1.0, 0.5, false), 1);
    assert_eq!(recast_count(2.3, 0.0, true), 0);
    assert_eq!(recast_count(0.0, 0.0, false), 0);
    let mods = [change(Change::RecastChance(4.0))];
    assert_eq!(spell("fireball", &mods).recast, 4.0);
    for key in ["divine_shield", "rejuvenation", "searing_totem", "hellfire"] {
        assert_eq!(spell(key, &mods).recast, 0.0);
    }
}
#[test]
fn repeat_hundreds_are_guaranteed_and_only_the_fraction_is_random() {
    for (chance, guaranteed) in [(0.3, 0), (1.3, 1), (2.3, 2), (4.3, 4)] {
        let mut extra = 0;
        for n in 0..10_000 {
            let count = recast_count(chance, (n as f32 + 0.5) / 10_000.0, false);
            assert!(count == guaranteed || count == guaranteed + 1);
            extra += usize::from(count == guaranteed + 1);
        }
        assert_eq!(extra, 3000);
    }
    assert_eq!(recast_count(1.0, 0.0, false), 1);
    assert_eq!(recast_count(2.0, 0.999, false), 2);
    assert_eq!(recast_count(-1.0, 0.0, false), 0);
}
#[test]
fn large_repeat_bursts_use_one_job_and_are_spaced_without_dropping_new_casts() {
    let mut queue = Recasts::default();
    queue.schedule(0, 0.0, 1000);
    queue.schedule(0, 0.0, 1000);
    assert_eq!(queue.len(), 1);
    assert!(!queue.take_due(0, RECAST_DELAY - 0.01));
    assert!(queue.take_due(0, RECAST_DELAY));
    assert!(!queue.take_due(0, RECAST_DELAY));
    assert!(queue.take_due(0, RECAST_DELAY * 2.0));
    assert_eq!(queue.0[0].remaining, 1998);
    queue.schedule(1, 1.0, u64::MAX);
    queue.schedule(1, 1.0, 10);
    assert_eq!(queue.len(), 2);
    assert_eq!(queue.0[1].remaining, u64::MAX);
}
#[test]
fn damage_resolution_preserves_crit_execute_armour_and_shields() {
    assert_eq!(critical(20.0, 0.5, 0.2), (40.0, true));
    assert_eq!(
        damage_on_target(3.0, 20.0, 100.0, false, Effect::Execute(0.3)),
        20.0
    );
    assert_eq!(
        damage_on_target(3.0, 20.0, 100.0, true, Effect::Execute(0.3)),
        3.0
    );
    assert_eq!(mitigate(10.0, 2.0, 5.0, false), (3.0, 5.0));
    assert_eq!(mitigate(10.0, 2.0, 5.0, true), (0.0, 0.0));
}

#[test]
fn all_nine_mobility_skills_are_classified_and_ignore_projectile_damage_modifiers() {
    let mods = [
        change(Change::AdditionalProjectiles(10)),
        change(Change::Fork(2)),
        change(Change::IncreasedDamage(2.0)),
        change(Change::IncreasedDuration(1.0)),
        change(Change::IncreasedCooldownRecovery(1.0)),
    ];
    for class in data::CLASSES {
        let def = super::super::mobility::for_class(class.id).unwrap();
        let (key, tags) = tags::mobility(def.spell).expect("Unclassified mobility");
        assert!(tags.contains(Tags::UTILITY));
        assert!(!tags.contains(Tags::PROJECTILE));
        assert!(tags::school(key).is_some());
        let p = resolve_mobility(def, &mods);
        assert_eq!(p.cooldown, def.cooldown / 2.0);
        assert_eq!(p.duration, def.secs * 2.0);
    }
    let howl = super::super::mobility::for_class(9).unwrap();
    assert!(
        (resolve_mobility(howl, &[change(Change::IncreasedArea(3.0))]).radius - 24.0).abs() < 0.001
    );
}
