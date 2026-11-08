//! Observed damage, separate from the current spell-resolution snapshot.
use super::*;
use crate::survivors::{items::legendary::events::Origin, spell_engine::Tags};

pub(in crate::survivors) struct DamageEvent<'a> {
    pub skill: &'a str,
    pub tags: Tags,
    pub origin: Origin,
    pub item: Option<u16>,
    pub cast: u64,
    pub amount: f32,
    pub effective: f32,
    pub critical: bool,
    pub periodic: bool,
    pub target: &'a str,
    pub family: &'a str,
}

#[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct Diagnostics {
    casts: u64,
    repeats: u64,
    extra_casts: u64,
    triggers: u64,
    applications: u64,
    first: Option<f32>,
    last: Option<f32>,
    breakdown: BTreeMap<String, Bucket>,
    #[serde(skip)]
    seen: BTreeMap<String, f32>,
}

#[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Bucket {
    samples: Samples,
    critical: u64,
    amount: f64,
    excess: f64,
    periodic: bool,
    triggered: bool,
}

fn origin_name(origin: Origin) -> &'static str {
    match origin {
        Origin::Skill => "Skill",
        Origin::Talent => "Talent / pet",
        Origin::Item => "Item affix",
        Origin::Legendary => "Legendary",
    }
}

#[cfg(test)]
fn ratio(value: f64, count: u64) -> String {
    if count == 0 {
        "N/A (no measured activation)".into()
    } else {
        format!("{:.3}", value / count as f64)
    }
}

pub(in crate::survivors::combat_ui) struct DiagnosticSection {
    pub title: String,
    pub rows: Vec<(String, String)>,
    pub note: String,
}
impl DiagnosticSection {
    pub fn new(title: impl Into<String>, rows: Vec<(String, String)>) -> Self {
        Self {
            title: title.into(),
            rows,
            note: String::new(),
        }
    }
}
pub(in crate::survivors::combat_ui) struct DiagnosticPage {
    pub cards: Vec<(String, String)>,
    pub sections: [Vec<DiagnosticSection>; 5],
}

fn number(value: f64) -> String {
    let raw = format!("{:.3}", if value.abs() < 0.0005 { 0.0 } else { value });
    let (integer, fraction) = raw.split_once('.').unwrap();
    let mut grouped = String::new();
    for (index, digit) in integer.chars().enumerate() {
        if index > 0 && (integer.len() - index) % 3 == 0 {
            grouped.push(' ');
        }
        grouped.push(digit);
    }
    format!("{grouped}.{fraction}")
}

