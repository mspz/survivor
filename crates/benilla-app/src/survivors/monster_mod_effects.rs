//! Shared monster mod effects and caster tuning, independent of expedition balance.

use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
const FILE: &str = include_str!("../../../../config/survivors-monster-mod-effects.toml");

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Config {
    version: u32,
    #[serde(default = "default_ground_damage")]
    pub monster_ground_damage_multiplier: f32,
    #[serde(default)]
    pub elites: Elites,
    #[serde(default)]
    pub sprinters: Sprinters,
    #[serde(default)]
    pub explosives: Explosives,
    #[serde(default)]
    pub healers: Healers,
    #[serde(default)]
    pub protectors: Protectors,
    #[serde(default)]
    pub ranged: Ranged,
    #[serde(default)]
    pub stats: StatEffects,
    #[serde(default)]
    pub special: super::monster_specials::Config,
}
fn default_ground_damage() -> f32 { 1.5 }

#[derive(Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct StatEffects {
    pub action_speed: f32,
    pub physical_damage: f32,
    pub added_elemental: f32,
    pub dot_damage: f32,
    pub critical_chance: f32,
    pub critical_multiplier: f32,
    pub projectiles: u32,
    pub chains: u32,
    pub area: f32,
    pub range: f32,
    pub physical_taken: f32,
    pub elemental_taken: f32,
    pub control_duration: f32,
    pub slow_effect: f32,
    pub regeneration: f32,
    pub life_leech: f32,
    #[serde(rename = "thorns_per_level", skip_serializing)]
    _obsolete_thorns_per_level: Option<f32>,
    pub aura_radius: f32,
    pub aura_action_speed: f32,
    pub oppression: f32,
}
impl Default for StatEffects {
    fn default() -> Self {
        Self {
            action_speed: 1.4,
            physical_damage: 1.4,
            added_elemental: 0.35,
            dot_damage: 1.75,
            critical_chance: 0.15,
            critical_multiplier: 0.75,
            projectiles: 2,
            chains: 2,
            area: 1.75,
            range: 1.4,
            physical_taken: 0.8,
            elemental_taken: 0.8,
            control_duration: 0.5,
            slow_effect: 0.5,
            regeneration: 0.05,
            life_leech: 0.05,
            _obsolete_thorns_per_level: None,
            aura_radius: 10.0,
            aura_action_speed: 1.2,
            oppression: 0.15,
        }
    }
}
impl StatEffects {
    fn valid(&self) -> bool {
        [
            self.action_speed,
            self.physical_damage,
            self.added_elemental,
            self.dot_damage,
            self.critical_chance,
            self.critical_multiplier,
            self.area,
            self.range,
            self.physical_taken,
            self.elemental_taken,
            self.control_duration,
            self.slow_effect,
            self.regeneration,
            self.life_leech,
            self.aura_radius,
            self.aura_action_speed,
            self.oppression,
        ]
        .into_iter()
        .all(|v| v.is_finite() && (0.0..=1000.0).contains(&v))
            && [
                self.action_speed,
                self.physical_damage,
                self.dot_damage,
                self.area,
                self.range,
                self.physical_taken,
                self.elemental_taken,
                self.control_duration,
                self.aura_action_speed,
                self.aura_radius,
            ]
            .into_iter()
            .all(|v| v > 0.0)
            && [
                self.physical_taken,
                self.elemental_taken,
                self.control_duration,
                self.slow_effect,
                self.regeneration,
                self.life_leech,
                self.oppression,
                self.critical_chance,
            ]
            .into_iter()
            .all(|v| v <= 1.0)
            && self.oppression < 1.0
            && self.projectiles <= 8
            && self.chains <= 8
            && self.aura_radius >= 0.1
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct Elites {
    pub health_multiplier: f32,
    pub size_multiplier: f32,
    pub damage_reduction: f32,
    pub xp_multiplier: u32,
}
impl Default for Elites {
    fn default() -> Self {
        Self {
            health_multiplier: 2.5,
            size_multiplier: 1.25,
            damage_reduction: 0.0,
            xp_multiplier: 3,
        }
    }
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct Sprinters {
    pub speed_multiplier: f32,
}
impl Default for Sprinters {
    fn default() -> Self {
        Self {
            speed_multiplier: 1.5,
        }
    }
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct Explosives {
    pub radius: f32,
    pub delay: f32,
    pub damage_multiplier: f32,
}
impl Default for Explosives {
    fn default() -> Self {
        Self {
            radius: 3.5,
            delay: 1.5,
            damage_multiplier: 2.0,
        }
    }
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct Healers {
    pub radius: f32,
    pub interval: f32,
    pub heal_fraction: f32,
}
impl Default for Healers {
    fn default() -> Self {
        Self {
            radius: 10.0,
            interval: 4.0,
            heal_fraction: 0.1,
        }
    }
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct Protectors {
    pub radius: f32,
    pub damage_reduction: f32,
}
impl Default for Protectors {
    fn default() -> Self {
        Self {
            radius: 8.0,
            damage_reduction: 0.25,
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct Ranged {
    pub chance: f32,
    pub range: f32,
    pub stand_off: f32,
    pub interval: f32,
    pub windup: f32,
}
impl Default for Ranged {
    fn default() -> Self {
        Self {
            chance: 0.02,
            range: 20.0,
            stand_off: 16.0,
            interval: 3.0,
            windup: 1.0,
        }
    }
}

impl Config {
    pub(super) fn parse(text: &str) -> Result<Self, String> {
        let c: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        let valid = |v: f32| v.is_finite() && v > 0.0 && v <= 1000.0;
        if c.version != 1
            || !valid(c.monster_ground_damage_multiplier)
            || !c.special.valid()
            || !c.stats.valid()
            || ![
                c.elites.health_multiplier,
                c.elites.size_multiplier,
                c.sprinters.speed_multiplier,
                c.explosives.radius,
                c.explosives.delay,
                c.explosives.damage_multiplier,
                c.healers.radius,
                c.healers.interval,
                c.protectors.radius,
            ]
            .into_iter()
            .all(valid)
            || !c.healers.heal_fraction.is_finite()
            || !(0.0..=1.0).contains(&c.healers.heal_fraction)
            || !c.protectors.damage_reduction.is_finite()
            || !(0.0..1.0).contains(&c.protectors.damage_reduction)
            || !c.elites.damage_reduction.is_finite()
            || !(0.0..1.0).contains(&c.elites.damage_reduction)
            || c.elites.xp_multiplier == 0
            || c.elites.xp_multiplier > 1000
            || !c.ranged.chance.is_finite()
            || !(0.0..=1.0).contains(&c.ranged.chance)
            || ![
                c.ranged.range,
                c.ranged.stand_off,
                c.ranged.interval,
                c.ranged.windup,
            ]
            .into_iter()
            .all(valid)
            || c.ranged.stand_off >= c.ranged.range
        {
            return Err("invalid monster mod effect settings".into());
        }
        Ok(c)
    }
}

/// One shared effect snapshot for campaign, Endless and dungeon mods; restart after disk edits.
pub(super) fn get() -> &'static Config {
    static CONFIG: LazyLock<Config> = LazyLock::new(|| {
        let builtin = || Config::parse(FILE).expect("valid embedded monster mod effects");
        if cfg!(test) {
            return builtin();
        }
        match crate::local_state::read_to_string(std::path::Path::new(
            "config/survivors-monster-mod-effects.toml",
        )) {
            Ok(text) => Config::parse(&text).unwrap_or_else(|error| {
                bevy::log::warn!("Invalid monster mod effects: {error}; using embedded defaults");
                builtin()
            }),
            Err(_) => builtin(),
        }
    });
    &CONFIG
}

#[cfg(test)]
mod tests {
    #[test]
    fn special_caps_and_timers_reject_invalid_configuration() {
        for (key, value) in [("death_reduction_cap", 0.61), ("block_cap", 0.76),
            ("blood_heal", 0.51), ("skin_shield", 1.01), ("adaptation_taken", 1.01),
            ("magnetic_windup", 0.0), ("boss_repeat_delay", 0.0)] {
            let mut table: toml::Value = toml::from_str(super::FILE).unwrap();
            table["special"][key] = toml::Value::Float(value);
            assert!(super::Config::parse(&toml::to_string(&table).unwrap()).is_err(), "{key}");
        }
    }
    use super::*;
    #[test]
    fn stat_effects_reject_invalid_multipliers_regeneration_and_projectile_counts() {
        for (key, value) in [
            ("physical_taken", -0.8),
            ("elemental_taken", 1.2),
            ("regeneration", 1.1),
            ("control_duration", 0.0),
            ("oppression", 1.0),
        ] {
            let mut table: toml::Value = toml::from_str(FILE).unwrap();
            table["stats"][key] = toml::Value::Float(value);
            assert!(
                Config::parse(&toml::to_string(&table).unwrap()).is_err(),
                "{key}"
            );
        }
        assert_eq!(get().stats.regeneration, 0.05);
        assert_eq!(get().stats.physical_taken, 0.8);
        assert_eq!(get().stats.elemental_taken, 0.8);
    }
    use crate::survivors::data;
    #[test]
    fn ranged_profiles_validate_probability_geometry_and_timing() {
        for (field, value) in [
            ("chance", -0.1),
            ("chance", 1.1),
            ("range", 0.0),
            ("stand_off", 20.0),
            ("interval", 0.0),
            ("windup", 0.0),
        ] {
            let mut table: toml::Value = toml::from_str(FILE).unwrap();
            table["ranged"][field] = toml::Value::Float(value);
            assert!(Config::parse(&toml::to_string(&table).unwrap()).is_err());
        }
    }

    #[test]
    fn all_modes_use_the_shared_types_without_serializing_them_into_balance_documents() {
        let check = |engine: super::super::expedition_config::Config| {
            assert_eq!(
                engine.elites.health_multiplier,
                get().elites.health_multiplier
            );
            assert_eq!(
                engine.sprinters.speed_multiplier,
                get().sprinters.speed_multiplier
            );
            assert_eq!(engine.explosives.delay, get().explosives.delay);
            assert_eq!(engine.healers.interval, get().healers.interval);
            assert_eq!(
                engine.protectors.damage_reduction,
                get().protectors.damage_reduction
            );
            assert_eq!(engine.ranged_chance(), get().ranged.chance);
            let serialized: toml::Value = toml::Value::try_from(&engine).unwrap();
            for key in [
                "elites",
                "sprinters",
                "explosives",
                "healers",
                "protectors",
                "ranged",
                "ranged_monster_chance",
            ] {
                assert!(
                    serialized.get(key).is_none(),
                    "{key} belongs in the shared file"
                );
            }
        };
        check(super::super::expedition_config::Config::default());
        for &map in data::DUNGEONS {
            check(super::super::dungeon_config::get(map).expedition().unwrap());
        }
        let mut run = super::super::Run::default();
        super::super::endless::begin(&mut run);
        check(run.expeditions);
        let caster = super::super::ranged_enemies::profile(data::CAMPAIGN[1]).unwrap();
        assert_eq!(caster.range, get().ranged.range);
        assert_eq!(caster.interval, get().ranged.interval);
    }
}
