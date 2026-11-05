//! Character-owned, rolled equipment. Saved rolls never depend on later loot-table edits.
use super::{
    characters::Roster,
    enemies::Loot,
    hero::Hero,
    reward_config::weighted_index,
    spell_engine::{Change, Filter, Modifier, Tags},
    Phase, Rng, Run,
};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
pub(super) mod armour;
pub(super) mod combat;
mod config;
mod equipment;
mod item_level;
pub(super) mod legendary;
mod starter;
mod tooltip;
pub(super) mod visuals;
pub(super) mod weapons;
pub(super) use config::Config;
pub(super) use equipment::EquipmentType;
pub(super) use visuals::Appearance;
const SOURCE: u64 = 0x3000_0000;
pub(super) const MAX_BASE_BLOCK: f32 = 0.30;
pub(in crate::survivors) const MAX_ITEM_LEVEL: u32 = 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SlotKind {
    Head,
    Neck,
    Shoulders,
    Back,
    Chest,
    Shirt,
    Tabard,
    Wrists,
    Hands,
    Waist,
    Legs,
    Feet,
    Ring,
    Trinket,
    MainHand,
    OffHand,
    Ranged,
}
impl SlotKind {
    pub fn weapon(self) -> bool {
        matches!(self, Self::MainHand | Self::OffHand | Self::Ranged)
    }
}
pub(super) const SLOTS: [(&str, SlotKind); 19] = [
    ("Head", SlotKind::Head),
    ("Neck", SlotKind::Neck),
    ("Shoulders", SlotKind::Shoulders),
    ("Back", SlotKind::Back),
    ("Chest", SlotKind::Chest),
    ("Shirt", SlotKind::Shirt),
    ("Tabard", SlotKind::Tabard),
    ("Wrists", SlotKind::Wrists),
    ("Hands", SlotKind::Hands),
    ("Waist", SlotKind::Waist),
    ("Legs", SlotKind::Legs),
    ("Feet", SlotKind::Feet),
    ("Ring 1", SlotKind::Ring),
    ("Ring 2", SlotKind::Ring),
    ("Trinket 1", SlotKind::Trinket),
    ("Trinket 2", SlotKind::Trinket),
    ("Main hand", SlotKind::MainHand),
    ("Off hand", SlotKind::OffHand),
    ("Ranged", SlotKind::Ranged),
];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Rarity {
    Normal,
    Magic,
    Epic,
    Legendary,
}
impl Rarity {
    pub fn color(self) -> Color {
        match self {
            Self::Normal => Color::WHITE,
            Self::Magic => Color::srgb(0.12, 1.0, 0.0),
            Self::Epic => Color::srgb(0.72, 0.3, 1.0),
            Self::Legendary => Color::srgb(1.0, 0.5, 0.0),
        }
    }
    fn legacy_count(self) -> usize {
        match self {
            Self::Normal => 1,
            Self::Magic => 2,
            Self::Epic => 4,
            Self::Legendary => 4,
        }
    }
    fn affix_limits(self) -> (usize, usize) {
        match self {
            Self::Normal => (0, 1),
            Self::Magic => (2, 2),
            Self::Epic | Self::Legendary => (4, 3),
        }
    }
    fn primary_count(self) -> usize {
        match self {
            Self::Normal | Self::Magic => 1,
            Self::Epic => 2,
            Self::Legendary => 2,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum AffixKind {
    Prefix,
    Suffix,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Effect {
    Stamina,
    Strength,
    Agility,
    Intelligence,
    BlockChance,
    Damage,
    DotDamage,
    CritChance,
    CritMultiplier,
    CooldownRecovery,
    Area,
    Duration,
    Repeat,
    Projectiles,
    Chain,
    Fork,
    MeleeTargets,
    FlatDamage,
    SpellDamage,
    AttackSpeed,
    Life,
    Armour,
    FireResistance,
    ColdResistance,
    LightningResistance,
    NatureResistance,
    ArcaneResistance,
    HolyResistance,
    ShadowResistance,
    AllResistance,
    LifeLeech,
    LifeOnKill,
    Regeneration,
    MovementSpeed,
    DamageReduction,
    HealingEffectiveness,
    BossDamage,
    FullLifeDamage,
    LowLifeDamage,
    BleedChance,
    IgniteChance,
    ChillChance,
    KnockbackChance,
    ArmourPenetration,
    PickupRadius,
    Experience,
    Luck,
    MagicFind,
    Proc,
}
impl Effect {
    fn affix_kind(self) -> AffixKind {
        if matches!(
            self,
            Self::Damage
                | Self::DotDamage
                | Self::CritChance
                | Self::CritMultiplier
                | Self::CooldownRecovery
                | Self::Area
                | Self::Duration
                | Self::Repeat
                | Self::Projectiles
                | Self::Chain
                | Self::Fork
                | Self::MeleeTargets
                | Self::FlatDamage
                | Self::SpellDamage
                | Self::AttackSpeed
                | Self::BossDamage
                | Self::FullLifeDamage
                | Self::LowLifeDamage
                | Self::BleedChance
                | Self::IgniteChance
                | Self::ChillChance
                | Self::KnockbackChance
                | Self::ArmourPenetration
                | Self::Proc
        ) {
            AffixKind::Prefix
        } else {
            AffixKind::Suffix
        }
    }
    fn primary(self) -> bool {
        matches!(
            self,
            Self::Stamina | Self::Strength | Self::Agility | Self::Intelligence
        )
    }
    fn offensive_tags(self, class: u8) -> Option<Tags> {
        match (self, class) {
            (Self::Strength, 1 | 2) | (Self::Agility, 3 | 4 | 7 | 11) => Some(Tags::ATTACK),
            (Self::Intelligence, 2 | 5 | 7 | 8 | 9 | 11) => Some(Tags::SPELL),
            _ => None,
        }
    }
    fn change(self, v: f32) -> Option<Change> {
        Some(match self {
            Self::Damage => Change::IncreasedDamage(v),
            Self::DotDamage => Change::IncreasedDotDamage(v),
            Self::CritChance => Change::CriticalChance(v),
            Self::CritMultiplier => Change::CriticalMultiplier(v),
            Self::CooldownRecovery => Change::IncreasedCooldownRecovery(v),
            Self::Area => Change::IncreasedArea(v),
            Self::Duration => Change::IncreasedDuration(v),
            Self::Repeat => Change::RecastChance(v),
            Self::Projectiles => Change::AdditionalProjectiles(v as u32),
            Self::Chain => Change::AdditionalChains(v as u32),
            Self::Fork => Change::Fork(v as u32),
            Self::MeleeTargets => Change::AdditionalMeleeTargets(v as u32),
            Self::FlatDamage => Change::FlatDamage(v),
            Self::SpellDamage => Change::FlatSpellDamage(v),
            Self::AttackSpeed => Change::IncreasedAttackSpeed(v),
            Self::LifeLeech => Change::LifeLeech(v),
            Self::BossDamage => Change::DamageAgainstBosses(v),
            Self::FullLifeDamage => Change::DamageAgainstFullLife(v),
            Self::LowLifeDamage => Change::DamageAgainstLowLife(v),
            Self::BleedChance => Change::BleedChance(v),
            Self::IgniteChance => Change::IgniteChance(v),
            Self::ChillChance => Change::ChillChance(v),
            Self::KnockbackChance => Change::KnockbackChance(v),
            Self::ArmourPenetration => Change::ArmourPenetration(v),
            _ => return None,
        })
    }
    fn discrete(self) -> bool {
        if self.resistance() {
            return true;
        }
        matches!(
            self,
            Self::Stamina
                | Self::Strength
                | Self::Agility
                | Self::Intelligence
                | Self::Projectiles
                | Self::Chain
                | Self::Fork
                | Self::MeleeTargets
                | Self::Life
                | Self::Armour
                | Self::FlatDamage
                | Self::SpellDamage
                | Self::LifeOnKill
        )
    }
    fn global(self) -> bool {
        if self.resistance() {
            return true;
        }
        if self == Self::BlockChance {
            return true;
        }
        matches!(
            self,
            Self::Stamina
                | Self::Strength
                | Self::Agility
                | Self::Intelligence
                | Self::Life
                | Self::Armour
                | Self::LifeOnKill
                | Self::Regeneration
                | Self::MovementSpeed
                | Self::DamageReduction
                | Self::HealingEffectiveness
                | Self::PickupRadius
                | Self::Experience
                | Self::Luck
                | Self::MagicFind
        )
    }
    fn valid_slot(self, slot: SlotKind) -> bool {
        match self {
            Self::BlockChance => slot == SlotKind::OffHand,
            Self::FlatDamage | Self::AttackSpeed | Self::LifeLeech => slot.weapon(),
            Self::Luck => matches!(slot, SlotKind::Ring | SlotKind::Neck),
            Self::Proc => slot == SlotKind::Trinket,
            _ => true,
        }
    }
    fn resistance(self) -> bool {
        matches!(
            self,
            Self::FireResistance
                | Self::ColdResistance
                | Self::LightningResistance
                | Self::NatureResistance
                | Self::ArcaneResistance
                | Self::HolyResistance
                | Self::ShadowResistance
                | Self::AllResistance
        )
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ProcKind {
    Damage,
    CritChance,
    AttackSpeed,
    ClassSpell,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProcRoll {
    pub kind: ProcKind,
    pub cooldown: f32,
    pub duration: f32,
    pub power: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spell: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<u8>,
}
impl ProcRoll {
    fn valid(&self) -> bool {
        self.cooldown.is_finite()
            && (0.2..=300.0).contains(&self.cooldown)
            && self.duration.is_finite()
            && (0.0..=120.0).contains(&self.duration)
            && self.power.is_finite()
            && (0.001..=10.0).contains(&self.power)
            && if matches!(self.kind, ProcKind::ClassSpell) {
                self.class
                    .and_then(|id| super::data::CLASSES.iter().find(|c| c.id == id))
                    .is_some_and(|class| {
                        self.spell.as_deref().is_some_and(|key| {
                            class.pool.contains(&key) && combat::proc_spell(key).is_some()
                        })
                    })
            } else {
                self.spell.is_none() && self.class.is_none() && self.duration > 0.0
            }
    }
}
#[derive(Clone, Copy, Default)]
pub(super) struct Bonuses {
    pub resistances: super::resistances::Resistances,
    pub block: f32,
    pub life_on_kill: f32,
    pub regeneration: f32,
    pub movement: f32,
    pub reduction: f32,
    pub healing: f32,
    pub pickup: f32,
    pub experience: f32,
    pub luck: f32,
    pub magic_find: f32,
}
impl Bonuses {
    fn add(&mut self, effect: Effect, value: f32) {
        match effect {
            Effect::FireResistance => self.resistances.fire += value,
            Effect::ColdResistance => self.resistances.cold += value,
            Effect::LightningResistance => self.resistances.lightning += value,
            Effect::NatureResistance => self.resistances.nature += value,
            Effect::ArcaneResistance => self.resistances.arcane += value,
            Effect::HolyResistance => self.resistances.holy += value,
            Effect::ShadowResistance => self.resistances.shadow += value,
            Effect::AllResistance => self.resistances.add_all(value),
            Effect::BlockChance => self.block += value,
            Effect::LifeOnKill => self.life_on_kill += value,
            Effect::Regeneration => self.regeneration += value,
            Effect::MovementSpeed => self.movement += value,
            Effect::DamageReduction => self.reduction += value,
            Effect::HealingEffectiveness => self.healing += value,
            Effect::PickupRadius => self.pickup += value,
            Effect::Experience => self.experience += value,
            Effect::Luck => self.luck += value,
            Effect::MagicFind => self.magic_find += value,
            _ => {}
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Affix {
    pub id: String,
    pub name: String,
    pub kind: AffixKind,
    pub group: String,
    pub effect: Effect,
    pub value: f32,
    pub tags: Vec<config::Tag>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proc: Option<ProcRoll>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Item {
    /// Zero preserves saved allocations from before prefix/suffix budgets.
    #[serde(default)]
    pub affix_rules: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legendary: Option<legendary::Roll>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_damage: Option<weapons::DamageRange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_armour: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub appearance: Option<Appearance>,
    #[serde(default)]
    pub equipment_type: EquipmentType,
    #[serde(default)]
    pub base_block: f32,
    #[serde(default)]
    pub loot_class: Option<u8>,
    pub id: u64,
    pub base: String,
    pub name: String,
    pub slot: SlotKind,
    pub icon: String,
    pub rarity: Rarity,
    pub level: u32,
    pub affixes: Vec<Affix>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Save {
    version: u32,
    #[serde(default)]
    starter_granted: bool,
    #[serde(default)]
    titan_grip: bool,
    next_id: u64,
    /// The bag grid in cell order: an item id, or 0 for an empty cell. Older saves have none and
    /// fill up in item order; see [`bag_layout`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    bag_cells: Vec<u64>,
    items: Vec<Item>,
    equipped: BTreeMap<String, u64>,
}
impl Default for Save {
    fn default() -> Self {
        Self {
            version: 1,
            starter_granted: false,
            titan_grip: false,
            next_id: 1,
            bag_cells: Vec::new(),
            items: Vec::new(),
            equipped: BTreeMap::new(),
        }
    }
}
/// The bag cells of `save`, made consistent: worn, deleted and repeated ids leave their cell, and
/// any bag item without a cell takes the first free one. Items stay where the player put them.
fn bag_layout(save: &Save) -> Vec<u64> {
    let worn: BTreeSet<u64> = save.equipped.values().copied().collect();
    let bag: BTreeSet<u64> = save
        .items
        .iter()
        .map(|item| item.id)
        .filter(|id| !worn.contains(id))
        .collect();
    let mut placed = BTreeSet::new();
    let mut cells: Vec<u64> = save
        .bag_cells
        .iter()
        .map(|&id| {
            if bag.contains(&id) && placed.insert(id) {
                id
            } else {
                0
            }
        })
        .collect();
    for item in &save.items {
        if !bag.contains(&item.id) || !placed.insert(item.id) {
            continue;
        }
        match cells.iter().position(|&cell| cell == 0) {
            Some(free) => cells[free] = item.id,
            None => cells.push(item.id),
        }
    }
    while cells.last() == Some(&0) {
        cells.pop();
    }
    cells
}
#[derive(Resource)]
pub(super) struct Inventory {
    class: u8,
    character: Option<u64>,
    save: Save,
    pub config: Config,
    pub revision: u64,
    pub notice: String,
    blocked: bool,
    native_titan_grip: bool,
}
impl Default for Inventory {
    fn default() -> Self {
        Self {
            character: None,
            class: 1,
            save: Save::default(),
            config: Config::load(),
            revision: 0,
            notice: String::new(),
            blocked: false,
            native_titan_grip: false,
        }
    }
}
impl Inventory {
    #[cfg(not(target_arch = "wasm32"))]
    pub(in crate::survivors) fn shared_save(&self) -> Result<String, String> {
        toml::to_string(&self.save).map_err(|e| e.to_string())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(in crate::survivors) fn shared_character(&self) -> Option<u64> {
        self.character
    }

    /// Trusted friend-session inventory; never binds or writes a character on the host.
    #[cfg(not(target_arch = "wasm32"))]
    pub(in crate::survivors) fn from_shared(class: u8, text: &str) -> Result<Self, String> {
        Ok(Self {
            class,
            save: toml::from_str(text).map_err(|e| e.to_string())?,
            revision: 1,
            ..Self::default()
        })
    }
    #[cfg(test)]
    pub(super) fn fixture(mut items: Vec<Item>) -> Self {
        for (index, item) in items.iter_mut().enumerate() {
            item.id = index as u64 + 1;
        }
        Self {
            character: Some(1),
            class: 1,
            save: Save {
                next_id: items.len() as u64 + 1,
                items,
                ..default()
            },
            config: Config::builtin(),
            revision: 0,
            notice: String::new(),
            blocked: false,
            native_titan_grip: false,
        }
    }
    #[cfg(test)]
    pub fn all(&self) -> &[Item] {
        &self.save.items
    }
    pub fn item(&self, id: u64) -> Option<&Item> {
        self.save.items.iter().find(|i| i.id == id)
    }
    pub fn equipped(&self, slot: &str) -> Option<&Item> {
        self.save.equipped.get(slot).and_then(|&id| self.item(id))
    }
    pub fn worn(&self, id: u64) -> bool {
        self.save.equipped.values().any(|&v| v == id)
    }
    #[cfg(test)]
    pub fn bag(&self) -> impl Iterator<Item = &Item> {
        bag_layout(&self.save)
            .into_iter()
            .filter_map(move |id| self.item(id))
    }
    /// The bag grid, one entry per cell up to the last occupied one.
    pub fn bag_cells(&self) -> Vec<Option<&Item>> {
        bag_layout(&self.save)
            .into_iter()
            .map(|id| self.item(id))
            .collect()
    }
    fn persist(&self, save: &Save) -> Result<(), String> {
        if self.blocked {
            return Err("Inventory save is unreadable; fix it before changing items.".into());
        }
        let Some(id) = self.character else {
            return Err("Enter the world on a character first.".into());
        };
        let Some(home) = crate::local_state::home() else {
            return Ok(());
        };
        let text = toml::to_string_pretty(save).map_err(|e| e.to_string())?;
        crate::local_state::write_atomic(&home.join(format!("characters/{id}-items.toml")), &text)
            .map_err(|e| e.to_string())
    }
    fn commit(&mut self, mut save: Save) -> Result<(), String> {
        legendary::validate_loadout(&save)?;
        let previous_permission = self.save.titan_grip;
        save.titan_grip = self.native_titan_grip || legendary::titan_grip(&save);
        if previous_permission && !save.titan_grip {
            if save.equipped.get("Off hand").is_some_and(|id| {
                save.items
                    .iter()
                    .any(|item| item.id == *id && item.equipment_type == EquipmentType::TwoHand)
            }) || save.equipped.get("Main hand").is_some_and(|id| {
                save.items
                    .iter()
                    .any(|item| item.id == *id && item.equipment_type.two_handed())
            }) {
                save.equipped.remove("Off hand");
            }
        }
        save.bag_cells = bag_layout(&save);
        self.validate_loadout(&save)?;
        self.persist(&save)?;
        self.save = save;
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }
    fn bind(&mut self, character: Option<u64>) {
        self.character = character;
        self.save = Save::default();
        self.blocked = false;
        self.notice.clear();
        if let (Some(id), Some(home)) = (character, crate::local_state::home()) {
            match crate::local_state::read_to_string(
                &home.join(format!("characters/{id}-items.toml")),
            ) {
                Ok(text) => match Self::decode(&text) {
                    Ok(save) => self.save = save,
                    Err(e) => {
                        self.blocked = true;
                        self.notice = format!("Cannot load inventory: {e}");
                    }
                },
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                    self.blocked = true;
                    self.notice = format!("Cannot load inventory: {e}");
                }
                _ => {}
            }
        }
        if !self.blocked {
            let mut save = self.save.clone();
            let mut appearances_changed = false;
            for item in &mut save.items {
                if item.appearance.is_none() {
                    if let Some((appearance, icon)) = self.config.appearance_for(item, self.class) {
                        item.appearance = Some(appearance);
                        item.icon = icon.to_owned();
                        appearances_changed = true;
                    }
                }
            }
            save.equipped.retain(|name, id| {
                let slot = SLOTS.iter().find(|(s, _)| s == name).map(|(_, s)| *s);
                save.items.iter().any(|item| {
                    item.id == *id && slot.is_some_and(|slot| self.can_equip(item, slot))
                })
            });
            if appearances_changed || save.equipped != self.save.equipped {
                if let Err(error) = self.commit(save) {
                    self.notice = error;
                    self.blocked = true;
                }
            }
        }
        self.revision = self.revision.wrapping_add(1);
    }
    fn decode(text: &str) -> Result<Save, String> {
        let mut save: Save = toml::from_str(text).map_err(|e| e.to_string())?;
        // Identity is the saved unique ID; display names can change or be translated.
        for item in &mut save.items {
            if let Some(definition) = item
                .legendary
                .as_ref()
                .and_then(|roll| legendary::definition(roll.unique))
            {
                item.name = definition.name.into();
            }
        }
        if save.version != 1 || save.next_id == 0 {
            return Err("unsupported inventory".into());
        }
        let mut ids = BTreeSet::new();
        for item in &save.items {
            if item.id == 0
                || item.id >= save.next_id
                || !ids.insert(item.id)
                || item.name.is_empty()
                || !item.valid_equipment()
                || item.appearance.is_some_and(|a| !a.valid_for(item.slot))
                || item
                    .loot_class
                    .is_some_and(|id| !super::data::CLASSES.iter().any(|c| c.id == id))
                || item.level == 0
                || !legendary::validate_item(item)
                || item.affix_rules > 1
                || (item.affix_rules == 0
                    && item.affixes.len() != item.rarity.legacy_count()
                    && !starter::is_plain(item))
            {
                return Err("invalid item".into());
            }
            let mut groups = BTreeSet::new();
            let mut kinds = [0; 2];
            for a in &item.affixes {
                if !a.value.is_finite()
                    || a.value < 0.0
                    || a.value > 1.0e6
                    || (a.effect == Effect::BlockChance && a.value > 1.0)
                    || !groups.insert(&a.group)
                    || !a.effect.valid_slot(item.slot)
                    || (a.effect.global() && !a.tags.is_empty())
                    || (a.effect.discrete() && a.value.fract() != 0.0)
                    || (a.effect == Effect::Proc
                        && (a.value > 1.0 || !a.proc.as_ref().is_some_and(ProcRoll::valid)))
                    || (a.effect != Effect::Proc && a.proc.is_some())
                {
                    return Err("invalid affix".into());
                }
                kinds[usize::from(a.kind == AffixKind::Suffix)] += 1;
            }
            let primary = item.affixes.iter().filter(|a| a.effect.primary()).count();
            if item.affix_rules == 1 {
                let (minimum, cap) = item.rarity.affix_limits();
                if !(minimum..=cap * 2).contains(&item.affixes.len())
                    || kinds.iter().any(|&count| count > cap)
                    || item.affixes.iter().any(|a| a.kind != a.effect.affix_kind())
                {
                    return Err("invalid prefix/suffix allocation".into());
                }
            }
            if item.affix_rules == 0
                && primary > 0
                && ((primary != item.rarity.primary_count()
                    // Preserve Uncommon rolls saved before the one-primary allocation.
                    && !(item.rarity == Rarity::Magic && primary == 2))
                    || (primary == 2 && !item.affixes.iter().any(|a| a.effect == Effect::Stamina))
                    || item.affixes.iter().enumerate().any(|(i, a)| {
                        a.effect.primary() && item.affixes[..i].iter().any(|b| b.effect == a.effect)
                    }))
            {
                return Err("invalid primary allocation".into());
            }
            if item.affix_rules == 0
                && !item.affixes.iter().any(|a| a.effect.primary())
                && ((item.rarity == Rarity::Magic && kinds != [1, 1])
                    || (item.rarity == Rarity::Epic && kinds != [2, 2]))
            {
                return Err("invalid affix allocation".into());
            }
        }
        let mut worn = BTreeSet::new();
        for (slot, id) in &save.equipped {
            let kind = SLOTS
                .iter()
                .find(|(name, _)| *name == slot)
                .map(|(_, kind)| kind);
            if !worn.insert(*id)
                || !save.items.iter().any(|i| {
                    i.id == *id
                        && kind.is_some_and(|kind| {
                            i.fits(*kind)
                                || save.titan_grip
                                    && *kind == SlotKind::OffHand
                                    && i.equipment_type == EquipmentType::TwoHand
                        })
                })
            {
                return Err("invalid equipment slot".into());
            }
        }
        save.validate_hands()?;
        legendary::validate_loadout(&save)?;
        Ok(save)
    }
    pub(in crate::survivors) fn collect(&mut self, mut item: Item) -> Result<(), String> {
        let mut save = self.save.clone();
        item.id = save.next_id;
        save.next_id = save
            .next_id
            .checked_add(1)
            .ok_or("Item identity exhausted")?;
        save.items.push(item);
        self.commit(save)
    }
    pub fn equip(&mut self, id: u64, slot: &str) -> Result<(), String> {
        let item = self.item(id).ok_or("Item missing")?;
        if !SLOTS
            .iter()
            .any(|(name, kind)| *name == slot && self.can_equip(item, *kind))
        {
            return Err("This class cannot equip this item in that slot.".into());
        }
        let mut save = self.save.clone();
        if !self.worn(id) {
            Self::replace_at_bag_position(&mut save, id, slot);
        }
        save.equipped.retain(|_, equipped| *equipped != id);
        save.equipped.insert(slot.to_owned(), id);
        Self::clear_off_hand_for_two_hander(&mut save, id, slot);
        self.commit(save)
    }
    pub fn unequip(&mut self, slot: &str) -> Result<(), String> {
        let mut save = self.save.clone();
        save.equipped.remove(slot);
        self.commit(save)
    }
    pub fn delete(&mut self, id: u64) -> Result<(), String> {
        if self.item(id).is_none() {
            return Err("Item missing".into());
        }
        let mut save = self.save.clone();
        save.equipped.retain(|_, equipped| *equipped != id);
        save.items.retain(|item| item.id != id);
        self.commit(save)
    }
    fn replace_at_bag_position(save: &mut Save, incoming: u64, slot: &str) {
        if let Some(outgoing) = save.equipped.get(slot).copied() {
            if let (Some(source), Some(destination)) = (
                save.items.iter().position(|item| item.id == incoming),
                save.items.iter().position(|item| item.id == outgoing),
            ) {
                save.items.swap(source, destination);
            }
            // The item coming off takes the cell of the one going on.
            save.bag_cells = bag_layout(save);
            if let Some(cell) = save.bag_cells.iter().position(|&id| id == incoming) {
                save.bag_cells[cell] = outgoing;
            }
        }
    }
    pub fn quick_equip(&mut self, id: u64) -> Result<(), String> {
        if let Some(slot) = SLOTS
            .iter()
            .find(|(slot, _)| self.equipped(slot).is_some_and(|item| item.id == id))
            .map(|(slot, _)| *slot)
        {
            return self.unequip(slot);
        }
        let item = self.item(id).ok_or("Item missing")?;
        let slot = SLOTS
            .iter()
            .find(|(_, kind)| self.can_equip(item, *kind))
            .ok_or("No compatible slot")?
            .0;
        self.equip(id, slot)
    }
    pub fn drop_on_equipment(&mut self, id: u64, slot: &str) -> Result<(), String> {
        let item = self.item(id).ok_or("Item missing")?;
        if !SLOTS
            .iter()
            .any(|(name, kind)| *name == slot && self.can_equip(item, *kind))
        {
            return Err("This item does not fit that slot.".into());
        }
        let mut save = self.save.clone();
        let source = save
            .equipped
            .iter()
            .find(|(_, worn)| **worn == id)
            .map(|(slot, _)| slot.clone());
        let replaced = save.equipped.get(slot).copied();
        if source.is_none() {
            Self::replace_at_bag_position(&mut save, id, slot);
        }
        save.equipped.retain(|_, worn| *worn != id);
        save.equipped.insert(slot.to_owned(), id);
        if let (Some(source), Some(replaced)) = (source, replaced) {
            if source != slot {
                save.equipped.insert(source, replaced);
            }
        }
        Self::clear_off_hand_for_two_hander(&mut save, id, slot);
        self.commit(save)
    }
    /// Put `id` in bag cell `cell`. A bag item swaps with the cell's occupant; coming off the body
    /// there is no cell to swap with, so the occupant moves to the first free one.
    pub fn drop_in_bag(&mut self, id: u64, cell: usize) -> Result<(), String> {
        if self.item(id).is_none() {
            return Err("Item missing".into());
        }
        let mut save = self.save.clone();
        let mut cells = bag_layout(&save);
        let source = cells.iter().position(|&held| held == id);
        if let Some(source) = source {
            cells[source] = 0;
        }
        save.equipped.retain(|_, worn| *worn != id);
        if cells.len() <= cell {
            cells.resize(cell + 1, 0);
        }
        let occupant = std::mem::replace(&mut cells[cell], id);
        if let (Some(source), true) = (source, occupant != 0) {
            cells[source] = occupant;
        }
        save.bag_cells = cells;
        self.commit(save)
    }
    /// Take a worn item off into the first free cell; a bag item stays where it is.
    pub fn stash(&mut self, id: u64) -> Result<(), String> {
        if self.item(id).is_none() {
            return Err("Item missing".into());
        }
        if !self.worn(id) {
            return Ok(());
        }
        let mut save = self.save.clone();
        save.equipped.retain(|_, worn| *worn != id);
        self.commit(save)
    }
    pub fn apply(&self, run: &mut Run) {
        let signature = (self.character, self.revision);
        if run.equipment_version == Some(signature) {
            return;
        }
        let old_max = run.stats().max_hp;
        run.legendary.weapon_items = self
            .save
            .equipped
            .iter()
            .filter_map(|(slot, id)| match slot.as_str() {
                "Main hand" => Some((*id, 0)),
                "Off hand" => Some((*id, 1)),
                _ => None,
            })
            .collect();
        run.legendary.levels = self
            .save
            .equipped
            .values()
            .filter_map(|id| self.item(*id))
            .filter_map(|item| {
                item.legendary
                    .as_ref()
                    .map(|roll| (roll.unique, item.level))
            })
            .collect();
        run.legendary.equipped = self
            .save
            .equipped
            .iter()
            .filter_map(|(name, id)| {
                let item = self.item(*id)?;
                if !SLOTS
                    .iter()
                    .any(|(slot, kind)| slot == name && self.can_equip(item, *kind))
                {
                    return None;
                }
                item.legendary.clone().map(|roll| (roll.unique, roll))
            })
            .collect();
        legendary::milestones::equip(run);
        let mut modifiers = Vec::new();
        let (mut hp, mut armour) = (0.0, 0.0);
        let mut bonuses = Bonuses::default();
        let mut procs = Vec::new();
        let mut weapons = weapons::Loadout::default();
        let mut uniques = BTreeMap::new();
        for (name, id) in &self.save.equipped {
            let Some(item) = self.item(*id) else { continue };
            if !SLOTS
                .iter()
                .any(|(slot, kind)| slot == name && self.can_equip(item, *kind))
            {
                continue;
            }
            bonuses.block += item.intrinsic_block();
            if let Some(roll) = &item.legendary {
                uniques.entry(roll.unique).or_insert_with(|| roll.clone());
            }
            armour += item.armour();
            weapons.equip(name, item.weapon_damage());
            for (index, a) in item.affixes.iter().enumerate() {
                let mut a = a.clone();
                if weapons::is_weapon(item.equipment_type)
                    && super::talent_runtime::active(run, super::talent_design::Mechanic::Weapon)
                {
                    a.value *= 1.35;
                }
                if a.effect == Effect::Proc {
                    if let Some(roll) = &a.proc {
                        procs.push(combat::EquippedProc {
                            item: *id,
                            index,
                            chance: a.value.min(1.0),
                            filter: Filter::requiring(
                                a.tags.iter().fold(Tags(0), |all, tag| all | tag.tags()),
                            ),
                            roll: roll.clone(),
                        });
                    }
                    continue;
                }
                bonuses.add(a.effect, a.value);
                if let Some(tags) = a.effect.offensive_tags(run.class.id) {
                    modifiers.push(Modifier {
                        source: SOURCE,
                        filter: Filter {
                            all: tags,
                            any: Tags::DAMAGE | Tags::HEAL,
                            ..default()
                        },
                        change: if a.effect == Effect::Intelligence {
                            Change::IncreasedEffect(a.value / 100.0)
                        } else {
                            Change::IncreasedDamage(a.value / 100.0)
                        },
                    });
                }
                match a.effect {
                    Effect::Stamina => hp += a.value,
                    Effect::Life => hp += a.value,
                    Effect::Armour => armour += a.value,
                    _ if a.effect.change(a.value).is_some() => {
                        let tags = a.tags.iter().fold(Tags(0), |all, tag| all | tag.tags());
                        modifiers.push(Modifier {
                            source: SOURCE,
                            filter: Filter::requiring(tags),
                            change: a.effect.change(a.value).unwrap(),
                        });
                    }
                    _ => {}
                }
            }
        }
        run.equipment_hp = hp;
        run.equipment_armour = armour;
        run.weapon_damage = weapons;
        bonuses.block = bonuses.block.clamp(0.0, 0.75);
        run.equipment = bonuses;
        combat::equip(run, procs);
        run.legendary.equipped = uniques;
        legendary::events::equip(run);
        legendary::casting::equip(run);
        legendary::effects::equip(run);
        if (run.legendary.has(46) || run.legendary.has(279))
            && !super::talent_runtime::active(run, super::talent_design::Mechanic::Titans)
        {
            modifiers.push(Modifier {
                source: SOURCE,
                filter: Filter::requiring(Tags::ATTACK),
                change: Change::IncreasedAttackSpeed(0.25),
            });
        }
        run.replace_spell_modifiers(SOURCE, modifiers);
        run.equipment_version = Some(signature);
        let max = run.stats().max_hp;
        run.hp = if run.elapsed == 0.0 && run.hp >= old_max {
            max
        } else {
            run.hp.min(max)
        };
    }
}

pub(super) fn plugin(app: &mut App) {
    visuals::plugin(app);
    combat::plugin(app);
    // Also registered by the sound plugin; registering here keeps headless apps working.
    app.init_resource::<Inventory>()
        .add_message::<crate::sound::CoinSound>()
        .add_systems(
            Update,
            sync.before(super::combat_ui::input)
                .before(super::abilities::cast_abilities),
        )
        .add_systems(
            Update,
            chest_drops
                .after(super::abilities::apply_hits)
                .before(super::enemies::drop_loot)
                .run_if(super::multiplayer::simulating),
        )
        .add_systems(
            Update,
            collect_drops
                .after(chest_drops)
                .before(super::hero::hero_vitals)
                .run_if(
                    |phase: Res<State<Phase>>,
                     role: Option<Res<super::multiplayer::Role>>,
                     selection: Res<super::Selection>| {
                        *phase.get() == Phase::GameOver
                            || (!selection.in_hub
                                && super::multiplayer::runs_in(
                                    *phase.get(),
                                    role.as_deref().copied().unwrap_or_default(),
                                ))
                    },
                ),
        );
}
fn sync(roster: Res<Roster>, mut inventory: ResMut<Inventory>, mut run: ResMut<Run>) {
    inventory.class = roster
        .current()
        .and_then(|c| super::data::CLASSES.get(c.class))
        .map_or(run.class.id, |c| c.id);
    if inventory.character != roster.active {
        inventory.bind(roster.active)
    }
    if roster.current().is_some_and(|c| c.starter_equipment) {
        if let Err(error) = inventory.grant_starter(
            roster
                .current()
                .is_some_and(|c| super::characters::is_test_character(&c.name)),
        ) {
            inventory.notice = error;
        }
    }
    inventory.native_titan_grip = run.talent_build[1][18] > 0 && inventory.class == 1;
    let titan = inventory.native_titan_grip || legendary::titan_grip(&inventory.save);
    if inventory.save.titan_grip != titan {
        let mut save = inventory.save.clone();
        save.titan_grip = titan;
        if !titan {
            if save
                .equipped
                .get("Off hand")
                .and_then(|id| save.items.iter().find(|i| i.id == *id))
                .is_some_and(|i| i.equipment_type == EquipmentType::TwoHand)
            {
                save.equipped.remove("Off hand");
            }
            if save
                .equipped
                .get("Main hand")
                .and_then(|id| save.items.iter().find(|i| i.id == *id))
                .is_some_and(|i| i.equipment_type.two_handed())
            {
                save.equipped.remove("Off hand");
            }
        }
        if let Err(error) = inventory.commit(save) {
            inventory.notice = error;
        }
    }
    inventory.apply(&mut run);
}
#[derive(Component)]
struct ItemDrop(Item);
// Elwynn's small wooden treasure chest: GameObjectDisplayInfo -> Chest01.mdx.
const BOSS_CHEST_DISPLAY: u32 = 10;
pub(in crate::survivors) fn drops_chest(boss: bool, boss_chance: f32, roll: f32) -> bool {
    roll < if boss { boss_chance } else { 1.0 / 2000.0 }
}
pub(in crate::survivors) fn chest_item_level(monster_level: u32, boss: bool) -> u32 {
    // Campaign bosses end at level 59; their chest must unlock the final level-60 bases.
    if boss && monster_level >= MAX_ITEM_LEVEL - 1 {
        MAX_ITEM_LEVEL
    } else {
        monster_level.clamp(1, MAX_ITEM_LEVEL)
    }
}
fn chest_drops(
    mut commands: Commands,
    mut drops: MessageReader<Loot>,
    mut inventory: ResMut<Inventory>,
    mut rng: ResMut<Rng>,
    mut run: ResMut<Run>,
) {
    for drop in drops.read() {
        if inventory.character.is_none()
            || inventory.blocked
            || !drops_chest(drop.boss, inventory.config.boss_drop_chance, rng.f32())
        {
            continue;
        }
        match inventory.config.generate_loot(
            chest_item_level(drop.item_level, drop.boss),
            run.class.id,
            run.equipment.magic_find,
            &mut rng,
        ) {
            Ok(item) => {
                let e = super::units::spawn_object(
                    &mut commands,
                    run.guid(),
                    BOSS_CHEST_DISPLAY,
                    3.0,
                    drop.pos,
                );
                commands.entity(e).insert(ItemDrop(item));
            }
            Err(e) => {
                inventory.notice = e;
                warn!("{}", inventory.notice);
            }
        }
    }
}
fn collect_drops(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    mut inventory: ResMut<Inventory>,
    mut run: ResMut<Run>,
    hero: Query<&Transform, With<Hero>>,
    drops: Query<(Entity, &Transform, &ItemDrop)>,
    mut coin: MessageWriter<crate::sound::CoinSound>,
) {
    let hero = hero.iter().next();
    for (e, tf, drop) in &drops {
        if *phase.get() != Phase::GameOver
            && !hero.is_some_and(|hero| hero.translation.distance(tf.translation) <= 3.0)
        {
            continue;
        }
        match inventory.collect(drop.0.clone()) {
            Ok(()) => {
                run.banner = Some((format!("Boss loot: {}", drop.0.name), 4.0));
                coin.write(crate::sound::CoinSound);
                commands.entity(e).despawn();
            }
            Err(error) => {
                if inventory.notice != error {
                    warn!("Inventory: {error}");
                    inventory.notice = error;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn shared_inventory_applies_client_rolls_without_binding_host_saves() {
        let mut source = Inventory::fixture(vec![item(
            SlotKind::Ring,
            vec![affix(Effect::Life, 12345.0, AffixKind::Prefix, vec![])],
        )]);
        source.save.equipped.insert("Ring 1".into(), 1);
        let encoded = source.shared_save().unwrap();
        let remote = Inventory::from_shared(1, &encoded).unwrap();
        let mut run = Run::default();
        let before = run.stats().max_hp;
        remote.apply(&mut run);
        assert_eq!(remote.character, None);
        assert_eq!(run.stats().max_hp, before + 12345.0);
        assert_eq!(remote.shared_save().unwrap(), encoded);
    }
    use bevy::ecs::system::RunSystemOnce;
    fn inventory() -> Inventory {
        Inventory {
            class: 1,
            character: Some(1),
            save: Save::default(),
            config: Config::builtin(),
            revision: 0,
            notice: String::new(),
            blocked: false,
            native_titan_grip: false,
        }
    }
    pub(super) fn affix(
        effect: Effect,
        value: f32,
        kind: AffixKind,
        tags: Vec<config::Tag>,
    ) -> Affix {
        Affix {
            id: format!("{effect:?}"),
            name: format!("{effect:?}"),
            group: format!("{effect:?}"),
            effect,
            value,
            kind,
            tags,
            proc: None,
        }
    }
    pub(super) fn item(slot: SlotKind, affixes: Vec<Affix>) -> Item {
        Item {
            affix_rules: 0,
            legendary: None,
            base_damage: None,
            base_armour: None,
            appearance: None,
            equipment_type: EquipmentType::Legacy,
            base_block: 0.0,
            loot_class: None,
            id: 0,
            base: "test".into(),
            name: "Test item".into(),
            slot,
            icon: "Interface\\Icons\\INV_Misc_QuestionMark".into(),
            rarity: if affixes.len() == 2 {
                Rarity::Magic
            } else {
                Rarity::Normal
            },
            level: 1,
            affixes,
        }
    }
    #[test]
    fn deletion_is_persisted_and_failed_writes_leave_items_and_equipment_intact() {
        let mut inventory = Inventory::fixture(vec![item(
            SlotKind::Ring,
            vec![affix(Effect::Life, 5.0, AffixKind::Prefix, vec![])],
        )]);
        inventory.equip(1, "Ring 1").unwrap();
        let before = toml::to_string(&inventory.save).unwrap();
        inventory.blocked = true;
        assert!(inventory.delete(1).is_err());
        assert_eq!(toml::to_string(&inventory.save).unwrap(), before);
        inventory.blocked = false;
        inventory.delete(1).unwrap();
        let saved = Inventory::decode(&toml::to_string(&inventory.save).unwrap()).unwrap();
        assert!(saved.items.is_empty());
        assert!(saved.equipped.is_empty());
        assert_eq!(saved.next_id, 2);
        assert!(inventory.delete(1).is_err());
    }
    #[test]
    fn equipping_from_bags_replaces_the_incoming_cell_and_persists_its_order() {
        let mut inventory = Inventory::fixture(
            (0..6)
                .map(|_| {
                    item(
                        SlotKind::Ring,
                        vec![affix(Effect::Life, 5.0, AffixKind::Prefix, vec![])],
                    )
                })
                .collect(),
        );
        inventory.equip(1, "Ring 1").unwrap();
        assert_eq!(
            inventory.bag().map(|i| i.id).collect::<Vec<_>>(),
            [2, 3, 4, 5, 6]
        );
        inventory.quick_equip(5).unwrap();
        assert_eq!(
            inventory.bag().map(|i| i.id).collect::<Vec<_>>(),
            [2, 3, 4, 1, 6]
        );
        assert_eq!(inventory.equipped("Ring 1").unwrap().id, 5);
        inventory.quick_equip(1).unwrap();
        assert_eq!(
            inventory.bag().map(|i| i.id).collect::<Vec<_>>(),
            [2, 3, 4, 5, 6]
        );
        inventory.drop_on_equipment(6, "Ring 1").unwrap();
        assert_eq!(
            inventory.bag().map(|i| i.id).collect::<Vec<_>>(),
            [2, 3, 4, 5, 1]
        );
        inventory.save = Inventory::decode(&toml::to_string(&inventory.save).unwrap()).unwrap();
        assert_eq!(
            inventory.bag().map(|i| i.id).collect::<Vec<_>>(),
            [2, 3, 4, 5, 1]
        );
    }
    #[test]
    fn spell_damage_affix_adds_flat_spell_hits_only_and_roundtrips() {
        let spell_damage = affix(Effect::SpellDamage, 6.0, AffixKind::Prefix, vec![]);
        assert_eq!(spell_damage.description(), "+6 Spell damage");
        let mut inventory = Inventory::fixture(vec![item(SlotKind::Ring, vec![spell_damage])]);
        let mut run = Run::default();
        let fire = super::super::data::ability("fireball");
        let before = run.resolve_spell(fire, 1).dmg;
        inventory.equip(1, "Ring 1").unwrap();
        inventory.apply(&mut run);
        assert_eq!(run.resolve_spell(fire, 1).dmg, before + 6.0);
        for key in [
            "heroic_strike",
            "arcane_shot",
            "rejuvenation",
            "power_word_shield",
            "corruption",
        ] {
            let def = super::super::data::ability(key);
            assert_eq!(
                run.resolve_spell(def, 1),
                Run::default().resolve_spell(def, 1),
                "{key}"
            );
        }
        run.replace_spell_modifiers(
            123,
            vec![Modifier {
                source: 123,
                filter: Filter::requiring(Tags::SPELL),
                change: Change::IncreasedDamage(0.5),
            }],
        );
        assert_eq!(run.resolve_spell(fire, 1).dmg, (before + 6.0) * 1.5);
        let saved = toml::to_string(&inventory.save).unwrap();
        assert_eq!(
            Inventory::decode(&saved).unwrap().items[0].affixes[0].effect,
            Effect::SpellDamage
        );
        inventory.unequip("Ring 1").unwrap();
        inventory.apply(&mut run);
        assert_eq!(run.resolve_spell(fire, 1).dmg, before * 1.5);
        let mut rng = Rng(123);
        let config = Config::builtin();
        assert!((0..1000).any(|_| config
            .generate_for_class(10, 8, &mut rng)
            .unwrap()
            .affixes
            .iter()
            .any(|a| a.effect == Effect::SpellDamage)));
    }
    #[test]
    fn primary_stats_scale_only_the_requested_classes_and_spell_types() {
        for class in super::super::data::CLASSES {
            for effect in [
                Effect::Stamina,
                Effect::Strength,
                Effect::Agility,
                Effect::Intelligence,
            ] {
                let mut inv = Inventory::fixture(vec![item(
                    SlotKind::Head,
                    vec![affix(effect, 20.0, AffixKind::Prefix, vec![])],
                )]);
                inv.equip(1, "Head").unwrap();
                let mut run = Run { class, ..default() };
                let hp = run.stats().max_hp;
                let attack = super::super::data::ability("heroic_strike");
                let spell = super::super::data::ability("fireball");
                let heal = super::super::data::ABILITIES
                    .iter()
                    .find(|a| a.kind == super::super::data::Kind::Heal)
                    .unwrap();
                let before = [attack, spell, heal].map(|a| run.resolve_spell(a, 1).dmg);
                let shield = super::super::data::ability("power_word_shield");
                let shield_before = run.resolve_spell(shield, 1);
                inv.apply(&mut run);
                assert_eq!(run.resolve_spell(shield, 1), shield_before);
                assert_eq!(
                    run.stats().max_hp,
                    hp + if effect == Effect::Stamina { 20.0 } else { 0.0 }
                );
                let attacks = (effect == Effect::Strength && matches!(class.id, 1 | 2))
                    || (effect == Effect::Agility && matches!(class.id, 3 | 4 | 7 | 11));
                let spells =
                    effect == Effect::Intelligence && matches!(class.id, 2 | 5 | 7 | 8 | 9 | 11);
                for (index, def) in [attack, spell, heal].into_iter().enumerate() {
                    let scale = if (index == 0 && attacks) || (index > 0 && spells) {
                        1.2
                    } else {
                        1.0
                    };
                    assert!(
                        (run.resolve_spell(def, 1).dmg - before[index] * scale).abs() < 0.001,
                        "{} {effect:?} {}",
                        class.name,
                        def.key
                    );
                }
                inv.unequip("Head").unwrap();
                inv.apply(&mut run);
                assert_eq!(run.stats().max_hp, hp);
                assert_eq!(run.resolve_spell(spell, 1).dmg, before[1]);
            }
        }
    }
    #[test]
    fn global_affixes_update_real_stats_healing_and_incoming_damage() {
        let mods = [
            (SlotKind::Head, "Head", Effect::Experience, 0.1),
            (SlotKind::Feet, "Feet", Effect::MovementSpeed, 0.1),
            (SlotKind::Chest, "Chest", Effect::Regeneration, 2.0),
            (
                SlotKind::Shoulders,
                "Shoulders",
                Effect::DamageReduction,
                0.25,
            ),
            (SlotKind::Neck, "Neck", Effect::HealingEffectiveness, 0.5),
            (SlotKind::Waist, "Waist", Effect::PickupRadius, 0.2),
            (SlotKind::MainHand, "Main hand", Effect::LifeOnKill, 5.0),
            (SlotKind::Ring, "Ring 1", Effect::Luck, 0.1),
            (SlotKind::Ring, "Ring 2", Effect::MagicFind, 0.2),
        ];
        let mut inventory = Inventory::fixture(
            mods.iter()
                .map(|(slot, _, effect, value)| {
                    item(
                        *slot,
                        vec![affix(*effect, *value, AffixKind::Prefix, vec![])],
                    )
                })
                .collect(),
        );
        for (index, (_, slot, _, _)) in mods.iter().enumerate() {
            inventory.equip(index as u64 + 1, slot).unwrap();
        }
        let mut run = Run {
            elapsed: 1.0,
            hp: 100.0,
            ..default()
        };
        let base = run.stats();
        inventory.apply(&mut run);
        assert!((run.stats().speed - base.speed * 1.1).abs() < 0.001);
        assert!((run.stats().magnet - base.magnet * 1.2).abs() < 0.001);
        assert_eq!(run.stats().regen, 2.0);
        assert_eq!(run.stats().growth, 1.1);
        assert_eq!(run.equipment.luck, 0.1);
        assert_eq!(run.equipment.magic_find, 0.2);
        assert_eq!(run.equipment.life_on_kill, 5.0);
        assert_eq!(super::super::hero::hurt_hero(&mut run, 40.0), 30.0);
        assert_eq!(run.heal(10.0), 15.0);
        assert_eq!(run.hp, 85.0);
        for (_, slot, _, _) in mods {
            inventory.unequip(slot).unwrap();
        }
        inventory.apply(&mut run);
        assert_eq!(run.stats().regen, base.regen);
        assert_eq!(run.equipment.luck, 0.0);
        assert_eq!(run.equipment.magic_find, 0.0);
        assert_eq!(run.heal(10.0), 10.0);
    }
    #[test]
    fn right_click_uses_first_slot_and_drag_swaps_equipment_and_bag_order_atomically() {
        let ring = || {
            item(
                SlotKind::Ring,
                vec![affix(Effect::Life, 10.0, AffixKind::Prefix, vec![])],
            )
        };
        let mut inventory = Inventory::fixture(vec![ring(), ring(), ring()]);
        inventory.quick_equip(1).unwrap();
        inventory.quick_equip(2).unwrap();
        assert_eq!(inventory.equipped("Ring 1").unwrap().id, 2);
        assert!(inventory.equipped("Ring 2").is_none());
        inventory.equip(1, "Ring 2").unwrap();
        inventory.quick_equip(1).unwrap();
        assert!(!inventory.worn(1));
        assert!(inventory.equipped("Ring 2").is_none());
        inventory.drop_on_equipment(1, "Ring 2").unwrap();
        inventory.drop_on_equipment(2, "Ring 2").unwrap();
        assert_eq!(inventory.equipped("Ring 1").unwrap().id, 1);
        assert_eq!(inventory.equipped("Ring 2").unwrap().id, 2);
        let before = toml::to_string(&inventory.save).unwrap();
        assert!(inventory.drop_on_equipment(2, "Head").is_err());
        assert_eq!(toml::to_string(&inventory.save).unwrap(), before);
        inventory.drop_in_bag(2, 0).unwrap();
        assert!(!inventory.worn(2));
        assert_eq!(
            inventory.bag().map(|i| i.id).collect::<Vec<_>>(),
            vec![2, 3]
        );
        inventory.drop_in_bag(2, 1).unwrap();
        assert_eq!(
            inventory.bag().map(|i| i.id).collect::<Vec<_>>(),
            vec![3, 2]
        );
        inventory.blocked = true;
        let before = toml::to_string(&inventory.save).unwrap();
        assert!(inventory.drop_in_bag(1, 0).is_err());
        assert_eq!(toml::to_string(&inventory.save).unwrap(), before);
        assert_eq!(Inventory::decode(&before).unwrap().items.len(), 3);
    }
    #[test]
    fn bag_items_stay_in_the_cell_they_are_dropped_in() {
        let ring = || {
            item(
                SlotKind::Ring,
                vec![affix(Effect::Life, 10.0, AffixKind::Prefix, vec![])],
            )
        };
        let mut inventory = Inventory::fixture(vec![ring(), ring(), ring()]);
        let cells = |inv: &Inventory| {
            inv.bag_cells()
                .iter()
                .map(|cell| cell.map_or(0, |item| item.id))
                .collect::<Vec<_>>()
        };
        assert_eq!(cells(&inventory), [1, 2, 3]);
        // An empty cell far away: the others do not close the gap.
        inventory.drop_in_bag(1, 20).unwrap();
        let mut expected = vec![0; 21];
        expected[1] = 2;
        expected[2] = 3;
        expected[20] = 1;
        assert_eq!(cells(&inventory), expected);
        // The layout survives a reload.
        inventory.save = Inventory::decode(&toml::to_string(&inventory.save).unwrap()).unwrap();
        assert_eq!(cells(&inventory), expected);
        // Onto an occupied cell: the two swap.
        inventory.drop_in_bag(1, 1).unwrap();
        assert_eq!(cells(&inventory)[..3], [0, 1, 3]);
        assert_eq!(cells(&inventory)[20], 2);
        // Equipping leaves a hole; a new item takes the first free cell.
        inventory.equip(3, "Ring 1").unwrap();
        assert_eq!(cells(&inventory)[..3], [0, 1, 0]);
        inventory.collect(ring()).unwrap();
        assert_eq!(cells(&inventory)[..3], [4, 1, 0]);
        // Off the body onto an occupied cell: the occupant moves to the first free cell.
        inventory.drop_in_bag(3, 0).unwrap();
        assert_eq!(cells(&inventory)[..3], [3, 1, 4]);
        // The grid gap only takes a worn item off, and moves nothing else.
        let before = cells(&inventory);
        inventory.stash(1).unwrap();
        assert_eq!(cells(&inventory), before);
        // Deleting empties the cell without moving anything.
        inventory.delete(1).unwrap();
        assert_eq!(cells(&inventory)[..3], [3, 0, 4]);
    }
    #[test]
    fn equipment_replaces_sources_and_uses_spell_tags_and_weapon_attack_speed() {
        let mut inventory = inventory();
        for item in [
            item(
                SlotKind::Head,
                vec![affix(Effect::Life, 40.0, AffixKind::Prefix, vec![])],
            ),
            item(
                SlotKind::Chest,
                vec![affix(Effect::Armour, 7.0, AffixKind::Prefix, vec![])],
            ),
            item(
                SlotKind::Ring,
                vec![affix(
                    Effect::Damage,
                    0.3,
                    AffixKind::Prefix,
                    vec![config::Tag::Fire],
                )],
            ),
            item(
                SlotKind::MainHand,
                vec![
                    affix(
                        Effect::FlatDamage,
                        5.0,
                        AffixKind::Prefix,
                        vec![config::Tag::Attack],
                    ),
                    affix(
                        Effect::AttackSpeed,
                        0.25,
                        AffixKind::Suffix,
                        vec![config::Tag::Attack],
                    ),
                ],
            ),
        ] {
            inventory.collect(item).unwrap()
        }
        for (id, slot) in [(1, "Head"), (2, "Chest"), (3, "Ring 1"), (4, "Main hand")] {
            inventory.equip(id, slot).unwrap()
        }
        let mut run = Run {
            elapsed: 1.0,
            ..default()
        };
        let before = run.stats();
        let fire = super::super::data::ability("fireball");
        let melee = super::super::data::ability("heroic_strike");
        let fire_before = run.resolve_spell(fire, 1);
        let melee_before = run.resolve_spell(melee, 1);
        inventory.apply(&mut run);
        assert_eq!(run.stats().max_hp, before.max_hp + 40.0);
        assert_eq!(run.stats().armor, 7.0);
        assert!((run.resolve_spell(fire, 1).dmg - fire_before.dmg * 1.3).abs() < 0.001);
        assert_eq!(run.resolve_spell(fire, 1).cooldown, fire_before.cooldown);
        assert_eq!(run.resolve_spell(melee, 1).dmg, melee_before.dmg + 5.0);
        assert!(
            (run.resolve_spell(melee, 1).cooldown - melee_before.cooldown / 1.25).abs() < 0.001
        );
        let count = run.spell_modifiers.entries().len();
        run.hp = 90.0;
        inventory.apply(&mut run);
        assert_eq!(run.hp, 90.0);
        assert_eq!(run.spell_modifiers.entries().len(), count);
        inventory.unequip("Main hand").unwrap();
        inventory.apply(&mut run);
        assert_eq!(run.resolve_spell(melee, 1).dmg, melee_before.dmg);
        assert_eq!(run.resolve_spell(melee, 1).cooldown, melee_before.cooldown);
        inventory.unequip("Head").unwrap();
        inventory.unequip("Chest").unwrap();
        inventory.unequip("Ring 1").unwrap();
        inventory.apply(&mut run);
        assert_eq!(run.stats().max_hp, before.max_hp);
        assert_eq!(run.stats().armor, 0.0);
        assert_eq!(run.resolve_spell(fire, 1), fire_before);
    }
    #[test]
    fn rings_swap_without_duplication_and_saved_rolls_survive_roundtrip() {
        let mut inventory = inventory();
        for _ in 0..3 {
            inventory
                .collect(item(
                    SlotKind::Ring,
                    vec![affix(Effect::Life, 20.0, AffixKind::Prefix, vec![])],
                ))
                .unwrap()
        }
        inventory.equip(1, "Ring 1").unwrap();
        inventory.equip(2, "Ring 2").unwrap();
        assert!(inventory.equip(3, "Head").is_err());
        inventory.equip(3, "Ring 1").unwrap();
        assert!(!inventory.worn(1));
        inventory.equip(3, "Ring 2").unwrap();
        assert!(inventory.equipped("Ring 1").is_none());
        assert_eq!(inventory.equipped("Ring 2").unwrap().id, 3);
        assert_eq!(inventory.bag().count(), 2);
        let text = toml::to_string_pretty(&inventory.save).unwrap();
        let save = Inventory::decode(&text).unwrap();
        assert_eq!(save.items.len(), 3);
        assert_eq!(save.items[0].affixes[0].value, 20.0);
        assert_eq!(save.equipped.get("Ring 2"), Some(&3));
        let mut bad = save.clone();
        bad.equipped.insert("Head".into(), 3);
        assert!(Inventory::decode(&toml::to_string(&bad).unwrap()).is_err());
        inventory.bind(Some(2));
        assert!(inventory.all().is_empty());
        assert!(inventory.save.equipped.is_empty());
    }
    #[test]
    fn invalid_saves_and_failed_writes_do_not_destroy_items() {
        let mut inventory = inventory();
        let mut rng = Rng(123);
        inventory
            .collect(item(
                SlotKind::Chest,
                vec![affix(Effect::Stamina, 12.0, AffixKind::Suffix, vec![])],
            ))
            .unwrap();
        inventory.blocked = true;
        let slot = SLOTS
            .iter()
            .find(|(_, kind)| *kind == inventory.item(1).unwrap().slot)
            .unwrap()
            .0;
        assert!(inventory.equip(1, slot).is_err());
        assert_eq!(inventory.bag().count(), 1);
        assert!(inventory
            .collect(inventory.config.generate(10, &mut rng).unwrap())
            .is_err());
        assert_eq!(inventory.all().len(), 1);
        let mut save = inventory.save.clone();
        save.items[0].affixes[0].value = -1.0;
        assert!(Inventory::decode(&toml::to_string(&save).unwrap()).is_err());
    }
    #[test]
    fn boss_chests_and_pickups_are_saved_once_with_final_loot_recovered() {
        let mut world = World::new();
        world.insert_resource(inventory());
        world.insert_resource(Run::default());
        world.insert_resource(Rng(123));
        world.init_resource::<Messages<Loot>>();
        world.init_resource::<Messages<crate::sound::CoinSound>>();
        world.resource_mut::<Messages<Loot>>().write(Loot {
            item_level: 1,
            pos: Vec3::ZERO,
            xp: 1,
            boss: false,
        });
        world.resource_mut::<Messages<Loot>>().write(Loot {
            item_level: 9,
            pos: Vec3::X * 100.0,
            xp: 1,
            boss: true,
        });
        world.run_system_once(chest_drops).unwrap();
        assert_eq!(world.query::<&ItemDrop>().iter(&world).count(), 1);
        let (drop_entity, position, model) = world
            .query_filtered::<(Entity, &Transform, &crate::net::NetEntity), With<ItemDrop>>()
            .single(&world)
            .map(|(e, tf, model)| (e, tf.translation, model.display_id))
            .unwrap();
        assert_eq!(model, Some(BOSS_CHEST_DISPLAY));
        assert_eq!(position, Vec3::X * 100.0);
        world.insert_resource(State::new(Phase::Playing));
        let hero = world
            .spawn((Hero { vy: 0.0, air: 0.0 }, Transform::default()))
            .id();
        world.run_system_once(collect_drops).unwrap();
        assert!(world.resource::<Inventory>().all().is_empty());
        world.get_mut::<Transform>(hero).unwrap().translation = position;
        world.run_system_once(collect_drops).unwrap();
        assert_eq!(world.resource::<Inventory>().all().len(), 1);
        assert!(world.get_entity(drop_entity).is_err());
        // A later chest is recovered at the results screen even if the hero is gone.
        world.spawn((
            Transform::from_xyz(500.0, 0.0, 0.0),
            ItemDrop(item(
                SlotKind::Head,
                vec![affix(Effect::Stamina, 1.0, AffixKind::Prefix, vec![])],
            )),
        ));
        world.despawn(hero);
        world.insert_resource(State::new(Phase::GameOver));
        world.run_system_once(collect_drops).unwrap();
        world.run_system_once(collect_drops).unwrap();
        assert_eq!(world.resource::<Inventory>().all().len(), 2);
        assert_eq!(world.query::<&ItemDrop>().iter(&world).count(), 0);
    }
    #[test]
    fn ordinary_monster_chests_have_one_in_two_thousand_chance_without_changing_boss_chance() {
        assert!(drops_chest(false, 0.0, 0.0));
        assert!(drops_chest(false, 0.0, 0.000499));
        assert!(!drops_chest(false, 1.0, 0.0005));
        assert!(!drops_chest(false, 1.0, 0.5));
        assert!(!drops_chest(true, 0.0, 0.0));
        assert!(drops_chest(true, 1.0, 0.999));
        let mut rng = Rng(742);
        let count = (0..100000)
            .filter(|_| drops_chest(false, 1.0, rng.f32()))
            .count();
        assert!((25..=80).contains(&count), "{count}/100000");
    }
    #[test]
    fn final_boss_chests_reach_the_last_base_tier_at_level_sixty() {
        assert_eq!(chest_item_level(59, false), 59);
        assert_eq!(chest_item_level(59, true), 60);
        assert_eq!(chest_item_level(100, true), 60);
        assert_eq!(chest_item_level(100, false), 60);
        let mut inventory = inventory();
        inventory.config.boss_drop_chance = 1.0;
        for base in &mut inventory.config.bases {
            base.weight = if base.id == "sword" { 1.0 } else { 0.0 };
        }
        let mut world = World::new();
        world.insert_resource(inventory);
        world.insert_resource(Run::default());
        world.insert_resource(Rng(123));
        world.init_resource::<Messages<Loot>>();
        world.write_message(Loot {
            item_level: 59,
            pos: Vec3::ZERO,
            xp: 1,
            boss: true,
        });
        world.run_system_once(chest_drops).unwrap();
        let item = &world.query::<&ItemDrop>().single(&world).unwrap().0;
        assert_eq!(item.level, 60);
        assert_eq!(item.base, "sword");
        let tier = world
            .resource::<Inventory>()
            .config
            .bases
            .iter()
            .find(|b| b.id == "sword")
            .unwrap()
            .tiers
            .last()
            .unwrap();
        assert_eq!(item.appearance, tier.appearance);
        assert_eq!(item.icon, tier.icon);
        assert_eq!(
            item.weapon_damage().min,
            tier.base_damage.unwrap().min.round()
        );
    }
    #[test]
    fn ordinary_monster_chests_use_the_boss_model_and_monster_item_level() {
        let seed = (1..100000)
            .find(|seed| drops_chest(false, 0.0, Rng(*seed).f32()))
            .unwrap();
        let mut world = World::new();
        let mut inventory = inventory();
        inventory.config.boss_drop_chance = 0.0;
        world.insert_resource(inventory);
        world.insert_resource(Run::default());
        world.insert_resource(Rng(seed));
        world.init_resource::<Messages<Loot>>();
        world.write_message(Loot {
            item_level: 35,
            pos: Vec3::X * 12.0,
            xp: 1,
            boss: false,
        });
        world.run_system_once(chest_drops).unwrap();
        let (drop, transform, model) = world
            .query::<(&ItemDrop, &Transform, &crate::net::NetEntity)>()
            .single(&world)
            .unwrap();
        assert_eq!(drop.0.level, 35);
        assert_eq!(transform.translation, Vec3::X * 12.0);
        assert_eq!(model.display_id, Some(BOSS_CHEST_DISPLAY));
    }
}