impl Meter {
    pub(in crate::survivors::combat_ui) fn debug_page(
        &self,
        key: &str,
        elapsed: f32,
        boss: bool,
    ) -> DiagnosticPage {
        let mut page = DiagnosticPage {
            cards: Vec::new(),
            sections: Default::default(),
        };
        let Some(e) = self.entries.get(key) else {
            return page;
        };
        let d = &e.debug;
        let direct = e.normal.total + e.critical.total;
        let hits = e.normal.count + e.critical.count;
        let casts = d.casts + d.repeats;
        let observed: f64 = d.breakdown.values().map(|b| b.samples.total).sum();
        let trigger_damage: f64 = d
            .breakdown
            .values()
            .filter(|b| b.triggered)
            .map(|b| b.samples.total)
            .sum();
        let skill_damage: f64 = d
            .breakdown
            .values()
            .filter(|b| !b.triggered)
            .map(|b| b.samples.total)
            .sum();
        let count = |triggered, periodic| {
            d.breakdown
                .values()
                .filter(|b| b.triggered == triggered && b.periodic == periodic)
                .map(|b| b.samples.count)
                .sum::<u64>()
        };
        let share = |value| format!("{:.1} %", value / e.total.max(f64::MIN_POSITIVE) * 100.0);
        let rate = |value, count| {
            if count == 0 {
                "—".into()
            } else {
                number(value / count as f64)
            }
        };
        page.cards = vec![
            ("Effective damage".into(), number(e.total)),
            (
                "Average DPS over the run".into(),
                number(e.total / f64::from(elapsed.max(1.0))),
            ),
            ("HIT · Damage share".into(), share(direct)),
            ("DoT · Damage share".into(), share(e.periodic.total)),
        ];
        page.sections[0].push(DiagnosticSection::new(
            "Damage breakdown",
            vec![
                ("Direct damage (HIT)".into(), number(direct)),
                ("Periodic damage (DoT)".into(), number(e.periodic.total)),
                ("Direct hits".into(), hits.to_string()),
                ("DoT ticks".into(), e.periodic.count.to_string()),
                (
                    "HIT critical rate".into(),
                    format!(
                        "{:.1} %",
                        e.critical.count as f64 / hits.max(1) as f64 * 100.0
                    ),
                ),
                (
                    "Damage events".into(),
                    (hits + e.periodic.count).to_string(),
                ),
            ],
        ));
        let profile = if observed == 0.0 {
            "Detailed data unavailable"
        } else if trigger_damage > 0.0 && skill_damage > 0.0 {
            "Mixed: skill and triggered damage"
        } else if trigger_damage > 0.0 {
            "Trigger / Proc"
        } else {
            "Spell / Skill"
        };
        page.sections[0].push(DiagnosticSection::new(
            "Origin and timeline",
            vec![
                ("Execution profile".into(), profile.into()),
                ("Skill damage".into(), number(skill_damage)),
                ("Triggered damage".into(), number(trigger_damage)),
                (
                    "First damage in run".into(),
                    d.first.map_or("—".into(), |t| format!("{t:.2} s")),
                ),
                (
                    "Last damage in run".into(),
                    d.last.map_or("—".into(), |t| format!("{t:.2} s")),
                ),
            ],
        ));
        for (label, s) in [
            ("Normal HITs", &e.normal),
            ("Critical HITs", &e.critical),
            ("DoT ticks", &e.periodic),
        ] {
            page.sections[0].push(DiagnosticSection::new(
                label,
                vec![
                    ("Count".into(), s.count.to_string()),
                    ("Total damage".into(), number(s.total)),
                    (
                        "Minimum".into(),
                        if s.count == 0 {
                            "—".into()
                        } else {
                            number(s.min)
                        },
                    ),
                    (
                        "Average per event".into(),
                        if s.count == 0 {
                            "—".into()
                        } else {
                            number(s.average())
                        },
                    ),
                    (
                        "Maximum".into(),
                        if s.count == 0 {
                            "—".into()
                        } else {
                            number(s.max)
                        },
                    ),
                ],
            ));
        }
        page.sections[1].push(DiagnosticSection::new(
            "Casts and activations",
            vec![
                ("Original casts".into(), d.casts.to_string()),
                ("Repeats".into(), d.repeats.to_string()),
                ("Casts including repeats".into(), casts.to_string()),
                (
                    "Extra triggered spell casts".into(),
                    d.extra_casts.to_string(),
                ),
                (
                    "Admitted triggers / DoT activations".into(),
                    d.triggers.to_string(),
                ),
                (
                    "DoT applications per target".into(),
                    if boss {
                        "Unavailable in the boss filter".into()
                    } else {
                        d.applications.to_string()
                    },
                ),
            ],
        ));
        page.sections[1].push(DiagnosticSection::new(
            "Per-cast efficiency",
            vec![
                (
                    "Skill HIT / cast".into(),
                    rate(count(false, false) as f64, casts),
                ),
                (
                    "Skill ticks / cast".into(),
                    rate(count(false, true) as f64, casts),
                ),
                (
                    "Skill damage / cast".into(),
                    rate(skill_damage, casts),
                ),
            ],
        ));
        page.sections[1].push(DiagnosticSection::new(
            "Per-trigger efficiency",
            vec![
                (
                    "Triggered HIT / trigger".into(),
                    rate(count(true, false) as f64, d.triggers),
                ),
                (
                    "Triggered ticks / trigger".into(),
                    rate(count(true, true) as f64, d.triggers),
                ),
                (
                    "Triggered damage / trigger".into(),
                    rate(trigger_damage, d.triggers),
                ),
            ],
        ));
        for (label, bucket) in &d.breakdown {
            let parts: Vec<_> = label.split(" | ").collect();
            let s = &bucket.samples;
            let mut section = DiagnosticSection::new(
                format!(
                    "{} · {}",
                    parts.get(1).unwrap_or(&"Damage"),
                    parts.get(2).unwrap_or(&"Channel")
                ),
                vec![
                    (
                        "Internal spell".into(),
                        parts.first().unwrap_or(&label.as_str()).to_string(),
                    ),
                    ("Origin".into(), parts.get(3).unwrap_or(&"—").to_string()),
                    (
                        "Target type".into(),
                        parts.get(4).unwrap_or(&"—").to_string(),
                    ),
                    (
                        "Gameplay tags / Damage schools".into(),
                        parts.get(5).unwrap_or(&"—").to_string(),
                    ),
                    (
                        "Item identity".into(),
                        parts.get(6).unwrap_or(&"—").to_string(),
                    ),
                    (
                        "Events / Critical hits".into(),
                        format!("{} / {}", s.count, bucket.critical),
                    ),
                    (
                        "Effective damage".into(),
                        format!("{} · {}", number(s.total), share(s.total)),
                    ),
                    (
                        "Final damage before HP cap".into(),
                        number(bucket.amount),
                    ),
                    ("Excess / Overkill".into(), number(bucket.excess)),
                    (
                        "Minimum / Average / Maximum".into(),
                        format!(
                            "{} / {} / {}",
                            number(s.min),
                            number(s.average()),
                            number(s.max)
                        ),
                    ),
                ],
            );
            section.note =
                "Actual damage after mitigation, broken down by channel.".into();
            page.sections[2].push(section);
        }
        if page.sections[2].is_empty() {
            page.sections[2].push(DiagnosticSection::new(
                "Damage channels",
                vec![(
                    "Status".into(),
                    "No detailed damage channels recorded since reset.".into(),
                )],
            ));
        }
        page.sections[4].push(DiagnosticSection::new(
            "Measurement scope",
            vec![
                ("Attribution source".into(), key.into()),
                (
                    "Damage with detailed diagnostics".into(),
                    format!("{} / {}", number(observed), number(e.total)),
                ),
                (
                    "Filter".into(),
                    if boss {
                        "Bosses, elites and training dummy".into()
                    } else {
                        "All targets".into()
                    },
                ),
            ],
        ));
        let mut notes = DiagnosticSection::new("How to read these numbers", Vec::new());
        notes.note = "A HIT counts one event per target, not an animation or projectile. DoT ticks are not casts or triggers.\n\nTriggers share a source / origin / item / parent cast identity. Legacy ID 0 uses the engine's time-based grouping. Deduplication history is limited to 310 s and 4,096 activations per source.\n\nDamage is measured after mitigation. Damage before the HP cap is not damage before armor. Training dummies do not cap damage by their HP.\n\nCasts include those made against other targets in the boss tab. Older snapshots may lack diagnostics. A dash means no denominator was measured.\n\nThe current build is a snapshot; it does not reconstruct the modifiers of every past impact.".into();
        page.sections[4].push(notes);
        page
    }
}

