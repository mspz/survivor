//! Character-owned copper, granted directly on kills; no pickup entities or animations.
use super::{enemies::Loot, items::Inventory};
use bevy::prelude::*;
pub(super) const SILVER: u64 = 100;
pub(super) const GOLD: u64 = 100 * SILVER;
pub(super) const MAX_BALANCE: u64 = i64::MAX as u64;
pub(super) fn split(copper: u64) -> (u64, u64, u64) {
    (copper / GOLD, copper / SILVER % 100, copper % 100)
}
pub(super) fn reward(level: u32, boss: bool) -> u64 {
    let steps = u64::from(level.clamp(1, 60) - 1);
    if boss {
        10 + (10 * GOLD - 10) * steps.pow(3) / 59u64.pow(3)
    } else {
        3 + steps.pow(2) / 4
    }
}
#[derive(Resource, Default)]
struct Pending(std::collections::BTreeMap<u64, u64>);
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Pending>().add_systems(
        Update,
        credit
            .after(super::abilities::apply_hits)
            .before(super::enemies::drop_loot)
            .run_if(super::multiplayer::authoritative),
    );
}
fn credit(
    mut drops: MessageReader<Loot>,
    mut inventory: Option<ResMut<Inventory>>,
    mut pending: ResMut<Pending>,
) {
    let gained = drops
        .read()
        .map(|drop| reward(drop.item_level, drop.boss))
        .fold(0u64, u64::saturating_add);
    let Some(inventory) = inventory.as_deref_mut() else {
        return;
    };
    let Some(character) = inventory.money_owner() else {
        return;
    };
    let amount = pending.0.entry(character).or_default();
    *amount = amount.saturating_add(gained).min(MAX_BALANCE);
    if *amount == 0 {
        return;
    }
    match inventory.add_money(*amount) {
        Ok(()) => *amount = 0,
        Err(error) => inventory.notice = format!("Money could not be saved: {error}"),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn money_frame_icons_exist_in_vanilla_data() {
        let path = benilla_formats::wow_data_or_skip!();
        let mut chain = benilla_formats::open_chain(&path).unwrap();
        let bytes = chain
            .read_file("Interface\\MoneyFrame\\UI-MoneyIcons.blp")
            .unwrap();
        benilla_formats::blp_to_rgba(&bytes).unwrap();
    }
    #[test]
    fn denominations_follow_one_hundred_copper_per_silver_and_silver_per_gold() {
        assert_eq!(split(0), (0, 0, 0));
        assert_eq!(split(99), (0, 0, 99));
        assert_eq!(split(100), (0, 1, 0));
        assert_eq!(split(10000), (1, 0, 0));
        assert_eq!(split(123456), (12, 34, 56));
    }
    #[test]
    fn kill_messages_credit_immediately_once_without_spawning_pickups() {
        let mut world = World::new();
        world.init_resource::<Messages<Loot>>();
        world.init_resource::<Pending>();
        world.insert_resource(Inventory::fixture(vec![]));
        let system = world.register_system(credit);
        let entities = world.entities().len();
        world.write_message(Loot {
            item_level: 1,
            pos: Vec3::ZERO,
            xp: 1,
            boss: false,
        });
        world.write_message(Loot {
            item_level: 60,
            pos: Vec3::ZERO,
            xp: 1,
            boss: true,
        });
        world.run_system(system).unwrap();
        assert_eq!(world.resource::<Inventory>().money(), 100003);
        world.run_system(system).unwrap();
        assert_eq!(world.resource::<Inventory>().money(), 100003);
        assert_eq!(world.entities().len(), entities);
    }
    #[test]
    fn loot_starts_with_a_few_copper_and_bosses_cap_at_ten_gold() {
        assert_eq!(reward(1, false), 3);
        assert_eq!(reward(1, true), 10);
        assert_eq!(reward(60, true), 10 * GOLD);
        assert_eq!(reward(u32::MAX, true), 10 * GOLD);
        for level in 1..60 {
            assert!(reward(level + 1, false) >= reward(level, false));
            assert!(reward(level + 1, true) >= reward(level, true));
            assert!(reward(level, true) >= reward(level, false));
            assert!(reward(level, false) < GOLD);
        }
    }
}
