//! Run rewards are generic stats or tag-filtered engine sources with a useful owned recipient.
use super::{
    data::{self, PassiveDef, PassiveEffect},
    spell_engine::{self as engine, Change, Filter, Modifier},
    Run,
};

pub(super) fn discrete(passive: &PassiveDef) -> bool {
    legendary(passive)
        || matches!(
            passive.effect,
            PassiveEffect::Modifier(_, Change::AdditionalTotems(_))
        )
}

pub(super) fn legendary(passive: &PassiveDef) -> bool {
    matches!(
        passive.effect,
        PassiveEffect::Modifier(
            _,
            Change::ProjectileChance(_)
                | Change::AdditionalTotems(2)
                | Change::MeleeTargetChance(_)
                | Change::ForkChance(_)
                | Change::ChainChance(_)
                | Change::FlatRange(_)
        )
    )
}

pub(super) fn eligible(passive: &PassiveDef, run: &Run) -> bool {
    if matches!(passive.effect, PassiveEffect::Modifier(_, Change::DodgeChance(_))) {
        return matches!(run.class.id, 3 | 4 | 11)
            && super::extra_spells::dodge_chance(run) < 0.75;
    }
    if matches!(
        passive.effect,
        PassiveEffect::Stat(data::Stat::DamageReduction, _)
    ) {
        return run.stats().damage_reduction < 0.75;
    }
    let PassiveEffect::Modifier(filter, change) = passive.effect else {
        return true;
    };
    let source = data::PASSIVES
        .iter()
        .position(|p| p.key == passive.key)
        .map_or(u64::MAX, |i| 0x2000_0000 + i as u64);
    let rank = run
        .passives
        .iter()
        .find(|(p, _)| p.key == passive.key)
        .map_or(1, |(_, rank)| rank.saturating_add(1));
    let mut after: Vec<_> = run
        .spell_modifiers
        .entries()
        .iter()
        .filter(|m| m.source != source)
        .copied()
        .collect();
    after.push(Modifier {
        source,
        filter,
        change: change.scaled(rank),
    });
    let before = run.spell_modifiers.entries();
    let stats = run.stats();
    run.abilities.iter().any(|owned| {
        engine::resolve(owned.def, owned.level, &stats, before)
            != engine::resolve(owned.def, owned.level, &stats, &after)
    }) || super::procs::available(run).iter().any(|def| {
        let tags = super::procs::gameplay_tags(run, def.key, engine::tags::for_spell(def));
        engine::resolve_tagged(def, 1, &stats, before, 0.0, tags)
            != engine::resolve_tagged(def, 1, &stats, &after, 0.0, tags)
    }) || run.mobility().is_some_and(|def| {
        engine::resolve_mobility(def, before) != engine::resolve_mobility(def, &after)
    })
}
pub(super) fn sync(run: &mut Run) {
    for (index, p) in data::PASSIVES.iter().enumerate() {
        let level = run
            .passives
            .iter()
            .find(|(owned, _)| owned.key == p.key)
            .map(|(_, rank)| *rank);
        let modifier = match (p.effect, level) {
            (PassiveEffect::Modifier(filter, change), Some(rank)) => Some(Modifier {
                source: 0,
                filter,
                change: change.scaled(rank),
            }),
            _ => None,
        };
        run.replace_spell_modifiers(0x2000_0000 + index as u64, modifier);
    }
}

