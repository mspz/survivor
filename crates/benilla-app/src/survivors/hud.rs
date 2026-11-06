//! The mode's screens, in bevy_ui since FrameXML is off: the class and hero pick, the in-run HUD,
//! the level-up cards, pause, and the results.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::ui_render::ui_material::{MaterialNode, UiMaterial};
use bevy::window::PrimaryWindow;

use super::data::{self, AbilityDef, Effect, Kind, PassiveDef, Stat};
use super::hero::{self, Hero};
use super::minimap::{self, MiniMap};
use super::{Owned, Phase, Rng, Run, Selection, Stats};

const GOLD: Color = Color::srgb(1.0, 0.82, 0.0);
const SILVER: Color = Color::srgb(0.78, 0.82, 0.88);
/// The mobility button's edge, an action button's 36 units.
const MOBILITY_SIZE: f32 = 40.0;
const EDGE: Color = Color::srgb(0.55, 0.45, 0.25);
const DIM: Color = Color::srgb(0.6, 0.6, 0.6);
pub(super) const DIM_GREY: Color = DIM;
pub(super) const GOLD_TONE: Color = GOLD;

#[derive(Component)]
pub(super) struct MenuRoot;
#[derive(Component)]
pub(super) struct HudRoot;
#[derive(Component)]
pub(super) struct LevelUpRoot;
#[derive(Component)]
pub(super) struct PauseRoot;
#[derive(Component)]
pub(super) struct OverRoot;
#[derive(Component)]
pub(super) struct XpBar;
#[derive(Component)]
pub(super) struct XpFill;
#[derive(Component)]
pub(super) struct XpText;
#[derive(Component)]
pub(super) struct LevelText;
#[derive(Component)]
pub(super) struct TimerText;
#[derive(Component)]
pub(super) struct KillText;
#[derive(Component)]
pub(super) struct BannerText;
#[derive(Component)]
pub(super) struct AbilityRow;
#[derive(Component)]
pub(super) struct PassiveRow;
#[derive(Component)]
pub(super) struct OfferRow;
/// The line under LEVEL UP: which level this is.
#[derive(Component)]
pub(super) struct LevelUpSub;

/// A radial cooldown sweep over one ability icon: `sweep` is the remaining fraction
/// (1.0 = just cast, 0.0 = ready), drawn by `ui_sweep.wgsl`.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub(super) struct SweepUiMaterial {
    #[uniform(0)]
    pub(super) sweep: f32,
}

