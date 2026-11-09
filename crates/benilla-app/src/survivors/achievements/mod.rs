//! Account-wide achievements: lifetime counters, records and challenges, in tiers whose small
//! bonuses raise the specialty-point gain ([`super::specialty`]). Every client tracks its own
//! player from its local run, so a guest's run record comes with its replicated stats.
use super::{data, enemies, talents, Phase, Run, Selection};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

mod catalog;
pub(super) use catalog::{all, format_target, group, Achievement, Category, Metric};

const FILE: &str = "survivors-achievements.toml";

/// What a run did that its replicated stats do not say: the cards, the spells' kills, the bosses.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(in crate::survivors) struct RunRecord {
    pub spell_kills: BTreeMap<String, u32>,
    pub picks: u32,
    /// The best card rarity taken: 0 none, 1 Common, 2 Uncommon, 3 Epic, 4 Legendary.
    pub best_pick: u8,
    pub legendary_picks: u32,
    pub spells_learned: u32,
    pub passives_taken: u32,
    pub rerolls: u32,
    pub banishes: u32,
    pub skips: u32,
    /// Seconds from a boss's arrival to its death, the fastest of the run.
    pub fastest_boss: Option<f32>,
    pub boss_kills: u32,
    pub biggest_hit: f32,
}

impl RunRecord {
    pub fn spell_kill(&mut self, skill: &str) {
        *self.spell_kills.entry(skill.to_owned()).or_default() += 1;
    }
    /// A card taken from an offer, with its rarity.
    pub fn pick(&mut self, choice: super::hud::Choice, rarity: super::cards::Rarity) {
        use super::cards::Rarity;
        use super::hud::Choice;
        let rank = match rarity {
            Rarity::Common => 1,
            Rarity::Uncommon => 2,
            Rarity::Epic => 3,
            Rarity::Legendary => 4,
        };
        if matches!(choice, Choice::Heal) {
            return;
        }
        self.picks += 1;
        self.best_pick = self.best_pick.max(rank);
        self.legendary_picks += u32::from(rank == 4);
        match choice {
            Choice::NewAbility(_) => self.spells_learned += 1,
            Choice::NewPassive(_) | Choice::UpPassive(_) | Choice::NumericPassive(..) => {
                self.passives_taken += 1;
            }
            _ => {}
        }
    }
}

/// Support spells never deal damage: buffs, shields, immunities and heals.
pub(in crate::survivors) fn support(def: &data::AbilityDef) -> bool {
    matches!(
        def.kind,
        data::Kind::Buff | data::Kind::Shield | data::Kind::Immune | data::Kind::Heal
    )
}

#[derive(Default, Serialize, Deserialize)]
struct Save {
    version: u32,
    /// Lifetime counters and best values, by metric key.
    #[serde(default)]
    counters: BTreeMap<String, u64>,
    /// Best times in seconds, by metric key.
    #[serde(default)]
    times: BTreeMap<String, f64>,
    /// How many tiers of each achievement are reached.
    #[serde(default)]
    tiers: BTreeMap<String, u8>,
}

/// What the tracker last folded in from the current run, so each frame adds only what is new.
#[derive(Default)]
struct Folded {
    elapsed: f32,
    kills: u32,
    dead: bool,
    won: bool,
    started: bool,
    record: RunRecord,
    /// The lowest health of the run, as a share of the maximum.
    lowest_hp: Option<f32>,
    /// The spells already counted at their top rank this run.
    maxed: Vec<&'static str>,
}

#[derive(Resource)]
pub(in crate::survivors) struct Achievements {
    save: Save,
    folded: Folded,
    dirty: bool,
    changed: bool,
    /// Bumped when a value or a tier changes, for the window.
    pub revision: u64,
    inventory_revision: Option<u64>,
    money: Option<u64>,
    /// Newly reached tiers, shown one after another.
    toasts: Vec<(Earned, f32)>,
}

