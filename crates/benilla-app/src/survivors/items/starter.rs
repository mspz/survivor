//! Plain starting gear is separate from boss rolls and their mandatory affixes.
use super::*;

pub(super) fn is_plain(item: &Item) -> bool {
    item.base.starts_with("starter/")
        && item.rarity == Rarity::Normal
        && item.level == 1
        && item.affixes.is_empty()
        && item.base_block == 0.0
        && item.armour() == 0.0
}

impl Inventory {
    pub(super) fn grant_starter(&mut self, test_character: bool) -> Result<(), String> {
        if self.save.starter_granted {
            return Ok(());
        }
        if self.blocked || self.character.is_none() {
            return Err("Cannot grant starting equipment until the inventory is loaded.".into());
        }
        // Built-in bases guarantee a usable kit even when custom loot tables disable entries.
        let bases = Config::builtin();
        let armour = match self.class {
            1 | 2 => "plate",
            3 | 7 => "mail",
            4 | 11 => "leather",
            _ => "cloth",
        };
        let (weapon, slot) = match self.class {
            1 => ("sword_two_hand", "Main hand"),
            2 => ("mace", "Main hand"),
            3 => ("bow", "Ranged"),
            4 => ("dagger", "Main hand"),
            _ => ("staff", "Main hand"),
        };
        let mut kit = vec![
            (format!("{armour}_chest"), "Chest"),
            (format!("{armour}_leggings"), "Legs"),
            (weapon.to_owned(), slot),
        ];
        if self.class == 4 {
            kit.push(("dagger".into(), "Off hand"));
        }
        let mut save = self.save.clone();
        for (base_id, slot) in kit {
            let base = bases
                .bases
                .iter()
                .find(|base| base.id == base_id)
                .ok_or("Missing starting equipment base")?;
            let id = save.next_id;
            save.next_id = id.checked_add(1).ok_or("Item identity exhausted")?;
            save.items.push(Item {
                legendary: None,
                base_damage: weapons::is_weapon(base.equipment_type)
                    .then(|| weapons::base_for(base.equipment_type, base.appearance)),
                base_armour: Some(0.0),
                id,
                base: format!("starter/{}", base.id),
                name: format!("Starting {}", base.name),
                slot: base.slot,
                icon: base.icon.clone(),
                appearance: base.appearance,
                equipment_type: base.equipment_type,
                base_block: 0.0,
                loot_class: None,
                rarity: Rarity::Normal,
                level: 1,
                affixes: Vec::new(),
            });
            save.equipped.insert(slot.into(), id);
        }
        if test_character {
            for mf in [10u32, 100, 1000, 10000] {
                let id = save.next_id;
                save.next_id = id.checked_add(1).ok_or("Item identity exhausted")?;
                save.items.push(Item {
                    legendary: None,
                    base_damage: None,
                    base_armour: Some(0.0),
                    appearance: None,
                    equipment_type: EquipmentType::Universal,
                    base_block: 0.0,
                    loot_class: None,
                    id,
                    base: format!("test/magic-find/{mf}"),
                    name: format!("TEST — {mf}% Magic Find"),
                    slot: SlotKind::Trinket,
                    icon: r"Interface\Icons\INV_Jewelry_Talisman_07".into(),
                    rarity: Rarity::Normal,
                    level: 1,
                    affixes: vec![Affix {
                        id: "test_magic_find".into(),
                        name: "Magic Find".into(),
                        kind: AffixKind::Suffix,
                        group: "magic_find".into(),
                        effect: Effect::MagicFind,
                        value: mf as f32 / 100.0,
                        tags: vec![],
                        proc: None,
                    }],
                });
            }
        }
        save.starter_granted = true;
        self.commit(save)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_characters_get_four_exact_find_trinkets_once_and_they_survive_saving() {
        let mut inventory = Inventory::fixture(vec![]);
        inventory.grant_starter(true).unwrap();
        let trinkets: Vec<_> = inventory
            .bag()
            .filter(|item| item.slot == SlotKind::Trinket)
            .collect();
        assert_eq!(trinkets.len(), 4);
        for (item, expected) in trinkets.iter().zip([0.1, 1.0, 10.0, 100.0]) {
            assert_eq!(item.affixes[0].value, expected);
            assert_eq!(item.affixes[0].effect, Effect::MagicFind);
        }
        let count = inventory.all().len();
        let saved = toml::to_string(&inventory.save).unwrap();
        inventory.save = Inventory::decode(&saved).unwrap();
        inventory.grant_starter(true).unwrap();
        assert_eq!(inventory.all().len(), count);
        let id = inventory
            .bag()
            .find(|i| i.name.contains("10000"))
            .unwrap()
            .id;
        inventory.equip(id, "Trinket 1").unwrap();
        let mut run = Run::default();
        inventory.apply(&mut run);
        assert_eq!(run.equipment.magic_find, 100.0);
    }

    #[test]
    fn every_class_gets_usable_plain_gear_once_and_saved_rogues_keep_two_daggers() {
        for class in super::super::super::data::CLASSES {
            let mut inventory = Inventory::fixture(Vec::new());
            inventory.class = class.id;
            inventory.grant_starter(false).unwrap();
            let count = if class.id == 4 { 4 } else { 3 };
            assert_eq!(inventory.all().len(), count, "{}", class.name);
            assert_eq!(inventory.save.equipped.len(), count);
            assert!(inventory.equipped("Chest").is_some());
            assert!(inventory.equipped("Legs").is_some());
            for (slot, id) in &inventory.save.equipped {
                let kind = SLOTS.iter().find(|(name, _)| name == slot).unwrap().1;
                let item = inventory.item(*id).unwrap();
                assert!(item.usable(class.id, kind), "{}: {slot}", class.name);
                assert!(is_plain(item));
                assert!(item.appearance.is_some());
            }
            if class.id == 4 {
                for slot in ["Main hand", "Off hand"] {
                    assert_eq!(inventory.equipped(slot).unwrap().base, "starter/dagger");
                }
            }
            let saved = toml::to_string(&inventory.save).unwrap();
            inventory.save = Inventory::decode(&saved).unwrap();
            inventory.grant_starter(false).unwrap();
            assert_eq!(toml::to_string(&inventory.save).unwrap(), saved);
            // Unequipping does not cause a replacement kit on the next sync/login.
            inventory.save.equipped.clear();
            inventory.grant_starter(false).unwrap();
            assert!(inventory.save.equipped.is_empty());
            assert_eq!(inventory.all().len(), count);
        }
    }
}