#[cfg(test)]
mod passive_offer_tests {
    #[test]
    fn specialized_upgrade_cards_keep_the_same_compact_stat_deltas_as_ordinary_ranks() {
        let mut run = run("fireball");
        for rank in 1..=5 {
            run.abilities[0].level = rank;
            let card = describe_card(Choice::Upgrade(0), &run, None);
            assert!(card.tag.starts_with("Upgrade | Rank"));
            assert!(
                card.desc.lines().all(|line| line.contains("»")),
                "{}",
                card.desc
            );
            assert!(!card.desc.contains("Specialize:"));
            assert!(!card.desc.contains("Choose your"));
        }
    }
    #[test]
    fn passive_cards_show_compact_cumulative_before_after_values() {
        let mut run = run("fireball");
        for passive in data::PASSIVES {
            let new = describe_card(Choice::NewPassive(passive), &run, None);
            assert!(new.desc.contains("»"), "{}", passive.key);
            assert!(new.desc.lines().count() <= 2, "{}", passive.key);
            assert!(!new.desc.contains("Applies to:"));
            assert!(!new.tag.contains("Unlimited"));
        }
        let buff = data::PASSIVES
            .iter()
            .find(|p| p.key == "buff_effect")
            .unwrap();
        run.passives.push((buff, 2));
        let card = describe_card(Choice::NumericPassive(buff, 3), &run, None);
        assert!(card.desc.contains("+10% » +25%"));
        assert!(card.tag.contains("2 » 5"));
        let upgraded = describe_card(Choice::UpPassive(0), &run, None);
        assert!(upgraded.desc.contains("+10% » +15%"));
    }
    use super::*;
    fn only_passives(allowed: &[&str]) -> super::super::reward_config::Config {
        let mut text =
            "version=1\n[rarities]\ncommon=1\nuncommon=1\nepic=1\nlegendary=1\n[spells]\n"
                .to_owned();
        for def in data::ABILITIES {
            text.push_str(&format!("{}=0\n", def.key));
        }
        text.push_str("[passives]\n");
        for def in data::PASSIVES {
            text.push_str(&format!(
                "{}={}\n",
                def.key,
                u8::from(allowed.contains(&def.key))
            ));
        }
        super::super::reward_config::Config::parse(&text).unwrap()
    }
    #[test]
    fn learned_demon_spells_enable_real_projectile_offers_and_upgrades_then_respec_removes_them() {
        let mut run = run("heroic_strike");
        run.class = &data::CLASSES[7];
        let config = only_passives(&["projectiles", "chains", "fork"]);
        let mut rng = Rng(42);
        assert!(matches!(
            choices_for(&run, &mut rng, &config).as_slice(),
            [Choice::Heal]
        ));
        run.talent_build[1][8] = 1;
        super::super::talent_runtime::configure(&mut run);
        let choices = choices_for(&run, &mut rng, &config);
        let offered: std::collections::BTreeSet<_> = choices
            .iter()
            .map(|c| match c {
                Choice::NewPassive(p) => p.key,
                _ => panic!("expected a proc-compatible legendary card"),
            })
            .collect();
        assert_eq!(
            offered,
            ["projectiles", "chains", "fork"].into_iter().collect()
        );
        let mut offer = Offer { choices, config };
        pick(0, &mut run, &mut offer, &mut rng, &mut NextState::default());
        assert_eq!(
            run.abilities.len(),
            1,
            "hidden spells must not consume active slots"
        );
        assert!(choice_pool(&run)
            .iter()
            .any(|c| matches!(c, Choice::UpPassive(0))));
        run.talent_build = [[0; 19]; 3];
        super::super::talent_runtime::configure(&mut run);
        assert!(!choice_pool(&run)
            .iter()
            .any(|c| matches!(c, Choice::UpPassive(0))));
        assert!(matches!(
            choices_for(&run, &mut rng, &offer.config).as_slice(),
            [Choice::Heal]
        ));
    }
    #[test]
    fn talent_dots_unlock_real_numeric_offers_but_permanent_invocations_do_not_unlock_duration() {
        let mut run = run("heroic_strike");
        let config = only_passives(&["dot", "duration", "fire"]);
        let mut rng = Rng(42);
        assert!(matches!(
            choices_for(&run, &mut rng, &config).as_slice(),
            [Choice::Heal]
        ));
        run.talent_build[1][8] = 1; // Mage: Ignite, without any visible Fire or DoT ability.
        super::super::talent_runtime::configure(&mut run);
        let choices = choices_for(&run, &mut rng, &config);
        let offered: std::collections::BTreeSet<_> = choices
            .iter()
            .map(|c| match c {
                Choice::NumericPassive(p, _) => p.key,
                _ => panic!("expected a proc-compatible numeric card"),
            })
            .collect();
        assert_eq!(offered, ["dot", "duration", "fire"].into_iter().collect());
        run.class = &data::CLASSES[7]; // Warlock: a permanent imp, initially without burning hits.
        super::super::talent_runtime::configure(&mut run);
        // Global Duration can still help Howl of Terror; isolate the demon as recipient.
        let duration = PassiveDef {
            key: "demon_duration",
            name: "Demon duration",
            spell: 139,
            effect: data::PassiveEffect::Modifier(
                super::super::spell_engine::Filter {
                    skill: Some("proc_demon_bolt"),
                    ..super::super::spell_engine::Filter::requiring(
                        super::super::spell_engine::Tags::DURATION,
                    )
                },
                super::super::spell_engine::Change::IncreasedDuration(0.1),
            ),
        };
        assert!(!super::super::passives::eligible(&duration, &run));
        assert!(!keys(&run).contains(&"dot"));
        run.talent_build[1][12] = 1; // Legion adds temporary burning damage to demon hits.
        super::super::talent_runtime::configure(&mut run);
        assert!(keys(&run).contains(&"duration"));
        assert!(super::super::passives::eligible(&duration, &run));
        assert!(keys(&run).contains(&"dot"));
    }
    #[test]
    fn queued_level_rewards_alternate_active_and_passive_after_each_confirmed_pick() {
        let mut run = run("fireball");
        run.level = 5;
        run.pending_levels = 4;
        let config = super::super::reward_config::Config::builtin();
        let mut rng = Rng::default();
        let mut offer = Offer {
            choices: choices_for(&run, &mut rng, &config),
            config,
        };
        for turn in 0..4 {
            assert_eq!(offer.choices.len(), 3);
            assert!(offer
                .choices
                .iter()
                .all(|choice| choice.is_active() == (turn % 2 == 0)));
            pick(0, &mut run, &mut offer, &mut rng, &mut NextState::default());
        }
        assert!(offer.choices.is_empty());
        assert!(!run.next_passive);
    }
    fn maxed_run() -> Run {
        let mut run = run("fireball");
        run.abilities = run
            .class
            .pool
            .iter()
            .take(data::MAX_ABILITIES)
            .enumerate()
            .map(|(i, key)| Owned {
                def: data::ability(key),
                level: [6, 5, 5, 3, 3, 3][i],
                cd: 0.0,
            })
            .collect();
        run
    }
    #[test]
    fn all_active_slots_maxed_means_passives_on_every_subsequent_level() {
        let mut run = maxed_run();
        run.pending_levels = 6;
        let config = super::super::reward_config::Config::builtin();
        let mut rng = Rng::default();
        let mut offer = Offer {
            choices: choices_for(&run, &mut rng, &config),
            config,
        };
        for _ in 0..6 {
            assert_eq!(offer.choices.len(), 3);
            assert!(offer
                .choices
                .iter()
                .all(|c| !c.is_active() && !matches!(c, Choice::Heal)));
            pick(0, &mut run, &mut offer, &mut rng, &mut NextState::default());
        }
    }
    #[test]
    fn a_maxed_spell_does_not_hide_new_spells_when_active_slots_are_free() {
        let mut run = run("fireball");
        run.abilities[0].level = data::MAX_ABILITY_LEVEL;
        let choices = choices_for(
            &run,
            &mut Rng::default(),
            &super::super::reward_config::Config::builtin(),
        );
        assert_eq!(choices.len(), 3);
        assert!(choices.iter().all(|c| matches!(c, Choice::NewAbility(_))));
    }
    #[test]
    fn specialization_caps_remove_offers_and_reject_stale_upgrade_cards() {
        let mut run = maxed_run();
        run.pending_levels = 1;
        assert!(!choice_pool(&run)
            .iter()
            .any(|c| matches!(c, Choice::Upgrade(_))));
        let mut offer = Offer {
            choices: vec![Choice::Upgrade(3)],
            ..default()
        };
        pick(
            0,
            &mut run,
            &mut offer,
            &mut Rng::default(),
            &mut NextState::default(),
        );
        assert_eq!(run.abilities[3].level, 3);
        assert_eq!(run.pending_levels, 1);
        offer.choices = vec![Choice::Upgrade(1)];
        pick(
            0,
            &mut run,
            &mut offer,
            &mut Rng::default(),
            &mut NextState::default(),
        );
        assert_eq!(run.abilities[1].level, 5);
        assert_eq!(run.pending_levels, 1);
    }
    #[test]
    fn last_active_upgrade_is_offered_then_both_turns_use_passives() {
        let mut run = maxed_run();
        run.abilities[0].level -= 1;
        run.pending_levels = 3;
        let config = super::super::reward_config::Config::builtin();
        let mut rng = Rng::default();
        let mut offer = Offer {
            choices: choices_for(&run, &mut rng, &config),
            config,
        };
        assert_eq!(offer.choices.len(), 3);
        assert!(offer
            .choices
            .iter()
            .all(|c| matches!(c, Choice::Upgrade(0..=2))));
        for _ in 0..2 {
            pick(0, &mut run, &mut offer, &mut rng, &mut NextState::default());
            assert_eq!(offer.choices.len(), 3);
            assert!(offer
                .choices
                .iter()
                .all(|c| !c.is_active() && !matches!(c, Choice::Heal)));
        }
    }
    #[test]
    fn configured_rarity_and_card_weights_drive_the_actual_offer_and_chained_levels() {
        use super::super::reward_config::Config;
        let config=Config::parse("version=1\n[rarities]\ncommon=1\nuncommon=0\nepic=0\nlegendary=0\n[passives]\nwisdom=3\npack=1\n").unwrap();
        let mut run = run("fireball");
        let wisdom = data::PASSIVES.iter().find(|p| p.key == "wisdom").unwrap();
        let pack = data::PASSIVES.iter().find(|p| p.key == "pack").unwrap();
        let mut counts = [0; 2];
        for n in 0..400 {
            let mut buckets = std::array::from_fn(|_| Vec::new());
            buckets[0] = vec![
                Choice::NumericPassive(wisdom, 1),
                Choice::NumericPassive(pack, 1),
            ];
            let choice = draw_weighted_reward(
                &mut buckets,
                0.99,
                (n as f32 + 0.5) / 400.0,
                config.rarity_weights(),
                |c| config.card_weight(c, &run),
            )
            .unwrap();
            let Choice::NumericPassive(p, _) = choice else {
                panic!("numeric");
            };
            counts[usize::from(p.key == "pack")] += 1;
        }
        assert_eq!(counts, [300, 100]);
        run.pending_levels = 2;
        let mut offer = Offer {
            choices: choices_for(&run, &mut Rng::default(), &config),
            config,
        };
        assert_eq!(offer.choices.len(), 3);
        assert!(offer
            .choices
            .iter()
            .all(|c| rarity_index(super::super::cards::rarity(*c, &run)) == 0));
        pick(
            0,
            &mut run,
            &mut offer,
            &mut Rng::default(),
            &mut NextState::default(),
        );
        assert_eq!(offer.choices.len(), 3);
        assert!(offer
            .choices
            .iter()
            .all(|c| rarity_index(super::super::cards::rarity(*c, &run)) == 0));
    }
    #[test]
    fn disabled_cards_and_rarities_cannot_reappear_through_fallback() {
        let run = run("fireball");
        let mut buckets = reward_buckets(&run);
        let choice = draw_weighted_reward(&mut buckets, 0.99, 0.5, [50.0, 30.0, 15.0, 5.0], |c| {
            if matches!(c,Choice::NumericPassive(p,_) if p.key=="wisdom") {
                1.0
            } else {
                0.0
            }
        })
        .unwrap();
        assert!(matches!(choice,Choice::NumericPassive(p,4) if p.key=="wisdom"));
        let mut buckets = std::array::from_fn(|_| Vec::new());
        buckets[0].push(Choice::Heal);
        assert!(
            draw_weighted_reward(&mut buckets, 0.1, 0.0, [0.0, 1.0, 0.0, 0.0], |_| 1.0).is_none()
        );
    }
    #[test]
    fn buff_effect_card_exists_in_every_rarity_and_accumulates_its_advertised_power() {
        let mut run = run("battle_shout");
        let p = data::PASSIVES
            .iter()
            .find(|p| p.key == "buff_effect")
            .unwrap();
        let buckets = reward_buckets(&run);
        for (i, bucket) in buckets.iter().enumerate() {
            assert!(bucket.iter().any(|c|matches!(c,Choice::NumericPassive(def,strength) if def.key==p.key && usize::from(*strength)==i+1)));
        }
        let mut total = 0;
        for strength in 1..=4 {
            let mut offer = Offer {
                choices: vec![Choice::NumericPassive(p, strength)],
                ..default()
            };
            pick(
                0,
                &mut run,
                &mut offer,
                &mut Rng::default(),
                &mut NextState::default(),
            );
            total += strength;
            let buff = run.resolve_spell(data::ability("battle_shout"), 1);
            assert!((buff.buff_effect - 0.05 * f32::from(total)).abs() < 0.001);
            assert_eq!(
                run.resolve_spell(data::ability("fireball"), 1).buff_effect,
                0.0
            );
        }
        assert!(super::super::passives::eligible(p, &run));
    }
    #[test]
    fn numeric_rewards_exist_at_every_rarity_and_accumulate_the_advertised_power() {
        let mut run = run("fireball");
        for key in ["might", "kings", "intellect", "crit_multi", "wisdom"] {
            let p = data::PASSIVES.iter().find(|p| p.key == key).unwrap();
            let buckets = reward_buckets(&run);
            for (i, bucket) in buckets.iter().enumerate() {
                assert!(bucket.iter().any(|c| matches!(c, Choice::NumericPassive(def, strength) if def.key == key && usize::from(*strength) == i + 1)), "{key} tier {i}");
            }
            for strength in 1..=4 {
                let mut offer = Offer {
                    choices: vec![Choice::NumericPassive(p, strength)],
                    ..Default::default()
                };
                pick(
                    0,
                    &mut run,
                    &mut offer,
                    &mut Rng::default(),
                    &mut NextState::default(),
                );
            }
            assert_eq!(
                run.passives
                    .iter()
                    .find(|(def, _)| def.key == key)
                    .unwrap()
                    .1,
                10
            );
        }
        let cast = run.resolve_spell(data::ability("fireball"), 1);
        assert!((cast.crit_multiplier - 3.0).abs() < 0.001);
        let (damage, critical) = super::super::spell_engine::critical_with_multiplier(
            cast.dmg,
            1.0,
            cast.crit_multiplier,
            0.0,
        );
        assert!(critical);
        assert!((damage - cast.dmg * 3.0).abs() < 0.001);
    }
    #[test]
    fn drawing_a_numeric_passive_removes_all_its_rarity_variants() {
        let run = run("fireball");
        let mut buckets = reward_buckets(&run);
        let Choice::NumericPassive(p, _) = draw_reward(&mut buckets, 0.1, 0.0).unwrap() else {
            panic!("numeric passive");
        };
        assert!(buckets
            .iter()
            .flatten()
            .all(|c| !matches!(c, Choice::NumericPassive(other, _) if other.key == p.key)));
    }
    fn run(key: &str) -> Run {
        Run {
            class: &data::CLASSES[6],
            abilities: vec![Owned {
                def: data::ability(key),
                level: 1,
                cd: 0.0,
            }],
            ..Default::default()
        }
    }
    fn keys(run: &Run) -> Vec<&'static str> {
        choice_pool(run)
            .into_iter()
            .filter_map(|c| {
                if let Choice::NewPassive(p) | Choice::NumericPassive(p, _) = c {
                    Some(p.key)
                } else {
                    None
                }
            })
            .collect()
    }
    #[test]
    fn actual_reward_pool_filters_unowned_tags_even_when_the_class_can_learn_them() {
        let mut run = run("heroic_strike");
        let pool = keys(&run);
        assert!(
            pool.contains(&"wisdom") && pool.contains(&"pack") && pool.contains(&"melee_targets")
        );
        for key in ["fire", "projectiles", "fork", "chains", "dot"] {
            assert!(!pool.contains(&key), "{key}");
        }
        run.abilities.push(Owned {
            def: data::ability("fireball"),
            level: 1,
            cd: 0.0,
        });
        let pool = keys(&run);
        for key in ["fire", "projectiles", "fork", "chains"] {
            assert!(pool.contains(&key), "{key}");
        }
    }
    #[test]
    fn xp_passives_stack_beyond_byte_ranks_and_remain_available() {
        let mut run = run("fireball");
        let p = data::PASSIVES.iter().find(|p| p.key == "wisdom").unwrap();
        let mut offer = Offer {
            choices: vec![Choice::NewPassive(p)],
            ..Default::default()
        };
        pick(
            0,
            &mut run,
            &mut offer,
            &mut Rng::default(),
            &mut NextState::default(),
        );
        for _ in 1..300 {
            offer.choices = vec![Choice::UpPassive(0)];
            pick(
                0,
                &mut run,
                &mut offer,
                &mut Rng::default(),
                &mut NextState::default(),
            );
        }
        assert_eq!(run.passives[0].1, 300);
        assert!((run.stats().growth - 31.0).abs() < 0.001);
        assert!(choice_pool(&run)
            .iter()
            .any(|c| matches!(c, Choice::NumericPassive(p, _) if p.key == "wisdom")));
        let card = describe_card(Choice::UpPassive(0), &run, None);
        assert!(card.ranks.is_none());
        assert!(card.tag.contains("300") && card.tag.contains("301"));
        assert!(!card.tag.contains("Unlimited"));
        assert!(card.desc.contains("»"));
    }
    #[test]
    fn more_than_six_different_passives_can_be_owned() {
        let mut run = run("fireball");
        for p in data::PASSIVES
            .iter()
            .filter(|p| matches!(p.effect, data::PassiveEffect::Stat(..)))
        {
            run.passives.push((p, 1));
        }
        assert_eq!(run.passives.len(), 6);
        assert!(keys(&run).contains(&"fire"));
        let before = run.resolve_spell(data::ability("fireball"), 1).dmg;
        let p = data::PASSIVES.iter().find(|p| p.key == "fire").unwrap();
        let mut offer = Offer {
            choices: vec![Choice::NewPassive(p)],
            ..Default::default()
        };
        pick(
            0,
            &mut run,
            &mut offer,
            &mut Rng::default(),
            &mut NextState::default(),
        );
        assert_eq!(run.passives.len(), 7);
        assert!(run.resolve_spell(data::ability("fireball"), 1).dmg > before);
    }

    #[test]
    fn coverage_passives_and_their_stack_upgrades_are_legendary_but_damage_and_recovery_are_not() {
        let mut run = run("fireball");
        for key in [
            "projectiles",
            "chains",
            "fork",
            "melee_targets",
            "fire",
            "might",
            "intellect",
            "amplify",
        ] {
            let p = data::PASSIVES.iter().find(|p| p.key == key).unwrap();
            let expected = if ["projectiles", "chains", "fork", "melee_targets"].contains(&key) {
                super::super::cards::Rarity::Legendary
            } else {
                super::super::cards::Rarity::Uncommon
            };
            assert!(
                super::super::cards::rarity(Choice::NewPassive(p), &run) == expected,
                "{key}"
            );
            run.passives.push((p, 1));
            assert!(
                super::super::cards::rarity(Choice::UpPassive(run.passives.len() - 1), &run)
                    == expected,
                "{key}"
            );
        }
    }

    #[test]
    fn fixed_rarity_odds_do_not_depend_on_the_number_of_cards() {
        let mut run = run("fireball");
        run.abilities[0].level = 2;
        let passive = |key| data::PASSIVES.iter().find(|p| p.key == key).unwrap();
        let entries = [
            Choice::NewPassive(passive("wisdom")),
            Choice::NewPassive(passive("fire")),
            Choice::NumericPassive(passive("wisdom"), 3),
            Choice::NewPassive(passive("projectiles")),
        ];
        for size in [1, 17] {
            let mut counts = [0; 4];
            for n in 0..10_000 {
                let mut buckets = std::array::from_fn(|i| vec![entries[i]; size]);
                let choice = draw_reward(&mut buckets, (n as f32 + 0.5) / 10_000.0, 0.4).unwrap();
                counts[rarity_index(super::super::cards::rarity(choice, &run))] += 1;
            }
            assert_eq!(counts, [5000, 3000, 1500, 500]);
        }
    }
    #[test]
    fn cards_within_one_rarity_are_uniform_and_missing_categories_only_fall_downward() {
        let run = run("fireball");
        let entries: Vec<_> = data::PASSIVES
            .iter()
            .filter(|p| matches!(p.effect, data::PassiveEffect::Stat(..)))
            .take(4)
            .collect();
        let mut counts = [0; 4];
        for n in 0..400 {
            let mut buckets = std::array::from_fn(|_| Vec::new());
            buckets[0] = entries.iter().map(|p| Choice::NewPassive(p)).collect();
            let Choice::NewPassive(p) =
                draw_reward(&mut buckets, 0.1, (n as f32 + 0.5) / 400.0).unwrap()
            else {
                panic!("passive")
            };
            counts[entries.iter().position(|entry| entry.key == p.key).unwrap()] += 1;
        }
        assert_eq!(counts, [100; 4]);
        let mut buckets = reward_buckets(&run);
        buckets[2].clear();
        buckets[3].clear();
        let choice = draw_reward(&mut buckets, 0.99, 0.5).unwrap();
        assert_eq!(rarity_index(super::super::cards::rarity(choice, &run)), 1);
        let mut buckets = std::array::from_fn(|_| Vec::new());
        buckets[3].push(Choice::Heal);
        assert!(draw_reward(&mut buckets, 0.1, 0.5).is_none());
    }
}
impl UiMaterial for SweepUiMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://benilla_app/shaders/ui_sweep.wgsl".into()
    }
}

/// The cooldown overlay on one ability slot, driven every frame.
#[derive(Component)]
pub(super) struct AbilitySweep {
    handle: Handle<SweepUiMaterial>,
    index: usize,
}

