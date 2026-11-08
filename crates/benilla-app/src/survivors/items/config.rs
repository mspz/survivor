pub(super) mod crafting;
use super::*;
pub(super) const DEFAULT_BASES: &str =
    include_str!("../../../../../config/survivors-item-bases.toml");
const DEFAULT_AFFIXES: &str = include_str!("../../../../../config/survivors-item-affixes.toml");
#[cfg(test)]
const DEFAULT_FILE: &str = concat!(
    include_str!("../../../../../config/survivors-item-bases.toml"),
    "\n",
    include_str!("../../../../../config/survivors-item-affixes.toml")
);
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::survivors) enum Tag {
    Attack,
    Spell,
    Melee,
    Projectile,
    Area,
    Chain,
    Dot,
    Duration,
    Totem,
    Minion,
    Trap,
    Damage,
    Physical,
    Fire,
    Cold,
    Lightning,
    Nature,
    Arcane,
    Holy,
    Shadow,
}
impl Tag {
    pub fn tags(self) -> Tags {
        match self {
            Self::Attack => Tags::ATTACK,
            Self::Spell => Tags::SPELL,
            Self::Melee => Tags::MELEE,
            Self::Projectile => Tags::PROJECTILE,
            Self::Area => Tags::AREA,
            Self::Chain => Tags::CHAIN,
            Self::Dot => Tags::DOT,
            Self::Duration => Tags::DURATION,
            Self::Totem => Tags::TOTEM,
            Self::Minion => Tags::MINION,
            Self::Trap => Tags::TRAP,
            Self::Damage => Tags::DAMAGE,
            Self::Physical => Tags::PHYSICAL,
            Self::Fire => Tags::FIRE,
            Self::Cold => Tags::COLD,
            Self::Lightning => Tags::LIGHTNING,
            Self::Nature => Tags::NATURE,
            Self::Arcane => Tags::ARCANE,
            Self::Holy => Tags::HOLY,
            Self::Shadow => Tags::SHADOW,
        }
    }
    pub fn label(self) -> String {
        self.tags().labels()
    }
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Base {
    /// Level variants share this family's drop weight and affix eligibility.
    #[serde(default)]
    pub tiers: Vec<BaseTier>,
    #[serde(default)]
    pub base_damage: Option<weapons::DamageRange>,
    #[serde(default)]
    pub base_armour: Option<f32>,
    #[serde(default)]
    pub appearance: Option<Appearance>,
    #[serde(default)]
    pub equipment_type: EquipmentType,
    #[serde(default)]
    pub base_block: f32,
    #[serde(default)]
    pub base_block_max: Option<f32>,
    pub id: String,
    pub name: String,
    pub slot: SlotKind,
    pub icon: String,
    pub weight: f64,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BaseTier {
    pub level: u32,
    pub name: String,
    pub icon: String,
    pub appearance: Option<Appearance>,
    pub base_damage: Option<weapons::DamageRange>,
    pub base_armour: Option<f32>,
}
impl Base {
    pub(super) fn tier(&self, level: u32) -> Option<&BaseTier> {
        self.tiers
            .iter()
            .rev()
            .find(|t| t.level <= level.clamp(1, MAX_ITEM_LEVEL))
    }
    fn damage_at(&self, level: u32) -> weapons::DamageRange {
        let level = level.clamp(1, MAX_ITEM_LEVEL);
        let tier = self.tier(level);
        let damage = tier
            .and_then(|t| t.base_damage)
            .or(self.base_damage)
            .unwrap_or_else(|| weapons::base_for(self.equipment_type, self.appearance));
        let divisor = tier
            .filter(|t| t.base_damage.is_some())
            .map_or(1.0, |t| item_level::factor(t.level));
        let factor = item_level::factor(level) / divisor;
        weapons::DamageRange {
            min: (damage.min * factor).round().min(1.0e6),
            max: (damage.max * factor).round().min(1.0e6),
        }
    }
    fn armour_at(&self, level: u32) -> f32 {
        let level = level.clamp(1, MAX_ITEM_LEVEL);
        let tier = self.tier(level);
        let armour = tier
            .and_then(|t| t.base_armour)
            .or(self.base_armour)
            .unwrap_or_else(|| armour::base_for(self.equipment_type, self.slot));
        let divisor = tier
            .filter(|t| t.base_armour.is_some())
            .map_or(1.0, |t| item_level::factor(t.level));
        (armour * item_level::factor(level) / divisor)
            .round()
            .min(1.0e6)
    }
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct AffixDef {
    id: String,
    name: String,
    kind: AffixKind,
    group: String,
    slots: Vec<SlotKind>,
    effect: Effect,
    min: f32,
    max: f32,
    weight: f64,
    #[serde(default)]
    tags: Vec<Tag>,
    #[serde(default)]
    proc: Option<ProcDef>,
}
impl AffixDef {
    fn compatible_base(&self, base: &Base) -> bool {
        !matches!(self.effect, Effect::Experience | Effect::MagicFind | Effect::Luck)
            && self.slots.contains(&base.slot)
            && self.effect.valid_equipment_type(base.equipment_type)
            && !(matches!(
                base.equipment_type,
                EquipmentType::Wand | EquipmentType::Staff
            ) && (matches!(
                self.effect,
                Effect::FlatDamage | Effect::AttackSpeed | Effect::LifeLeech
            ) || self.tags.iter().any(|t| matches!(t, Tag::Attack))))
            && !(matches!(
                base.equipment_type,
                EquipmentType::Bow | EquipmentType::MeleeOneHand | EquipmentType::TwoHand
            ) && self.tags.iter().any(|t| matches!(t, Tag::Spell)))
    }
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProcDef {
    kind: ProcKind,
    cooldown: f32,
    duration: f32,
    power: f32,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RarityWeights {
    normal: f64,
    magic: f64,
    epic: f64,
    legendary: f64,
}
#[derive(Clone, Resource, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Config {
    version: u32,
    pub boss_drop_chance: f32,
    rarities: RarityWeights,
    pub(super) bases: Vec<Base>,
    affixes: Vec<AffixDef>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BaseTable {
    version: u32,
    boss_drop_chance: f32,
    rarities: RarityWeights,
    bases: Vec<Base>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AffixTable {
    affixes: Vec<AffixDef>,
}
impl Config {
    pub fn builtin() -> Self {
        Self::parse_parts(DEFAULT_BASES, DEFAULT_AFFIXES).expect("valid built-in item tables")
    }
    fn parse_parts(bases: &str, affixes: &str) -> Result<Self, String> {
        let bases: BaseTable = toml::from_str(bases).map_err(|e| format!("Item bases: {e}"))?;
        let affixes: AffixTable =
            toml::from_str(affixes).map_err(|e| format!("Item affixes: {e}"))?;
        Self {
            version: bases.version,
            boss_drop_chance: bases.boss_drop_chance,
            rarities: bases.rarities,
            bases: bases.bases,
            affixes: affixes.affixes,
        }
        .validate()
    }
    pub fn parse(text: &str) -> Result<Self, String> {
        let c: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        c.validate()
    }
    fn validate(self) -> Result<Self, String> {
        let mut c = self;
        // Existing local tables predate appearance metadata; retain their loot settings.
        let defaults: BaseTable = toml::from_str(DEFAULT_BASES).map_err(|e| e.to_string())?;
        for base in &mut c.bases {
            if base.appearance.is_none() {
                base.appearance = defaults
                    .bases
                    .iter()
                    .find(|b| b.id == base.id)
                    .or_else(|| {
                        defaults.bases.iter().find(|b| {
                            b.slot == base.slot && b.equipment_type == base.equipment_type
                        })
                    })
                    .and_then(|b| b.appearance);
            }
        }
        let valid_weight = |v: f64| v.is_finite() && v >= 0.0;
        if c.version != 1
            || !c.boss_drop_chance.is_finite()
            || !(0.0..=1.0).contains(&c.boss_drop_chance)
            || !valid_weight(c.rarities.legendary)
            || !c.weights().iter().copied().all(valid_weight)
            || !(c.weights().iter().any(|w| *w > 0.0) || c.rarities.legendary > 0.0)
            || c.bases.is_empty()
            || c.affixes.is_empty()
        {
            return Err("Invalid version, drop chance or rarity weights.".into());
        }
        let mut ids = BTreeSet::new();
        let mut groups = BTreeMap::new();
        for a in &c.affixes {
            if a.id.is_empty()
                || a.name.is_empty()
                || a.group.is_empty()
                || a.kind != a.effect.affix_kind()
                || !ids.insert(&a.id)
                || a.slots.is_empty()
                || !valid_weight(a.weight)
                || !a.min.is_finite()
                || !a.max.is_finite()
                || a.min < 0.0
                || a.max < a.min
                || a.max > 1.0e6
                || (a.effect == Effect::BlockChance && a.max > 1.0)
                || (a.effect.discrete() && (a.min.fract() != 0.0 || a.max.fract() != 0.0))
                || a.slots.iter().any(|slot| !a.effect.valid_slot(*slot))
                || (a.effect.global() && !a.tags.is_empty())
                || (a.effect == Effect::Proc
                    && (a.max > 1.0
                        || !a.proc.as_ref().is_some_and(|p| {
                            p.cooldown.is_finite()
                                && (0.2..=300.0).contains(&p.cooldown)
                                && p.duration.is_finite()
                                && (0.0..=120.0).contains(&p.duration)
                                && p.power.is_finite()
                                && (0.001..=10.0).contains(&p.power)
                                && (matches!(p.kind, ProcKind::ClassSpell) || p.duration > 0.0)
                        })))
                || (a.effect != Effect::Proc && a.proc.is_some())
            {
                return Err(format!("Invalid affix {}", a.id));
            }
            if let Some(allocation) = groups.insert(&a.group, (a.kind, a.effect.primary())) {
                if allocation != (a.kind, a.effect.primary()) {
                    return Err(format!(
                        "Affix group {} mixes naming kinds or primary and secondary stats",
                        a.group
                    ));
                }
            }
        }
        ids.clear();
        for b in &c.bases {
            let mut previous = 1;
            for tier in &b.tiers {
                if tier.level <= previous
                    || tier.level > MAX_ITEM_LEVEL
                    || tier.name.is_empty()
                    || tier.icon.is_empty()
                    || tier.appearance.is_some_and(|a| {
                        !a.valid_for(b.slot)
                            || b.appearance.is_some_and(|base| {
                                a.item_class != base.item_class
                                    || a.subclass != base.subclass
                                    || a.inventory_type != base.inventory_type
                            })
                    })
                    || tier
                        .base_damage
                        .is_some_and(|d| !d.valid() || !weapons::is_weapon(b.equipment_type))
                    || tier
                        .base_armour
                        .is_some_and(|a| !a.is_finite() || !(0.0..=1.0e6).contains(&a))
                {
                    return Err(format!("Invalid level tier {} for {}", tier.level, b.id));
                }
                previous = tier.level;
            }
            if b.id.is_empty()
                || b.name.is_empty()
                || b.icon.is_empty()
                || b.base_damage
                    .is_some_and(|range| !range.valid() || !weapons::is_weapon(b.equipment_type))
                || b.base_armour
                    .is_some_and(|v| !v.is_finite() || !(0.0..=1.0e6).contains(&v))
                || b.appearance.is_some_and(|a| !a.valid_for(b.slot))
                || b.equipment_type == EquipmentType::Legacy
                || !b.equipment_type.valid_slot(b.slot)
                || !b.base_block.is_finite()
                || !(0.0..=MAX_BASE_BLOCK).contains(&b.base_block)
                || b.base_block_max.is_some_and(|max| {
                    !max.is_finite()
                        || max < b.base_block
                        || max > MAX_BASE_BLOCK
                        || b.equipment_type != EquipmentType::Shield
                })
                || (b.equipment_type != EquipmentType::Shield && b.base_block != 0.0)
                || !valid_weight(b.weight)
                || !ids.insert(&b.id)
            {
                return Err(format!("Invalid base {}", b.id));
            }
            if b.weight == 0.0 {
                continue;
            }
            // Enabled rarities must have enough distinct groups for both budgets, for every class.
            let required = if c.rarities.epic > 0.0 || c.rarities.legendary > 0.0 {
                3
            } else if c.rarities.magic > 0.0 {
                2
            } else {
                1
            };
            for class in super::super::data::CLASSES
                .iter()
                .filter(|c| b.equipment_type.can_drop(c.id))
            {
                for kind in [AffixKind::Prefix, AffixKind::Suffix] {
                    let count = c
                        .affixes
                        .iter()
                        .filter(|a| {
                            a.weight > 0.0
                                && a.kind == kind
                                && a.compatible_base(b)
                                && (!a.effect.primary()
                                    || b.equipment_type.drop_primary_allowed(class.id, a.effect))
                        })
                        .map(|a| &a.group)
                        .collect::<BTreeSet<_>>()
                        .len();
                    if count < required {
                        return Err(format!(
                            "{} needs {required} compatible {kind:?} groups for {}",
                            b.id, class.name
                        ));
                    }
                }
            }
        }
        if !c.bases.iter().any(|b| b.weight > 0.0) {
            return Err("No enabled item bases".into());
        }
        for class in super::super::data::CLASSES {
            if c.rarities.legendary > 0.0
                && !legendary::DEFINITIONS.iter().any(|d| {
                    legendary::eligible(d.id, class.id)
                        && c.bases.iter().any(|base| {
                            base.weight > 0.0
                                && base.slot == d.slot
                                && base.equipment_type.can_drop(class.id)
                                && legendary::base_allowed(d.id, class.id, base.equipment_type)
                        })
                })
            {
                return Err(format!("No compatible legendary base for {}", class.name));
            }
            if !c
                .bases
                .iter()
                .any(|b| b.weight > 0.0 && b.equipment_type.can_drop(class.id))
            {
                return Err(format!("No compatible base for {}", class.name));
            }
        }
        Ok(c)
    }
    fn weights(&self) -> [f64; 3] {
        [
            self.rarities.normal,
            self.rarities.magic,
            self.rarities.epic,
        ]
    }
    #[cfg(test)]
    pub fn generate(&self, level: u32, rng: &mut Rng) -> Result<Item, String> {
        self.generate_for_class(level, 1, rng)
    }
    #[cfg(test)]
    pub fn generate_for_class(
        &self,
        level: u32,
        class_id: u8,
        rng: &mut Rng,
    ) -> Result<Item, String> {
        self.generate_loot(level, class_id, 0.0, rng)
    }
    pub fn generate_loot(
        &self,
        level: u32,
        class_id: u8,
        magic_find: f32,
        rng: &mut Rng,
    ) -> Result<Item, String> {
        if !super::super::data::CLASSES.iter().any(|c| c.id == class_id) {
            return Err("Unknown loot class".into());
        }
        let target = if rng.f32() < 0.8 {
            class_id
        } else {
            let others: Vec<_> = super::super::data::CLASSES
                .iter()
                .filter(|c| c.id != class_id)
                .collect();
            others[rng.index(others.len())].id
        };
        self.roll_for_target(level, target, magic_find, rng)
    }
    #[cfg(test)]
    fn generate_for_target(&self, level: u32, class_id: u8, rng: &mut Rng) -> Result<Item, String> {
        self.roll_for_target(level, class_id, 0.0, rng)
    }
    #[cfg(test)]
    pub(in crate::survivors) fn gamble_available(&self, slot: SlotKind, class: u8) -> bool {
        self.bases.iter().any(|b| {
            b.slot == slot
                && b.weight > 0.0
                && b.equipment_type.can_drop(class)
                && !(slot == SlotKind::OffHand
                    && b.equipment_type.one_handed()
                    && !super::equipment::dual_wield(class))
        })
    }
    #[cfg(test)]
    pub(in crate::survivors) fn generate_gamble(
        &self,
        level: u32,
        class: u8,
        slot: SlotKind,
        rng: &mut Rng,
    ) -> Result<Item, String> {
        if !self.gamble_available(slot, class) {
            return Err("No items available for this class and slot.".into());
        }
        let mut pool = self.clone();
        pool.bases.retain(|b| {
            b.slot == slot
                && b.equipment_type.can_drop(class)
                && !(slot == SlotKind::OffHand
                    && b.equipment_type.one_handed()
                    && !super::equipment::dual_wield(class))
        });
        pool.roll_for_target(level, class, 0.0, rng)
    }
    fn rarity_weights(&self, level: u32, magic_find: f32) -> [f64; 4] {
        let ordinary = item_level::quality_weights(self.weights(), level, magic_find);
        let total = ordinary.iter().sum::<f64>();
        if level < 20 {
            return if total > 0.0 {
                [ordinary[0], ordinary[1], ordinary[2], 0.0]
            } else {
                [0.0, 0.0, 1.0, 0.0]
            };
        }
        let find = if magic_find.is_finite() {
            f64::from(magic_find.max(0.0))
        } else {
            0.0
        };
        let chance = (self.rarities.legendary / 100.0 * (1.0 + find)).clamp(0.0, 1.0);
        if chance >= 1.0 || total == 0.0 {
            return [0.0, 0.0, 0.0, self.rarities.legendary];
        }
        [
            ordinary[0],
            ordinary[1],
            ordinary[2],
            total * chance / (1.0 - chance),
        ]
    }
    fn roll_for_target(
        &self,
        level: u32,
        class_id: u8,
        magic_find: f32,
        rng: &mut Rng,
    ) -> Result<Item, String> {
        let level = level.clamp(1, MAX_ITEM_LEVEL);
        let class = super::super::data::CLASSES
            .iter()
            .find(|c| c.id == class_id)
            .ok_or("Unknown loot class")?;
        let pool: Vec<_> = legendary::DEFINITIONS
            .iter()
            .filter(|d| {
                level >= d.min_drop_level()
                    && legendary::eligible(d.id, class_id)
                    && self.bases.iter().any(|b| {
                        b.slot == d.slot
                            && b.weight > 0.0
                            && b.equipment_type.can_drop(class_id)
                            && legendary::base_allowed(d.id, class_id, b.equipment_type)
                    })
            })
            .collect();
        let mut weights = self.rarity_weights(level, magic_find);
        if pool.is_empty() {
            weights[3] = 0.0;
            if weights.iter().sum::<f64>() == 0.0 {
                weights[2] = 1.0;
            }
        }
        let rarity = [
            Rarity::Normal,
            Rarity::Magic,
            Rarity::Epic,
            Rarity::Legendary,
        ][weighted_index(&weights, rng.f32()).ok_or("No rarity available")?];
        let unique = if rarity == Rarity::Legendary {
            if pool.is_empty() {
                return Err("No compatible legendary".into());
            }
            Some(pool[rng.index(pool.len())])
        } else {
            None
        };
        let weights: Vec<_> = self
            .bases
            .iter()
            .map(|b| {
                if b.equipment_type.can_drop(class_id)
                    && unique.is_none_or(|d| {
                        d.slot == b.slot
                            && legendary::base_allowed(d.id, class_id, b.equipment_type)
                    })
                {
                    b.weight
                } else {
                    0.0
                }
            })
            .collect();
        let base =
            &self.bases[weighted_index(&weights, rng.f32()).ok_or("No item base available")?];
        let tier = base.tier(level);
        let mut affixes: Vec<Affix> = Vec::new();
        let (minimum, cap) = rarity.affix_limits();
        let available = |kind| {
            self.affixes
                .iter()
                .filter(|a| {
                    a.weight > 0.0
                        && a.kind == kind
                        && a.compatible_base(base)
                        && (!a.effect.primary()
                            || base.equipment_type.drop_primary_allowed(class_id, a.effect))
                })
                .map(|a| &a.group)
                .collect::<BTreeSet<_>>()
                .len()
                .min(cap)
        };
        let prefixes = available(AffixKind::Prefix);
        let suffixes = available(AffixKind::Suffix);
        let maximum = (prefixes + suffixes).min(cap * 2);
        if maximum < minimum {
            return Err("Not enough compatible prefix/suffix groups".into());
        }
        let total = minimum + rng.index(maximum - minimum + 1);
        let allocations: Vec<_> = (0..=prefixes)
            .filter(|&p| p <= total && total - p <= suffixes)
            .collect();
        if allocations.is_empty() {
            return Err("Not enough compatible prefix/suffix groups".into());
        }
        let prefix_count = allocations[rng.index(allocations.len())];
        for index in 0..total {
            let kind = if index < prefix_count {
                AffixKind::Prefix
            } else {
                AffixKind::Suffix
            };
            let pool: Vec<_> = self
                .affixes
                .iter()
                .filter(|a| {
                    a.weight > 0.0
                        && a.kind == kind
                        && a.compatible_base(base)
                        && (!a.effect.primary()
                            || base.equipment_type.drop_primary_allowed(class_id, a.effect))
                        && !affixes.iter().any(|rolled| {
                            rolled.group == a.group
                                || a.effect.primary() && rolled.effect == a.effect
                        })
                })
                .collect();
            let weights: Vec<_> = pool.iter().map(|a| a.weight).collect();
            let a = pool[weighted_index(&weights, rng.f32())
                .ok_or("No compatible affix; check the item table")?];
            let maximum = item_level::roll_max(a.effect, a.max, rarity);
            let value = if a.effect.discrete() {
                a.min + (rng.next_u64() % ((maximum - a.min) as u64 + 1)) as f32
            } else {
                ((rng.range(a.min, maximum) * 1000.0).round() / 1000.0).clamp(a.min, maximum)
            };
            affixes.push(Affix {
                id: a.id.clone(),
                name: a.name.clone(),
                kind: a.kind,
                group: a.group.clone(),
                effect: a.effect,
                value: item_level::scale(a.effect, value, level),
                tags: a.tags.clone(),
                proc: a.proc.as_ref().map(|p| {
                    let spell = if matches!(p.kind, ProcKind::ClassSpell) {
                        let spells: Vec<_> = class
                            .pool
                            .iter()
                            .filter(|&&key| combat::proc_spell(key).is_some())
                            .collect();
                        Some((*spells[rng.index(spells.len())]).to_owned())
                    } else {
                        None
                    };
                    ProcRoll {
                        kind: p.kind,
                        cooldown: p.cooldown,
                        duration: p.duration,
                        power: p.power,
                        class: spell.as_ref().map(|_| class_id),
                        spell,
                    }
                }),
            });
        }
        let prefix = affixes
            .iter()
            .find(|a| a.kind == AffixKind::Prefix)
            .map(|a| a.name.as_str());
        let suffix = affixes
            .iter()
            .find(|a| a.kind == AffixKind::Suffix)
            .map(|a| a.name.as_str());
        let name = [
            prefix,
            Some(tier.map_or(base.name.as_str(), |t| t.name.as_str())),
            suffix,
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");
        Ok(Item {
            affix_rules: 1,
            legendary: unique.map(|d| legendary::Roll {
                unique: d.id,
                milestone: (d.id == 301).then(|| rng.index(81) as u8),
            }),
            base_damage: weapons::is_weapon(base.equipment_type).then(|| base.damage_at(level)),
            base_armour: Some(base.armour_at(level)),
            appearance: tier.and_then(|t| t.appearance).or(base.appearance),
            equipment_type: base.equipment_type,
            base_block: base
                .base_block_max
                .map_or(base.base_block, |max| {
                    (rng.range(base.base_block, max) * 1000.0).round() / 1000.0
                })
                .min(MAX_BASE_BLOCK),
            loot_class: Some(class_id),
            id: 0,
            base: base.id.clone(),
            name: unique.map_or(name, |d| d.name.to_owned()),
            slot: base.slot,
            icon: tier.map_or(&base.icon, |t| &t.icon).clone(),
            rarity,
            level: level.max(1),
            affixes,
        })
    }
    fn read(path: &std::path::Path) -> Result<Self, String> {
        let text = crate::local_state::read_to_string(path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(&text)
    }
    pub(super) fn appearance_for(&self, item: &Item, class: u8) -> Option<(Appearance, &str)> {
        let base = self
            .bases
            .iter()
            .find(|b| b.id == item.base)
            .or_else(|| {
                self.bases
                    .iter()
                    .find(|b| b.slot == item.slot && b.equipment_type == item.equipment_type)
            })
            .or_else(|| {
                self.bases
                    .iter()
                    .find(|b| b.slot == item.slot && b.equipment_type.can_drop(class))
            })?;
        // Missing saved metadata predates variants; keep those items' original family look.
        Some((base.appearance?, base.icon.as_str()))
    }
    fn split_legacy(text: &str) -> Result<(String, String), String> {
        Self::parse(text)?;
        let mut bases: toml::Table = toml::from_str(text).map_err(|e| e.to_string())?;
        let affixes = bases.remove("affixes").ok_or("Missing legacy affixes")?;
        let mut affix_table = toml::Table::new();
        affix_table.insert("affixes".into(), affixes);
        Ok((
            toml::to_string_pretty(&bases).map_err(|e| e.to_string())?,
            toml::to_string_pretty(&affix_table).map_err(|e| e.to_string())?,
        ))
    }
    fn try_load() -> Result<Self, String> {
        if let Some(path) = crate::webenv::var("WOW_ITEMS_CONFIG") {
            return Self::read(std::path::Path::new(&path));
        }
        let bases = Self::load_table(
            "survivors-item-bases.toml",
            "WOW_ITEM_BASES_CONFIG",
            DEFAULT_BASES,
        )?;
        let affixes = Self::load_table(
            "survivors-item-affixes.toml",
            "WOW_ITEM_AFFIXES_CONFIG",
            DEFAULT_AFFIXES,
        )?;
        Self::parse_parts(&bases, &affixes)
    }
    fn load_table(name: &str, override_key: &str, defaults: &str) -> Result<String, String> {
        if let Some(path) = crate::webenv::var(override_key) {
            return crate::local_state::read_to_string(std::path::Path::new(&path))
                .map_err(|e| format!("{path}: {e}"));
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = std::path::Path::new("config").join(name);
            if path.exists() {
                return crate::local_state::read_to_string(&path)
                    .map_err(|e| format!("{}: {e}", path.display()));
            }
        }
        if let Some(home) = crate::local_state::home() {
            let path = home.join(name);
            match crate::local_state::read_to_string(&path) {
                Ok(text) => return Ok(text),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    let content = match crate::local_state::read_to_string(
                        &home.join("survivors-items.toml"),
                    ) {
                        Ok(text) => {
                            let (bases, affixes) = Self::split_legacy(&text)?;
                            if name == "survivors-item-bases.toml" {
                                bases
                            } else {
                                affixes
                            }
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => defaults.to_owned(),
                        Err(e) => return Err(e.to_string()),
                    };
                    crate::local_state::write_atomic(&path, &content).map_err(|e| e.to_string())?;
                    return Ok(content);
                }
                Err(e) => return Err(e.to_string()),
            }
        }
        Ok(defaults.to_owned())
    }
    pub fn load() -> Self {
        Self::try_load().unwrap_or_else(|e| {
            error!("Invalid item configuration: {e}; using built-in item table");
            Self::builtin()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn life_on_block_affixes_are_rare_shield_only_rolls_and_persist() {
        let config = Config::builtin();
        for effect in [Effect::LifeOnBlock, Effect::LifeOnBlockPercent] {
            let affix = config.affixes.iter().find(|a| a.effect == effect).unwrap();
            assert!(affix.weight > 0.0 && affix.weight < 12.0);
            for base in &config.bases {
                assert_eq!(
                    affix.compatible_base(base),
                    base.equipment_type == EquipmentType::Shield
                );
            }
            let mut pool = config.clone();
            pool.bases
                .retain(|b| b.equipment_type == EquipmentType::Shield);
            pool.affixes.retain(|a| a.effect == effect);
            pool.rarities = RarityWeights {
                normal: 100.0,
                magic: 0.0,
                epic: 0.0,
                legendary: 0.0,
            };
            let item = pool.generate_for_target(60, 1, &mut Rng(42)).unwrap();
            assert!(item
                .affixes
                .iter()
                .any(|a| a.effect == effect && a.value > 0.0));
            assert!(item.valid_equipment());
            let saved: Item = toml::from_str(&toml::to_string(&item).unwrap()).unwrap();
            assert_eq!(saved.affixes[0].effect, effect);
            assert!(saved.affixes[0]
                .description()
                .contains(if effect == Effect::LifeOnBlock {
                    "Life on block"
                } else {
                    "Maximum life recovered on block"
                }));
        }
    }
    #[test]
    fn gambling_uses_run_loot_rarities_at_the_same_level() {
        let config = Config::builtin();
        let mut normal_count = 0;
        for slot in [SlotKind::Head, SlotKind::Ring] {
            let mut run_pool = config.clone();
            run_pool.bases.retain(|b| b.slot == slot);
            for level in [1, 20, 40, 60] {
                for seed in 1..=64 {
                    let expected = run_pool
                        .roll_for_target(level, 1, 0.0, &mut Rng(seed))
                        .unwrap();
                    let actual = config
                        .generate_gamble(level, 1, slot, &mut Rng(seed))
                        .unwrap();
                    assert_eq!(
                        actual.rarity, expected.rarity,
                        "{slot:?}, level {level}, seed {seed}"
                    );
                    assert_eq!(actual.base, expected.base);
                    normal_count += usize::from(actual.rarity == Rarity::Normal);
                }
            }
        }
        assert!(normal_count > 0, "Gambling must allow ordinary white drops");
    }
    #[test]
    fn level_bases_keep_all_twenty_one_variants_between_one_and_sixty_without_double_scaling() {
        let config = Config::builtin();
        for base in &config.bases {
            if matches!(base.slot, SlotKind::Shirt | SlotKind::Tabard) {
                continue;
            }
            assert_eq!(
                base.tiers.iter().map(|t| t.level).collect::<Vec<_>>(),
                (3..=60).step_by(3).collect::<Vec<_>>(),
                "{}",
                base.id
            );
            let mut last_armour = base.armour_at(1);
            let mut last_damage = base.damage_at(1);
            for level in 1..=105 {
                let tier = base.tier(level);
                let expected_level = (level.min(60) / 3) * 3;
                assert_eq!(tier.map_or(0, |t| t.level), expected_level);
                let armour = base.armour_at(level);
                let damage = base.damage_at(level);
                assert!(
                    armour >= last_armour
                        && damage.min >= last_damage.min
                        && damage.max >= last_damage.max,
                    "{} level {level}",
                    base.id
                );
                if let Some(tier) = tier.filter(|t| t.level == level) {
                    if let Some(value) = tier.base_armour {
                        assert_eq!(
                            armour,
                            value.round(),
                            "{} tier armour must not scale twice",
                            base.id
                        );
                    }
                    if let Some(value) = tier.base_damage {
                        assert_eq!(damage.min, value.min.round());
                        assert_eq!(damage.max, value.max.round());
                    }
                }
                if level > 60 {
                    assert_eq!(armour, base.armour_at(60));
                    assert_eq!(damage, base.damage_at(60));
                }
                last_armour = armour;
                last_damage = damage;
            }
            if matches!(
                base.slot,
                SlotKind::Ring | SlotKind::Neck | SlotKind::Trinket
            ) {
                let icons: BTreeSet<_> = std::iter::once(&base.icon)
                    .chain(base.tiers.iter().map(|t| &t.icon))
                    .collect();
                assert_eq!(icons.len(), 21, "{} needs distinct tier icons", base.id);
            }
        }
    }
    #[test]
    fn generated_level_variants_keep_their_saved_stats_and_appearance() {
        let config = Config::builtin();
        let mut rng = Rng(741);
        for level in [0, 1, 2, 3, 5, 6, 9, 10, 57, 59, 60, 61, 95, 100, 105] {
            for _ in 0..80 {
                let item = config.generate_for_target(level, 1, &mut rng).unwrap();
                assert_eq!(item.level, level.clamp(1, 60));
                let base = config.bases.iter().find(|b| b.id == item.base).unwrap();
                let tier = base.tier(level);
                assert_eq!(item.icon, tier.map_or(&base.icon, |t| &t.icon).as_str());
                assert_eq!(
                    item.appearance,
                    tier.and_then(|t| t.appearance).or(base.appearance)
                );
                assert_eq!(item.base_armour, Some(base.armour_at(level)));
                let encoded = toml::to_string(&item).unwrap();
                let saved: Item = toml::from_str(&encoded).unwrap();
                assert_eq!(saved.appearance, item.appearance);
                assert_eq!(saved.base_armour, item.base_armour);
                assert_eq!(saved.base_damage, item.base_damage);
                assert_eq!(saved.icon, item.icon);
            }
        }
    }
    #[test]
    fn malformed_level_variants_are_rejected() {
        for invalid in [0, 1, 61, 101] {
            let mut config = Config::builtin();
            config.bases[0].tiers[0].level = invalid;
            assert!(config.validate().is_err());
        }
        let mut config = Config::builtin();
        config.bases[0].tiers[1].level = 3;
        assert!(config.validate().is_err());
        let mut config = Config::builtin();
        config.bases[0].tiers[0].base_armour = Some(f32::NAN);
        assert!(config.validate().is_err());
    }
    #[test]
    fn shield_base_block_stays_below_thirty_percent_and_block_affixes_can_roll() {
        let mut c = Config::builtin();
        let block = c
            .affixes
            .iter()
            .find(|a| a.effect == Effect::BlockChance)
            .unwrap();
        assert_eq!(block.kind, AffixKind::Suffix);
        assert!(block.weight > 0.0);
        for base in &c.bases {
            if base.equipment_type != EquipmentType::Shield {
                assert!(!block.compatible_base(base));
            }
        }
        for base in &mut c.bases {
            base.weight = if base.equipment_type == EquipmentType::Shield {
                1.0
            } else {
                0.0
            };
        }
        let mut rng = Rng(310);
        let mut rolled_block_affix = false;
        for rarity in [
            Rarity::Normal,
            Rarity::Magic,
            Rarity::Epic,
            Rarity::Legendary,
        ] {
            c.rarities = RarityWeights {
                normal: if rarity == Rarity::Normal { 1.0 } else { 0.0 },
                magic: if rarity == Rarity::Magic { 1.0 } else { 0.0 },
                epic: if rarity == Rarity::Epic { 1.0 } else { 0.0 },
                legendary: if rarity == Rarity::Legendary {
                    1.0
                } else {
                    0.0
                },
            };
            for level in [1, 60, 100] {
                for _ in 0..50 {
                    let item = c.generate_for_target(level, 1, &mut rng).unwrap();
                    let expected = if rarity == Rarity::Legendary && level < 20 {
                        Rarity::Epic
                    } else {
                        rarity
                    };
                    assert_eq!(item.rarity, expected);
                    assert!((0.25..=MAX_BASE_BLOCK).contains(&item.base_block));
                    rolled_block_affix |=
                        item.affixes.iter().any(|a| a.effect == Effect::BlockChance);
                    let inventory = Inventory::fixture(vec![item]);
                    Inventory::decode(&toml::to_string(&inventory.save).unwrap()).unwrap();
                }
            }
        }
        assert!(rolled_block_affix);
        assert!(Config::parse(
            &DEFAULT_FILE.replace("base_block_max = 0.30", "base_block_max = 0.31")
        )
        .is_err());
    }
    #[test]
    fn common_stats_and_resistances_are_frequent_without_dominating_suffix_rolls() {
        let mut c = Config::builtin();
        let all = c
            .affixes
            .iter()
            .find(|a| a.effect == Effect::AllResistance)
            .unwrap();
        assert_eq!(all.kind, AffixKind::Suffix);
        for a in c
            .affixes
            .iter()
            .filter(|a| a.effect.resistance() && a.effect != Effect::AllResistance)
        {
            assert_eq!(all.min * 2.0, a.min);
            assert_eq!(all.max * 2.0, a.max);
            assert!(all.weight < a.weight);
        }
        for kind in [AffixKind::Prefix, AffixKind::Suffix] {
            let weights: Vec<_> = c
                .affixes
                .iter()
                .filter(|a| {
                    a.kind == kind
                        && a.weight > 0.0
                        && !matches!(a.effect, Effect::LifeOnBlock | Effect::LifeOnBlockPercent)
                })
                .map(|a| a.weight)
                .collect();
            let min = weights.iter().copied().fold(f64::INFINITY, f64::min);
            let max = weights.iter().copied().fold(0.0, f64::max);
            assert!(
                max <= min * 3.0,
                "{kind:?}: weights {min}..{max} exceed 3:1"
            );
        }
        let single_weight = c
            .affixes
            .iter()
            .find(|a| a.effect == Effect::FireResistance)
            .unwrap()
            .weight;
        for a in c.affixes.iter().filter(|a| a.effect.primary()) {
            assert!(a.weight >= single_weight);
        }
        for base in &mut c.bases {
            base.weight = if base.id == "ring" { 1.0 } else { 0.0 };
        }
        c.rarities = RarityWeights {
            normal: 0.0,
            magic: 1.0,
            epic: 0.0,
            legendary: 0.0,
        };
        let mut rng = Rng(7711);
        let (mut common, mut total, mut all_count) = (0, 0, 0);
        let mut counts = BTreeMap::<String, usize>::new();
        for _ in 0..5000 {
            let item = c.generate_for_target(20, 1, &mut rng).unwrap();
            for a in item.affixes.iter().filter(|a| a.kind == AffixKind::Suffix) {
                total += 1;
                *counts.entry(a.id.clone()).or_default() += 1;
                if a.effect == Effect::AllResistance {
                    all_count += 1;
                } else if a.effect.primary() || a.effect.resistance() {
                    common += 1;
                }
            }
        }
        assert!(
            common * 100 > total * 40 && common * 100 < total * 70,
            "{common}/{total} suffixes were common"
        );
        let largest = counts.values().copied().max().unwrap();
        assert!(
            largest * 100 < total * 10,
            "A single suffix occupied {largest}/{total} rolls"
        );
        assert!(all_count > 0);
        for id in [
            "stamina",
            "strength",
            "fire_resistance",
            "cold_resistance",
            "lightning_resistance",
            "nature_resistance",
            "arcane_resistance",
            "holy_resistance",
            "shadow_resistance",
        ] {
            assert!(
                counts[id] * 2 > all_count * 3,
                "{id}: {} vs all: {all_count}",
                counts[id]
            );
        }
    }

    #[test]
    fn all_resistance_rolls_save_stack_and_protect_every_element_but_not_physical() {
        use super::super::super::{hero, Run};
        use super::super::tests::{affix, item};
        let mut c = Config::builtin();
        for base in &mut c.bases {
            base.weight = if base.id == "ring" { 1.0 } else { 0.0 };
        }
        for a in &mut c.affixes {
            if a.kind == AffixKind::Suffix && a.effect != Effect::AllResistance {
                a.weight = 0.0;
            }
        }
        c.rarities = RarityWeights {
            normal: 0.0,
            magic: 1.0,
            epic: 0.0,
            legendary: 0.0,
        };
        let mut rng = Rng(572);
        let rolled = (0..100)
            .map(|_| c.generate_for_target(60, 1, &mut rng).unwrap())
            .find(|item| {
                item.affixes
                    .iter()
                    .any(|a| a.effect == Effect::AllResistance)
            })
            .unwrap();
        let value = rolled
            .affixes
            .iter()
            .find(|a| a.effect == Effect::AllResistance)
            .unwrap()
            .value;
        let fire = item(
            SlotKind::Ring,
            vec![affix(
                Effect::FireResistance,
                80.0,
                AffixKind::Suffix,
                vec![],
            )],
        );
        let mut inv = Inventory::fixture(vec![rolled, fire]);
        inv.equip(1, "Ring 1").unwrap();
        inv.equip(2, "Ring 2").unwrap();
        inv.save = Inventory::decode(&toml::to_string(&inv.save).unwrap()).unwrap();
        let mut run = Run::default();
        inv.apply(&mut run);
        for (tag, _, percent) in run.stats().resistances.entries() {
            let expected = if tag == Tags::FIRE {
                value + 80.0
            } else {
                value
            };
            assert_eq!(percent, expected);
            let damage = hero::hurt_hero_tagged(&mut run, 100.0, tag | Tags::DOT);
            assert!((damage - 100.0 * (1.0 - expected.min(75.0) / 100.0)).abs() < 0.001);
        }
        assert_eq!(
            hero::hurt_hero_tagged(&mut run, 100.0, Tags::PHYSICAL),
            100.0
        );
        inv.unequip("Ring 1").unwrap();
        inv.apply(&mut run);
        assert_eq!(run.stats().resistances.fire, 80.0);
        assert_eq!(run.stats().resistances.shadow, 0.0);
    }
    #[test]
    fn prefix_suffix_counts_cover_every_rarity_range_and_survive_saving() {
        let mut c = Config::builtin();
        let mut rng = Rng(922);
        for rarity in [
            Rarity::Normal,
            Rarity::Magic,
            Rarity::Epic,
            Rarity::Legendary,
        ] {
            c.rarities = RarityWeights {
                normal: if rarity == Rarity::Normal { 1.0 } else { 0.0 },
                magic: if rarity == Rarity::Magic { 1.0 } else { 0.0 },
                epic: if rarity == Rarity::Epic { 1.0 } else { 0.0 },
                legendary: if rarity == Rarity::Legendary {
                    1.0
                } else {
                    0.0
                },
            };
            let (minimum, cap) = rarity.affix_limits();
            let mut totals = BTreeSet::new();
            for _ in 0..200 {
                let item = c.generate_for_target(60, 1, &mut rng).unwrap();
                totals.insert(item.affixes.len());
                assert_eq!(item.affix_rules, 1);
                for kind in [AffixKind::Prefix, AffixKind::Suffix] {
                    assert!(item.affixes.iter().filter(|a| a.kind == kind).count() <= cap);
                }
                let inventory = Inventory::fixture(vec![item]);
                let text = toml::to_string(&inventory.save).unwrap();
                Inventory::decode(&text).unwrap();
            }
            assert_eq!(totals, (minimum..=cap * 2).collect());
        }
    }

    #[test]
    fn new_allocations_reject_overflow_and_legacy_items_keep_their_rolls() {
        use super::super::tests::{affix, item};
        let mut old_white = item(SlotKind::Chest, vec![]);
        old_white.affix_rules = 1;
        old_white.rarity = Rarity::Normal;
        let old_inventory = Inventory::fixture(vec![old_white]);
        assert!(Inventory::decode(&toml::to_string(&old_inventory.save).unwrap()).is_ok());
        let mut rolled = item(
            SlotKind::Chest,
            vec![
                affix(Effect::Damage, 0.1, AffixKind::Prefix, vec![]),
                affix(Effect::CritChance, 0.1, AffixKind::Prefix, vec![]),
            ],
        );
        rolled.affix_rules = 1;
        rolled.rarity = Rarity::Normal;
        let inventory = Inventory::fixture(vec![rolled.clone()]);
        assert!(Inventory::decode(&toml::to_string(&inventory.save).unwrap()).is_err());
        rolled.rarity = Rarity::Magic;
        let inventory = Inventory::fixture(vec![rolled]);
        assert!(Inventory::decode(&toml::to_string(&inventory.save).unwrap()).is_ok());
        let old = item(
            SlotKind::Chest,
            vec![affix(Effect::Stamina, 12.0, AffixKind::Prefix, vec![])],
        );
        let inventory = Inventory::fixture(vec![old]);
        let text = toml::to_string(&inventory.save)
            .unwrap()
            .replace("affix_rules = 0\n", "");
        let saved = Inventory::decode(&text).unwrap();
        assert_eq!(saved.items[0].affixes[0].value, 12.0);
        assert_eq!(saved.items[0].affixes[0].kind, AffixKind::Prefix);
    }
    #[test]
    fn saved_uniques_load_the_current_english_name_without_changing_their_roll() {
        let mut config = Config::builtin();
        config.rarities = RarityWeights {
            normal: 0.0,
            magic: 0.0,
            epic: 0.0,
            legendary: 100.0,
        };
        let item = config.roll_for_target(60, 1, 0.0, &mut Rng(160)).unwrap();
        let mut inventory = Inventory::fixture(vec![item]);
        let expected = toml::to_string(&inventory.save).unwrap();
        inventory.save.items[0].name = "Ancien nom français".into();
        let decoded = Inventory::decode(&toml::to_string(&inventory.save).unwrap()).unwrap();
        assert_eq!(toml::to_string(&decoded).unwrap(), expected);
    }
    #[test]
    fn legendary_chance_starts_at_item_level_twenty_then_scales_with_find() {
        let config = Config::builtin();
        for level in [1, 10, 19] {
            assert_eq!(config.rarity_weights(level, 100.0)[3], 0.0);
        }
        for level in [20, 60, 100] {
            for (find, expected) in [
                (0.0, 0.05),
                (0.1, 0.055),
                (1.0, 0.10),
                (10.0, 0.55),
                (100.0, 1.0),
            ] {
                let weights = config.rarity_weights(level, find);
                assert!((weights[3] / weights.iter().sum::<f64>() - expected).abs() < 1e-8);
            }
        }
    }
    #[test]
    fn legendary_drops_respect_each_effects_minimum_item_level() {
        let mut config = Config::builtin();
        config.rarities = RarityWeights {
            normal: 0.0,
            magic: 0.0,
            epic: 0.0,
            legendary: 100.0,
        };
        let mut rng = Rng(160);
        for class in super::super::super::data::CLASSES {
            for level in [1, 19, 20, 29, 30, 39, 40, 49, 50, 59, 60, 100] {
                for _ in 0..20 {
                    let item = config
                        .roll_for_target(level, class.id, 100.0, &mut rng)
                        .unwrap();
                    if level < 20 {
                        assert!(item.legendary.is_none());
                    } else {
                        let roll = item
                            .legendary
                            .as_ref()
                            .expect("eligible legendary available");
                        assert!(
                            legendary::definition(roll.unique).unwrap().min_drop_level()
                                <= item.level
                        );
                    }
                }
            }
        }
        assert!(
            legendary::DEFINITIONS
                .iter()
                .filter(|d| d.min_drop_level() >= 50)
                .count()
                > legendary::DEFINITIONS.len() / 2
        );
    }
    #[test]
    fn legendary_rolls_keep_identity_stats_and_quality_across_saving() {
        let mut config = Config::builtin();
        config.rarities = RarityWeights {
            normal: 0.0,
            magic: 0.0,
            epic: 0.0,
            legendary: 1.0,
        };
        let mut rng = Rng(160);
        let mut seen = BTreeSet::new();
        for class in super::super::super::data::CLASSES {
            for _ in 0..50 {
                let item = config.roll_for_target(60, class.id, 0.0, &mut rng).unwrap();
                assert_eq!(item.rarity, Rarity::Legendary);
                assert!((4..=6).contains(&item.affixes.len()));
                assert!(item.affixes.iter().filter(|a| a.effect.primary()).count() <= 3);
                let roll = item.legendary.as_ref().unwrap().clone();
                assert!(legendary::eligible(roll.unique, class.id));
                seen.insert(roll.unique);
                let inventory = Inventory::fixture(vec![item]);
                let text = toml::to_string(&inventory.save).unwrap();
                let decoded = Inventory::decode(&text).unwrap();
                assert_eq!(decoded.items[0].legendary.as_ref(), Some(&roll));
                assert_eq!(decoded.items[0].level, 60);
                assert!((4..=6).contains(&decoded.items[0].affixes.len()));
            }
        }
        assert!(seen.len() > 80);
        for d in legendary::DEFINITIONS {
            assert!(legendary::supported(d.id));
            assert!(
                super::super::super::data::CLASSES
                    .iter()
                    .any(|class| legendary::eligible(d.id, class.id)
                        && config.bases.iter().any(|base| base.weight > 0.0
                            && base.slot == d.slot
                            && base.equipment_type.can_drop(class.id)
                            && legendary::base_allowed(d.id, class.id, base.equipment_type))),
                "{} has no usable drop base",
                d.name
            );
        }
        // Item level one must never accidentally select a zero-weight legendary tier.
        config.rarities.normal = 1.0;
        config.rarities.legendary = 0.0;
        for _ in 0..50 {
            assert_eq!(
                config.roll_for_target(1, 1, 0.0, &mut rng).unwrap().rarity,
                Rarity::Normal
            );
        }
    }
    #[test]
    fn split_tables_preserve_rolls_and_reject_fields_in_the_wrong_file() {
        let split = Config::parse_parts(DEFAULT_BASES, DEFAULT_AFFIXES).unwrap();
        let combined = Config::parse(DEFAULT_FILE).unwrap();
        let (bases, affixes) = Config::split_legacy(DEFAULT_FILE).unwrap();
        let migrated = Config::parse_parts(&bases, &affixes).unwrap();
        let (mut a, mut b, mut c) = (Rng(123), Rng(123), Rng(123));
        for _ in 0..100 {
            let expected = toml::to_string(&combined.generate(20, &mut b).unwrap()).unwrap();
            assert_eq!(
                toml::to_string(&split.generate(20, &mut a).unwrap()).unwrap(),
                expected
            );
            assert_eq!(
                toml::to_string(&migrated.generate(20, &mut c).unwrap()).unwrap(),
                expected
            );
        }
        assert!(Config::parse_parts(DEFAULT_FILE, DEFAULT_AFFIXES).is_err());
        assert!(Config::parse_parts(DEFAULT_BASES, DEFAULT_FILE).is_err());
        assert!(Config::parse_parts(DEFAULT_AFFIXES, DEFAULT_BASES).is_err());
    }
    #[test]
    fn split_tables_apply_base_settings_and_affix_ranges_independently() {
        let bases = DEFAULT_BASES.replace("boss_drop_chance = 0.5", "boss_drop_chance = 0.25");
        let affixes = DEFAULT_AFFIXES.replace("max = 40", "max = 41");
        let config = Config::parse_parts(&bases, &affixes).unwrap();
        assert_eq!(config.boss_drop_chance, 0.25);
        assert_eq!(
            config
                .affixes
                .iter()
                .find(|a| a.id == "stamina")
                .unwrap()
                .max,
            41.0
        );
        assert!(Config::parse_parts(
            DEFAULT_BASES,
            &DEFAULT_AFFIXES.replace("max = 40", "max = -1")
        )
        .is_err());
    }
    #[test]
    fn druids_drop_intelligence_and_agility_gear_and_can_drop_and_equip_bows() {
        let mut config = Config::builtin();
        let mut rng = Rng(11011);
        let (mut intelligence, mut agility) = (false, false);
        for _ in 0..1000 {
            let item = config.generate_for_target(60, 11, &mut rng).unwrap();
            intelligence |= item
                .affixes
                .iter()
                .any(|a| a.effect == Effect::Intelligence);
            agility |= item.affixes.iter().any(|a| a.effect == Effect::Agility);
        }
        assert!(intelligence && agility);
        for base in &mut config.bases {
            base.weight = if base.equipment_type == EquipmentType::Bow {
                1.0
            } else {
                0.0
            };
        }
        let bow = config.generate_for_target(60, 11, &mut rng).unwrap();
        assert_eq!(bow.equipment_type, EquipmentType::Bow);
        assert!(bow.valid_equipment());
        let mut inventory = Inventory::fixture(vec![bow]);
        let id = inventory.all()[0].id;
        inventory.class = 11;
        inventory.equip(id, "Ranged").unwrap();
        assert_eq!(inventory.equipped("Ranged").unwrap().id, id);
    }
    #[test]
    fn targeted_loot_obeys_material_stat_rules_and_the_eighty_twenty_split() {
        let config = Config::builtin();
        let mut rng = Rng(89123);
        let mut seen = BTreeSet::new();
        for player in super::super::super::data::CLASSES {
            let mut targets = BTreeMap::<u8, usize>::new();
            for _ in 0..2_000 {
                let item = config.generate_for_class(20, player.id, &mut rng).unwrap();
                seen.insert(item.base.clone());
                let target = item.loot_class.unwrap();
                *targets.entry(target).or_default() += 1;
                let allowed: &[u8] = match item.equipment_type {
                    EquipmentType::Plate => &[1, 2],
                    EquipmentType::Mail => &[1, 2, 3, 7],
                    EquipmentType::Leather => &[3, 4, 7, 11],
                    EquipmentType::Cloth => &[5, 8, 9, 11],
                    EquipmentType::Shield => &[1, 2, 7],
                    EquipmentType::Wand => &[2, 5, 7, 8, 9, 11],
                    EquipmentType::Bow => &[1, 2, 3, 4, 7, 11],
                    EquipmentType::MeleeOneHand | EquipmentType::HybridOneHand => {
                        &[1, 2, 3, 4, 7, 11]
                    }
                    EquipmentType::TwoHand => &[1, 2, 3, 7, 11],
                    EquipmentType::Staff => &[2, 5, 7, 8, 9, 11],
                    _ => &[1, 2, 3, 4, 5, 7, 8, 9, 11],
                };
                assert!(
                    allowed.contains(&target),
                    "{} {:?} {target}",
                    player.name,
                    item.equipment_type
                );
                for affix in &item.affixes {
                    if item.equipment_type == EquipmentType::HybridOneHand
                        && affix.effect == Effect::Intelligence
                    {
                        assert!([2, 7].contains(&target));
                        assert!(["sword", "axe", "mace"].contains(&item.base.as_str()));
                    }
                    if matches!(
                        item.equipment_type,
                        EquipmentType::Bow | EquipmentType::MeleeOneHand | EquipmentType::TwoHand
                    ) {
                        if target != 2 {
                            assert_ne!(affix.effect, Effect::Intelligence);
                        }
                    }
                    if matches!(
                        item.equipment_type,
                        EquipmentType::Wand | EquipmentType::Staff
                    ) {
                        assert!(!matches!(
                            affix.effect,
                            Effect::Strength
                                | Effect::Agility
                                | Effect::FlatDamage
                                | Effect::AttackSpeed
                                | Effect::LifeLeech
                        ));
                        assert!(!affix.tags.iter().any(|t| matches!(t, Tag::Attack)));
                    }
                    match affix.effect {
                        Effect::Stamina => {}
                        Effect::Strength => assert!(
                            target == 1
                                && !matches!(
                                    item.equipment_type,
                                    EquipmentType::Leather | EquipmentType::Cloth
                                )
                        ),
                        Effect::Agility => assert!(
                            [3, 4, 7, 11].contains(&target)
                                && !matches!(
                                    item.equipment_type,
                                    EquipmentType::Plate | EquipmentType::Cloth
                                )
                        ),
                        Effect::Intelligence => assert!([2, 5, 7, 8, 9, 11].contains(&target)),
                        Effect::BlockChance => {
                            assert_eq!(item.equipment_type, EquipmentType::Shield)
                        }
                        _ => {}
                    }
                }
                if item.equipment_type == EquipmentType::Shield {
                    assert!((0.25..=MAX_BASE_BLOCK).contains(&item.base_block));
                }
                assert!(item.valid_equipment());
            }
            assert!(
                (1520..1680).contains(&targets[&player.id]),
                "{} {targets:?}",
                player.name
            );
            for other in super::super::super::data::CLASSES
                .iter()
                .filter(|c| c.id != player.id)
            {
                assert!(
                    (15..90).contains(targets.get(&other.id).unwrap()),
                    "{} {targets:?}",
                    player.name
                );
            }
        }
        for id in [
            "sword",
            "axe",
            "mace",
            "sword_two_hand",
            "axe_two_hand",
            "mace_two_hand",
            "staff",
            "polearm",
        ] {
            assert!(seen.contains(id), "Base {id} never generated");
        }
    }
    #[test]
    fn one_handed_swords_axes_and_maces_can_roll_intelligence_for_paladins_and_shamans() {
        let mut config = Config::builtin();
        config.rarities = RarityWeights {
            normal: 0.0,
            magic: 0.0,
            epic: 1.0,
            legendary: 0.0,
        };
        let mut rng = Rng(771);
        for id in ["sword", "axe", "mace"] {
            for base in &mut config.bases {
                base.weight = if base.id == id { 1.0 } else { 0.0 };
            }
            for (class, attack) in [
                (1, Effect::Strength),
                (2, Effect::Strength),
                (3, Effect::Agility),
                (4, Effect::Agility),
                (7, Effect::Agility),
                (11, Effect::Agility),
            ] {
                let (mut intelligence, mut attacks) = (0, 0);
                for _ in 0..100 {
                    let item = config.generate_for_target(20, class, &mut rng).unwrap();
                    assert_eq!(item.base, id);
                    for affix in item.affixes.iter().filter(|a| a.effect.primary()) {
                        assert_eq!(affix.kind, AffixKind::Suffix);
                        match affix.effect {
                            Effect::Stamina => {}
                            Effect::Intelligence => {
                                assert!([2, 7].contains(&class));
                                intelligence += 1;
                            }
                            effect => {
                                assert_eq!(effect, attack);
                                attacks += 1;
                            }
                        }
                    }
                    assert!(item.valid_equipment());
                }
                assert_eq!(attacks > 0, class != 2);
                assert_eq!(intelligence > 0, [2, 7].contains(&class));
            }
        }
    }
    #[test]
    fn class_spell_procs_are_bound_at_drop_and_survive_saving_for_all_nine_classes() {
        let mut config = Config::builtin();
        let mut rng = Rng(42);
        for base in &mut config.bases {
            base.weight = if base.slot == SlotKind::Trinket {
                1.0
            } else {
                0.0
            };
        }
        for affix in &mut config.affixes {
            affix.weight =
                if affix.effect.primary() || affix.id == "proc_class_spell" || affix.id == "armour"
                {
                    1.0
                } else {
                    0.0
                };
        }
        config.rarities = RarityWeights {
            normal: 0.0,
            magic: 0.0,
            epic: 1.0,
            legendary: 0.0,
        };
        for class in super::super::super::data::CLASSES {
            let item = config.generate_for_target(20, class.id, &mut rng).unwrap();
            let proc = item.affixes.iter().find_map(|a| a.proc.as_ref()).unwrap();
            assert_eq!(proc.class, Some(class.id));
            assert!(class.pool.contains(&proc.spell.as_deref().unwrap()));
            assert!(proc.valid());
            let inventory = Inventory::fixture(vec![item]);
            let text = toml::to_string_pretty(&inventory.save).unwrap();
            let loaded = Inventory::decode(&text).unwrap();
            assert_eq!(
                loaded.items[0]
                    .affixes
                    .iter()
                    .find_map(|a| a.proc.as_ref())
                    .unwrap()
                    .class,
                Some(class.id)
            );
        }
    }
    #[test]
    fn elemental_resistances_stack_from_equipment_and_cap_after_summing() {
        use super::super::super::{hero, Run};
        use super::super::tests::{affix, item};
        let resistance = |value| {
            item(
                SlotKind::Ring,
                vec![affix(
                    Effect::FireResistance,
                    value,
                    AffixKind::Prefix,
                    vec![],
                )],
            )
        };
        let mut inventory = Inventory::fixture(vec![resistance(20.0), resistance(70.0)]);
        inventory.equip(1, "Ring 1").unwrap();
        inventory.equip(2, "Ring 2").unwrap();
        let saved = toml::to_string(&inventory.save).unwrap();
        inventory.save = Inventory::decode(&saved).unwrap();
        let mut run = Run::default();
        inventory.apply(&mut run);
        assert_eq!(run.stats().resistances.fire, 90.0);
        assert_eq!(
            hero::hurt_hero_tagged(&mut run, 100.0, Tags::FIRE | Tags::DOT),
            25.0
        );
        inventory.unequip("Ring 2").unwrap();
        inventory.apply(&mut run);
        assert_eq!(run.stats().resistances.fire, 20.0);
        assert_eq!(hero::hurt_hero_tagged(&mut run, 100.0, Tags::FIRE), 80.0);
    }

    #[test]
    fn elemental_resistances_roll_equip_roundtrip_and_unequip() {
        use super::super::super::Run;
        for effect in [
            Effect::FireResistance,
            Effect::ColdResistance,
            Effect::LightningResistance,
            Effect::NatureResistance,
            Effect::ArcaneResistance,
            Effect::HolyResistance,
            Effect::ShadowResistance,
        ] {
            let mut c = Config::builtin();
            for base in &mut c.bases {
                base.weight = if base.id == "ring" { 1.0 } else { 0.0 };
            }
            for a in &mut c.affixes {
                a.weight = if a.effect == Effect::Stamina || a.effect == effect {
                    1.0
                } else {
                    0.0
                };
            }
            c.rarities = RarityWeights {
                normal: 0.0,
                magic: 1.0,
                epic: 0.0,
                legendary: 0.0,
            };
            let item = c.generate(60, &mut Rng(123)).unwrap();
            let a = item.affixes.iter().find(|a| a.effect == effect).unwrap();
            let value = a.value;
            assert!(value > 0.0 && value.fract() == 0.0);
            assert!(a.description().contains("resistance"));
            assert!(a.description().contains('%'));
            let mut inventory = Inventory::fixture(vec![item]);
            inventory.equip(1, "Ring 1").unwrap();
            let saved = toml::to_string(&inventory.save).unwrap();
            inventory.save = Inventory::decode(&saved).unwrap();
            let mut run = Run::default();
            inventory.apply(&mut run);
            let entries = run.stats().resistances.entries();
            assert_eq!(entries.iter().map(|(_, _, v)| v).sum::<f32>(), value);
            inventory.unequip("Ring 1").unwrap();
            inventory.apply(&mut run);
            assert!(run
                .stats()
                .resistances
                .entries()
                .iter()
                .all(|(_, _, v)| *v == 0.0));
        }
    }

    #[test]
    fn new_effects_have_configured_pools_and_jewellery_proc_restrictions_are_enforced() {
        let c = Config::builtin();
        for effect in [Effect::Experience, Effect::MagicFind, Effect::Luck] {
            let mut affix = c.affixes.iter().find(|a| a.effect == effect).unwrap().clone();
            assert_eq!(affix.weight, 0.0);
            affix.weight = 100.0; // Old/custom tables cannot re-enable these drop stats.
            assert!(c.bases.iter().all(|base| !affix.compatible_base(base)));
        }
        for id in [
            "vampiric",
            "life_on_kill",
            "regeneration",
            "movement_speed",
            "damage_reduction",
            "healing_effectiveness",
            "boss_damage",
            "full_life_damage",
            "low_life_damage",
            "bleed_chance",
            "ignite_chance",
            "chill_chance",
            "knockback_chance",
            "armour_penetration",
            "pickup_radius",
            "proc_class_spell",
        ] {
            assert!(
                c.affixes.iter().any(|a| a.id == id && a.weight > 0.0),
                "{id}"
            );
        }
        assert!(Config::parse(
            &DEFAULT_FILE.replace("effect = \"luck\"", "effect = \"life_leech\"")
        )
        .is_err());
        assert!(Config::parse(
            &DEFAULT_FILE.replace("slots = [\"trinket\"]", "slots = [\"ring\"]")
        )
        .is_err());
        assert!(Config::parse(&DEFAULT_FILE.replace("duration = 5", "duration = 0")).is_err());
    }
    #[test]
    fn rolls_obey_prefix_suffix_budgets_ranges_and_item_restrictions() {
        let mut config = Config::builtin();
        config.rarities.legendary = 0.0;
        let mut rng = Rng(42);
        let mut counts = [0; 3];
        for _ in 0..10_000 {
            let item = config.generate(20, &mut rng).unwrap();
            counts[match item.rarity {
                Rarity::Normal => 0,
                Rarity::Magic => 1,
                Rarity::Epic => 2,
                Rarity::Legendary => panic!("legendary is disabled"),
            }] += 1;
            let (minimum, cap) = item.rarity.affix_limits();
            assert!((minimum..=cap * 2).contains(&item.affixes.len()));
            for kind in [AffixKind::Prefix, AffixKind::Suffix] {
                assert!(item.affixes.iter().filter(|a| a.kind == kind).count() <= cap);
            }
            assert!(item.affixes.iter().all(|a| a.kind == a.effect.affix_kind()));
            assert!(item
                .affixes
                .iter()
                .all(|a| !matches!(a.effect, Effect::Life | Effect::Damage | Effect::FlatDamage
                    | Effect::Experience | Effect::MagicFind | Effect::Luck)));
            let mut groups = BTreeSet::new();

            for a in &item.affixes {
                let def = config.affixes.iter().find(|d| d.id == a.id).unwrap();
                assert!(def.slots.contains(&item.slot));
                assert!((item_level::scale(def.effect, def.min, item.level)
                    ..=item_level::scale(
                        def.effect,
                        item_level::roll_max(def.effect, def.max, item.rarity),
                        item.level
                    ))
                    .contains(&a.value));
                assert!(groups.insert(&a.group));

                if matches!(a.effect, Effect::AttackSpeed | Effect::FlatDamage) {
                    assert!(item.slot.weapon())
                }
            }
            if counts.iter().sum::<u32>() <= 50 {
                let inventory = Inventory::fixture(vec![item]);
                let text = toml::to_string(&inventory.save).unwrap();
                assert_eq!(Inventory::decode(&text).unwrap().items.len(), 1);
            }
        }
        let weights = item_level::quality_weights(config.weights(), 20, 0.0);
        for (count, weight) in counts.into_iter().zip(weights) {
            let expected = weight / weights.iter().sum::<f64>() * 10_000.0;
            assert!((f64::from(count) - expected).abs() < 250.0);
        }
    }
    #[test]
    fn config_rejects_invalid_weights_weapon_mods_and_incomplete_affix_pools() {
        let config = Config::builtin();
        let mut invalid = config.clone();
        invalid.bases[0].equipment_type = EquipmentType::Legacy;
        assert!(invalid.validate().is_err());
        let mut invalid = config.clone();
        invalid.rarities.legendary = -1.0;
        assert!(invalid.validate().is_err());
        let mut valid = config.clone();
        valid.rarities.legendary = 1.0;
        assert!(valid.validate().is_ok());
        let mut invalid = config.clone();
        invalid.boss_drop_chance = 2.0;
        assert!(invalid.validate().is_err());
        let mut invalid = config.clone();
        invalid.affixes.iter_mut().find(|a| a.kind == AffixKind::Suffix).unwrap().effect = Effect::FlatDamage;
        assert!(invalid.validate().is_err());
        let mut invalid = config.clone();
        invalid.affixes.iter_mut().find(|a| a.kind == AffixKind::Suffix).unwrap().kind = AffixKind::Prefix;
        assert!(invalid.validate().is_err());
        let mut invalid = config;
        invalid.affixes[0].max = -1.0;
        assert!(invalid.validate().is_err());
    }
    #[test]
    fn disabled_bases_and_affixes_never_roll_and_rarity_is_independent_of_pool_size() {
        let mut config = Config::builtin();
        let mut rng = Rng(42);
        for base in &mut config.bases {
            base.weight = if base.id == "ring" { 1.0 } else { 0.0 };
        }
        config.rarities = RarityWeights {
            normal: 0.0,
            magic: 0.0,
            epic: 1.0,
            legendary: 0.0,
        };
        config
            .affixes
            .iter_mut()
            .find(|a| a.id == "armour")
            .unwrap()
            .weight = 0.0;
        for _ in 0..100 {
            let item = config.generate(1, &mut rng).unwrap();
            assert_eq!(item.base, "ring");
            assert_eq!(item.rarity, Rarity::Epic);
            assert!(item.affixes.iter().all(|a| a.id != "armour"));
        }
    }
}
