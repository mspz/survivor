//! Minion commands use owner snapshots; each pet supplies its own origin and target.
use super::{
    abilities::Hit,
    data::{self, AbilityDef},
    spell_engine::{self, Change, Filter, Modifier, Resolved, Tags},
    talent_design::Mechanic as M,
    talent_runtime, Run,
};
use bevy::prelude::*;
const WRATH_SOURCE: u64 = 0x7200_0000;
pub(super) const PACK_ATTACK_SOURCE: &str = "Hungry Pack (attacks)";
pub(super) const PACK_BLEED_SOURCE: &str = "Hungry Pack (bleed)";

pub(super) fn tame_damage(level: u8) -> f32 {
    18.0 + 5.0 * super::specialization::steps(level)
}

#[derive(Default)]
pub(super) struct Runtime {
    pub tame_active: bool,
    pub stomp: Option<Resolved>,
    pub command: Option<Resolved>,
    wrath_until: f32,
}
pub(super) fn has_pet_source(run: &Run) -> bool {
    talent_runtime::active(run, M::Pet)
        || talent_runtime::active(run, M::Pack)
        || run
            .abilities
            .iter()
            .any(|a| a.def.key == "tame_beast" && !run.disabled_spells.contains(a.def.key))
}
pub(super) fn eligible(run: &Run, key: &str) -> bool {
    !matches!(key, "stomp" | "kill_command" | "bestial_wrath") || has_pet_source(run)
}

/// Pet-executed damage may activate pet passives without reopening recursive skill procs.
pub(super) fn on_hit(
    run: &mut Run,
    hit: &Hit,
    tags: Tags,
    enemy: &mut super::enemies::Enemy,
    target: Entity,
    pos: Vec3,
) {
    if run.proc_runtime.blocked_dot_target == Some(target) { return; }
    use super::items::legendary::{events::Origin, periodic};
    if !tags.contains(Tags::MINION | Tags::DAMAGE)
        || hit.damage <= 0.0
        || hit.origin == Origin::Legendary
        || !talent_runtime::active(run, M::Pack)
    {
        return;
    }
    let bleed_tags =
        Tags::MINION | Tags::ATTACK | Tags::PHYSICAL | Tags::DAMAGE | Tags::DOT | Tags::DURATION;
    let (mut dot_increase, mut dot_more, mut duration) = (0.0f32, 1.0f32, 0.0f32);
    for modifier in run
        .spell_modifiers
        .entries()
        .iter()
        .filter(|m| m.filter.matches("proc_bleed", bleed_tags))
    {
        match modifier.change {
            Change::IncreasedDotDamage(v) if v.is_finite() => dot_increase += v,
            Change::MoreDotDamage(v) if v.is_finite() => dot_more *= (1.0 + v).max(0.0),
            Change::IncreasedDuration(v) if v.is_finite() => duration += v,
            _ => {}
        }
    }
    // The hit already contains weapon damage and ordinary damage multipliers. Add only
    // periodic-specific scaling, so pet commands do not apply those bonuses twice.
    let dps = hit.damage * 0.2 * (1.0 + dot_increase).max(0.0) * dot_more;
    let left = (4.0 * (1.0 + duration).max(0.0)).min(300.0);
    if dps <= 0.0 || left <= 0.0 {
        return;
    }
    periodic::install(
        run,
        enemy,
        target,
        pos,
        super::enemies::Dot {
            owner: hit.owner,
            skill: "proc_bleed",
            meter_source: Some(PACK_BLEED_SOURCE),
            tags: Some(bleed_tags),
            origin: Origin::Talent,
            item_source: None,
            meta: Default::default(),
            dps,
            left,
            tick: 1.0,
        },
    );
}
pub(super) fn name(key: &str) -> Option<&'static str> {
    Some(match key {
        "tame_beast" => "Tame Beast",
        "stomp" => "Stomp",
        "kill_command" => "Kill Command",
        "bestial_wrath" => "Bestial Wrath",
        _ => return None,
    })
}
pub(super) fn description(def: &AbilityDef, p: &Resolved, level: u8) -> Option<String> {
    Some(match def.key {
        "tame_beast" => format!("Summons {} permanent {}. Each wolf's attacks hit one target and gain damage with this spell's ranks. They inherit your stats and Minion bonuses and cannot be attacked by enemies.", if level >= 6 { 2 } else { 1 }, if level >= 6 { "wolves" } else { "wolf" }),
        "stomp" => format!("Every pet stomps within {:.1} yd, dealing {:.0} damage and slowing enemies by 50%. Requires a pet. Scales with Minion bonuses.", p.radius, p.dmg),
        "kill_command" => format!("Every pet chooses its own nearby target, charges it and attacks for {:.0} damage. Requires a pet. Scales with Minion bonuses.", p.dmg),
        "bestial_wrath" => format!("Minions gain +{:.1}% damage, attack speed and movement speed for {:.1} sec. Scales with buff effect. Requires a pet.", 20.0 * (1.0 + p.buff_effect), p.duration),
        _ => return None,
    })
}
pub(super) fn cast(
    run: &mut Run,
    def: &AbilityDef,
    p: Resolved,
    enemies_present: bool,
) -> Option<bool> {
    name(def.key)?;
    if !eligible(run, def.key) {
        return Some(false);
    }
    match def.key {
        "tame_beast" => run.talent_runtime.minions.tame_active = true,
        "stomp" if enemies_present => run.talent_runtime.minions.stomp = Some(p),
        "kill_command" if enemies_present => run.talent_runtime.minions.command = Some(p),
        "bestial_wrath" if enemies_present => {
            if p.duration <= 0.0 {
                return Some(false);
            }
            let power = 0.2 * (1.0 + p.buff_effect);
            run.talent_runtime.minions.wrath_until = run.elapsed + p.duration;
            run.replace_spell_modifiers(
                WRATH_SOURCE,
                [
                    Change::IncreasedDamage(power),
                    Change::IncreasedAttackSpeed(power),
                    Change::IncreasedMovementSpeed(power),
                ]
                .into_iter()
                .map(|change| Modifier {
                    source: WRATH_SOURCE,
                    filter: Filter::requiring(Tags::MINION),
                    change,
                }),
            );
        }
        _ => return Some(false),
    }
    Some(true)
}