/// The class mobility's action button, bottom centre over the XP bar; shown once it is learned.
#[derive(Component)]
pub(super) struct MobilitySlot;
#[derive(Component)]
pub(super) struct MobilityIcon;
#[derive(Component)]
pub(super) struct MobilitySweep(Handle<SweepUiMaterial>);
#[derive(Component)]
pub(super) struct OverTitle;
#[derive(Component)]
pub(super) struct OverStats;
#[derive(Component)]
pub(super) struct OverLine;
#[derive(Component)]
pub(super) struct OverStat(usize);

/// An icon slot filled from the spell's `SpellIcon` once the client data is up.
#[derive(Component)]
pub(super) struct SpellIcon(pub(super) u32);
#[derive(Component)]
pub(super) struct IconDone;

/// A FrameXML `Texture` region: a client art file, its `TexCoords` (left, right, top, bottom) and
/// vertex colour, drawn once the archives are readable.
#[derive(Component)]
pub(super) struct UiArt {
    path: String,
    coords: [f32; 4],
    tint: Color,
    /// Draw in greyscale: an item the current class cannot equip.
    grey: bool,
}

impl UiArt {
    pub(super) fn crop(path: &'static str, coords: [f32; 4]) -> Self {
        Self {
            path: path.to_owned(),
            coords,
            tint: Color::WHITE,
            grey: false,
        }
    }

    pub(super) fn tinted(path: &'static str, tint: Color) -> Self {
        Self {
            path: path.to_owned(),
            coords: [0.0, 1.0, 0.0, 1.0],
            tint,
            grey: false,
        }
    }
    pub(super) fn item(path: &str) -> Self {
        Self {
            path: path.to_owned(),
            coords: [0.0, 1.0, 0.0, 1.0],
            tint: Color::WHITE,
            grey: false,
        }
    }
    pub(super) fn item_grey(path: &str) -> Self {
        Self {
            grey: true,
            ..Self::item(path)
        }
    }
}

/// A greyscale copy of an RGBA8 image; `None` for any other pixel format.
fn greyscale(image: &Image) -> Option<Image> {
    use bevy::render::render_resource::TextureFormat;
    if !matches!(
        image.texture_descriptor.format,
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb
    ) {
        return None;
    }
    let mut grey = image.clone();
    for px in grey.data.as_mut()?.chunks_exact_mut(4) {
        let luma = (u32::from(px[0]) * 77 + u32::from(px[1]) * 150 + u32::from(px[2]) * 29) >> 8;
        // Dimmed too, so the icon reads as disabled and not as a grey item.
        let v = (luma * 3 / 4) as u8;
        px[..3].fill(v);
    }
    Some(grey)
}

#[derive(Component)]
pub(super) struct ArtDone;

/// What an icon slot's tooltip describes: an index into the run's abilities or passives.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum TipFor {
    Ability(usize),
    Proc(&'static str),
    Passive(usize),
    Mobility,
}

/// The one `GameTooltip`, and the column its lines go in.
#[derive(Component)]
pub(super) struct GameTooltip;
#[derive(Component)]
pub(super) struct TipBody;

#[cfg(test)]
mod tooltip_tests;

#[derive(Component, Clone, Copy, PartialEq)]
pub(super) enum UiAction {
    Codex,
    Options,
    Resume,
    MainMenu,
    Quit,
    Logout,
}

/// The colour a button returns to when not hovered.

#[derive(Clone, Copy)]
pub(super) enum Choice {
    NumericPassive(&'static PassiveDef, u8),
    NewAbility(&'static AbilityDef),
    Upgrade(usize),
    NewPassive(&'static PassiveDef),
    UpPassive(usize),
    Heal,
}
impl Choice {
    pub(super) fn is_active(self) -> bool {
        matches!(self, Self::NewAbility(_) | Self::Upgrade(_))
    }
}

/// The level-up cards on offer.
#[derive(Resource, Default)]
pub(super) struct Offer {
    pub(in crate::survivors) choices: Vec<Choice>,
    pub(in crate::survivors) config: super::reward_config::Config,
}

impl Offer {
    pub(super) fn len(&self) -> usize {
        self.choices.len()
    }

    pub(super) fn choices(&self) -> &[Choice] {
        &self.choices
    }
}

#[derive(Resource)]
pub(super) struct HudFont(pub(super) Handle<Font>);

pub(super) fn rgb(hex: u32) -> Color {
    Color::srgb_u8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

fn text(font: &Handle<Font>, s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(s),
        TextFont {
            font: font.clone(),
            font_size: size,
            ..default()
        },
        TextColor(color),
        TextShadow {
            offset: Vec2::splat(1.5),
            color: Color::BLACK,
        },
    )
}

pub(super) fn text_bundle(
    font: &Handle<Font>,
    s: impl Into<String>,
    size: f32,
    color: Color,
) -> impl Bundle {
    text(font, s, size, color)
}

pub(super) fn centered_bundle(
    font: &Handle<Font>,
    s: impl Into<String>,
    size: f32,
    color: Color,
) -> impl Bundle {
    centered(font, s, size, color)
}

fn centered(font: &Handle<Font>, s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        text(font, s, size, color),
        TextLayout::new_with_justify(Justify::Center),
    )
}

fn icon(spell: u32, size: f32) -> impl Bundle {
    (
        Node {
            width: Val::Px(size),
            height: Val::Px(size),
            ..default()
        },
        ImageNode {
            color: Color::srgba(0.0, 0.0, 0.0, 0.6),
            ..default()
        },
        SpellIcon(spell),
    )
}

pub(super) fn spell_name(
    spells: Option<&crate::ui_action::Spells>,
    spell: u32,
    fallback: &str,
) -> String {
    if let Some(name) = super::extra_spells::name(fallback) {
        return name.to_owned();
    }
    spells
        .and_then(|s| s.catalog.get(spell))
        .map(|d| d.name.clone())
        .unwrap_or_else(|| fallback.replace('_', " "))
}

pub(super) fn setup_hud(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut sweeps_init: ResMut<Assets<SweepUiMaterial>>,
    mut fills: ResMut<Assets<super::fill::FillMaterial>>,
) {
    let font = crate::char_select::wow_font(&asset_server);
    let number_font: Handle<Font> = asset_server.load("mpq://Fonts/ARIALN.ttf");
    commands.insert_resource(HudFont(font.clone()));
    let f = &font;
    let map = MiniMap::new(&mut images);

    // --- The in-run HUD. ---
    commands
        .spawn((
            HudRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            GlobalZIndex(400),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            // Run XP sits above the run information, clear of the bottom menu.
            root.spawn((
                XpBar,
                Interaction::default(),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(50.0),
                    top: Val::Px(0.0),
                    margin: UiRect::left(Val::Px(-512.0)),
                    width: Val::Px(1024.0),
                    height: Val::Px(13.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
            ))
            .with_children(|bar| {
                bar.spawn((
                    XpFill,
                    Node {
                        width: Val::Percent(0.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    ImageNode {
                        color: Color::NONE,
                        ..default()
                    },
                    UiArt::tinted(
                        "Interface\\TargetingFrame\\UI-StatusBar",
                        Color::srgb(0.58, 0.0, 0.55),
                    ),
                ));
                // `MainMenuXPBarTexture0`..`3`: the segment rules, 256×10 on the bar's top.
                for (i, top) in [0.792_968_75, 0.542_968_75, 0.292_968_75, 0.042_968_75]
                    .into_iter()
                    .enumerate()
                {
                    bar.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(256.0 * i as f32),
                            top: Val::Px(0.0),
                            width: Val::Px(256.0),
                            height: Val::Px(10.0),
                            ..default()
                        },
                        ImageNode {
                            color: Color::NONE,
                            ..default()
                        },
                        UiArt::crop(
                            "Interface\\MainMenuBar\\UI-MainMenuBar-Dwarf",
                            [0.0, 1.0, top, top + 0.039_062_5],
                        ),
                    ));
                }
                // `MainMenuBarExpText`, `TextStatusBarText` centred one unit up, shown on hover.
                bar.spawn(Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    top: Val::Px(-1.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|n| {
                    n.spawn((
                        XpText,
                        Text::new(""),
                        TextFont {
                            font: number_font.clone(),
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                        TextShadow {
                            offset: Vec2::new(1.0, 1.0),
                            color: Color::BLACK,
                        },
                        TextLayout::new(Justify::Center, LineBreak::NoWrap),
                        Visibility::Hidden,
                    ));
                });
            });
            minimap::spawn_cluster(root, f, &map);
            // The mobility button: a 36-unit action slot in `UI-Quickslot2`, its key over it.
            root.spawn((
                MobilitySlot,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(50.0),
                    bottom: Val::Px(24.0),
                    margin: UiRect::left(Val::Px(-MOBILITY_SIZE / 2.0)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(2.0),
                    display: Display::None,
                    ..default()
                },
            ))
            .with_children(|col| {
                col.spawn(centered(f, "F", 18.0, Color::WHITE));
                col.spawn((
                    Node {
                        width: Val::Px(MOBILITY_SIZE),
                        height: Val::Px(MOBILITY_SIZE),
                        ..default()
                    },
                    Interaction::default(),
                    TipFor::Mobility,
                ))
                .with_children(|slot| {
                    // `UI-Quickslot2` is the empty slot's 64×64 frame, the button 36 inside it.
                    slot.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(-MOBILITY_SIZE * 0.42),
                            top: Val::Px(-MOBILITY_SIZE * 0.42),
                            width: Val::Px(MOBILITY_SIZE * 1.84),
                            height: Val::Px(MOBILITY_SIZE * 1.84),
                            ..default()
                        },
                        ImageNode {
                            color: Color::NONE,
                            ..default()
                        },
                        UiArt::crop("Interface\\Buttons\\UI-Quickslot2", [0.0, 1.0, 0.0, 1.0]),
                    ));
                    slot.spawn((
                        MobilityIcon,
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        ImageNode {
                            color: Color::srgba(0.0, 0.0, 0.0, 0.6),
                            ..default()
                        },
                    ));
                    let handle = sweeps_init.add(SweepUiMaterial { sweep: 0.0 });
                    slot.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        MaterialNode(handle.clone()),
                        MobilitySweep(handle),
                    ));
                });
            });
            root.spawn(Node {
                width: Val::Percent(100.0),
                padding: UiRect {
                    top: Val::Px(24.0),
                    ..UiRect::all(Val::Px(10.0))
                },
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexStart,
                ..default()
            })
            .with_children(|row| {
                row.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    width: Val::Px(300.0),
                    ..default()
                })
                .with_children(|col| {
                    col.spawn((
                        AbilityRow,
                        Node {
                            flex_wrap: FlexWrap::Wrap,
                            width: Val::Percent(100.0),
                            row_gap: Val::Px(4.0),
                            column_gap: Val::Px(4.0),
                            ..default()
                        },
                    ));
                    col.spawn((
                        PassiveRow,
                        Node {
                            flex_wrap: FlexWrap::Wrap,
                            width: Val::Percent(100.0),
                            row_gap: Val::Px(4.0),
                            column_gap: Val::Px(4.0),
                            ..default()
                        },
                    ));
                });
                // The clock, with the level and the kill count on one line under it.
                row.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(0.0),
                    ..default()
                })
                .with_children(|col| {
                    col.spawn((TimerText, centered(f, "00:00", 34.0, Color::WHITE)));
                    col.spawn(Node {
                        column_gap: Val::Px(14.0),
                        ..default()
                    })
                    .with_children(|line| {
                        line.spawn((LevelText, text(f, "Level 1", 14.0, GOLD)));
                        line.spawn((KillText, text(f, "Kills 0", 14.0, Color::WHITE)));
                    });
                });
                // The minimap cluster's column.
                row.spawn(Node {
                    width: Val::Px(300.0),
                    ..default()
                });
            });
            root.spawn(Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(22.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            })
            .with_children(|n| {
                n.spawn((BannerText, centered(f, "", 30.0, GOLD)));
            });
        });
    commands.insert_resource(map);

    // --- The run's pick screen; [`super::menu`] builds into it once the glue art is in. ---
    commands.spawn((
        MenuRoot,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        GlobalZIndex(410),
        bevy::ui::FocusPolicy::Pass,
    ));

    // --- Level up. ---
    commands
        .spawn((
            LevelUpRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(18.0),
                ..default()
            },
            // A dark vignette closing in on the cards, a slow golden light at its heart (the
            // 1400-unit burst's radius over the 768-unit canvas).
            MaterialNode(
                fills.add(
                    super::fill::FillMaterial::radial(
                        Color::srgba(0.04, 0.025, 0.0, 0.72),
                        Color::srgba(0.0, 0.0, 0.0, 0.9),
                    )
                    .with_glow(
                        Color::srgba(1.0, 0.75, 0.25, 0.22),
                        Vec2::splat(0.5),
                        Vec2::splat(700.0 / 768.0),
                    ),
                ),
            ),
            GlobalZIndex(420),
            Visibility::Hidden,
            // The build's icons stay hoverable under the cards.
            bevy::ui::FocusPolicy::Pass,
        ))
        .with_children(|root| {
            // The slow golden burst behind everything.
            root.spawn((
                super::cards::Rays,
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(1400.0),
                    height: Val::Px(1400.0),
                    left: Val::Percent(50.0),
                    top: Val::Percent(50.0),
                    margin: UiRect::new(
                        Val::Px(-700.0),
                        Val::Px(0.0),
                        Val::Px(-700.0),
                        Val::Px(0.0),
                    ),
                    ..default()
                },
                BackgroundGradient(vec![Gradient::Conic(ConicGradient::new(
                    UiPosition::CENTER,
                    (0..24)
                        .map(|i| {
                            let a = i as f32 / 24.0 * std::f32::consts::TAU;
                            let on = i % 2 == 0;
                            AngularColorStop::new(
                                if on {
                                    Color::srgba(1.0, 0.8, 0.3, 0.07)
                                } else {
                                    Color::srgba(1.0, 0.8, 0.3, 0.0)
                                },
                                a,
                            )
                        })
                        .collect(),
                ))]),
                UiTransform::default(),
                bevy::ui::FocusPolicy::Pass,
            ));
            root.spawn(centered(f, "LEVEL UP", 52.0, GOLD));
            root.spawn((
                LevelUpSub,
                centered(f, "Choose your power", 16.0, Color::srgb(0.85, 0.78, 0.6)),
            ));
            root.spawn((
                OfferRow,
                Node {
                    column_gap: Val::Px(22.0),
                    // Room for a hovered card's lift.
                    margin: UiRect::new(Val::ZERO, Val::ZERO, Val::Px(28.0), Val::Px(16.0)),
                    ..default()
                },
            ));
            root.spawn(centered(
                f,
                "Click a card, or use your card-selection keys",
                14.0,
                DIM,
            ));
        });

    // --- Pause. ---
    commands
        .spawn((
            PauseRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(12.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            GlobalZIndex(430),
            Visibility::Hidden,
            bevy::ui::FocusPolicy::Pass,
        ))
        .with_children(|root| {
            root.spawn(Node {
                width: Val::Px(300.0),
                padding: UiRect::all(Val::Px(24.0)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(12.0),
                ..default()
            })
            .with_children(|root| {
                super::combat_ui::skin::backdrop(root);
                root.spawn(centered(f, "Game Menu", 26.0, GOLD));
                for (label, action) in [
                    ("Options", UiAction::Options),
                    ("Codex", UiAction::Codex),
                    ("Resume", UiAction::Resume),
                    ("Return to Hub", UiAction::MainMenu),
                    ("Character selection", UiAction::Logout),
                    ("Quit game", UiAction::Quit),
                ] {
                    super::combat_ui::skin::button_sized(
                        root, f, label, action, 240.0, 34.0, 16.0, false,
                    );
                }
            });
        });

    // --- Results. ---
    commands
        .spawn((
            OverRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(14.0),
                ..default()
            },
            MaterialNode(fills.add(super::fill::FillMaterial::radial(
                Color::srgba(0.07, 0.02, 0.0, 0.8),
                Color::srgba(0.0, 0.0, 0.0, 0.94),
            ))),
            GlobalZIndex(440),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((OverTitle, centered(f, "", 64.0, GOLD)));
            root.spawn((
                OverLine,
                centered(f, "", 18.0, Color::srgb(0.85, 0.78, 0.6)),
            ));
            // The run in numbers, one tile each.
            root.spawn(Node {
                column_gap: Val::Px(12.0),
                margin: UiRect::vertical(Val::Px(14.0)),
                ..default()
            })
            .with_children(|row| {
                for (i, label) in ["Survived", "Level", "Kills", "Damage", "Bosses"]
                    .into_iter()
                    .enumerate()
                {
                    row.spawn((
                        Node {
                            width: Val::Px(128.0),
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            padding: UiRect::axes(Val::Px(10.0), Val::Px(12.0)),
                            row_gap: Val::Px(4.0),
                            border: UiRect::all(Val::Px(1.0)),
                            border_radius: BorderRadius::all(Val::Px(8.0)),
                            ..default()
                        },
                        BackgroundGradient::from(LinearGradient::to_bottom(vec![
                            ColorStop::percent(Color::srgba(0.2, 0.14, 0.05, 0.92), 0.0),
                            ColorStop::percent(Color::srgba(0.04, 0.03, 0.02, 0.94), 100.0),
                        ])),
                        BorderColor::all(EDGE),
                    ))
                    .with_children(|tile| {
                        super::combat_ui::skin::backdrop(tile);
                        tile.spawn((OverStat(i), centered(f, "", 26.0, Color::WHITE)));
                        tile.spawn(centered(f, label, 12.0, DIM));
                    });
                }
            });
            root.spawn((OverStats, centered(f, "", 15.0, DIM)));
            super::combat_ui::skin::button_sized(
                root,
                f,
                "Return to Hub",
                UiAction::MainMenu,
                260.0,
                42.0,
                18.0,
                false,
            );
            super::combat_ui::skin::button(root, f, "Quit game", UiAction::Quit, 160.0);
        });
}

