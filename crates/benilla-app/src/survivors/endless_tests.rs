use super::*;
use bevy::ecs::system::RunSystemOnce;

#[test]
fn monster_levels_follow_transitions_even_when_returning_to_the_same_map() {
    let mut run=Run::default();
    begin_at(&mut run,0);
    let base=&roster(data::CAMPAIGN[0]).enemies[0];
    for index in 0..4 {
        assert_eq!(run.endless.unwrap().monster_level(),60+index);
        let actor=leveled_actor(run.endless.unwrap(),base);
        assert_eq!(actor.level,60+index);
        assert_eq!(actor.display,base.display);
        assert_eq!(actor.name,base.name);
        assert!(std::ptr::eq(actor,leveled_actor(run.endless.unwrap(),base)),"definitions are shared across spawns");
        advance(&mut run,data::CAMPAIGN[index as usize%2]);
    }
    assert_eq!(Stage {index:u32::MAX,..default()}.monster_level(),u32::MAX);
    assert_eq!(base.level,60,"original roster is immutable");
}

#[test]
fn gold_and_xp_have_separate_additive_progression_and_start_without_a_bonus() {
    let mut run = Run::default();
    assert_eq!(run.expeditions.gold_multiplier(), 1.0);
    for index in 0..4 {
        begin_at(&mut run, index);
        assert!((run.expeditions.gold_multiplier() - (1.0 + index as f32 * 0.6)).abs() < 1e-5);
        assert!((run.expeditions.reward_multiplier() - (1.0 + index as f32 * 0.3)).abs() < 1e-5);
    }
    let mut old: toml::Value = toml::from_str(FILE).unwrap();
    old.as_table_mut().unwrap().remove("gold_per_map");
    assert!((Config::parse(&toml::to_string(&old).unwrap()).unwrap().gold(1) - 1.6).abs() < 1e-5);
}

#[test]
fn fourth_map_diagnostic_start_applies_all_stage_modifiers_without_fake_past_rewards() {
    let mut run = Run::default();
    begin_at(&mut run, 3);
    let stage = run.endless.unwrap();
    assert_eq!(stage.index, 3);
    assert_eq!(stage.started, 0.0);
    assert_eq!(stage.boss_offset, 0);
    assert_eq!(run.expeditions.duration(), config().duration(3));
    assert_eq!(run.expeditions.boss_waves_due(0.0), 0);
    assert_eq!(
        run.expeditions.boss_waves_due(run.expeditions.duration()),
        data::BOSS_WAVES
    );
    assert_eq!(run.expeditions.reward_multiplier(), config().reward(3));
    assert_eq!(stage.xp_earned, 0);
}

#[test]
fn configuration_scales_each_stage_and_caps_duration_and_cooldowns() {
    let mut document: toml::Value = toml::from_str(FILE).unwrap();
    for (key, value) in [
        ("initial_seconds", 360.0),
        ("shorter_per_map", 60.0),
        ("minimum_seconds", 120.0),
        ("movement_per_map", 1.2),
        ("monster_stats_per_map", 1.2),
        ("boss_stats_per_map", 1.4),
        ("boss_cooldown_per_map", 0.8),
        ("minimum_boss_cooldown", 1.0),
        ("safe_time_per_map", 0.8),
        ("reward_per_map", 0.3),
    ] {
        document[key] = toml::Value::Float(value);
    }
    let config = Config::parse(&toml::to_string(&document).unwrap()).unwrap();
    for (index, duration) in [
        (0, 360.0),
        (1, 300.0),
        (2, 240.0),
        (3, 180.0),
        (4, 120.0),
        (100, 120.0),
    ] {
        assert_eq!(config.duration(index), duration);
    }
    assert!((config.movement(2) - 1.44).abs() < 1e-5);
    assert_eq!(config.movement(39), 5.0);
    assert_eq!(config.movement(u32::MAX), 5.0);
    assert!((config.stats(2, false) - 1.44).abs() < 1e-5);
    assert!((config.stats(2, true) - 1.96).abs() < 1e-5);
    assert_eq!(config.reward(0), 1.0);
    assert!((config.reward(1) - 1.3).abs() < 1e-5);
    assert!((config.reward(2) - 1.6).abs() < 1e-5);
    assert!((config.cooldown(10.0, 2) - 6.4).abs() < 1e-5);
    assert_eq!(config.cooldown(10.0, 100), 1.0);
    assert!((config.safe_time(2.0, 2) - 1.28).abs() < 1e-5);
    assert!(config.movement(u32::MAX).is_finite());
    let mut invalid = document.clone();
    invalid["minimum_seconds"] = toml::Value::Float(0.0);
    assert!(Config::parse(&toml::to_string(&invalid).unwrap()).is_err());
    document["boss_cooldown_per_map"] = toml::Value::Float(1.2);
    assert!(Config::parse(&toml::to_string(&document).unwrap()).is_err());
}