impl Default for Achievements {
    fn default() -> Self {
        let save = crate::local_state::home()
            .and_then(|home| crate::local_state::read_to_string(&home.join(FILE)).ok())
            .and_then(|text| match toml::from_str::<Save>(&text) {
                Ok(save) => Some(save),
                Err(error) => {
                    warn!("Ignoring {FILE}: {error}");
                    None
                }
            })
            .unwrap_or_default();
        Self {
            save,
            folded: Folded::default(),
            dirty: false,
            changed: true,
            revision: 0,
            inventory_revision: None,
            money: None,
            toasts: Vec::new(),
        }
    }
}

impl Achievements {
    #[cfg(test)]
    pub fn empty() -> Self {
        Self {
            save: Save::default(),
            folded: Folded::default(),
            dirty: false,
            changed: true,
            revision: 0,
            inventory_revision: None,
            money: None,
            toasts: Vec::new(),
        }
    }
    pub fn counter(&self, key: &str) -> u64 {
        self.save.counters.get(key).copied().unwrap_or(0)
    }
    pub fn time(&self, key: &str) -> Option<f64> {
        self.save.times.get(key).copied()
    }
    fn distinct(&self, prefix: &str) -> u64 {
        self.save
            .counters
            .range(prefix.to_owned()..)
            .take_while(|(key, _)| key.starts_with(prefix))
            .filter(|(_, &value)| value > 0)
            .count() as u64
    }
    pub fn add(&mut self, key: &str, amount: u64) {
        if amount == 0 {
            return;
        }
        let value = self.save.counters.entry(key.to_owned()).or_default();
        *value = value.saturating_add(amount);
        self.changed = true;
    }
    pub fn max(&mut self, key: &str, value: u64) {
        let old = self.save.counters.entry(key.to_owned()).or_default();
        if value > *old {
            *old = value;
            self.changed = true;
        }
    }
    fn best_time(&mut self, key: &str, seconds: f64) {
        if self.time(key).is_none_or(|old| seconds < old) {
            self.save.times.insert(key.to_owned(), seconds);
            self.changed = true;
        }
    }
    /// The tiers this achievement's value has reached.
    pub fn reached(&self, achievement: &Achievement) -> usize {
        achievement
            .tiers
            .iter()
            .take_while(|tier| match &achievement.metric {
                Metric::Counter(key) => self.counter(key) >= tier.target,
                Metric::Distinct(prefix) => self.distinct(prefix) >= tier.target,
                Metric::Time(key) => self.time(key).is_some_and(|t| t <= tier.target as f64),
            })
            .count()
    }
    /// The value shown against the next tier.
    pub fn value(&self, achievement: &Achievement) -> Option<f64> {
        match &achievement.metric {
            Metric::Counter(key) => Some(self.counter(key) as f64),
            Metric::Distinct(prefix) => Some(self.distinct(prefix) as f64),
            Metric::Time(key) => self.time(key),
        }
    }
    pub fn unlocked(&self, achievement: &Achievement) -> usize {
        usize::from(self.save.tiers.get(&achievement.id).copied().unwrap_or(0))
            .min(achievement.tiers.len())
    }
    /// The specialty-point gain multiplier's bonus from every reached tier.
    pub fn bonus(&self) -> f32 {
        all()
            .iter()
            .map(|a| a.tiers[..self.unlocked(a)].iter().map(|t| t.bonus).sum::<f32>())
            .sum()
    }
    pub fn totals(&self) -> (usize, usize) {
        all().iter().fold((0, 0), |(done, total), a| {
            (done + self.unlocked(a), total + a.tiers.len())
        })
    }

