//! Orthogonal monster bases and distinct, independently composable modifiers.
use super::{Rng, Run, data};
use bevy::prelude::Component;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

const FILE: &str = include_str!("../../../../config/survivors-monster-mods.toml");
// Retired IDs stay reserved so saved masks never acquire a different mod.
const RETIRED: u64 = 1 << 26; // Thorny / Épineux.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum Base {
    Melee,
    Caster,
    Boss,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "SavedMonster")]
pub(super) struct Monster {
    pub base: Base,
    mods: u64,
}

// Accept the previous single-variant wire representation when reading old snapshots.
#[derive(Deserialize)]
#[serde(untagged)]
enum SavedMonster {
    Current { base: Base, mods: u64 },
    Legacy(String),
}
impl TryFrom<SavedMonster> for Monster {
    type Error = &'static str;
    fn try_from(saved: SavedMonster) -> Result<Self, Self::Error> {
        match saved {
            SavedMonster::Current { base, mods } if mods & !((1_u64 << 59) - 1) == 0 => {
                Ok(Self { base, mods: mods & !RETIRED })
            }
            SavedMonster::Legacy(name) => match name.as_str() {
                "Normal" => Ok(Self::Normal),
                "Boss" => Ok(Self::Boss),
                "Elite" => Ok(Self::Elite),
                "Sprinter" => Ok(Self::Sprinter),
                "Explosive" => Ok(Self::Explosive),
                "Healer" => Ok(Self::Healer),
                "Protector" => Ok(Self::Protector),
                _ => Err("unknown monster variant"),
            },
            _ => Err("invalid monster mod mask"),
        }
    }
}

