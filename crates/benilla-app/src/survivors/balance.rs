//! Final effect-power tuning, independent of item rolls and reward weights.
use super::data;
#[cfg(test)]
use super::spell_engine::tags;
use super::spell_engine::tags::Tags;
use bevy::prelude::*;
use serde::Deserialize;
use std::collections::BTreeMap;

const FILE: &str = include_str!("../../../../config/survivors-balance.toml");
const TAGS: &[(&str, Tags)] = &[
    ("buff", Tags::BUFF),
    ("attack", Tags::ATTACK),
    ("spell", Tags::SPELL),
    ("melee", Tags::MELEE),
    ("projectile", Tags::PROJECTILE),
    ("area", Tags::AREA),
    ("chain", Tags::CHAIN),
    ("dot", Tags::DOT),
    ("duration", Tags::DURATION),
    ("totem", Tags::TOTEM),
    ("trap", Tags::TRAP),
    ("aura", Tags::AURA),
    ("guard", Tags::GUARD),
    ("heal", Tags::HEAL),
    ("damage", Tags::DAMAGE),
    ("utility", Tags::UTILITY),
    ("movement", Tags::MOVEMENT),
    ("physical", Tags::PHYSICAL),
    ("fire", Tags::FIRE),
    ("cold", Tags::COLD),
    ("lightning", Tags::LIGHTNING),
    ("nature", Tags::NATURE),
    ("arcane", Tags::ARCANE),
    ("holy", Tags::HOLY),
    ("shadow", Tags::SHADOW),
];

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Config {
    version: u32,
    classes: BTreeMap<String, Class>,
    #[serde(default)]
    tags: BTreeMap<String, f32>,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Class {
    multiplier: f32,
    #[serde(default)]
    spells: BTreeMap<String, f32>,
}
impl Config {
    pub fn parse(text: &str) -> Result<Self, String> {
        let config: Self = toml::from_str(text).map_err(|error| error.to_string())?;
        if config.version != 1 {
            return Err("unsupported balance version".into());
        }
        for (name, value) in &config.tags {
            if !TAGS.iter().any(|(key, _)| *key == name) {
                return Err(format!("unknown spell tag: {name}"));
            }
            if !value.is_finite() || !(0.0..=1000.0).contains(value) {
                return Err(format!(
                    "invalid multiplier for tag {name}: expected 0..1000"
                ));
            }
        }
        for (name, class) in &config.classes {
            let def = data::CLASSES
                .iter()
                .find(|c| c.name.to_lowercase() == *name)
                .ok_or_else(|| format!("unknown class: {name}"))?;
            if std::iter::once(&class.multiplier)
                .chain(class.spells.values())
                .any(|value| !value.is_finite() || !(0.0..=1000.0).contains(value))
            {
                return Err(format!("invalid multiplier for {name}: expected 0..1000"));
            }
            for spell in class.spells.keys() {
                if spell != def.start && !def.pool.contains(&spell.as_str()) {
                    return Err(format!("unknown spell for {name}: {spell}"));
                }
            }
        }
        Ok(config)
    }
    #[cfg(test)]
    pub fn multiplier(&self, class: &data::ClassDef, spell: &str) -> f32 {
        let spell_tags = data::ABILITIES
            .iter()
            .find(|def| def.key == spell)
            .or_else(|| super::procs::find(spell).map(|s| &s.def))
            .map(tags::for_spell)
            .unwrap_or_default();
        self.multiplier_tagged(class, spell, spell_tags)
    }
    pub fn multiplier_tagged(&self, class: &data::ClassDef, spell: &str, spell_tags: Tags) -> f32 {
        let class_factor = self
            .classes
            .get(&class.name.to_lowercase())
            .map_or(1.0, |entry| {
                entry.multiplier * entry.spells.get(spell).copied().unwrap_or(1.0)
            });
        // Every matching tag is an independent factor, even on multi-school spells.
        TAGS.iter()
            .filter(|(_, tag)| spell_tags.contains(*tag))
            .fold(class_factor, |factor, (name, _)| {
                factor * self.tags.get(*name).copied().unwrap_or(1.0)
            })
    }
}
impl Default for Config {
    fn default() -> Self {
        let builtin = || Self::parse(FILE).expect("valid embedded balance configuration");
        if cfg!(test) {
            return builtin();
        }
        let explicit = crate::webenv::var("WOW_BALANCE_CONFIG");
        let path = explicit
            .as_deref()
            .unwrap_or("config/survivors-balance.toml");
        match crate::local_state::read_to_string(std::path::Path::new(path)) {
            Ok(text) => match Self::parse(&text) {
                Ok(config) => {
                    info!("Survivors balance loaded from {path}");
                    config
                }
                Err(error) => {
                    warn!("Invalid balance config {path}: {error}; using defaults");
                    builtin()
                }
            },
            Err(error) => {
                if explicit.is_some() || error.kind() != std::io::ErrorKind::NotFound {
                    warn!("Cannot read balance config {path}: {error}; using defaults");
                }
                builtin()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::survivors::Run;
    #[test]
    fn configured_defaults_cover_every_spell_and_omitted_multipliers_are_neutral() {
        let config = Config::parse(FILE).unwrap();
        let neutral = Config::parse("version=1\nclasses={}").unwrap();
        assert_eq!(config.classes.len(), data::CLASSES.len());
        assert_eq!(config.tags.len(), TAGS.len());
        for (name, _) in TAGS {
            assert!(config.tags[*name].is_finite() && config.tags[*name] >= 0.0);
        }
        for class in data::CLASSES {
            let entry = &config.classes[&class.name.to_lowercase()];
            for spell in std::iter::once(&class.start).chain(class.pool.iter()) {
                assert!(entry.spells.contains_key(*spell));
                assert!(config.multiplier(class, spell).is_finite());
                assert_eq!(neutral.multiplier(class, spell), 1.0);
            }
        }
    }
    #[test]
    fn class_and_spell_multiply_final_effects_and_leave_cooldown_unchanged() {
        let mut run = Run::default();
        let def = data::ability(run.class.start);
        let before = run.resolve_spell(def, 1);
        run.balance = Config::parse("version = 1\n[classes.warrior]\nmultiplier = 2.0\n[classes.warrior.spells]\ncleave = 1.5").unwrap();
        let after = run.resolve_spell(def, 1);
        assert_eq!(after.dmg, before.dmg * 3.0);
        assert_eq!(after.dot, before.dot * 3.0);
        assert_eq!(after.cooldown, before.cooldown);
        assert!(Config::parse("version=1\n[classes.warrior]\nmultiplier=-1").is_err());
        assert!(Config::parse(
            "version=1\n[classes.mage]\nmultiplier=1\n[classes.mage.spells]\ncleave=1"
        )
        .is_err());
    }

    #[test]
    fn matching_tags_multiply_with_class_and_spell_and_omit_unmatched_tags() {
        let mut run = Run::default();
        run.balance = Config::parse("version=1\nclasses={}").unwrap();
        run.class = data::CLASSES
            .iter()
            .find(|class| class.name == "Mage")
            .unwrap();
        let fireball = data::ability("fireball");
        let before = run.resolve_spell(fireball, 1);
        run.balance = Config::parse("version=1\n[tags]\nspell=2\nfire=1.5\nprojectile=0.5\narea=2\nattack=0\n[classes.mage]\nmultiplier=2\n[classes.mage.spells]\nfireball=1.5").unwrap();
        let after = run.resolve_spell(fireball, 1);
        assert!((after.dmg - before.dmg * 9.0).abs() < 0.001);
        assert_eq!(after.cooldown, before.cooldown);
        assert_eq!(after.range, before.range);
        assert_eq!(after.radius, before.radius);
        for value in ["-1", "nan", "inf", "1001"] {
            assert!(
                Config::parse(&format!("version=1\nclasses={{}}\n[tags]\nfire={value}")).is_err()
            );
        }
        assert!(Config::parse("version=1\nclasses={}\n[tags]\nfirre=1").is_err());
    }
}
