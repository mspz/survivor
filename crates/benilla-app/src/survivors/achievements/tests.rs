use super::*;
use std::collections::HashSet;

#[test]
fn the_catalog_has_unique_ids_and_tiers_that_get_harder() {
    let mut ids = HashSet::new();
    assert!(all().len() > 150, "{} achievements", all().len());
    for achievement in all() {
        assert!(ids.insert(achievement.id.clone()), "{}", achievement.id);
        assert!(!achievement.tiers.is_empty());
        assert!(achievement.text.contains("{}"), "{}", achievement.id);
        for pair in achievement.tiers.windows(2) {
            match achievement.metric {
                Metric::Time(_) => assert!(pair[1].target < pair[0].target, "{}", achievement.id),
                _ => assert!(pair[1].target > pair[0].target, "{}", achievement.id),
            }
        }
        assert!(achievement.tiers.iter().all(|t| t.bonus > 0.0 && t.bonus <= 0.03));
    }
    for category in Category::ALL {
        assert!(all().iter().any(|a| a.category == category), "{category:?}");
    }
}

#[test]
fn a_run_is_folded_once_and_a_new_run_starts_over() {
    let mut progress = Achievements::empty();
    let map = data::CAMPAIGN[0];
    let mut run = Run {
        elapsed: 30.0,
        kills: 700,
        ..default()
    };
    run.record.spell_kill("fireball");
    for _ in 0..3 {
        track_run(&mut progress, &run, map, None);
    }
    assert_eq!(progress.counter("kills"), 700);
    assert_eq!(progress.counter("runs"), 1);
    assert_eq!(progress.counter("spell:fireball"), 1);
    assert_eq!(progress.counter(&format!("map_kills:{map}")), 700);
    run.kills = 900;
    run.elapsed = 40.0;
    track_run(&mut progress, &run, map, None);
    assert_eq!(progress.counter("kills"), 900);
    assert_eq!(progress.counter("kills_run"), 900);
    // The next run starts from nothing again.
    let next = Run {
        elapsed: 5.0,
        kills: 100,
        ..default()
    };
    track_run(&mut progress, &next, map, None);
    assert_eq!(progress.counter("kills"), 1_000);
    assert_eq!(progress.counter("runs"), 2);
    assert_eq!(progress.counter("kills_run"), 900);
    assert_eq!(progress.evaluate() > 0, true);
    let exterminator = all().iter().find(|a| a.id == "kills").unwrap();
    assert_eq!(progress.unlocked(exterminator), 1);
    assert!(progress.bonus() > 0.0);
}

#[test]
fn victories_count_deaths_bosses_and_challenges_per_expedition() {
    let mut progress = Achievements::empty();
    let map = data::CAMPAIGN[1];
    let mut run = Run {
        elapsed: 900.0,
        kills: 10,
        hp: 100.0,
        ..default()
    };
    run.record.boss_kills = 2;
    run.record.fastest_boss = Some(12.5);
    track_run(&mut progress, &run, map, None);
    assert_eq!(progress.counter("bosses"), 2);
    assert_eq!(progress.time(&format!("boss_time:{map}")), Some(12.5));
    run.won = true;
    track_run(&mut progress, &run, map, None);
    track_run(&mut progress, &run, map, None);
    assert_eq!(progress.counter("wins"), 1);
    assert_eq!(progress.counter(&format!("map_wins:{map}")), 1);
    // Nothing was picked or learned: every card challenge is met, falling was not.
    for key in ["no_spells", "no_cards", "common_only", "no_draft", "no_death", "solo"] {
        assert_eq!(progress.counter(&format!("challenge:{key}:{map}")), 1, "{key}");
    }
    assert_eq!(progress.counter(&format!("challenge:naked:{map}")), 0);
    progress.evaluate();
    let purist = all().iter().find(|a| a.id == "challenge:no_spells").unwrap();
    assert_eq!(progress.unlocked(purist), 1);
    let rush = all().iter().find(|a| a.id == format!("boss_time:{map}")).unwrap();
    assert_eq!(progress.unlocked(rush), 4);

    let mut fallen = Achievements::empty();
    let mut run = Run {
        elapsed: 900.0,
        hp: 0.0,
        dead_t: Some(0.0),
        ..default()
    };
    track_run(&mut fallen, &run, map, None);
    run.won = true;
    track_run(&mut fallen, &run, map, None);
    assert_eq!(fallen.counter("deaths"), 1);
    assert_eq!(fallen.counter(&format!("challenge:no_death:{map}")), 0);
}

#[test]
fn the_record_keeps_the_best_card_and_what_was_learned() {
    use super::super::cards::Rarity;
    use super::super::hud::Choice;
    let mut record = RunRecord::default();
    record.pick(Choice::NewAbility(data::ability("fireball")), Rarity::Epic);
    record.pick(Choice::Heal, Rarity::Legendary);
    record.pick(Choice::Upgrade(0), Rarity::Legendary);
    assert_eq!(record.picks, 2);
    assert_eq!(record.best_pick, 4);
    assert_eq!(record.legendary_picks, 1);
    assert_eq!(record.spells_learned, 1);
    assert_eq!(record.passives_taken, 0);
}