    /// Record reached tiers; returns how many are new.
    fn evaluate(&mut self) -> usize {
        if !self.changed {
            return 0;
        }
        self.changed = false;
        self.revision += 1;
        self.dirty = true;
        let mut new = 0;
        let mut earned = Vec::new();
        for achievement in all() {
            let reached = self.reached(achievement);
            let unlocked = self.unlocked(achievement);
            if reached > unlocked {
                for tier in unlocked..reached {
                    earned.push(Earned {
                        id: achievement.id.clone(),
                        title: format!(
                            "{}{}",
                            achievement.name,
                            if achievement.tiers.len() > 1 {
                                format!(" {}", roman(tier + 1))
                            } else {
                                String::new()
                            }
                        ),
                        line: format!(
                            "{}  |  +{:.2}% specialty point gain",
                            achievement.describe(tier),
                            achievement.tiers[tier].bonus * 100.0
                        ),
                    });
                }
                self.save.tiers.insert(achievement.id.clone(), reached as u8);
                new += reached - unlocked;
            }
        }
        // A burst (the first launch, a big run) shows a few and sums up the rest.
        if earned.len() > 3 {
            let rest = earned.len() - 2;
            earned.truncate(2);
            earned.push(Earned {
                id: String::new(),
                title: format!("{rest} more achievements"),
                line: "See them all in the Achievements window".into(),
            });
        }
        self.toasts.extend(earned.into_iter().map(|e| (e, TOAST_SECS)));
        new
    }

    fn save(&mut self) {
        self.dirty = false;
        self.save.version = 1;
        let Some(home) = crate::local_state::home() else {
            return;
        };
        match toml::to_string(&self.save) {
            Ok(text) => {
                if let Err(error) = crate::local_state::write_queued(&home.join(FILE), &text) {
                    warn!("Could not save achievements: {error}");
                }
            }
            Err(error) => warn!("Could not encode achievements: {error}"),
        }
    }

    /// Fold a victory's challenges in: each counts once per expedition.
    fn victory(&mut self, run: &Run, map: usize, naked: bool) {
        let record = &run.record;
        let met = [
            ("no_spells", record.spells_learned == 0),
            ("one_spell", record.spells_learned <= 1),
            (
                "no_support",
                !run.abilities.iter().any(|owned| support(owned.def)),
            ),
            (
                "no_draft",
                record.rerolls + record.banishes + record.skips == 0,
            ),
            ("no_cards", record.picks == 0),
            ("common_only", record.best_pick <= 1),
            ("no_passive", record.passives_taken == 0),
            ("no_death", !self.folded.dead),
            ("solo", run.expedition_players <= 1),
            ("naked", naked),
            ("pacifist", run.kills == 0),
        ];
        for (key, done) in met {
            if done {
                self.max(&format!("challenge:{key}:{map}"), 1);
                self.add(&format!("challenge_wins:{key}:{map}"), 1);
                self.max(&format!("challenge_any:{key}"), 1);
            }
        }
    }

    /// The achievement to announce now and how long it has been shown, if any.
    pub fn toast(&mut self, dt: f32) -> Option<(&Earned, f32)> {
        let first = self.toasts.first_mut()?;
        first.1 -= dt;
        if first.1 <= 0.0 {
            self.toasts.remove(0);
            return self.toasts.first().map(|t| (&t.0, TOAST_SECS - t.1));
        }
        let first = &self.toasts[0];
        Some((&first.0, TOAST_SECS - first.1))
    }
}

/// An announced tier: the achievement (empty for a summary), its title and its detail line.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::survivors) struct Earned {
    pub id: String,
    pub title: String,
    pub line: String,
}

pub(in crate::survivors) const TOAST_SECS: f32 = 5.0;

pub(in crate::survivors) fn roman(n: usize) -> &'static str {
    ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"]
        .get(n.saturating_sub(1))
        .copied()
        .unwrap_or("X+")
}

/// When a boss arrived, for the boss-rush records.
#[derive(Component)]
pub(super) struct BossArrival(pub f32);
#[derive(Component)]
struct BossTimed;