/// The menu's hero is the run's hero: respawned whenever the pick changes.
pub(super) fn menu_preview(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    selection: Res<Selection>,
    roster: Res<super::characters::Roster>,
    mut run: ResMut<Run>,
    heroes: Query<Entity, With<Hero>>,
    world: benilla_world::world_point::WorldPoint,
    mut shown: Local<Option<(usize, u32, usize, Option<u64>)>>,
) {
    if *phase.get() != Phase::Menu {
        return;
    }
    let want = selection
        .class
        .zip(selection.look.and_then(|i| selection.looks.get(i)).copied());
    let key = want.map(|(c, l)| (c, l.0, selection.stage, roster.selected));
    if key == *shown && (key.is_none() || !heroes.is_empty()) {
        return;
    }
    for e in &heroes {
        commands.entity(e).despawn();
    }
    *shown = key;
    let Some((class, (display, race, sex))) = want else {
        return;
    };
    let mut pos = hero::arena(selection.stage_def());
    if !selection.in_hub {
        if let Some(h) = world.terrain_height_under(pos) {
            pos.y = h;
        }
    }
    let serial = run.guid();
    hero::spawn_hero(
        &mut commands,
        &data::CLASSES[class],
        display,
        race,
        sex,
        if roster.creating {
            1
        } else {
            roster.preview_level()
        },
        serial,
        pos,
    );
}

pub(super) fn start_run(run: &mut Run, selection: &mut Selection) -> bool {
    if selection.talents_open {
        return false;
    }
    if !selection.allows_map(selection.map) {
        return false;
    }
    let (Some(class), Some(&(_, race, _))) = (
        selection.class_def(),
        selection.look.and_then(|i| selection.looks.get(i)),
    ) else {
        return false;
    };
    // The world goes to the battleground picked; until now it stood in the last one.
    selection.stage = selection.map;
    selection.in_hub = false;
    let next_guid = run.next_guid;
    *run = Run {
        class,
        race,
        next_guid,
        expedition_map: selection.map,
        ..Run::default()
    };
    run.abilities.push(Owned {
        def: data::ability(class.start),
        level: 1,
        cd: 0.5,
    });
    run.hp = run.stats().max_hp;
    run.version += 1;
    true
}

fn choice_pool(run: &Run) -> Vec<Choice> {
    let mut pool: Vec<Choice> = Vec::new();
    for i in 0..run.abilities.len() {
        if super::specialization::can_upgrade(run, i) {
            pool.push(Choice::Upgrade(i));
        }
    }
    if run.abilities.len() < data::MAX_ABILITIES {
        for key in run.class.pool {
            if !run.abilities.iter().any(|o| o.def.key == *key) {
                pool.push(Choice::NewAbility(data::ability(key)));
            }
        }
    }
    for (i, (p, _)) in run.passives.iter().enumerate() {
        if super::passives::legendary(p) && super::passives::eligible(p, run) {
            pool.push(Choice::UpPassive(i));
        }
    }
    {
        for p in data::PASSIVES {
            if !super::passives::legendary(p) {
                if super::passives::eligible(p, run) {
                    for strength in 1..=4 {
                        pool.push(Choice::NumericPassive(p, strength));
                    }
                }
            } else if !run.passives.iter().any(|(q, _)| q.key == p.key)
                && super::passives::eligible(p, run)
            {
                pool.push(Choice::NewPassive(p));
            }
        }
    }
    pool
}

fn rarity_index(rarity: super::cards::Rarity) -> usize {
    use super::cards::Rarity;
    match rarity {
        Rarity::Common => 0,
        Rarity::Uncommon => 1,
        Rarity::Epic => 2,
        Rarity::Legendary => 3,
    }
}
fn reward_buckets(run: &Run) -> [Vec<Choice>; 4] {
    let mut buckets: [Vec<Choice>; 4] = std::array::from_fn(|_| Vec::new());
    for choice in choice_pool(run) {
        buckets[rarity_index(super::cards::rarity(choice, run))].push(choice);
    }
    buckets
}
#[cfg(test)]
fn draw_reward(
    buckets: &mut [Vec<Choice>; 4],
    category_roll: f32,
    entry_roll: f32,
) -> Option<Choice> {
    draw_weighted_reward(
        buckets,
        category_roll,
        entry_roll,
        [50.0, 30.0, 15.0, 5.0],
        |_| 1.0,
    )
}
/// Rarity first, then card; disabled cards and rarities never receive fallback weight.
fn draw_weighted_reward(
    buckets: &mut [Vec<Choice>; 4],
    category_roll: f32,
    entry_roll: f32,
    rarity_weights: [f64; 4],
    weight: impl Fn(Choice) -> f64,
) -> Option<Choice> {
    use super::reward_config::weighted_index;
    for bucket in buckets.iter_mut() {
        bucket.retain(|choice| weight(*choice) > 0.0);
    }
    let rolled = weighted_index(&rarity_weights, category_roll)?;
    let category = (0..=rolled)
        .rev()
        .find(|&i| rarity_weights[i] > 0.0 && !buckets[i].is_empty())?;
    let pool = &mut buckets[category];
    let weights: Vec<_> = pool.iter().map(|choice| weight(*choice)).collect();
    let index = weighted_index(&weights, entry_roll)?;
    let choice = pool.swap_remove(index);
    // Different rarities of the same passive cannot occupy several slots in one offer.
    if let Choice::NumericPassive(p, _) = choice {
        for bucket in buckets.iter_mut() {
            bucket.retain(|c| !matches!(c, Choice::NumericPassive(other, _) if other.key == p.key));
        }
    }
    Some(choice)
}
pub(in crate::survivors) fn choices_for(
    run: &Run,
    rng: &mut Rng,
    config: &super::reward_config::Config,
) -> Vec<Choice> {
    let mut buckets = reward_buckets(run);
    let weights = super::items::combat::lucky_weights(config.rarity_weights(), run.equipment.luck);
    for (i, bucket) in buckets.iter_mut().enumerate() {
        bucket.retain(|choice| weights[i] > 0.0 && config.card_weight(*choice, run) > 0.0);
    }
    let has_active = buckets.iter().flatten().any(|choice| choice.is_active());
    let has_passive = buckets.iter().flatten().any(|choice| !choice.is_active());
    let active = has_active && (!run.next_passive || !has_passive);
    for bucket in &mut buckets {
        bucket.retain(|choice| choice.is_active() == active);
    }
    let mut out = Vec::with_capacity(3);
    for _ in 0..3 {
        // Condition the configured rarity weights on this offer's remaining eligible cards.
        // Active cards have no Common entries; an empty category must not waste a choice.
        let available = std::array::from_fn(|i| {
            if buckets[i].is_empty() {
                0.0
            } else {
                weights[i]
            }
        });
        if let Some(choice) =
            draw_weighted_reward(&mut buckets, rng.f32(), rng.f32(), available, |choice| {
                config.card_weight(choice, run)
            })
        {
            out.push(choice);
        }
    }
    if out.is_empty() {
        out.push(Choice::Heal);
    }
    out
}
pub(super) fn roll_offer(run: Res<Run>, mut rng: ResMut<Rng>, mut offer: ResMut<Offer>) {
    offer.choices = choices_for(&run, &mut rng, &offer.config);
}

