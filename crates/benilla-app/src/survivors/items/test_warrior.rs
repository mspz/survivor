//! Reproducible, legal smith rolls for a local level-60 physical/critical test warrior.
use super::*;

fn value(item: &Item, effect: Effect) -> f32 {
    item.affixes
        .iter()
        .filter(|a| a.effect == effect)
        .map(|a| a.value)
        .sum()
}

#[test]
fn level_sixty_epic_warrior_kit_is_legal_equippable_and_resistance_capped() {
    let config = Config::builtin();
    let mut rng = Rng(0x7465_7374_7761_7272);
    let mut items = Vec::new();
    for (index, (name, slot)) in SLOTS.iter().enumerate() {
        let cosmetic = matches!(slot, SlotKind::Shirt | SlotKind::Tabard);
        let resistance = index < 5 || *slot == SlotKind::Wrists;
        let equipment = match slot {
            SlotKind::Head
            | SlotKind::Shoulders
            | SlotKind::Chest
            | SlotKind::Wrists
            | SlotKind::Hands
            | SlotKind::Waist
            | SlotKind::Legs
            | SlotKind::Feet => EquipmentType::Plate,
            SlotKind::MainHand => EquipmentType::MeleeOneHand,
            SlotKind::OffHand => EquipmentType::Shield,
            SlotKind::Ranged => EquipmentType::Bow,
            _ => EquipmentType::Universal,
        };
        let critical = cosmetic || resistance || *slot == SlotKind::MainHand;
        let offensive = if critical {
            "precision"
        } else if matches!(slot, SlotKind::Hands | SlotKind::OffHand | SlotKind::Ranged) {
            "armour_penetration"
        } else {
            "recovery"
        };
        let mut affixes = vec![offensive, "ferocity", "stamina"];
        if !cosmetic {
            affixes.push(
                if *slot == SlotKind::MainHand || *slot == SlotKind::Ranged {
                    "weapon_damage"
                } else {
                    "increased_attack_damage"
                },
            );
        }
        if *slot == SlotKind::OffHand {
            affixes.extend(["block_chance", "life_on_block_percent"]);
        } else {
            affixes.push("strength");
            affixes.push(if resistance {
                "all_resistance"
            } else {
                "armour"
            });
        }
        let request = CraftRequest {
            slot: *slot,
            equipment,
            profile: CraftProfile::Strength,
            rarity: Rarity::Epic,
            affixes: Some(affixes.into_iter().map(str::to_owned).collect()),
        };
        let score = |item: &Item| {
            value(item, Effect::CritChance) * 20.0
                + value(item, Effect::CritMultiplier) * 4.0
                + value(item, Effect::AttackDamagePercent) * 4.0
                + value(item, Effect::WeaponDamage) * 5.0
                + value(item, Effect::Strength) / 150.0
                + value(item, Effect::Stamina) / 200.0
                + value(item, Effect::CooldownRecovery) * 3.0
                + value(item, Effect::ArmourPenetration) * 4.0
                + value(item, Effect::BlockChance) * 10.0
                + value(item, Effect::LifeOnBlockPercent) * 50.0
        };
        let best = (0..768)
            .filter_map(|_| {
                let item = config
                    .generate_craft(&request, 60, 1, &mut rng)
                    .unwrap_or_else(|error| panic!("{name}: {error}"));
                (!resistance || value(&item, Effect::AllResistance) >= 12.6).then_some(item)
            })
            .max_by(|a, b| score(a).total_cmp(&score(b)))
            .expect("eligible roll");
        assert_eq!(best.level, 60);
        assert_eq!(best.rarity, Rarity::Epic);
        assert!(best.valid_equipment());
        assert!(value(&best, Effect::Stamina) > 0.0);
        items.push(best);
    }
    let mut inventory = Inventory::fixture(items);
    inventory.save.starter_granted = true;
    for (index, (name, kind)) in SLOTS.iter().enumerate() {
        let item = &inventory.save.items[index];
        assert!(inventory.can_equip(item, *kind), "{name}");
        inventory.save.equipped.insert((*name).into(), item.id);
    }
    let text = inventory.shared_save().unwrap();
    Inventory::decode(&text).unwrap();
    let mut run = Run::default();
    inventory.apply(&mut run);
    let stats = run.stats();
    for (tag, _, total) in stats.resistances.entries() {
        assert!(total >= 75.0);
        assert_eq!(stats.resistances.reduction(tag), 0.75);
    }
    assert!(run.equipment.block > 0.0 && run.equipment.life_on_block_percent > 0.0);
    assert!(
        value(
            inventory.equipped("Main hand").unwrap(),
            Effect::WeaponDamage
        ) > 0.0
    );
    let critical_chance = 0.05
        + inventory
            .all()
            .iter()
            .map(|item| value(item, Effect::CritChance))
            .sum::<f32>();
    assert!(critical_chance > 0.5 && critical_chance < 1.0);
    // Explicit opt-in exports a reviewed kit, never writes to the player's account in tests.
    if let Some(path) = std::env::var_os("SURVIVORS_WARRIOR_KIT_EXPORT") {
        let summary = serde_json::json!({
            "inventory": text,
            "xp": super::super::talents::threshold(60),
            "all_resistance": stats.resistances.fire,
            "block": run.equipment.block,
            "life_on_block_percent": run.equipment.life_on_block_percent,
            "weapon_damage": value(inventory.equipped("Main hand").unwrap(), Effect::WeaponDamage),
            "items": inventory.all().len(),
            "critical_chance": critical_chance,
        });
        std::fs::write(path, serde_json::to_string_pretty(&summary).unwrap()).unwrap();
    }
}
