#[test]
fn chase_pact_only_pays_for_successful_casts_and_prioritizes_high_level_skills() {
    let (mut world, _) = legendary_world("fireball", chase::PACT, &[]);
    let hp = world.resource::<Run>().hp;
    world.run_system_once(cast_abilities).unwrap();
    assert_eq!(world.resource::<Run>().hp, hp);
    let (mut world, _) = legendary_world("fireball", chase::PACT, &[3.0]);
    {
        let mut run = world.resource_mut::<Run>();
        run.abilities.push(Owned {
            def: data::ability("frostbolt"),
            level: 6,
            cd: 0.0,
        });
        run.hp = run.stats().max_hp * 0.075;
    }
    world.run_system_once(cast_abilities).unwrap();
    let messages = world.resource::<Messages<Hit>>();
    let mut cursor = messages.get_cursor();
    let keys: Vec<_> = cursor.read(messages).map(|h| h.skill).collect();
    assert!(!keys.is_empty());
    assert!(keys.iter().all(|key| *key == "frostbolt"));
    assert!(world.resource::<Run>().hp > 0.0);
    let (mut world, _) = legendary_world("commanding_shout", chase::PACT, &[3.0]);
    {
        let mut run = world.resource_mut::<Run>();
        run.hp = run.stats().max_hp * 0.051;
    }
    world.run_system_once(cast_abilities).unwrap();
    assert!(
        world.resource::<Run>().hp > 0.0,
        "a maximum-life buff must pay the cost snapshotted before its activation"
    );
}
#[test]
fn chase_gospel_disables_other_offensives_and_respects_the_tag_priority() {
    let (mut world, _) = legendary_world("fireball", chase::GOSPEL, &[3.0]);
    world.resource_mut::<Run>().abilities.push(Owned {
        def: data::ability("frostbolt"),
        level: 1,
        cd: 0.0,
    });
    world.run_system_once(cast_abilities).unwrap();
    let messages = world.resource::<Messages<Hit>>();
    let mut cursor = messages.get_cursor();
    assert!(cursor.read(messages).all(|h| h.skill == "fireball"));
    let run = world.resource::<Run>();
    let base = Run::default().resolve_spell(data::ability("blizzard"), 1);
    let p = run.resolve_spell(data::ability("blizzard"), 1);
    assert!((p.radius / base.radius - 1.5_f32.sqrt()).abs() < 0.001);
    assert_eq!(p.forks, base.forks);
}
#[test]
fn chase_hourglass_saves_uses_against_elites_then_bursts_only_at_bosses() {
    let (mut world, targets) = legendary_world("fireball", chase::HOURGLASS, &[3.0]);
    world.entity_mut(targets[0]).insert(enemies::Rank::Colossus);
    world
        .resource_mut::<Run>()
        .legendary
        .chase
        .stored
        .insert(0, (0.0, 0.0, 3));
    world.run_system_once(cast_abilities).unwrap();
    let run = world.resource::<Run>();
    let expected = run.resolve_spell(data::ability("fireball"), 1).dmg;
    assert_eq!(run.legendary.chase.stored[&0].2, 1); // clamped to the skill's own 2s capacity
    world.resource_mut::<Messages<Hit>>().clear();
    world.get_mut::<Enemy>(targets[0]).unwrap().boss = true;
    world.resource_mut::<Run>().elapsed = 0.2;
    world.run_system_once(cast_abilities).unwrap();
    let messages = world.resource::<Messages<Hit>>();
    let mut cursor = messages.get_cursor();
    let hits: Vec<_> = cursor.read(messages).collect();
    assert_eq!(hits.len(), 1);
    assert!((hits[0].damage / expected - 0.2).abs() < 0.001);
    assert_eq!(world.resource::<Run>().legendary.chase.stored[&0].2, 0);
}
#[test]
fn chase_crown_propagates_from_living_targets_and_copies_propagate_again() {
    let (mut world, targets) = legendary_world("shadow_word_pain", chase::CROWN, &[3.0, 9.0, 15.0]);
    world
        .get_mut::<Enemy>(targets[0])
        .unwrap()
        .dots
        .push(enemies::Dot {
            item_source: None,
            meta: default(),
            origin: Origin::Skill,
            owner: None,
            skill: "shadow_word_pain",
            meter_source: None,
            tags: Some(engine::Tags::SHADOW | engine::Tags::DOT),
            dps: 100.0,
            left: 8.0,
            tick: 0.0,
        });
    world.run_system_once(chase::spread).unwrap();
    assert_eq!(world.get::<Enemy>(targets[1]).unwrap().dots[0].dps, 25.0);
    // Move the original away; only its copy can reach the third enemy.
    world
        .get_mut::<Transform>(targets[0])
        .unwrap()
        .translation
        .x = -100.0;
    world.resource_mut::<Run>().elapsed = 1.0;
    world.run_system_once(chase::spread).unwrap();
    assert_eq!(world.get::<Enemy>(targets[2]).unwrap().dots[0].dps, 6.25);
    assert!(!world.get::<Enemy>(targets[0]).unwrap().dead);
    assert!(world.get::<Enemy>(targets[2]).unwrap().dots.len() == 1);
}
#[test]
fn chase_hands_hits_at_full_life_and_area_heal_explosions_can_overlap() {
    let (mut world, targets) = legendary_world("flash_heal", chase::HANDS, &[3.0]);
    world.run_system_once(cast_abilities).unwrap();
    assert!(!world.resource::<Messages<Hit>>().is_empty());
    let before = world.get::<Enemy>(targets[0]).unwrap().hp;
    world.run_system_once(apply_hits).unwrap();
    assert!(world.get::<Enemy>(targets[0]).unwrap().hp < before);
    let (mut world, targets) = legendary_world("efflorescence", chase::HANDS, &[3.0]);
    let party = support_party(&mut world);
    world.get_mut::<Transform>(party[1]).unwrap().translation.x = 2.0;
    world.run_system_once(cast_abilities).unwrap();
    world.resource_mut::<Run>().elapsed = 1.1;
    world.run_system_once(cast_abilities).unwrap();
    let messages = world.resource::<Messages<Hit>>();
    let mut cursor = messages.get_cursor();
    assert!(
        cursor
            .read(messages)
            .filter(|h| h.target == targets[0] && h.origin == Origin::Legendary)
            .count()
            >= 2
    );
}
#[test]
fn chase_testament_casts_again_before_native_cooldown_but_waits_without_power() {
    let (mut world, _) = legendary_world("fireball", chase::TESTAMENT, &[3.0]);
    world.run_system_once(cast_abilities).unwrap();
    assert_eq!(
        world.resource::<Run>().abilities[0].cd,
        chase::config().minimum_cast_interval
    );
    world.resource_mut::<Messages<Hit>>().clear();
    world.run_system_once(cast_abilities).unwrap();
    assert!(!world.resource::<Messages<Hit>>().is_empty());
    world.resource_mut::<Messages<Hit>>().clear();
    world.resource_mut::<Run>().legendary.chase.power = Some(0.0);
    world.run_system_once(cast_abilities).unwrap();
    assert!(world.resource::<Messages<Hit>>().is_empty());
}