pub(super) fn pick(
    i: usize,
    run: &mut Run,
    offer: &mut Offer,
    rng: &mut Rng,
    next: &mut NextState<Phase>,
) {
    let Some(&choice) = offer.choices.get(i) else {
        return;
    };
    match choice {
        Choice::NumericPassive(p, strength) => {
            if let Some((_, total)) = run
                .passives
                .iter_mut()
                .find(|(owned, _)| owned.key == p.key)
            {
                *total = total.saturating_add(u64::from(strength));
            } else {
                run.passives.push((p, u64::from(strength)));
            }
        }
        Choice::NewAbility(def) => run.abilities.push(Owned {
            def,
            level: 1,
            cd: 0.3,
        }),
        Choice::Upgrade(k) => {
            if !super::specialization::can_upgrade(run, k) {
                offer.choices = choices_for(run, rng, &offer.config);
                return;
            }
            run.abilities[k].level += 1;
        }
        Choice::NewPassive(p) => run.passives.push((p, 1)),
        Choice::UpPassive(k) => run.passives[k].1 = run.passives[k].1.saturating_add(1),
        Choice::Heal => run.hp = run.stats().max_hp,
    }
    super::passives::sync(run);
    run.next_passive = !run.next_passive;
    run.version += 1;
    run.pending_levels = run.pending_levels.saturating_sub(1);
    if run.pending_levels > 0 {
        offer.choices = choices_for(run, rng, &offer.config);
    } else {
        offer.choices.clear();
        next.set(Phase::Playing);
    }
}

pub(super) fn looks_for(
    class: &data::ClassDef,
    creatures: &crate::entities::Creatures,
) -> Vec<(u32, u8, u8)> {
    let mut out: Vec<(u32, u8, u8)> = Vec::new();
    for &d in class.looks {
        if let Some((race, sex)) = creatures.display_race_sex(d) {
            if !out.iter().any(|&(_, r, s)| r == race && s == sex) {
                out.push((d, race, sex));
            }
        }
    }
    out.sort_by_key(|&(_, r, s)| (r, s));
    out
}

#[allow(clippy::too_many_arguments)]
pub(super) fn buttons(
    presses: Query<(&Interaction, &UiAction), Changed<Interaction>>,
    mut next: ResMut<NextState<Phase>>,
    mut exit: MessageWriter<AppExit>,
    phase: Res<State<Phase>>,
    mut panel: ResMut<super::options::Panel>,
    selection: Res<Selection>,
) {
    for (interaction, action) in &presses {
        if *interaction == Interaction::Pressed {
            match (*action, *phase.get()) {
                (UiAction::Options, Phase::Paused) => panel.open = true,
                (UiAction::Resume, Phase::Paused) => next.set(if selection.in_hub {
                    Phase::Hub
                } else {
                    Phase::Playing
                }),
                (UiAction::MainMenu, Phase::Paused | Phase::GameOver) => next.set(Phase::Hub),
                (UiAction::Logout, Phase::Paused | Phase::GameOver) => next.set(Phase::Menu),
                (UiAction::Quit, Phase::Paused | Phase::GameOver) => {
                    exit.write(AppExit::Success);
                }
                _ => {}
            }
        }
    }
}

pub(super) fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    phase: Res<State<Phase>>,
    settings: Res<super::options::Settings>,
    panel: Res<super::options::Panel>,
    codex: Option<Res<super::codex::Panel>>,
    mut selection: ResMut<Selection>,
    mut next: ResMut<NextState<Phase>>,
) {
    if panel.open || codex.as_ref().is_some_and(|c| c.open) {
        return;
    }
    match phase.get() {
        Phase::Menu => {}
        Phase::Hub => {
            if keys.just_pressed(KeyCode::Escape)
                || settings.just_pressed(super::options::Action::Pause, &keys)
            {
                if selection.talents_open {
                    selection.talents_open = false;
                } else {
                    next.set(Phase::Paused);
                }
            }
        }
        Phase::Playing => {
            if keys.just_pressed(KeyCode::Escape)
                || settings.just_pressed(super::options::Action::Pause, &keys)
            {
                next.set(Phase::Paused);
            }
        }
        Phase::Paused => {
            if keys.just_pressed(KeyCode::Escape)
                || settings.just_pressed(super::options::Action::Pause, &keys)
            {
                next.set(if selection.in_hub {
                    Phase::Hub
                } else {
                    Phase::Playing
                });
            }
        }
        Phase::LevelUp | Phase::CharacterSheet => {}
        Phase::GameOver => {
            if keys.just_pressed(KeyCode::Enter) {
                next.set(Phase::Hub);
            }
        }
    }
}

/// The class's looks once the creature data is in, defaulting to the first. The auto-runner picks
/// a class with no look; the menu itself always sets both.
pub(super) fn refresh_menu(
    mut selection: ResMut<Selection>,
    creatures: Option<Res<crate::entities::Creatures>>,
) {
    if let (Some(class), Some(creatures)) = (selection.class_def(), creatures.as_deref()) {
        if selection.looks.is_empty() {
            let looks = looks_for(class, creatures);
            if !looks.is_empty() {
                selection.looks = looks;
                selection.look = Some(0);
                selection.version += 1;
            }
        }
    }
}

/// What a level-up card shows.
pub(super) struct CardText {
    pub spell: u32,
    pub name: String,
    pub tag: String,
    pub tag_color: Color,
    pub desc: String,
    /// Bounded spell ranks for pips; passives use unlimited stack counters in their tag.
    pub ranks: Option<(u8, u8)>,
}

pub(super) fn describe_card(
    choice: Choice,
    run: &Run,
    spells: Option<&crate::ui_action::Spells>,
) -> CardText {
    let new = Color::srgb(0.3, 1.0, 0.3);
    match choice {
        Choice::NumericPassive(p, strength) => {
            let total = run
                .passives
                .iter()
                .find(|(owned, _)| owned.key == p.key)
                .map_or(0, |(_, total)| *total);
            CardText {
                spell: p.spell,
                name: p.name.into(),
                tag: format!(
                    "Passive | Stacks {total} » {}",
                    total.saturating_add(u64::from(strength))
                ),
                tag_color: new,
                desc: passive_card_delta(p, total, total.saturating_add(u64::from(strength))),
                ranks: None,
            }
        }
        Choice::NewAbility(def) => {
            let c = run.resolve_spell(def, 1);
            CardText {
                spell: def.spell,
                name: spell_name(spells, def.spell, def.key),
                tag: "New spell | Base cap 3".into(),
                tag_color: new,
                desc: if def.kind == Kind::Buff {
                    super::support_spells::tooltip(def.key, 1, &c)
                        .unwrap_or_else(|| ability_body(def, &c))
                } else {
                    format!("{}.", ability_body(def, &c))
                },
                ranks: Some((0, 3)),
            }
        }
        Choice::Upgrade(k) => {
            let o = &run.abilities[k];
            let next = o.level + 1;
            let (a, b) = (
                run.resolve_spell(o.def, o.level),
                run.resolve_spell(o.def, next),
            );
            // What actually changes, as before → after.
            let mut lines = Vec::new();
            lines.extend(effect_changes(a.effect, b.effect));
            if super::support_spells::magnitude(o.def.key, 1) > 0.0 {
                lines.push(format!(
                    "Bonus {} » {}",
                    super::support_spells::power_label(o.def.key, o.level, a.buff_effect),
                    super::support_spells::power_label(o.def.key, next, b.buff_effect)
                ));
            }
            let (da, db) = if a.dmg > 0.0 {
                (a.dmg, b.dmg)
            } else {
                (a.dot, b.dot)
            };
            if db > da {
                lines.push(format!("Damage {} » {}", num(da), num(db)));
            }
            if b.cooldown < a.cooldown - 0.01 {
                lines.push(format!(
                    "Cooldown {}s » {}s",
                    num(a.cooldown),
                    num(b.cooldown)
                ));
            }
            if b.count > a.count {
                lines.push(format!("Targets {} » {}", a.count, b.count));
            }
            if b.radius > a.radius + 0.05 {
                lines.push(format!("Area {} » {} yd", num(a.radius), num(b.radius)));
            }
            if b.duration > a.duration + 0.05 {
                lines.push(format!("Lasts {}s » {}s", num(a.duration), num(b.duration)));
            }
            CardText {
                spell: o.def.spell,
                name: spell_name(spells, o.def.spell, o.def.key),
                tag: format!("Upgrade | Rank {} » {next}", o.level),
                tag_color: if next == 6 {
                    GOLD
                } else if next >= 4 {
                    SILVER
                } else {
                    Color::WHITE
                },
                desc: lines.join("\n"),
                ranks: Some((o.level, super::specialization::rank_cap(next))),
            }
        }
        Choice::NewPassive(p) => CardText {
            spell: p.spell,
            name: p.name.into(),
            tag: "New passive | Stacks 0 » 1".into(),
            tag_color: new,
            desc: passive_card_delta(p, 0, 1),
            ranks: None,
        },
        Choice::UpPassive(k) => {
            let (p, level) = run.passives[k];
            CardText {
                spell: p.spell,
                name: p.name.into(),
                tag: format!("Upgrade | Stacks {level} » {}", level.saturating_add(1)),
                tag_color: Color::WHITE,
                desc: passive_card_delta(p, level, level.saturating_add(1)),
                ranks: None,
            }
        }
        Choice::Heal => CardText {
            spell: 5720,
            name: "Healthstone".into(),
            tag: "Restore".into(),
            tag_color: Color::WHITE,
            desc: "Everything is mastered. Restores all health.".into(),
            ranks: None,
        },
    }
}

type BuildIcons = (
    Vec<(&'static str, u8)>,
    Vec<(&'static str, u64)>,
    Vec<&'static str>,
);

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn refresh_hud(
    mut commands: Commands,
    time: Res<Time>,
    phase: Res<State<Phase>>,
    mut run: ResMut<Run>,
    selection: Res<Selection>,
    font: Res<HudFont>,
    mut texts: ParamSet<(
        Query<&mut Text, With<LevelText>>,
        Query<&mut Text, With<TimerText>>,
        Query<&mut Text, With<KillText>>,
        Query<&mut Text, With<BannerText>>,
        Query<&mut Text, With<OverTitle>>,
        Query<&mut Text, With<OverStats>>,
        Query<&mut Text, With<XpText>>,
    )>,
    mut xp_fill: Query<&mut Node, With<XpFill>>,
    rows: Query<(Entity, Has<AbilityRow>), Or<(With<AbilityRow>, With<PassiveRow>)>>,
    mut sweeps: ResMut<Assets<SweepUiMaterial>>,
    mut last_build: Local<Option<BuildIcons>>,
) {
    let need = data::xp_to_next(run.level);
    if let Ok(mut n) = xp_fill.single_mut() {
        n.width = Val::Percent((run.xp / need * 100.0).clamp(0.0, 100.0));
    }
    if let Ok(mut t) = texts.p6().single_mut() {
        let xp = format!("XP {} / {}", run.xp as u32, need as u32);
        if **t != xp {
            **t = xp;
        }
    }
    if let Ok(mut t) = texts.p0().single_mut() {
        **t = format!("Level {}", run.level);
    }
    let secs = run.elapsed as u32;
    if let Ok(mut t) = texts.p1().single_mut() {
        **t = format!("{:02}:{:02}", secs / 60, secs % 60);
    }
    let kills = run.kills;
    if let Ok(mut t) = texts.p2().single_mut() {
        **t = format!("Kills {kills}");
    }
    let banner = match run.banner.as_mut() {
        Some((msg, t)) => {
            *t -= time.delta_secs();
            if *t > 0.0 {
                msg.clone()
            } else {
                String::new()
            }
        }
        None => String::new(),
    };
    if banner.is_empty() {
        run.banner = None;
    }
    // The picker owns the screen: an announcement waits until it closes.
    let banner = if let Some(revival) = super::revival::hint(&run) {
        revival
    } else if *phase.get() == Phase::LevelUp {
        String::new()
    } else {
        banner
    };
    if let Ok(mut t) = texts.p3().single_mut() {
        if **t != banner {
            **t = banner;
        }
    }
    let title = if run.won {
        "Victory!"
    } else {
        "You have fallen"
    }
    .to_string();
    if let Ok(mut t) = texts.p4().single_mut() {
        **t = title;
    }
    let summary = format!(
        "{} {} in {}   —   press Enter to play again",
        crate::char_select::race_name(run.race),
        run.class.name,
        selection.map_def().name,
    );
    if let Ok(mut t) = texts.p5().single_mut() {
        if **t != summary {
            **t = summary;
        }
    }

    // The ability and passive icons, rebuilt when the build changes.
    let procs = super::procs::available(&run);
    let build = (
        run.abilities.iter().map(|o| (o.def.key, o.level)).collect(),
        run.passives
            .iter()
            .map(|(p, rank)| (p.key, *rank))
            .collect(),
        procs.iter().map(|p| p.key).collect(),
    );
    if last_build.as_ref() == Some(&build) {
        return;
    }
    *last_build = Some(build);
    let f = &font.0;
    for (row, is_abilities) in &rows {
        commands.entity(row).despawn_related::<Children>();
        commands.entity(row).with_children(|row| {
            let entries: Vec<(u32, u64, f32, TipFor)> = if is_abilities {
                run.abilities
                    .iter()
                    .enumerate()
                    .map(|(i, o)| (o.def.spell, u64::from(o.level), 40.0, TipFor::Ability(i)))
                    .chain(
                        procs
                            .iter()
                            .map(|p| (p.spell, 0, 40.0, TipFor::Proc(p.key))),
                    )
                    .collect()
            } else {
                run.passives
                    .iter()
                    .enumerate()
                    .map(|(i, (p, l))| (p.spell, *l, 30.0, TipFor::Passive(i)))
                    .collect()
            };
            for (spell, level, size, tip) in entries {
                let ability = match &tip {
                    TipFor::Ability(i) => Some(*i),
                    _ => None,
                };
                let specialized = ability.is_some() && level >= 4;
                row.spawn((
                    Node {
                        width: Val::Px(size),
                        height: Val::Px(size),
                        flex_shrink: 0.0,
                        border: UiRect::all(Val::Px(if specialized { 2.0 } else { 1.0 })),
                        ..default()
                    },
                    BorderColor::all(if specialized {
                        if level >= 6 {
                            GOLD
                        } else {
                            SILVER
                        }
                    } else {
                        EDGE
                    }),
                    Interaction::default(),
                    tip,
                ))
                .with_children(|slot| {
                    slot.spawn(icon(spell, size - if specialized { 4.0 } else { 2.0 }));
                    // The cooldown sweep sits over the icon, under the level text.
                    if let Some(i) = ability {
                        let handle = sweeps.add(SweepUiMaterial { sweep: 0.0 });
                        slot.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            MaterialNode(handle.clone()),
                            AbilitySweep { handle, index: i },
                        ));
                        // Over the sweep: shown while the skill is switched off.
                        slot.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.45, 0.0, 0.0, 0.7)),
                            Visibility::Hidden,
                            AbilityOff(i),
                        ));
                    }
                    slot.spawn(Node {
                        position_type: PositionType::Absolute,
                        right: Val::Px(2.0),
                        bottom: Val::Px(0.0),
                        ..default()
                    })
                    .with_children(|n| {
                        n.spawn(text(
                            f,
                            if matches!(tip, TipFor::Proc(_)) {
                                "P".into()
                            } else {
                                level.to_string()
                            },
                            12.0,
                            Color::WHITE,
                        ));
                    });
                });
            }
        });
    }
}

