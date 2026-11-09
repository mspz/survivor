//! Legal level-60 smith rolls and a purchased Assassination/Combat build for local testing.
use super::*;
use crate::survivors::{data, talent_design::Mechanic, talent_runtime, talents};

fn value(item: &Item, effect: Effect) -> f32 {
    item.affixes
        .iter()
        .filter(|a| a.effect == effect)
        .map(|a| a.value)
        .sum()
}

#[test]
fn level_sixty_dot_rogue_has_legal_gear_and_fifty_nine_purchased_talents() {
    let config = Config::builtin();
    let mut rng = Rng(0x7465_7374_726f_6775);
    let mut items = Vec::new();
    for (index, (name, slot)) in SLOTS.iter().enumerate() {
        let resistance = index < 5 || *slot == SlotKind::Wrists;
        let melee = matches!(slot, SlotKind::MainHand | SlotKind::OffHand);
        let cosmetic = matches!(slot, SlotKind::Shirt | SlotKind::Tabard);
        let jewelry = matches!(slot, SlotKind::Neck | SlotKind::Ring | SlotKind::Trinket);
        let equipment = match slot {
            SlotKind::Head
            | SlotKind::Shoulders
            | SlotKind::Chest
            | SlotKind::Wrists
            | SlotKind::Hands
            | SlotKind::Waist
            | SlotKind::Legs
            | SlotKind::Feet => EquipmentType::Leather,
            SlotKind::MainHand | SlotKind::OffHand => EquipmentType::MeleeOneHand,
            SlotKind::Ranged => EquipmentType::Bow,
            _ => EquipmentType::Universal,
        };
        let mut affixes = vec![
            "stamina",
            "agility",
            if resistance {
                "all_resistance"
            } else {
                "armour"
            },
        ];
        if melee {
            affixes.extend(["weapon_damage", "dot", "ailment_chance"]);
        } else if *slot == SlotKind::Ranged {
            affixes.extend(["weapon_damage", "dot", "recovery"]);
        } else if jewelry {
            affixes.extend([
                "increased_attack_damage",
                "dot",
                if *name == "Ring 1" {
                    "ailment_chance"
                } else {
                    "duration"
                },
            ]);
        } else if cosmetic {
            affixes.extend(["precision", "recovery", "ferocity"]);
        } else {
            affixes.extend(["increased_attack_damage", "recovery", "precision"]);
        }
        let request = CraftRequest {
            // One-handed bases can legally be equipped in either hand.
            slot: if melee { SlotKind::MainHand } else { *slot },
            equipment,
            profile: CraftProfile::Agility,
            rarity: Rarity::Epic,
            affixes: Some(affixes.into_iter().map(str::to_owned).collect()),
        };
        let score = |item: &Item| {
            value(item, Effect::DotDamage) * 6.0
                + value(item, Effect::WeaponDamage) * 4.0
                + value(item, Effect::AilmentChance) * 5.0
                + value(item, Effect::AttackDamagePercent) * 4.0
                + value(item, Effect::CooldownRecovery) * 3.0
                + value(item, Effect::Agility) / 150.0
                + value(item, Effect::Stamina) / 200.0
        };
        let best = (0..1024)
            .filter_map(|_| {
                let item = config
                    .generate_craft(&request, 60, 4, &mut rng)
                    .unwrap_or_else(|error| panic!("{name}: {error}"));
                ((!resistance || value(&item, Effect::AllResistance) >= 12.6)
                    && (!melee || item.base == "dagger"))
                    .then_some(item)
            })
            .max_by(|a, b| score(a).total_cmp(&score(b)))
            .expect("eligible legal roll");
        assert!(value(&best, Effect::Stamina) > 0.0 && value(&best, Effect::Agility) > 0.0);
        assert!(best.valid_equipment());
        assert_eq!(best.level, 60);
        assert_eq!(best.rarity, Rarity::Epic);
        items.push(best);
    }
    let mut inventory = Inventory::fixture(items);
    inventory.class = 4;
    inventory.save.starter_granted = true;
    for (index, (name, slot)) in SLOTS.iter().enumerate() {
        let item = &inventory.save.items[index];
        assert!(inventory.can_equip(item, *slot), "{name}");
        inventory.save.equipped.insert((*name).into(), item.id);
    }
    let inventory_text = inventory.shared_save().unwrap();
    Inventory::decode(&inventory_text).unwrap();
    let mut account = talents::Account::fresh();
    account.xp = talents::threshold(60);
    for (tree, nodes) in [
        (
            0,
            vec![
                (0, 5),
                (1, 5),
                (4, 5),
                (8, 1),
                (2, 5),
                (10, 5),
                (12, 1),
                (9, 3),
                (18, 1),
            ],
        ),
        (
            1,
            vec![
                (0, 5),
                (1, 5),
                (4, 5),
                (8, 1),
                (5, 4),
                (12, 1),
                (9, 5),
                (6, 2),
            ],
        ),
    ] {
        for (node, count) in nodes {
            for _ in 0..count {
                assert!(account.buy(3, tree, node), "tree={tree}, node={node}");
            }
        }
    }
    assert_eq!(account.available(3), 0);
    assert_eq!(account.tree_points(3, 0), 31);
    assert_eq!(account.tree_points(3, 1), 28);
    let mut run = Run {
        class: &data::CLASSES[3],
        ..Default::default()
    };
    account.apply(3, &mut run);
    inventory.apply(&mut run);
    for mechanic in [
        Mechanic::Poison,
        Mechanic::Proliferate,
        Mechanic::Poisonburst,
        Mechanic::Splash,
    ] {
        assert!(talent_runtime::active(&run, mechanic));
    }
    let stats = run.stats();
    for (tag, _, total) in stats.resistances.entries() {
        assert!(total >= 75.0);
        assert_eq!(stats.resistances.reduction(tag), 0.75);
    }
    for slot in ["Main hand", "Off hand"] {
        let item = inventory.equipped(slot).unwrap();
        assert_eq!(item.base, "dagger");
        assert!(value(item, Effect::WeaponDamage) > 0.0);
        assert!(value(item, Effect::AilmentChance) > 0.0);
    }
    let mut talent_text = format!("v4 {}", account.xp);
    for class in 0..data::CLASSES.len() {
        for rank in account.shared_build(class).iter().flatten() {
            talent_text.push_str(&format!(" {rank}"));
        }
    }
    if let Some(path) = std::env::var_os("SURVIVORS_ROGUE_KIT_EXPORT") {
        let summary = serde_json::json!({
            "inventory": inventory_text, "talents": talent_text, "xp": account.xp,
            "all_resistance": stats.resistances.fire, "items": inventory.all().len(),
            "main_hand_local_damage": value(inventory.equipped("Main hand").unwrap(), Effect::WeaponDamage),
            "off_hand_local_damage": value(inventory.equipped("Off hand").unwrap(), Effect::WeaponDamage),
            "ailment_chance": inventory.all().iter().map(|item| value(item, Effect::AilmentChance)).sum::<f32>(),
        });
        std::fs::write(path, serde_json::to_string_pretty(&summary).unwrap()).unwrap();
    }
}
