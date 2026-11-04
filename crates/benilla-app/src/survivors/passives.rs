//! Run rewards are generic stats or tag-filtered engine sources with a useful owned recipient.
use super::{
    data::{self, PassiveDef, PassiveEffect},
    spell_engine::{self as engine, Change, Filter, Modifier},
    Run,
};

pub(super) fn legendary(passive: &PassiveDef) -> bool {
    matches!(
        passive.effect,
        PassiveEffect::Modifier(
            _,
            Change::AdditionalProjectiles(_)
                | Change::AdditionalMeleeTargets(_)
                | Change::Fork(_)
                | Change::AdditionalChains(_)
        )
    )
}

pub(super) fn eligible(passive: &PassiveDef, run: &Run) -> bool {
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
        engine::resolve(def, 1, &stats, before) != engine::resolve(def, 1, &stats, &after)
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
        Change::AdditionalChains(_) => "Projectile / Chain".to_owned(),
        Change::RecastChance(_) => "Repeatable damaging skills".to_owned(),
        Change::CriticalChance(_) | Change::CriticalMultiplier(_) => "Damaging hits".to_owned(),
        _ => "Damaging skills".to_owned(),
    }
}

#[cfg(test)]
mod tests {
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
        assert!(!eligible(passive("holy"), &run(&["power_word_shield"])));
        assert!(eligible(passive("duration"), &run(&["power_word_shield"])));
        assert!(!eligible(passive("kings"), &run(&["thorns"])));
    }
    #[test]
    fn picking_upgrading_and_removing_a_passive_updates_one_engine_source() {
        let mut run = run(&["fireball", "heroic_strike"]);
        run.passives.push((passive("projectiles"), 1));
        sync(&mut run);
        assert_eq!(run.resolve_spell(data::ability("fireball"), 1).count, 2);
        assert_eq!(
            run.resolve_spell(data::ability("heroic_strike"), 1).count,
            1
        );
        run.passives[0].1 = 2;
        sync(&mut run);
        sync(&mut run);
        assert_eq!(run.resolve_spell(data::ability("fireball"), 1).count, 3);
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
        assert!(!eligible(passive("fork"), &run));
    }
    #[test]
    fn damage_stacks_continue_past_previous_rank_limits() {
        let mut run = run(&["fireball"]);
        run.passives.push((passive("fire"), 300));
        sync(&mut run);
        assert!((run.resolve_spell(data::ability("fireball"), 1).dmg - 740.0).abs() < 0.01);
        assert!(eligible(passive("fire"), &run));
        assert!(matches!(
            Change::AdditionalProjectiles(2).scaled(u64::MAX),
            Change::AdditionalProjectiles(u32::MAX)
        ));
    }
}
