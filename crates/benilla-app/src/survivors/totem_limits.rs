//! Per-owner, per-spell totem slots include spawns queued in the current cast batch.
use bevy::prelude::Entity;
use std::collections::{HashMap, VecDeque};

#[derive(Default)]
pub(super) struct Slots(HashMap<&'static str, VecDeque<Entity>>);

impl Slots {
    pub fn track(&mut self, key: &'static str, entity: Entity) {
        self.0.entry(key).or_default().push_back(entity);
    }

    pub fn insert(&mut self, key: &'static str, entity: Entity, limit: usize) -> Vec<Entity> {
        let slots = self.0.entry(key).or_default();
        let mut retired = Vec::new();
        while slots.len() >= limit.max(1) {
            retired.push(slots.pop_front().unwrap());
        }
        slots.push_back(entity);
        retired
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_oldest_per_spell_including_queued_spawns_and_lowered_limits() {
        let mut slots = Slots::default();
        let entities: Vec<_> = (1..=6).map(Entity::from_bits).collect();
        slots.track("searing_totem", entities[0]);
        slots.track("magma_totem", entities[1]);
        assert_eq!(
            slots.insert("searing_totem", entities[2], 1),
            vec![entities[0]]
        );
        assert!(slots.insert("magma_totem", entities[3], 2).is_empty());
        assert_eq!(
            slots.insert("magma_totem", entities[4], 2),
            vec![entities[1]]
        );
        assert_eq!(
            slots.insert("magma_totem", entities[5], 1),
            vec![entities[3], entities[4]]
        );
    }
}