/// Fill icon slots from `SpellIcon.dbc` once the client data and the asset reader are up.
pub(super) fn fill_icons(
    mut commands: Commands,
    mut slots: Query<(Entity, &mut ImageNode, &SpellIcon), Without<IconDone>>,
    spells: Option<Res<crate::ui_action::Spells>>,
    mut assets: Option<ResMut<benilla_assets::WorldAssets>>,
    mut images: ResMut<Assets<Image>>,
    mut cache: Local<HashMap<u32, Handle<Image>>>,
) {
    let (Some(spells), Some(assets)) = (spells.as_deref(), assets.as_deref_mut()) else {
        return;
    };
    for (e, mut img, icon) in &mut slots {
        let handle = match cache.get(&icon.0) {
            Some(h) => Some(h.clone()),
            None => {
                let path = spells
                    .catalog
                    .get(icon.0)
                    .and_then(|d| d.icon.clone())
                    .unwrap_or_else(|| "Interface\\Icons\\INV_Misc_QuestionMark".into());
                let h = assets.sprite_texture(&format!("{path}.blp"), &mut images);
                if let Some(h) = &h {
                    cache.insert(icon.0, h.clone());
                }
                h
            }
        };
        if let Some(h) = handle {
            img.image = h;
            img.color = Color::WHITE;
        }
        commands.entity(e).try_insert(IconDone);
    }
}

