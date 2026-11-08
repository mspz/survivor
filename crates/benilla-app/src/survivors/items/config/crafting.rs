//! The smith chooses a base independently of the primary-stat affix profile.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::survivors) enum CraftProfile {
    Strength,
    Agility,
    Intelligence,
}
impl CraftProfile {
    pub fn label(self) -> &'static str {
        match self {
            Self::Strength => "Strength",
            Self::Agility => "Agility",
            Self::Intelligence => "Intelligence",
        }
    }
    fn allows(self, affix: &AffixDef) -> bool {
        let primary = match self {
            Self::Strength => Effect::Strength,
            Self::Agility => Effect::Agility,
            Self::Intelligence => Effect::Intelligence,
        };
        (!affix.effect.primary() || affix.effect == Effect::Stamina || affix.effect == primary)
            && !affix.tags.iter().any(|tag| match tag {
                Tag::Attack | Tag::Melee => self == Self::Intelligence,
                Tag::Spell => self != Self::Intelligence,
                _ => false,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> CraftRequest {
        CraftRequest {
            slot: SlotKind::Chest,
            equipment: EquipmentType::Plate,
            profile: CraftProfile::Agility,
            rarity: Rarity::Epic,
            affixes: None,
        }
    }
    #[test]
    fn forged_plate_can_roll_agility_and_preserves_rarity_and_unique_affix_groups() {
        let config = Config::builtin();
        let mut rng = Rng(73);
        let mut saw_agility = false;
        for rarity in [Rarity::Normal, Rarity::Magic, Rarity::Epic] {
            for _ in 0..40 {
                let mut request = request();
                request.rarity = rarity;
                let item = config.generate_craft(&request, 60, 1, &mut rng).unwrap();
                assert_eq!(item.equipment_type, EquipmentType::Plate);
                assert_eq!(item.rarity, rarity);
                assert!(item.valid_equipment());
                assert!(legendary::validate_item(&item));
                assert_eq!(
                    item.affixes
                        .iter()
                        .map(|a| &a.group)
                        .collect::<BTreeSet<_>>()
                        .len(),
                    item.affixes.len()
                );
                assert!(item
                    .affixes
                    .iter()
                    .all(|a| !matches!(a.effect, Effect::Intelligence | Effect::Strength)));
                saw_agility |= item.affixes.iter().any(|a| a.effect == Effect::Agility);
            }
        }
        assert!(saw_agility);
    }
    #[test]
    fn crafted_prices_double_for_rarity_and_each_selected_mod_and_reject_duplicates() {
        let config = Config::builtin();
        let mut request = request();
        request.rarity = Rarity::Normal;
        let base = crate::survivors::gambling::price(SlotKind::Chest, 1) * 3;
        for (index, rarity) in [Rarity::Normal, Rarity::Magic, Rarity::Epic]
            .into_iter()
            .enumerate()
        {
            request.rarity = rarity;
            assert_eq!(request.price(1).unwrap(), base * (1 << index));
        }
        let id = config
            .craft_pool()
            .iter()
            .find(|a| {
                a.effect == Effect::Agility
                    && a.compatible_base(config.craft_base(&request).unwrap())
            })
            .unwrap()
            .id
            .clone();
        request.affixes = Some(vec![id.clone()]);
        assert_eq!(request.price(1).unwrap(), base * 4 * 2);
        let item = config
            .generate_craft(&request, 60, 1, &mut Rng(10))
            .unwrap();
        assert_eq!(item.affixes.len(), 1);
        assert_eq!(item.affixes[0].id, id);
        let definition = config
            .craft_pool()
            .into_iter()
            .find(|a| a.id == id)
            .unwrap();
        let minimum = item_level::scale(definition.effect, definition.min, 60);
        let maximum = item_level::scale(
            definition.effect,
            item_level::roll_max(definition.effect, definition.max, request.rarity),
            60,
        );
        let mut values = BTreeSet::new();
        for seed in 1..=64 {
            let item = config
                .generate_craft(&request, 60, 1, &mut Rng(seed))
                .unwrap();
            assert_eq!(item.affixes[0].id, id);
            let value = item.affixes[0].value;
            assert!(value >= minimum && value <= maximum);
            values.insert(value.to_bits());
        }
        assert!(values.len() > 1, "Chosen affixes must roll variable values");
        assert!(values.iter().any(|value| f32::from_bits(*value) < maximum));
        request.affixes = Some(vec![id.clone(), id]);
        assert!(config.generate_craft(&request, 60, 1, &mut Rng(2)).is_err());
    }
    #[test]
    fn training_legendary_craft_persists_equips_and_activates_its_effect() {
        let mut inventory = Inventory::fixture(vec![]);
        inventory
            .add_money(crate::survivors::gambling::TEST_GOLD_AMOUNT)
            .unwrap();
        let request = CraftRequest {
            slot: SlotKind::Ring,
            equipment: EquipmentType::Universal,
            profile: CraftProfile::Strength,
            rarity: Rarity::Legendary,
            affixes: Some(vec!["strength".into()]),
        };
        let roll = legendary::Roll {
            unique: 46,
            milestone: None,
        };
        let selection = crate::survivors::Selection {
            stage: crate::survivors::data::TRAINING_MAP,
            ..Default::default()
        };
        let before = inventory.money();
        assert!(inventory
            .craft_training(
                &crate::survivors::Selection::default(),
                &request,
                60,
                Some(&roll),
                &mut Rng(1)
            )
            .is_err());
        assert!(inventory
            .craft_training(&selection, &request, 60, None, &mut Rng(1))
            .is_err());
        assert_eq!(inventory.money(), before);
        assert!(inventory.all().is_empty());
        let item = inventory
            .craft_training(&selection, &request, 60, Some(&roll), &mut Rng(1))
            .unwrap();
        assert_eq!(
            inventory.money(),
            before - request.training_price(60).unwrap()
        );
        assert_eq!(item.affixes.len(), 1);
        inventory.equip(item.id, "Ring 1").unwrap();
        let save = Inventory::decode(&inventory.shared_save().unwrap()).unwrap();
        assert_eq!(save.items[0].legendary.as_ref(), Some(&roll));
        assert!(legendary::titan_grip(&save));
        let mut run = crate::survivors::Run::default();
        inventory.apply(&mut run);
        assert!(run.legendary.has(46));
    }

    #[test]
    fn training_forge_supports_real_legendary_effects_and_all_milestone_variants() {
        let config = Config::builtin();
        let mut request = request();
        request.rarity = Rarity::Legendary;
        request.affixes = Some(vec!["agility".into()]);
        let mut found_milestones = false;
        let mut count = 0;
        for &(slot, _) in crate::survivors::gambling::SLOTS {
            request.slot = slot;
            for equipment in config.craft_types(slot) {
                request.equipment = equipment;
                let choices = config.training_legendary_choices(&request, 1);
                if choices.iter().any(|c| c.roll.unique == 301) {
                    assert_eq!(choices.iter().filter(|c| c.roll.unique == 301).count(), 81);
                    found_milestones = true;
                }
                // Empty ordinary affixes allow accessory and weapon bases as well.
                request.affixes = Some(vec![]);
                for choice in choices {
                    let item = config
                        .generate_training_craft(&request, 60, 1, Some(&choice.roll), &mut Rng(9))
                        .unwrap();
                    assert!(item.valid_equipment());
                    assert!(legendary::validate_item(&item));
                    assert_eq!(item.legendary.as_ref(), Some(&choice.roll));
                    assert_eq!(item.rarity, Rarity::Legendary);
                    assert!(!choice.description.is_empty());
                    count += 1;
                }
                assert!(config
                    .generate_training_craft(&request, 60, 1, None, &mut Rng(1))
                    .is_err());
            }
        }
        assert!(found_milestones && count > 300);
        request = self::request();
        request.rarity = Rarity::Legendary;
        let mut epic = request.clone();
        epic.rarity = Rarity::Epic;
        assert_eq!(
            request.training_price(60).unwrap(),
            epic.price(60).unwrap() * 2
        );
    }

    #[test]
    fn unique_crafting_is_rejected_by_generation_and_pricing() {
        let config = Config::builtin();
        let mut request = request();
        request.rarity = Rarity::Legendary;
        assert!(request.price(60).is_err());
        assert!(config.generate_craft(&request, 60, 1, &mut Rng(1)).is_err());
    }
    #[test]
    fn every_profile_only_offers_its_primary_stat_and_compatible_combat_mods() {
        let config = Config::builtin();
        for profile in [
            CraftProfile::Strength,
            CraftProfile::Agility,
            CraftProfile::Intelligence,
        ] {
            let mut request = request();
            request.profile = profile;
            let base = config.craft_base(&request).unwrap();
            let choices = config.craft_affixes(&request, 60, 1);
            for (id, expected) in [
                ("strength", CraftProfile::Strength),
                ("agility", CraftProfile::Agility),
                ("intelligence", CraftProfile::Intelligence),
            ] {
                assert_eq!(choices.iter().any(|a| a.id == id), profile == expected);
                let mut selected = request.clone();
                selected.affixes = Some(vec![id.into()]);
                assert_eq!(
                    config.generate_craft(&selected, 60, 1, &mut Rng(8)).is_ok(),
                    profile == expected
                );
            }
            assert!(choices.iter().any(|a| a.id == "stamina"));
            assert!(base.equipment_type == EquipmentType::Plate);
        }
    }
}
#[derive(Clone, Debug)]
pub(in crate::survivors) struct CraftRequest {
    pub slot: SlotKind,
    pub equipment: EquipmentType,
    pub profile: CraftProfile,
    pub rarity: Rarity,
    /// None rolls affix identities; Some selects identities. Both roll their values.
    pub affixes: Option<Vec<String>>,
}
impl CraftRequest {
    pub fn training_price(&self, level: u32) -> Result<u64, String> {
        if self.rarity != Rarity::Legendary {
            return self.price(level);
        }
        let mut ordinary = self.clone();
        ordinary.rarity = Rarity::Epic;
        ordinary
            .price(level)?
            .checked_mul(2)
            .filter(|p| *p <= crate::survivors::currency::MAX_BALANCE)
            .ok_or_else(|| "Price too high.".into())
    }
    pub fn price(&self, level: u32) -> Result<u64, String> {
        let rarity = match self.rarity {
            Rarity::Normal => 0,
            Rarity::Magic => 1,
            Rarity::Epic => 2,
            Rarity::Legendary => {
                return Err("Unique items cannot be crafted at this blacksmith.".into())
            }
        };
        let selected = self.affixes.as_ref().map_or(0, Vec::len);
        let multiplier = 1u64
            .checked_shl((rarity + selected) as u32)
            .ok_or("Price too high.")?;
        crate::survivors::gambling::price(self.slot, level)
            .checked_mul(3)
            .and_then(|p| p.checked_mul(multiplier))
            .filter(|p| *p <= crate::survivors::currency::MAX_BALANCE)
            .ok_or_else(|| "Price too high.".into())
    }
}
#[derive(Clone)]
pub(in crate::survivors) struct LegendaryChoice {
    pub roll: legendary::Roll,
    pub label: String,
    pub description: String,
}

#[derive(Clone)]
pub(in crate::survivors) struct CraftAffix {
    pub id: String,
    pub label: String,
    pub kind: AffixKind,
    pub group: String,
}
impl Config {
    pub(in crate::survivors) fn training_legendary_choices(
        &self,
        request: &CraftRequest,
        class: u8,
    ) -> Vec<LegendaryChoice> {
        let mut choices = Vec::new();
        for def in legendary::DEFINITIONS.iter().filter(|d| {
            d.slot == request.slot && legendary::base_allowed(d.id, class, request.equipment)
        }) {
            if def.id == 301 {
                for option in 0..81 {
                    let grant = legendary::milestones::Grant { option };
                    choices.push(LegendaryChoice {
                        roll: legendary::Roll {
                            unique: def.id,
                            milestone: Some(option),
                        },
                        label: format!("{} - {}", def.name, grant.milestone().name),
                        description: legendary::milestones::description(option),
                    });
                }
            } else {
                choices.push(LegendaryChoice {
                    roll: legendary::Roll {
                        unique: def.id,
                        milestone: None,
                    },
                    label: def.name.into(),
                    description: def.description.into(),
                });
            }
        }
        choices.sort_by_key(|c| c.label.to_lowercase());
        choices
    }
    pub(in crate::survivors) fn generate_training_craft(
        &self,
        request: &CraftRequest,
        level: u32,
        class: u8,
        roll: Option<&legendary::Roll>,
        rng: &mut Rng,
    ) -> Result<Item, String> {
        if request.rarity != Rarity::Legendary {
            return self.generate_craft(request, level, class, rng);
        }
        let choices = self.training_legendary_choices(request, class);
        let roll = roll.ok_or("Select a legendary effect.")?;
        if !choices.iter().any(|choice| choice.roll == *roll) {
            return Err("This legendary effect is incompatible with the item.".into());
        }
        let mut item = self.generate_craft_inner(request, level, class, rng, true)?;
        item.legendary = Some(roll.clone());
        item.name = legendary::definition(roll.unique).unwrap().name.into();
        Ok(item)
    }
    pub(in crate::survivors) fn craft_preview(
        &self,
        request: &CraftRequest,
        level: u32,
        class: u8,
    ) -> Option<Item> {
        let mut preview = request.clone();
        // Unknown affixes are revealed only by purchasing; selected affixes show an example roll.
        if preview.affixes.is_none() {
            preview.affixes = Some(vec![]);
        }
        self.generate_craft(&preview, level, class, &mut Rng(1))
            .ok()
    }
    fn craft_pool(&self) -> Vec<AffixDef> {
        let mut pool = self.affixes.clone();
        // The smith can supply primary attributes even when natural loot tables omit them.
        for (id, name, effect) in [
            ("strength", "of Strength", Effect::Strength),
            ("agility", "of Agility", Effect::Agility),
            ("intelligence", "of Intelligence", Effect::Intelligence),
            ("stamina", "of Stamina", Effect::Stamina),
        ] {
            if !pool.iter().any(|a| a.effect == effect) {
                pool.push(AffixDef {
                    id: id.into(),
                    name: name.into(),
                    kind: AffixKind::Suffix,
                    group: id.into(),
                    slots: crate::survivors::items::SLOTS
                        .iter()
                        .map(|(_, slot)| *slot)
                        .collect(),
                    effect,
                    min: 8.0,
                    max: 20.0,
                    weight: 24.0,
                    tags: vec![],
                    proc: None,
                });
            }
        }
        pool
    }
    pub(in crate::survivors) fn craft_types(&self, slot: SlotKind) -> Vec<EquipmentType> {
        let mut types = vec![];
        for base in self
            .bases
            .iter()
            .filter(|b| b.slot == slot && b.weight > 0.0)
        {
            if !types.contains(&base.equipment_type) {
                types.push(base.equipment_type);
            }
        }
        types
    }
    fn craft_base(&self, request: &CraftRequest) -> Option<&Base> {
        self.bases.iter().find(|b| {
            b.slot == request.slot && b.equipment_type == request.equipment && b.weight > 0.0
        })
    }
    fn craft_affix_allowed(&self, a: &AffixDef, base: &Base, profile: CraftProfile) -> bool {
        a.weight > 0.0 && a.compatible_base(base) && profile.allows(a)
    }
    fn craft_roll_affix(
        &self,
        a: &AffixDef,
        level: u32,
        class: u8,
        rarity: Rarity,
        selected: bool,
        rng: &mut Rng,
    ) -> Affix {
        let max = item_level::roll_max(a.effect, a.max, rarity);
        let value = if a.effect.discrete() {
            a.min + (rng.next_u64() % ((max - a.min) as u64 + 1)) as f32
        } else {
            ((rng.range(a.min, max) * 1000.0).round() / 1000.0).clamp(a.min, max)
        };
        let proc = a.proc.as_ref().map(|p| {
            let spell = if matches!(p.kind, ProcKind::ClassSpell) {
                let class = crate::survivors::data::CLASSES
                    .iter()
                    .find(|c| c.id == class)
                    .expect("validated class");
                let spells: Vec<_> = class
                    .pool
                    .iter()
                    .filter(|key| combat::proc_spell(key).is_some())
                    .collect();
                Some((*spells[if selected { 0 } else { rng.index(spells.len()) }]).to_string())
            } else {
                None
            };
            ProcRoll {
                kind: p.kind,
                cooldown: p.cooldown,
                duration: p.duration,
                power: p.power,
                class: spell.as_ref().map(|_| class),
                spell,
            }
        });
        Affix {
            id: a.id.clone(),
            name: a.name.clone(),
            kind: a.kind,
            group: a.group.clone(),
            effect: a.effect,
            value: item_level::scale(a.effect, value, level),
            tags: a.tags.clone(),
            proc,
        }
    }
    pub(in crate::survivors) fn craft_affixes(
        &self,
        request: &CraftRequest,
        level: u32,
        class: u8,
    ) -> Vec<CraftAffix> {
        let Some(base) = self.craft_base(request) else {
            return vec![];
        };
        let mut rng = Rng(1);
        self.craft_pool()
            .iter()
            .filter(|a| self.craft_affix_allowed(a, base, request.profile))
            .map(|a| {
                let mut rolled =
                    self.craft_roll_affix(a, level, class, request.rarity, true, &mut rng);
                rolled.value = item_level::scale(a.effect, a.min, level);
                let minimum = rolled.description();
                rolled.value = item_level::scale(
                    a.effect,
                    item_level::roll_max(a.effect, a.max, request.rarity),
                    level,
                );
                let maximum = rolled.description();
                CraftAffix {
                    id: a.id.clone(),
                    label: if minimum == maximum {
                        maximum
                    } else {
                        format!("{minimum} - {maximum}")
                    },
                    kind: a.kind,
                    group: a.group.clone(),
                }
            })
            .collect()
    }
    pub(in crate::survivors) fn generate_craft(
        &self,
        request: &CraftRequest,
        level: u32,
        class: u8,
        rng: &mut Rng,
    ) -> Result<Item, String> {
        self.generate_craft_inner(request, level, class, rng, false)
    }
    fn generate_craft_inner(
        &self,
        request: &CraftRequest,
        level: u32,
        class: u8,
        rng: &mut Rng,
        training: bool,
    ) -> Result<Item, String> {
        if !crate::survivors::data::CLASSES
            .iter()
            .any(|c| c.id == class)
        {
            return Err("Classe inconnue.".into());
        }
        if request.rarity == Rarity::Legendary && !training {
            return Err("Unique items cannot be crafted at this blacksmith.".into());
        }
        let level = level.clamp(1, MAX_ITEM_LEVEL);
        let bases: Vec<_> = self
            .bases
            .iter()
            .filter(|b| {
                b.slot == request.slot && b.equipment_type == request.equipment && b.weight > 0.0
            })
            .collect();
        if bases.is_empty() {
            return Err("No base available for this slot and type.".into());
        }
        let base = bases[weighted_index(
            &bases.iter().map(|b| b.weight).collect::<Vec<_>>(),
            rng.f32(),
        )
        .ok_or("No base available.")?];
        let (minimum, cap) = request.rarity.affix_limits();
        let pool = self.craft_pool();
        let mut chosen: Vec<&AffixDef> = vec![];
        if let Some(ids) = &request.affixes {
            for id in ids {
                let a = pool
                    .iter()
                    .find(|a| &a.id == id && self.craft_affix_allowed(a, base, request.profile))
                    .ok_or("This affix is incompatible with the craft.")?;
                if chosen.iter().any(|b| b.id == a.id || b.group == a.group) {
                    return Err("Each affix can only appear once.".into());
                }
                if chosen.iter().filter(|b| b.kind == a.kind).count() >= cap {
                    return Err("Prefix or suffix limit reached for this rarity.".into());
                }
                chosen.push(a);
            }
        } else {
            let pool: Vec<_> = pool
                .iter()
                .filter(|a| self.craft_affix_allowed(a, base, request.profile))
                .collect();
            let counts = [AffixKind::Prefix, AffixKind::Suffix].map(|kind| {
                pool.iter()
                    .filter(|a| a.kind == kind)
                    .map(|a| &a.group)
                    .collect::<BTreeSet<_>>()
                    .len()
                    .min(cap)
            });
            let max = counts.iter().sum::<usize>();
            if max < minimum {
                return Err("Not enough compatible affixes.".into());
            }
            let total = minimum + rng.index(max - minimum + 1);
            for _ in 0..total {
                let choices: Vec<_> = pool
                    .iter()
                    .copied()
                    .filter(|a| {
                        !chosen.iter().any(|b| b.group == a.group)
                            && chosen.iter().filter(|b| b.kind == a.kind).count() < cap
                    })
                    .collect();
                let weights: Vec<_> = choices.iter().map(|a| a.weight).collect();
                let index = weighted_index(&weights, rng.f32())
                    .ok_or("Not enough compatible affixes.")?;
                chosen.push(choices[index]);
            }
        }
        let affixes: Vec<_> = chosen
            .into_iter()
            .map(|a| {
                self.craft_roll_affix(
                    a,
                    level,
                    class,
                    request.rarity,
                    request.affixes.is_some(),
                    rng,
                )
            })
            .collect();
        let tier = base.tier(level);
        Ok(Item {
            affix_rules: 2,
            loot_class: None,
            id: 0,
            base: base.id.clone(),
            name: format!(
                "{} forged ({})",
                tier.map_or(base.name.as_str(), |t| t.name.as_str()),
                request.profile.label()
            ),
            slot: base.slot,
            icon: tier.map_or(&base.icon, |t| &t.icon).clone(),
            rarity: request.rarity,
            level,
            affixes,
            legendary: None,
            base_damage: weapons::is_weapon(base.equipment_type).then(|| base.damage_at(level)),
            base_armour: Some(base.armour_at(level)),
            appearance: tier.and_then(|t| t.appearance).or(base.appearance),
            equipment_type: base.equipment_type,
            base_block: base
                .base_block_max
                .map_or(base.base_block, |max| rng.range(base.base_block, max))
                .min(MAX_BASE_BLOCK),
        })
    }
}