/// Every player's run is credited the fastest boss kill: the host's own and each guest's.
fn time_bosses(
    mut commands: Commands,
    mut run: ResMut<Run>,
    mut players: Option<ResMut<super::multiplayer::Companions>>,
    bosses: Query<(Entity, &enemies::Enemy, &BossArrival), Without<BossTimed>>,
) {
    for (entity, enemy, arrival) in &bosses {
        if !enemy.dead {
            continue;
        }
        commands.entity(entity).try_insert(BossTimed);
        let seconds = (run.elapsed - arrival.0).max(0.0);
        let credit = |record: &mut RunRecord| {
            record.fastest_boss = Some(record.fastest_boss.map_or(seconds, |s| s.min(seconds)));
        };
        run.record.boss_kills += 1;
        credit(&mut run.record);
        for player in players.iter_mut().flat_map(|p| p.0.values_mut()) {
            player.run.record.boss_kills += 1;
            credit(&mut player.run.record);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn track(
    phase: Res<State<Phase>>,
    run: Res<Run>,
    selection: Res<Selection>,
    account: Option<Res<talents::Account>>,
    inventory: Option<Res<super::items::Inventory>>,
    mut progress: ResMut<Achievements>,
) {
    if let Some(account) = account.as_deref().filter(|a| a.character.is_some()) {
        progress.max("level", u64::from(talents::level(account.xp)));
        progress.max("paragon", u64::from(super::paragon::progress(account.xp).level));
    }
    if let Some(inventory) = inventory.as_deref() {
        track_items(&mut progress, inventory);
    }
    let in_run = !selection.in_hub
        && selection.stage != data::TRAINING_MAP
        && matches!(
            *phase.get(),
            Phase::Playing | Phase::LevelUp | Phase::Paused | Phase::CharacterSheet | Phase::GameOver
        );
    if !in_run {
        progress.folded = Folded::default();
    } else {
        track_run(&mut progress, &run, selection.stage, inventory.as_deref());
    }
    let new = progress.evaluate();
    // Saves are queued and the writer holds them until a calm moment; during a run only a new
    // tier asks for one, the rest waits for its end.
    if progress.dirty && (new > 0 || !in_run || *phase.get() == Phase::GameOver) {
        progress.save();
    }
}

fn track_run(
    progress: &mut Achievements,
    run: &Run,
    map: usize,
    inventory: Option<&super::items::Inventory>,
) {
    // A new run starts over the folded values.
    if run.elapsed + 0.5 < progress.folded.elapsed || run.kills < progress.folded.kills {
        progress.folded = Folded::default();
    }
    let campaign = data::CAMPAIGN.contains(&map);
    if !progress.folded.started && run.elapsed > 1.0 {
        progress.folded.started = true;
        progress.add("runs", 1);
    }
    let kills = run.kills.saturating_sub(progress.folded.kills);
    progress.add("kills", u64::from(kills));
    if campaign {
        progress.add(&format!("map_kills:{map}"), u64::from(kills));
        progress.max(&format!("map_kills_run:{map}"), u64::from(run.kills));
    }
    progress.max("kills_run", u64::from(run.kills));
    for (skill, &count) in &run.record.spell_kills {
        let before = progress
            .folded
            .record
            .spell_kills
            .get(skill)
            .copied()
            .unwrap_or(0);
        if count > before {
            progress.add(&format!("spell:{skill}"), u64::from(count - before));
            if progress.counter(&format!("spell:{skill}")) >= 100 {
                progress.max(&format!("spell100:{skill}"), 1);
            }
        }
    }
    progress.add(
        "bosses",
        u64::from(run.record.boss_kills.saturating_sub(progress.folded.record.boss_kills)),
    );
    if let Some(seconds) = run.record.fastest_boss {
        if progress.folded.record.fastest_boss != Some(seconds) {
            if campaign {
                progress.best_time(&format!("boss_time:{map}"), f64::from(seconds));
            }
            progress.best_time("boss_time:any", f64::from(seconds));
        }
    }
    progress.max("damage_run", run.damage_done.max(0.0) as u64);
    progress.max("hit", run.record.biggest_hit.max(0.0) as u64);
    progress.max("legendary_run", u64::from(run.record.legendary_picks));
    progress.max("rerolls_run", u64::from(run.record.rerolls));
    progress.max("banishes_run", u64::from(run.record.banishes));
    progress.max("stored", u64::from(run.stored_cards));
    progress.max("spells_owned", run.abilities.len() as u64);
    progress.max(
        "max_spells",
        run.abilities
            .iter()
            .filter(|owned| owned.level >= data::MAX_ABILITY_LEVEL)
            .count() as u64,
    );
    for owned in &run.abilities {
        let key = owned.def.key;
        if owned.level >= data::MAX_ABILITY_LEVEL && !progress.folded.maxed.contains(&key) {
            progress.folded.maxed.push(key);
            progress.add(&format!("spell_max:{key}"), 1);
        }
    }
    let dead = run.dead_t.is_some();
    if run.hp > 0.0 && !dead {
        let share = run.hp / run.stats().max_hp.max(1.0);
        progress.folded.lowest_hp = Some(progress.folded.lowest_hp.map_or(share, |low| low.min(share)));
    }
    if dead && !progress.folded.dead {
        progress.add("deaths", 1);
    }
    if run.won && !progress.folded.won {
        progress.add("wins", 1);
        progress.max(&format!("class_win:{}", run.class.id), 1);
        if run.expedition_players > 1 {
            progress.add("coop_wins", 1);
        }
        if campaign {
            progress.add(&format!("map_wins:{map}"), 1);
            let naked = inventory.is_some_and(|i| super::items::SLOTS.iter().all(|(slot, _)| i.equipped(slot).is_none()));
            progress.victory(run, map, naked);
        }
        // A close call: the expedition won without falling, after dropping that low.
        if !progress.folded.dead && !dead {
            let low = progress.folded.lowest_hp.unwrap_or(1.0);
            for (key, threshold) in [("close_call:10", 0.10), ("close_call:5", 0.05), ("close_call:1", 0.01)] {
                if low < threshold {
                    progress.add(key, 1);
                }
            }
        }
    }
    progress.folded.dead |= dead;
    progress.folded.won |= run.won;
    progress.folded.kills = run.kills;
    progress.folded.elapsed = run.elapsed;
    progress.folded.record = run.record.clone();
}

fn track_items(progress: &mut Achievements, inventory: &super::items::Inventory) {
    use super::items::Rarity;
    if progress.inventory_revision == Some(inventory.revision) {
        return;
    }
    progress.inventory_revision = Some(inventory.revision);
    let mut levels = 0u64;
    let mut counts = [0u64; 4];
    for (slot, _) in super::items::SLOTS {
        let Some(item) = inventory.equipped(slot) else {
            continue;
        };
        counts[0] += 1;
        levels += u64::from(item.level);
        let rank = match item.rarity {
            Rarity::Normal => 0,
            Rarity::Magic => 1,
            Rarity::Epic => 2,
            Rarity::Legendary => 3,
        };
        for count in counts.iter_mut().take(rank + 1).skip(1) {
            *count += 1;
        }
        progress.max(&format!("type:{:?}", item.equipment_type), 1);
    }
    progress.max("slots:any", counts[0]);
    progress.max("slots:magic", counts[1]);
    progress.max("slots:epic", counts[2]);
    progress.max("slots:legendary", counts[3]);
    // Shirt and tabard carry no level: the average is over the other seventeen slots.
    progress.max("ilvl", levels / 17);
    let money = inventory.money();
    if let Some(old) = progress.money {
        progress.add("gold", money.saturating_sub(old));
    }
    progress.money = Some(money);
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Achievements>()
        .add_systems(Update, (
            time_bosses.run_if(super::multiplayer::authoritative),
            track,
        )
            .chain());
}

#[cfg(test)]
mod tests;