/// The mobility button: shown once learned, its icon the class's spell, its sweep the cooldown.
#[allow(clippy::type_complexity)]
pub(super) fn refresh_mobility(
    run: Res<Run>,
    mut commands: Commands,
    mut slot: Query<&mut Node, With<MobilitySlot>>,
    icon: Query<(Entity, Option<&SpellIcon>), With<MobilityIcon>>,
    sweep: Query<&MobilitySweep>,
    mut mats: ResMut<Assets<SweepUiMaterial>>,
) {
    let def = run.mobility();
    if let Ok(mut node) = slot.single_mut() {
        let display = if def.is_some() {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
    }
    let Some(def) = def else { return };
    // A new spell re-arms the icon fill ([`fill_icons`]).
    if let Ok((e, have)) = icon.single() {
        if have.is_none_or(|s| s.0 != def.spell) {
            commands
                .entity(e)
                .remove::<IconDone>()
                .insert(SpellIcon(def.spell));
        }
    }
    if let Ok(s) = sweep.single() {
        let resolved = super::spell_engine::resolve_mobility(def, run.spell_modifiers.entries());
        let f = (run.mobile.cd / resolved.cooldown.max(0.01)).clamp(0.0, 1.0);
        if let Some(m) = mats.get_mut(&s.0) {
            if m.sweep != f {
                m.sweep = f;
            }
        }
    }
}

/// The red cover of a switched-off skill, by its index in the run's abilities.
#[derive(Component)]
pub(super) struct AbilityOff(usize);

/// On the training ground, a right click on a skill icon switches the skill off or back on, to
/// test one skill at a time. A new run starts with everything on.
pub(super) fn toggle_training_spells(
    mouse: Res<ButtonInput<MouseButton>>,
    phase: Res<State<Phase>>,
    selection: Res<Selection>,
    mut run: ResMut<Run>,
    icons: Query<(&Interaction, &TipFor)>,
    mut covers: Query<(&AbilityOff, &mut Visibility)>,
) {
    let usable = *phase.get() == Phase::Playing && super::training::active(&selection);
    if usable && mouse.just_pressed(MouseButton::Right) {
        let hovered = icons.iter().find_map(|(interaction, tip)| match tip {
            TipFor::Ability(index) if *interaction != Interaction::None => Some(*index),
            _ => None,
        });
        if let Some(key) = hovered
            .and_then(|i| run.abilities.get(i))
            .map(|o| o.def.key)
        {
            let enabled = !run.disabled_spells.insert(key);
            if enabled {
                run.disabled_spells.remove(key);
            }
            let state = if enabled { "enabled" } else { "disabled" };
            run.banner = Some((format!("{} {state}", key.replace('_', " ")), 2.0));
        }
    }
    for (cover, mut visibility) in &mut covers {
        let off = run
            .abilities
            .get(cover.0)
            .is_some_and(|o| run.disabled_spells.contains(o.def.key));
        visibility.set_if_neq(if off {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
}

/// Drive each ability icon's cooldown sweep from its remaining cooldown.
pub(super) fn update_cooldown_sweeps(
    run: Res<Run>,
    mut mats: ResMut<Assets<SweepUiMaterial>>,
    sweeps: Query<&AbilitySweep>,
) {
    for s in &sweeps {
        let Some(o) = run.abilities.get(s.index) else {
            continue;
        };
        let total = run.resolve_spell(o.def, o.level).cooldown.max(0.01);
        let f = (o.cd / total).clamp(0.0, 1.0);
        if let Some(m) = mats.get_mut(&s.handle) {
            m.sweep = f;
        }
    }
}

/// Draw each [`UiArt`] region once its file can be read.
pub(super) fn fill_art(
    mut commands: Commands,
    mut regions: Query<(Entity, &mut ImageNode, &UiArt), Without<ArtDone>>,
    assets: Option<ResMut<benilla_assets::WorldAssets>>,
    mut images: ResMut<Assets<Image>>,
    mut greys: Local<HashMap<String, Handle<Image>>>,
) {
    let Some(mut assets) = assets else { return };
    for (e, mut img, art) in &mut regions {
        commands.entity(e).try_insert(ArtDone);
        let Some(h) = assets.sprite_texture(&art.path, &mut images) else {
            warn!("survivors: no UI art {}", art.path);
            continue;
        };
        let [l, r, t, b] = art.coords;
        if let Some(size) = images.get(&h).map(Image::size_f32) {
            img.rect = Some(Rect::new(l * size.x, t * size.y, r * size.x, b * size.y));
        }
        img.color = art.tint;
        img.image = if art.grey {
            let cached = greys.get(&art.path).cloned();
            let grey = cached.or_else(|| {
                let copy = greyscale(images.get(&h)?)?;
                let handle = images.add(copy);
                greys.insert(art.path.clone(), handle.clone());
                Some(handle)
            });
            if grey.is_none() {
                // Compressed art cannot be rewritten: fall back to a dark tint.
                img.color = Color::srgb(0.35, 0.35, 0.35);
            }
            grey.unwrap_or(h)
        } else {
            h
        };
    }
}

/// The reference lays its UI out on a screen 768 units tall at any resolution (`useUiScale` off).
pub(super) fn apply_ui_scale(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut scale: ResMut<UiScale>,
) {
    let Ok(window) = windows.single() else { return };
    let want = window.height() / 768.0;
    if want > 0.0 && (scale.0 - want).abs() > 1e-4 {
        scale.0 = want;
    }
}

/// `MainMenuExpBar`'s `OnEnter` and `OnLeave`: the numbers show while the cursor is on the bar.
pub(super) fn xp_hover(
    bars: Query<&Interaction, (With<XpBar>, Changed<Interaction>)>,
    mut texts: Query<&mut Visibility, With<XpText>>,
) {
    for interaction in &bars {
        let want = if *interaction == Interaction::None {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        for mut v in &mut texts {
            *v = want;
        }
    }
}

// ── The build icons' `GameTooltip` ──────────────────────────────────────────────────────────────

/// `TOOLTIP_DEFAULT_BACKGROUND_COLOR`, the `GameTooltip` backdrop tint (`GameTooltip.lua`).
const TIP_FILL: Color = Color::srgb(0.09, 0.09, 0.19);
/// The grey a spell tooltip prints its rank in, right of the name.
const TIP_GREY: Color = Color::srgb(0.5, 0.5, 0.5);

/// One tooltip line: the left text, an optional right-aligned one, the colour and font size
/// (`GameTooltipHeaderText` 14, `GameTooltipText` 12).
#[derive(PartialEq)]
pub(super) struct TipLine {
    left: String,
    right: Option<(String, Color)>,
    color: Color,
    size: f32,
}

impl TipLine {
    fn new(left: impl Into<String>, color: Color) -> Self {
        Self {
            left: left.into(),
            right: None,
            color,
            size: 12.0,
        }
    }

    fn right(mut self, text: impl Into<String>, color: Color) -> Self {
        self.right = Some((text.into(), color));
        self
    }

    fn header(mut self) -> Self {
        self.size = 14.0;
        self
    }
}

/// A number as the tooltips print it: whole when near enough, else to one decimal.

/// The results screen's tiles and its line under the title.
#[allow(clippy::type_complexity)]
pub(super) fn refresh_results(
    run: Res<Run>,
    selection: Res<Selection>,
    mut over_texts: ParamSet<(
        Query<(&OverStat, &mut Text)>,
        Query<&mut Text, With<OverLine>>,
    )>,
) {
    let secs = run.elapsed as u32;
    let stats_line = if run.won {
        selection.map_def().victory
    } else {
        "The horde was too much this time."
    };
    let values = [
        format!("{:02}:{:02}", secs / 60, secs % 60),
        run.level.to_string(),
        run.kills.to_string(),
        compact(run.damage_done),
        run.bosses.to_string(),
    ];
    for (stat, mut t) in &mut over_texts.p0() {
        if let Some(v) = values.get(stat.0) {
            if **t != *v {
                **t = v.clone();
            }
        }
    }
    if let Ok(mut t) = over_texts.p1().single_mut() {
        if **t != stats_line {
            **t = stats_line.to_string();
        }
    }
}

/// 12.3k, 4.5M.
fn compact(v: f64) -> String {
    if v >= 1e6 {
        format!("{:.1}M", v / 1e6)
    } else if v >= 1e4 {
        format!("{:.1}k", v / 1e3)
    } else {
        format!("{}", v.round() as u64)
    }
}

fn num(v: f32) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.1}")
    }
}

fn pct(v: f32) -> String {
    format!("{}%", num(v * 100.0))
}

/// The numbers an ability casts with at a level, as [`super::abilities::cast_abilities`] takes
/// them from [`Owned`].
type Cast = super::spell_engine::Resolved;
fn effect_text(effect: Effect) -> Option<String> {
    Some(match effect {
        Effect::None => return None,
        Effect::Slow(m, t) => format!("slowing them by {} for {} sec", pct(1.0 - m), num(t)),
        Effect::Root(t) => format!("freezing them in place for {} sec", num(t)),
        Effect::Stun(t) => format!("stunning them for {} sec", num(t)),
        Effect::Fear(t) => format!("sending them fleeing for {} sec", num(t)),
        Effect::Weaken(m, t) => format!(
            "cutting their damage by {} for {} sec",
            pct(1.0 - m),
            num(t)
        ),
        Effect::Drain(f) => format!("healing you for {} of the damage", pct(f)),
        Effect::Execute(f) => format!("killing them outright below {} health", pct(f)),
    })
}

fn effect_changes(before: Effect, after: Effect) -> Vec<String> {
    let mut lines = Vec::new();
    let (name, a, b, ta, tb) = match (before, after) {
        (Effect::Slow(a, ta), Effect::Slow(b, tb)) => ("Slow", Some(a), Some(b), ta, tb),
        (Effect::Weaken(a, ta), Effect::Weaken(b, tb)) => {
            ("Enemy damage reduction", Some(a), Some(b), ta, tb)
        }
        (Effect::Root(a), Effect::Root(b)) => ("Root duration", None, None, a, b),
        (Effect::Stun(a), Effect::Stun(b)) => ("Stun duration", None, None, a, b),
        (Effect::Fear(a), Effect::Fear(b)) => ("Fear duration", None, None, a, b),
        _ => return lines,
    };
    if let (Some(a), Some(b)) = (a, b) {
        if (a - b).abs() > 0.0001 {
            lines.push(format!("{name} {} » {}", pct(1.0 - a), pct(1.0 - b)));
        }
    }
    if (ta - tb).abs() > 0.01 {
        lines.push(format!(
            "{} {}s » {}s",
            if a.is_some() { "Debuff duration" } else { name },
            num(ta),
            num(tb)
        ));
    }
    lines
}

/// What an ability does at these numbers, in the voice of a spell description.
fn ability_body(def: &AbilityDef, c: &Cast) -> String {
    if let Some(text) = super::extra_spells::description(def.key) {
        let mut values = Vec::new();
        if c.dmg > 0.0 {
            let label = match def.key {
                "efflorescence" => "total healing per ally",
                "holy_shield" => "damage per block",
                "penance" => "damage per strike",
                _ => "damage",
            };
            values.push(format!("{} {label}", num(c.dmg)));
        }
        if c.dot > 0.0 {
            values.push(format!("{} damage/sec", num(c.dot)));
        }
        if c.radius > 0.0 {
            values.push(format!("{} yd radius", num(c.radius)));
        }
        if c.duration > 0.0 {
            values.push(format!("{} sec duration", num(c.duration)));
        }
        return format!("{text} {}.", values.join(", "));
    }
    let d = num(c.dmg);
    let burst = def.duration <= 0.0;
    let mut s = match def.kind {
        Kind::Buff => super::support_spells::description(def.key).unwrap_or("Strengthens nearby allies").to_owned(),
        Kind::Bolt | Kind::Strike | Kind::Melee => {
            let melee = def.kind == Kind::Melee;
            let who = match (melee, c.count) {
                (true, 1) => "the closest enemy".into(),
                (true, n) => format!("the {n} closest enemies"),
                (false, 1) => "the enemy with the lowest remaining health in range".into(),
                (false, n) => format!("the {n} enemies with the lowest remaining health in range"),
            };
            let mut s = format!("{} {who} for {d} damage", if melee { "Strikes" } else { "Hits" });
            if c.radius > 0.0 && !melee {
                s += &format!(", and everything within {} yd of the hit", num(c.radius));
            }
            s
        }
        Kind::Nova => format!("Deals {d} damage to every enemy within {} yd of you", num(c.radius)),
        Kind::Cone => format!("Deals {d} damage to every enemy in a {} yd cone toward the closest", num(c.radius)),
        Kind::Ground | Kind::GroundSelf => {
            let at = if def.kind == Kind::GroundSelf { "around you" } else { "on the thickest pack" };
            if burst {
                format!("Strikes a {} yd area {at} for {d} damage", num(c.radius))
            } else {
                format!(
                    "Covers a {} yd area {at} for {} sec, dealing {d} damage every second",
                    num(c.radius),
                    num(c.duration)
                )
            }
        }
        Kind::Chain => format!(
            "Strikes the enemy with the lowest remaining health in range for {d} damage, then jumps to {} more within {} yd of each other, 15% weaker with every jump",
            c.count,
            num(c.radius)
        ),
        Kind::Dot => format!(
            "Afflicts the {} enemies with the lowest remaining health in range with {} damage every second for {} sec",
            c.count,
            num(c.dot),
            num(c.duration)
        ),
        Kind::Aura => format!(
            "Every {} sec, deals {d} damage to every enemy within {} yd of you",
            num(c.cooldown),
            num(c.radius)
        ),
        Kind::Orbit => format!(
            "Every {} sec, zaps {} within {} yd of you for {d} damage",
            num(c.cooldown),
            if c.count == 1 { "an enemy".to_string() } else { format!("{} enemies", c.count) },
            num(c.radius)
        ),
        Kind::Shield => format!("Absorbs the next {d} damage you take within {} sec", num(c.duration)),
        Kind::Immune => format!(
            "When an enemy closes in, makes you immune to all damage for {} sec",
            num(c.duration)
        ),
        Kind::Thorns => format!(
            "When enemies close in, deals {d} damage to every attacker for {} sec",
            num(c.duration)
        ),
        Kind::Heal => {
            let target = format!("Heals you or the living ally with the lowest health percentage within {} yd for {d}", num(c.range));
            if c.duration > 0.0 { format!("{target} over {} sec", num(c.duration)) } else { target }
        },
        Kind::Totem if def.key == "magma_totem" => format!(
            "Drops a totem for {} sec that burns every enemy within {} yd for {d} damage every 2 sec",
            num(c.duration),
            num(c.radius)
        ),
        Kind::Totem => format!(
            "Drops a totem for {} sec that hits the nearest enemy within {} yd for {d} damage every second",
            num(c.duration),
            num(c.range)
        ),
    };
    // Areas that tick carry no over-time part; a burst leaves a 5 second one.
    let ground = matches!(def.kind, Kind::Ground | Kind::GroundSelf);
    if c.dot > 0.0 && def.kind != Kind::Dot && (!ground || burst) {
        let secs = if ground { 5.0 } else { c.duration };
        s += &format!(
            ", then {} damage every second for {} sec",
            num(c.dot),
            num(secs)
        );
    }
    if let Some(e) = effect_text(c.effect) {
        s += ", ";
        s += &e;
    }
    s.push('.');
    s
}

/// What the next rank changes, at the current stats.
fn next_rank(def: &AbilityDef, now: &Cast, next: &Cast) -> String {
    let what = match def.kind {
        Kind::Shield => "absorbed",
        Kind::Heal => "healing",
        _ => "damage",
    };
    let mut gains = Vec::new();
    gains.extend(effect_changes(now.effect, next.effect));
    if next.dmg > 0.0 && def.kind != Kind::Dot && def.kind != Kind::Immune {
        gains.push(format!("{} {what}", num(next.dmg)));
    }
    if next.dot > 0.0 {
        gains.push(format!("{} damage per second", num(next.dot)));
    }
    if next.count != now.count
        && matches!(
            def.kind,
            Kind::Bolt | Kind::Strike | Kind::Melee | Kind::Dot | Kind::Chain | Kind::Orbit
        )
    {
        gains.push(match def.kind {
            Kind::Chain => format!("{} jumps", next.count),
            _ => format!("{} targets", next.count),
        });
    }
    if next.radius != now.radius {
        gains.push(format!("{} yd", num(next.radius)));
    }
    if next.duration != now.duration {
        gains.push(format!("{} sec", num(next.duration)));
    }
    gains.push(format!("{} sec cooldown", num(next.cooldown)));
    gains.join(", ")
}

fn proc_tip(proc: super::procs::ViewSpell) -> Vec<TipLine> {
    let mut lines = vec![TipLine::new(proc.name, Color::WHITE).header()];
    for (index, text) in proc.lines.into_iter().enumerate() {
        let color = if index == 0 {
            Color::WHITE
        } else if proc
            .description
            .is_some_and(|description| description == text)
        {
            GOLD
        } else {
            DIM
        };
        lines.push(TipLine::new(text, color));
    }
    lines
}

fn ability_tip(o: &Owned, run: &Run, spells: Option<&crate::ui_action::Spells>) -> Vec<TipLine> {
    let def = o.def;
    let now = run.resolve_spell(def, o.level);
    let support_description = super::support_spells::tooltip(def.key, o.level, &now);
    let mut lines = vec![
        TipLine::new(spell_name(spells, def.spell, def.key), Color::WHITE)
            .header()
            .right(
                format!(
                    "Rank {}/{}",
                    o.level,
                    super::specialization::rank_cap(o.level)
                ),
                TIP_GREY,
            ),
    ];
    let range = match def.kind {
        Kind::Melee => Some(format!("Melee · {} yd range", num(now.range))),
        Kind::Bolt | Kind::Strike | Kind::Chain | Kind::Dot | Kind::Ground | Kind::Totem => {
            Some(format!("{} yd range", num(now.range)))
        }
        _ => None,
    };
    let mut cast = TipLine::new("Casts itself", Color::WHITE);
    if let Some(range) = range {
        cast = cast.right(range, Color::WHITE);
    }
    lines.push(cast);
    lines.push(TipLine::new(now.tags.labels(), DIM));
    lines.push(
        TipLine::new("Instant", Color::WHITE)
            .right(format!("{} sec cooldown", num(now.cooldown)), Color::WHITE),
    );
    lines.push(TipLine::new(
        if def.kind == Kind::Buff {
            support_description.unwrap_or_else(|| ability_body(def, &now))
        } else {
            ability_body(def, &now)
        },
        GOLD,
    ));
    if let Some(buff) = super::healer_buffs::tooltip(run, def, o.level) {
        lines.push(TipLine::new(buff, GOLD));
    }
    if now.chains > 0 {
        lines.push(TipLine::new(
            format!("{} additional jumps", now.chains),
            DIM,
        ));
    }
    if now.forks > 0 {
        lines.push(TipLine::new(
            format!("Forks into two projectiles ({} generations)", now.forks),
            DIM,
        ));
    }
    if now.recast > 0.0 {
        lines.push(TipLine::new(
            format!("{} repeat chance", pct(now.recast)),
            DIM,
        ));
    }
    if !matches!(
        def.kind,
        Kind::Shield | Kind::Immune | Kind::Heal | Kind::Dot
    ) {
        lines.push(TipLine::new(
            format!(
                "{} chance to critically strike for {} damage",
                pct(now.crit),
                pct(now.crit_multiplier)
            ),
            DIM,
        ));
    }
    if o.level >= 6 {
        lines.push(TipLine::new(
            super::specialization::signature_text(def).trim_start_matches("Signature: "),
            GOLD,
        ));
    }
    let index = run
        .abilities
        .iter()
        .position(|owned| std::ptr::eq(owned, o));
    if index.is_some_and(|i| super::specialization::can_upgrade(run, i)) {
        let next = run.resolve_spell(def, o.level + 1);
        lines.push(TipLine::new("Next rank:", Color::WHITE));
        lines.push(TipLine::new(next_rank(def, &now, &next), GOLD));
        if super::support_spells::magnitude(def.key, 1) > 0.0 {
            lines.push(TipLine::new(
                format!(
                    "Bonus: {}",
                    super::support_spells::power_label(def.key, o.level + 1, next.buff_effect)
                ),
                GOLD,
            ));
        }
    } else {
        lines.push(TipLine::new(
            if o.level >= 6 {
                "Maximum rank"
            } else {
                "Rank limit reached"
            },
            DIM,
        ));
    }
    lines
}

/// What a passive's bonus `v` does, in the voice of a buff.
/// A stat's short name, for the card's before » after line.
fn stat_label(stat: Stat) -> &'static str {
    match stat {
        Stat::AllResistance => "All elemental resistances",
        Stat::DamageReduction => "Damage taken reduction",
        Stat::Might => "All damage",
        Stat::MaxHp => "Maximum health",
        Stat::Speed => "Movement speed",
        Stat::Regen => "Health per second",
        Stat::Haste => "Recharge speed",
        Stat::Area => "Area size",
        Stat::Magnet => "Pickup range",
        Stat::Growth => "Experience gained",
        Stat::Armor => "Armour",
        Stat::Crit => "Critical strike chance",
    }
}

fn passive_text(stat: Stat, v: f32) -> String {
    match stat {
        Stat::AllResistance => format!(
            "Adds {} percentage points to every elemental resistance.",
            num(v)
        ),
        Stat::DamageReduction => format!("Reduces all damage taken by {}.", pct(v)),
        Stat::Might => format!("Increases all damage you deal by {}.", pct(v)),
        Stat::MaxHp => format!("Increases your maximum health by {}.", pct(v)),
        Stat::Speed => format!("Increases your movement speed by {}.", pct(v)),
        Stat::Regen => format!("Restores {} health every second.", num(v)),
        Stat::Haste => format!("Your abilities recharge {} faster.", pct(v)),
        Stat::Area => format!("Increases the size of your areas of effect by {}.", pct(v)),
        Stat::Magnet => format!("Increases your pickup range by {}.", pct(v)),
        Stat::Growth => format!("Increases the experience you gain by {}.", pct(v)),
        Stat::Armor => format!("Increases your armour by {}.", num(v)),
        Stat::Crit => format!("Increases your critical strike chance by {}.", pct(v)),
    }
}

/// The hero's total for the stat a passive raises, every source counted.
fn stat_total(stat: Stat, s: &Stats) -> String {
    match stat {
        Stat::AllResistance => format!(
            "Minimum elemental resistance: {}%",
            num(s
                .resistances
                .entries()
                .iter()
                .map(|(_, _, v)| *v)
                .fold(75.0, f32::min)
                .clamp(0.0, 75.0))
        ),
        Stat::DamageReduction => format!(
            "Damage taken reduction: {}",
            pct(s.damage_reduction.clamp(0.0, 0.75))
        ),
        Stat::Might => format!("Damage dealt: {}", pct(s.might)),
        Stat::MaxHp => format!("Maximum health: {}", num(s.max_hp)),
        Stat::Speed => format!("Movement speed: {} yd per second", num(s.speed)),
        Stat::Regen => format!("Health restored: {} per second", num(s.regen)),
        Stat::Haste => format!("Cooldowns recharge {} faster", pct(s.haste)),
        Stat::Area => format!("Area of effect: {}", pct(s.area)),
        Stat::Magnet => format!("Pickup range: {} yd", num(s.magnet)),
        Stat::Growth => format!("Experience gained: {}", pct(s.growth)),
        Stat::Armor => format!(
            "Armour: {} (damage reduction: {:.1}%)",
            num(s.armor),
            super::items::armour::reduction(s.armor) * 100.0
        ),
        Stat::Crit => format!("Critical strike chance: {}", pct(s.crit)),
    }
}

/// The picker shows only this passive's cumulative bonus, not a second tooltip.
fn passive_card_delta(p: &PassiveDef, before: u64, after: u64) -> String {
    match p.effect {
        data::PassiveEffect::Stat(stat, amount) => {
            let value = |rank| {
                let v = amount * rank as f32;
                if matches!(stat, Stat::Armor | Stat::Regen) {
                    format!("+{}", num(v))
                } else {
                    format!("+{}", pct(v))
                }
            };
            format!("{} {} » {}", stat_label(stat), value(before), value(after))
        }
        data::PassiveEffect::Modifier(filter, change) => {
            // Engine descriptions consistently start with the numeric bonus, followed by its label.
            let a = change.scaled(before).description();
            let b = change.scaled(after).description();
            let (old, _) = a.split_once(' ').unwrap_or((&a, ""));
            let (new, label) = b.split_once(' ').unwrap_or((&b, ""));
            let mut label = label.to_owned();
            if let Some(first) = label.get_mut(..1) {
                first.make_ascii_uppercase();
            }
            let scope = filter.all.labels();
            let line = format!("{label} {old} » {new}");
            if scope.is_empty() {
                line
            } else {
                format!("{line}\n{scope}")
            }
        }
    }
}

pub(super) fn passive_effect_text(p: &PassiveDef, rank: u64) -> String {
    match p.effect {
        data::PassiveEffect::Stat(stat, amount) => format!(
            "{}: {}",
            stat_label(stat),
            passive_text(stat, amount * rank as f32)
        ),
        data::PassiveEffect::Modifier(filter, change) => format!(
            "{}\nApplies to: {}",
            change.scaled(rank).description(),
            super::passives::scope(filter, change)
        ),
    }
}
fn passive_tip(p: &PassiveDef, level: u64, stats: &Stats) -> Vec<TipLine> {
    let mut lines = vec![
        TipLine::new(p.name, Color::WHITE)
            .header()
            .right(format!("{level} stacks | Unlimited"), TIP_GREY),
        TipLine::new(passive_effect_text(p, level), GOLD),
    ];
    if let data::PassiveEffect::Stat(stat, _) = p.effect {
        lines.push(TipLine::new(stat_total(stat, stats), Color::WHITE));
    }
    lines.push(TipLine::new("Next stack:", Color::WHITE));
    lines.push(TipLine::new(
        passive_effect_text(p, level.saturating_add(1)),
        GOLD,
    ));
    lines
}

/// The `GameTooltip` frame: `UI-Tooltip-Background` tiled under `UI-Tooltip-Border`, 16-unit edges
/// and 5-unit insets, its lines 10 units in. Returns the frame and its line column.
fn spawn_tooltip(
    commands: &mut Commands,
    mut assets: Option<&mut benilla_assets::WorldAssets>,
    images: &mut Assets<Image>,
    ui_scale: f32,
) -> (Entity, Entity) {
    use crate::glue::backdrop::{backdrop_border, backdrop_edges, tiled_bg_node};
    let bg = assets.as_deref_mut().and_then(|assets| {
        assets.sprite_texture("Interface\\Tooltips\\UI-Tooltip-Background", images)
    });
    let edges = assets.and_then(|assets| {
        backdrop_edges(assets, "Interface\\Tooltips\\UI-Tooltip-Border", images)
    });
    let mut body = Entity::PLACEHOLDER;
    let mut root = commands.spawn((
        GameTooltip,
        Node {
            position_type: PositionType::Absolute,
            max_width: Val::Px(300.0),
            ..default()
        },
        GlobalZIndex(1000),
        Visibility::Hidden,
        bevy::ui::FocusPolicy::Pass,
    ));
    if bg.is_none() || edges.is_none() {
        root.insert(BackgroundColor(TIP_FILL.with_alpha(0.95)));
    }
    root.with_children(|t| {
        if let Some(bg) = bg {
            t.spawn((
                tiled_bg_node(bg, 16.0, ui_scale, TIP_FILL),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(5.0),
                    right: Val::Px(5.0),
                    top: Val::Px(5.0),
                    bottom: Val::Px(5.0),
                    ..default()
                },
            ));
        }
        if let Some(edges) = &edges {
            backdrop_border(t, edges, 16.0, Color::WHITE);
        }
        body = t
            .spawn((
                TipBody,
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(2.0),
                    padding: UiRect::all(Val::Px(10.0)),
                    ..default()
                },
            ))
            .id();
    });
    (root.id(), body)
}

