use super::*;
use crate::survivors::{
    Rng, hero, hud, reward_config,
    server::Owned,
    spell_engine::{Change, Filter, Modifier},
};

fn owner(ids: &[u16]) -> Run {
    let mut run = Run::default();
    for &id in ids {
        run.legendary.equipped.insert(
            id,
            Roll {
                unique: id,
                milestone: None,
            },
        );
    }
    run.hp = run.stats().max_hp;
    run
}
#[test]
fn chase_config_rejects_invalid_timers_and_all_drops_have_one_exclusive_source() {
    assert!(parse_config(FILE).is_some());
    assert!(
        parse_config(&FILE.replace("deferred_seconds = 4.0", "deferred_seconds = 0.0")).is_none()
    );
    for id in 401..=415 {
        let source = dungeon(id).unwrap();
        assert!(definition(id).is_some());
        assert_eq!(weight(id, None), 0.0);
        for map in 17..=22 {
            assert_eq!(weight(id, Some(map)) > 0.0, map == source);
        }
    }
    assert!(super::super::base_allowed(
        GOSPEL,
        8,
        super::super::EquipmentType::Staff
    ));
    assert!(!super::super::base_allowed(
        TESTAMENT,
        1,
        super::super::EquipmentType::MeleeOneHand
    ));
}
#[test]
fn pact_adds_scaled_damage_and_will_not_spend_the_last_life() {
    let mut run = owner(&[PACT]);
    let def = data::ability("fireball");
    let base = Run::default().resolve_spell(def, 1).dmg;
    let hp = run.stats().max_hp;
    let resolved = run.resolve_spell(def, 1);
    let balance = run
        .balance
        .multiplier_tagged(run.class, def.key, resolved.tags);
    assert!((resolved.dmg - base - hp * 0.2 * balance).abs() < 0.001);
    assert!(can_pay(&run, def, 2.0));
    cast(&mut run, def, 2.0, hp * 0.05);
    assert!((run.hp - hp * 0.95).abs() < 0.001);
    run.hp = hp * 0.05;
    assert!(!can_pay(&run, def, 2.0));
}
#[test]
fn blocked_damage_fuels_skin_and_resets_the_longest_offensive_cooldown() {
    let mut run = owner(&[SKIN, BULWARK]);
    run.equipment.block = 1.0;
    run.abilities = vec![
        Owned {
            def: data::ability("fireball"),
            level: 1,
            cd: 3.0,
        },
        Owned {
            def: data::ability("frostbolt"),
            level: 1,
            cd: 5.0,
        },
        Owned {
            def: data::ability("flash_heal"),
            level: 1,
            cd: 10.0,
        },
    ];
    let hp = run.hp;
    assert_eq!(hero::hurt_hero_with_block(&mut run, 100.0, 0.84), 0.0);
    assert_eq!(run.hp, hp);
    assert_eq!(run.legendary.chase.reserve, 200.0);
    assert_eq!(run.abilities[0].cd, 3.0);
    assert_eq!(run.abilities[1].cd, 0.0);
    assert_eq!(run.abilities[2].cd, 10.0);
    cast(&mut run, data::ability("fireball"), 2.0, 0.0);
    assert_eq!(run.legendary.chase.reserve, 180.0);
    assert_eq!(
        crate::survivors::monster_specials::player::block(&run, 1.0),
        0.85
    );
}
#[test]
fn oath_debt_does_not_reset_cancel_or_receive_mitigation_twice() {
    let mut run = owner(&[OATH]);
    let hp = run.hp;
    assert!(defer(&mut run, 40.0, Tags::FIRE));
    advance(&mut run, 1.0);
    assert_eq!(run.hp, hp - 10.0);
    assert!(defer(&mut run, 20.0, Tags::COLD));
    run.legendary.equipped.clear();
    run.immune_t = 20.0;
    advance(&mut run, 3.0);
    assert_eq!(run.hp, hp - 55.0);
    advance(&mut run, 1.0);
    assert_eq!(run.hp, hp - 60.0);
    assert!(run.legendary.chase.deferred.is_empty());
    run.legendary.equipped.insert(
        OATH,
        Roll {
            unique: OATH,
            milestone: None,
        },
    );
    run.immune_t = 0.0;
    defer(&mut run, 1000.0, Tags::FIRE);
    advance(&mut run, 4.0);
    assert!(run.hp <= 0.0);
    assert!(run.death_log.last.is_some());
}
#[test]
fn damage_increases_convert_to_life_once_but_more_damage_remains() {
    let mut run = owner(&[BLOOD]);
    let hp = run.stats().max_hp;
    let damage = run.resolve_spell(data::ability("fireball"), 1).dmg;
    run.replace_spell_modifiers(
        1,
        [Modifier {
            source: 1,
            filter: Filter::default(),
            change: Change::IncreasedDamage(1.0),
        }],
    );
    assert!((run.stats().max_hp / hp - 1.1).abs() < 0.001);
    assert_eq!(run.resolve_spell(data::ability("fireball"), 1).dmg, damage);
    run.replace_spell_modifiers(
        2,
        [Modifier {
            source: 2,
            filter: Filter::default(),
            change: Change::MoreDamage(1.0),
        }],
    );
    assert!((run.resolve_spell(data::ability("fireball"), 1).dmg / damage - 2.0).abs() < 0.001);
    assert!((run.stats().max_hp / hp - 1.1).abs() < 0.001);
}
#[test]
fn hunger_stores_overleech_at_full_life_and_drains_faster_at_high_values() {
    let mut run = owner(&[HUNGER]);
    let hp = run.hp;
    let heal = crate::survivors::items::combat::leech(&mut run, hp);
    assert_eq!(heal, 0.0);
    assert!((run.legendary.chase.hunger / hp - 0.2).abs() < 0.001);
    run.legendary.chase.hunger = hp;
    let dmg = run.resolve_spell(data::ability("fireball"), 1).dmg;
    let base = Run::default()
        .resolve_spell(data::ability("fireball"), 1)
        .dmg;
    assert!((dmg / base - 2.0).abs() < 0.001);
    advance(&mut run, 1.0);
    let low_fraction = run.legendary.chase.hunger / hp;
    run.legendary.chase.hunger = hp * 10.0;
    advance(&mut run, 1.0);
    assert!(run.legendary.chase.hunger / (hp * 10.0) < low_fraction);
}
#[test]
fn testament_uses_power_excludes_buffs_and_cooldown_damage_uses_effective_duration() {
    let mut run = owner(&[TESTAMENT, EXECUTIONER]);
    let def = data::ability("fireball");
    assert!(power_skill(def));
    assert!(!power_skill(data::ability("battle_shout")));
    assert!(!power_skill(data::ability("blizzard")));
    assert!(!power_skill(data::ability("shadow_word_pain")));
    run.legendary.equipped.insert(
        70,
        Roll {
            unique: 70,
            milestone: None,
        },
    );
    assert!(
        !power_skill_for(&run, def),
        "a direct skill converted to periodic damage keeps its cooldown"
    );
    run.legendary.equipped.remove(&70);
    let base = Run::default().resolve_spell(def, 1);
    assert_eq!(run.resolve_spell(def, 1).dmg, base.dmg);
    cast(&mut run, def, 2.0, 0.0);
    assert_eq!(run.legendary.chase.power, Some(80.0));
    run.legendary.chase.power = Some(0.0);
    assert!(!can_pay(&run, def, 2.0));
    advance(&mut run, 1.0);
    assert_eq!(run.legendary.chase.power, Some(25.0));
    run.legendary.equipped.remove(&TESTAMENT);
    let mult = 1.0 + base.cooldown + 0.1 * base.cooldown.powi(2);
    assert!((run.resolve_spell(def, 1).dmg / base.dmg - mult).abs() < 0.001);
}
#[test]
fn head_hunter_never_steals_monster_only_pressure_mods() {
    let mut run = owner(&[HEAD_HUNTER]);
    let rank = Rank::MODS.into_iter().skip(50).fold(Rank::Normal, |r, m| r.with(m));
    let mut rng = Rng(42);
    for _ in 0..100 { steal(&mut run, rank, &mut rng, Vec3::ZERO); }
    assert_eq!(run.monster_mods.rank(0.0).count(), 0);
}
#[test]
fn head_hunter_refreshes_without_duplicates_and_has_no_distinct_mod_limit() {
    let mut run = owner(&[HEAD_HUNTER]);
    let rank = Rank::MODS.into_iter().fold(Rank::Normal, |r, m| r.with(m));
    let mut rng = Rng(42);
    for _ in 0..20000 {
        steal(&mut run, rank, &mut rng, Vec3::ZERO);
    }
    assert_eq!(run.monster_mods.rank(0.0).count(), 49);
    assert_eq!(run.monster_mods.rank(2.0).count(), 0);
}
#[test]
fn stolen_mods_use_player_buff_duration_and_refresh_without_stacking() {
    for (increased, expected) in [(0.0, 2.0), (0.5, 3.0), (1.0, 4.0), (-0.5, 1.0)] {
        let mut run = owner(&[HEAD_HUNTER]);
        run.replace_spell_modifiers(
            1,
            [
                Modifier {
                    source: 1,
                    filter: Filter::default(),
                    change: Change::IncreasedDuration(increased),
                },
                Modifier {
                    source: 1,
                    filter: Filter::requiring(Tags::DOT),
                    change: Change::IncreasedDuration(10.0),
                },
            ],
        );
        let mut rng = Rng(0); // Guaranteed proc, with exactly one candidate mod.
        steal(&mut run, Rank::Brutal, &mut rng, Vec3::ZERO);
        assert_eq!(run.monster_mods.rank(0.0).count(), 1);
        assert_eq!(
            crate::survivors::monster_specials::player::buffs(&run)[0].left,
            Some(expected)
        );
        assert_eq!(run.monster_mods.rank(expected).count(), 0);
        run.elapsed = 0.5;
        steal(&mut run, Rank::Brutal, &mut rng, Vec3::ZERO);
        assert_eq!(run.monster_mods.rank(0.5).count(), 1);
        assert_eq!(
            crate::survivors::monster_specials::player::buffs(&run)[0].left,
            Some(expected)
        );
        assert_eq!(run.monster_mods.rank(expected + 0.5).count(), 0);
    }
}
#[test]
fn legacy_and_visage_make_cross_class_and_duplicate_cards_available_without_extra_slots() {
    let mut run = owner(&[LEGACY, VISAGE]);
    run.abilities.push(Owned {
        def: data::ability("fireball"),
        level: 1,
        cd: 0.0,
    });
    let config = reward_config::Config::default();
    let mut rng = Rng(42);
    let mut duplicate = false;
    let mut foreign = false;
    let mut upgrade = false;
    for _ in 0..1000 {
        for choice in hud::choices_for(&run, &mut rng, &config) {
            match choice {
                hud::Choice::NewAbility(d) => {
                    duplicate |= d.key == "fireball";
                    foreign |= !run.class.pool.contains(&d.key);
                }
                hud::Choice::Upgrade(0) => upgrade = true,
                _ => {}
            }
        }
    }
    assert!(duplicate && foreign && upgrade);
    let mut offer = hud::Offer::default();
    let mut next = NextState::<crate::survivors::Phase>::default();
    run.pending_levels = 1;
    offer.choices = vec![hud::Choice::NewAbility(data::ability("fireball"))];
    hud::pick(0, &mut run, &mut offer, &mut rng, &mut next);
    assert_eq!(run.abilities.len(), 2);
    run.pending_levels = 1;
    offer.choices = vec![hud::Choice::Upgrade(1)];
    hud::pick(0, &mut run, &mut offer, &mut rng, &mut next);
    assert_eq!(run.abilities[0].level, 1);
    assert_eq!(run.abilities[1].level, 2);
}

#[test]
fn bulwark_blocks_direct_area_spells_but_never_periodic_ticks() {
    let mut run = owner(&[BULWARK]);
    run.equipment.block = 1.0;
    run.extra_spells.dodge_rng = Rng(1);
    assert_eq!(
        hero::hurt_hero_tagged(&mut run, 100.0, Tags::FIRE | Tags::AREA),
        0.0
    );
    assert_eq!(run.legendary.chase.block_serial, 1);
    assert!(hero::hurt_hero_tagged(&mut run, 10.0, Tags::FIRE | Tags::DOT) > 0.0);
    assert_eq!(run.legendary.chase.block_serial, 1);
}
