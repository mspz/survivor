//! Character-owned automatic sales use the same price and transaction as manual sales.
use super::*;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(in crate::survivors) struct Rule {
    pub enabled: bool,
    pub below_ilvl: u32,
}
fn index(rarity: Rarity) -> usize {
    match rarity {
        Rarity::Normal => 0,
        Rarity::Magic => 1,
        Rarity::Epic => 2,
        Rarity::Legendary => 3,
    }
}
pub(super) fn apply(save: &mut Save) {
    let worn: BTreeSet<_> = save.equipped.values().copied().collect();
    let rules = save.auto_salvage;
    let mut copper = save.copper;
    save.items.retain(|item| {
        let rule = rules[index(item.rarity)];
        if worn.contains(&item.id) || !rule.enabled || item.level >= rule.below_ilvl {
            return true;
        }
        if let Some(total) = copper
            .checked_add(item.sale_price())
            .filter(|&v| v <= super::super::currency::MAX_BALANCE)
        {
            copper = total;
            false
        } else {
            true
        }
    });
    save.copper = copper;
}
impl Inventory {
    pub(in crate::survivors) fn auto_salvage(&self) -> [Rule; 4] {
        self.save.auto_salvage
    }
    pub(in crate::survivors) fn set_auto_salvage(
        &mut self,
        rules: [Rule; 4],
    ) -> Result<(), String> {
        if rules.iter().any(|r| r.below_ilvl > MAX_ITEM_LEVEL + 1) {
            return Err("Le seuil doit etre compris entre 0 et 61.".into());
        }
        let mut save = self.save.clone();
        save.auto_salvage = rules;
        self.commit(save)
    }
}
#[cfg(test)]
mod tests {
    use super::super::tests::item;
    use super::*;
    #[test]
    fn rules_sell_only_matching_bag_items_below_threshold_and_preserve_equipped_items() {
        let mut items = Vec::new();
        for (rarity, level) in [
            (Rarity::Normal, 39),
            (Rarity::Normal, 40),
            (Rarity::Magic, 20),
            (Rarity::Normal, 20),
        ] {
            let mut item = item(SlotKind::Ring, vec![]);
            item.level = level;
            item.rarity = rarity;
            items.push(item);
        }
        let mut inventory = Inventory::fixture(items);
        inventory.equip(4, "Ring 1").unwrap();
        let price = inventory.item(1).unwrap().sale_price();
        let mut rules = [Rule::default(); 4];
        rules[0] = Rule {
            enabled: true,
            below_ilvl: 40,
        };
        inventory.set_auto_salvage(rules).unwrap();
        assert!(inventory.item(1).is_none());
        for id in [2, 3, 4] {
            assert!(inventory.item(id).is_some());
        }
        assert_eq!(inventory.money(), price);
        let mut drop = item(SlotKind::Ring, vec![]);
        drop.level = 10;
        let expected = inventory.money() + drop.sale_price();
        inventory.collect(drop).unwrap();
        assert_eq!(inventory.money(), expected);
        assert_eq!(inventory.all().len(), 3);
        assert_eq!(inventory.auto_salvage(), rules);
    }
    #[test]
    fn rules_round_trip_default_for_old_saves_and_apply_to_each_rarity() {
        let mut inventory = Inventory::fixture(vec![]);
        let rules = [Rule {
            enabled: true,
            below_ilvl: 40,
        }; 4];
        inventory.set_auto_salvage(rules).unwrap();
        for rarity in [
            Rarity::Normal,
            Rarity::Magic,
            Rarity::Epic,
            Rarity::Legendary,
        ] {
            let mut drop = item(SlotKind::Ring, vec![]);
            drop.rarity = rarity;
            drop.level = 39;
            let expected = inventory.money() + drop.sale_price();
            inventory.collect(drop).unwrap();
            assert_eq!(inventory.money(), expected);
            assert!(inventory.all().is_empty());
        }
        let encoded = toml::to_string(&inventory.save).unwrap();
        assert_eq!(Inventory::decode(&encoded).unwrap().auto_salvage, rules);
        let mut legacy: toml::Value = toml::from_str(&encoded).unwrap();
        legacy.as_table_mut().unwrap().remove("auto_salvage");
        assert_eq!(
            Inventory::decode(&toml::to_string(&legacy).unwrap())
                .unwrap()
                .auto_salvage,
            [Rule::default(); 4]
        );
    }
    #[test]
    fn failed_saves_or_full_wallets_keep_items_and_previous_rules() {
        let mut inventory = Inventory::fixture(vec![item(SlotKind::Ring, vec![])]);
        let mut rules = [Rule::default(); 4];
        rules[0] = Rule {
            enabled: true,
            below_ilvl: 40,
        };
        inventory.blocked = true;
        assert!(inventory.set_auto_salvage(rules).is_err());
        assert!(inventory.item(1).is_some());
        assert_eq!(inventory.auto_salvage(), [Rule::default(); 4]);
        inventory.blocked = false;
        inventory.save.copper = super::super::super::currency::MAX_BALANCE;
        inventory.set_auto_salvage(rules).unwrap();
        assert!(inventory.item(1).is_some());
    }
}
