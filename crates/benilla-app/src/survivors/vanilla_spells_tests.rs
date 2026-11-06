use super::{data, spell_engine::tags::Tags, Run};

const ADDITIONS: &[(u8, &str, &str)] = &[
    (1, "rend", "overpower"),
    (2, "holy_light", "seal_of_righteousness"),
    (3, "raptor_strike", "wing_clip"),
    (4, "backstab", "hemorrhage"),
    (5, "mind_flay", "renew"),
    (7, "stormstrike", "lesser_healing_wave"),
    (8, "scorch", "ice_barrier"),
    (9, "drain_life", "siphon_life"),
    (11, "swipe", "regrowth"),
];

#[test]
fn two_new_skills_per_class_are_learnable_classified_balanced_and_weighted() {
    let rewards: toml::Value = toml::from_str(super::reward_config::DEFAULT_FILE).unwrap();
    for &(id, first, second) in ADDITIONS {
        let class = data::CLASSES.iter().find(|class| class.id == id).unwrap();
        let run = Run {
            class,
            balance: super::balance::Config::parse("version=1\nclasses={}").unwrap(),
            ..Run::default()
        };
        for key in [first, second] {
            assert_eq!(class.pool.iter().filter(|entry| **entry == key).count(), 1);
            assert_eq!(
                data::ABILITIES.iter().filter(|def| def.key == key).count(),
                1
            );
            assert_eq!(rewards["spells"][key].as_integer(), Some(1));
            assert_eq!(run.balance.multiplier(class, key), 1.0);
            let spell = run.resolve_spell(data::ability(key), 1);
            assert!(spell.dmg > 0.0 || spell.dot > 0.0, "{key}");
            assert!(spell.cooldown > 0.0);
            let tags = super::spell_engine::tags::for_spell(data::ability(key));
            assert_eq!(
                tags.contains(Tags::ATTACK),
                matches!(id, 1 | 3 | 4) || key == "stormstrike" || key == "swipe"
            );
        }
    }
}

#[test]
fn added_spell_icons_and_visual_chains_exist_in_local_vanilla_data() {
    let path = benilla_formats::wow_data_or_skip!();
    let mut chain = benilla_formats::open_chain(&path).unwrap();
    let spells = benilla_formats::load_spell_catalog(&mut chain).unwrap();
    let visuals = benilla_formats::load_spell_visual_catalog(&mut chain).unwrap();
    let keys = ADDITIONS
        .iter()
        .chain(super::extra_spells::ADDITIONS)
        .flat_map(|&(_, first, second)| [first, second])
        .chain(super::support_spells::SPELLS.iter().map(|s| s.1));
    for key in keys {
        let def = data::ability(key);
        let display = spells
            .get(def.spell)
            .unwrap_or_else(|| panic!("missing Vanilla spell {key}"));
        assert!(!display.name.is_empty());
        let icon = display.icon.as_ref().unwrap();
        let icon = if icon.to_ascii_lowercase().ends_with(".blp") {
            icon.clone()
        } else {
            format!("{icon}.blp")
        };
        let bytes = chain
            .read_file(&icon)
            .unwrap_or_else(|error| panic!("{key} icon: {error}"));
        benilla_formats::blp_to_rgba(&bytes).unwrap();
        if display.visual != 0 {
            let stages = visuals
                .stages(display.visual)
                .unwrap_or_else(|| panic!("missing visual for {key}"));
            for kit in [
                stages.precast,
                stages.cast,
                stages.impact,
                stages.state,
                stages.channel,
            ] {
                if kit == 0 {
                    continue;
                }
                let kit = visuals.kit(kit).unwrap();
                for (_, effect) in kit.effects() {
                    if effect == 0 {
                        continue;
                    }
                    if let Some(path) = visuals.effect_path(effect) {
                        let path = if let Some(stem) = path
                            .strip_suffix(".mdx")
                            .or_else(|| path.strip_suffix(".MDX"))
                        {
                            format!("{stem}.m2")
                        } else {
                            path.to_owned()
                        };
                        chain
                            .read_file(&path)
                            .unwrap_or_else(|error| panic!("{key} model {path}: {error}"));
                    }
                }
            }
        } else {
            assert!(
                matches!(def.kind, data::Kind::Melee | data::Kind::Buff),
                "spell {key} requires a visual or a weapon swing"
            );
        }
    }
}