#[cfg(test)]
mod tests {
    use super::super::{passives, procs, talents, Owned};
    use super::*;

    fn hunter() -> Run {
        Run {
            class: &data::CLASSES[2],
            abilities: vec![Owned {
                def: data::ability("tame_beast"),
                level: 1,
                cd: 0.0,
            }],
            ..Default::default()
        }
    }
    fn near(a: f32, b: f32) {
        assert!((a - b).abs() < 0.0001, "{a} != {b}");
    }

    #[test]
    fn minion_cards_scale_both_pet_and_demon_damage_speed_without_buffing_the_owner() {
        let mut run = hunter();
        let owner_before = run.resolve_spell(data::ability("arcane_shot"), 1);
        let pets_before = ["proc_pet_bite", "proc_demon_bolt"]
            .map(|key| procs::resolve(&run, key, 18.0, 0.0, 0.0, 0.0, 1));
        for key in [
            "minion_damage",
            "minion_attack_speed",
            "minion_movement_speed",
        ] {
            let p = data::PASSIVES.iter().find(|p| p.key == key).unwrap();
            assert!(passives::eligible(p, &run));
            run.passives.push((p, 1));
        }
        passives::sync(&mut run);
        assert_eq!(
            run.resolve_spell(data::ability("arcane_shot"), 1),
            owner_before
        );
        for (key, before) in ["proc_pet_bite", "proc_demon_bolt"]
            .into_iter()
            .zip(pets_before)
        {
            let after = procs::resolve(&run, key, 18.0, 0.0, 0.0, 0.0, 1);
            assert!(after.tags.contains(Tags::MINION));
            near(after.dmg, before.dmg * 1.1);
            near(after.cooldown, before.cooldown / 1.1);
            near(after.movement_speed, 1.1);
        }
        for key in ["tame_beast", "stomp", "kill_command", "bestial_wrath"] {
            assert!(run
                .resolve_spell(data::ability(key), 1)
                .tags
                .contains(Tags::MINION));
        }
    }

    #[test]
    fn action_speed_multiplies_attack_and_movement_bonuses() {
        let mut run = hunter();
        let before = procs::resolve(&run, "proc_pet_bite", 18.0, 0.0, 0.0, 0.0, 1);
        run.replace_spell_modifiers(
            42,
            [
                Change::IncreasedAttackSpeed(0.3),
                Change::IncreasedMovementSpeed(0.4),
                Change::MoreActionSpeed(0.2),
            ]
            .into_iter()
            .map(|change| Modifier {
                source: 42,
                filter: Filter::requiring(Tags::MINION),
                change,
            }),
        );
        let after = procs::resolve(&run, "proc_pet_bite", 18.0, 0.0, 0.0, 0.0, 1);
        near(after.cooldown, before.cooldown / 1.3 / 1.2);
        near(after.movement_speed, 1.4 * 1.2);
    }

