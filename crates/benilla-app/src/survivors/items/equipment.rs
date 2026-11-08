use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::survivors) enum EquipmentType {
    #[default]
    Legacy,
    Universal,
    Plate,
    Mail,
    Leather,
    Cloth,
    OneHand,
    MeleeOneHand,
    HybridOneHand,
    TwoHand,
    Staff,
    Shield,
    HeldOffHand,
    Bow,
    Wand,
}
impl EquipmentType {
    pub fn can_equip(self, class: u8) -> bool {
        match self {
            Self::MeleeOneHand | Self::HybridOneHand => {
                matches!(class, 1 | 2 | 3 | 4 | 7 | 11)
            }
            Self::TwoHand => matches!(class, 1 | 2 | 3 | 7 | 11),
            Self::Staff => matches!(class, 2 | 5 | 7 | 8 | 9 | 11),
            Self::Plate => matches!(class, 1 | 2),
            Self::Mail => matches!(class, 1 | 2 | 3 | 7),
            Self::Leather => matches!(class, 1 | 2 | 3 | 4 | 7 | 11),
            Self::Shield => matches!(class, 1 | 2 | 7),
            Self::Wand => matches!(class, 2 | 5 | 7 | 8 | 9 | 11),
            Self::Bow => matches!(class, 1 | 2 | 3 | 4 | 7 | 11),
            _ => true,
        }
    }
    pub fn can_drop(self, class: u8) -> bool {
        self.can_equip(class)
            && match self {
                Self::Leather => !matches!(class, 1 | 2),
                Self::Cloth => matches!(class, 5 | 8 | 9 | 11),
                _ => true,
            }
    }
    pub fn valid_slot(self, slot: SlotKind) -> bool {
        match self {
            Self::Plate | Self::Mail | Self::Leather | Self::Cloth => matches!(
                slot,
                SlotKind::Head
                    | SlotKind::Shoulders
                    | SlotKind::Chest
                    | SlotKind::Wrists
                    | SlotKind::Hands
                    | SlotKind::Waist
                    | SlotKind::Legs
                    | SlotKind::Feet
            ),
            Self::OneHand | Self::MeleeOneHand | Self::HybridOneHand => {
                matches!(slot, SlotKind::MainHand | SlotKind::OffHand)
            }
            Self::TwoHand | Self::Staff => slot == SlotKind::MainHand,
            Self::Shield | Self::HeldOffHand => slot == SlotKind::OffHand,
            Self::Bow | Self::Wand => slot == SlotKind::Ranged,
            Self::Universal => !slot.weapon(),
            Self::Legacy => true,
        }
    }
    pub fn primary_allowed(self, class: u8, effect: Effect) -> bool {
        match effect {
            Effect::Stamina => true,
            Effect::Strength => {
                matches!(class, 1 | 2)
                    && !matches!(self, Self::Leather | Self::Cloth | Self::Wand | Self::Staff)
            }
            Effect::Agility => {
                matches!(class, 3 | 4 | 7 | 11)
                    && !matches!(self, Self::Plate | Self::Cloth | Self::Wand | Self::Staff)
            }
            Effect::Intelligence => {
                if class == 2 {
                    return true;
                }
                if self == Self::HybridOneHand {
                    return matches!(class, 2 | 7);
                }
                !matches!(self, Self::Bow | Self::MeleeOneHand | Self::TwoHand)
                    && matches!(class, 2 | 5 | 7 | 8 | 9 | 11)
            }
            _ => false,
        }
    }
    /// Existing strength gear stays usable; new paladin drops use intelligence.
    pub fn drop_primary_allowed(self, class: u8, effect: Effect) -> bool {
        self.primary_allowed(class, effect) && !(class == 2 && effect == Effect::Strength)
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Legacy => "Legacy equipment",
            Self::Universal => "Accessory",
            Self::Plate => "Plate",
            Self::Mail => "Mail",
            Self::Leather => "Leather",
            Self::Cloth => "Cloth",
            Self::OneHand | Self::MeleeOneHand | Self::HybridOneHand => "One-handed weapon",
            Self::TwoHand => "Two-handed melee weapon",
            Self::Staff => "Two-handed caster staff",
            Self::Shield => "Shield",
            Self::HeldOffHand => "Held in off-hand",
            Self::Bow => "Bow",
            Self::Wand => "Wand",
        }
    }
    pub fn one_handed(self) -> bool {
        matches!(
            self,
            Self::OneHand | Self::MeleeOneHand | Self::HybridOneHand
        )
    }
    pub fn two_handed(self) -> bool {
        matches!(self, Self::TwoHand | Self::Staff)
    }
}
pub(super) fn dual_wield(class: u8) -> bool {
    matches!(class, 1 | 3 | 4 | 7)
}