/// Hovering a build icon raises its `GameTooltip` under the icon, every number from the current
/// rank and stats, and what the next rank brings.
#[allow(clippy::too_many_arguments)]
pub(super) fn icon_tooltip(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    run: Res<Run>,
    spells: Option<Res<crate::ui_action::Spells>>,
    font: Res<HudFont>,
    ui_scale: Res<UiScale>,
    mut assets: Option<ResMut<benilla_assets::WorldAssets>>,
    mut images: ResMut<Assets<Image>>,
    slots: Query<(
        &Interaction,
        &TipFor,
        &ComputedNode,
        &bevy::ui::UiGlobalTransform,
    )>,
    mut frames: Query<(&mut Node, &mut Visibility), With<GameTooltip>>,
    mut built: Local<Option<(Entity, Entity)>>,
    mut shown: Local<Option<(TipFor, Vec<TipLine>)>>,
) {
    let Some((frame, body)) = *built else {
        *built = Some(spawn_tooltip(
            &mut commands,
            assets.as_deref_mut(),
            &mut images,
            ui_scale.0,
        ));
        return;
    };
    let Ok((mut node, mut vis)) = frames.get_mut(frame) else {
        *built = None;
        *shown = None;
        return;
    };
    let hovered = slots
        .iter()
        .find(|(i, ..)| **i != Interaction::None)
        .filter(|_| *phase.get() != Phase::Menu);
    let Some((_, &tip, computed, at)) = hovered else {
        if *vis != Visibility::Hidden {
            *vis = Visibility::Hidden;
        }
        *shown = None;
        return;
    };
    // Physical pixels to the 768-unit canvas the `Val`s are on.
    let inv = computed.inverse_scale_factor;
    let (left, top) = (
        Val::Px((at.translation.x - computed.size().x / 2.0) * inv),
        Val::Px((at.translation.y + computed.size().y / 2.0) * inv + 4.0),
    );
    if node.left != left || node.top != top {
        node.left = left;
        node.top = top;
    }
    let stats = run.stats();
    let lines = match tip {
        TipFor::Proc(key) => super::procs::views(&run)
            .into_iter()
            .find(|p| p.key == key)
            .map(proc_tip),
        TipFor::Ability(i) => run
            .abilities
            .get(i)
            .map(|o| ability_tip(o, &run, spells.as_deref())),
        TipFor::Passive(i) => run
            .passives
            .get(i)
            .map(|&(p, level)| passive_tip(p, level, &stats)),
        TipFor::Mobility => run.mobility().map(|m| {
            let resolved = super::spell_engine::resolve_mobility(m, run.spell_modifiers.entries());
            vec![
                TipLine::new(spell_name(spells.as_deref(), m.spell, m.name), Color::WHITE).header(),
                TipLine::new("Movement skill key", Color::WHITE).right(
                    format!("{} sec cooldown", num(resolved.cooldown)),
                    Color::WHITE,
                ),
                TipLine::new(resolved.tags.labels(), DIM),
                TipLine::new(m.describe(&resolved), GOLD),
            ]
        }),
    };
    let Some(lines) = lines else {
        *vis = Visibility::Hidden;
        *shown = None;
        return;
    };
    *vis = Visibility::Inherited;
    if shown
        .as_ref()
        .is_some_and(|(previous, content)| *previous == tip && content == &lines)
    {
        return;
    }
    // Compare the displayed values so unrelated version changes cannot reset the tooltip.
    *shown = Some((tip, lines));
    let lines = &shown.as_ref().unwrap().1;
    let f = &font.0;
    commands.entity(body).despawn_related::<Children>();
    commands.entity(body).with_children(|b| {
        for line in lines {
            let face = |s: String, size: f32, color: Color| {
                (
                    Text::new(s),
                    TextFont {
                        font: f.clone(),
                        font_size: size,
                        ..default()
                    },
                    TextColor(color),
                    TextShadow {
                        offset: Vec2::splat(1.0),
                        color: Color::BLACK,
                    },
                )
            };
            b.spawn(Node {
                justify_content: JustifyContent::SpaceBetween,
                column_gap: Val::Px(16.0),
                ..default()
            })
            .with_children(|row| {
                row.spawn(face(line.left.clone(), line.size, line.color));
                if let Some((text, color)) = &line.right {
                    row.spawn((
                        face(text.clone(), line.size, *color),
                        TextLayout::new(Justify::Right, LineBreak::NoWrap),
                    ));
                }
            });
        }
    });
}

#[allow(clippy::type_complexity)]
pub(super) fn show_panels(
    phase: Res<State<Phase>>,
    roster: Res<super::characters::Roster>,
    mut roots: ParamSet<(
        Query<(&mut Visibility, &mut Node), With<MenuRoot>>,
        Query<(&mut Visibility, &mut Node), With<HudRoot>>,
        Query<(&mut Visibility, &mut Node), With<LevelUpRoot>>,
        Query<(&mut Visibility, &mut Node), With<PauseRoot>>,
        Query<(&mut Visibility, &mut Node), With<OverRoot>>,
    )>,
) {
    let p = *phase.get();
    // A hidden panel leaves the layout too: a merely invisible button still takes clicks.
    let set = |(mut v, mut node): (Mut<Visibility>, Mut<Node>), on: bool| {
        let want = if on {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *v != want {
            *v = want;
        }
        let display = if on { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
    };
    for root in &mut roots.p0() {
        set(root, p == Phase::Menu && roster.creating);
    }
    for root in &mut roots.p1() {
        set(
            root,
            !matches!(p, Phase::Menu | Phase::GameOver | Phase::Hub),
        );
    }
    for root in &mut roots.p2() {
        set(root, p == Phase::LevelUp);
    }
    for root in &mut roots.p3() {
        set(root, p == Phase::Paused);
    }
    for root in &mut roots.p4() {
        set(root, p == Phase::GameOver);
    }
}
