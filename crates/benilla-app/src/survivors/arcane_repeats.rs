//! Arcane repeat bonuses use planned copies, never recursively generated chains.
use super::{
    items::legendary,
    spell_engine::{Resolved, Tags},
    talent_design::Mechanic,
    talent_runtime, Rng, Run,
};

pub(super) fn extra_spell(run: &Run, source: usize, tags: Tags, rng: &mut Rng) -> Option<usize> {
    if !tags.contains(Tags::SPELL) || !talent_runtime::active(run, Mechanic::ArcaneCompanion) {
        return None;
    }
    let candidates: Vec<_> = run
        .abilities
        .iter()
        .enumerate()
        .filter(|(index, owned)| {
            *index != source && legendary::profile(run, owned.def).1.contains(Tags::SPELL)
        })
        .map(|(index, _)| index)
        .collect();
    if candidates.is_empty() {
        return run
            .abilities
            .get(source)
            .filter(|owned| legendary::profile(run, owned.def).1.contains(Tags::SPELL))
            .map(|_| source);
    }
    Some(candidates[rng.index(candidates.len())])
}

pub(super) fn empower(run: &Run, p: &mut Resolved, ordinal: u64) {
    if !p.tags.contains(Tags::SPELL | Tags::DAMAGE)
        || !talent_runtime::active(run, Mechanic::ArcaneCascade)
    {
        return;
    }
    let multiplier = 1.2_f64.powf(ordinal as f64).min(1.0e12);
    let scale = |value: f32| (f64::from(value) * multiplier).clamp(0.0, 1.0e12) as f32;
    p.dmg = scale(p.dmg);
    p.dot = scale(p.dot);
    if let Some(dot) = p.direct_dot.as_mut() {
        dot.dps = scale(dot.dps);
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::super::{data, spell_engine::PeriodicSnapshot, talents, Owned};
    use super::*;
    fn run() -> Run {
        let mut run = Run {
            class: &data::CLASSES[6],
            ..Default::default()
        };
        let mut build = [[0; talents::NODES]; 3];
        for node in [8, 12, 18] {
            build[0][node] = 1;
        }
        talents::Account::from_shared(6, 0, build).apply(6, &mut run);
        run.abilities = [
            "arcane_missiles",
            "fireball",
            "heroic_strike",
            "flash_heal",
            "ice_barrier",
        ]
        .into_iter()
        .map(|key| Owned {
            def: data::ability(key),
            level: 1,
            cd: 99.0,
        })
        .collect();
        run
    }
    #[test]
    fn companion_chooses_learned_spells_and_falls_back_when_only_one_is_known() {
        let mut run = run();
        let mut rng = Rng(912);
        for _ in 0..50 {
            let chosen = extra_spell(&run, 0, Tags::SPELL, &mut rng).unwrap();
            assert!([1, 3, 4].contains(&chosen));
        }
        assert_eq!(extra_spell(&run, 0, Tags::ATTACK, &mut rng), None);
        run.abilities.truncate(1);
        assert_eq!(extra_spell(&run, 0, Tags::SPELL, &mut rng), Some(0));
        run.abilities.clear();
        assert_eq!(extra_spell(&run, 0, Tags::SPELL, &mut rng), None);
    }
    #[test]
    fn cascade_scales_direct_and_periodic_damage_without_overflow() {
        let run = run();
        let mut p = run.resolve_spell(data::ability("arcane_missiles"), 1);
        p.dmg = 10.0;
        p.dot = 3.0;
        p.direct_dot = Some(PeriodicSnapshot {
            dps: 2.0,
            duration: 4.0,
            tags: Tags::FIRE | Tags::DOT,
        });
        empower(&run, &mut p, 4);
        assert!((p.dmg - 20.736).abs() < 0.001);
        assert!((p.dot - 6.2208).abs() < 0.001);
        assert!((p.direct_dot.unwrap().dps - 4.1472).abs() < 0.001);
        empower(&run, &mut p, u64::MAX);
        assert!(p.dmg.is_finite() && p.dot.is_finite());
        assert!(p.dmg <= 1.0e12);
    }
}
