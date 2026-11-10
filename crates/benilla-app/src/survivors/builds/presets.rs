//! Card priorities follow this game's authored talent identities, not a retail rotation.
use super::*;

struct Preset {
    spells: &'static [&'static str],
    passives: &'static [&'static str],
}
macro_rules! preset {
    ([$($spell:literal),+], [$($passive:literal),+]) => {
        Preset { spells: &[$($spell),+], passives: &[$($passive),+] }
    };
}
// Same class/tree order as talents::TREES. The first three spells are the main commitments.
const PRESETS: [[Preset; 3]; 9] = [
    [
        preset!(
            [
                "mortal_strike",
                "cleave",
                "heroic_strike",
                "sweeping_strikes",
                "overpower",
                "battle_shout"
            ],
            [
                "physical",
                "attack",
                "melee_targets",
                "kings",
                "crit_multi",
                "recast",
                "attack_range"
            ]
        ),
        preset!(
            [
                "whirlwind",
                "bloodthirst",
                "rend",
                "sweeping_strikes",
                "execute",
                "battle_shout"
            ],
            [
                "dot",
                "physical",
                "attack",
                "duration",
                "melee_targets",
                "intellect",
                "recast"
            ]
        ),
        preset!(
            [
                "thunder_clap",
                "revenge",
                "retaliation",
                "demoralizing_shout",
                "commanding_shout",
                "cleave"
            ],
            [
                "wild",
                "fortitude",
                "amplify",
                "physical",
                "intellect",
                "renew",
                "dodge"
            ]
        ),
    ],
    [
        preset!(
            [
                "holy_shock",
                "exorcism",
                "holy_wrath",
                "holy_light",
                "blessing_of_might",
                "consecration"
            ],
            [
                "holy",
                "spell",
                "intellect",
                "recast",
                "amplify",
                "buff_effect",
                "fortitude"
            ]
        ),
        preset!(
            [
                "consecration",
                "holy_shield",
                "hammer_of_the_righteous",
                "devotion_aura",
                "divine_shield",
                "hammer_of_justice"
            ],
            [
                "wild",
                "fortitude",
                "holy",
                "amplify",
                "intellect",
                "buff_effect",
                "renew"
            ]
        ),
        preset!(
            [
                "judgement",
                "hammer_of_wrath",
                "seal_of_righteousness",
                "hammer_of_the_righteous",
                "blessing_of_might",
                "consecration"
            ],
            [
                "attack",
                "holy",
                "melee_targets",
                "kings",
                "crit_multi",
                "amplify",
                "recast"
            ]
        ),
    ],
    [
        preset!(
            [
                "tame_beast",
                "kill_command",
                "bestial_wrath",
                "stomp",
                "hunters_mark",
                "aspect_of_the_pack"
            ],
            [
                "minion_damage",
                "minion_attack_speed",
                "dot",
                "duration",
                "minion_movement_speed",
                "physical",
                "buff_effect"
            ]
        ),
        preset!(
            [
                "aimed_shot",
                "multi_shot",
                "volley",
                "arcane_shot",
                "hunters_mark",
                "concussive_shot"
            ],
            [
                "projectiles",
                "chains",
                "fork",
                "kings",
                "crit_multi",
                "attack",
                "physical"
            ]
        ),
        preset!(
            [
                "explosive_trap",
                "serpent_sting",
                "frost_trap",
                "wing_clip",
                "mongoose_bite",
                "hunters_mark"
            ],
            [
                "dot",
                "fire",
                "duration",
                "amplify",
                "intellect",
                "wild",
                "fortitude"
            ]
        ),
    ],
    [
        preset!(
            [
                "fan_of_knives",
                "sinister_strike",
                "rupture",
                "garrote",
                "eviscerate",
                "slice_and_dice"
            ],
            [
                "dot",
                "duration",
                "attack",
                "melee_targets",
                "physical",
                "amplify",
                "intellect"
            ]
        ),
        preset!(
            [
                "blade_flurry",
                "sinister_strike",
                "slice_and_dice",
                "fan_of_knives",
                "eviscerate",
                "hemorrhage"
            ],
            [
                "melee_targets",
                "attack",
                "physical",
                "recast",
                "amplify",
                "intellect",
                "attack_range"
            ]
        ),
        preset!(
            [
                "backstab",
                "eviscerate",
                "sinister_strike",
                "slice_and_dice",
                "hemorrhage",
                "kidney_shot"
            ],
            [
                "kings",
                "crit_multi",
                "attack",
                "physical",
                "recast",
                "intellect",
                "melee_targets"
            ]
        ),
    ],
    [
        preset!(
            [
                "power_word_shield",
                "penance",
                "smite",
                "prayer_of_mending",
                "holy_nova",
                "renew"
            ],
            [
                "fortitude",
                "intellect",
                "holy",
                "amplify",
                "recast",
                "wild",
                "spell"
            ]
        ),
        preset!(
            [
                "holy_nova",
                "holy_fire",
                "penance",
                "prayer_of_mending",
                "renew",
                "smite"
            ],
            [
                "holy",
                "spell",
                "amplify",
                "intellect",
                "recast",
                "fortitude",
                "renew"
            ]
        ),
        preset!(
            [
                "shadow_word_pain",
                "mind_flay",
                "devouring_plague",
                "mind_blast",
                "psychic_scream",
                "power_word_shield"
            ],
            [
                "shadow",
                "dot",
                "duration",
                "spell",
                "intellect",
                "amplify",
                "recast"
            ]
        ),
    ],
    [
        preset!(
            [
                "chain_lightning",
                "lightning_bolt",
                "flame_shock",
                "earth_shock",
                "frost_shock",
                "lightning_shield"
            ],
            [
                "chains",
                "projectiles",
                "fork",
                "lightning",
                "spell",
                "recast",
                "intellect"
            ]
        ),
        preset!(
            [
                "stormstrike",
                "lava_lash",
                "lightning_shield",
                "earth_shock",
                "strength_of_earth_totem",
                "frost_shock"
            ],
            [
                "attack",
                "melee_targets",
                "lightning",
                "fire",
                "recast",
                "kings",
                "crit_multi"
            ]
        ),
        preset!(
            [
                "healing_stream_totem",
                "magma_totem",
                "totemic_recall",
                "searing_totem",
                "strength_of_earth_totem",
                "lesser_healing_wave"
            ],
            [
                "totem_limit_legendary",
                "totem_limit_epic",
                "totem_power",
                "totem_recovery",
                "totem_duration",
                "totem_area",
                "buff_effect"
            ]
        ),
    ],
    [
        preset!(
            [
                "arcane_missiles",
                "arcane_explosion",
                "arcane_intellect",
                "ice_barrier",
                "frost_nova",
                "fire_blast"
            ],
            [
                "recast",
                "arcane",
                "spell",
                "projectiles",
                "chains",
                "intellect",
                "amplify"
            ]
        ),
        preset!(
            [
                "fireball",
                "pyroblast",
                "living_bomb",
                "flamestrike",
                "fire_blast",
                "blast_wave",
                "scorch"
            ],
            [
                "fire",
                "dot",
                "duration",
                "spell",
                "amplify",
                "projectiles",
                "recast"
            ]
        ),
        preset!(
            [
                "blizzard",
                "frostbolt",
                "frost_nova",
                "cone_of_cold",
                "ice_barrier",
                "frost_guardian",
                "ice_armor"
            ],
            [
                "cold",
                "amplify",
                "duration",
                "spell",
                "projectiles",
                "intellect",
                "recast"
            ]
        ),
    ],
    [
        preset!(
            [
                "corruption",
                "curse_of_agony",
                "seed_of_corruption",
                "siphon_life",
                "drain_life",
                "death_coil"
            ],
            [
                "dot",
                "shadow",
                "duration",
                "spell",
                "amplify",
                "intellect",
                "recast"
            ]
        ),
        preset!(
            [
                "hellfire",
                "seed_of_corruption",
                "shadow_bolt",
                "demon_armor",
                "curse_of_elements",
                "soul_burn"
            ],
            [
                "minion_damage",
                "minion_attack_speed",
                "fire",
                "dot",
                "amplify",
                "minion_movement_speed",
                "duration"
            ]
        ),
        preset!(
            [
                "shadowburn",
                "rain_of_fire",
                "soul_burn",
                "immolate",
                "searing_pain",
                "curse_of_elements"
            ],
            [
                "kings",
                "crit_multi",
                "spell",
                "fire",
                "shadow",
                "amplify",
                "intellect",
                "recast"
            ]
        ),
    ],
    [
        preset!(
            [
                "starfire",
                "moonfire",
                "hurricane",
                "wrath",
                "insect_swarm",
                "mark_of_the_wild"
            ],
            [
                "arcane",
                "projectiles",
                "spell",
                "dot",
                "chains",
                "amplify",
                "recast"
            ]
        ),
        preset!(
            [
                "rake",
                "swipe",
                "mangle",
                "rip",
                "ferocious_bite",
                "claw",
                "savage_roar",
                "barkskin"
            ],
            [
                "dot",
                "physical",
                "attack",
                "melee_targets",
                "duration",
                "wild",
                "recast"
            ]
        ),
        preset!(
            [
                "rejuvenation",
                "regrowth",
                "efflorescence",
                "mark_of_the_wild",
                "insect_swarm",
                "barkskin"
            ],
            [
                "buff_effect",
                "nature",
                "intellect",
                "duration",
                "fortitude",
                "wild",
                "renew"
            ]
        ),
    ],
];

