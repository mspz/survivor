//! Compact item text shared by the bag inspector and hover tooltip.
use super::*;

impl Affix {
    pub fn description(&self) -> String {
        let flat = match self.effect {
            Effect::Stamina => Some("Stamina"),
            Effect::Strength => Some("Strength"),
            Effect::Agility => Some("Agility"),
            Effect::Intelligence => Some("Intelligence"),
            Effect::Life => Some("Health"),
            Effect::Armour => Some("Armour"),
            Effect::FireResistance => Some("Fire resistance"),
            Effect::ColdResistance => Some("Cold resistance"),
            Effect::LightningResistance => Some("Lightning resistance"),
            Effect::NatureResistance => Some("Nature resistance"),
            Effect::ArcaneResistance => Some("Arcane resistance"),
            Effect::HolyResistance => Some("Holy resistance"),
            Effect::ShadowResistance => Some("Shadow resistance"),
            Effect::AllResistance => Some("All resistances"),
            Effect::LifeOnKill => Some("Life on kill"),
            Effect::LifeOnBlock => Some("Life on block"),
            Effect::FlatDamage => Some("Damage"),
            Effect::SpellDamage => Some("Spell damage"),
            Effect::Projectiles => Some("Projectiles"),
            Effect::Chain => Some("Chain jumps"),
            Effect::Fork => Some("Forks"),
            Effect::MeleeTargets => Some("Melee targets"),
            _ => None,
        };
        let value = if let Some(label) = flat {
            format!(
                "+{:.0}{} {label}",
                self.value,
                if self.effect.resistance() { "%" } else { "" }
            )
        } else if self.effect == Effect::Regeneration {
            format!("+{:.1} Health/s", self.value)
        } else if self.effect == Effect::Proc {
            return self.proc.as_ref().map_or_else(String::new, |p| {
                let effect = match p.kind {
                    ProcKind::ClassSpell => p.spell.as_deref().unwrap_or("Spell").replace('_', " "),
                    ProcKind::Damage => format!("+{:.0}% Damage", p.power * 100.0),
                    ProcKind::CritChance => format!("+{:.0}% Critical strike", p.power * 100.0),
                    ProcKind::AttackSpeed => format!("+{:.0}% Attack speed", p.power * 100.0),
                };
                if matches!(p.kind, ProcKind::ClassSpell) {
                    format!("On hit: {:.0}% chance to cast {effect}", self.value * 100.0)
                } else {
                    format!(
                        "On hit: {:.0}% chance for {effect} ({:.0}s)",
                        self.value * 100.0,
                        p.duration
                    )
                }
            });
        } else {
            let label = match self.effect {
                Effect::BlockChance => "Increased chance to block",
                Effect::LifeOnBlockPercent => "Maximum life recovered on block",
                Effect::Damage => "Damage",
                Effect::DotDamage => "Damage over time",
                Effect::CritChance => "Critical strike",
                Effect::CritMultiplier => "Critical damage",
                Effect::CooldownRecovery => "Cooldown recovery",
                Effect::Area => "Area of effect",
                Effect::Duration => "Duration",
                Effect::Repeat => "Repeat chance",
                Effect::AttackSpeed => "Attack speed",
                Effect::LifeLeech => "Life leech",
                Effect::MovementSpeed => "Movement speed",
                Effect::DamageReduction => "Damage reduction",
                Effect::HealingEffectiveness => "Healing received",
                Effect::BossDamage => "Boss damage",
                Effect::FullLifeDamage => "Damage against full health",
                Effect::LowLifeDamage => "Damage against low health",
                Effect::BleedChance => "Bleed chance",
                Effect::IgniteChance => "Ignite chance",
                Effect::ChillChance => "Chill chance",
                Effect::KnockbackChance => "Knockback chance",
                Effect::ArmourPenetration => "Armour penetration",
                Effect::PickupRadius => "Pickup radius",
                Effect::Experience => "Experience",
                Effect::Luck => "Luck",
                Effect::MagicFind => "Magic Find",
                _ => unreachable!("flat effects handled above"),
            };
            format!("+{:.1}% {label}", self.value * 100.0)
        };
        if self.tags.is_empty() {
            value
        } else {
            format!(
                "{value} ({})",
                self.tags
                    .iter()
                    .map(|tag| tag.label())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    }
}

impl Item {
    /// Compare the complete loadout before and after the same swap as quick-equip.
    pub fn comparison_lines(&self, inventory: &Inventory) -> Vec<(String, Color)> {
        if inventory.worn(self.id) {
            return Vec::new();
        }
        let Some((slot, _)) = SLOTS
            .iter()
            .find(|(_, kind)| self.usable(inventory.class, *kind))
        else {
            return Vec::new();
        };
        let mut after = inventory.save.clone();
        after.equipped.insert((*slot).to_owned(), self.id);
        Inventory::clear_off_hand_for_two_hander(&mut after, self.id, slot);
        fn values(save: &Save) -> std::collections::BTreeMap<String, (f32, bool)> {
            let mut values = std::collections::BTreeMap::new();
            for id in save.equipped.values() {
                let Some(item) = save.items.iter().find(|item| item.id == *id) else {
                    continue;
                };
                let mut add = |label: String, value: f32, percent: bool| {
                    let entry = values.entry(label).or_insert((0.0, percent));
                    entry.0 += value;
                };
                add("Armour".into(), item.armour(), false);
                add("Block chance".into(), item.intrinsic_block() * 100.0, true);
                let damage = item.weapon_damage();
                add("Minimum weapon damage".into(), damage.min, false);
                add("Maximum weapon damage".into(), damage.max, false);
                for affix in &item.affixes {
                    if affix.effect == Effect::BlockChance {
                        add("Block chance".into(), affix.value * 100.0, true);
                        continue;
                    }
                    let mut unit = affix.clone();
                    unit.value = 1.0;
                    let description = unit.description();
                    if affix.effect == Effect::Proc {
                        unit.value = 0.0;
                        add(
                            unit.description().replace("0% chance", "Proc chance"),
                            affix.value * 100.0,
                            true,
                        );
                    } else if let Some((number, label)) = description.split_once(' ') {
                        let percent = number.contains('%');
                        add(
                            label.to_owned(),
                            affix.value
                                * if percent && !affix.effect.resistance() {
                                    100.0
                                } else {
                                    1.0
                                },
                            percent,
                        );
                    }
                }
            }
            values
        }
        let before = values(&inventory.save);
        let after = values(&after);
        let keys: std::collections::BTreeSet<_> = before.keys().chain(after.keys()).collect();
        let mut lines = Vec::new();
        let mut losses = Vec::new();
        for key in keys {
            let old = before.get(key).copied().unwrap_or_default();
            let new = after.get(key).copied().unwrap_or_default();
            let delta = new.0 - old.0;
            if delta.abs() < 0.0001 {
                continue;
            }
            let group = if delta > 0.0 { &mut lines } else { &mut losses };
            group.push((
                format!("{delta:+.1}{} {key}", if old.1 || new.1 { "%" } else { "" }),
                if delta > 0.0 {
                    Color::srgb(0.0, 1.0, 0.0)
                } else {
                    Color::srgb(1.0, 0.15, 0.15)
                },
            ));
        }
        lines.extend(losses);
        if !lines.is_empty() {
            lines.insert(0, ("Compared to equipped".into(), Color::WHITE));
        }
        lines
    }

    pub fn tooltip_lines(&self, inventory: &Inventory) -> Vec<(String, Color)> {
        let slot = match self.slot {
            SlotKind::Head => "Head",
            SlotKind::Neck => "Neck",
            SlotKind::Shoulders => "Shoulders",
            SlotKind::Back => "Back",
            SlotKind::Chest => "Chest",
            SlotKind::Shirt => "Shirt",
            SlotKind::Tabard => "Tabard",
            SlotKind::Wrists => "Wrists",
            SlotKind::Hands => "Hands",
            SlotKind::Waist => "Waist",
            SlotKind::Legs => "Legs",
            SlotKind::Feet => "Feet",
            SlotKind::Ring => "Finger",
            SlotKind::Trinket => "Trinket",
            SlotKind::MainHand if self.equipment_type.two_handed() => "Two-Hand",
            SlotKind::MainHand if self.equipment_type.one_handed() => "One-Hand",
            SlotKind::MainHand => "Main Hand",
            SlotKind::OffHand => "Off Hand",
            SlotKind::Ranged => "Ranged",
        };
        let kind = if self.equipment_type == EquipmentType::Legacy
            || self.equipment_type == EquipmentType::Universal
        {
            String::new()
        } else if let Some(appearance) = self.appearance.filter(|a| a.item_class == 2) {
            match appearance.subclass {
                0 | 1 => "Axe",
                2 => "Bow",
                4 | 5 => "Mace",
                6 => "Polearm",
                7 | 8 => "Sword",
                10 => "Staff",
                15 => "Dagger",
                19 => "Wand",
                _ => self.equipment_type.label(),
            }
            .to_owned()
        } else {
            self.equipment_type.label().to_owned()
        };
        let mut lines = vec![
            (self.name.clone(), self.rarity.color()),
            (format!("Ilvl {}", self.level), Color::WHITE),
            (
                format!("{slot}    {kind}").trim_end().to_owned(),
                Color::WHITE,
            ),
        ];
        if self.armour() > 0.0 {
            lines.push((format!("{:.0} Armour", self.armour()), Color::WHITE));
        }
        if weapons::is_weapon(self.equipment_type) {
            let damage = self.weapon_damage();
            lines.push((
                format!("{:.0}-{:.0} Damage", damage.min, damage.max),
                Color::WHITE,
            ));
        }
        if self.intrinsic_block() > 0.0 {
            lines.push((
                format!("{:.1}% Block chance", self.intrinsic_block() * 100.0),
                Color::WHITE,
            ));
        }
        if let Some(roll) = &self.legendary {
            if let Some(d) = legendary::definition(roll.unique) {
                lines.push(("Unique equipped".into(), self.rarity.color()));
                lines.push((d.description.into(), self.rarity.color()));
                if let Some(option) = roll.milestone {
                    lines.push((
                        legendary::milestones::description(option),
                        self.rarity.color(),
                    ));
                }
            }
        }
        for (kind, label) in [
            (AffixKind::Prefix, "Prefixes"),
            (AffixKind::Suffix, "Suffixes"),
        ] {
            if self.affixes.iter().any(|a| a.kind == kind) {
                lines.push((label.into(), Color::WHITE));
            }
            for affix in self.affixes.iter().filter(|a| a.kind == kind) {
                lines.push((affix.description(), Color::WHITE));
            }
        }
        if !inventory.equippable(self) {
            lines.push((
                "Cannot equip this item".into(),
                Color::srgb(1.0, 0.15, 0.15),
            ));
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{affix, item};
    use super::*;

    #[test]
    fn resistance_tooltips_and_comparisons_use_percentage_points() {
        assert_eq!(
            affix(Effect::AllResistance, 4.0, AffixKind::Suffix, vec![]).description(),
            "+4% All resistances"
        );
        let resistance = |value| {
            item(
                SlotKind::Chest,
                vec![affix(
                    Effect::FireResistance,
                    value,
                    AffixKind::Prefix,
                    vec![],
                )],
            )
        };
        let mut inv = Inventory::fixture(vec![resistance(20.0), resistance(30.0)]);
        assert_eq!(
            inv.item(1).unwrap().affixes[0].description(),
            "+20% Fire resistance"
        );
        inv.equip(1, "Chest").unwrap();
        let lines = inv.item(2).unwrap().comparison_lines(&inv);
        assert!(lines.contains(&("+10.0% Fire resistance".into(), Color::srgb(0.0, 1.0, 0.0))));
    }

    #[test]
    fn comparison_includes_missing_stats_and_aggregates_duplicate_affixes() {
        let old = item(
            SlotKind::Chest,
            vec![
                affix(Effect::Stamina, 12.0, AffixKind::Prefix, vec![]),
                affix(Effect::Luck, 0.05, AffixKind::Suffix, vec![]),
            ],
        );
        let new = item(
            SlotKind::Chest,
            vec![
                affix(Effect::Strength, 8.0, AffixKind::Prefix, vec![]),
                affix(Effect::Strength, 3.0, AffixKind::Prefix, vec![]),
            ],
        );
        let mut inv = Inventory::fixture(vec![old, new]);
        inv.equip(1, "Chest").unwrap();
        let lines = inv.item(2).unwrap().comparison_lines(&inv);
        assert!(lines.contains(&("-12.0 Stamina".into(), Color::srgb(1.0, 0.15, 0.15))));
        assert!(lines.contains(&("-5.0% Luck".into(), Color::srgb(1.0, 0.15, 0.15))));
        assert!(lines.contains(&("+11.0 Strength".into(), Color::srgb(0.0, 1.0, 0.0))));
        assert_eq!(lines[1].0, "+11.0 Strength");
        assert!(lines.iter().skip(2).all(|(line, _)| line.starts_with('-')));
        assert!(inv.item(1).unwrap().comparison_lines(&inv).is_empty());
        assert_eq!(inv.equipped("Chest").unwrap().id, 1);
    }
}
