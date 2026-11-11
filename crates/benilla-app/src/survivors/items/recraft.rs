//! Reforge ordinary affixes in place, preserving the saved base and unique power.
use super::*;

/// 500 gold at ilvl 60; doubles every ten item levels without the crafting level-60 cap.
pub(in crate::survivors) fn price(level: u32) -> Result<u64, String> {
    if !(1..=super::super::dungeon_config::MAX_REWARD_LEVEL).contains(&level) {
        return Err("Invalid item level.".into());
    }
    let copper = (500.0
        * super::super::currency::GOLD as f64
        * 2.0f64.powf((f64::from(level) - 60.0) / 10.0))
    .ceil();
    if !copper.is_finite() || copper >= super::super::currency::MAX_BALANCE as f64 {
        return Err("Recraft price too high.".into());
    }
    Ok(copper as u64)
}

impl Inventory {
    pub(in crate::survivors) fn recraft_affixes(&self, id: u64) -> Vec<CraftAffix> {
        self.item(id).map_or_else(Vec::new, |item| {
            self.config.recraft_affixes(item, self.class)
        })
    }

    pub(in crate::survivors) fn recraft(
        &mut self,
        id: u64,
        edits: &[(usize, Option<String>)],
        rng: &mut Rng,
    ) -> Result<Item, String> {
        self.writable()?;
        let source = self
            .item(id)
            .ok_or("This item is no longer in your inventory.")?;
        if edits.is_empty() {
            return Err("Select at least one affix to edit.".into());
        }
        let cost = price(source.level)?
            .checked_mul(edits.len() as u64)
            .filter(|cost| *cost <= super::super::currency::MAX_BALANCE)
            .ok_or("Recraft price too high.")?;
        let copper = self.money().checked_sub(cost).ok_or("Not enough gold.")?;
        // Invalid selections do not consume RNG, money, or alter the original item.
        let mut next_rng = Rng(rng.0);
        let item = self
            .config
            .generate_recraft(source, edits, self.class, &mut next_rng)?;
        let mut save = self.save.clone();
        *save.items.iter_mut().find(|i| i.id == id).unwrap() = item.clone();
        save.copper = copper;
        self.validate_loadout(&save)?;
        legendary::validate_loadout(&save)?;
        // This edits an existing item, so auto-salvage must not sell it or unrelated items.
        self.persist(&save)?;
        self.save = save;
        self.money_unsaved = false;
        self.revision = self.revision.wrapping_add(1);
        *rng = next_rng;
        Ok(item)
    }
}

