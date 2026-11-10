use bevy::prelude::*;

use super::{Run, Selection, data};

pub(super) fn zone_order() -> Vec<usize> {
    data::CAMPAIGN.to_vec()
}

#[derive(Debug)]
pub(super) struct Progress {
    character: Option<u64>,
    completed: Vec<usize>,
}

impl Default for Progress {
    fn default() -> Self {
        let text = crate::local_state::home()
            .and_then(|h| {
                crate::local_state::read_to_string(&h.join("survivors-progress.txt")).ok()
            })
            .unwrap_or_default();
        Self {
            character: None,
            completed: text
                .split_whitespace()
                .filter_map(|v| v.parse().ok())
                .collect(),
        }
    }
}

impl Progress {
    #[cfg(not(target_arch = "wasm32"))]
    pub(in crate::survivors) fn complete_coop(&mut self, zone: usize) {
        if (data::CAMPAIGN.contains(&zone) || data::dungeon(zone))
            && !self.completed.contains(&zone)
        {
            self.completed.push(zone);
            self.save();
        }
    }
    pub fn fresh(id: u64) -> Self {
        Self {
            character: Some(id),
            completed: Vec::new(),
        }
    }
    pub fn bind(&mut self, id: u64) {
        self.character = Some(id);
        self.save();
    }
    pub fn load_character(id: u64) -> Self {
        let text = crate::local_state::home()
            .and_then(|home| {
                crate::local_state::read_to_string(&home.join(format!("characters/{id}-zones.txt")))
                    .ok()
            })
            .unwrap_or_default();
        Self {
            character: Some(id),
            completed: text
                .split_whitespace()
                .filter_map(|s| s.parse().ok())
                .collect(),
        }
    }
    fn save(&self) {
        if let Some(home) = crate::local_state::home() {
            let file = self.character.map_or_else(
                || "survivors-progress.txt".to_owned(),
                |id| format!("characters/{id}-zones.txt"),
            );
            let text = self
                .completed
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(" ");
            if let Err(error) = crate::local_state::write_queued(&home.join(file), &text) {
                warn!("Could not save zone progression: {error}");
            }
        }
    }
    pub(super) fn allows(&self, zone: usize) -> bool {
        let order = zone_order();
        if data::dungeon(zone) {
            return order.iter().all(|i| self.completed.contains(i));
        }
        order
            .iter()
            .position(|&i| i == zone)
            .is_some_and(|rank| order[..rank].iter().all(|i| self.completed.contains(i)))
    }

    fn complete(&mut self, zone: usize) {
        if !self.allows(zone) || self.completed.contains(&zone) {
            return;
        }
        self.completed.push(zone);
        self.save();
    }
}

pub(super) fn finish_run(run: Res<Run>, mut selection: ResMut<Selection>) {
    if run.won && run.endless.is_none() {
        let zone = selection.stage;
        selection.progress.complete(zone);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_dungeon_unlocks_together_only_after_the_whole_campaign() {
        let mut progress = Progress {
            character: None,
            completed: vec![],
        };
        for (rank, &zone) in data::CAMPAIGN.iter().enumerate() {
            assert!(data::DUNGEONS.iter().all(|&d| !progress.allows(d)));
            progress.complete(zone);
            if rank + 1 < data::CAMPAIGN.len() {
                assert!(data::DUNGEONS.iter().all(|&d| !progress.allows(d)));
            }
        }
        assert!(data::DUNGEONS.iter().all(|&d| progress.allows(d)));
        progress.complete(data::DUNGEONS[5]);
        assert!(data::DUNGEONS.iter().all(|&d| progress.allows(d)));
        assert!(!progress.allows(data::TRAINING_MAP));
    }

    #[test]
    fn zones_require_all_previous_victories() {
        let order = zone_order();
        let mut progress = Progress {
            completed: vec![],
            character: None,
        };
        assert!(progress.allows(order[0]));
        assert!(!progress.allows(order[1]));
        progress.complete(order[1]);
        assert!(progress.completed.is_empty());
        progress.complete(order[0]);
        assert!(progress.allows(order[1]));
        assert!(!progress.allows(order[2]));
        assert!(!progress.allows(data::MAPS.len()));
    }
}