pub(super) fn scope(filter: Filter, change: Change) -> String {
    let labels = filter.all.labels();
    if !labels.is_empty() {
        return labels;
    }
    match change {
        Change::AdditionalChains(_) | Change::ChainChance(_) => "Projectile / Chain".to_owned(),
        Change::RecastChance(_) => "Repeatable damaging skills".to_owned(),
        Change::CriticalChance(_) | Change::CriticalMultiplier(_) => "Damaging hits".to_owned(),
        _ => "Damaging skills".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn regeneration_cards_restore_a_percentage_of_final_max_health_per_second() {
        let regeneration = passive("renew");
        let health = passive("fortitude");
        let mut run = run(&["fireball"]);
        run.equipment.regeneration = 5.0;
        for strength in 1..=4 {
            for reverse in [false, true] {
                run.passives = vec![(regeneration, strength), (health, 2)];
                if reverse {
                    run.passives.reverse();
                }
                let stats = run.stats();
                let fraction = 0.01 * strength as f32;
                assert!((stats.regen - (5.0 + stats.max_hp * fraction)).abs() < 0.0001);
                run.equipment_hp += 100.0;
                let scaled = run.stats();
                assert!(scaled.regen > stats.regen);
                assert!((scaled.regen - (5.0 + scaled.max_hp * fraction)).abs() < 0.0001);
            }
        }
        run.passives = vec![(regeneration, 10)];
        assert!((run.stats().regen - (5.0 + run.stats().max_hp * 0.1)).abs() < 0.0001);
    }
    #[test]
    fn totem_cards_stack_limits_and_improve_offensive_and_support_totems() {
        for key in [
            "searing_totem",
            "magma_totem",
            "earthbind_totem",
            "healing_stream_totem",
            "strength_of_earth_totem",
        ] {
            let mut run = run(&[key]);
            let def = data::ability(key);
            assert!(run.resolve_spell(def, 1).tags.contains(engine::Tags::TOTEM));
            assert_eq!(run.resolve_spell(def, 1).max_totems, 1);
            for (card, rank) in [("totem_limit_epic", 1), ("totem_limit_legendary", 2)] {
                assert!(eligible(passive(card), &run));
                run.passives.push((passive(card), rank));
            }
            sync(&mut run);
            assert_eq!(run.resolve_spell(def, 1).max_totems, 6);
            assert_eq!(
                run.resolve_spell(data::ability("fireball"), 1).max_totems,
                0
            );
            assert_eq!(
                run.resolve_spell(data::ability("totemic_recall"), 1)
                    .max_totems,
                0
            );
            for card in [
                "totem_power",
                "totem_duration",
                "totem_area",
                "totem_recovery",
            ] {
                let expected = !(card == "totem_area" && key == "searing_totem"
                    || card == "totem_power" && key == "earthbind_totem");
                assert_eq!(eligible(passive(card), &run), expected, "{key} {card}");
            }
        }
        assert!(!eligible(
            passive("totem_limit_epic"),
            &run(&["fireball", "totemic_recall"])
        ));
    }
    #[test]
    fn legendary_attack_range_stacks_and_only_offers_to_attacks_with_target_range() {
        let p = passive("attack_range");
        assert!(legendary(p));
        for key in ["heroic_strike", "arcane_shot", "serpent_sting"] {
            let mut run = run(&[key]);
            assert!(eligible(p, &run), "{key}");
            let def = data::ability(key);
            let before = run.resolve_spell(def, 1);
            run.passives.push((p, 3));
            sync(&mut run);
            let after = run.resolve_spell(def, 1);
            assert_eq!(after.range, before.range + 6.0, "{key}");
            assert_eq!(after.radius, before.radius);
            assert!(eligible(p, &run));
            assert_eq!(
                run.resolve_spell(data::ability("fireball"), 1).range,
                data::ability("fireball").range
            );
        }
        assert!(!eligible(p, &run(&["fireball"])));
        assert!(!eligible(p, &run(&["whirlwind"])));
    }
    use super::super::Owned;
    use super::*;
    fn run(keys: &[&str]) -> Run {
        Run {
            abilities: keys
                .iter()
                .map(|key| Owned {
                    def: data::ability(key),
                    level: 1,
                    cd: 0.0,
                })
                .collect(),
            ..Default::default()
        }
    }
    fn passive(key: &str) -> &'static PassiveDef {
        data::PASSIVES.iter().find(|p| p.key == key).unwrap()
    }
    #[test]
    fn pack_frenzy_is_physical_and_does_not_unlock_arcane_damage_offers() {
        use super::super::{procs, talent_runtime};
        for frenzy in [false, true] {
            let mut run = run(&["kill_command"]);
            run.class = &data::CLASSES[2];
            run.talent_build[0][8] = 1;
            run.talent_build[0][12] = 1;
            if frenzy { run.talent_build[0][18] = 1; }
            talent_runtime::configure(&mut run);
            assert!(!eligible(passive("arcane"), &run), "frenzy={frenzy}");
            let p = procs::resolve(&run, "proc_bloodburst", 100.0, 0.0, 0.0, 6.0, 64);
            assert!(p.tags.contains(engine::Tags::PHYSICAL | engine::Tags::MINION | engine::Tags::ATTACK));
            assert!(!p.tags.intersects(engine::Tags::ARCANE | engine::Tags::SPELL));
            if frenzy {
                let view = procs::views(&run).into_iter().find(|v| v.key == "proc_bloodburst").unwrap();
                assert!(view.lines[1].contains("Attack"));
                assert!(!view.lines[1].contains("Spell"));
                let def = procs::available(&run).into_iter().find(|d| d.key == "proc_bloodburst").unwrap();
                let tags = procs::gameplay_tags(&run, def.key, engine::tags::for_spell(&def));
                assert_eq!(tags, p.tags);
            }
            for key in ["arcane_shot", "volley", "concussive_shot"] {
                run.abilities.push(Owned { def: data::ability(key), level: 1, cd: 0.0 });
                assert!(eligible(passive("arcane"), &run), "{key}");
                run.abilities.pop();
            }
        }
    }

    #[test]
    fn legendary_chances_stack_and_are_applied_when_casting() {
        for (key, spell, index) in [
            ("projectiles", "fireball", 0),
            ("melee_targets", "heroic_strike", 1),
            ("chains", "fireball", 2),
            ("fork", "fireball", 3),
        ] {
            let mut run = run(&[spell]);
            run.passives.push((passive(key), 6));
            sync(&mut run);
            assert!(
                (run.resolve_spell(data::ability(spell), 1).extra_chances[index] - 1.2).abs()
                    < 0.001
            );
            assert!(eligible(passive(key), &run));
            let cast = run.resolve_spell_for_cast(
                data::ability(spell),
                1,
                &mut super::super::Rng::default(),
            );
            match index {
                0 | 1 => assert!(cast.count >= 2),
                2 => assert!(cast.chains >= 1),
                _ => assert!(cast.forks >= 1),
            }
            assert_eq!(cast.extra_chances, [0.0; 4]);
        }
    }
    #[test]
    fn repeat_passives_remain_useful_above_one_hundred_percent() {
        let mut run = run(&["fireball"]);
        run.passives.push((passive("recast"), 25));
        sync(&mut run);
        assert!((run.resolve_spell(data::ability("fireball"), 1).recast - 1.0).abs() < 0.001);
        assert!(eligible(passive("recast"), &run));
        run.passives[0].1 = 50;
        sync(&mut run);
        assert!((run.resolve_spell(data::ability("fireball"), 1).recast - 2.0).abs() < 0.001);
        assert!(eligible(passive("recast"), &run));
    }
    #[test]
    fn physical_melee_never_receives_fire_projectiles_fork_or_chain_rewards() {
        let run = run(&["heroic_strike"]);
        for key in [
            "fire",
            "projectiles",
            "fork",
            "chains",
            "spell",
            "amplify",
            "dot",
        ] {
            assert!(!eligible(passive(key), &run), "{key}");
        }
        for key in [
            "physical",
            "attack",
            "melee_targets",
            "recast",
            "wisdom",
            "pack",
        ] {
            assert!(eligible(passive(key), &run), "{key}");
        }
    }
    #[test]
    fn rewards_follow_owned_spells_rather_than_the_class_pool() {
        let mut run = run(&["heroic_strike"]);
        run.class = &data::CLASSES[6];
        assert!(!eligible(passive("fire"), &run));
        run.abilities.push(Owned {
            def: data::ability("fireball"),
            level: 1,
            cd: 0.0,
        });
        for key in ["fire", "projectiles", "fork", "chains", "amplify", "spell"] {
            assert!(eligible(passive(key), &run), "{key}");
        }
    }
    #[test]
    fn utility_and_dot_only_builds_do_not_receive_useless_damage_or_crit_bonuses() {
        assert!(!eligible(passive("kings"), &run(&["corruption"])));
        assert!(eligible(passive("dot"), &run(&["corruption"])));
        assert!(eligible(passive("holy"), &run(&["power_word_shield"])));
        assert!(eligible(passive("duration"), &run(&["power_word_shield"])));
        assert!(!eligible(passive("kings"), &run(&["thorns"])));
    }
    #[test]
    fn picking_upgrading_and_removing_a_passive_updates_one_engine_source() {
        let mut run = run(&["fireball", "heroic_strike"]);
        run.passives.push((passive("projectiles"), 1));
        sync(&mut run);
        assert_eq!(
            run.resolve_spell(data::ability("fireball"), 1)
                .extra_chances[0],
            0.2
        );
        assert_eq!(
            run.resolve_spell(data::ability("heroic_strike"), 1).count,
            1
        );
        run.passives[0].1 = 2;
        sync(&mut run);
        sync(&mut run);
        assert_eq!(
            run.resolve_spell(data::ability("fireball"), 1)
                .extra_chances[0],
            0.4
        );
        run.passives.clear();
        sync(&mut run);
        assert_eq!(run.resolve_spell(data::ability("fireball"), 1).count, 1);
    }
    #[test]
    fn capped_mechanics_do_not_produce_wasted_rewards() {
        let mut run = run(&["fireball"]);
        run.replace_spell_modifiers(
            123,
            [
                Modifier {
                    source: 123,
                    filter: Filter::default(),
                    change: Change::CriticalChance(1.0),
                },
                Modifier {
                    source: 123,
                    filter: Filter::default(),
                    change: Change::Fork(3),
                },
            ],
        );
        assert!(!eligible(passive("kings"), &run));
        assert!(eligible(passive("fork"), &run));
    }
    #[test]
    fn damage_stacks_continue_past_previous_rank_limits() {
        let mut run = run(&["fireball"]);
        run.passives.push((passive("fire"), 300));
        sync(&mut run);
        let damage = engine::resolve(
            data::ability("fireball"),
            1,
            &run.stats(),
            run.spell_modifiers.entries(),
        )
        .dmg;
        assert!((damage - 740.0).abs() < 0.01);
        assert!(eligible(passive("fire"), &run));
        assert!(matches!(
            Change::AdditionalProjectiles(2).scaled(u64::MAX),
            Change::AdditionalProjectiles(u32::MAX)
        ));
    }
}