pub(in crate::survivors) fn slots(item: &Item) -> Result<[Option<&Affix>; 6], String> {
    let mut slots = [None; 6];
    let mut counts = [0; 2];
    let (_, cap) = item.rarity.affix_limits();
    for affix in &item.affixes {
        let group = usize::from(affix.kind == AffixKind::Suffix);
        if counts[group] >= cap {
            return Err("This legacy item has too many affixes of one kind to recraft.".into());
        }
        slots[group * 3 + counts[group]] = Some(affix);
        counts[group] += 1;
    }
    Ok(slots)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn inventory(level: u32, unique: bool) -> Inventory {
        let config = Config::builtin();
        let request = CraftRequest {
            slot: SlotKind::Ring,
            equipment: EquipmentType::Universal,
            profile: CraftProfile::Strength,
            rarity: if unique {
                Rarity::Legendary
            } else {
                Rarity::Epic
            },
            affixes: Some(vec!["strength".into()]),
        };
        let mut item = if unique {
            config
                .generate_training_craft(
                    &request,
                    60,
                    1,
                    Some(&legendary::Roll {
                        unique: 403,
                        milestone: None,
                    }),
                    &mut Rng(42),
                )
                .unwrap()
        } else {
            config
                .generate_craft(&request, 60, 1, &mut Rng(42))
                .unwrap()
        };
        if level > 60 {
            item.raise_item_level(level).unwrap();
        }
        let mut inventory = Inventory::fixture(vec![item]);
        inventory.save.copper = super::super::super::currency::MAX_BALANCE;
        inventory.save.equipped.insert("Ring 1".into(), 1);
        inventory
    }
    #[test]
    fn exponential_price_uses_actual_ilvl_and_each_edit_is_charged() {
        let gold = super::super::super::currency::GOLD;
        assert_eq!(price(60).unwrap(), 500 * gold);
        assert_eq!(price(70).unwrap(), 1000 * gold);
        assert_eq!(price(110).unwrap(), 16000 * gold);
        assert!(price(61).unwrap() > price(60).unwrap());
        assert!(price(200).unwrap() > price(110).unwrap());
        assert!(price(0).is_err());
        let mut inventory = inventory(110, true);
        let choices = inventory.recraft_affixes(1);
        let mut groups = BTreeSet::new();
        let mut edits = vec![];
        for (group, kind) in [(0, AffixKind::Prefix), (1, AffixKind::Suffix)] {
            for a in choices.iter().filter(|a| a.kind == kind) {
                if edits.iter().filter(|(slot, _)| *slot / 3 == group).count() == 3 {
                    break;
                }
                if groups.insert(a.group.clone()) {
                    let index = edits.iter().filter(|(slot, _)| *slot / 3 == group).count();
                    edits.push((group * 3 + index, Some(a.id.clone())));
                }
            }
        }
        assert_eq!(edits.len(), 6);
        let money = inventory.money();
        inventory.recraft(1, &edits, &mut Rng(9)).unwrap();
        assert_eq!(inventory.money(), money - 6 * price(110).unwrap());
        assert_eq!(inventory.item(1).unwrap().affixes.len(), 6);
    }
    #[test]
    fn adding_or_editing_an_affix_preserves_other_rolls_unique_base_and_equipment() {
        for unique in [false, true] {
            let mut inventory = inventory(110, unique);
            inventory.save.auto_salvage[if unique { 3 } else { 2 }] = AutoSalvageRule {
                enabled: true,
                below_ilvl: 200,
            };
            let before = inventory.item(1).unwrap().clone();
            let original = serde_json::to_value(&before.affixes[0]).unwrap();
            let prefix = inventory
                .recraft_affixes(1)
                .into_iter()
                .find(|a| a.kind == AffixKind::Prefix)
                .unwrap();
            let money = inventory.money();
            let added = inventory
                .recraft(1, &[(0, Some(prefix.id))], &mut Rng(4))
                .unwrap();
            assert_eq!(inventory.money(), money - price(110).unwrap());
            assert_eq!(
                serde_json::to_value(added.affixes.iter().find(|a| a.id == "strength").unwrap())
                    .unwrap(),
                original
            );
            assert_eq!(added.legendary, before.legendary);
            assert_eq!(added.id, before.id);
            assert_eq!(added.base, before.base);
            assert_eq!(added.level, before.level);
            assert_eq!(added.name, before.name);
            assert_eq!(added.icon, before.icon);
            assert_eq!(
                serde_json::to_value(added.base_damage).unwrap(),
                serde_json::to_value(before.base_damage).unwrap()
            );
            assert_eq!(added.base_armour, before.base_armour);
            assert_eq!(added.base_block, before.base_block);
            assert_eq!(added.appearance, before.appearance);
            assert_eq!(inventory.equipped("Ring 1").unwrap().id, 1);
            assert!(added.upgrade_basis.is_none());
            let edited = inventory
                .recraft(1, &[(3, Some("agility".into()))], &mut Rng(5))
                .unwrap();
            assert_eq!(
                serde_json::to_value(&edited.affixes[0]).unwrap(),
                serde_json::to_value(&added.affixes[0]).unwrap()
            );
            assert!(edited.affixes.iter().any(|a| a.id == "agility"));
            let restored = Inventory::decode(&toml::to_string(&inventory.save).unwrap()).unwrap();
            assert_eq!(restored.items[0].legendary, before.legendary);
            let mut upgraded = edited.clone();
            upgraded.raise_item_level(111).unwrap();
            assert!(upgraded.affixes.iter().any(|a| a.id == "agility"));
            assert!(!upgraded.affixes.iter().any(|a| a.id == "strength"));
        }
    }
    #[test]
    fn invalid_or_unaffordable_edits_leave_gold_items_and_rng_untouched() {
        let mut inventory = inventory(60, true);
        let before = serde_json::to_value(&inventory.save).unwrap();
        let mut rng = Rng(42);
        for edits in [
            vec![],
            vec![(6, Some("strength".into()))],
            vec![(0, Some("strength".into()))],
            vec![(4, Some("strength".into()))],
            vec![(3, Some("missing".into()))],
            vec![(3, None), (3, None)],
        ] {
            assert!(inventory.recraft(1, &edits, &mut rng).is_err());
            assert_eq!(serde_json::to_value(&inventory.save).unwrap(), before);
            assert_eq!(rng.0, 42);
        }
        inventory.save.copper = price(60).unwrap() - 1;
        let before = serde_json::to_value(&inventory.save).unwrap();
        assert!(
            inventory
                .recraft(1, &[(3, Some("agility".into()))], &mut rng)
                .is_err()
        );
        assert_eq!(serde_json::to_value(&inventory.save).unwrap(), before);
        assert_eq!(rng.0, 42);
    }
}
