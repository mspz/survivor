//! Two independent weighted draws; defaults also ship embedded for standalone/web builds.
use super::{data, hud::Choice, Run};
use bevy::prelude::*;
use serde::Deserialize;
use std::{collections::BTreeMap, path::Path};

pub(super) const DEFAULT_FILE: &str = include_str!("../../../../config/survivors-rewards.toml");

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Config {
    version: u32,
    rarities: Rarities,
    #[serde(default)]
    spells: BTreeMap<String, f64>,
    #[serde(default)]
    passives: BTreeMap<String, f64>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rarities {
    common: f64,
    uncommon: f64,
    epic: f64,
    legendary: f64,
}
impl Config {
    pub fn parse(text: &str) -> Result<Self, String> {
        let config: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        if config.version != 1 {
            return Err("unsupported version (expected 1)".into());
        }
        let rarities = config.rarity_weights();
        if rarities
            .iter()
            .chain(config.spells.values())
            .chain(config.passives.values())
            .any(|w| !w.is_finite() || *w < 0.0)
        {
            return Err("weights must be finite and non-negative".into());
        }
        if !rarities.iter().any(|w| *w > 0.0) {
            return Err("at least one rarity must have positive weight".into());
        }
        for key in config.spells.keys() {
            if !data::ABILITIES.iter().any(|def| def.key == key) {
                return Err(format!("unknown spell: {key}"));
            }
        }
        for key in config.passives.keys() {
            if !data::PASSIVES.iter().any(|def| def.key == key) {
                return Err(format!("unknown passive: {key}"));
            }
        }
        Ok(config)
    }
    pub fn builtin() -> Self {
        Self::parse(DEFAULT_FILE).expect("valid built-in reward configuration")
    }
    pub fn rarity_weights(&self) -> [f64; 4] {
        [
            self.rarities.common,
            self.rarities.uncommon,
            self.rarities.epic,
            self.rarities.legendary,
        ]
    }
    pub fn card_weight(&self, choice: Choice, run: &Run) -> f64 {
        let (table, key) = match choice {
            Choice::NewAbility(def) => (&self.spells, def.key),
            Choice::Upgrade(i) => (&self.spells, run.abilities[i].def.key),
            Choice::NumericPassive(def, _) | Choice::NewPassive(def) => (&self.passives, def.key),
            Choice::UpPassive(i) => (&self.passives, run.passives[i].0.key),
            Choice::Heal => return 1.0,
        };
        table.get(key).copied().unwrap_or(1.0)
    }
    fn read(path: &Path) -> Self {
        match crate::local_state::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|text| Self::parse(&text))
        {
            Ok(config) => {
                info!("Survivors rewards loaded from {}", path.display());
                config
            }
            Err(error) => {
                warn!(
                    "Invalid/unreadable reward config {}: {error}; using built-in defaults",
                    path.display()
                );
                Self::builtin()
            }
        }
    }
}
impl Default for Config {
    fn default() -> Self {
        if cfg!(test) {
            return Self::builtin();
        }
        if let Some(path) = crate::webenv::var("WOW_REWARDS_CONFIG") {
            return Self::read(Path::new(&path));
        }
        #[cfg(not(target_arch = "wasm32"))]
        if Path::new("config/survivors-rewards.toml").exists() {
            return Self::read(Path::new("config/survivors-rewards.toml"));
        }
        if let Some(home) = crate::local_state::home() {
            let path = home.join("survivors-rewards.toml");
            if !matches!(crate::local_state::read_to_string(&path), Err(error) if error.kind() == std::io::ErrorKind::NotFound)
            {
                return Self::read(&path);
            }
            if let Err(error) = crate::local_state::write_atomic(&path, DEFAULT_FILE) {
                warn!("Could not create {}: {error}", path.display());
            } else {
                info!("Created reward configuration {}", path.display());
            }
        }
        Self::builtin()
    }
}

/// Normalize by the largest weight to avoid overflow even for very large valid weights.
pub(super) fn weighted_index(weights: &[f64], roll: f32) -> Option<usize> {
    let max = weights.iter().copied().fold(0.0, f64::max);
    if max <= 0.0 {
        return None;
    }
    let total: f64 = weights.iter().map(|w| w / max).sum();
    let mut cursor = f64::from(roll.clamp(0.0, 1.0)) * total;
    for (i, w) in weights.iter().enumerate() {
        let w = w / max;
        if w > 0.0 && cursor < w {
            return Some(i);
        }
        cursor -= w;
    }
    weights.iter().rposition(|w| *w > 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_cover_the_entire_reward_catalog_with_equal_weights() {
        let config = Config::builtin();
        assert_eq!(config.spells.len(), data::ABILITIES.len());
        assert_eq!(config.passives.len(), data::PASSIVES.len());
        for def in data::ABILITIES {
            assert_eq!(config.spells.get(def.key), Some(&1.0));
        }
        for def in data::PASSIVES {
            assert_eq!(config.passives.get(def.key), Some(&1.0));
        }
        assert_eq!(config.rarity_weights(), [50.0, 30.0, 15.0, 5.0]);
    }
    #[test]
    fn relative_weights_are_normalized_and_zero_entries_are_never_drawn() {
        let mut counts = [0; 3];
        for n in 0..10_000 {
            counts[weighted_index(&[1.0, 3.0, 0.0], (n as f32 + 0.5) / 10_000.0).unwrap()] += 1;
        }
        assert_eq!(counts, [2500, 7500, 0]);
        assert_eq!(weighted_index(&[0.0; 4], 0.5), None);
        assert_eq!(weighted_index(&[f64::MAX, f64::MAX], 0.75), Some(1));
    }
    #[test]
    fn invalid_files_are_rejected_instead_of_corrupting_draws() {
        for text in [
            DEFAULT_FILE.replace("common = 50", "common = -1"),
            DEFAULT_FILE.replace("common = 50", "common = nan"),
            DEFAULT_FILE.replace("version = 1", "version = 2"),
            DEFAULT_FILE.replace("fireball = 1", "misspelled_spell = 1"),
            DEFAULT_FILE
                .replace("common = 50", "common = 0")
                .replace("uncommon = 30", "uncommon = 0")
                .replace("epic = 15", "epic = 0")
                .replace("legendary = 5", "legendary = 0"),
        ] {
            assert!(Config::parse(&text).is_err());
        }
        let sparse = Config::parse(
            "version = 1\n[rarities]\ncommon = 1\nuncommon = 0\nepic = 0\nlegendary = 0\n",
        )
        .unwrap();
        let run = Run::default();
        assert_eq!(
            sparse.card_weight(Choice::NewAbility(&data::ABILITIES[0]), &run),
            1.0
        );
    }
}
