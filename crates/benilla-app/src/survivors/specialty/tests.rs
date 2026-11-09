use super::*;

fn empty() -> Specialty {
    Specialty {
        save: Save::default(),
        revision: 1,
        dirty: false,
        applied: Applied::default(),
    }
}

#[test]
fn experience_converts_once_per_character_with_the_achievement_bonus() {
    let mut sp = empty();
    sp.credit(7, 10_000, 0.0);
    assert_eq!(sp.earned(), 1_000);
    sp.credit(7, 10_000, 0.0);
    assert_eq!(sp.earned(), 1_000);
    sp.credit(7, 12_000, 0.5);
    assert_eq!(sp.earned(), 1_300);
    // Another character's past experience counts the first time it is seen.
    sp.credit(8, 1_000, 0.0);
    assert_eq!(sp.earned(), 1_400);
}

#[test]
fn ranks_cost_more_each_time_and_promotions_more_than_the_last() {
    for promotion in PROMOTIONS {
        for node in promotion.nodes {
            let prices: Vec<u64> = (0..node.ranks).map(|r| cost(promotion, node, r)).collect();
            assert!(prices.windows(2).all(|p| p[1] > p[0]), "{}", node.id);
        }
    }
    for pair in PROMOTIONS.windows(2) {
        assert!(pair[1].scale > pair[0].scale);
    }
    let mut ids = std::collections::HashSet::new();
    assert!(PROMOTIONS.iter().flat_map(|p| p.nodes).all(|n| ids.insert(n.id)));
}

#[test]
fn buying_needs_points_and_an_open_promotion_and_refunds_are_free() {
    let mut achievements = Achievements::empty();
    let mut sp = empty();
    assert!(sp.buy("b_damage", &mut achievements).is_err());
    sp.save.earned = 1_000.0;
    sp.buy("b_damage", &mut achievements).unwrap();
    assert_eq!(sp.rank("b_damage"), 1);
    assert_eq!(sp.spent(), 100);
    assert_eq!(achievements.counter("sp_spent"), 100);
    assert!(sp.buy("r_critdmg", &mut achievements).unwrap_err().contains("locked"));
    for map in data::CAMPAIGN {
        achievements.add(&format!("map_wins:{map}"), 1);
    }
    sp.buy("r_critdmg", &mut achievements).unwrap();
    assert!(sp.buy("a_boss", &mut achievements).unwrap_err().contains("locked"));
    sp.reset(&PROMOTIONS[0]);
    assert_eq!(sp.rank("b_damage"), 0);
    assert_eq!(sp.spent(), 400);
}

#[test]
fn ranks_apply_to_the_run_and_survive_the_wire() {
    let mut ranks = Ranks::new();
    ranks.insert("b_health".into(), 5);
    ranks.insert("r_reroll".into(), 2);
    ranks.insert("m_multicrit".into(), 10);
    ranks.insert("unknown".into(), 3);
    let decoded = decode(&encode(&ranks));
    assert_eq!(decoded.len(), 3);
    assert_eq!(decoded.get("r_reroll"), Some(&2));
    let mut run = Run::default();
    let before = run.stats().max_hp;
    apply(&decoded, &mut run, 4);
    assert!((run.stats().max_hp / before - 1.25).abs() < 0.001);
    assert_eq!(run.draft_uses, [7, 5, 5]);
    assert_eq!(run.specialty.version, Some(4));
    assert!((run.specialty.multi_crit - 0.005).abs() < 1e-6);
}

#[test]
fn multi_crits_roll_a_third_of_the_crit_chance_each_time() {
    let mut rng = Rng(0x1234_5678);
    assert_eq!(multi_crit(&Applied::default(), 1.0, &mut rng), 1.0);
    let open = Applied {
        multi_unlocked: true,
        multi_chain: 2,
        ..default()
    };
    assert_eq!(multi_crit(&open, 0.0, &mut rng), 1.0);
    // Certain crits: a third of the chance, then a third of that, each adding half the hit.
    let rolls = 30_000;
    let mut total = 0.0;
    for _ in 0..rolls {
        let factor = multi_crit(&open, 1.0, &mut rng);
        assert!([1.0, 1.5, 2.0, 2.5].contains(&factor), "{factor}");
        total += factor;
    }
    // Each further crit needs the last: 1/3, then 1/3 x 1/9, then that x 1/27.
    let expected = 1.0 + 0.5 * (1.0 / 3.0 + 1.0 / 27.0 + 1.0 / 729.0);
    assert!((total / rolls as f32 - expected).abs() < 0.02, "{}", total / rolls as f32);
}
