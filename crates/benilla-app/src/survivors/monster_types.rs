//! Shared monster variants and caster tuning, independent of expedition balance.
use super::data;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
const FILE: &str = include_str!("../../../../config/survivors-monster-types.toml");

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Config {
    version: u32,
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
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct Elites {
    /// Campaign zone number, starting at 1; the variant remains available afterwards.
    pub first_zone: usize,
    pub chance: f32,
    pub health_multiplier: f32,
    pub size_multiplier: f32,
    pub damage_reduction: f32,
    pub xp_multiplier: u32,
}
impl Default for Elites {
    fn default() -> Self {
        Self {
            first_zone: 2,
            chance: 0.02,
            health_multiplier: 10.0,
            size_multiplier: 1.5,
            damage_reduction: 0.2,
            xp_multiplier: 3,
        }
    }
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct Sprinters {
    /// Campaign zone number, starting at 1; the variant remains available afterwards.
    pub first_zone: usize,
    pub chance: f32,
    pub speed_multiplier: f32,
}
impl Default for Sprinters {
    fn default() -> Self {
        Self {
            first_zone: 3,
            chance: 0.03,
            speed_multiplier: 3.0,
        }
    }
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct Explosives {
    /// Campaign zone number, starting at 1; the variant remains available afterwards.
    pub first_zone: usize,
    pub chance: f32,
    pub radius: f32,
    pub delay: f32,
    pub damage_multiplier: f32,
}
impl Default for Explosives {
    fn default() -> Self {
        Self {
            first_zone: 4,
            chance: 0.03,
            radius: 3.5,
            delay: 1.5,
            damage_multiplier: 2.0,
        }
    }
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct Healers {
    /// Campaign zone number, starting at 1; the variant remains available afterwards.
    pub first_zone: usize,
    pub chance: f32,
    pub radius: f32,
    pub interval: f32,
    pub heal_fraction: f32,
}
impl Default for Healers {
    fn default() -> Self {
        Self {
            first_zone: 5,
            chance: 0.02,
            radius: 10.0,
            interval: 4.0,
            heal_fraction: 0.1,
        }
    }
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct Protectors {
    /// Campaign zone number, starting at 1; the variant remains available afterwards.
    pub first_zone: usize,
    pub chance: f32,
    pub radius: f32,
    pub damage_reduction: f32,
}
impl Default for Protectors {
    fn default() -> Self {
        Self {
            first_zone: 6,
            chance: 0.02,
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
            || ![
                c.elites.first_zone,
                c.sprinters.first_zone,
                c.explosives.first_zone,
                c.healers.first_zone,
                c.protectors.first_zone,
            ]
            .into_iter()
            .all(|zone| (1..=data::CAMPAIGN.len()).contains(&zone))
            || ![
                c.elites.chance,
                c.sprinters.chance,
                c.explosives.chance,
                c.healers.chance,
                c.protectors.chance,
            ]
            .into_iter()
            .all(|v| v.is_finite() && (0.0..=1.0).contains(&v))
            || c.elites.chance
                + c.sprinters.chance
                + c.explosives.chance
                + c.healers.chance
                + c.protectors.chance
                > 1.0
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
            return Err("invalid monster type settings".into());
        }
        Ok(c)
    }
}

/// One shared snapshot for campaign, Endless and dungeon variants; restart after disk edits.
pub(super) fn get() -> &'static Config {
    static CONFIG: LazyLock<Config> = LazyLock::new(|| {
        let builtin = || Config::parse(FILE).expect("valid embedded monster types");
        if cfg!(test) {
            return builtin();
        }
        match crate::local_state::read_to_string(std::path::Path::new(
            "config/survivors-monster-types.toml",
        )) {
            Ok(text) => Config::parse(&text).unwrap_or_else(|error| {
                bevy::log::warn!("Invalid monster types: {error}; using embedded defaults");
                builtin()
            }),
            Err(_) => builtin(),
        }
    });
    &CONFIG
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn variants_are_configurable_and_reject_invalid_settings() {
        let mut table: toml::Value = toml::from_str(FILE).unwrap();
        table["elites"]["chance"] = toml::Value::Float(0.1);
        table["elites"]["health_multiplier"] = toml::Value::Float(15.0);
        table["sprinters"]["chance"] = toml::Value::Float(0.2);
        table["sprinters"]["speed_multiplier"] = toml::Value::Float(4.0);
        let c = Config::parse(&toml::to_string(&table).unwrap()).unwrap();
        assert_eq!(c.elites.chance, 0.1);
        assert_eq!(c.elites.health_multiplier, 15.0);
        assert_eq!(c.sprinters.chance, 0.2);
        assert_eq!(c.sprinters.speed_multiplier, 4.0);
        for (group, key, value) in [
            ("elites", "chance", -0.1),
            ("sprinters", "chance", 1.0),
            ("elites", "damage_reduction", 1.0),
            ("elites", "health_multiplier", 0.0),
            ("sprinters", "speed_multiplier", 0.0),
        ] {
            let mut invalid = table.clone();
            invalid[group][key] = toml::Value::Float(value);
            assert!(Config::parse(&toml::to_string(&invalid).unwrap()).is_err());
        }
        table.as_table_mut().unwrap().remove("elites");
        table.as_table_mut().unwrap().remove("sprinters");
        assert!(Config::parse(&toml::to_string(&table).unwrap()).is_ok());
    }
    #[test]
    fn support_and_explosive_settings_validate_ranges_and_legacy_defaults() {
        let mut table: toml::Value = toml::from_str(FILE).unwrap();
        for (group, key, value) in [
            ("explosives", "delay", 0.0),
            ("explosives", "radius", -1.0),
            ("explosives", "damage_multiplier", -1.0),
            ("healers", "interval", 0.0),
            ("healers", "heal_fraction", 1.1),
            ("protectors", "damage_reduction", 1.0),
            ("protectors", "chance", 1.0),
        ] {
            let mut invalid = table.clone();
            invalid[group][key] = toml::Value::Float(value);
            assert!(Config::parse(&toml::to_string(&invalid).unwrap()).is_err());
        }
        table["healers"]["radius"] = toml::Value::Float(15.0);
        table["protectors"]["damage_reduction"] = toml::Value::Float(0.3);
        let config = Config::parse(&toml::to_string(&table).unwrap()).unwrap();
        assert_eq!(config.healers.radius, 15.0);
        assert_eq!(config.protectors.damage_reduction, 0.3);
        for group in ["explosives", "healers", "protectors"] {
            table.as_table_mut().unwrap().remove(group);
        }
        assert!(Config::parse(&toml::to_string(&table).unwrap()).is_ok());
    }
    #[test]
    fn variant_introductions_are_configurable_and_validate_campaign_zone_numbers() {
        let mut table: toml::Value = toml::from_str(FILE).unwrap();
        for (group, expected) in [
            ("elites", 2),
            ("sprinters", 3),
            ("explosives", 4),
            ("healers", 5),
            ("protectors", 6),
        ] {
            assert_eq!(table[group]["first_zone"].as_integer(), Some(expected));
            for invalid in [0, 7] {
                let mut config = table.clone();
                config[group]["first_zone"] = toml::Value::Integer(invalid);
                assert!(Config::parse(&toml::to_string(&config).unwrap()).is_err());
            }
            table[group].as_table_mut().unwrap().remove("first_zone");
        }
        let legacy = Config::parse(&toml::to_string(&table).unwrap()).unwrap();
        assert_eq!(
            [
                legacy.elites.first_zone,
                legacy.sprinters.first_zone,
                legacy.explosives.first_zone,
                legacy.healers.first_zone,
                legacy.protectors.first_zone
            ],
            [2, 3, 4, 5, 6]
        );
        table["protectors"]
            .as_table_mut()
            .unwrap()
            .insert("first_zone".into(), toml::Value::Integer(3));
        let custom = Config::parse(&toml::to_string(&table).unwrap()).unwrap();
        assert_eq!(custom.protectors.first_zone, 3);
    }
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
            assert_eq!(engine.elites.chance, get().elites.chance);
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