impl Meter {
    pub(in crate::survivors) fn debug_cast(&mut self, key: &str, repeat: bool, extra: bool) {
        let debug = &mut self.entries.entry(key.into()).or_default().debug;
        if extra {
            debug.extra_casts += 1;
        } else if repeat {
            debug.repeats += 1;
        } else {
            debug.casts += 1;
        }
    }

    /// Target copies and periodic ticks reuse their parent activation identity.
    pub(in crate::survivors) fn debug_activation(
        &mut self,
        key: &str,
        skill: &str,
        now: f32,
        cast: u64,
        origin: Origin,
        item: Option<u16>,
        application: bool,
    ) {
        let debug = &mut self.entries.entry(key.into()).or_default().debug;
        if application {
            debug.applications += 1;
        }
        if origin == Origin::Skill {
            return;
        }
        let token = format!(
            "{skill}:{origin:?}:{item:?}:{}",
            if cast != 0 {
                cast
            } else {
                (1 << 62) | (f64::from(now.max(0.0)) * 10000.0) as u64
            }
        );
        if debug.seen.contains_key(&token) {
            return;
        }
        debug.seen.retain(|_, at| now - *at < 310.0);
        if debug.seen.len() >= 4096 {
            if let Some(oldest) = debug
                .seen
                .iter()
                .min_by(|a, b| a.1.total_cmp(b.1))
                .map(|(key, _)| key.clone())
            {
                debug.seen.remove(&oldest);
            }
        }
        debug.seen.insert(token, now);
        debug.triggers += 1;
    }

