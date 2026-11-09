//! Item power and loot quality are independent of the temporary run level.
use super::*;

pub(super) fn factor(level: u32) -> f32 {
    1.015_f32.powi(level.clamp(1, super::super::dungeon_config::MAX_REWARD_LEVEL) as i32 - 1)
}

pub(super) fn growth(effect: Effect, level: u32) -> f32 {
    if effect == Effect::CritChance && level > 30 {
        factor(30) + (factor(level) - factor(30)) * 0.5
    } else {
        factor(level)
    }
}

pub(super) fn roll_max(effect: Effect, maximum: f32, rarity: Rarity) -> f32 {
    let multiplier = match rarity {
        Rarity::Normal => 1.0,
        Rarity::Magic => 1.25,
        Rarity::Epic => 1.5,
        Rarity::Legendary => 2.0,
    };
    let maximum = (maximum * multiplier).min(1.0e6);
    if effect.discrete() {
        maximum.round()
    } else {
        maximum
    }
}

pub(super) fn scale(effect: Effect, value: f32, level: u32) -> f32 {
    // Extra targets/projectiles are structural counts, rather than scalar power.
    if matches!(
        effect,
        Effect::Projectiles | Effect::Chain | Effect::Fork | Effect::MeleeTargets
    ) {
        return value;
    }
    // Preserve the level-30 value; only subsequent critical-chance growth is halved.
    let scaled = value * growth(effect, level);
    let scaled = if effect.discrete() {
        scaled.round()
    } else {
        scaled
    };
    if matches!(
        effect,
        Effect::BlockChance
            | Effect::LifeOnBlockPercent
            | Effect::Proc
            | Effect::CritChance
            | Effect::Repeat
            | Effect::LifeLeech
            | Effect::DamageReduction
            | Effect::AilmentChance
            | Effect::BleedChance
            | Effect::IgniteChance
            | Effect::ChillChance
            | Effect::KnockbackChance
    ) {
        scaled.min(1.0)
    } else {
        scaled.min(1.0e6)
    }
}

pub(super) fn quality_weights(weights: [f64; 3], level: u32, magic_find: f32) -> [f64; 3] {
    let progression = f64::from(level.clamp(1, 100) - 1) / 59.0;
    let find = if magic_find.is_finite() {
        f64::from(magic_find.max(0.0))
    } else {
        0.0
    };
    let bonus = 1.0 + find;
    [
        weights[0],
        weights[1] * (0.25 + progression) * bonus,
        weights[2] * (0.01 + progression * progression * 2.0) * bonus,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maximum_dungeon_rewards_keep_growing_beyond_one_hundred() {
        let maximum = super::super::super::data::DUNGEON_ITEM_LEVEL
            + super::super::super::dungeon_mods::MAX_BONUS;
        assert_eq!(maximum, 110);
        assert!(factor(maximum) > factor(100));
        assert!(scale(Effect::Armour, 10.0, maximum) > scale(Effect::Armour, 10.0, 100));
    }

    #[test]
    fn critical_affix_growth_is_halved_above_thirty_without_losing_existing_power() {
        let at_thirty = scale(Effect::CritChance, 0.04, 30);
        for level in [1, 30] {
            assert!((scale(Effect::CritChance, 0.04, level) - 0.04 * factor(level)).abs() < 1e-6);
        }
        for level in [31, 60, 100] {
            let actual = scale(Effect::CritChance, 0.04, level);
            let original_growth = 0.04 * (factor(level) - factor(30));
            assert!(actual > at_thirty);
            assert!((actual - at_thirty - original_growth * 0.5).abs() < 1e-6);
        }
    }

    #[test]
    fn regeneration_affix_scales_to_two_percent_at_sixty_and_uses_maximum_life() {
        let config: toml::Value = toml::from_str(include_str!(
            "../../../../../config/survivors-item-affixes.toml"
        ))
        .unwrap();
        let affix = config["affixes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["id"].as_str() == Some("regeneration"))
            .unwrap();
        assert_eq!(affix["effect"].as_str(), Some("regeneration_percent"));
        let value = scale(
            Effect::RegenerationPercent,
            affix["max"].as_float().unwrap() as f32,
            60,
        );
        assert!((value - 0.02).abs() < 1e-6);
        let mut run = Run::default();
        let before = run.stats();
        run.equipment.regeneration_percent = value;
        let after = run.stats();
        assert!((after.regen - before.regen - after.max_hp * 0.02).abs() < 1e-4);
        run.equipment_hp += 100.0;
        let boosted = run.stats();
        assert!((boosted.regen - after.regen - 2.0).abs() < 1e-4);
    }

    #[test]
    fn rarity_increases_roll_ceilings_and_preserves_integer_stats() {
        for (rarity, expected) in [
            (Rarity::Normal, 20.0),
            (Rarity::Magic, 25.0),
            (Rarity::Epic, 30.0),
            (Rarity::Legendary, 40.0),
        ] {
            assert_eq!(roll_max(Effect::Strength, 20.0, rarity), expected);
            assert_eq!(roll_max(Effect::Projectiles, 1.0, rarity).fract(), 0.0);
        }
        assert_eq!(
            scale(Effect::Proc, roll_max(Effect::Proc, 0.9, Rarity::Epic), 60),
            1.0
        );
    }

    #[test]
    fn power_grows_gently_and_keeps_counts_and_chances_valid() {
        assert_eq!(factor(1), 1.0);
        assert!((factor(60) - 2.407).abs() < 0.01);
        assert!(factor(30) < factor(60));
        assert_eq!(scale(Effect::Projectiles, 2.0, 60), 2.0);
        assert_eq!(scale(Effect::BlockChance, 0.8, 60), 1.0);
        assert_eq!(scale(Effect::Strength, 10.0, 60).fract(), 0.0);
        assert!(scale(Effect::MagicFind, 0.1, 60) > 0.2);
    }

    #[test]
    fn boss_item_levels_increase_within_each_zone_and_between_zones() {
        let mut previous = 0;
        for &map in super::super::super::data::CAMPAIGN {
            let roster = super::super::super::data::roster(map);
            for boss in &roster.bosses {
                let level = boss.level;
                assert!(level > previous);
                previous = level;
            }
        }
    }

    #[test]
    fn level_and_magic_find_improve_quality_without_enabling_disabled_rarities() {
        let low = quality_weights([65.0, 30.0, 5.0], 1, 0.0);
        let high = quality_weights([65.0, 30.0, 5.0], 60, 0.0);
        let found = quality_weights([65.0, 30.0, 5.0], 60, 1.0);
        assert!(low[0] / low.iter().sum::<f64>() > 0.89);
        assert!(high[2] / high.iter().sum::<f64>() > 0.08);
        assert_eq!(found[0], high[0]);
        assert_eq!(found[1], high[1] * 2.0);
        assert_eq!(found[2], high[2] * 2.0);
        assert_eq!(quality_weights([1.0, 0.0, 0.0], 60, 100.0), [1.0, 0.0, 0.0]);
    }
}
