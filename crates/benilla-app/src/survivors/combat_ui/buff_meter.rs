//! Sample actual aura state; never count a periodic tick as a new application.
use super::meter::{DiagnosticPage, DiagnosticSection};
use crate::survivors::{self as game, Run, enemies::Enemy};
use bevy::prelude::*;
use std::collections::{BTreeMap, VecDeque};
mod dots;
mod instances;
use instances::InstanceKey;
type Snapshots = BTreeMap<(String, String), Snapshot>;

fn observe(snapshots: &mut Snapshots, observation: Observation) {
    let snapshot = snapshots
        .entry((observation.key.clone(), observation.target.clone()))
        .or_default();
    *snapshot
        .instances
        .entry(observation.token.clone().into())
        .or_default() += 1;
    snapshot.stacks += 1;
    snapshot.power += observation.power;
    if let Some(old) = &mut snapshot.observation {
        old.until = match (old.until, observation.until) {
            (Some(a), Some(b)) => Some(a.max(b)),
            _ => None,
        };
    } else {
        snapshot.observation = Some(observation);
    }
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub(in crate::survivors) struct EffectMeter {
    entries: BTreeMap<String, Entry>,
    sampled_at: f32,
}
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct Entry {
    name: String,
    spell: u32,
    kind: String,
    tags: String,
    description: String,
    origin: String,
    applications: u64,
    refreshes: u64,
    removals: u64,
    peak_stacks: u32,
    duration_total: f64,
    duration_count: u64,
    duration_min: f32,
    duration_max: f32,
    power_min: f32,
    power_max: f32,
    power_seen: bool,
    uptime: f64,
    target_seconds: f64,
    first: f32,
    last: f32,
    targets: BTreeMap<String, Target>,
    history: VecDeque<String>,
    recent: VecDeque<(u64, u64)>,
}
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct Target {
    name: String,
    active: bool,
    until: Option<f32>,
    stacks: u32,
    power: f32,
    applications: u64,
    refreshes: u64,
    uptime: f64,
    instances: BTreeMap<InstanceKey, u32>,
}
#[derive(Clone)]
struct Observation {
    source: Option<u64>,
    key: String,
    name: String,
    spell: u32,
    kind: String,
    tags: String,
    description: String,
    origin: String,
    target: String,
    target_name: String,
    until: Option<f32>,
    power: f32,
    token: String,
}
#[derive(Default)]
struct Snapshot {
    observation: Option<Observation>,
    instances: BTreeMap<InstanceKey, u32>,
    stacks: u32,
    power: f32,
}

impl Entry {
    fn event(&mut self, now: f32, count: u64, message: String) {
        self.last = now;
        self.history.push_back(format!("{now:.2}s · {message}"));
        while self.history.len() > 128 {
            self.history.pop_front();
        }
        let tick = (now.max(0.0) * 10.0) as u64;
        while self
            .recent
            .front()
            .is_some_and(|(t, _)| tick.saturating_sub(*t) >= 50)
        {
            self.recent.pop_front();
        }
        if let Some((_, n)) = self.recent.back_mut().filter(|(t, _)| *t == tick) {
            *n += count;
        } else if count > 0 {
            self.recent.push_back((tick, count));
        }
    }
}
impl EffectMeter {
    #[cfg(test)]
    pub(super) fn fixture() -> Self {
        let mut meter = Self::default();
        let mut run = Run::default();
        run.hp = 100.0;
        game::support_spells::grant(&mut run, "battle_shout", 0.12, 8.0, None);
        let mut observations = Vec::new();
        allied(&run, &mut observations);
        meter.sample(0.0, observations);
        meter
    }

    #[cfg(test)]
    fn sample(&mut self, now: f32, observations: Vec<Observation>) {
        let mut snapshots = Snapshots::new();
        for observation in observations {
            observe(&mut snapshots, observation);
        }
        self.sample_snapshots(now, snapshots);
    }

    fn sample_snapshots(&mut self, now: f32, snapshots: Snapshots) {
        let before = self.sampled_at;
        let dt = (now - before).max(0.0);
        self.sampled_at = now;
        for (key, entry) in &mut self.entries {
            let mut any = 0.0_f32;
            let mut ended = Vec::new();
            for (id, target) in &mut entry.targets {
                if !target.active {
                    continue;
                }
                let seconds = target
                    .until
                    .map_or(dt, |end| (end.min(now) - before).clamp(0.0, dt));
                target.uptime += f64::from(seconds);
                entry.target_seconds += f64::from(seconds);
                any = any.max(seconds);
                if !snapshots.contains_key(&(key.clone(), id.clone())) {
                    target.active = false;
                    target.stacks = 0;
                    target.instances.clear();
                    entry.removals += 1;
                    ended.push(target.name.clone());
                }
            }
            entry.uptime += f64::from(any);
            for target in ended {
                entry.event(
                    now,
                    0,
                    format!("Removed / expired / target unavailable: {target}"),
                );
            }
        }
        for ((key, target_id), snapshot) in snapshots {
            let o = snapshot.observation.unwrap();
            let entry = self.entries.entry(key).or_insert_with(|| Entry {
                name: o.name.clone(),
                first: now,
                ..Default::default()
            });
            entry.name = o.name;
            entry.spell = o.spell;
            entry.kind = o.kind;
            entry.tags = o.tags;
            entry.description = o.description;
            entry.origin = o.origin;
            let target = entry.targets.entry(target_id).or_default();
            let new = snapshot
                .instances
                .iter()
                .map(|(token, n)| n.saturating_sub(*target.instances.get(token).unwrap_or(&0)))
                .sum::<u32>();
            let restarted = !target.active;
            let refreshed = !restarted
                && new == 0
                && (match (target.until, o.until) {
                    (Some(a), Some(b)) => b > a + 0.025,
                    (Some(_), None) => true,
                    _ => false,
                });
            let count = if restarted { snapshot.stacks } else { new };
            target.name = o.target_name;
            target.active = true;
            target.until = o.until;
            target.stacks = snapshot.stacks;
            target.power = snapshot.power;
            target.instances = snapshot.instances;
            target.applications += u64::from(count);
            target.refreshes += u64::from(refreshed);
            let target_name = target.name.clone();
            entry.applications += u64::from(count);
            entry.refreshes += u64::from(refreshed);
            entry.peak_stacks = entry.peak_stacks.max(snapshot.stacks);
            if !entry.power_seen {
                entry.power_min = snapshot.power;
                entry.power_seen = true;
            }
            entry.power_min = entry.power_min.min(snapshot.power);
            entry.power_max = entry.power_max.max(snapshot.power);
            if count > 0 || refreshed {
                if let Some(until) = o.until {
                    let duration = (until - now).max(0.0);
                    if entry.duration_count == 0 {
                        entry.duration_min = duration;
                    }
                    entry.duration_min = entry.duration_min.min(duration);
                    entry.duration_max = entry.duration_max.max(duration);
                    entry.duration_total += f64::from(duration);
                    entry.duration_count += 1;
                }
                entry.event(now,u64::from(count)+u64::from(refreshed),format!("{} · {target_name} · +{count} application(s) · {} stack(s) · remaining {} · magnitude {:.3}",if refreshed {"Refresh"} else {"Apply"},snapshot.stacks,o.until.map_or("persistent".into(),|end|format!("{:.2}s",(end-now).max(0.0))),snapshot.power));
            }
            // Keep active targets and a bounded set of completed target records.
            while entry.targets.len() > 256 {
                let old = entry
                    .targets
                    .iter()
                    .find(|(_, t)| !t.active)
                    .map(|(id, _)| id.clone());
                if let Some(old) = old {
                    entry.targets.remove(&old);
                } else {
                    break;
                }
            }
        }
    }
    pub(super) fn rows(&self, now: f32) -> Vec<super::meter::Row> {
        let tick = (now.max(0.0) * 10.0) as u64;
        self.entries
            .iter()
            .map(|(key, e)| super::meter::Row {
                key: key.clone(),
                total: (e.applications + e.refreshes) as f64,
                dps: e
                    .recent
                    .iter()
                    .filter(|(t, _)| tick.saturating_sub(*t) < 50)
                    .map(|(_, n)| *n as f64)
                    .sum::<f64>()
                    / f64::from(now.clamp(1.0, 5.0)),
            })
            .collect()
    }
    pub(super) fn identity(&self, key: &str) -> (String, u32) {
        self.entries.get(key).map_or((key.into(), 0), |e| {
            (format!("{} · {}", e.name, e.kind), e.spell)
        })
    }
    pub(super) fn details(&self, key: &str, now: f32) -> Vec<(String, String)> {
        let Some(e) = self.entries.get(key) else {
            return Vec::new();
        };
        vec![
            ("Effect type".into(), e.kind.clone()),
            ("Applications".into(), e.applications.to_string()),
            ("Refreshes".into(), e.refreshes.to_string()),
            (
                "Active targets".into(),
                e.targets.values().filter(|t| t.active).count().to_string(),
            ),
            ("Peak stacks / target".into(), e.peak_stacks.to_string()),
            (
                "Sampled uptime".into(),
                format!(
                    "{:.2}s ({:.1}%)",
                    e.uptime,
                    e.uptime / f64::from((now - e.first).max(0.001)) * 100.0
                ),
            ),
        ]
    }
    pub(super) fn debug_page(&self, key: &str, now: f32) -> DiagnosticPage {
        let mut page = DiagnosticPage {
            cards: Vec::new(),
            sections: Default::default(),
        };
        let Some(e) = self.entries.get(key) else {
            return page;
        };
        page.cards = vec![
            ("Applications".into(), e.applications.to_string()),
            ("Refreshes".into(), e.refreshes.to_string()),
            (
                "Active targets".into(),
                e.targets.values().filter(|t| t.active).count().to_string(),
            ),
            ("Sampled uptime".into(), format!("{:.2}s", e.uptime)),
        ];
        page.sections[0].push(DiagnosticSection::new(
            "Effect overview",
            self.details(key, now),
        ));
        page.sections[0].push(DiagnosticSection::new(
            "Coverage and magnitude",
            vec![
                (
                    "Target-seconds (all targets)".into(),
                    format!("{:.2}s", e.target_seconds),
                ),
                (
                    "Magnitude min / max".into(),
                    format!("{:.3} / {:.3}", e.power_min, e.power_max),
                ),
                (
                    "First / last event".into(),
                    format!("{:.2}s / {:.2}s", e.first, e.last),
                ),
            ],
        ));
        page.sections[1].push(DiagnosticSection::new(
            "Application durations",
            vec![
                ("Duration samples".into(), e.duration_count.to_string()),
                (
                    "Min / average / max".into(),
                    if e.duration_count == 0 {
                        "Persistent / unavailable".into()
                    } else {
                        format!(
                            "{:.2}s / {:.2}s / {:.2}s",
                            e.duration_min,
                            e.duration_total / e.duration_count as f64,
                            e.duration_max
                        )
                    },
                ),
                (
                    "Removals / expirations observed".into(),
                    e.removals.to_string(),
                ),
            ],
        ));
        let mut history = DiagnosticSection::new("Recent effect events", Vec::new());
        history.note = e
            .history
            .iter()
            .rev()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        page.sections[1].push(history);
        for target in e.targets.values() {
            page.sections[2].push(DiagnosticSection::new(
                &target.name,
                vec![
                    (
                        "State".into(),
                        if target.active { "Active" } else { "Ended" }.into(),
                    ),
                    ("Stacks".into(), target.stacks.to_string()),
                    (
                        "Remaining duration".into(),
                        if !target.active {
                            "0.00s".into()
                        } else {
                            target.until.map_or("Persistent".into(), |end| {
                                format!("{:.2}s", (end - now).max(0.0))
                            })
                        },
                    ),
                    (
                        "Applications / refreshes".into(),
                        format!("{} / {}", target.applications, target.refreshes),
                    ),
                    ("Sampled uptime".into(), format!("{:.2}s", target.uptime)),
                    ("Current magnitude".into(), format!("{:.3}", target.power)),
                ],
            ));
        }
        let mut effect = DiagnosticSection::new(
            "Observed effect metadata",
            vec![
                ("Effect".into(), e.name.clone()),
                ("Type".into(), e.kind.clone()),
                ("Spell ID".into(), e.spell.to_string()),
                ("Gameplay tags".into(), e.tags.clone()),
                ("Attribution".into(), e.origin.clone()),
            ],
        );
        effect.note = e.description.clone();
        page.sections[3].push(effect);
        let mut notes = DiagnosticSection::new("How effects are measured", Vec::new());
        notes.note="Actual aura state is sampled after simulation. New instances and added DoT stacks count as applications; extending an existing instance counts as a refresh. Ticks do not count as new applications.\n\nApplications that start and end between samples cannot be observed. Existing effects present after a meter reset seed the new window. Uptime is sampled in simulation seconds; target-seconds sum coverage across targets.\n\nSupport spells and DoTs retain caster ownership. Other self-state auras use recipient attribution, shown in Effect details. Shared control timers use the last source that changed their state.\n\nThe most recent 128 events and up to 256 completed target records are retained per effect. Active targets are never discarded. Magnitude is the raw effect value; DoT magnitude is damage per second, not damage dealt.".into();
        page.sections[4].push(notes);
        page
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aura(target: &str, token: &str, until: f32) -> Observation {
        Observation {
            source: Some(42),
            key: "test".into(),
            name: "Test effect".into(),
            spell: 1,
            kind: "Buff on ally".into(),
            tags: "Test".into(),
            description: "Fixture".into(),
            origin: "Player 42".into(),
            target: target.into(),
            target_name: target.into(),
            until: Some(until),
            power: 10.0,
            token: token.into(),
        }
    }

    #[test]
    fn refreshes_and_target_uptime_are_distinct_from_applications() {
        let mut meter = EffectMeter::default();
        meter.sample(0.0, vec![aura("ally", "aura", 10.0)]);
        meter.sample(1.0, vec![aura("ally", "aura", 10.0)]);
        assert_eq!(meter.entries["test"].applications, 1);
        assert_eq!(meter.entries["test"].refreshes, 0);
        meter.sample(
            2.0,
            vec![aura("ally", "aura", 12.0), aura("other", "aura", 12.0)],
        );
        meter.sample(3.0, vec![]);
        let entry = &meter.entries["test"];
        assert_eq!(
            (entry.applications, entry.refreshes, entry.removals),
            (2, 1, 2)
        );
        assert_eq!(entry.uptime, 3.0);
        assert_eq!(entry.target_seconds, 4.0);
        assert!(entry.targets.values().all(|target| !target.active));
        let wire = serde_json::to_string(&meter).unwrap();
        let restored: EffectMeter = serde_json::from_str(&wire).unwrap();
        assert_eq!(restored.entries["test"].refreshes, 1);
        assert_eq!(restored.rows(3.0).len(), 1);
        assert!(!restored.debug_page("test", 3.0).sections[0].is_empty());
    }

    #[test]
    fn dots_count_new_stacks_without_counting_ticks_and_expiry_caps_uptime() {
        let mut meter = EffectMeter::default();
        let stack = || {
            (0..24)
                .map(|i| aura("enemy", &i.to_string(), 5.0))
                .collect()
        };
        meter.sample(0.0, stack());
        meter.sample(1.0, stack());
        assert_eq!(meter.entries["test"].applications, 24);
        assert_eq!(meter.entries["test"].peak_stacks, 24);
        let mut replacement = stack();
        replacement.pop();
        replacement.push(aura("enemy", "new stack", 5.0));
        meter.sample(2.0, replacement);
        assert_eq!(meter.entries["test"].applications, 25);
        meter.sample(10.0, vec![]);
        assert_eq!(meter.entries["test"].uptime, 5.0);
        assert_eq!(meter.entries["test"].targets["enemy"].stacks, 0);
    }

    #[test]
    fn tracker_records_actual_support_state_and_reset_starts_a_new_window() {
        let mut app = App::new();
        let mut run = Run::default();
        run.hp = 100.0;
        game::support_spells::grant(&mut run, "battle_shout", 0.12, 8.0, None);
        app.insert_resource(run).add_systems(Update, track);
        app.update();
        assert_eq!(app.world().resource::<Run>().effect_meter.entries.len(), 1);
        app.world_mut().resource_mut::<Run>().elapsed = 1.0;
        app.update();
        let run = app.world().resource::<Run>();
        let entry = run.effect_meter.entries.values().next().unwrap();
        assert_eq!((entry.applications, entry.refreshes), (1, 0));
        assert_eq!(entry.uptime, 1.0);
        let mut run = app.world_mut().resource_mut::<Run>();
        game::multiplayer::reset_meters(&mut run, None);
        assert!(run.effect_meter.entries.is_empty());
        run.elapsed = 9.0;
        app.update();
        assert!(
            app.world()
                .resource::<Run>()
                .effect_meter
                .entries
                .is_empty()
        );
    }
}

#[cfg(test)]
mod ownership_tests {
    use super::*;
    #[test]
    fn multiplayer_buffs_are_credited_to_the_caster_instead_of_the_recipient() {
        let mut app = App::new();
        let mut host = Run::default();
        host.hp = 100.0;
        game::support_spells::grant(&mut host, "battle_shout", 0.12, 8.0, Some(42));
        let mut guest = Run::default();
        guest.owner = Some(42);
        guest.hp = 100.0;
        let entity = app.world_mut().spawn_empty().id();
        let mut companions = game::multiplayer::Companions::default();
        companions.0.insert(
            42,
            game::multiplayer::Companion {
                entity,
                run: guest,
                physics: game::hero::Hero { vy: 0.0, air: 0.0 },
                offer: default(),
                offer_id: 0,
            },
        );
        app.insert_resource(host)
            .insert_resource(companions)
            .add_systems(Update, track);
        app.update();
        assert!(
            app.world()
                .resource::<Run>()
                .effect_meter
                .entries
                .is_empty()
        );
        let companions = app.world().resource::<game::multiplayer::Companions>();
        let effect = companions.0[&42]
            .run
            .effect_meter
            .entries
            .values()
            .next()
            .unwrap();
        assert_eq!(effect.applications, 1);
        assert!(effect.targets.contains_key("ally:None"));
        assert!(effect.origin.contains("42"));
    }
}

#[derive(Component, Default)]
pub(in crate::survivors) struct ControlSources(BTreeMap<usize, (Option<u64>, String)>);
pub(in crate::survivors) fn control_source(
    commands: &mut Commands,
    target: Entity,
    kind: usize,
    owner: Option<u64>,
    skill: &str,
) {
    let skill = skill.to_owned();
    commands.queue(move |world: &mut World| {
        if let Some(mut source) = world.get_mut::<ControlSources>(target) {
            source.0.insert(kind, (owner, skill));
        } else if let Ok(mut entity) = world.get_entity_mut(target) {
            let mut source = ControlSources::default();
            source.0.insert(kind, (owner, skill));
            entity.insert(source);
        }
    });
}
fn control_state(enemy: &Enemy) -> [(f32, f32); 6] {
    [
        (enemy.slow_t, 1.0 - enemy.slow),
        (enemy.root_t, 1.0),
        (enemy.stun_t, 1.0),
        (enemy.fear_t, 1.0),
        (enemy.weaken_t, 1.0 - enemy.weaken),
        (0.0, 0.0),
    ]
}
pub(in crate::survivors) fn control_changes(
    commands: &mut Commands,
    target: Entity,
    owner: Option<u64>,
    skill: &str,
    before: [f32; 5],
    enemy: &Enemy,
) {
    for (kind, (remaining, _)) in control_state(enemy).into_iter().take(5).enumerate() {
        if remaining > before[kind] + 0.0001 {
            control_source(commands, target, kind, owner, skill);
        }
    }
}
fn allied(run: &Run, out: &mut Vec<Observation>) {
    if run.hp <= 0.0 || run.dead_t.is_some() {
        return;
    }
    for aura in game::player_auras::collect(run) {
        let support = run
            .support
            .buffs
            .iter()
            .find(|b| b.until > run.elapsed && game::data::ability(b.key).spell == aura.spell);
        let source = support.map_or(run.owner, |b| b.caster);
        let kind = if aura.debuff {
            "Debuff on ally"
        } else {
            "Buff on ally"
        };
        out.push(Observation {
            source,
            key: format!("{kind}:{}:{}", aura.spell, aura.name),
            name: aura.name,
            spell: aura.spell,
            kind: kind.into(),
            tags: support.map_or("Aura state".into(), |b| {
                game::spell_engine::tags::for_spell(game::data::ability(b.key)).labels()
            }),
            description: aura.description,
            origin: if support.is_some() {
                format!(
                    "Caster: {}",
                    source.map_or("Host".into(), |id| format!("Player {id}"))
                )
            } else {
                "Recipient self-state (caster not exposed by aura)".into()
            },
            target: format!("ally:{:?}", run.owner),
            target_name: run
                .owner
                .map_or("Host / self".into(), |id| format!("Ally {id}")),
            until: aura.left.map(|left| run.elapsed + left),
            power: support.map_or(0.0, |b| b.power),
            token: "aura".into(),
        });
    }
}

pub(in crate::survivors) fn track(
    mut run: ResMut<Run>,
    mut players: Option<ResMut<game::multiplayer::Companions>>,
    enemies: Query<(Entity, &Enemy, Option<&ControlSources>)>,
    debuffs: Option<Res<game::support_spells::Debuffs>>,
    allies: Query<
        (
            &game::unit_frames::Status,
            Option<&game::multiplayer::RemoteOwner>,
        ),
        Or<(With<game::hero::Hero>, With<game::multiplayer::RemoteOwner>)>,
    >,
) {
    let mut all = Vec::new();
    let mut grouped = BTreeMap::<Option<u64>, Snapshots>::new();
    allied(&run, &mut all);
    if let Some(players) = players.as_deref() {
        for player in players.0.values() {
            allied(&player.run, &mut all);
        }
    }
    for (status, remote) in &allies {
        if status.health <= 0.0 {
            continue;
        }
        let owner = remote.map(|r| r.0);
        for &(spell, left) in status.debuffs.iter().filter(|(_, left)| *left > 0.0) {
            let def = game::data::ABILITIES.iter().find(|d| d.spell == spell);
            all.push(Observation {
                source: owner,
                key: format!("received:{spell}"),
                name: def.map_or_else(|| format!("Spell {spell}"), |d| d.key.replace('_', " ")),
                spell,
                kind: "Debuff received by ally".into(),
                tags: def.map_or("Unknown".into(), |d| {
                    game::spell_engine::tags::for_spell(d).labels()
                }),
                description: "Observed from the ally's active debuff list.".into(),
                origin: "Recipient attribution; debuff caster is not exposed by status".into(),
                target: format!("ally:{owner:?}"),
                target_name: owner.map_or("Host / self".into(), |id| format!("Ally {id}")),
                until: Some(run.elapsed + left),
                power: 0.0,
                token: "received".into(),
            });
        }
    }
    for (entity, enemy, sources) in &enemies {
        if enemy.dead || enemy.hp <= 0.0 {
            continue;
        }
        let target = format!("enemy:{}", entity.to_bits());
        let target_name = format!("{} #{}", enemy.def.name, entity.to_bits());
        dots::collect(run.elapsed, entity, enemy, &mut grouped);
        if let Some(sources) = sources {
            for (&kind, (source, skill)) in &sources.0 {
                let (left, power) = control_state(enemy)[kind];
                if left <= 0.0 {
                    continue;
                }
                let label = ["Slow", "Root", "Stun", "Fear", "Weaken", "Other"][kind];
                let def = game::data::ability(skill);
                all.push(Observation {
                    source: *source,
                    key: format!("control:{skill}:{kind}"),
                    name: format!("{} · {label}", skill.replace('_', " ")),
                    spell: def.spell,
                    kind: "Control on enemy".into(),
                    tags: game::spell_engine::tags::for_spell(def).labels(),
                    description: format!("{label} measured from the target's live control timer."),
                    origin: "Last caster to extend the shared control timer".into(),
                    target: target.clone(),
                    target_name: target_name.clone(),
                    until: Some(run.elapsed + left),
                    power,
                    token: "control".into(),
                });
            }
        }
    }
    if let Some(debuffs) = debuffs {
        for (target, buff) in debuffs.active(run.elapsed) {
            let Ok((_, enemy, _)) = enemies.get(target) else {
                continue;
            };
            if enemy.dead {
                continue;
            }
            let def = game::data::ability(buff.key);
            all.push(Observation {
                source: buff.caster,
                key: format!("debuff:{}", buff.key),
                name: buff.key.replace('_', " "),
                spell: def.spell,
                kind: "Debuff on enemy".into(),
                tags: game::spell_engine::tags::for_spell(def).labels(),
                description: game::support_spells::description(buff.key)
                    .unwrap_or("Enemy debuff")
                    .into(),
                origin: format!("Support spell · Caster {:?}", buff.caster),
                target: format!("enemy:{}", target.to_bits()),
                target_name: format!("{} #{}", enemy.def.name, target.to_bits()),
                until: Some(buff.until),
                power: buff.power,
                token: "debuff".into(),
            });
        }
    }
    for observation in all {
        observe(grouped.entry(observation.source).or_default(), observation);
    }
    let now = run.elapsed;
    let owner = run.owner;
    run.effect_meter
        .sample_snapshots(now, grouped.remove(&owner).unwrap_or_default());
    if let Some(players) = players.as_deref_mut() {
        for player in players.0.values_mut() {
            let now = player.run.elapsed;
            player
                .run
                .effect_meter
                .sample_snapshots(now, grouped.remove(&player.run.owner).unwrap_or_default());
        }
    }
}