    pub(in crate::survivors) fn record_damage(
        &mut self,
        key: &str,
        now: f32,
        event: DamageEvent<'_>,
    ) {
        if !event.effective.is_finite() || event.effective <= 0.0 {
            return;
        }
        if !event.periodic || event.cast != 0 {
            self.debug_activation(
                key,
                event.skill,
                now,
                event.cast,
                event.origin,
                event.item,
                false,
            );
        }
        self.record_hit(key, now, event.effective, event.critical, event.periodic);
        let debug = &mut self.entries.get_mut(key).unwrap().debug;
        debug.first.get_or_insert(now);
        debug.last = Some(now);
        let label = format!(
            "{} | {} | {} | {} | {} | {} | item {:?}",
            event.skill,
            if event.periodic { "DoT" } else { "HIT" },
            event.family,
            origin_name(event.origin),
            event.target,
            event.tags.labels(),
            event.item
        );
        let bucket = debug.breakdown.entry(label).or_default();
        bucket.periodic = event.periodic;
        bucket.triggered = event.origin != Origin::Skill;
        bucket.samples.add(f64::from(event.effective));
        bucket.critical += u64::from(event.critical);
        if event.amount.is_finite() {
            bucket.amount += f64::from(event.amount.max(0.0));
            bucket.excess += f64::from((event.amount - event.effective).max(0.0));
        }
    }

