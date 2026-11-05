//! Intrinsic weapon damage, separate from rolled affixes and spell power.
use super::{Appearance, EquipmentType, Item};
use crate::survivors::{
    data::AbilityDef,
    spell_engine::{self, Tags},
    Rng, Run,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::survivors) struct DamageRange {
    pub min: f32,
    pub max: f32,
}
impl DamageRange {
    pub fn valid(self) -> bool {
        self.min.is_finite()
            && self.max.is_finite()
            && self.min >= 0.0
            && self.min <= self.max
            && self.max <= 1.0e6
    }
    pub(super) fn scaled(self, level: u32) -> Self {
        Self {
            min: super::armour::scaled_base(self.min, level),
            max: super::armour::scaled_base(self.max, level),
        }
    }
    fn roll(self, roll: f32) -> f32 {
        self.min + (self.max - self.min) * roll.clamp(0.0, 1.0)
    }
    fn plus(self, other: Self) -> Self {
        Self {
            min: self.min + other.min,
            max: self.max + other.max,
        }
    }
}

pub(super) fn is_weapon(kind: EquipmentType) -> bool {
    kind.one_handed()
        || kind.two_handed()
        || matches!(kind, EquipmentType::Bow | EquipmentType::Wand)
}
pub(super) fn base_for(kind: EquipmentType, appearance: Option<Appearance>) -> DamageRange {
    let (min, max) = match kind {
        EquipmentType::OneHand | EquipmentType::MeleeOneHand | EquipmentType::HybridOneHand
            if appearance.is_some_and(|a| a.subclass == 15) =>
        {
            (4.0, 7.0)
        }
        EquipmentType::OneHand | EquipmentType::MeleeOneHand | EquipmentType::HybridOneHand => {
            (6.0, 10.0)
        }
        EquipmentType::TwoHand => (12.0, 20.0),
        EquipmentType::Staff => (7.0, 12.0),
        EquipmentType::Bow => (8.0, 14.0),
        EquipmentType::Wand => (3.0, 6.0),
        _ => (0.0, 0.0),
    };
    DamageRange { min, max }
}
impl Item {
    pub fn weapon_damage(&self) -> DamageRange {
        self.base_damage
            .unwrap_or_else(|| base_for(self.equipment_type, self.appearance).scaled(self.level))
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(in crate::survivors) struct Loadout {
    melee: DamageRange,
    hands: [DamageRange; 2],
    ranged: DamageRange,
}
impl Loadout {
    pub(super) fn equip(&mut self, slot: &str, damage: DamageRange) {
        match slot {
            "Main hand" | "Off hand" => {
                self.melee = self.melee.plus(damage);
                self.hands[usize::from(slot == "Off hand")] = damage;
            }
            "Ranged" => self.ranged = damage,
            _ => {}
        }
    }
    fn for_spell(self, def: &AbilityDef, tags: Tags) -> DamageRange {
        if !tags.contains(Tags::ATTACK) || def.damage <= 0.0 {
            return DamageRange::default();
        }
        if matches!(
            def.key,
            "arcane_shot"
                | "multi_shot"
                | "serpent_sting"
                | "volley"
                | "explosive_trap"
                | "concussive_shot"
                | "aimed_shot"
        ) {
            self.ranged
        } else {
            self.melee
        }
    }
}
impl Run {
    pub(in crate::survivors) fn weapon_damage_range(&self, def: &AbilityDef) -> DamageRange {
        let (profile, tags) = super::legendary::profile(self, def);
        if self.legendary.has(279) && tags.contains(Tags::ATTACK | Tags::MELEE) && def.damage > 0.0
        {
            return self.weapon_damage.hands[self.legendary.casting.melee as usize % 2];
        }
        self.weapon_damage.for_spell(&profile, tags)
    }
    pub fn resolve_spell_for_cast(
        &self,
        def: &AbilityDef,
        level: u8,
        rng: &mut Rng,
    ) -> spell_engine::Resolved {
        let roll = if self.weapon_damage_range(def).max > 0.0 {
            rng.f32()
        } else {
            0.5
        };
        self.resolve_spell_with_weapon_roll(def, level, roll)
    }
    pub fn resolve_spell_with_weapon_roll(
        &self,
        def: &AbilityDef,
        level: u8,
        roll: f32,
    ) -> spell_engine::Resolved {
        let (profile, tags) = super::legendary::profile(self, def);
        let mut resolved = spell_engine::resolve_tagged(
            &profile,
            level,
            &self.stats(),
            self.spell_modifiers.entries(),
            self.weapon_damage_range(def).roll(roll)
                * if super::super::talent_runtime::active(
                    self,
                    super::super::talent_design::Mechanic::Weapon,
                ) {
                    1.35
                } else {
                    1.0
                },
            tags,
        );
        let multiplier = self
            .balance
            .multiplier_tagged(self.class, def.key, resolved.tags);
        resolved.dmg *= multiplier;
        resolved.dot *= multiplier;
        if (self.legendary.has(265) && def.kind == crate::survivors::data::Kind::Dot)
            || (self.legendary.has(303) && def.key == "entangling_roots")
        {
            resolved.dmg = resolved.dot;
            resolved.dot = 0.0;
        }
        if self.legendary.has(70) && resolved.tags.contains(Tags::DAMAGE) && def.damage > 0.0 {
            let direct = super::legendary::events::dot_conversion(
                self,
                def.key,
                resolved.tags,
                resolved.dmg,
            ) / 3.0;
            resolved.direct_dot = Some(spell_engine::PeriodicSnapshot {
                dps: direct,
                duration: 3.0,
                tags: resolved.tags | Tags::DOT | Tags::DURATION,
            });
            resolved.dmg = 0.0;
            resolved.crit = 0.0;
            if def.dot > 0.0 {
                let mut native_def = def.clone();
                native_def.damage = 0.0;
                let (native_def, original_tags) = super::legendary::profile(self, &native_def);
                let native = spell_engine::resolve_tagged(
                    &native_def,
                    level,
                    &self.stats(),
                    self.spell_modifiers.entries(),
                    0.0,
                    original_tags,
                );
                resolved.dot = native.dot
                    * self
                        .balance
                        .multiplier_tagged(self.class, def.key, original_tags);
                resolved.dot_tags = Some(original_tags);
            }
        }
        super::legendary::events::modify(self, &profile, &mut resolved);
        if resolved.direct_dot.is_some() {
            resolved.primary_critical = false;
        }
        resolved
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::survivors::{
        data,
        items::{
            tests::{affix, item},
            AffixKind, Effect, Inventory, SlotKind,
        },
        spell_engine::{Change, Filter, Modifier},
    };

    #[test]
    fn intelligence_weapon_damage_only_adds_to_attacks_before_multipliers_and_survives_saving() {
        let mut weapon = item(
            SlotKind::MainHand,
            vec![affix(Effect::Intelligence, 20.0, AffixKind::Prefix, vec![])],
        );
        weapon.equipment_type = EquipmentType::HybridOneHand;
        weapon.base_damage = Some(DamageRange {
            min: 6.0,
            max: 10.0,
        });
        let mut inventory = Inventory::fixture(vec![weapon]);
        inventory.class = 2;
        inventory.equip(1, "Main hand").unwrap();
        let mut run = Run {
            class: &data::CLASSES[1],
            ..Default::default()
        };
        let attack = data::ability("heroic_strike");
        let fire = data::ability("fireball");
        let heal = data::ability("rejuvenation");
        let guard = data::ability("power_word_shield");
        let attack_before = run.resolve_spell(attack, 1).dmg;
        let fire_before = run.resolve_spell(fire, 1).dmg;
        let heal_before = run.resolve_spell(heal, 1).dmg;
        let guard_before = run.resolve_spell(guard, 1).dmg;
        inventory.apply(&mut run);
        assert_eq!(
            run.resolve_spell_with_weapon_roll(attack, 1, 0.0).dmg,
            attack_before + 6.0
        );
        assert_eq!(
            run.resolve_spell_with_weapon_roll(attack, 1, 1.0).dmg,
            attack_before + 10.0
        );
        assert_eq!(run.resolve_spell(attack, 1).dmg, attack_before + 8.0);
        for roll in [0.0, 1.0] {
            assert!(
                (run.resolve_spell_with_weapon_roll(fire, 1, roll).dmg - fire_before * 1.2).abs()
                    < 0.001
            );
            assert!(
                (run.resolve_spell_with_weapon_roll(heal, 1, roll).dmg - heal_before * 1.2).abs()
                    < 0.001
            );
            assert_eq!(
                run.resolve_spell_with_weapon_roll(guard, 1, roll).dmg,
                guard_before
            );
        }
        run.replace_spell_modifiers(
            123,
            vec![
                Modifier {
                    source: 123,
                    filter: Filter::requiring(Tags::ATTACK),
                    change: Change::IncreasedDamage(0.5),
                },
                Modifier {
                    source: 123,
                    filter: Filter::requiring(Tags::ATTACK),
                    change: Change::FlatDamage(3.0),
                },
            ],
        );
        assert_eq!(
            run.resolve_spell_with_weapon_roll(attack, 1, 0.0).dmg,
            (attack_before + 6.0 + 3.0) * 1.5
        );
        let text = toml::to_string(&inventory.save).unwrap();
        let mut saved = Inventory::decode(&text).unwrap();
        assert_eq!(
            saved.items[0].weapon_damage(),
            DamageRange {
                min: 6.0,
                max: 10.0
            }
        );
        saved.items[0].base_damage = Some(DamageRange {
            min: 10.0,
            max: 2.0,
        });
        assert!(Inventory::decode(&toml::to_string(&saved).unwrap()).is_err());
        saved.items[0].base_damage = None;
        assert_eq!(
            Inventory::decode(&toml::to_string(&saved).unwrap())
                .unwrap()
                .items[0]
                .weapon_damage(),
            DamageRange {
                min: 6.0,
                max: 10.0
            }
        );
        inventory.unequip("Main hand").unwrap();
        inventory.apply(&mut run);
        assert_eq!(run.weapon_damage_range(attack), DamageRange::default());
    }

    #[test]
    fn melee_and_ranged_slots_use_separate_ranges_and_casts_roll_within_them() {
        let mut run = Run::default();
        run.weapon_damage
            .equip("Main hand", DamageRange { min: 4.0, max: 7.0 });
        run.weapon_damage
            .equip("Off hand", DamageRange { min: 4.0, max: 7.0 });
        run.weapon_damage.equip(
            "Ranged",
            DamageRange {
                min: 8.0,
                max: 14.0,
            },
        );
        assert_eq!(
            run.weapon_damage_range(data::ability("sinister_strike")),
            DamageRange {
                min: 8.0,
                max: 14.0
            }
        );
        run.weapon_damage.equip(
            "Ranged",
            DamageRange {
                min: 20.0,
                max: 30.0,
            },
        );
        assert_eq!(
            run.weapon_damage_range(data::ability("arcane_shot")),
            DamageRange {
                min: 20.0,
                max: 30.0
            }
        );
        assert_eq!(
            run.weapon_damage_range(data::ability("thunder_clap")),
            DamageRange {
                min: 8.0,
                max: 14.0
            }
        );
        for def in data::ABILITIES {
            if spell_engine::tags::for_spell(def).contains(Tags::SPELL) {
                assert_eq!(
                    run.weapon_damage_range(def),
                    DamageRange::default(),
                    "{}",
                    def.key
                );
            }
        }
        let mut rng = Rng(123);
        let attack = data::ability("heroic_strike");
        let base = attack.damage;
        let values: Vec<_> = (0..20)
            .map(|_| run.resolve_spell_for_cast(attack, 1, &mut rng).dmg)
            .collect();
        assert!(values
            .iter()
            .all(|v| (base + 8.0..=base + 14.0).contains(v)));
        assert!(values.windows(2).any(|pair| pair[0] != pair[1]));
        for kind in [
            EquipmentType::HybridOneHand,
            EquipmentType::Staff,
            EquipmentType::Wand,
            EquipmentType::Bow,
            EquipmentType::TwoHand,
        ] {
            let range = base_for(kind, None);
            assert!(range.valid() && range.min > 0.0);
        }
    }
}