#[test]
fn movement_cap_is_configurable_and_compatible_with_older_configs() {
    let mut document: toml::Value = toml::from_str(FILE).unwrap();
    document
        .as_table_mut()
        .unwrap()
        .remove("maximum_movement_multiplier");
    let old = Config::parse(&toml::to_string(&document).unwrap()).unwrap();
    assert_eq!(old.movement(39), 5.0);
    document.as_table_mut().unwrap().insert(
        "maximum_movement_multiplier".into(),
        toml::Value::Float(3.0),
    );
    let custom = Config::parse(&toml::to_string(&document).unwrap()).unwrap();
    assert_eq!(custom.movement(39), 3.0);
    assert_eq!(custom.cap_movement(20.0), 3.0);
    assert_eq!(custom.cap_movement(2.0), 2.0);
    assert_eq!(custom.movement(0), 1.0);
    for cap in [0.0, -1.0, 0.5, f64::INFINITY, f64::NAN] {
        document["maximum_movement_multiplier"] = toml::Value::Float(cap);
        assert!(Config::parse(&toml::to_string(&document).unwrap()).is_err());
    }
}

#[test]
fn offers_two_distinct_destinations_excluding_the_previous_map() {
    let mut rng = Rng(42);
    let mut seen = std::collections::BTreeSet::new();
    for &current in data::CAMPAIGN {
        for _ in 0..100 {
            let [a, b] = destinations(current, &mut rng);
            assert_ne!(a, current);
            assert_ne!(b, current);
            assert_ne!(a, b);
            assert!(data::CAMPAIGN.contains(&a) && data::CAMPAIGN.contains(&b));
            seen.extend([a, b]);
        }
    }
    assert_eq!(seen.len(), data::CAMPAIGN.len());
}

#[test]
fn every_map_uses_level_sixty_actors_and_the_same_dedicated_baseline() {
    let mut run = Run::default();
    begin(&mut run);
    let reference = run.expeditions.stats(data::CAMPAIGN[0], false);
    for &map in data::CAMPAIGN {
        let roster = roster(map);
        assert!(
            roster
                .enemies
                .iter()
                .chain(&roster.bosses)
                .all(|e| e.level == 60)
        );
        assert_eq!(run.expeditions.stats(map, false), reference);
        // The ordinary campaign's actor levels remain unchanged.
        assert!(data::roster(map).enemies.iter().all(|e| e.level < 60));
    }
}

#[test]
fn advancing_preserves_the_build_and_past_rewards_and_restarts_only_the_stage_clock() {
    let mut run = Run {
        hp: 123.0,
        elapsed: 360.0,
        bosses: 8,
        meta_kills: 20,
        level: 17,
        stored_cards: 9,
        shield: 10.0,
        shield_t: 3.0,
        ..default()
    };
    run.abilities.push(Owned {
        def: data::ability("fireball"),
        level: 3,
        cd: 2.5,
    });
    begin(&mut run);
    run.won = true;
    let reward = talents::earned(&run);
    advance(&mut run, data::CAMPAIGN[1]);
    assert_eq!(talents::earned(&run), reward);
    assert_eq!((run.hp, run.level, run.stored_cards), (123.0, 17, 9));
    assert_eq!((run.shield, run.shield_t), (10.0, 3.0));
    assert!(
        run.abilities
            .iter()
            .any(|a| a.def.key == "fireball" && a.level == 3 && a.cd == 2.5)
    );
    assert_eq!(run.elapsed, 360.0);
    assert_eq!(run.expeditions.elapsed(run.elapsed), 0.0);
    assert_eq!(run.expeditions.duration(), 300.0);
    assert_eq!(run.expeditions.boss_waves_due(run.elapsed), 8);
    assert_eq!(run.expeditions.boss_waves_due(run.elapsed + 37.5), 9);
    assert_eq!(run.expeditions.boss_waves_due(run.elapsed + 300.0), 16);
    run.meta_kills += 10;
    assert_eq!(talents::earned(&run), reward + 13);
    let paid = talents::earned(&run);
    advance(&mut run, data::CAMPAIGN[2]);
    assert_eq!(
        talents::earned(&run),
        paid,
        "old kills are not repriced with the new bonus"
    );
    run.meta_kills += 10;
    assert_eq!(talents::earned(&run), paid + 16);
}

