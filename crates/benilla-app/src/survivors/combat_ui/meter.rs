//! Effective health removed, attributed to the originating ability (including periodic effects).
use std::collections::{BTreeMap, VecDeque};
mod debug;
pub(in crate::survivors) use debug::DamageEvent;
pub(super) use debug::{DiagnosticPage, DiagnosticSection};
const BUCKETS: u64 = 50;
#[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Meter {
    entries: BTreeMap<String, Entry>,
}
#[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Entry {
    #[serde(default)]
    debug: debug::Diagnostics,
    total: f64,
    recent: VecDeque<(u64, f64)>,
    normal: Samples,
    critical: Samples,
    periodic: Samples,
    casts: u64,
}
#[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct Samples {
    pub count: u64,
    pub total: f64,
    pub min: f64,
    pub max: f64,
}
impl Samples {
    fn add(&mut self, damage: f64) {
        self.min = if self.count == 0 {
            damage
        } else {
            self.min.min(damage)
        };
        self.max = self.max.max(damage);
        self.count += 1;
        self.total += damage;
    }
    pub fn average(&self) -> f64 {
        self.total / self.count.max(1) as f64
    }
}
pub(crate) struct Row {
    pub key: String,
    pub dps: f64,
    pub total: f64,
}
impl Meter {
    #[cfg(test)]
    pub fn record(&mut self, key: &str, now: f32, damage: f32) {
        self.record_hit(key, now, damage, false, false);
    }
    pub fn cast(&mut self, key: &str) {
        self.entries.entry(key.to_owned()).or_default().casts += 1;
    }
    pub fn record_hit(&mut self, key: &str, now: f32, damage: f32, critical: bool, periodic: bool) {
        if !damage.is_finite() || damage <= 0.0 {
            return;
        }
        let tick = (now.max(0.0) * 10.0) as u64;
        let entry = self.entries.entry(key.to_owned()).or_default();
        let damage = f64::from(damage);
        if periodic {
            entry.periodic.add(damage);
        } else if critical {
            entry.critical.add(damage);
        } else {
            entry.normal.add(damage);
        }
        entry.total += damage;
        while entry
            .recent
            .front()
            .is_some_and(|(t, _)| tick.saturating_sub(*t) >= BUCKETS)
        {
            entry.recent.pop_front();
        }
        if let Some((_, value)) = entry.recent.back_mut().filter(|(t, _)| *t == tick) {
            *value += damage;
        } else {
            entry.recent.push_back((tick, damage));
        }
    }
    pub fn rows(&self, now: f32) -> Vec<Row> {
        let tick = (now.max(0.0) * 10.0) as u64;
        let seconds = f64::from(now.clamp(1.0, 5.0));
        let mut rows: Vec<_> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.total > 0.0)
            .map(|(key, entry)| Row {
                key: key.clone(),
                total: entry.total,
                dps: entry
                    .recent
                    .iter()
                    .filter(|(t, _)| tick.saturating_sub(*t) < BUCKETS)
                    .map(|(_, damage)| damage)
                    .sum::<f64>()
                    / seconds,
            })
            .collect();
        rows.sort_by(|a, b| {
            b.dps
                .total_cmp(&a.dps)
                .then_with(|| b.total.total_cmp(&a.total))
                .then_with(|| a.key.cmp(&b.key))
        });
        rows
    }

    pub(super) fn details(&self, key: &str, elapsed: f32) -> Vec<(String, String)> {
        let Some(e) = self.entries.get(key) else {
            return vec![];
        };
        let hits = e.normal.count + e.critical.count;
        let all = hits + e.periodic.count;
        let total: f64 = self.entries.values().map(|entry| entry.total).sum();
        let mut lines = vec![
            ("Total damage".into(), format!("{:.1}", e.total)),
            (
                "Damage share".into(),
                format!("{:.1}%", e.total / total.max(f64::MIN_POSITIVE) * 100.0),
            ),
            (
                "Average DPS / run".into(),
                format!("{:.1}", e.total / f64::from(elapsed.max(1.0))),
            ),
            ("Casts (including repeats)".into(), e.casts.to_string()),
            ("Direct hits".into(), hits.to_string()),
            ("Normal hits".into(), e.normal.count.to_string()),
            ("All damage events".into(), all.to_string()),
            (
                "Damage / cast".into(),
                if e.casts == 0 {
                    "—".into()
                } else {
                    format!("{:.1}", e.total / e.casts as f64)
                },
            ),
            (
                "Critical hits".into(),
                format!(
                    "{} ({:.1}%)",
                    e.critical.count,
                    e.critical.count as f64 / hits.max(1) as f64 * 100.0
                ),
            ),
            ("Periodic ticks".into(), e.periodic.count.to_string()),
            (
                "Average / damage event".into(),
                format!("{:.1}", e.total / all.max(1) as f64),
            ),
        ];
        for (label, samples) in [
            ("Normal hits", &e.normal),
            ("Critical hits", &e.critical),
            ("Periodic damage", &e.periodic),
        ] {
            lines.push((
                format!("{label}: min / avg / max"),
                if samples.count == 0 {
                    "—".into()
                } else {
                    format!(
                        "{:.1} / {:.1} / {:.1}",
                        samples.min,
                        samples.average(),
                        samples.max
                    )
                },
            ));
            lines.push((format!("{label}: total"), format!("{:.1}", samples.total)));
        }
        lines
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn details_separate_crits_ticks_and_casts_and_survive_network_roundtrip() {
        let mut meter = Meter::default();
        meter.cast("fireball");
        meter.record_hit("fireball", 1.0, 10.0, false, false);
        meter.record_hit("fireball", 1.0, 30.0, false, false);
        meter.record_hit("fireball", 1.0, 60.0, true, false);
        meter.record_hit("fireball", 1.0, 8.0, false, true);
        meter.record_hit("fireball", 1.0, f32::NAN, true, false);
        let entry = &meter.entries["fireball"];
        assert_eq!(entry.casts, 1);
        assert_eq!(entry.normal.count, 2);
        assert_eq!(
            (entry.normal.min, entry.normal.average(), entry.normal.max),
            (10.0, 20.0, 30.0)
        );
        assert_eq!(entry.critical.count, 1);
        assert_eq!(entry.periodic.count, 1);
        assert_eq!(entry.total, 108.0);
        let encoded = serde_json::to_string(&meter).unwrap();
        let decoded: Meter = serde_json::from_str(&encoded).unwrap();
        assert_eq!(
            decoded.details("fireball", 5.0),
            meter.details("fireball", 5.0)
        );
        assert!(meter.details("missing", 0.0).is_empty());
    }
    #[test]
    fn rolling_damage_decays_and_total_is_kept() {
        let mut meter = Meter::default();
        meter.record("fireball", 1.0, 100.0);
        meter.record("fireball", 1.05, 50.0);
        meter.record("corruption", 4.0, 50.0);
        let rows = meter.rows(5.0);
        assert_eq!(rows[0].key, "fireball");
        assert_eq!(rows[0].dps, 30.0);
        assert_eq!(rows[0].total, 150.0);
        let rows = meter.rows(6.1);
        assert_eq!(rows[0].key, "corruption");
        assert_eq!(rows.iter().find(|r| r.key == "fireball").unwrap().dps, 0.0);
        assert!(meter.rows(10.0).iter().all(|r| r.dps == 0.0));
    }
    #[test]
    fn samples_are_bounded_by_time_and_invalid_damage_is_ignored() {
        let mut meter = Meter::default();
        for i in 0..10_000 {
            meter.record("fireball", i as f32 / 10.0, 1.0);
        }
        meter.record("bad", 0.0, f32::NAN);
        meter.record("bad", 0.0, -1.0);
        assert_eq!(meter.entries.len(), 1);
        assert!(meter.entries["fireball"].recent.len() <= 50);
        assert_eq!(meter.entries["fireball"].total, 10_000.0);
    }
}
