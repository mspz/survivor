//! Runes: account-wide progression shared by every character.
//!
//! Winning a campaign zone drops a rune of that zone's rarity (sometimes one above). At the
//! results the player places it in an empty slot, swaps it for a slotted rune, or spends it to
//! try an upgrade on a slotted rune. One slotted rune is equipped at a time and its stats feed
//! the spell engine like equipment does. The stat table lives in `config/survivors-runes.toml`.
use super::{
    data,
    spell_engine::{Change, Filter, Modifier, Tags},
    Phase, Rng, Run, Selection,
};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

pub(super) mod ui;

const FILE: &str = include_str!("../../../../config/survivors-runes.toml");
/// Spell-modifier source id owned by the equipped rune.
const SOURCE: u64 = 0x5000_0000;
/// Highest upgrade level.
pub(super) const MAX_LEVEL: u8 = 10;
const SAVE_FILE: &str = "runes.toml";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum RuneEffect {
    Damage,
    FinalDamage,
    CritChance,
    CritDamage,
    Cooldown,
    Acceleration,
    Area,
    Duration,
    BossDamage,
    LifeLeech,
    MovementSpeed,
    Experience,
    /// Reserved for the SP system; rolls but has no effect yet.
    SpGain,
}

fn yes() -> bool {
    true
}
fn default_icon() -> String {
    "Interface\\Icons\\INV_Misc_Rune_01".into()
}
fn no_bonus() -> Vec<f32> {
    vec![1.0]
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StatDef {
    pub id: String,
    pub label: String,
    pub effect: RuneEffect,
    pub min: f32,
    pub max: f32,
    #[serde(default)]
    pub tier_min: f32,
    #[serde(default)]
    pub tier_max: f32,
    pub step: f32,
    #[serde(default = "yes")]
    pub enabled: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RarityDef {
    pub name: String,
    pub color: String,
    #[serde(default = "default_icon")]
    pub icon: String,
    pub stats: usize,
    pub milestone_stats: usize,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Config {
    pub slots: usize,
    pub upper_rarity_chance: f32,
    pub upgrade_chances: Vec<f32>,
    /// Chance multiplier by the rarity of the rune spent on an upgrade.
    #[serde(default = "no_bonus")]
    pub upgrade_rarity_bonus: Vec<f32>,
    pub milestones: Vec<u8>,
    pub rarities: Vec<RarityDef>,
    pub stats: Vec<StatDef>,
}

impl Config {
    pub(super) fn parse(text: &str) -> Result<Self, String> {
        let c: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        let fraction = |v: f32| v.is_finite() && (0.0..=1.0).contains(&v);
        if !(1..=10).contains(&c.slots) {
            return Err("slots must be 1 to 10".into());
        }
        if !fraction(c.upper_rarity_chance) {
            return Err("upper_rarity_chance must be 0 to 1".into());
        }
        if c.upgrade_chances.len() != MAX_LEVEL as usize
            || !c.upgrade_chances.iter().all(|&p| fraction(p))
        {
            return Err(format!(
                "upgrade_chances needs {MAX_LEVEL} values between 0 and 1"
            ));
        }
        if c.upgrade_rarity_bonus.is_empty()
            || !c
                .upgrade_rarity_bonus
                .iter()
                .all(|&m| m.is_finite() && (0.0..=100.0).contains(&m))
        {
            return Err("upgrade_rarity_bonus needs at least one multiplier >= 0".into());
        }
        if c.milestones.iter().any(|&m| m == 0 || m > MAX_LEVEL) {
            return Err(format!("milestones must be 1 to {MAX_LEVEL}"));
        }
        if c.rarities.is_empty() || c.rarities.len() > u8::MAX as usize {
            return Err("at least one rarity is needed".into());
        }
        for r in &c.rarities {
            if r.stats == 0 || parse_color(&r.color).is_none() {
                return Err(format!(
                    "rarity {}: needs stats >= 1 and a 6-digit hex color",
                    r.name
                ));
            }
        }
        let mut ids = std::collections::BTreeSet::new();
        for s in &c.stats {
            let ok = [s.min, s.max, s.tier_min, s.tier_max, s.step]
                .iter()
                .all(|v| v.is_finite())
                && s.min >= 0.0
                && s.min <= s.max
                && s.tier_min >= 0.0
                && s.tier_max >= 0.0
                && s.step >= 0.0
                && [s.min, s.max, s.step].iter().all(|v| v.fract() == 0.0);
            if !ok {
                return Err(format!(
                    "stat {}: min, max and step must be whole percents, min <= max",
                    s.id
                ));
            }
            if !ids.insert(s.id.as_str()) {
                return Err(format!("stat {} is listed twice", s.id));
            }
        }
        if !c.stats.iter().any(|s| s.enabled) {
            return Err("at least one stat must be enabled".into());
        }
        Ok(c)
    }

    pub(super) fn rarity(&self, rarity: u8) -> &RarityDef {
        &self.rarities[(rarity as usize).min(self.rarities.len() - 1)]
    }
    pub(super) fn stat(&self, id: &str) -> Option<&StatDef> {
        self.stats.iter().find(|s| s.id == id)
    }
    fn top_rarity(&self) -> u8 {
        (self.rarities.len() - 1) as u8
    }

    /// The rarity a victory in campaign zone `tier` drops.
    pub(super) fn drop_rarity(&self, tier: u8, rng: &mut Rng) -> u8 {
        let base = tier.min(self.top_rarity());
        if rng.f32() < self.upper_rarity_chance {
            (base + 1).min(self.top_rarity())
        } else {
            base
        }
    }

    /// The whole-percent range a stat rolls in at `rarity`.
    pub(super) fn range(&self, stat: &StatDef, rarity: u8) -> (u32, u32) {
        let t = rarity.min(self.top_rarity()) as f32;
        let lo = (stat.min + t * stat.tier_min).floor() as u32;
        let hi = ((stat.max + t * stat.tier_max).floor() as u32).max(lo);
        (lo, hi)
    }

    /// A whole-percent value, uniform over the rarity's range.
    fn roll_value(&self, stat: &StatDef, rarity: u8, rng: &mut Rng) -> f32 {
        let (lo, hi) = self.range(stat, rarity);
        (lo + rng.index((hi - lo + 1) as usize) as u32) as f32
    }

    /// Add the next `count` stats of the table the rune does not have yet, in table order, so
    /// every rune of a rarity has the same stats; when none are left, the extra stats become
    /// one more upgrade step on every stat instead.
    fn add_stats(&self, rune: &mut Rune, count: usize, rng: &mut Rng) {
        for _ in 0..count {
            let next = self
                .stats
                .iter()
                .find(|s| s.enabled && !rune.stats.iter().any(|r| r.id == s.id));
            let Some(next) = next else {
                self.boost(rune);
                continue;
            };
            let value = self.roll_value(next, rune.rarity, rng);
            rune.stats.push(RuneStat {
                id: next.id.clone(),
                value,
                bonus: 0.0,
            });
        }
        self.sort(rune);
    }

    /// Put a rune's stats in table order (older saves may differ); unknown ids go last.
    pub(super) fn sort(&self, rune: &mut Rune) {
        rune.stats.sort_by_key(|s| {
            self.stats
                .iter()
                .position(|d| d.id == s.id)
                .unwrap_or(usize::MAX)
        });
    }

    /// Older saves merged upgrades into the rolled value. Rebuild the bonus from the level:
    /// a stat from the drop got one step per level, one unlocked at a milestone got one step
    /// per level above that milestone. Stats are expected in table order.
    fn split_bonus(&self, rune: &mut Rune) {
        if rune.level == 0 || rune.stats.iter().any(|s| s.bonus != 0.0) {
            return;
        }
        let def = self.rarity(rune.rarity);
        let per_milestone = def.milestone_stats.max(1);
        for (index, stat) in rune.stats.iter_mut().enumerate() {
            let Some(step) = self.stat(&stat.id).map(|d| d.step) else {
                continue;
            };
            let since = if index < def.stats {
                0
            } else {
                let k = (index - def.stats) / per_milestone;
                self.milestones.get(k).copied().unwrap_or(rune.level)
            };
            let upgrades = rune.level.saturating_sub(since) as f32;
            let bonus = (upgrades * step).min((stat.value - 1.0).max(0.0));
            stat.value -= bonus;
            stat.bonus = bonus;
        }
    }

    fn boost(&self, rune: &mut Rune) {
        for s in &mut rune.stats {
            if let Some(def) = self.stat(&s.id) {
                s.bonus = (s.bonus + def.step).round();
            }
        }
    }

    /// A fresh rune: always +0.
    pub(super) fn roll(&self, rarity: u8, rng: &mut Rng) -> Rune {
        let rarity = rarity.min(self.top_rarity());
        let mut rune = Rune {
            rarity,
            level: 0,
            stats: Vec::new(),
        };
        self.add_stats(&mut rune, self.rarity(rarity).stats, rng);
        rune
    }

    /// The multiplier for spending a rune of rarity `spent`, on any rune.
    pub(super) fn rarity_bonus(&self, spent: u8) -> f32 {
        let table = &self.upgrade_rarity_bonus;
        table[(spent as usize).min(table.len() - 1)]
    }

    /// The success chance of upgrading `target` by spending a rune of rarity `spent`, `None` at
    /// the maximum level.
    pub(super) fn upgrade_chance(&self, target: &Rune, spent: u8) -> Option<f32> {
        (target.level < MAX_LEVEL)
            .then(|| (self.upgrade_chances[target.level as usize] * self.rarity_bonus(spent)).min(1.0))
    }

    /// Try one upgrade, spending a rune of rarity `spent`. A success adds one step to every
    /// stat; reaching a milestone also adds the rarity's milestone stats. A failure changes
    /// nothing.
    pub(super) fn upgrade(&self, rune: &mut Rune, spent: u8, rng: &mut Rng) -> Upgrade {
        let Some(chance) = self.upgrade_chance(rune, spent) else {
            return Upgrade::Maxed;
        };
        if rng.f32() >= chance {
            return Upgrade::Failed;
        }
        rune.level += 1;
        self.boost(rune);
        let before = rune.stats.len();
        if self.milestones.contains(&rune.level) {
            self.add_stats(rune, self.rarity(rune.rarity).milestone_stats, rng);
        }
        Upgrade::Success {
            new_stats: rune.stats.len() - before,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        let builtin = || Self::parse(FILE).expect("valid embedded rune config");
        if cfg!(test) {
            return builtin();
        }
        let explicit = crate::webenv::var("WOW_RUNE_CONFIG");
        let path = explicit.as_deref().unwrap_or("config/survivors-runes.toml");
        match crate::local_state::read_to_string(std::path::Path::new(path)) {
            Ok(text) => Self::parse(&text).unwrap_or_else(|e| {
                warn!("Invalid rune config {path}: {e}; using defaults");
                builtin()
            }),
            Err(_) => builtin(),
        }
    }
}

pub(super) fn parse_color(hex: &str) -> Option<Color> {
    if hex.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(hex, 16).ok()?;
    Some(Color::srgb_u8((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Upgrade {
    Success { new_stats: usize },
    Failed,
    Maxed,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RuneStat {
    pub id: String,
    /// The rolled value, whole percents: 12 = +12 %.
    pub value: f32,
    /// Whole percents gained from upgrades, on top of the roll.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub bonus: f32,
}
impl RuneStat {
    /// Roll plus upgrades.
    pub(super) fn total(&self) -> f32 {
        self.value + self.bonus
    }
}
fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

/// Save layout: 0 stored stat values as fractions (0.12); 1 whole percents (12) with upgrades
/// merged into the value; 2 keeps upgrades apart in `bonus`.
const SAVE_VERSION: u32 = 2;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Rune {
    pub rarity: u8,
    pub level: u8,
    pub stats: Vec<RuneStat>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Slot {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rune: Option<Rune>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Save {
    #[serde(default)]
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equipped: Option<usize>,
    /// A dropped rune not yet placed, kept across restarts so it cannot be lost.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending: Option<Rune>,
    #[serde(default)]
    pub slots: Vec<Slot>,
}

impl Save {
    pub(super) fn rune(&self, slot: usize) -> Option<&Rune> {
        self.slots.get(slot).and_then(|s| s.rune.as_ref())
    }
    pub(super) fn equipped_rune(&self) -> Option<&Rune> {
        self.equipped.and_then(|i| self.rune(i))
    }
}

/// The account's runes plus the results-screen state.
#[derive(Resource, Default)]
pub(super) struct Runes {
    pub config: Config,
    pub save: Save,
    /// Bumped on every change, so the run and the panel follow.
    pub revision: u64,
    /// The result of the last action, shown on the panel.
    pub message: String,
    /// The panel is open in the hub (R).
    pub open: bool,
    /// The save could not be read: changes are refused so it is not overwritten.
    pub blocked: bool,
    /// The slot whose Delete button was pressed once, awaiting confirmation.
    pub delete_armed: Option<usize>,
}

/// Something the player does on the rune panel.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RuneAction {
    Place(usize),
    Swap(usize),
    Upgrade(usize),
    Equip(usize),
    /// The first press arms it, the second destroys the slotted rune.
    Delete(usize),
    Discard,
    Close,
}

impl Runes {
    fn normalise(&mut self) {
        if self.save.slots.len() < self.config.slots {
            self.save.slots.resize(self.config.slots, Slot::default());
        }
        if self.save.equipped_rune().is_none() {
            self.save.equipped = None;
        }
        let config = &self.config;
        let version = self.save.version;
        let slotted = self.save.slots.iter_mut().filter_map(|s| s.rune.as_mut());
        for rune in slotted.chain(self.save.pending.as_mut()) {
            if version == 0 {
                for stat in &mut rune.stats {
                    stat.value = (stat.value * 100.0).round().max(1.0);
                    stat.bonus = (stat.bonus * 100.0).round();
                }
            }
            config.sort(rune);
            if version < 2 {
                config.split_bonus(rune);
            }
        }
        self.save.version = SAVE_VERSION;
    }

    fn load(&mut self) {
        self.save = Save::default();
        self.blocked = false;
        if let Some(home) = crate::local_state::home() {
            match crate::local_state::read_to_string(&home.join(SAVE_FILE)) {
                Ok(text) => match toml::from_str::<Save>(&text) {
                    Ok(save) => self.save = save,
                    Err(e) => {
                        self.blocked = true;
                        self.message = format!("Cannot load runes: {e}");
                        warn!("{}", self.message);
                    }
                },
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                    self.blocked = true;
                    self.message = format!("Cannot load runes: {e}");
                    warn!("{}", self.message);
                }
                _ => {}
            }
        }
        self.normalise();
        self.revision = self.revision.wrapping_add(1);
    }

    fn commit(&mut self, save: Save) -> Result<(), String> {
        if self.blocked {
            return Err("The rune save is unreadable; fix it before changing runes.".into());
        }
        if !cfg!(test) {
            if let Some(home) = crate::local_state::home() {
                let text = toml::to_string_pretty(&save).map_err(|e| e.to_string())?;
                crate::local_state::write_atomic(&home.join(SAVE_FILE), &text)
                    .map_err(|e| e.to_string())?;
            }
        }
        self.save = save;
        self.normalise();
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }

    /// Hand the player a dropped rune.
    pub(super) fn grant(&mut self, rune: Rune) -> Result<(), String> {
        let mut save = self.save.clone();
        save.pending = Some(rune);
        self.commit(save)
    }

    /// Apply one panel action; the message says what happened.
    pub(super) fn act(&mut self, action: RuneAction, rng: &mut Rng) -> Result<String, String> {
        let mut save = self.save.clone();
        // Any other action disarms a pending delete.
        let armed = self.delete_armed.take();
        let message = match action {
            RuneAction::Delete(slot) => {
                let Some(rune) = save.rune(slot) else {
                    return Err("That slot is empty.".into());
                };
                let name = self.name(rune);
                if armed != Some(slot) {
                    self.delete_armed = Some(slot);
                    return Ok(format!("Delete {name} from slot {}?", slot + 1));
                }
                save.slots[slot].rune = None;
                if save.equipped == Some(slot) {
                    save.equipped = None;
                }
                format!("{name} destroyed.")
            }
            RuneAction::Close => {
                self.open = false;
                self.message.clear();
                return Ok(String::new());
            }
            RuneAction::Discard => {
                if save.pending.take().is_none() {
                    return Err("No rune to discard.".into());
                }
                "The rune crumbles to dust.".to_string()
            }
            RuneAction::Equip(slot) => {
                if save.rune(slot).is_none() {
                    return Err("That slot is empty.".into());
                }
                save.equipped = if save.equipped == Some(slot) {
                    None
                } else {
                    Some(slot)
                };
                match save.equipped {
                    Some(_) => "Rune equipped.".to_string(),
                    None => "Rune unequipped.".to_string(),
                }
            }
            RuneAction::Place(slot) => {
                let Some(rune) = save.pending.clone() else {
                    return Err("No rune to place.".into());
                };
                let name = self.name(&rune);
                let Some(target) = save.slots.get_mut(slot) else {
                    return Err("No such slot.".into());
                };
                if target.rune.is_some() {
                    return Err("That slot is taken.".into());
                }
                target.rune = Some(rune);
                save.pending = None;
                if save.equipped.is_none() {
                    save.equipped = Some(slot);
                }
                format!("{name} placed in slot {}.", slot + 1)
            }
            RuneAction::Swap(slot) => {
                let Some(rune) = save.pending.clone() else {
                    return Err("No rune to swap in.".into());
                };
                let name = self.name(&rune);
                let Some(target) = save.slots.get_mut(slot).filter(|s| s.rune.is_some()) else {
                    return Err("That slot is empty.".into());
                };
                let old = target.rune.replace(rune);
                save.pending = None;
                let old = old.map(|r| self.name(&r)).unwrap_or_default();
                format!("{name} replaces {old} in slot {}.", slot + 1)
            }
            RuneAction::Upgrade(slot) => {
                let Some(spent) = save.pending.as_ref().map(|r| r.rarity) else {
                    return Err("Win a zone to earn an upgrade attempt.".into());
                };
                let Some(rune) = save.slots.get_mut(slot).and_then(|s| s.rune.as_mut()) else {
                    return Err("That slot is empty.".into());
                };
                let outcome = self.config.upgrade(rune, spent, rng);
                let name = self.name(rune);
                if outcome == Upgrade::Maxed {
                    return Err(format!("{name} is already at +{MAX_LEVEL}."));
                }
                save.pending = None;
                match outcome {
                    Upgrade::Success { new_stats: 0 } => format!("Success! {name}."),
                    Upgrade::Success { new_stats } => format!(
                        "Success! {name} — milestone reached, {new_stats} new stat{}.",
                        if new_stats > 1 { "s" } else { "" }
                    ),
                    _ => format!("The upgrade failed. {name} is unchanged."),
                }
            }
        };
        self.commit(save)?;
        Ok(message)
    }

    /// "Gold Rune +3".
    pub(super) fn name(&self, rune: &Rune) -> String {
        format!("{} +{}", self.config.rarity(rune.rarity).name, rune.level)
    }

    /// One stat line in three parts: the roll "+20", the upgrade bonus " +2" (empty without
    /// upgrades, shown in yellow), and the label "% Damage".
    pub(super) fn stat_parts(&self, stat: &RuneStat) -> (String, String, String) {
        let def = self.config.stat(&stat.id);
        let label = def.map_or(stat.id.as_str(), |s| s.label.as_str());
        let inactive = def.is_none_or(|s| s.effect == RuneEffect::SpGain);
        (
            format!("+{:.0}", stat.value),
            if stat.bonus > 0.0 {
                format!(" +{:.0}", stat.bonus)
            } else {
                String::new()
            },
            format!("% {label}{}", if inactive { " (inactive)" } else { "" }),
        )
    }

    /// The stats a rune of `rarity` gets over its life, in order, with the level that unlocks
    /// each (0 = from the drop) and its roll range: "5 - 15% Damage".
    pub(super) fn possible_stats(&self, rarity: u8) -> Vec<(u8, String)> {
        let def = self.config.rarity(rarity);
        let mut unlock = Vec::new();
        unlock.extend(std::iter::repeat_n(0u8, def.stats));
        for &m in &self.config.milestones {
            unlock.extend(std::iter::repeat_n(m, def.milestone_stats));
        }
        self.config
            .stats
            .iter()
            .filter(|s| s.enabled)
            .zip(unlock)
            .map(|(s, level)| {
                let (lo, hi) = self.config.range(s, rarity);
                (level, format!("{lo} - {hi}% {}", s.label))
            })
            .collect()
    }
}

/// The equipped rune's effects outside the spell engine.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Applied {
    pub version: Option<u64>,
    pub movement: f32,
    pub experience: f32,
}

fn change(effect: RuneEffect, v: f32) -> Option<Change> {
    Some(match effect {
        RuneEffect::Damage => Change::IncreasedDamage(v),
        RuneEffect::FinalDamage => Change::MoreDamage(v),
        RuneEffect::CritChance => Change::CriticalChance(v),
        RuneEffect::CritDamage => Change::CriticalMultiplier(v),
        RuneEffect::Cooldown => Change::IncreasedCooldownRecovery(v),
        RuneEffect::Area => Change::IncreasedArea(v),
        RuneEffect::Duration => Change::IncreasedDuration(v),
        RuneEffect::BossDamage => Change::DamageAgainstBosses(v),
        RuneEffect::LifeLeech => Change::LifeLeech(v),
        RuneEffect::Acceleration
        | RuneEffect::MovementSpeed
        | RuneEffect::Experience
        | RuneEffect::SpGain => return None,
    })
}

/// Feed the equipped rune into `run`; cheap when nothing changed.
pub(super) fn apply(runes: &Runes, run: &mut Run) {
    if run.rune.version == Some(runes.revision) {
        return;
    }
    let mut applied = Applied {
        version: Some(runes.revision),
        ..default()
    };
    let mut modifiers = Vec::new();
    if let Some(rune) = runes.save.equipped_rune() {
        for stat in &rune.stats {
            let Some(def) = runes.config.stat(&stat.id) else {
                continue;
            };
            // Saved as whole percents; the engine takes fractions.
            let v = stat.total() / 100.0;
            match def.effect {
                RuneEffect::MovementSpeed => applied.movement += v,
                RuneEffect::Experience => applied.experience += v,
                // Every speed at once: attacks, cooldown recharge (haste) and movement.
                RuneEffect::Acceleration => {
                    applied.movement += v;
                    for change in [
                        Change::IncreasedAttackSpeed(v),
                        Change::IncreasedCooldownRecovery(v),
                    ] {
                        modifiers.push(Modifier {
                            source: SOURCE,
                            filter: Filter::requiring(Tags(0)),
                            change,
                        });
                    }
                }
                effect => {
                    if let Some(change) = change(effect, v) {
                        modifiers.push(Modifier {
                            source: SOURCE,
                            filter: Filter::requiring(Tags(0)),
                            change,
                        });
                    }
                }
            }
        }
    }
    run.rune = applied;
    run.replace_spell_modifiers(SOURCE, modifiers);
}

fn load(mut runes: ResMut<Runes>) {
    runes.load();
}

fn sync_run(runes: Res<Runes>, mut run: ResMut<Run>) {
    apply(&runes, &mut run);
}

/// A campaign victory drops a rune of the zone's rarity.
fn drop_on_victory(
    run: Res<Run>,
    selection: Res<Selection>,
    mut runes: ResMut<Runes>,
    mut rng: ResMut<Rng>,
) {
    if !run.won || super::training::active(&selection) {
        return;
    }
    let Some(tier) = data::CAMPAIGN.iter().position(|&m| m == run.expedition_map) else {
        return;
    };
    let rarity = runes.config.drop_rarity(tier as u8, &mut rng);
    let rune = runes.config.roll(rarity, &mut rng);
    let name = runes.name(&rune);
    match runes.grant(rune) {
        Ok(()) => runes.message = format!("You found a {name}!"),
        Err(e) => {
            warn!("{e}");
            runes.message = e;
        }
    }
}

/// Test mode: on the training ground every Training Boss kill drops a rune of a random rarity
/// and opens the panel, so the whole rune flow can be tried without a 15-minute run.
fn training_drops(
    mut loot: MessageReader<super::enemies::Loot>,
    selection: Res<Selection>,
    mut runes: ResMut<Runes>,
    mut rng: ResMut<Rng>,
) {
    let training = super::training::active(&selection);
    for drop in loot.read() {
        if !training || !drop.boss {
            continue;
        }
        let rarity = rng.index(runes.config.rarities.len()) as u8;
        let rune = runes.config.roll(rarity, &mut rng);
        let name = runes.name(&rune);
        runes.message = match runes.grant(rune) {
            Ok(()) => format!("Training loot: {name}!"),
            Err(e) => e,
        };
        runes.open = true;
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Runes>()
        .add_systems(Startup, load)
        .add_systems(OnEnter(Phase::GameOver), drop_on_victory)
        .add_systems(Update, sync_run.before(super::abilities::cast_abilities))
        .add_systems(
            Update,
            training_drops.after(super::enemies::drop_loot),
        );
    ui::plugin(app);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runes() -> Runes {
        let mut r = Runes {
            config: Config::parse(FILE).unwrap(),
            ..default()
        };
        r.normalise();
        r
    }

    #[test]
    fn embedded_config_has_one_rarity_per_zone_and_one_above_the_last() {
        let c = Config::parse(FILE).unwrap();
        assert_eq!(c.rarities.len(), data::CAMPAIGN.len() + 1);
        assert!(c.rarities.iter().all(|r| r.milestone_stats == 1));
        assert_eq!(c.slots, 3);
        assert!(Config::parse(&FILE.replace("slots = 3", "slots = 0")).is_err());
        assert!(Config::parse(&FILE.replace("0.30, 0.20,", "0.30,")).is_err());
    }

    #[test]
    fn new_runes_are_plus_zero_with_distinct_stats_in_the_tier_range() {
        let c = Config::parse(FILE).unwrap();
        let mut rng = Rng(7);
        for rarity in 0..c.rarities.len() as u8 {
            // Every rune of a rarity has the first stats of the table, in table order.
            let expected: Vec<&str> = c
                .stats
                .iter()
                .filter(|s| s.enabled)
                .take(c.rarity(rarity).stats)
                .map(|s| s.id.as_str())
                .collect();
            for _ in 0..50 {
                let rune = c.roll(rarity, &mut rng);
                assert_eq!(rune.level, 0);
                let ids: Vec<&str> = rune.stats.iter().map(|s| s.id.as_str()).collect();
                assert_eq!(ids, expected);
                for s in &rune.stats {
                    let def = c.stat(&s.id).unwrap();
                    assert!(def.enabled);
                    let (lo, hi) = c.range(def, rarity);
                    assert_eq!(s.value.fract(), 0.0, "{} = {}", s.id, s.value);
                    assert!((lo as f32..=hi as f32).contains(&s.value));
                }
            }
        }
    }

    #[test]
    fn stone_starts_with_damage_cooldown_crit_and_milestones_unlock_the_next_stats() {
        let mut c = Config::parse(FILE).unwrap();
        c.upgrade_chances = vec![1.0; MAX_LEVEL as usize];
        let mut rng = Rng(2);
        let mut rune = c.roll(0, &mut rng);
        let ids = |r: &Rune| r.stats.iter().map(|s| s.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&rune), ["damage", "cooldown", "crit_chance"]);
        for _ in 0..5 {
            c.upgrade(&mut rune, 0, &mut rng);
        }
        assert_eq!(ids(&rune), ["damage", "cooldown", "crit_chance", "crit_damage"]);
        // A shuffled old save is put back in table order.
        let mut shuffled = rune.clone();
        shuffled.stats.reverse();
        c.sort(&mut shuffled);
        assert_eq!(shuffled, rune);
    }

    #[test]
    fn old_fraction_saves_become_whole_percents() {
        let mut r = runes();
        r.save = Save {
            version: 0,
            equipped: Some(0),
            pending: None,
            slots: vec![Slot {
                rune: Some(Rune {
                    rarity: 4,
                    level: 1,
                    stats: vec![
                        RuneStat {
                            id: "damage".into(),
                            value: 0.20,
                            bonus: 0.0,
                        },
                        RuneStat {
                            id: "crit_chance".into(),
                            value: 0.042,
                            bonus: 0.0,
                        },
                    ],
                }),
            }],
        };
        r.normalise();
        let totals: Vec<f32> = r.save.rune(0).unwrap().stats.iter().map(|s| s.total()).collect();
        assert_eq!(totals, [20.0, 4.0]);
        // The +1 upgrade is split out of the merged value.
        assert_eq!(r.save.rune(0).unwrap().stats[0].bonus, 1.0);
        assert_eq!(r.save.version, SAVE_VERSION);
        // A second pass changes nothing.
        let once = r.save.clone();
        r.normalise();
        assert_eq!(r.save, once);
    }

    #[test]
    fn merged_upgrades_of_version_1_saves_are_split_into_the_yellow_bonus() {
        let mut r = runes();
        let stat = |id: &str, value: f32| RuneStat {
            id: id.into(),
            value,
            bonus: 0.0,
        };
        // A Diamond +4 and a Bronze +7 saved with upgrades merged into the values.
        r.save = Save {
            version: 1,
            equipped: None,
            pending: None,
            slots: vec![
                Slot {
                    rune: Some(Rune {
                        rarity: 5,
                        level: 4,
                        stats: vec![stat("damage", 25.0), stat("cooldown", 10.0)],
                    }),
                },
                Slot {
                    rune: Some(Rune {
                        rarity: 0,
                        level: 7,
                        // damage, cooldown, crit chance from the drop; crit damage from +5.
                        stats: vec![
                            stat("damage", 17.0),
                            stat("cooldown", 9.0),
                            stat("crit_chance", 9.0),
                            stat("crit_damage", 12.0),
                        ],
                    }),
                },
            ],
        };
        r.normalise();
        let titanium = &r.save.rune(0).unwrap().stats;
        assert_eq!((titanium[0].value, titanium[0].bonus), (21.0, 4.0));
        assert_eq!((titanium[1].value, titanium[1].bonus), (6.0, 4.0));
        let stone = &r.save.rune(1).unwrap().stats;
        assert_eq!((stone[0].value, stone[0].bonus), (10.0, 7.0));
        // Unlocked at +5, so only two upgrades since.
        assert_eq!((stone[3].value, stone[3].bonus), (10.0, 2.0));
        // Runes that already track their bonus are left alone.
        let once = r.save.clone();
        r.save.version = 1;
        r.normalise();
        assert_eq!(r.save, once);
    }

    #[test]
    fn possible_stats_list_every_unlock_with_its_range() {
        let r = runes();
        let stone = r.possible_stats(0);
        // 3 from the drop, 1 at +5, 1 at +10.
        assert_eq!(stone.len(), 5);
        assert_eq!(stone[0], (0, "5 - 15% Damage".to_string()));
        assert_eq!(stone[3].0, 5);
        assert_eq!(stone[4].0, 10);
        // Higher rarities roll higher.
        assert_eq!(r.possible_stats(2)[0], (0, "7 - 19% Damage".to_string()));
    }

    #[test]
    fn drops_stay_at_the_zone_rarity_or_one_above() {
        let c = Config::parse(FILE).unwrap();
        let mut rng = Rng(11);
        let mut upper = 0;
        for _ in 0..10_000 {
            match c.drop_rarity(2, &mut rng) {
                2 => {}
                3 => upper += 1,
                other => panic!("rarity {other}"),
            }
        }
        assert!((700..1300).contains(&upper), "{upper}");
        // The top rarity has nothing above it.
        assert_eq!(c.drop_rarity(6, &mut rng), 6);
    }

    #[test]
    fn upgrades_add_a_step_and_milestones_add_stats() {
        let mut c = Config::parse(FILE).unwrap();
        c.upgrade_chances = vec![1.0; MAX_LEVEL as usize];
        let mut rng = Rng(3);
        let mut rune = c.roll(0, &mut rng);
        let first = rune.stats[0].clone();
        let step = c.stat(&first.id).unwrap().step;
        assert_eq!(c.upgrade(&mut rune, 0, &mut rng), Upgrade::Success { new_stats: 0 });
        // The roll is kept; the upgrade goes in the bonus shown in yellow.
        assert_eq!(rune.stats[0].value, first.value);
        assert_eq!(rune.stats[0].bonus, step);
        assert_eq!(rune.stats[0].total(), first.value + step);
        for _ in 1..4 {
            c.upgrade(&mut rune, 0, &mut rng);
        }
        assert_eq!(rune.level, 4);
        assert_eq!(
            c.upgrade(&mut rune, 0, &mut rng),
            Upgrade::Success { new_stats: 1 }
        );
        assert_eq!(rune.level, 5);
        assert_eq!(rune.stats.len(), 3 + 1);
        for _ in 5..10 {
            c.upgrade(&mut rune, 0, &mut rng);
        }
        assert_eq!(rune.level, 10);
        assert_eq!(rune.stats.len(), 5);
        assert_eq!(c.upgrade(&mut rune, 0, &mut rng), Upgrade::Maxed);
        c.upgrade_chances = vec![0.0; MAX_LEVEL as usize];
        let mut fresh = c.roll(0, &mut rng);
        let before = fresh.clone();
        assert_eq!(c.upgrade(&mut fresh, 5, &mut rng), Upgrade::Failed);
        assert_eq!(fresh, before);
    }

    #[test]
    fn the_spent_rune_rarity_alone_sets_the_upgrade_multiplier() {
        let c = Config::parse(FILE).unwrap();
        let mut rng = Rng(1);
        let bronze = c.roll(0, &mut rng);
        let mut diamond = c.roll(5, &mut rng);
        diamond.level = 5; // 5 % base
        for (spent, multiplier) in [(0, 1.0), (1, 2.0), (2, 3.0), (6, 7.0)] {
            assert_eq!(c.rarity_bonus(spent), multiplier);
            let low = c.upgrade_chance(&diamond, spent).unwrap();
            // The same multiplier on a rune above the spent one as on one below it.
            assert!((low - 0.05 * multiplier).abs() < 1e-6);
        }
        assert!((c.upgrade_chance(&bronze, 1).unwrap() - 0.60).abs() < 1e-6);
        // Capped at 100 %.
        assert_eq!(c.upgrade_chance(&bronze, 6), Some(1.0));
    }

    #[test]
    fn panel_actions_place_swap_upgrade_and_equip() {
        let mut r = runes();
        let mut rng = Rng(5);
        assert!(r.act(RuneAction::Place(0), &mut rng).is_err());
        let rune = r.config.roll(0, &mut rng);
        r.grant(rune).unwrap();
        r.act(RuneAction::Place(0), &mut rng).unwrap();
        assert!(r.save.pending.is_none());
        assert_eq!(r.save.equipped, Some(0));
        // A slotted rune cannot be overwritten by Place, only by Swap.
        let rune = r.config.roll(1, &mut rng);
        r.grant(rune).unwrap();
        assert!(r.act(RuneAction::Place(0), &mut rng).is_err());
        r.act(RuneAction::Swap(0), &mut rng).unwrap();
        assert_eq!(r.save.rune(0).unwrap().rarity, 1);
        // An upgrade spends the dropped rune whatever the outcome.
        let rune = r.config.roll(0, &mut rng);
        r.grant(rune).unwrap();
        r.act(RuneAction::Upgrade(0), &mut rng).unwrap();
        assert!(r.save.pending.is_none());
        assert!(r.act(RuneAction::Upgrade(0), &mut rng).is_err());
        r.act(RuneAction::Equip(0), &mut rng).unwrap();
        assert_eq!(r.save.equipped, None);
        assert!(r.act(RuneAction::Equip(1), &mut rng).is_err());
    }

    #[test]
    fn deleting_a_rune_needs_a_second_press_on_the_same_slot() {
        let mut r = runes();
        let mut rng = Rng(4);
        for slot in 0..2 {
            let rune = r.config.roll(0, &mut rng);
            r.grant(rune).unwrap();
            r.act(RuneAction::Place(slot), &mut rng).unwrap();
        }
        assert_eq!(r.save.equipped, Some(0));
        assert!(r.act(RuneAction::Delete(2), &mut rng).is_err());
        // The first press changes nothing; another slot or action disarms it.
        r.act(RuneAction::Delete(0), &mut rng).unwrap();
        assert!(r.save.rune(0).is_some());
        r.act(RuneAction::Delete(1), &mut rng).unwrap();
        r.act(RuneAction::Delete(0), &mut rng).unwrap();
        assert!(r.save.rune(0).is_some() && r.save.rune(1).is_some());
        r.act(RuneAction::Equip(1), &mut rng).unwrap();
        r.act(RuneAction::Delete(0), &mut rng).unwrap();
        assert!(r.save.rune(0).is_some());
        // Deleting the equipped rune leaves nothing equipped, and its effects stop.
        r.act(RuneAction::Delete(1), &mut rng).unwrap();
        r.act(RuneAction::Delete(1), &mut rng).unwrap();
        assert!(r.save.rune(1).is_none());
        assert_eq!(r.save.equipped, None);
    }

    #[test]
    fn save_roundtrips_through_toml() {
        let mut r = runes();
        let mut rng = Rng(9);
        let rune = r.config.roll(3, &mut rng);
        r.grant(rune).unwrap();
        r.act(RuneAction::Place(1), &mut rng).unwrap();
        let rune = r.config.roll(0, &mut rng);
        r.grant(rune).unwrap();
        let text = toml::to_string_pretty(&r.save).unwrap();
        let back: Save = toml::from_str(&text).unwrap();
        assert_eq!(back, r.save);
    }

    #[test]
    fn acceleration_speeds_up_attacks_cooldowns_and_movement() {
        let mut r = runes();
        r.save.slots[0].rune = Some(Rune {
            rarity: 0,
            level: 0,
            stats: vec![RuneStat {
                id: "acceleration".into(),
                value: 20.0,
                bonus: 0.0,
            }],
        });
        r.save.equipped = Some(0);
        r.revision += 1;
        let mut run = Run::default();
        let strike = data::ability("heroic_strike");
        let fire = data::ability("fireball");
        let (strike_cd, fire_cd) = (
            run.resolve_spell(strike, 1).cooldown,
            run.resolve_spell(fire, 1).cooldown,
        );
        let speed = run.stats().speed;
        apply(&r, &mut run);
        assert!(run.resolve_spell(strike, 1).cooldown < strike_cd);
        assert!(run.resolve_spell(fire, 1).cooldown < fire_cd);
        assert!((run.stats().speed - speed * 1.20).abs() < 1e-4);
    }

    #[test]
    fn the_equipped_rune_feeds_the_spell_engine_and_hero_stats() {
        let mut r = runes();
        r.save.slots[0].rune = Some(Rune {
            rarity: 0,
            level: 0,
            stats: vec![
                // Roll and upgrade bonus both count: 30 + 10 = 40 %.
                RuneStat {
                    id: "crit_damage".into(),
                    value: 30.0,
                    bonus: 10.0,
                },
                RuneStat {
                    id: "movement_speed".into(),
                    value: 10.0,
                    bonus: 0.0,
                },
            ],
        });
        r.save.equipped = Some(0);
        r.revision += 1;
        let mut run = Run::default();
        let fire = data::ability("fireball");
        let before = run.resolve_spell(fire, 1).crit_multiplier;
        let speed = run.stats().speed;
        apply(&r, &mut run);
        assert!((run.resolve_spell(fire, 1).crit_multiplier - before - 0.40).abs() < 1e-4);
        assert!((run.stats().speed - speed * 1.10).abs() < 1e-4);
        // Unequipping removes everything.
        r.save.equipped = None;
        r.revision += 1;
        apply(&r, &mut run);
        assert!((run.resolve_spell(fire, 1).crit_multiplier - before).abs() < 1e-4);
        assert!((run.stats().speed - speed).abs() < 1e-4);
    }
}
