use super::xp_to_next;

#[test]
fn mid_run_xp_costs_are_preserved_after_the_early_transition() {
    for level in 40..60 {
        let steps = f64::from(level - 1);
        let steps = steps.min(9.0) + (steps - 9.0).max(0.0) * (40.0 / 90.0);
        let previous = (5.0 + 10.0 * (1.06_f64.powf(steps) - 1.0) / 0.06).round() as f32;
        assert_eq!(xp_to_next(level), previous, "level {level}");
    }
}

#[test]
fn early_levels_cost_less_and_transition_smoothly_back_to_baseline() {
    let baseline = |level: u32| {
        let steps = f64::from(level - 1);
        let steps = steps.min(9.0) + (steps - 9.0).max(0.0) * (40.0 / 90.0);
        5.0 + 10.0 * (1.06_f64.powf(steps) - 1.0) / 0.06
    };
    for level in 1..=30 {
        assert!(f64::from(xp_to_next(level)) <= baseline(level) * 0.8 + 0.5);
    }
    for (level, factor) in [(20, 0.65), (30, 0.8), (35, 0.9), (40, 1.0)] {
        assert_eq!(xp_to_next(level), (baseline(level) * factor).round() as f32);
    }
    assert!(xp_to_next(31) / xp_to_next(30) < 1.1);
    assert!(xp_to_next(40) / xp_to_next(39) < 1.1);
}

#[test]
fn late_run_has_cost_jumps_and_increasing_growth_at_60_and_100() {
    let growth = |level| xp_to_next(level + 1) / xp_to_next(level);
    assert!(xp_to_next(60) > xp_to_next(59) * 1.5);
    assert!(xp_to_next(100) > xp_to_next(99) * 2.0);
    assert!(growth(70) > growth(50));
    assert!(growth(110) > growth(70));
}

#[test]
fn xp_costs_stay_positive_monotonic_and_finite() {
    let mut previous = xp_to_next(0);
    assert!(previous > 0.0);
    for level in 1..=1000 {
        let cost = xp_to_next(level);
        assert!(cost.is_finite() && cost >= previous, "level {level}");
        previous = cost;
    }
    assert_eq!(xp_to_next(u32::MAX), f32::MAX);
}