    #[test]
    fn signatures_keep_one_kill_command_target_and_scale_wrath_buff_power() {
        let mut run = hunter();
        run.replace_spell_modifiers(
            44,
            [Modifier {
                source: 44,
                filter: Filter::requiring(Tags::MINION),
                change: Change::AdditionalMeleeTargets(10),
            }],
        );
        for level in 1..=6 {
            assert_eq!(
                run.resolve_spell(data::ability("kill_command"), level)
                    .count,
                1
            );
        }
        run.replace_spell_modifiers(
            45,
            [Modifier {
                source: 45,
                filter: Filter::requiring(Tags::BUFF),
                change: Change::IncreasedBuffEffect(0.5),
            }],
        );
        let wrath = run.resolve_spell(data::ability("bestial_wrath"), 6);
        near(0.2 * (1.0 + wrath.buff_effect), 0.625);
        assert!(description(data::ability("bestial_wrath"), &wrath, 6)
            .unwrap()
            .contains("62.5%"));
    }

    #[test]
    fn wrath_ranks_increase_buff_effect_instead_of_reducing_cooldown() {
        let run = hunter();
        let def = data::ability("bestial_wrath");
        let base = run.resolve_spell(def, 1);
        let mut previous = base.buff_effect;
        for level in 2..=6 {
            let next = run.resolve_spell(def, level);
            near(next.cooldown, base.cooldown);
            assert!(next.buff_effect > previous);
            previous = next.buff_effect;
        }
        near(
            run.resolve_spell(def, 2).buff_effect - base.buff_effect,
            0.1,
        );
    }

    #[test]
    fn wrath_scales_all_three_bonuses_with_buff_effect_expires_and_does_not_stack() {
        let mut run = hunter();
        run.replace_spell_modifiers(
            43,
            [Modifier {
                source: 43,
                filter: Filter::requiring(Tags::BUFF),
                change: Change::IncreasedBuffEffect(0.5),
            }],
        );
        let before = procs::resolve(&run, "proc_pet_bite", 18.0, 0.0, 0.0, 0.0, 1);
        let owner = run.resolve_spell(data::ability("arcane_shot"), 1);
        let def = data::ability("bestial_wrath");
        for _ in 0..2 {
            let p = run.resolve_spell(def, 1);
            assert_eq!(cast(&mut run, def, p, true), Some(true));
            let after = procs::resolve(&run, "proc_pet_bite", 18.0, 0.0, 0.0, 0.0, 1);
            near(after.dmg, before.dmg * 1.3);
            near(after.cooldown, before.cooldown / 1.3);
            near(after.movement_speed, 1.3);
            assert_eq!(run.resolve_spell(data::ability("arcane_shot"), 1), owner);
        }
        run.elapsed = 6.0;
        tick(&mut run);
        assert_eq!(
            procs::resolve(&run, "proc_pet_bite", 18.0, 0.0, 0.0, 0.0, 1),
            before
        );
    }

    #[test]
    fn wild_companion_replaces_arcane_shot_without_granting_bleed() {
        let mut run = hunter();
        run.abilities = vec![Owned {
            def: data::ability("arcane_shot"),
            level: 3,
            cd: 0.2,
        }];
        let mut build = [[0; talents::NODES]; 3];
        build[0][8] = 1;
        talents::Account::from_shared(2, 0, build).apply(2, &mut run);
        assert_eq!(run.abilities[0].def.key, "kill_command");
        assert_eq!(run.abilities[0].level, 3);
        near(
            procs::resolve(&run, "proc_pet_bite", 18.0, 0.0, 0.0, 0.0, 1).movement_speed,
            1.2,
        );
        assert!(!run
            .spell_modifiers
            .entries()
            .iter()
            .any(|m| matches!(m.change, Change::BleedChance(_))));
    }
}
pub(super) fn tick(run: &mut Run) {
    let until = run.talent_runtime.minions.wrath_until;
    if until > 0.0 && run.elapsed >= until {
        run.talent_runtime.minions.wrath_until = 0.0;
        run.replace_spell_modifiers(WRATH_SOURCE, []);
    }
}
pub(super) fn attack(
    run: &Run,
    key: &str,
    mut p: Resolved,
    target: Entity,
    rng: &mut super::Rng,
) -> Hit {
    p.triggered = true;
    let (damage, critical) =
        spell_engine::critical_with_multiplier(p.dmg, p.crit, p.crit_multiplier, rng.f32());
    let mut hit = super::abilities::hit_for(data::ability(key), &p, target, damage, critical);
    hit.owner = run.owner;
    hit.proc_resolved = true;
    hit.splash = 0.0;
    if key == "stomp" {
        if let data::Effect::Slow(_, duration) = hit.effect {
            hit.effect = data::Effect::Slow(0.5, duration);
        }
    }
    hit
}