    #[cfg(test)]
    pub(in crate::survivors::combat_ui) fn debug_report(&self, key: &str, elapsed: f32) -> String {
        let Some(entry) = self.entries.get(key) else {
            return "No observations since reset.".into();
        };
        let debug = &entry.debug;
        let hits = entry.normal.count + entry.critical.count;
        let casts = debug.casts + debug.repeats;
        let direct: f64 = entry.normal.total + entry.critical.total;
        let observed: f64 = debug.breakdown.values().map(|b| b.samples.total).sum();
        let triggered_hits: u64 = debug
            .breakdown
            .values()
            .filter(|b| b.triggered && !b.periodic)
            .map(|b| b.samples.count)
            .sum();
        let skill_hits: u64 = debug
            .breakdown
            .values()
            .filter(|b| !b.triggered && !b.periodic)
            .map(|b| b.samples.count)
            .sum();
        let trigger_damage: f64 = debug
            .breakdown
            .values()
            .filter(|b| b.triggered)
            .map(|b| b.samples.total)
            .sum();
        let skill_damage: f64 = debug
            .breakdown
            .values()
            .filter(|b| !b.triggered)
            .map(|b| b.samples.total)
            .sum();
        let share = |v: f64| v / entry.total.max(f64::MIN_POSITIVE) * 100.0;
        let mut out = format!(
            "OBSERVED SINCE RESET\nSource: {key}\nEffective damage: {:.3} | Average DPS / run: {:.3}\nHIT: {direct:.3} ({:.2}%) | DoT: {:.3} ({:.2}%)\nDirect hits: {hits} | Normal: {} | Critical: {} | Critical rate / HIT: {:.2}%\nDoT ticks: {} | All damage events: {}\n\nCASTS AND TRIGGERS\nOriginal casts: {} | Repeats: {} | Extra triggered spell casts: {}\nMeasured casts including repeats: {casts}\nAdmitted damaging triggers / DoT activations: {}\nDoT applications (per target): {}\nSkill HIT / cast: {} | Skill damage / cast: {}\nTriggered HIT / trigger: {} | Triggered damage / trigger: {}\nAll events / cast: {}\nTrigger damage: {trigger_damage:.3} | Skill damage: {skill_damage:.3}\nFirst damage: {:?}s | Last damage: {:?}s\n\nDISTRIBUTIONS (effective damage per target event)\n",
            entry.total, entry.total / f64::from(elapsed.max(1.0)), share(direct), entry.periodic.total, share(entry.periodic.total),
            entry.normal.count, entry.critical.count, entry.critical.count as f64 / hits.max(1) as f64 * 100.0,
            entry.periodic.count, hits + entry.periodic.count,
            debug.casts, debug.repeats, debug.extra_casts, debug.triggers, debug.applications,
            ratio(skill_hits as f64, casts), ratio(skill_damage, casts), ratio(triggered_hits as f64, debug.triggers), ratio(trigger_damage, debug.triggers),
            ratio((hits + entry.periodic.count) as f64, casts), debug.first, debug.last);
        for (label, samples) in [
            ("Normal HIT", &entry.normal),
            ("Critical HIT", &entry.critical),
            ("DoT tick", &entry.periodic),
        ] {
            out.push_str(&format!(
                "{label}: count {} | total {:.3} | min {:.3} | avg {:.3} | max {:.3}\n",
                samples.count,
                samples.total,
                samples.min,
                samples.average(),
                samples.max
            ));
        }
        out.push_str(
            "\nACTUAL DAMAGE CHANNELS (school / tags / origin / target / periodic family)\n",
        );
        for (label, bucket) in &debug.breakdown {
            let s = &bucket.samples;
            out.push_str(&format!("\n{label}\n  Events: {} | Crits: {} | Effective: {:.3} ({:.2}%)\n  Final before HP cap: {:.3} | Excess / overkill: {:.3}\n  Min / avg / max: {:.3} / {:.3} / {:.3}\n", s.count, bucket.critical, s.total, share(s.total), bucket.amount, bucket.excess, s.min, s.average(), s.max));
        }
        out.push_str(&format!("\nMEASUREMENT NOTES\nDetailed damage coverage: {observed:.3} / {:.3}. Older snapshots may lack diagnostics.\nHIT counts one event per target, not one projectile or animation. DoT ticks never count as casts or triggers.\nTrigger activations are deduplicated by source, origin, item and cast/parent ID; legacy ID 0 uses the engine's frame-time grouping. History is bounded to 310s / 4096 activations per source.\nBoss-tab damage is filtered; cast counts include casts made while fighting other targets.\nDamage is after mitigation; final damage before HP cap is not pre-armour damage. Training dummies do not cap damage by HP.\n", entry.total));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostics_separate_casts_triggers_target_copies_ticks_and_overkill() {
        let mut meter = Meter::default();
        meter.debug_cast("fireball", false, false);
        meter.debug_cast("fireball", true, false);
        meter.debug_cast("fireball", false, true);
        for (origin, cast, periodic, critical, effective, amount, tags) in [
            (Origin::Skill, 1, false, false, 10.0, 10.0, Tags::PHYSICAL),
            (Origin::Skill, 2, false, true, 30.0, 30.0, Tags::PHYSICAL),
            (Origin::Talent, 42, false, false, 20.0, 100.0, Tags::FIRE),
            (Origin::Talent, 42, false, false, 10.0, 10.0, Tags::FIRE),
            (Origin::Talent, 42, true, false, 3.0, 3.0, Tags::FIRE),
            (Origin::Talent, 42, true, false, 3.0, 3.0, Tags::FIRE),
        ] {
            meter.record_damage(
                "fireball",
                1.0,
                DamageEvent {
                    skill: "fireball",
                    tags,
                    origin,
                    item: None,
                    cast,
                    amount,
                    effective,
                    critical,
                    periodic,
                    target: "Elite",
                    family: if periodic { "Ignite / burn" } else { "Direct" },
                },
            );
        }
        meter.debug_activation("fireball", "fireball", 1.0, 42, Origin::Talent, None, true);
        let entry = &meter.entries["fireball"];
        assert_eq!(entry.total, 76.0);
        assert_eq!(entry.debug.triggers, 1);
        assert_eq!(entry.debug.applications, 1);
        assert_eq!(
            entry
                .debug
                .breakdown
                .values()
                .map(|b| b.excess)
                .sum::<f64>(),
            80.0
        );
        let page = meter.debug_page("fireball", 5.0, false);
        assert_eq!(page.cards[0].1, "76.000");
        assert_eq!(page.sections[2].len(), 3);
        assert!(page.sections[1]
            .iter()
            .flat_map(|s| &s.rows)
            .any(|(label, value)| label == "Triggered HIT / trigger" && value == "2.000"));
        assert!(page.sections[0]
            .iter()
            .flat_map(|s| &s.rows)
            .all(|(_, value)| !value.contains("Some(")));
        let boss_page = meter.debug_page("fireball", 5.0, true);
        assert!(boss_page.sections[1]
            .iter()
            .flat_map(|s| &s.rows)
            .any(|(label, value)| label == "DoT applications per target"
                && value.contains("Unavailable")));
        let report = meter.debug_report("fireball", 5.0);
        assert!(report.contains("Skill HIT / cast: 1.000"));
        assert!(report.contains("Triggered HIT / trigger: 2.000"));
        assert!(report.contains("Extra triggered spell casts: 1"));
        assert!(
            report.contains("Physical")
                && report.contains("Fire")
                && report.contains("Ignite / burn")
        );
        let json = serde_json::to_string(&meter).unwrap();
        assert!(!json.contains("seen"));
        let decoded: Meter = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.debug_report("fireball", 5.0), report);
        let mut legacy = serde_json::to_value(&meter).unwrap();
        legacy["entries"]["fireball"]
            .as_object_mut()
            .unwrap()
            .remove("debug");
        let decoded: Meter = serde_json::from_value(legacy).unwrap();
        assert_eq!(decoded.rows(5.0)[0].total, 76.0);
        assert!(decoded.debug_report("fireball", 5.0).contains("N/A"));
    }
}
