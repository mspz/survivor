//! First milestones adapt the starting slot to the main specialization.
use super::{data, talent_design, talents, Run};

pub(super) fn replacement(spec: &talents::Spec) -> Option<&'static str> {
    Some(match talent_design::index(spec) {
        2 => "thunder_clap",
        3 => "holy_shock",
        4 => "consecration",
        5 => "judgement",
        6 => "kill_command",
        8 => "explosive_trap",
        14 => "shadow_word_pain",
        16 => "stormstrike",
        17 => "searing_totem",
        18 => "arcane_missiles",
        19 => "fireball",
        21 => "corruption",
        24 => "starfire",
        25 => "claw",
        // Bloom supplies healing; this keeps a Nature damage spell for solo play.
        26 => "insect_swarm",
        _ => return None,
    })
}

pub(super) fn sync(run: &mut Run, class: usize) {
    let mut main = None;
    let mut points = 0;
    for tree in 0..3 {
        if run.talent_build[tree][8] == 0 {
            continue;
        }
        let spent: u32 = run.talent_build[tree]
            .iter()
            .map(|&rank| u32::from(rank))
            .sum();
        if spent > points {
            main = Some(tree);
            points = spent;
        }
    }
    let next = main
        .and_then(|tree| replacement(&talents::TREES[class][tree]))
        .unwrap_or(run.class.start);
    let previous = run.starter_spell.unwrap_or(run.class.start);
    if next == previous {
        return;
    }
    let Some(index) = run.abilities.iter().position(|a| a.def.key == previous) else {
        return;
    };
    let mut starter = run.abilities.remove(index);
    starter.def = data::ability(next);
    // A milestone may already have unlocked this skill: merge without losing upgrades.
    if let Some(duplicate) = run.abilities.iter().position(|a| a.def.key == next) {
        let existing = run.abilities.remove(duplicate);
        starter.level = starter.level.max(existing.level);
        starter.cd = starter.cd.min(existing.cd);
    }
    run.abilities
        .insert(index.min(run.abilities.len()), starter);
    run.starter_spell = Some(next);
    run.version = run.version.wrapping_add(1);
}

pub(super) fn tooltip(spec: &talents::Spec) -> Option<String> {
    let next = replacement(spec)?;
    let class = talent_design::index(spec) / 3;
    let title = |key: &str| {
        key.split('_')
            .map(|part| {
                let mut chars = part.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    };
    Some(format!(
        "Replaces {} with {} as your starting spell.",
        title(data::CLASSES[class].start),
        title(next)
    ))
}

#[cfg(test)]
mod tests {
    use super::super::{talent_runtime, Owned};
    use super::*;
    fn run(class: usize) -> Run {
        let mut run = Run {
            class: &data::CLASSES[class],
            ..Default::default()
        };
        run.abilities.push(Owned {
            def: data::ability(run.class.start),
            level: 4,
            cd: 0.3,
        });
        run
    }
    #[test]
    fn every_replacement_is_owned_once_and_preserves_starting_rank() {
        for class in 0..9 {
            for tree in 0..3 {
                let Some(key) = replacement(&talents::TREES[class][tree]) else {
                    continue;
                };
                let mut run = run(class);
                run.talent_build[tree][8] = 1;
                talent_runtime::configure(&mut run);
                talent_runtime::configure(&mut run);
                assert_eq!(run.abilities[0].def.key, key);
                assert_eq!(run.abilities[0].level, 4);
                assert_eq!(run.abilities[0].cd, 0.3);
                assert_eq!(run.abilities.iter().filter(|a| a.def.key == key).count(), 1);
                assert!(!run.abilities.iter().any(|a| a.def.key == run.class.start));
            }
        }
    }
    #[test]
    fn feral_requires_milestone_and_respec_restores_wrath() {
        let mut run = run(8);
        run.talent_build[1][0] = 5;
        talent_runtime::configure(&mut run);
        assert_eq!(run.abilities[0].def.key, "wrath");
        run.talent_build[1][8] = 1;
        talent_runtime::configure(&mut run);
        assert_eq!(run.abilities[0].def.key, "claw");
        assert!(
            super::super::spell_engine::tags::for_spell(run.abilities[0].def)
                .contains(super::super::spell_engine::Tags::MELEE)
        );
        run.talent_build = [[0; talents::NODES]; 3];
        talent_runtime::configure(&mut run);
        assert_eq!(run.abilities[0].def.key, "wrath");
        assert_eq!(run.abilities[0].level, 4);
    }
    #[test]
    fn main_tree_wins_and_existing_unlocks_merge_without_losing_rank() {
        let mut run = run(6);
        run.talent_build[0][8] = 1;
        run.talent_build[1][8] = 1;
        run.abilities.push(Owned {
            def: data::ability("arcane_missiles"),
            level: 7,
            cd: 0.1,
        });
        talent_runtime::configure(&mut run);
        assert_eq!(run.abilities[0].def.key, "arcane_missiles");
        assert_eq!(run.abilities[0].level, 7);
        run.talent_build[1][0] = 5;
        talent_runtime::configure(&mut run);
        assert_eq!(run.abilities[0].def.key, "fireball");
        assert_eq!(run.abilities[0].level, 7);
    }
}
