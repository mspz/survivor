//! Clients own their files. The host only keeps a trusted, transient copy of their build.
use super::*;
use std::hash::{Hash, Hasher};

pub(super) type Signature = (u64, u64, u64, [[u8; 19]; 3], u64);

pub(super) fn signature(world: &mut World, class: usize) -> Option<Signature> {
    let inventory = world.get_resource::<game::items::Inventory>()?;
    let character = inventory.shared_character()?;
    let revision = inventory.revision;
    let account = world.get_resource::<game::talents::Account>()?;
    let (xp, talents) = (account.xp, account.shared_build(class));
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    inventory.money().hash(&mut hash);
    if let Ok((net, store)) = world
        .query_filtered::<(&NetEntity, &ObjectStore), With<hero::Hero>>()
        .single(world)
    {
        net.display_id.hash(&mut hash);
        for pair in store.0.raw_fields().filter(|(i, _)| {
            matches!(*i, 2 | 131 | 132 | 164 | 193 | 194)
                || (37..46).contains(i)
                || ((260..=476).contains(i) && (*i - 260) % 12 == 0)
        }) {
            pair.hash(&mut hash);
        }
    }
    Some((character, revision, xp, talents, hash.finish()))
}

#[derive(Default)]
pub(super) struct Receipts {
    money: u64,
    run: u64,
    earned: u64,
    victory: bool,
    pub loot: u64,
}

impl Receipts {
    fn credit(&mut self, run: u64, earned: u64) -> u64 {
        if self.run != run {
            self.run = run;
            self.earned = 0;
            self.victory = false;
        }
        let gained = earned.saturating_sub(self.earned);
        self.earned = self.earned.max(earned);
        gained
    }
}

