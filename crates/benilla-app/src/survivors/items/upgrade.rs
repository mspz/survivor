//! Upgrade an existing roll without replacing its identity, affix choices or legendary effect.
use super::*;

/// Preserve the original roll through repeated upgrades, saves and item transfers.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in crate::survivors) struct Basis {
    level: u32,
    affixes: Vec<Affix>,
    base_damage: Option<weapons::DamageRange>,
    base_armour: Option<f32>,
}

impl Item {
    pub(in crate::survivors) fn raise_item_level(&mut self, target: u32) -> Result<(), String> {
        if target <= self.level || target > super::super::dungeon_config::MAX_REWARD_LEVEL {
            return Err("Invalid target item level.".into());
        }
        if self.upgrade_basis.is_none() {
            self.upgrade_basis = Some(Box::new(Basis {
                level: self.level,
                affixes: self.affixes.clone(),
                base_damage: self.base_damage,
                base_armour: self.base_armour,
            }));
        }
        let basis = self.upgrade_basis.as_ref().unwrap();
        if basis.affixes.len() != self.affixes.len() {
            return Err("The original item roll is inconsistent.".into());
        }
        let ratio = item_level::factor(target) / item_level::factor(basis.level);
        for (affix, original) in self.affixes.iter_mut().zip(&basis.affixes) {
            if matches!(
                affix.effect,
                Effect::Projectiles | Effect::Chain | Effect::Fork | Effect::MeleeTargets
            ) {
                continue;
            }
            let raw = original.value / item_level::growth(affix.effect, basis.level);
            affix.value = item_level::scale(affix.effect, raw, target);
        }
        if let (Some(damage), Some(original)) = (&mut self.base_damage, basis.base_damage) {
            damage.min = (original.min * ratio).round().min(1.0e6);
            damage.max = (original.max * ratio).round().min(1.0e6);
        }
        if let (Some(armour), Some(original)) = (&mut self.base_armour, basis.base_armour) {
            *armour = (original * ratio).round().min(1.0e6);
        }
        self.level = target;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn item(level: u32) -> Item {
        let mut item = super::super::tests::item(
            SlotKind::Ring,
            vec![
                Affix {
                    id: "stamina".into(),
                    name: "Stamina".into(),
                    kind: AffixKind::Suffix,
                    group: "stamina".into(),
                    effect: Effect::Stamina,
                    value: 10.0,
                    tags: vec![],
                    proc: None,
                },
                Affix {
                    id: "chains".into(),
                    name: "Chains".into(),
                    kind: AffixKind::Prefix,
                    group: "chains".into(),
                    effect: Effect::Chain,
                    value: 2.0,
                    tags: vec![],
                    proc: None,
                },
            ],
        );
        item.level = level;
        item.legendary = Some(legendary::Roll {
            unique: 24,
            milestone: None,
        });
        item.base_damage = Some(weapons::DamageRange {
            min: 10.0,
            max: 20.0,
        });
        item.base_armour = Some(15.0);
        item
    }
    #[test]
    fn repeated_upgrades_preserve_the_original_roll_through_save_and_transfer() {
        let mut direct = item(60);
        direct.raise_item_level(110).unwrap();
        let mut repeated = item(60);
        for level in 61..=110 {
            repeated.raise_item_level(level).unwrap();
            // Trading and banking also serialize the Item, not just the inventory.
            repeated = serde_json::from_str(&serde_json::to_string(&repeated).unwrap()).unwrap();
        }
        assert_eq!(
            serde_json::to_value(&repeated).unwrap(),
            serde_json::to_value(&direct).unwrap()
        );
        assert!(repeated.affixes[0].value > 10.0);
        assert_eq!(repeated.affixes[1].value, 2.0);
        assert_eq!(repeated.legendary.unwrap().unique, 24);
        let mut old = serde_json::to_value(item(60)).unwrap();
        old.as_object_mut().unwrap().remove("upgrade_basis");
        let restored: Item = serde_json::from_value(old).unwrap();
        assert!(restored.upgrade_basis.is_none());
    }
    #[test]
    fn success_and_failure_charge_once_and_rejected_attempts_change_nothing() {
        let config = crate::survivors::dungeon_smith::config::get();
        let mut inventory = Inventory::fixture(vec![item(109), item(60)]);
        inventory.save.copper = 1_000_000 * crate::survivors::currency::GOLD;
        inventory.save.equipped.insert("Ring 1".into(), 1);
        inventory.save.auto_salvage[0] = AutoSalvageRule {
            enabled: true,
            below_ilvl: 200,
        };
        let original = serde_json::to_value(inventory.item(1)).unwrap();
        let before = inventory.money();
        let (gain, cost) = inventory.dungeon_upgrade(1, 110, config, 0.9).unwrap();
        assert_eq!(gain, 0);
        assert_eq!(inventory.money(), before - cost);
        assert_eq!(serde_json::to_value(inventory.item(1)).unwrap(), original);
        assert_eq!(inventory.revision, 1);
        let before = inventory.money();
        assert_eq!(inventory.dungeon_upgrade(1, 110, config, 0.0).unwrap().0, 1);
        assert!(inventory.money() < before);
        assert_eq!(inventory.equipped("Ring 1").unwrap().level, 110);
        assert_eq!(
            inventory.save.items.len(),
            2,
            "upgrades must not auto-sell unrelated items"
        );
        let before = serde_json::to_value(&inventory.save).unwrap();
        assert!(inventory.dungeon_upgrade(1, 110, config, 0.0).is_err());
        assert!(inventory.dungeon_upgrade(999, 110, config, 0.0).is_err());
        assert!(inventory.dungeon_upgrade(2, 110, config, f32::NAN).is_err());
        assert_eq!(serde_json::to_value(&inventory.save).unwrap(), before);
        inventory.save.copper = 0;
        let before = serde_json::to_value(&inventory.save).unwrap();
        assert!(inventory.dungeon_upgrade(2, 110, config, 0.0).is_err());
        assert_eq!(serde_json::to_value(&inventory.save).unwrap(), before);
        inventory.blocked = true;
        assert!(inventory.dungeon_upgrade(2, 110, config, 0.0).is_err());
        assert_eq!(serde_json::to_value(&inventory.save).unwrap(), before);
    }
}

impl Inventory {
    pub(in crate::survivors) fn upgrade_candidates(&self) -> impl Iterator<Item = &Item> {
        self.save.items.iter()
    }
    pub(in crate::survivors) fn dungeon_upgrade(
        &mut self,
        id: u64,
        dungeon_level: u32,
        config: &super::super::dungeon_smith::config::Config,
        roll: f32,
    ) -> Result<(u32, u64), String> {
        self.writable()?;
        if self.character_level < 60 {
            return Err("Dungeon upgrades require a level-60 character.".into());
        }
        let item = self
            .item(id)
            .ok_or("This item is no longer in your inventory.")?;
        let quote = config.quote(item.level, dungeon_level)?;
        if self.save.copper < quote.cost {
            return Err("Not enough gold.".into());
        }
        let gain = quote.roll(roll)?;
        let mut save = self.save.clone();
        save.copper -= quote.cost;
        if gain > 0 {
            let item = save
                .items
                .iter_mut()
                .find(|i| i.id == id)
                .expect("validated owned item");
            item.raise_item_level(item.level + gain)?;
        }
        // Gold and item change share one save. Do not run auto-salvage on unrelated items.
        self.persist(&save)?;
        self.save = save;
        self.money_unsaved = false;
        self.revision = self.revision.wrapping_add(1);
        Ok((gain, quote.cost))
    }
}
