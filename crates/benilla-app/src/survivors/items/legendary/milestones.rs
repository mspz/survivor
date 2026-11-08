//! Milestone identity includes the donor class, specialization and tier.
use super::*;
use crate::survivors::{
    data,
    talent_design::{self, Mechanic as M},
    Owned,
};
#[derive(Clone, Copy)]
pub(in crate::survivors) struct Grant {
    pub option: u8,
}
impl Grant {
    pub fn design(self) -> &'static talent_design::Design {
        &talent_design::DESIGNS[self.option as usize / 3]
    }
    pub fn milestone(self) -> &'static talent_design::Milestone {
        &self.design().milestones[self.option as usize % 3]
    }
    pub fn class(self) -> u8 {
        data::CLASSES[self.option as usize / 9].id
    }
    pub fn tree(self) -> usize {
        self.option as usize / 3 % 3
    }
    pub fn node(self) -> usize {
        [8, 12, 18][self.option as usize % 3]
    }
}
fn named(name: &str) -> Option<Grant> {
    talent_design::DESIGNS
        .iter()
        .flat_map(|d| d.milestones.iter())
        .position(|m| m.name == name)
        .map(|option| Grant {
            option: option as u8,
        })
}
pub(in crate::survivors) fn sources(run: &Run, mechanic: M) -> Vec<u8> {
    let mut classes = Vec::new();
    if let Some(index) = data::CLASSES.iter().position(|c| c.id == run.class.id) {
        for tree in 0..3 {
            for (tier, node) in [8, 12, 18].into_iter().enumerate() {
                if run.talent_build[tree][node] > 0
                    && talent_design::DESIGNS[index * 3 + tree].milestones[tier].mechanic
                        == mechanic
                {
                    classes.push(run.class.id);
                }
            }
        }
    }
    for grant in &run.legendary.grants {
        if grant.milestone().mechanic == mechanic
            || matches!(
                (grant.milestone().mechanic, mechanic),
                (M::SealStorm, M::Seal)
                    | (M::Grove, M::Hot)
                    | (M::ShieldHeal | M::ShieldNova, M::Shieldburst)
                    | (M::Demons | M::DemonLord, M::Demon)
                    | (M::Pack, M::Pet)
                    | (M::DualTotem | M::SpiritTotem, M::HealTotem)
                    | (M::SpiritTotem, M::DualTotem)
            )
        {
            classes.push(grant.class());
        }
    }
    classes.sort_unstable();
    classes.dedup();
    classes
}
pub(in crate::survivors) fn active(run: &Run, m: M) -> bool {
    run.legendary.grants.iter().any(|grant| {
        let direct = grant.milestone().mechanic;
        direct == m
            || matches!(
                (direct, m),
                (M::Pack, M::Pet)
                    | (M::Demons | M::DemonLord, M::Demon)
                    | (M::DualTotem | M::SpiritTotem, M::HealTotem)
                    | (M::SpiritTotem, M::DualTotem)
                    | (M::Grove, M::Hot)
                    | (M::ShieldHeal | M::ShieldNova, M::Shieldburst)
                    | (M::SealStorm, M::Seal)
            )
    })
}
pub(in crate::survivors) fn source_class(run: &Run, m: M) -> u8 {
    sources(run, m).first().copied().unwrap_or(run.class.id)
}
pub(in crate::survivors) fn source_name(class: u8, mechanic: M) -> Option<&'static str> {
    let index = data::CLASSES.iter().position(|c| c.id == class)?;
    talent_design::DESIGNS[index * 3..index * 3 + 3]
        .iter()
        .flat_map(|d| d.milestones.iter())
        .find(|m| m.mechanic == mechanic)
        .map(|m| m.name)
}
pub(in crate::survivors) fn equip(run: &mut Run) {
    let mut grants = Vec::new();
    for roll in run.legendary.equipped.values() {
        let grant = match roll.unique {
            36 => named("Wild Companion"),
            47 => named("Thunder Fortress"),
            55 => named("Seal of Righteousness"),
            65 => named("Deep Wounds"),
            75 => named("Contagion"),
            81 => named("Windfury Weapon"),
            87 => named("Living Grove"),
            91 => named("Demonic Pact"),
            301 => roll.milestone.map(|option| Grant { option }),
            _ => None,
        };
        if let Some(grant) = grant {
            let learned =
                grant.class() == run.class.id && run.talent_build[grant.tree()][grant.node()] > 0;
            if !learned && !grants.iter().any(|g: &Grant| g.option == grant.option) {
                grants.push(grant);
            }
        }
    }
    run.legendary.grants = grants;
    let mut modifiers = Vec::new();
    let mut unlocks = Vec::new();
    for grant in &run.legendary.grants {
        for change in grant.milestone().changes {
            let tags = if matches!(change, Change::BleedChance(_)) {
                if grant.design().filter.contains(Tags::MELEE) {
                    Tags::PHYSICAL | Tags::MELEE
                } else {
                    Tags::PHYSICAL
                }
            } else {
                grant.design().filter
            };
            modifiers.push(Modifier {
                source: 0x6100_0000,
                filter: Filter::requiring(tags),
                change: *change,
            });
        }
        match grant.milestone().mechanic {
            M::Taunt => unlocks.extend([
                "taunt",
                if grant.class() == 1 {
                    "thunder_clap"
                } else {
                    "consecration"
                },
            ]),
            M::Shieldburst | M::ShieldHeal | M::ShieldNova => unlocks.push("power_word_shield"),
            _ => {}
        }
        match (grant.class(), grant.tree(), grant.node()) {
            (3, 2, 8) => unlocks.push("explosive_trap"),
            (2, 0, 8) => unlocks.extend(["holy_shock", "exorcism", "holy_light"]),
            (5, 1, 8) => unlocks.extend(["flash_heal", "holy_nova", "holy_fire"]),
            (11, 0, 8) => unlocks.extend(["starfire", "moonfire"]),
            (11, 1, 8) => unlocks.extend(["claw", "swipe"]),
            (11, 2, 8) => unlocks.push("rejuvenation"),
            _ => {}
        }
    }
    run.replace_spell_modifiers(0x6100_0000, modifiers);
    let previous = std::mem::take(&mut run.legendary.grant_spells);
    let mut removed = false;
    for key in previous {
        if unlocks.contains(&key) {
            run.legendary.grant_spells.insert(key);
        } else {
            let native = match key {
                "taunt" | "thunder_clap" => run.class.id == 1 && run.talent_build[2][8] > 0,
                "consecration" => run.class.id == 2 && run.talent_build[1][8] > 0,
                "power_word_shield" => run.class.id == 5 && run.talent_build[0][8] > 0,
                "holy_shock" | "exorcism" | "holy_light" => {
                    run.class.id == 2 && run.talent_build[0][8] > 0
                }
                "flash_heal" | "holy_nova" | "holy_fire" => {
                    run.class.id == 5 && run.talent_build[1][8] > 0
                }
                "raptor_strike" => run.class.id == 3 && run.talent_build[2][8] > 0,
                "explosive_trap" => run.class.id == 3 && run.talent_build[2][8] > 0,
                "starfire" | "moonfire" => run.class.id == 11 && run.talent_build[0][8] > 0,
                "claw" | "swipe" => run.class.id == 11 && run.talent_build[1][8] > 0,
                "rejuvenation" => run.class.id == 11 && run.talent_build[2][8] > 0,
                _ => false,
            };
            if !native {
                run.abilities.retain(|o| o.def.key != key);
                removed = true;
            }
        }
    }
    if removed {
        run.recasts = default();
    }
    for key in unlocks {
        if !run.abilities.iter().any(|o| o.def.key == key) {
            run.legendary.grant_spells.insert(key);
            run.abilities.push(Owned {
                def: data::ability(key),
                level: 1,
                cd: 0.5,
            });
        }
    }
}
pub(in crate::survivors) fn description(option: u8) -> String {
    let g = Grant { option };
    format!(
        "{} — {} — tier {}: {}\n{}",
        data::CLASSES[option as usize / 9].name,
        g.design().title,
        g.node(),
        g.milestone().name,
        g.milestone().description
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_81_archive_variants_resolve_and_unequip_cleanly() {
        for option in 0..81 {
            let mut run = Run::default();
            let native = run.abilities.len();
            run.legendary.equipped.insert(
                301,
                Roll {
                    unique: 301,
                    milestone: Some(option),
                },
            );
            equip(&mut run);
            assert_eq!(run.legendary.grants.len(), 1, "{option}");
            let grant = run.legendary.grants[0];
            assert!(active(&run, grant.milestone().mechanic));
            assert!(description(option).contains(grant.milestone().name));
            for key in run.class.pool {
                let p = run.resolve_spell(data::ability(key), 1);
                assert!(
                    p.dmg.is_finite() && p.cooldown.is_finite(),
                    "{option}: {key}"
                );
            }
            run.legendary.equipped.clear();
            equip(&mut run);
            assert!(run.legendary.grants.is_empty());
            assert_eq!(
                run.abilities.len(),
                native,
                "archive {option} leaked spells"
            );
        }
    }
    #[test]
    fn exact_native_milestone_does_not_stack_but_foreign_seal_preserves_both_sources() {
        let mut run = Run {
            class: &data::CLASSES[5],
            ..default()
        };
        let native = named("Flametongue Weapon")
            .or_else(|| {
                talent_design::DESIGNS[15..18]
                    .iter()
                    .flat_map(|d| d.milestones.iter())
                    .find(|m| m.mechanic == M::Seal)
                    .and_then(|m| named(m.name))
            })
            .unwrap();
        run.talent_build[native.tree()][native.node()] = 1;
        run.legendary.equipped.insert(
            301,
            Roll {
                unique: 301,
                milestone: Some(native.option),
            },
        );
        equip(&mut run);
        assert!(run.legendary.grants.is_empty());
        run.legendary.equipped.insert(
            55,
            Roll {
                unique: 55,
                milestone: None,
            },
        );
        equip(&mut run);
        assert_eq!(sources(&run, M::Seal), vec![2, 7]);
    }
}