pub(super) fn capture(world: &mut World, class: usize) -> Option<wire::ClientSave> {
    let active = world.get_resource::<game::characters::Roster>()?.active?;
    let inventory = world.get_resource::<game::items::Inventory>()?;
    if inventory.shared_character() != Some(active) {
        return None;
    }
    let inventory = inventory.shared_save().ok()?;
    let account = world.get_resource::<game::talents::Account>()?;
    let talent_xp = account.xp;
    let talents = account.shared_build(class);
    let look = world
        .query_filtered::<(&NetEntity, &ObjectStore), With<hero::Hero>>()
        .single(world)
        .ok()
        .filter(|(net, _)| net.kind == EntityKind::Player)
        .and_then(|(net, store)| {
            Some((
                store
                    .0
                    .unit_native_displayid()
                    .filter(|v| *v > 0)
                    .map(|v| v as u32)
                    .or(net.display_id)?,
                store
                    .0
                    .raw_fields()
                    .filter(|(i, _)| {
                        matches!(*i, 2 | 131 | 132 | 164 | 193 | 194)
                            || (37..46).contains(i)
                            || ((260..=476).contains(i) && (*i - 260) % 12 == 0)
                    })
                    .map(|(i, value)| {
                        (
                            i,
                            if i == 131 {
                                store
                                    .0
                                    .unit_native_displayid()
                                    .filter(|v| *v > 0)
                                    .map_or(value, |v| v as u32)
                            } else {
                                value
                            },
                        )
                    })
                    .collect::<Vec<_>>(),
            ))
        });
    let look = look.map(|(display, fields)| {
        let entries: BTreeSet<u32> = fields
            .iter()
            .filter(|(i, e)| (260..=476).contains(i) && (*i - 260) % 12 == 0 && *e != 0)
            .map(|(_, e)| *e)
            .collect();
        let equipment = world
            .get_resource::<crate::items::Items>()
            .map(|items| {
                entries
                    .into_iter()
                    .filter_map(|entry| {
                        let item = items.template_cached(entry)?;
                        Some((
                            entry,
                            wire::Appearance {
                                display_id: item.display_info_id,
                                inventory_type: item.inventory_type,
                                item_class: item.class,
                                subclass: item.subclass,
                                material: item.material,
                                sheath: item.sheath,
                            },
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default();
        wire::SavedLook {
            display,
            fields,
            equipment,
        }
    });
    Some(wire::ClientSave {
        talent_xp,
        talents,
        inventory,
        look,
        specialty: world
            .get_resource::<game::specialty::Specialty>()
            .map(|s| game::specialty::encode(s.ranks()))
            .unwrap_or_default(),
    })
}

pub(super) fn apply(
    world: &mut World,
    entity: Entity,
    run: &mut Run,
    class: usize,
    save: &wire::ClientSave,
) -> Result<(), String> {
    let inventory = game::items::Inventory::from_shared(run.class.id, game::talents::level(save.talent_xp), &save.inventory)?;
    // Imported inventories have a fresh local revision: invalidate the previous equipment cache.
    run.equipment_version = None;
    game::talents::Account::from_shared(class, save.talent_xp, save.talents).apply(class, run);
    game::specialty::apply(&game::specialty::decode(&save.specialty), run, u64::MAX);
    inventory.apply(run);
    if let Some(look) = &save.look {
        let mut fields = look.fields.clone();
        if let Some(mut templates) = world.get_resource_mut::<crate::items::Items>() {
            for (slot, (old, appearance)) in look.equipment.iter().enumerate() {
                let entry = 0x7000_0000u32
                    .wrapping_add((run.owner.unwrap_or(0) as u32).wrapping_mul(32))
                    .wrapping_add(slot as u32);
                templates.insert_template(
                    entry,
                    Some(game::items::visuals::template(
                        game::items::visuals::Appearance {
                            display_id: appearance.display_id,
                            inventory_type: appearance.inventory_type,
                            item_class: appearance.item_class,
                            subclass: appearance.subclass,
                            material: appearance.material,
                            sheath: appearance.sheath,
                        },
                    )),
                );
                for (index, value) in &mut fields {
                    if (260..=476).contains(index) && (*index - 260) % 12 == 0 && *value == *old {
                        *value = entry;
                    }
                }
            }
        }
        if let Some(mut net) = world.get_mut::<NetEntity>(entity) {
            net.kind = EntityKind::Player;
            net.display_id = Some(look.display);
        }
        let guid = world.get::<Guid>(entity).map_or(0, |g| g.0);
        let mut changes = Vec::new();
        if let Some(mut store) = world.get_mut::<ObjectStore>(entity) {
            store
                .0
                .merge_diff(ObjectFields::from_pairs(&fields), |index, old, new| {
                    changes.push(FieldChanged {
                        entity,
                        guid,
                        kind: ObjectType::Player,
                        index,
                        old,
                        new,
                    })
                });
        }
        for change in changes {
            world.write_message(change);
        }
    }
    run.hp = run.hp.min(run.stats().max_hp);
    sync_vitals(world, entity, run);
    Ok(())
}

/// Equipment changes also reach replicated health fields while the world is paused.
pub(super) fn sync_vitals(world: &mut World, entity: Entity, run: &Run) {
    let guid = world.get::<Guid>(entity).map_or(0, |g| g.0);
    let kind = if world
        .get::<NetEntity>(entity)
        .is_some_and(|net| net.kind == EntityKind::Player)
    {
        ObjectType::Player
    } else {
        ObjectType::Unit
    };
    let mut changes = Vec::new();
    if let Some(mut store) = world.get_mut::<ObjectStore>(entity) {
        store.0.merge_diff(
            ObjectFields::from_pairs(&[
                (22, run.hp.max(0.0).ceil() as u32),
                (28, run.stats().max_hp.ceil() as u32),
            ]),
            |index, old, new| {
                changes.push(FieldChanged {
                    entity,
                    guid,
                    kind,
                    index,
                    old,
                    new,
                })
            },
        );
    }
    for change in changes {
        world.write_message(change);
    }
}

pub(super) fn persist(
    world: &mut World,
    receipts: &mut Receipts,
    snapshot: &wire::Snapshot,
    character: Option<u64>,
) {
    if let Some(character) = character {
        if world
            .get_resource::<game::talents::Account>()
            .and_then(|a| a.character)
            != Some(character)
            || world
                .get_resource::<game::items::Inventory>()
                .and_then(|i| i.shared_character())
                != Some(character)
        {
            return;
        }
    }
    let gained = receipts.credit(snapshot.run_id, snapshot.stats.earned);
    let money = snapshot.money_earned.saturating_sub(receipts.money);
    if money > 0 {
        let gold = game::specialty::gold(world, money);
        if let Some(mut inventory) = world.get_resource_mut::<game::items::Inventory>() {
            match inventory.gain_money(gold) {
                Ok(()) => receipts.money = snapshot.money_earned,
                Err(error) => inventory.notice = format!("Co-op money could not be saved: {error}"),
            }
        }
    }
    if gained > 0 {
        if let Some(mut account) = world.get_resource_mut::<game::talents::Account>() {
            if account.character.is_some() {
                account.gain_xp(gained);
                account.last_reward = snapshot.stats.earned;
                account.save();
            }
        }
    }
    if snapshot.stats.won && !receipts.victory {
        if let Some(mut selection) = world.get_resource_mut::<Selection>() {
            selection.progress.complete_coop(snapshot.map);
        }
        receipts.victory = true;
    }
    for (id, encoded) in &snapshot.loot {
        if *id <= receipts.loot {
            continue;
        }
        let Some(mut inventory) = world.get_resource_mut::<game::items::Inventory>() else {
            break;
        };
        let result = toml::from_str::<game::items::Item>(encoded)
            .map_err(|e| e.to_string())
            .and_then(|item| inventory.collect(item));
        match result {
            Ok(()) => receipts.loot = *id,
            Err(error) => {
                inventory.notice = format!("Co-op loot could not be saved: {error}");
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_snapshots_credit_only_new_progress_and_reset_for_a_new_run() {
        let mut receipt = Receipts::default();
        assert_eq!(receipt.credit(10, 25), 25);
        assert_eq!(receipt.credit(10, 25), 0);
        assert_eq!(receipt.credit(10, 20), 0);
        assert_eq!(receipt.credit(10, 35), 10);
        assert_eq!(receipt.credit(11, 5), 5);
        assert_eq!(receipt.credit(11, 5), 0);
    }

    #[test]
    fn peer_inventory_is_transient_and_uses_the_clients_actual_item_rolls() {
        let source = game::items::Inventory::fixture(vec![]);
        let encoded = source.shared_save().unwrap();
        let remote = game::items::Inventory::from_shared(1, 60, &encoded).unwrap();
        assert_eq!(remote.shared_character(), None);
        assert_eq!(remote.shared_save().unwrap(), encoded);
    }

    #[test]
    fn received_progress_and_loot_are_saved_once_for_the_local_character() {
        let mut world = World::new();
        let inventory = game::items::Inventory::fixture(vec![]);
        let mut rng = Rng::default();
        let item = inventory
            .config
            .generate_for_class(10, 1, &mut rng)
            .unwrap();
        let mut account = game::talents::Account::fresh();
        account.character = Some(1);
        world.insert_resource(account);
        world.insert_resource(inventory);
        world.init_resource::<Selection>();
        let zone = game::progression::zone_order()[0];
        world.resource_mut::<Selection>().progress = game::progression::Progress::fresh(1);
        let run = Run {
            meta_kills: 25,
            won: true,
            ..default()
        };
        let snapshot = wire::Snapshot {
            boss_warnings: vec![],
            money_earned: 12345,
            chest: default(),
            party: vec![],
            run_id: 9,
            loot: vec![(1, toml::to_string(&item).unwrap())],
            tick: 1,
            acknowledged: 0,
            phase: 6,
            map: zone,
            in_hub: false,
            stats: stats(&run, &hud::Offer::default(), 1),
            units: vec![],
            effects: vec![],
            equipment: vec![],
        };
        let mut receipts = Receipts::default();
        persist(&mut world, &mut receipts, &snapshot, Some(1));
        persist(&mut world, &mut receipts, &snapshot, Some(1));
        assert_eq!(world.resource::<game::talents::Account>().xp, 525);
        assert_eq!(world.resource::<game::items::Inventory>().all().len(), 1);
        assert_eq!(world.resource::<game::items::Inventory>().money(), 12345);
        assert_eq!(receipts.money, 12345);
        assert_eq!(receipts.loot, 1);
        let encoded = world
            .resource::<game::items::Inventory>()
            .shared_save()
            .unwrap();
        let restored = game::items::Inventory::from_shared(1, 60, &encoded).unwrap();
        assert_eq!(restored.all()[0].name, item.name);
        assert_eq!(restored.money(), 12345);
        let next_zone = game::progression::zone_order()[1];
        assert!(world.resource::<Selection>().progress.allows(next_zone));
        // A packet for the previous character must not touch the newly selected save.
        world.resource_mut::<game::talents::Account>().character = Some(2);
        persist(&mut world, &mut Receipts::default(), &snapshot, Some(1));
        assert_eq!(world.resource::<game::talents::Account>().xp, 525);
        // A later XP receipt crosses level 60 once and carries the surplus into paragon.
        let cap = game::talents::threshold(60);
        {
            let mut account = world.resource_mut::<game::talents::Account>();
            account.character = Some(1);
            account.xp = cap - 25;
        }
        let mut post_cap = snapshot.clone();
        post_cap.stats.earned += 200;
        persist(&mut world, &mut receipts, &post_cap, Some(1));
        persist(&mut world, &mut receipts, &post_cap, Some(1));
        let account = world.resource::<game::talents::Account>();
        assert_eq!(account.xp, cap + 175);
        assert_eq!(game::paragon::progress(account.xp).level, 1);
        assert_eq!(account.available(0), 59);
    }
}