#[test]
fn choosing_a_destination_preserves_the_hero_and_removes_previous_map_entities() {
    let mut world = World::new();
    world.init_resource::<Selection>();
    world.init_resource::<NextState<Phase>>();
    world.init_resource::<enemies::Spawner>();
    world.init_resource::<enemies::ExpeditionClear>();
    world.init_resource::<abilities::PendingHits>();
    world.init_resource::<boss_spells::Warnings>();
    let next_map = data::CAMPAIGN[1];
    let mut run = Run {
        won: true,
        elapsed: 360.0,
        bosses: 8,
        ..default()
    };
    begin(&mut run);
    run.endless.as_mut().unwrap().choices = Some([next_map, data::CAMPAIGN[2]]);
    world.insert_resource(run);
    let hero = world
        .spawn((RunEntity, hero::Hero { vy: 0.0, air: 0.0 }))
        .id();
    let old_actor = world.spawn(RunEntity).id();
    world.spawn((Interaction::Pressed, NextMap(next_map)));
    world.run_system_once(choose).unwrap();
    assert!(world.get_entity(hero).is_ok());
    assert!(world.get_entity(old_actor).is_err());
    assert_eq!(world.resource::<Selection>().stage, next_map);
    assert_eq!(world.resource::<Run>().endless.unwrap().index, 1);
    assert!(!world.resource::<Run>().won);
    assert!(matches!(
        world.resource::<NextState<Phase>>(),
        NextState::Pending(Phase::Playing)
    ));
    world.run_system_once(choose).unwrap();
    assert_eq!(
        world.resource::<Run>().endless.unwrap().index,
        1,
        "one click cannot advance twice"
    );
}

#[test]
fn a_forged_destination_cannot_skip_the_two_offered_maps() {
    let mut world = World::new();
    world.init_resource::<Selection>();
    world.init_resource::<NextState<Phase>>();
    world.init_resource::<enemies::Spawner>();
    world.init_resource::<enemies::ExpeditionClear>();
    world.init_resource::<abilities::PendingHits>();
    world.init_resource::<boss_spells::Warnings>();
    let mut run = Run {
        won: true,
        ..default()
    };
    begin(&mut run);
    run.endless.as_mut().unwrap().choices = Some([data::CAMPAIGN[1], data::CAMPAIGN[2]]);
    world.insert_resource(run);
    world.spawn((Interaction::Pressed, NextMap(data::CAMPAIGN[0])));
    world.run_system_once(choose).unwrap();
    assert_eq!(world.resource::<Run>().endless.unwrap().index, 0);
    assert!(matches!(
        world.resource::<NextState<Phase>>(),
        NextState::Unchanged
    ));
}

#[test]
fn launch_requires_level_sixty_and_selects_a_random_campaign_map() {
    let mut selection = Selection {
        endless: true,
        class: Some(0),
        look: Some(0),
        looks: vec![(*data::CLASSES[0].looks.first().unwrap(), 1, 0)],
        ..default()
    };
    let mut run = Run::default();
    assert!(!hud::start_run(&mut run, &mut selection));
    run.meta_level = 60;
    assert!(hud::start_run(&mut run, &mut selection));
    assert!(data::CAMPAIGN.contains(&selection.map));
    assert_eq!(selection.map, selection.stage);
    assert!(run.endless.is_some());
    assert_eq!(run.expeditions.duration(), 360.0);
    assert_eq!(run.expeditions.boss_interval(), 45.0);
    assert!(!selection.in_hub);
}