pub(super) fn seed(store: &mut Store) -> bool {
    let mut changed = false;
    for (class_index, class) in data::CLASSES.iter().enumerate() {
        for (tree, preset) in PRESETS[class_index].iter().enumerate() {
            let key = format!("{}:{tree}", class.id);
            if store.seeded_presets.contains(&key) {
                continue;
            }
            let Some(id) = store
                .builds
                .iter()
                .map(|b| b.id)
                .max()
                .unwrap_or(0)
                .checked_add(1)
            else {
                return changed;
            };
            let mut spells: Vec<String> = preset.spells.iter().map(|k| (*k).into()).collect();
            let mut passives: Vec<String> = preset.passives.iter().map(|k| (*k).into()).collect();
            reconcile(&mut spells, &spell_keys(class));
            reconcile(&mut passives, &passive_keys());
            store.builds.push(Build {
                id,
                class: class.id,
                name: format!(
                    "{} (Preset)",
                    crate::survivors::talents::TREES[class_index][tree].name
                ),
                spells,
                passives,
            });
            store.seeded_presets.insert(key);
            changed = true;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_specialization_has_valid_complete_and_distinct_priorities() {
        assert_eq!(PRESETS.len(), data::CLASSES.len());
        let mut store = Store::default();
        assert!(seed(&mut store));
        assert_eq!(store.builds.len(), 27);
        assert!(store.active.is_empty());
        for (i, class) in data::CLASSES.iter().enumerate() {
            let builds: Vec<_> = store
                .builds
                .iter()
                .filter(|b| b.class == class.id)
                .collect();
            assert_eq!(builds.len(), 3);
            for (tree, preset) in PRESETS[i].iter().enumerate() {
                let allowed = spell_keys(class);
                for key in preset.spells {
                    assert!(
                        allowed.iter().any(|s| s == key),
                        "{} {}: {key}",
                        class.name,
                        tree
                    );
                }
                for key in preset.passives {
                    assert!(data::PASSIVES.iter().any(|p| p.key == *key), "{key}");
                }
                assert_eq!(builds[tree].spells.len(), allowed.len());
                assert_eq!(builds[tree].passives.len(), passive_keys().len());
                assert_eq!(builds[tree].spells[0], preset.spells[0]);
                assert_eq!(builds[tree].passives[0], preset.passives[0]);
                assert!(
                    builds[tree]
                        .name
                        .starts_with(crate::survivors::talents::TREES[i][tree].name)
                );
            }
            assert_ne!(builds[0].spells, builds[1].spells);
            assert_ne!(builds[1].spells, builds[2].spells);
        }
    }
    #[test]
    fn seeding_preserves_custom_builds_edits_deletions_and_explicit_activation() {
        let mut store = Store::fixture(&data::CLASSES[3]);
        let custom = store.active[&data::CLASSES[3].id];
        let custom_order = store.builds[0].spells.clone();
        assert!(seed(&mut store));
        assert_eq!(store.builds.len(), 28);
        assert_eq!(store.active[&data::CLASSES[3].id], custom);
        assert_eq!(store.builds[0].spells, custom_order);
        let deleted = store.builds[1].id;
        let edited = store.builds[2].id;
        store.reorder(edited, ui::Tab::Spells, 1, 0);
        let order = store
            .builds
            .iter()
            .find(|b| b.id == edited)
            .unwrap()
            .spells
            .clone();
        store.delete(deleted);
        let mut loaded = Store::decode(&serde_json::to_string(&store).unwrap()).unwrap();
        assert!(!seed(&mut loaded));
        assert_eq!(loaded.builds.len(), 27);
        assert!(!loaded.builds.iter().any(|b| b.id == deleted));
        assert_eq!(
            loaded
                .builds
                .iter()
                .find(|b| b.id == edited)
                .unwrap()
                .spells,
            order
        );
        assert_eq!(loaded.active[&data::CLASSES[3].id], custom);
    }
}
