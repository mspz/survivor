//! Equipment armour and the hero's logarithmic damage reduction.
use super::{EquipmentType, Item, SlotKind};

/// Normalized logarithm: approaches 99% without a hard cap. About 5,379 armour gives
/// 75%, 27,612 gives 80%, and 450,924 gives 85%.
pub(in crate::survivors) fn reduction(armour: f32) -> f32 {
    let logarithm = (f64::from(armour.max(0.0)) / 50.0).ln_1p();
    (0.99 * (1.0 - 1.5 / (logarithm + 1.5))) as f32
}

pub(super) fn base_for(kind: EquipmentType, slot: SlotKind) -> f32 {
    let material = match kind {
        EquipmentType::Plate => 10.0,
        EquipmentType::Mail => 7.0,
        EquipmentType::Leather => 4.0,
        EquipmentType::Cloth => 2.0,
        EquipmentType::Shield => return 100.0,
        EquipmentType::Universal if slot == SlotKind::Back => return 10.0,
        _ => return 0.0,
    };
    material
        * match slot {
            SlotKind::Chest => 10.0,
            SlotKind::Head | SlotKind::Legs => 8.0,
            SlotKind::Shoulders => 6.0,
            SlotKind::Hands | SlotKind::Feet => 5.0,
            SlotKind::Waist => 4.0,
            SlotKind::Wrists => 3.0,
            _ => 0.0,
        }
}

pub(super) fn scaled_base(base: f32, level: u32) -> f32 {
    (base * super::item_level::factor(level)).round().min(1.0e6)
}

impl Item {
    pub fn armour(&self) -> f32 {
        // Keep old starting kits statless too. Other older saves acquire their material's armour.
        if self.base.starts_with("starter/") {
            return 0.0;
        }
        self.base_armour
            .unwrap_or_else(|| scaled_base(base_for(self.equipment_type, self.slot), self.level))
    }
}

impl crate::survivors::Run {
    pub(in crate::survivors) fn damage_reduction(&self) -> f32 {
        self.damage_reduction_tagged(crate::survivors::spell_engine::Tags::PHYSICAL)
    }

    pub(in crate::survivors) fn damage_reduction_tagged(
        &self,
        tags: crate::survivors::spell_engine::Tags,
    ) -> f32 {
        let stats = self.stats();
        let armour = if tags.contains(crate::survivors::spell_engine::Tags::PHYSICAL) {
            reduction(stats.armor)
        } else {
            0.0
        };
        1.0 - (1.0 - armour)
            * (1.0 - stats.damage_reduction.clamp(0.0, 0.75))
            * (1.0 - stats.resistances.reduction(tags))
            * (1.0 - self.equipment.reduction.clamp(0.0, 0.75))
            * (1.0 - super::legendary::events::reduction(self))
            * (1.0 - self.healer_buffs.bonuses.reduction)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::survivors::{hero, Run};

    #[test]
    fn logarithmic_armour_has_diminishing_returns_and_absorption_follows_mitigation() {
        assert_eq!(reduction(0.0), 0.0);
        assert_eq!(reduction(-10.0), 0.0);
        assert!((reduction(100.0) - 0.418_541_13).abs() < 0.0001);
        assert!(reduction(200.0) - reduction(100.0) < reduction(100.0));
        assert!(reduction(10_000.0) > 0.75);
        assert!(reduction(f32::MAX) < 0.99);
        assert!(
            reduction(10_000.0) - reduction(5_000.0) > reduction(30_000.0) - reduction(25_000.0)
        );
        assert!(reduction(27_612.0) > 0.7999);
        let mut run = Run {
            equipment_armour: 100.0,
            hp: 100.0,
            shield: 10.0,
            shield_t: 5.0,
            ..Default::default()
        };
        let damage = hero::hurt_hero(&mut run, 100.0);
        assert!((damage - 48.145_89).abs() < 0.001);
        assert_eq!(run.shield, 0.0);
        assert!((run.hp - 51.854_11).abs() < 0.001);
    }

    #[test]
    fn equipped_base_armour_and_affixes_stack_and_disappear_on_unequip() {
        use super::super::{
            tests::{affix, item},
            AffixKind, Effect, Inventory,
        };
        let mut piece = item(
            SlotKind::Chest,
            vec![affix(Effect::Armour, 7.0, AffixKind::Prefix, vec![])],
        );
        piece.equipment_type = EquipmentType::Plate;
        piece.base_armour = Some(100.0);
        let mut inventory = Inventory::fixture(vec![piece]);
        for base in &inventory.config.bases {
            let expected = base_for(base.equipment_type, base.slot);
            if expected > 0.0 {
                assert_eq!(base.base_armour, Some(expected), "{}", base.id);
            }
        }
        inventory.equip(1, "Chest").unwrap();
        let mut run = Run::default();
        inventory.apply(&mut run);
        assert_eq!(run.stats().armor, 107.0);
        let mut legacy = inventory.save.clone();
        legacy.items[0].base_armour = Some(-1.0);
        assert!(Inventory::decode(&toml::to_string(&legacy).unwrap()).is_err());
        legacy.items[0].base_armour = None;
        assert_eq!(
            Inventory::decode(&toml::to_string(&legacy).unwrap())
                .unwrap()
                .items[0]
                .armour(),
            100.0
        );
        inventory.unequip("Chest").unwrap();
        inventory.apply(&mut run);
        assert_eq!(run.stats().armor, 0.0);
        for slot in [
            SlotKind::Chest,
            SlotKind::Legs,
            SlotKind::Head,
            SlotKind::Shoulders,
            SlotKind::Wrists,
            SlotKind::Hands,
            SlotKind::Waist,
            SlotKind::Feet,
        ] {
            let values = [
                EquipmentType::Cloth,
                EquipmentType::Leather,
                EquipmentType::Mail,
                EquipmentType::Plate,
            ]
            .map(|kind| base_for(kind, slot));
            assert!(values.windows(2).all(|pair| pair[0] < pair[1]));
        }
    }
}