impl Effect {
    pub(super) fn valid_equipment_type(self, kind: EquipmentType) -> bool {
        match self {
            Self::BlockChance | Self::LifeOnBlock | Self::LifeOnBlockPercent => {
                kind == EquipmentType::Shield
            }
            Self::FlatDamage | Self::AttackSpeed | Self::LifeLeech => {
                !matches!(kind, EquipmentType::Shield | EquipmentType::Staff)
            }
            _ => true,
        }
    }
}

impl Item {
    pub fn required_level(&self) -> u32 { self.level.saturating_sub(5).clamp(1, 60) }
    /// Old saved rolls above the base cap remain readable, but cannot grant extra block.
    pub fn intrinsic_block(&self) -> f32 {
        if self.equipment_type == EquipmentType::Shield {
            self.base_block.clamp(0.0, MAX_BASE_BLOCK)
        } else {
            0.0
        }
    }
    pub fn fits(&self, slot: SlotKind) -> bool {
        if self.equipment_type.one_handed() {
            matches!(slot, SlotKind::MainHand | SlotKind::OffHand)
        } else {
            self.slot == slot
        }
    }
    pub fn usable(&self, class: u8, slot: SlotKind) -> bool {
        self.fits(slot)
            && self.equipment_type.can_equip(class)
            && !(self.equipment_type.one_handed()
                && slot == SlotKind::OffHand
                && !dual_wield(class))
    }
    pub fn valid_equipment(&self) -> bool {
        self.equipment_type.valid_slot(self.slot)
            && self
                .base_damage
                .is_none_or(|range| range.valid() && weapons::is_weapon(self.equipment_type))
            && self
                .base_armour
                .is_none_or(|v| v.is_finite() && (0.0..=1.0e6).contains(&v))
            && self.base_block.is_finite()
            && (0.0..=1.0).contains(&self.base_block)
            && (self.equipment_type == EquipmentType::Shield || self.base_block == 0.0)
            && self.affixes.iter().all(|a| {
                a.effect != Effect::BlockChance || self.equipment_type == EquipmentType::Shield
            })
            && self
                .affixes
                .iter()
                .all(|a| a.effect.valid_equipment_type(self.equipment_type))
            && self.loot_class.is_none_or(|class| {
                // Keep previously saved rogue two-handers readable; equip rules still reject them.
                (self.equipment_type.can_drop(class)
                    || (class == 4 && self.equipment_type == EquipmentType::TwoHand))
                    && self
                        .affixes
                        .iter()
                        .filter(|a| a.effect.primary())
                        .all(|a| self.equipment_type.primary_allowed(class, a.effect))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn required_level_tracks_ilvl_and_blocks_equipping_until_character_reaches_it() {
        let mut item = super::super::tests::item(SlotKind::Head, vec![]);
        for (ilvl, required) in [(1,1),(5,1),(6,1),(22,17),(60,55),(65,60),(100,60),(u32::MAX,60)] {
            item.level = ilvl; assert_eq!(item.required_level(), required);
        }
        item.level = 22;
        let mut inventory = Inventory::fixture(vec![item]);
        inventory.character_level = 16;
        assert!(!inventory.equippable(inventory.item(1).unwrap()));
        assert!(inventory.equip(1, "Head").is_err());
        assert!(inventory.item(1).unwrap().tooltip_lines(&inventory).iter().any(|(line,color)| line == "Niveau requis : 17" && *color == Color::srgb(1.0,0.15,0.15)));
        inventory.character_level = 17;
        inventory.equip(1, "Head").unwrap();
        inventory.character_level = 16;
        assert!(inventory.validate_loadout(&inventory.save).is_err());
        #[cfg(not(target_arch = "wasm32"))] {
            let save = inventory.shared_save().unwrap();
            assert!(Inventory::from_shared(1, 16, &save).unwrap().save.equipped.is_empty());
            assert_eq!(Inventory::from_shared(1, 17, &save).unwrap().save.equipped.len(), 1);
        }
    }
    #[test]
    fn armour_shield_wand_and_dual_wield_permissions_follow_class() {
        for (kind, allowed) in [
            (EquipmentType::Plate, &[1, 2][..]),
            (EquipmentType::Mail, &[1, 2, 3, 7][..]),
            (EquipmentType::Leather, &[1, 2, 3, 4, 7, 11][..]),
            (EquipmentType::Cloth, &[1, 2, 3, 4, 5, 7, 8, 9, 11][..]),
            (EquipmentType::Shield, &[1, 2, 7][..]),
            (EquipmentType::Wand, &[2, 5, 7, 8, 9, 11][..]),
            (EquipmentType::Bow, &[1, 2, 3, 4, 7, 11][..]),
            (EquipmentType::MeleeOneHand, &[1, 2, 3, 4, 7, 11][..]),
            (EquipmentType::HybridOneHand, &[1, 2, 3, 4, 7, 11][..]),
            (EquipmentType::TwoHand, &[1, 2, 3, 7, 11][..]),
            (EquipmentType::Staff, &[2, 5, 7, 8, 9, 11][..]),
        ] {
            for class in super::super::super::data::CLASSES {
                assert_eq!(
                    kind.can_equip(class.id),
                    allowed.contains(&class.id),
                    "{kind:?} {}",
                    class.name
                );
                let mut weapon = super::super::tests::item(SlotKind::MainHand, vec![]);
                weapon.equipment_type = EquipmentType::OneHand;
                assert!(weapon.usable(class.id, SlotKind::MainHand));
                assert_eq!(
                    weapon.usable(class.id, SlotKind::OffHand),
                    [1, 3, 4, 7].contains(&class.id)
                );
            }
        }
    }

    #[test]
    fn titan_grip_equips_and_round_trips_two_two_handed_weapons() {
        let mut weapon = super::super::tests::item(
            SlotKind::MainHand,
            vec![super::super::tests::affix(
                Effect::Stamina,
                20.0,
                AffixKind::Prefix,
                vec![],
            )],
        );
        weapon.equipment_type = EquipmentType::TwoHand;
        let mut inventory = Inventory::fixture(vec![weapon.clone(), weapon]);
        assert!(inventory.equip(2, "Off hand").is_err());
        inventory.save.titan_grip = true;
        inventory.native_titan_grip = true;
        inventory.equip(1, "Main hand").unwrap();
        inventory.equip(2, "Off hand").unwrap();
        let text = toml::to_string(&inventory.save).unwrap();
        let save = Inventory::decode(&text).unwrap();
        assert_eq!(save.equipped.get("Main hand"), Some(&1));
        assert_eq!(save.equipped.get("Off hand"), Some(&2));
        inventory.class = 4;
        assert!(!inventory.can_equip(inventory.item(2).unwrap(), SlotKind::OffHand));
    }
    #[test]
    fn melee_weapons_use_class_attack_stats_and_caster_staves_use_intelligence() {
        for class in super::super::super::data::CLASSES {
            for kind in [EquipmentType::MeleeOneHand, EquipmentType::TwoHand] {
                assert_eq!(
                    kind.primary_allowed(class.id, Effect::Intelligence),
                    class.id == 2
                );
                if [1, 2].contains(&class.id) {
                    assert!(kind.primary_allowed(class.id, Effect::Strength));
                    assert!(!kind.primary_allowed(class.id, Effect::Agility));
                } else if [3, 4, 7, 11].contains(&class.id) {
                    assert!(kind.primary_allowed(class.id, Effect::Agility));
                    assert!(!kind.primary_allowed(class.id, Effect::Strength));
                }
            }
            assert!(!EquipmentType::Staff.primary_allowed(class.id, Effect::Strength));
            assert!(!EquipmentType::Staff.primary_allowed(class.id, Effect::Agility));
            assert_eq!(
                EquipmentType::Staff.primary_allowed(class.id, Effect::Intelligence),
                [2, 5, 7, 8, 9, 11].contains(&class.id)
            );
        }
    }
    #[test]
    fn two_handed_weapons_and_staves_clear_and_exclude_the_off_hand_without_losing_items() {
        let make = |slot, kind| {
            let mut item = super::super::tests::item(
                slot,
                vec![super::super::tests::affix(
                    Effect::Stamina,
                    20.0,
                    AffixKind::Prefix,
                    vec![],
                )],
            );
            item.equipment_type = kind;
            item
        };
        let mut inv = Inventory::fixture(vec![
            make(SlotKind::MainHand, EquipmentType::MeleeOneHand),
            make(SlotKind::MainHand, EquipmentType::TwoHand),
            make(SlotKind::MainHand, EquipmentType::Staff),
            make(SlotKind::OffHand, EquipmentType::Shield),
        ]);
        inv.class = 2;
        inv.equip(1, "Main hand").unwrap();
        inv.equip(4, "Off hand").unwrap();
        inv.quick_equip(2).unwrap();
        assert_eq!(inv.equipped("Main hand").unwrap().id, 2);
        assert!(inv.equipped("Off hand").is_none());
        assert_eq!(inv.all().len(), 4);
        assert_eq!(inv.bag().count(), 3);
        let before = toml::to_string(&inv.save).unwrap();
        assert!(Inventory::decode(&before).is_ok());
        assert!(inv.equip(4, "Off hand").is_err());
        assert!(inv.drop_on_equipment(4, "Off hand").is_err());
        assert!(inv.drop_on_equipment(2, "Off hand").is_err());
        assert_eq!(toml::to_string(&inv.save).unwrap(), before);
        let mut invalid = inv.save.clone();
        invalid.equipped.insert("Off hand".into(), 4);
        assert!(Inventory::decode(&toml::to_string(&invalid).unwrap()).is_err());
        inv.equip(1, "Main hand").unwrap();
        inv.equip(4, "Off hand").unwrap();
        inv.drop_on_equipment(3, "Main hand").unwrap();
        assert_eq!(inv.equipped("Main hand").unwrap().id, 3);
        assert!(inv.equipped("Off hand").is_none());
        assert!(inv.equip(4, "Off hand").is_err());
        inv.quick_equip(3).unwrap();
        inv.equip(4, "Off hand").unwrap();
        assert_eq!(inv.all().len(), 4);
    }
    #[test]
    fn hybrid_ranged_weapons_roll_attack_or_spell_primaries() {
        for (class, attack) in [(2, Effect::Strength), (7, Effect::Agility)] {
            assert!(EquipmentType::Bow.can_equip(class));
            assert!(EquipmentType::Wand.can_equip(class));
            assert!(EquipmentType::Bow.primary_allowed(class, attack));
            assert_eq!(
                EquipmentType::Bow.primary_allowed(class, Effect::Intelligence),
                class == 2
            );
            assert!(EquipmentType::Wand.primary_allowed(class, Effect::Intelligence));
            assert!(!EquipmentType::Wand.primary_allowed(class, attack));
        }
    }
    #[test]
    fn equipment_actions_and_swaps_enforce_permissions_atomically() {
        let mut weapon = super::super::tests::item(SlotKind::MainHand, vec![]);
        weapon.equipment_type = EquipmentType::OneHand;
        let mut shield = super::super::tests::item(SlotKind::OffHand, vec![]);
        shield.equipment_type = EquipmentType::Shield;
        shield.base_block = 0.05;
        let mut plate = super::super::tests::item(SlotKind::Chest, vec![]);
        plate.equipment_type = EquipmentType::Plate;
        let mut inv = Inventory::fixture(vec![weapon.clone(), weapon, shield, plate]);
        inv.class = 2;
        inv.equip(1, "Main hand").unwrap();
        assert!(inv.equip(2, "Off hand").is_err());
        inv.equip(3, "Off hand").unwrap();
        let before = toml::to_string(&inv.save).unwrap();
        assert!(inv.drop_on_equipment(1, "Off hand").is_err());
        assert_eq!(toml::to_string(&inv.save).unwrap(), before);
        inv.unequip("Off hand").unwrap();
        inv.class = 4;
        assert!(inv.quick_equip(3).is_err());
        assert!(inv.equip(4, "Chest").is_err());
        inv.equip(2, "Off hand").unwrap();
        inv.drop_on_equipment(1, "Off hand").unwrap();
        assert_eq!(inv.equipped("Off hand").unwrap().id, 1);
        assert_eq!(inv.equipped("Main hand").unwrap().id, 2);
    }
    #[test]
    fn old_shields_with_excessive_base_block_use_the_cap_in_combat_and_tooltips() {
        use super::super::tests::{affix, item};
        let mut old = item(
            SlotKind::OffHand,
            vec![affix(Effect::BlockChance, 0.05, AffixKind::Suffix, vec![])],
        );
        old.equipment_type = EquipmentType::Shield;
        old.base_block = 1.0;
        let mut replacement = old.clone();
        replacement.base_block = 0.25;
        let mut inv = Inventory::fixture(vec![old, replacement]);
        inv.save = Inventory::decode(&toml::to_string(&inv.save).unwrap()).unwrap();
        inv.equip(1, "Off hand").unwrap();
        let mut run = Run::default();
        inv.apply(&mut run);
        assert!((run.equipment.block - 0.35).abs() < 0.0001);
        assert!(inv
            .item(1)
            .unwrap()
            .tooltip_lines(&inv)
            .iter()
            .any(|(line, _)| line == "30.0% Block chance"));
        assert!(inv
            .item(2)
            .unwrap()
            .comparison_lines(&inv)
            .iter()
            .any(|(line, _)| line == "-5.0% Block chance"));
    }

    #[test]
    fn shield_base_and_secondary_block_stack_and_unequipping_removes_them() {
        let affix =
            super::super::tests::affix(Effect::BlockChance, 0.05, AffixKind::Suffix, vec![]);
        let mut shield = super::super::tests::item(SlotKind::OffHand, vec![affix]);
        shield.equipment_type = EquipmentType::Shield;
        shield.base_block = 0.05;
        let mut inv = Inventory::fixture(vec![shield]);
        inv.equip(1, "Off hand").unwrap();
        let mut run = Run::default();
        inv.apply(&mut run);
        assert!((run.equipment.block - 0.1).abs() < 0.0001);
        run.shield = 20.0;
        run.shield_t = 10.0;
        let hp = run.hp;
        assert_eq!(
            super::super::super::hero::hurt_hero_with_block(&mut run, 50.0, 0.09),
            0.0
        );
        assert_eq!(run.hp, hp);
        assert_eq!(run.shield, 20.0);
        assert_eq!(run.stats().armor, 100.0);
        let damage = super::super::super::hero::hurt_hero_with_block(&mut run, 50.0, 0.11);
        assert!((damage - 9.072_944).abs() < 0.001);
        inv.unequip("Off hand").unwrap();
        inv.apply(&mut run);
        assert_eq!(run.equipment.block, 0.0);
        assert_eq!(
            super::super::super::hero::hurt_hero_with_block(&mut run, 50.0, 0.0),
            50.0
        );
    }

    #[test]
    fn shield_block_caps_at_seventy_five_percent_in_stats_and_combat() {
        let affix = super::super::tests::affix(Effect::BlockChance, 0.5, AffixKind::Suffix, vec![]);
        let mut shield = super::super::tests::item(SlotKind::OffHand, vec![affix]);
        shield.equipment_type = EquipmentType::Shield;
        shield.base_block = 0.4;
        let mut inv = Inventory::fixture(vec![shield]);
        inv.equip(1, "Off hand").unwrap();
        let mut run = Run::default();
        inv.apply(&mut run);
        assert_eq!(run.equipment.block, 0.75);
        assert_eq!(
            super::super::super::hero::hurt_hero_with_block(&mut run, 10.0, 0.749),
            0.0
        );
        assert!(super::super::super::hero::hurt_hero_with_block(&mut run, 10.0, 0.75) > 0.0);
        run.equipment.block = 1.0;
        assert!(super::super::super::hero::hurt_hero_with_block(&mut run, 10.0, 0.8) > 0.0);
    }
    #[test]
    fn life_on_block_affixes_heal_only_successful_blocks_and_stop_when_unequipped() {
        use super::super::super::hero::hurt_hero_with_block;
        use super::super::tests::{affix, item};
        let mut shield = item(
            SlotKind::OffHand,
            vec![
                affix(Effect::LifeOnBlock, 3.0, AffixKind::Prefix, vec![]),
                affix(Effect::LifeOnBlockPercent, 0.01, AffixKind::Suffix, vec![]),
            ],
        );
        shield.equipment_type = EquipmentType::Shield;
        shield.base_block = 0.4;
        let mut inv = Inventory::fixture(vec![shield]);
        inv.equip(1, "Off hand").unwrap();
        let mut run = Run::default();
        inv.apply(&mut run);
        assert_eq!(run.equipment.life_on_block, 3.0);
        assert_eq!(run.equipment.life_on_block_percent, 0.01);
        run.hp = 10.0;
        let expected = 10.0 + 3.0 + run.stats().max_hp * 0.01;
        assert_eq!(hurt_hero_with_block(&mut run, 10.0, 0.0), 0.0);
        assert!((run.hp - expected).abs() < 0.001);
        run.hp = run.stats().max_hp - 1.0;
        hurt_hero_with_block(&mut run, 10.0, 0.0);
        assert_eq!(run.hp, run.stats().max_hp);
        run.hp = 20.0;
        hurt_hero_with_block(&mut run, 0.0, 0.0);
        // Non-blocked hits retain the combat engine's minimum damage of one.
        assert_eq!(run.hp, 19.0);
        hurt_hero_with_block(&mut run, 1.0, 0.9);
        assert!(run.hp < 19.0);
        inv.unequip("Off hand").unwrap();
        inv.apply(&mut run);
        assert_eq!(run.equipment.life_on_block, 0.0);
        assert_eq!(run.equipment.life_on_block_percent, 0.0);
    }
    #[test]
    fn saves_reject_block_on_weapons_and_preserve_legacy_items() {
        let mut shield = super::super::tests::item(
            SlotKind::OffHand,
            vec![super::super::tests::affix(
                Effect::BlockChance,
                0.03,
                AffixKind::Suffix,
                vec![],
            )],
        );
        shield.equipment_type = EquipmentType::Shield;
        shield.base_block = 0.05;
        let mut inv = Inventory::fixture(vec![shield]);
        let encoded = toml::to_string(&inv.save).unwrap();
        assert!(Inventory::decode(&encoded).is_ok());
        inv.save.items[0].equipment_type = EquipmentType::OneHand;
        assert!(Inventory::decode(&toml::to_string(&inv.save).unwrap()).is_err());
        inv.save.items[0].base_block = 0.0;
        assert!(Inventory::decode(&toml::to_string(&inv.save).unwrap()).is_err());
        let legacy = Inventory::fixture(vec![super::super::tests::item(
            SlotKind::Head,
            vec![super::super::tests::affix(
                Effect::Life,
                20.0,
                AffixKind::Prefix,
                vec![],
            )],
        )]);
        let text = toml::to_string(&legacy.save).unwrap();
        let old = text
            .lines()
            .filter(|line| {
                !line.starts_with("equipment_type =") && !line.starts_with("base_block =")
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            Inventory::decode(&old).unwrap().items[0].equipment_type,
            EquipmentType::Legacy
        );
    }
}

impl Inventory {
    pub(super) fn clear_off_hand_for_two_hander(save: &mut Save, id: u64, slot: &str) {
        if slot == "Main hand"
            && save.items.iter().any(|i| {
                i.id == id
                    && i.equipment_type.two_handed()
                    && (!save.titan_grip || i.equipment_type == EquipmentType::Staff)
            })
        {
            save.equipped.remove("Off hand");
        }
    }
    pub fn can_equip(&self, item: &Item, slot: SlotKind) -> bool {
        self.character_level >= item.required_level()
            && (item.usable(self.class, slot)
                || item.equipment_type.can_equip(self.class) && self.save.titan_grip
                    && item.equipment_type == EquipmentType::TwoHand && slot == SlotKind::OffHand)
    }
    /// Whether the current class can wear `item` in at least one slot.
    pub fn equippable(&self, item: &Item) -> bool {
        SLOTS.iter().any(|(_, slot)| self.can_equip(item, *slot))
    }
    pub(super) fn validate_loadout(&self, save: &Save) -> Result<(), String> {
        legendary::validate_loadout(save)?;
        save.validate_hands()?;
        for (name, id) in &save.equipped {
            let item = save
                .items
                .iter()
                .find(|i| i.id == *id)
                .ok_or("Equipped item missing")?;
            let slot = SLOTS
                .iter()
                .find(|(s, _)| s == name)
                .map(|(_, s)| *s)
                .ok_or("Invalid equipment slot")?;
            if self.character_level < item.required_level() { return Err(format!("Niveau {} requis pour equiper cet objet.", item.required_level())); }
            if !item.usable(self.class, slot)
                && !(save.titan_grip
                    && slot == SlotKind::OffHand
                    && item.equipment_type == EquipmentType::TwoHand
                    && item.equipment_type.can_equip(self.class))
            {
                return Err("This class cannot equip this item in that slot.".into());
            }
        }
        Ok(())
    }
}

impl Save {
    pub(super) fn validate_hands(&self) -> Result<(), String> {
        if self.equipped.contains_key("Off hand")
            && self.equipped.get("Main hand").is_some_and(|id| {
                self.items.iter().any(|i| {
                    i.id == *id
                        && i.equipment_type.two_handed()
                        && (!self.titan_grip || i.equipment_type == EquipmentType::Staff)
                })
            })
        {
            return Err("Unequip the two-handed weapon before equipping an off-hand item.".into());
        }
        Ok(())
    }
}
