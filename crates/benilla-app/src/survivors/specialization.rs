//! Run build commitments are derived from owned ranks, including on the authoritative host.
use super::{
    data::{self, AbilityDef, Kind},
    Run,
};
pub(super) const LIMIT: usize = 3;
pub(super) const SIGNATURE_RADIUS: f32 = 1.5;
pub(super) const SIGNATURE_DURATION: f32 = 1.5;
pub(super) const SIGNATURE_POWER: f32 = 1.25;
pub(super) fn rank_cap(level: u8) -> u8 {
    if level >= 6 {
        6
    } else if level >= 4 {
        5
    } else {
        3
    }
}
pub(super) fn count(run: &Run) -> usize {
    run.abilities.iter().filter(|o| o.level >= 4).count()
}
pub(super) fn signature(run: &Run) -> bool {
    run.abilities.iter().any(|o| o.level >= 6)
}
pub(super) fn can_upgrade(run: &Run, index: usize) -> bool {
    run.abilities.get(index).is_some_and(|o| match o.level {
        0..=2 => true,
        3 => count(run) < LIMIT,
        4 => true,
        5 => !signature(run),
        _ => false,
    })
}
/// Power gains: ordinary x1, ranks 4 and 5 x2, signature rank 6 x4.
pub(super) fn steps(level: u8) -> f32 {
    let level = level.clamp(1, data::MAX_ABILITY_LEVEL);
    f32::from(level - 1)
        + if level >= 4 { 1.0 } else { 0.0 }
        + if level >= 5 { 1.0 } else { 0.0 }
        + if level >= 6 { 3.0 } else { 0.0 }
}
/// Specialized cooldown gains are one third of the former x3/x6 progression.
pub(super) fn cooldown_steps(level: u8) -> f32 {
    let level = level.clamp(1, data::MAX_ABILITY_LEVEL);
    f32::from(level - 1) + if level >= 6 { 1.0 } else { 0.0 }
}
pub(super) fn extra_target(def: &AbilityDef, level: u8) -> usize {
    usize::from(
        level >= 6
            && matches!(
                def.kind,
                Kind::Bolt | Kind::Strike | Kind::Melee | Kind::Dot | Kind::Orbit | Kind::Chain
            ),
    ) * 2
}
pub(super) fn signature_text(def: &AbilityDef) -> &'static str {
    if def.key == "totemic_recall" {
        return "Signature: 25% less cooldown";
    }
    match def.kind {
        Kind::Bolt => "Signature: +2 projectiles",
        Kind::Chain => "Signature: +2 chain targets",
        Kind::Strike | Kind::Melee | Kind::Dot | Kind::Orbit => "Signature: +2 targets",
        Kind::Buff => "Signature: 25% more buff power",
        Kind::Immune => "Signature: +1.5 sec protection duration",
        Kind::Thorns if def.damage == 0.0 => "Signature: +1.5 sec protection duration",
        Kind::Nova | Kind::Cone | Kind::Ground | Kind::GroundSelf | Kind::Aura => {
            "Signature: +1.5 yd radius"
        }
        Kind::Totem if def.radius > 0.0 => "Signature: +1.5 yd radius",
        Kind::Heal if def.radius > 0.0 => "Signature: +1.5 yd radius",
        _ => "Signature: 25% more power",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::survivors::Owned;
    fn run(levels: &[u8]) -> Run {
        Run {
            abilities: levels
                .iter()
                .zip([
                    "fireball",
                    "frostbolt",
                    "arcane_missiles",
                    "scorch",
                    "pyroblast",
                    "blizzard",
                ])
                .map(|(&level, key)| Owned {
                    def: data::ability(key),
                    level,
                    cd: 0.0,
                })
                .collect(),
            ..Default::default()
        }
    }
    #[test]
    fn three_specializations_and_one_signature_are_the_only_committable_slots() {
        let mut run = run(&[3, 3, 3, 3, 3, 3]);
        for i in 0..3 {
            assert!(can_upgrade(&run, i));
            run.abilities[i].level = 4;
        }
        assert_eq!(count(&run), 3);
        assert!(!can_upgrade(&run, 3));
        for i in 0..3 {
            assert!(can_upgrade(&run, i));
            run.abilities[i].level = 5;
        }
        assert!(can_upgrade(&run, 0));
        run.abilities[0].level = 6;
        for i in 0..6 {
            assert!(!can_upgrade(&run, i));
        }
        assert!(!can_upgrade(&run, 99));
    }
    #[test]
    fn ordinary_spells_can_still_reach_rank_three_after_committing_all_slots() {
        let run = run(&[6, 5, 5, 2, 3, 1]);
        assert!(can_upgrade(&run, 3));
        assert!(can_upgrade(&run, 5));
        assert!(!can_upgrade(&run, 4));
    }
    #[test]
    fn specialization_reduces_power_to_two_thirds_and_cooldown_gains_to_one_third() {
        assert_eq!(steps(2) - steps(1), 1.0);
        assert_eq!(steps(3) - steps(2), 1.0);
        assert_eq!(steps(4) - steps(3), 2.0);
        assert_eq!(steps(5) - steps(4), 2.0);
        assert_eq!(steps(6) - steps(5), 4.0);
        for (rank, gain) in [(2, 1.0), (3, 1.0), (4, 1.0), (5, 1.0), (6, 2.0)] {
            assert_eq!(cooldown_steps(rank) - cooldown_steps(rank - 1), gain);
        }
    }
}
