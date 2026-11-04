//! Effective health removed, attributed to the originating ability (including periodic effects).
use std::collections::{BTreeMap, VecDeque};
const BUCKETS: u64 = 50;
#[derive(Default)]
pub(crate) struct Meter {
    entries: BTreeMap<&'static str, Entry>,
}
#[derive(Default)]
struct Entry {
    total: f64,
    recent: VecDeque<(u64, f64)>,
}
pub(crate) struct Row {
    pub key: &'static str,
    pub dps: f64,
    pub total: f64,
}
impl Meter {
    pub fn record(&mut self, key: &'static str, now: f32, damage: f32) {
        if !damage.is_finite() || damage <= 0.0 {
            return;
        }
        let tick = (now.max(0.0) * 10.0) as u64;
        let entry = self.entries.entry(key).or_default();
        let damage = f64::from(damage);
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
            .map(|(&key, entry)| Row {
                key,
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
                .then_with(|| a.key.cmp(b.key))
        });
        rows
    }
}
#[cfg(test)]
mod tests {
    use super::*;
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