#[test]
fn one_zone_config_applies_to_every_endless_map_and_rejects_per_map_tables() {
    let document: toml::Value = toml::from_str(FILE).unwrap();
    assert!(document["combat"].get("zones").is_none());
    assert!(document["combat"]["zone"].is_table());
    let mut custom = document.clone();
    custom["combat"]["monster_health"] = toml::Value::Float(640.0);
    custom["combat"]["population_start"] = toml::Value::Integer(18);
    custom["combat"]["zone"]["monster_health"] = toml::Value::Float(2.0);
    custom["combat"]["zone"]["monster_quantity"] = toml::Value::Float(2.0);
    custom["combat"]["zone"]["damage_reduction"] = toml::Value::Float(0.6);
    let config = Config::parse(&toml::to_string(&custom).unwrap()).unwrap();
    let engine = config.expedition(Stage::default());
    for &map in data::CAMPAIGN {
        assert_eq!(engine.stats(map, false).0, 1280.0);
        assert_eq!(engine.population(map, 0.0), 36);
        assert_eq!(engine.defenses(map, 60, false, false).1, 0.6);
    }
    assert!(Config::parse(&FILE.replace("[combat.zone]", "[[combat.zones]]")).is_err());
    let mut missing = document;
    missing["combat"].as_table_mut().unwrap().remove("zone");
    assert!(Config::parse(&toml::to_string(&missing).unwrap()).is_err());
}

#[test]
fn rune_reward_and_its_result_must_be_closed_before_map_choices_appear() {
    let mut world = World::new();
    let mut run = Run {
        won: true,
        ..default()
    };
    begin(&mut run);
    world.insert_resource(run);
    world.insert_resource(Rng(42));
    world.insert_resource(hud::HudFont(Handle::default()));
    let mut runes = runes::Runes::default();
    runes.save.pending = Some(runes.config.roll(0, &mut Rng(123)));
    world.insert_resource(runes);
    world.run_system_once(offer).unwrap();
    assert!(world.resource::<Run>().endless.unwrap().choices.is_none());
    assert_eq!(world.query::<&ChoiceRoot>().iter(&world).count(), 0);
    {
        let mut runes = world.resource_mut::<runes::Runes>();
        runes.save.pending = None;
        runes.message = "Rune placed".into();
    }
    world.run_system_once(offer).unwrap();
    assert_eq!(world.query::<&ChoiceRoot>().iter(&world).count(), 0);
    world.resource_mut::<runes::Runes>().message.clear();
    world.run_system_once(offer).unwrap();
    assert!(world.resource::<Run>().endless.unwrap().choices.is_some());
    assert_eq!(world.query::<&ChoiceRoot>().iter(&world).count(), 1);
    let choices = world.resource::<Run>().endless.unwrap().choices;
    world.run_system_once(offer).unwrap();
    assert_eq!(world.query::<&ChoiceRoot>().iter(&world).count(), 1);
    assert_eq!(world.resource::<Run>().endless.unwrap().choices, choices);
}

#[test]
fn an_existing_destination_button_cannot_skip_an_active_rune_reward() {
    let mut world = World::new();
    world.init_resource::<Selection>();
    world.init_resource::<NextState<Phase>>();
    world.init_resource::<enemies::Spawner>();
    world.init_resource::<enemies::ExpeditionClear>();
    world.init_resource::<abilities::PendingHits>();
    world.init_resource::<boss_spells::Warnings>();
    let map = data::CAMPAIGN[1];
    let mut run = Run {
        won: true,
        ..default()
    };
    begin(&mut run);
    run.endless.as_mut().unwrap().choices = Some([map, data::CAMPAIGN[2]]);
    world.insert_resource(run);
    let mut runes = runes::Runes::default();
    runes.message = "Resolve the rune first".into();
    world.insert_resource(runes);
    world.spawn((Interaction::Pressed, NextMap(map)));
    world.run_system_once(choose).unwrap();
    assert_eq!(world.resource::<Run>().endless.unwrap().index, 0);
    assert!(matches!(
        world.resource::<NextState<Phase>>(),
        NextState::Unchanged
    ));
}