#[allow(non_upper_case_globals)]
impl Monster {
    pub const Normal: Self = Self {
        base: Base::Melee,
        mods: 0,
    };
    pub const Boss: Self = Self {
        base: Base::Boss,
        mods: 0,
    };
    pub const Colossus: Self = Self {
        base: Base::Melee,
        mods: 1 << 0,
    };
    pub const Sprinter: Self = Self {
        base: Base::Melee,
        mods: 1 << 1,
    };
    pub const Explosive: Self = Self {
        base: Base::Melee,
        mods: 1 << 2,
    };
    pub const Healer: Self = Self {
        base: Base::Melee,
        mods: 1 << 3,
    };
    pub const Protector: Self = Self {
        base: Base::Melee,
        mods: 1 << 4,
    };
    pub const Frenzied: Self = Self {
        base: Base::Melee,
        mods: 1 << 5,
    };
    pub const Brutal: Self = Self {
        base: Base::Melee,
        mods: 1 << 6,
    };
    pub const Pyromancer: Self = Self {
        base: Base::Melee,
        mods: 1 << 7,
    };
    pub const Glacial: Self = Self {
        base: Base::Melee,
        mods: 1 << 8,
    };
    pub const Storm: Self = Self {
        base: Base::Melee,
        mods: 1 << 9,
    };
    pub const Profane: Self = Self {
        base: Base::Melee,
        mods: 1 << 10,
    };
    pub const Venomous: Self = Self {
        base: Base::Melee,
        mods: 1 << 11,
    };
    pub const Precise: Self = Self {
        base: Base::Melee,
        mods: 1 << 12,
    };
    pub const Devastating: Self = Self {
        base: Base::Melee,
        mods: 1 << 13,
    };
    pub const Artillery: Self = Self {
        base: Base::Melee,
        mods: 1 << 14,
    };
    pub const Conductor: Self = Self {
        base: Base::Melee,
        mods: 1 << 15,
    };
    pub const Gigantic: Self = Self {
        base: Base::Melee,
        mods: 1 << 16,
    };
    pub const LongRange: Self = Self {
        base: Base::Melee,
        mods: 1 << 17,
    };
    pub const Armoured: Self = Self {
        base: Base::Melee,
        mods: 1 << 18,
    };
    pub const Fireproof: Self = Self {
        base: Base::Melee,
        mods: 1 << 19,
    };
    pub const Winter: Self = Self {
        base: Base::Melee,
        mods: 1 << 20,
    };
    pub const Insulated: Self = Self {
        base: Base::Melee,
        mods: 1 << 21,
    };
    pub const Occult: Self = Self {
        base: Base::Melee,
        mods: 1 << 22,
    };
    pub const Unstoppable: Self = Self {
        base: Base::Melee,
        mods: 1 << 23,
    };
    pub const Regenerating: Self = Self {
        base: Base::Melee,
        mods: 1 << 24,
    };
    pub const Vampiric: Self = Self {
        base: Base::Melee,
        mods: 1 << 25,
    };
    pub const Thorny: Self = Self {
        base: Base::Melee,
        mods: 1 << 26,
    };
    pub const Banner: Self = Self {
        base: Base::Melee,
        mods: 1 << 27,
    };
    pub const Oppressor: Self = Self {
        base: Base::Melee,
        mods: 1 << 28,
    };
    pub const Rally: Self = Self {
        base: Base::Melee,
        mods: 1 << 29,
    };
    pub const DeathAvenger: Self = Self {
        base: Base::Melee,
        mods: 1 << 30,
    };
    pub const Guardian: Self = Self {
        base: Base::Melee,
        mods: 1 << 31,
    };
    pub const BloodLink: Self = Self {
        base: Base::Melee,
        mods: 1 << 32,
    };
    pub const Electrostatic: Self = Self {
        base: Base::Melee,
        mods: 1 << 33,
    };
    pub const Magnetic: Self = Self {
        base: Base::Melee,
        mods: 1 << 34,
    };
    pub const Charger: Self = Self {
        base: Base::Melee,
        mods: 1 << 35,
    };
    pub const BurningWake: Self = Self {
        base: Base::Melee,
        mods: 1 << 36,
    };
    pub const Elusive: Self = Self {
        base: Base::Melee,
        mods: 1 << 37,
    };
    pub const SecondSkin: Self = Self {
        base: Base::Melee,
        mods: 1 << 38,
    };
    pub const Avenger: Self = Self {
        base: Base::Melee,
        mods: 1 << 39,
    };
    pub const Riposte: Self = Self {
        base: Base::Melee,
        mods: 1 << 40,
    };
    pub const Adaptive: Self = Self {
        base: Base::Melee,
        mods: 1 << 41,
    };
    pub const Aegis: Self = Self {
        base: Base::Melee,
        mods: 1 << 42,
    };
    pub const Bastion: Self = Self {
        base: Base::Melee,
        mods: 1 << 43,
    };
    pub const Predator: Self = Self {
        base: Base::Melee,
        mods: 1 << 44,
    };
    pub const Plagued: Self = Self {
        base: Base::Melee,
        mods: 1 << 45,
    };
    pub const Hemorrhagic: Self = Self {
        base: Base::Melee,
        mods: 1 << 46,
    };
    pub const Incendiary: Self = Self {
        base: Base::Melee,
        mods: 1 << 47,
    };
    pub const GreaterArtillery: Self = Self {
        base: Base::Melee,
        mods: 1 << 48,
    };
    pub const Echo: Self = Self {
        base: Base::Melee,
        mods: 1 << 49,
    };
    pub const Bombardier: Self = Self { base: Base::Melee, mods: 1 << 50 };
    pub const Aftermath: Self = Self { base: Base::Melee, mods: 1 << 51 };
    pub const Flusher: Self = Self { base: Base::Melee, mods: 1 << 52 };
    pub const MageHunter: Self = Self { base: Base::Melee, mods: 1 << 53 };
    pub const ConcentrationBreaker: Self = Self { base: Base::Melee, mods: 1 << 54 };
    pub const Beater: Self = Self { base: Base::Melee, mods: 1 << 55 };
    pub const Besieger: Self = Self { base: Base::Melee, mods: 1 << 56 };
    pub const SiegeVolley: Self = Self { base: Base::Melee, mods: 1 << 57 };
    pub const Desecrator: Self = Self { base: Base::Melee, mods: 1 << 58 };
    pub const Elite: Self = Self::Colossus;
    pub const MODS: [Self; 59] = [
        Self::Colossus,
        Self::Sprinter,
        Self::Explosive,
        Self::Healer,
        Self::Protector,
        Self::Frenzied,
        Self::Brutal,
        Self::Pyromancer,
        Self::Glacial,
        Self::Storm,
        Self::Profane,
        Self::Venomous,
        Self::Precise,
        Self::Devastating,
        Self::Artillery,
        Self::Conductor,
        Self::Gigantic,
        Self::LongRange,
        Self::Armoured,
        Self::Fireproof,
        Self::Winter,
        Self::Insulated,
        Self::Occult,
        Self::Unstoppable,
        Self::Regenerating,
        Self::Vampiric,
        Self::Thorny,
        Self::Banner,
        Self::Oppressor,
        Self::Rally,
        Self::DeathAvenger,
        Self::Guardian,
        Self::BloodLink,
        Self::Electrostatic,
        Self::Magnetic,
        Self::Charger,
        Self::BurningWake,
        Self::Elusive,
        Self::SecondSkin,
        Self::Avenger,
        Self::Riposte,
        Self::Adaptive,
        Self::Aegis,
        Self::Bastion,
        Self::Predator,
        Self::Plagued,
        Self::Hemorrhagic,
        Self::Incendiary,
        Self::GreaterArtillery,
        Self::Echo,
        Self::Bombardier,
        Self::Aftermath,
        Self::Flusher,
        Self::MageHunter,
        Self::ConcentrationBreaker,
        Self::Beater,
        Self::Besieger,
        Self::SiegeVolley,
        Self::Desecrator,
    ];
    pub const KEYS: [&'static str; 59] = [
        "colossus",
        "sprinter",
        "explosive",
        "healer",
        "protector",
        "frenzied",
        "brutal",
        "pyromancer",
        "glacial",
        "storm",
        "profane",
        "venomous",
        "precise",
        "devastating",
        "artillery",
        "conductor",
        "gigantic",
        "long_range",
        "armoured",
        "fireproof",
        "winter",
        "insulated",
        "occult",
        "unstoppable",
        "regenerating",
        "vampiric",
        "thorny",
        "banner",
        "oppressor",
        "rally",
        "death_avenger",
        "guardian",
        "blood_link",
        "electrostatic",
        "magnetic",
        "charger",
        "burning_wake",
        "elusive",
        "second_skin",
        "avenger",
        "riposte",
        "adaptive",
        "aegis",
        "bastion",
        "predator",
        "plagued",
        "hemorrhagic",
        "incendiary",
        "greater_artillery",
        "echo",
        "bombardier",
        "aftermath",
        "flusher",
        "mage_hunter",
        "concentration_breaker",
        "beater",
        "besieger",
        "siege_volley",
        "desecrator",
    ];
    pub fn is_special(self) -> bool {
        self.has(Self::Banner)
            || self.has(Self::Explosive)
            || self.has(Self::Healer)
            || self.has(Self::Protector)
            || self.mods & (((1_u64 << 59) - 1) & !((1_u64 << 29) - 1)) != 0
    }
    pub fn has(self, modifier: Self) -> bool {
        modifier.mods != 0 && !modifier.is_retired() && self.mods & modifier.mods == modifier.mods
    }
    pub fn is_retired(self) -> bool { self.mods != 0 && self.mods & !RETIRED == 0 }
    pub fn is_boss(self) -> bool {
        self.base == Base::Boss
    }
    pub fn count(self) -> u32 {
        (self.mods & !RETIRED).count_ones()
    }
    pub fn with(mut self, modifier: Self) -> Self {
        // Set union: acquiring the same mod again never stacks its effect.
        self.mods = (self.mods | modifier.mods) & !RETIRED;
        self
    }
    pub fn labels(self) -> Vec<&'static str> {
        Self::MODS
            .into_iter()
            .zip([
                "Colossus",
                "Sprinter",
                "Explosive",
                "Healer",
                "Protector",
                "Frenzied",
                "Brutal",
                "Pyromancer",
                "Glacial",
                "Stormy",
                "Profane",
                "Venomous",
                "Precise",
                "Devastating",
                "Artillery",
                "Conductor",
                "Gigantic",
                "Long Range",
                "Armoured",
                "Fireproof",
                "Winterborn",
                "Insulated",
                "Occult",
                "Unwavering",
                "Regenerating",
                "Vampiric",
                "Thorny",
                "Standard Bearer",
                "Oppressive",
                "Rally",
                "Death Avenger",
                "Guardian",
                "Blood Link",
                "Electrostatic",
                "Magnetic",
                "Charger",
                "Burning Wake",
                "Elusive",
                "Second Skin",
                "Avenger",
                "Riposte",
                "Adaptive",
                "Aegis",
                "Bastion",
                "Predator",
                "Plagued",
                "Hemorrhagic",
                "Incendiary",
                "Greater Artillery",
                "Echo",
                "Bombardier",
                "Remanence",
                "Flusher",
                "Mage Hunter",
                "Concentration Breaker",
                "Beater",
                "Besieger",
                "Siege Volley",
                "Desecrator",
            ])
            .filter_map(|(modifier, label)| self.has(modifier).then_some(label))
            .collect()
    }
    pub fn stealable(self) -> bool { !self.is_retired() && self.mods & !((1_u64 << 50) - 1) == 0 }
    pub fn eligible(self, modifier: Self, party: usize, dungeon: bool) -> bool {
        if modifier.is_retired() { return false; }
        if modifier == Self::Bombardier { return self.base == Base::Caster; }
        if modifier == Self::Artillery
            || modifier == Self::GreaterArtillery
            || modifier == Self::Conductor
        {
            return self.base == Base::Caster && party > 1;
        }
        if modifier == Self::Charger {
            return self.base == Base::Melee;
        }
        if modifier == Self::Venomous {
            return self.is_boss() && dungeon;
        }
        if modifier == Self::Devastating {
            return self.has(Self::Precise);
        }
        if modifier == Self::Gigantic {
            return self.is_boss() || self.has(Self::Explosive);
        }
        if modifier == Self::Brutal || modifier == Self::Oppressor || modifier == Self::Riposte || modifier == Self::Hemorrhagic {
            return self.base != Base::Caster;
        }
        true
    }
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Config {
    version: u32,
    #[serde(deserialize_with = "read_weights")]
    weights: [u32; 59],
    campaign: Campaign,
    dungeon: Dungeon,
    endless: Endless,
}
fn read_weights<'de, D: serde::Deserializer<'de>>(d: D) -> Result<[u32; 59], D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Saved {
        Named(std::collections::BTreeMap<String, u32>),
        List(Vec<u32>),
    }
    let mut weights = [0; 59];
    match Saved::deserialize(d)? {
        Saved::Named(map) => {
            for (name, weight) in map {
                let i = Monster::KEYS
                    .iter()
                    .position(|&key| key == name)
                    .ok_or_else(|| {
                        serde::de::Error::custom(format!("unknown monster mod: {name}"))
                    })?;
                weights[i] = weight;
            }
        }
        Saved::List(list) if list.len() == 5 || list.len() == 29 || list.len() == 50 || list.len() == 59 => {
            weights[..list.len()].copy_from_slice(&list)
        }
        _ => return Err(serde::de::Error::custom("expected named mod weights")),
    }
    Ok(weights)
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Campaign {
    chance: [f32; 6],
    max_mods: [u8; 6],
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Dungeon {
    monster_chance: f32,
    monster_max_mods: u8,
    boss_chance: f32,
    boss_max_mods: u8,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Endless {
    monsters: Progression,
    bosses: Progression,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Progression {
    chance: f32,
    chance_per_stage: f32,
    max_chance: f32,
    initial_max_mods: u8,
    stages_per_extra_mod: u32,
    max_mods: u8,
    initial_min_mods: u8,
    stages_per_extra_min_mod: u32,
}
#[derive(Clone, Copy, Debug)]
struct Distribution {
    chance: f32,
    min: u8,
    max: u8,
    extra: u8,
}
impl Progression {
    fn distribution(&self, index: u32) -> Distribution {
        let max = (u32::from(self.initial_max_mods)
            .saturating_add(index / self.stages_per_extra_mod))
        .min(u32::from(self.max_mods)) as u8;
        let min = (u32::from(self.initial_min_mods)
            .saturating_add(index / self.stages_per_extra_min_mod))
        .min(u32::from(max)) as u8;
        Distribution {
            chance: (self.chance + self.chance_per_stage * index as f32).min(self.max_chance),
            min,
            max,
            extra: 0,
        }
    }
}
impl Config {
    fn parse(text: &str) -> Result<Self, String> {
        let c: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        let probability = |v: f32| v.is_finite() && (0.0..=1.0).contains(&v);
        if c.version != 1
            || c.weights.iter().all(|&w| w == 0)
            || c.weights.iter().any(|&w| w > 10000)
            || !c.campaign.chance.into_iter().all(probability)
            || c.campaign.max_mods.iter().any(|&n| !(1..=5).contains(&n))
            || !probability(c.dungeon.monster_chance)
            || !probability(c.dungeon.boss_chance)
            || !(1..=5).contains(&c.dungeon.monster_max_mods)
            || !(1..=5).contains(&c.dungeon.boss_max_mods)
            || [&c.endless.monsters, &c.endless.bosses]
                .into_iter()
                .any(|p| {
                    !probability(p.chance)
                        || !probability(p.chance_per_stage)
                        || !probability(p.max_chance)
                        || p.chance > p.max_chance
                        || !(1..=5).contains(&p.initial_max_mods)
                        || !(1..=p.initial_max_mods).contains(&p.initial_min_mods)
                        || !(p.initial_max_mods..=5).contains(&p.max_mods)
                        || p.stages_per_extra_mod == 0
                        || p.stages_per_extra_min_mod == 0
                })
        {
            return Err("invalid monster mod distribution".into());
        }
        Ok(c)
    }
    fn distribution(&self, run: &Run, boss: bool) -> Distribution {
        if let Some(stage) = run.endless {
            return (if boss {
                &self.endless.bosses
            } else {
                &self.endless.monsters
            })
            .distribution(stage.index);
        }
        if data::dungeon(run.expedition_map) {
            return Distribution {
                chance: if boss {
                    self.dungeon.boss_chance
                } else {
                    self.dungeon.monster_chance
                },
                min: 1,
                max: if boss {
                    self.dungeon.boss_max_mods
                } else {
                    self.dungeon.monster_max_mods
                },
                extra: run.dungeon_modifiers().extra_monster_mods().saturating_add(
                    if boss { run.dungeon_modifiers().extra_boss_mods() } else { 0 }
                ).min(5),
            };
        }
        let zone = data::CAMPAIGN
            .iter()
            .position(|&map| map == run.expedition_map);
        Distribution {
            chance: if boss {
                0.0
            } else {
                zone.map_or(0.0, |i| self.campaign.chance[i])
            },
            min: 1,
            max: zone.map_or(1, |i| self.campaign.max_mods[i]),
            extra: 0,
        }
    }
    #[cfg(test)]
    fn roll(&self, rng: &mut Rng, distribution: Distribution, boss: bool) -> Monster {
        self.roll_for_base(
            rng,
            distribution,
            if boss { Base::Boss } else { Base::Melee },
            4,
            true,
        )
    }
    fn roll_for_base(
        &self,
        rng: &mut Rng,
        distribution: Distribution,
        base: Base,
        party: usize,
        dungeon: bool,
    ) -> Monster {
        let mut monster = Monster { base, mods: 0 };
        let count = if rng.f32() < distribution.chance {
            distribution.min + rng.index(usize::from(distribution.max - distribution.min + 1)) as u8
        } else {
            0
        };
        let mut weights = self.weights;
        for _ in 0..count.saturating_add(distribution.extra).min(5) {
            let total: u32 = weights
                .iter()
                .enumerate()
                .filter(|(i, _)| monster.eligible(Monster::MODS[*i], party, dungeon))
                .map(|(_, w)| *w)
                .sum();
            if total == 0 {
                break;
            }
            let mut pick = rng.index(total as usize) as u32;
            for (index, weight) in weights.iter_mut().enumerate() {
                if !monster.eligible(Monster::MODS[index], party, dungeon) {
                    continue;
                }
                if pick < *weight {
                    monster = monster.with(Monster::MODS[index]);
                    *weight = 0;
                    break;
                }
                pick -= *weight;
            }
        }
        monster
    }
}
pub(super) fn get() -> &'static Config {
    static CONFIG: LazyLock<Config> = LazyLock::new(|| {
        let builtin = || Config::parse(FILE).expect("valid embedded monster mods");
        if cfg!(test) {
            return builtin();
        }
        match crate::local_state::read_to_string(std::path::Path::new(
            "config/survivors-monster-mods.toml",
        )) {
            Ok(text) => Config::parse(&text).unwrap_or_else(|error| {
                bevy::log::warn!("Invalid monster mods: {error}; using embedded defaults");
                builtin()
            }),
            Err(_) => builtin(),
        }
    });
    &CONFIG
}
pub(super) fn roll(run: &Run, rng: &mut Rng, boss: bool) -> Monster {
    roll_base(run, rng, if boss { Base::Boss } else { Base::Melee })
}
pub(super) fn roll_base(run: &Run, rng: &mut Rng, base: Base) -> Monster {
    let config = get();
    config.roll_for_base(
        rng,
        config.distribution(run, base == Base::Boss),
        base,
        run.expedition_players,
        data::dungeon(run.expedition_map),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn retired_reflection_cannot_return_from_saves_or_custom_weights() {
        use super::*;
        let saved = serde_json::to_string(&Monster::Thorny).unwrap();
        let restored: Monster = serde_json::from_str(&saved).unwrap();
        assert_eq!(restored.count(), 0);
        assert!(!restored.has(Monster::Thorny));
        assert_eq!(Monster::Normal.with(Monster::Thorny).count(), 0);
        let mut config = Config::parse(FILE).unwrap();
        config.weights = [0; 59];
        config.weights[26] = 100;
        let distribution = Distribution { chance: 1.0, min: 1, max: 5, extra: 0 };
        assert_eq!(config.roll(&mut Rng(42), distribution, false).count(), 0);
    }
    #[test]
    fn every_mod_is_unique_even_when_reacquired_or_combined_repeatedly() {
        use super::*;
        let mut holder = Monster::Normal;
        for modifier in Monster::MODS {
            if modifier.is_retired() { continue; }
            let single = Monster::Normal.with(modifier);
            assert_eq!(single.count(), 1);
            assert_eq!(single.with(modifier).with(modifier), single);
            holder = holder.with(modifier).with(modifier);
        }
        assert_eq!(holder.count(), Monster::MODS.len() as u32 - 1);
        for modifier in Monster::MODS {
            assert_eq!(holder.with(modifier), holder);
        }
        let restored: Monster = serde_json::from_str(&serde_json::to_string(&holder).unwrap()).unwrap();
        assert_eq!(restored, holder);
    }
    use super::*;
    #[test]
    fn each_special_has_half_the_weight_of_a_common_stat_and_both_artilleries_stack() {
        let mut config = Config::parse(FILE).unwrap();
        assert_eq!(config.weights[Monster::KEYS.iter().position(|&k| k == "echo").unwrap()], 1);
        assert_eq!(config.weights[Monster::KEYS.iter().position(|&k| k == "regenerating").unwrap()], 2);
        assert_eq!(config.weights[27], 1, "Banner is a special, not a second duplicate mod");
        for (i, modifier) in Monster::MODS.into_iter().enumerate() {
            assert_eq!(config.weights[i], if modifier.is_retired() { 0 } else if modifier.is_special() { 1 } else { 2 });
        }
        config.weights = [0; 59]; config.weights[24] = 2; config.weights[49] = 1;
        let distribution = Distribution { chance: 1.0, min: 1, max: 1, extra: 0 };
        let mut rng = Rng(42); let mut rare = 0;
        for _ in 0..100_000 { rare += usize::from(config.roll(&mut rng, distribution, false).has(Monster::Echo)); }
        assert!((rare as f64 / 100_000.0 - 1.0/3.0).abs() < 0.01);
        assert_eq!(super::super::monster_stats::Bonuses(Monster::Artillery.with(Monster::GreaterArtillery)).projectiles(), 6);
    }
    #[test]
    fn specialized_mods_only_roll_for_compatible_bases_and_capabilities() {
        let mut config = Config::parse(FILE).unwrap();
        config.weights = [1; 59];
        let distribution = Distribution {
            chance: 1.0,
            min: 5,
            max: 5,
            extra: 0,
        };
        let mut rng = Rng(42);
        let mut seen = Monster::Normal;
        for base in [Base::Melee, Base::Caster, Base::Boss] {
            for party in [1, 4] {
                for _ in 0..10_000 {
                    let rank = config.roll_for_base(
                        &mut rng,
                        distribution,
                        base,
                        party,
                        base == Base::Boss,
                    );
                    seen = seen.with(rank);
                    if base != Base::Caster || party == 1 {
                        assert!(!rank.has(Monster::Artillery) && !rank.has(Monster::Conductor));
                    }
                    if base != Base::Boss {
                        assert!(!rank.has(Monster::Venomous));
                    }
                    if rank.has(Monster::Devastating) {
                        assert!(rank.has(Monster::Precise));
                    }
                    if rank.has(Monster::Gigantic) {
                        assert!(rank.is_boss() || rank.has(Monster::Explosive));
                    }
                    assert_eq!(rank.count(), 5);
                }
            }
        }
        assert_eq!(
            seen.count(),
            Monster::MODS.len() as u32 - 1,
            "every configured mod must be reachable"
        );
    }
    #[test]
    fn campaign_uses_configured_chances_and_limits_without_modded_bosses() {
        let config = Config::parse(FILE).unwrap();
        for (zone, &map) in data::CAMPAIGN.iter().enumerate() {
            let run = Run {
                expedition_map: map,
                ..Default::default()
            };
            let mut rng = Rng(42);
            let mut modified = 0;
            for _ in 0..100_000 {
                let monster = config.roll(&mut rng, config.distribution(&run, false), false);
                assert!(monster.count() <= u32::from(config.campaign.max_mods[zone]));
                modified += u32::from(monster.count() > 0);
                let boss = config.roll(&mut rng, config.distribution(&run, true), true);
                assert!(boss.is_boss());
                assert_eq!(boss.count(), 0);
            }
            let expected = config.campaign.chance[zone];
            assert!((modified as f32 / 100_000.0 - expected).abs() < 0.004);
        }
    }
    #[test]
    fn endless_progression_is_map_independent_and_bosses_gain_mods_more_slowly() {
        let config = Config::parse(FILE).unwrap();
        let mut run = Run::default();
        for index in [0, 3, 6, 12, 24, u32::MAX] {
            run.endless = Some(super::super::endless::Stage {
                index,
                ..Default::default()
            });
            let mobs = config.distribution(&run, false);
            let bosses = config.distribution(&run, true);
            assert!(mobs.max >= bosses.max && mobs.min >= bosses.min);
            assert!(mobs.chance >= bosses.chance);
            for &map in data::CAMPAIGN {
                run.expedition_map = map;
                assert_eq!(config.distribution(&run, false).max, mobs.max);
                assert_eq!(config.distribution(&run, true).max, bosses.max);
            }
            let mut rng = Rng(42);
            for boss in [false, true] {
                let distribution = config.distribution(&run, boss);
                for _ in 0..1000 {
                    let monster = config.roll(&mut rng, distribution, boss);
                    assert_eq!(monster.is_boss(), boss);
                    assert!(monster.count() <= u32::from(distribution.max));
                    assert!(monster.count() == 0 || monster.count() >= u32::from(distribution.min));
                }
            }
        }
        assert_eq!(config.endless.monsters.distribution(3).max, 2);
        assert_eq!(config.endless.bosses.distribution(3).max, 1);
        assert_eq!(config.endless.bosses.distribution(6).max, 2);
    }
    #[test]
    fn extra_boss_modifiers_only_affect_bosses_and_stack_with_monster_modifiers() {
        let config = Config::parse(FILE).unwrap();
        for &map in data::DUNGEONS {
            let mut run = Run { expedition_map: map, ..Default::default() };
            for rank in 1..=3 {
                run.dungeon_mods.change(4, true);
                assert_eq!(config.distribution(&run, false).extra, 0);
                assert_eq!(config.distribution(&run, true).extra, rank);
                for seed in 1..=20 {
                    let boss = config.roll(&mut Rng(seed), config.distribution(&run, true), true);
                    assert!(boss.count() >= u32::from(rank));
                    assert!(boss.count() <= 5);
                }
            }
            run.dungeon_mods.change(6, true);
            assert_eq!(config.distribution(&run, false).extra, 1);
            assert_eq!(config.distribution(&run, true).extra, 4);
            run.dungeon_mods.change(6, true);
            run.dungeon_mods.change(6, true);
            assert_eq!(config.distribution(&run, true).extra, 5);
        }
    }
    #[test]
    fn dungeon_option_guarantees_distinct_mods_for_mobs_and_bosses_and_old_ranks_load() {
        let config = Config::parse(FILE).unwrap();
        let old: super::super::dungeon_mods::Mods = serde_json::from_str("[1,0,0,0,0,0]").unwrap();
        assert_eq!(old.extra_monster_mods(), 0);
        for &map in data::DUNGEONS {
            let mut run = Run {
                expedition_map: map,
                ..Default::default()
            };
            let mut rng = Rng(42);
            for rank in 1..=3 {
                run.dungeon_mods.change(6, true);
                assert_eq!(run.dungeon_modifiers().extra_monster_mods(), rank);
                for boss in [false, true] {
                    for _ in 0..1000 {
                        let monster = config.roll(&mut rng, config.distribution(&run, boss), boss);
                        assert!(monster.count() >= u32::from(rank));
                        assert!(monster.count() <= 5);
                        assert_eq!(monster.is_boss(), boss);
                    }
                }
            }
        }
    }
    #[test]
    fn disabled_mods_never_roll_and_combined_bases_roundtrip() {
        let mut config = Config::parse(FILE).unwrap();
        config.weights = [0; 59];
        config.weights[1] = 1;
        config.weights[3] = 1;
        let distribution = Distribution {
            chance: 1.0,
            min: 5,
            max: 5,
            extra: 3,
        };
        let mut monster = config.roll(&mut Rng(42), distribution, true);
        assert_eq!(monster.count(), 2);
        assert!(monster.has(Monster::Sprinter) && monster.has(Monster::Healer));
        for base in [Base::Melee, Base::Caster, Base::Boss] {
            monster.base = base;
            let text = serde_json::to_string(&monster).unwrap();
            assert_eq!(serde_json::from_str::<Monster>(&text).unwrap(), monster);
        }
        assert_eq!(
            serde_json::from_str::<Monster>("\"Healer\"").unwrap(),
            Monster::Healer
        );
        assert!(
            serde_json::from_str::<Monster>(r#"{"base":"Boss","mods":18446744073709551615}"#)
                .is_err()
        );
    }
    #[test]
    fn invalid_distributions_are_rejected() {
        for (from, to) in [
            ("colossus = 2", "unknown_mod = 2"),
            ("monster_chance = 0.30", "monster_chance = 1.1"),
            ("boss_max_mods = 2", "boss_max_mods = 6"),
            ("stages_per_extra_mod = 3", "stages_per_extra_mod = 0"),
            ("initial_min_mods = 1", "initial_min_mods = 6"),
        ] {
            assert!(Config::parse(&FILE.replace(from, to)).is_err(), "{to}");
        }
    }
}
