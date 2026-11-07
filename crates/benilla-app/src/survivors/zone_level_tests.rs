use super::*;

#[test]
fn campaign_monsters_and_bosses_progress_inside_the_displayed_zone_band() {
    let mut previous_high = 0;
    for &map in CAMPAIGN {
        let (low, high) = campaign_levels(map).unwrap();
        assert!(low > previous_high);
        let roster = roster(map);
        for monsters in [&roster.enemies, &roster.bosses] {
            assert!(monsters
                .iter()
                .all(|enemy| (low..=high).contains(&enemy.level)));
            assert!(monsters
                .windows(2)
                .all(|pair| pair[0].level <= pair[1].level));
            assert!(monsters.first().unwrap().level < monsters.last().unwrap().level);
            assert_eq!(monsters.last().unwrap().level, high);
        }
        assert_eq!(roster.enemies[0].level, low);
        assert!(roster
            .bosses
            .windows(2)
            .all(|pair| pair[0].level < pair[1].level));
        let start = wave_pool(0.0, roster.enemies.len());
        let end = wave_pool(VICTORY_SECS, roster.enemies.len());
        assert!(roster.enemies[start[2]].level < roster.enemies[end[0]].level);
        previous_high = high;
    }
}
