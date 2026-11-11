//! Ten seconds of personal incoming damage, frozen on the lethal hit.
use super::{
    Run,
    enemies::{Enemy, Rank},
    multiplayer,
    spell_engine::Tags,
};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
#[path = "death_log_ui.rs"]
pub(super) mod ui;
const WINDOW: f32 = 10.0;
const MAX_EVENTS: usize = 2048;
const MAX_SOURCES: usize = 4096;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(in crate::survivors) struct Source {
    pub id: Option<u64>,
    pub name: String,
    pub mods: Option<Rank>,
    pub critical: bool,
}
impl Source {
    pub fn kind(&self) -> &'static str {
        match self.mods.map(|rank| rank.base) {
            Some(super::monster_mods::Base::Melee) => "Melee",
            Some(super::monster_mods::Base::Caster) => "Caster",
            Some(super::monster_mods::Base::Boss) => "Boss",
            None if self.id.is_none() => "Other",
            None => "Unknown",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum Outcome {
    Hit,
    Blocked,
    Dodged,
    Absorbed,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Event {
    pub at: f32,
    pub source: Source,
    pub ability: String,
    pub tags: u64,
    pub damage: f32,
    pub absorbed: f32,
    pub raw: f32,
    pub overkill: f32,
    pub before: f32,
    pub after: f32,
    pub outcome: Outcome,
    pub lethal: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in crate::survivors) struct Report {
    pub id: u64,
    pub at: f32,
    pub events: Vec<Event>,
    pub truncated: bool,
}
impl Report {
    pub fn total_damage(&self) -> f32 {
        self.events.iter().map(|e| e.damage).sum()
    }
    pub fn total_absorbed(&self) -> f32 {
        self.events.iter().map(|e| e.absorbed).sum()
    }
}
#[derive(Default)]
pub(super) struct Log {
    recent: VecDeque<Event>,
    sources: HashMap<u64, Source>,
    source_order: VecDeque<u64>,
    pub context: Option<Source>,
    pub last: Option<Report>,
    sealed: bool,
    sequence: u64,
    truncated_until: f32,
    remote_run: Option<u64>,
}
impl Log {
    pub fn remember(&mut self, entity: Entity, name: &str, rank: Rank) {
        let id = entity.to_bits();
        if self
            .sources
            .get(&id)
            .is_some_and(|s| s.name == name && s.mods == Some(rank))
        {
            return;
        }
        if !self.sources.contains_key(&id) {
            if self.sources.len() >= MAX_SOURCES {
                if let Some(old) = self.source_order.pop_front() {
                    self.sources.remove(&old);
                }
            }
            self.source_order.push_back(id);
        }
        let source = self.sources.entry(id).or_default();
        source.id = Some(id);
        source.name = name.to_owned();
        source.mods = Some(rank);
    }
    pub fn begin(
        &mut self,
        actor: Option<Entity>,
        rank: Rank,
        critical: bool,
        fallback: &str,
    ) -> Option<Source> {
        let mut source = self.source(actor, fallback);
        source.mods = Some(rank);
        source.critical = critical;
        self.context.replace(source)
    }
    fn source(&self, actor: Option<Entity>, fallback: &str) -> Source {
        let id = actor.map(Entity::to_bits);
        if let Some(context) = self.context.as_ref().filter(|s| s.id == id) {
            return context.clone();
        }
        id.and_then(|id| self.sources.get(&id))
            .cloned()
            .unwrap_or_else(|| Source {
                id,
                name: fallback.into(),
                ..default()
            })
    }
    #[allow(clippy::too_many_arguments)]
    pub fn record(
        &mut self,
        now: f32,
        actor: Option<Entity>,
        ability: &str,
        tags: Tags,
        raw: f32,
        damage: f32,
        absorbed: f32,
        before: f32,
        outcome: Outcome,
    ) {
        if !now.is_finite()
            || !before.is_finite()
            || before <= 0.0
            || !damage.is_finite()
            || !absorbed.is_finite()
        {
            return;
        }
        if self.sealed {
            self.recent.clear();
            self.sealed = false;
            self.truncated_until = 0.0;
        }
        self.recent.retain(|e| e.at >= now - WINDOW && e.at <= now);
        if self.recent.len() == MAX_EVENTS {
            if let Some(event) = self.recent.pop_front() {
                self.truncated_until = event.at + WINDOW;
            }
        }
        let damage = damage.max(0.0);
        let absorbed = absorbed.max(0.0);
        let lethal = damage >= before;
        self.recent.push_back(Event {
            at: now,
            source: self.source(actor, ability),
            ability: ability.into(),
            tags: tags.0,
            damage: damage.min(before),
            absorbed,
            raw: if raw.is_finite() { raw.max(0.0) } else { 0.0 },
            overkill: (damage - before).max(0.0),
            before,
            after: (before - damage).max(0.0),
            outcome,
            lethal,
        });
        if lethal {
            self.freeze(now);
        }
    }
    pub fn freeze(&mut self, now: f32) {
        if self.sealed {
            return;
        }
        self.recent.retain(|e| e.at >= now - WINDOW && e.at <= now);
        self.sequence = self.sequence.wrapping_add(1);
        self.last = Some(Report {
            id: self.sequence,
            at: now,
            events: self.recent.iter().cloned().collect(),
            truncated: self.truncated_until >= now && self.truncated_until > 0.0,
        });
        self.sealed = true;
    }
    pub fn receive(&mut self, run: u64, report: Option<Report>) {
        if self.remote_run != Some(run) {
            self.last = None;
            self.remote_run = Some(run);
        }
        if let Some(report) = report {
            self.last = Some(report);
        }
    }
}
pub(super) fn replica(log: &Log, run: u64, sent: &mut Option<(u64, u64)>) -> Option<Report> {
    let key = log.last.as_ref().map(|r| (run, r.id));
    if *sent == key {
        return None;
    }
    *sent = key;
    log.last.clone()
}
#[allow(clippy::type_complexity)]
pub(super) fn index_sources(
    mut run: ResMut<Run>,
    mut players: Option<ResMut<multiplayer::Companions>>,
    enemies: Query<(Entity, &Enemy, &Rank)>,
) {
    for (entity, enemy, rank) in &enemies {
        run.death_log.remember(entity, enemy.def.name, *rank);
        if let Some(players) = players.as_deref_mut() {
            for player in players.0.values_mut() {
                player.run.death_log.remember(entity, enemy.def.name, *rank);
            }
        }
    }
}
pub(super) fn record(
    run: &mut Run,
    tags: Tags,
    raw: f32,
    damage: f32,
    absorbed: f32,
    outcome: Outcome,
) {
    if run.immune_t > 0.0 || run.pending_levels > 0 || run.dead_t.is_some() {
        return;
    }
    run.death_log.record(
        run.elapsed,
        run.legendary.events.attacker,
        run.taken_source,
        tags,
        raw,
        damage,
        absorbed,
        run.hp,
        outcome,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn immunity_and_reward_selection_do_not_record_received_hits() {
        let mut run = Run {
            hp: 100.0,
            immune_t: 1.0,
            ..default()
        };
        run.equipment.block = 1.0;
        super::super::hero::hurt_hero_with_block(&mut run, 10.0, 0.0);
        assert!(run.death_log.recent.is_empty());
        run.immune_t = 0.0;
        run.pending_levels = 1;
        super::super::hero::hurt_hero_with_block(&mut run, 10.0, 0.0);
        assert!(run.death_log.recent.is_empty());
        run.pending_levels = 0;
        super::super::hero::hurt_hero_with_block(&mut run, 10.0, 1.0);
        assert_eq!(run.death_log.recent.len(), 1);
    }
    #[test]
    fn source_type_is_snapshotted_for_melee_caster_and_boss_even_without_mods() {
        use crate::survivors::monster_mods::Base;
        let actor = Entity::from_raw_u32(8).unwrap();
        for (base, label) in [
            (Base::Melee, "Melee"),
            (Base::Caster, "Caster"),
            (Base::Boss, "Boss"),
        ] {
            let mut rank = Rank::Normal;
            rank.base = base;
            let mut log = Log::default();
            log.remember(actor, "Source", rank);
            log.record(
                1.0,
                Some(actor),
                "Hit",
                Tags::PHYSICAL,
                20.0,
                20.0,
                0.0,
                10.0,
                Outcome::Hit,
            );
            log.remember(actor, "Another type", Rank::Normal.with(Rank::Echo));
            let report: Report =
                serde_json::from_str(&serde_json::to_string(log.last.as_ref().unwrap()).unwrap())
                    .unwrap();
            assert_eq!(report.events[0].source.kind(), label);
            assert_eq!(report.events[0].source.mods.unwrap().count(), 0);
        }
    }
    #[test]
    fn freezes_exactly_ten_seconds_and_keeps_lethal_overkill_and_absorption() {
        let mut log = Log::default();
        for (time, damage, hp, absorbed) in [
            (1.0, 10.0, 100.0, 0.0),
            (2.0, 20.0, 90.0, 5.0),
            (11.0, 30.0, 70.0, 0.0),
            (12.0, 100.0, 40.0, 8.0),
        ] {
            log.record(
                time,
                None,
                "Hit",
                Tags::FIRE,
                100.0,
                damage,
                absorbed,
                hp,
                Outcome::Hit,
            );
        }
        let report = log.last.as_ref().unwrap();
        assert_eq!(report.events.len(), 3);
        assert_eq!(report.events[0].at, 2.0);
        assert_eq!(report.total_damage(), 90.0);
        assert_eq!(report.total_absorbed(), 13.0);
        let lethal = report.events.last().unwrap();
        assert!(lethal.lethal);
        assert_eq!(lethal.damage, 40.0);
        assert_eq!(lethal.overkill, 60.0);
        assert_eq!(lethal.after, 0.0);
        log.freeze(30.0);
        assert_eq!(log.last.as_ref().unwrap().at, 12.0);
        log.record(
            13.0,
            None,
            "Late tick",
            Tags::DOT,
            10.0,
            10.0,
            0.0,
            0.0,
            Outcome::Hit,
        );
        assert_eq!(log.last.as_ref().unwrap().events.len(), 3);
    }
    #[test]
    fn source_name_and_mods_are_snapshots_not_live_enemy_queries() {
        let actor = Entity::from_raw_u32(7).unwrap();
        let mut log = Log::default();
        log.remember(actor, "Anvilrage Captain", Rank::Brutal.with(Rank::Echo));
        log.record(
            0.0,
            Some(actor),
            "Melee",
            Tags::PHYSICAL,
            20.0,
            20.0,
            0.0,
            100.0,
            Outcome::Hit,
        );
        log.remember(actor, "Changed name", Rank::Fireproof);
        log.record(
            1.0,
            Some(actor),
            "Melee",
            Tags::PHYSICAL,
            100.0,
            100.0,
            0.0,
            80.0,
            Outcome::Hit,
        );
        log.sources.clear();
        let events = &log.last.as_ref().unwrap().events;
        assert_eq!(events[0].source.name, "Anvilrage Captain");
        assert!(events[0].source.mods.unwrap().has(Rank::Echo));
        assert!(!events[0].source.mods.unwrap().has(Rank::Fireproof));
        assert_eq!(events[1].source.name, "Changed name");
    }
    #[test]
    fn real_damage_pipeline_records_added_schools_crits_and_periodic_snapshot_mods() {
        let actor = Entity::from_raw_u32(5).unwrap();
        let mut run = Run {
            hp: 500.0,
            ..default()
        };
        let rank = Rank::Pyromancer.with(Rank::Brutal);
        run.death_log.remember(actor, "Skeleton", rank);
        run.legendary.events.attacker = Some(actor);
        run.taken_source = "Melee";
        super::super::monster_stats::deal(
            &mut run,
            50.0,
            Tags::PHYSICAL | Tags::MELEE | Tags::ATTACK,
            super::super::monster_stats::Bonuses(rank),
            true,
            Some(1.0),
        );
        assert_eq!(run.death_log.recent.len(), 2);
        let events: Vec<_> = run.death_log.recent.iter().collect();
        assert!(events[0].source.critical);
        assert!(Tags(events[1].tags).contains(Tags::FIRE));
        assert!(events.iter().all(|e| e.source.mods == Some(rank)));
        run.elapsed = 1.0;
        let old = run.death_log.begin(Some(actor), rank, false, "Infection");
        run.taken_source = "Infection";
        super::super::hero::hurt_hero_tagged(&mut run, 1000.0, Tags::NATURE | Tags::DOT);
        run.death_log.context = old;
        let report = run.death_log.last.as_ref().unwrap();
        assert_eq!(report.events.len(), 3);
        assert!(Tags(report.events.last().unwrap().tags).contains(Tags::DOT));
        assert_eq!(report.events.last().unwrap().source.name, "Skeleton");
        assert!(run.death_log.context.is_none());
    }
    #[test]
    fn another_life_starts_a_new_window_without_erasing_the_previous_recap() {
        let mut log = Log::default();
        log.record(
            1.0,
            None,
            "First death",
            Tags::FIRE,
            50.0,
            50.0,
            0.0,
            50.0,
            Outcome::Hit,
        );
        log.record(
            5.0,
            None,
            "After revival",
            Tags::PHYSICAL,
            10.0,
            10.0,
            0.0,
            100.0,
            Outcome::Hit,
        );
        assert_eq!(log.last.as_ref().unwrap().id, 1);
        assert_eq!(log.recent.len(), 1);
        log.record(
            6.0,
            None,
            "Second death",
            Tags::PHYSICAL,
            100.0,
            100.0,
            0.0,
            90.0,
            Outcome::Hit,
        );
        let report = log.last.as_ref().unwrap();
        assert_eq!(report.id, 2);
        assert_eq!(report.events.len(), 2);
        assert!(report.events.iter().all(|e| e.at >= 5.0));
    }
    #[test]
    fn dense_combat_is_bounded_and_reports_truncation() {
        let mut log = Log::default();
        for _ in 0..MAX_EVENTS + 10 {
            log.record(
                1.0,
                None,
                "Blocked",
                Tags::PHYSICAL,
                1.0,
                0.0,
                0.0,
                100.0,
                Outcome::Blocked,
            );
        }
        log.record(
            2.0,
            None,
            "Fatal",
            Tags::FIRE,
            100.0,
            100.0,
            0.0,
            100.0,
            Outcome::Hit,
        );
        let report = log.last.as_ref().unwrap();
        assert_eq!(report.events.len(), MAX_EVENTS);
        assert!(report.truncated);
    }
    #[test]
    fn personal_recap_is_sent_once_per_death_and_retained_between_snapshots() {
        let mut log = Log::default();
        log.record(
            1.0,
            None,
            "Fatal",
            Tags::FIRE,
            100.0,
            100.0,
            0.0,
            100.0,
            Outcome::Hit,
        );
        let mut sent = None;
        let report = replica(&log, 20, &mut sent).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        let mut guest = Log::default();
        guest.receive(20, Some(serde_json::from_str(&json).unwrap()));
        guest.receive(20, replica(&log, 20, &mut sent));
        assert_eq!(guest.last.as_ref().unwrap().id, 1);
        assert!(replica(&log, 20, &mut sent).is_none());
        let other = Log::default();
        assert!(replica(&other, 30, &mut sent).is_none());
        guest.receive(30, None);
        assert!(guest.last.is_none());
    }
}
