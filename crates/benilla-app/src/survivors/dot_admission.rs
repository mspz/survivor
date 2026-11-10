//! Conservative admission of new DoTs when the existing next ticks already cover remaining HP.
use super::{
    Run, enemies::Enemy, items::legendary::periodic, resistances::Resistances,
    spell_engine::on_hit, support_spells::Debuffs, talent_runtime::Taunted,
};
use bevy::prelude::*;

pub(super) struct Target<'a> {
    pub entity: Entity,
    pub hp: f32,
    pub armour: f32,
    pub resistances: Resistances,
    pub multiplier: f32,
    pub debuffs: Option<&'a Debuffs>,
    pub taunt: Option<&'a Taunted>,
}

#[cfg(test)]
fn covered(run: &Run, enemy: &Enemy, target: Target<'_>) -> bool {
    covered_with(run, enemy, target, &mut Default::default())
}

pub(super) type ModifierCache =
    std::collections::HashMap<(Option<u64>, &'static str, u64), on_hit::HitModifiers>;

#[cfg(test)]
fn covered_with(
    run: &Run,
    enemy: &Enemy,
    target: Target<'_>,
    modifiers: &mut ModifierCache,
) -> bool {
    covered_cached(run, enemy, target, modifiers, &mut Default::default())
}

pub(super) type FactorCache = std::collections::HashMap<(&'static str, u64), f32>;

pub(super) fn covered_cached(
    run: &Run,
    enemy: &Enemy,
    target: Target<'_>,
    modifiers: &mut ModifierCache,
    factors: &mut FactorCache,
) -> bool {
    if target.hp <= 0.0 || enemy.dots.is_empty() || super::training::dummy(enemy)
        // These effects consume, redirect or turn a DoT into an area affecting other targets.
        // A forecast about this target alone must not suppress those interactions.
        || [147, 226, 240].into_iter().any(|id| run.legendary.has(id))
    {
        return false;
    }
    let mut covered = 0.0_f64;
    // Reuse storage across targets/frames, with no limit on the number of effect families.
    // Values themselves never survive a forecast: defenses and debuffs may have changed.
    factors.clear();
    for dot in &enemy.dots {
        // An absent owner's modifiers cannot be reconstructed from this player's state.
        // Ignore that contribution rather than overestimating the lethal damage.
        if dot.owner != run.owner {
            continue;
        }
        let Some((raw, tags)) = periodic::next_tick(run, dot, enemy.root_t > 0.0, enemy.boss)
        else {
            continue;
        };
        let key = (dot.skill, tags.0);
        let cached = factors.get(&key).copied();
        let factor = cached.unwrap_or_else(|| {
            // Shared with direct hits for this batch; target-dependent damage stays live.
            let mods = *modifiers
                .entry((run.owner, dot.skill, tags.0))
                .or_insert_with(|| {
                    on_hit::modifiers_tagged(dot.skill, tags, run.spell_modifiers.entries())
                });
            // Conditional full/low-life bonuses can change as the ticks land. Use the lowest
            // possible multiplier, so their ordering cannot cause premature suppression.
            let factor = [enemy.max_hp, enemy.max_hp * 0.5, 0.0]
                .into_iter()
                .map(|hp| on_hit::damage(1.0, hp, enemy.max_hp, enemy.boss, target.armour, mods))
                .fold(f32::INFINITY, f32::min)
                * (1.0 - target.resistances.reduction(tags))
                * target.multiplier;
            factors.insert(key, factor);
            factor
        });
        let at = run.elapsed + dot.tick.max(0.0);
        let multiplier = target.debuffs.map_or(1.0, |state| {
            state.multiplier(
                target.entity,
                at,
                tags,
                periodic::family(dot) == periodic::Family::Bleed,
            )
        }) * target
            .taunt
            .map_or(1.0, |taunt| taunt.damage_multiplier(at));
        covered += f64::from(raw * factor * multiplier);
        if covered > f64::from(target.hp) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::survivors::{
        enemies::Dot,
        items::legendary::events::Origin,
        spell_engine::{Change, Filter, Modifier, Tags},
    };
    #[test]
    fn forecasts_cache_every_family_and_reuse_storage_without_stale_defenses() {
        let run = Run::default();
        let mut en = enemy(1.0);
        en.dots = (0..24)
            .map(|i| {
                let mut dot = en.dots[0].clone();
                dot.tags = Some(Tags(
                    Tags::FIRE.0 | Tags::DAMAGE.0 | Tags::DOT.0 | (i << 40),
                ));
                dot
            })
            .collect();
        let mut mods = ModifierCache::new();
        let mut factors = FactorCache::new();
        assert!(!covered_cached(
            &run,
            &en,
            target(),
            &mut mods,
            &mut factors
        ));
        assert_eq!(factors.len(), 24);
        let capacity = factors.capacity();
        for dot in &mut en.dots {
            dot.dps = 10.0;
        }
        assert!(covered_cached(&run, &en, target(), &mut mods, &mut factors));
        let mut protected = target();
        protected.multiplier = 0.01;
        assert!(!covered_cached(
            &run,
            &en,
            protected,
            &mut mods,
            &mut factors
        ));
        assert_eq!(factors.capacity(), capacity);
    }
    fn enemy(dps: f32) -> Enemy {
        Enemy {
            def: &crate::survivors::data::roster(0).enemies[0],
            hp: 100.0,
            max_hp: 100.0,
            speed: 0.0,
            damage: 0.0,
            radius: 1.0,
            xp: 0,
            boss: false,
            attack_cd: 0.0,
            slow: 1.0,
            slow_t: 0.0,
            root_t: 0.0,
            stun_t: 0.0,
            fear_t: 0.0,
            weaken: 1.0,
            weaken_t: 0.0,
            dots: vec![Dot {
                owner: None,
                skill: "rend",
                meter_source: None,
                tags: Some(Tags::FIRE | Tags::DAMAGE | Tags::DOT),
                item_source: None,
                origin: Origin::Skill,
                meta: default(),
                dps,
                left: 4.0,
                tick: 1.0,
            }],
            dead: false,
        }
    }
    fn target() -> Target<'static> {
        Target {
            entity: Entity::PLACEHOLDER,
            hp: 100.0,
            armour: 0.0,
            resistances: default(),
            multiplier: 1.0,
            debuffs: None,
            taunt: None,
        }
    }
    #[test]
    fn admission_uses_strict_one_tick_damage_after_resistance_and_protection() {
        let run = Run::default();
        assert!(!covered(&run, &enemy(100.0), target()));
        assert!(covered(&run, &enemy(101.0), target()));
        let mut protected = target();
        protected.multiplier = 0.25;
        assert!(!covered(&run, &enemy(200.0), protected));
        let mut resistant = target();
        resistant.resistances.fire = 75.0;
        assert!(!covered(&run, &enemy(200.0), resistant));
        let mut armoured = enemy(200.0);
        armoured.dots[0].tags = Some(Tags::PHYSICAL | Tags::DOT | Tags::DAMAGE);
        let mut armour = target();
        armour.armour = 100_000.0;
        assert!(!covered(&run, &armoured, armour));
    }
    #[test]
    fn expired_future_held_and_unknown_owner_ticks_are_not_counted() {
        let mut run = Run::default();
        let mut en = enemy(500.0);
        en.dots[0].left = 0.5;
        assert!(!covered(&run, &en, target()));
        en.dots[0].left = 4.0;
        en.dots[0].tick = 1.1;
        assert!(!covered(&run, &en, target()));
        en.dots[0].tick = 1.0;
        en.dots[0].owner = Some(7);
        assert!(!covered(&run, &en, target()));
        en.dots[0].owner = None;
        en.dots[0].meta.family = periodic::Family::Burn;
        run.legendary.equipped.insert(
            24,
            crate::survivors::items::legendary::Roll {
                unique: 24,
                milestone: None,
            },
        );
        assert!(!covered(&run, &en, target()));
        assert_eq!(
            en.dots[0].meta.held, 0.0,
            "prediction must not modify the dot"
        );
    }
    #[test]
    fn conditional_damage_bonuses_do_not_inflate_the_prediction() {
        let mut run = Run::default();
        run.replace_spell_modifiers(
            99,
            [Modifier {
                source: 99,
                filter: Filter::default(),
                change: Change::DamageAgainstFullLife(1.0),
            }],
        );
        let mut en = enemy(30.0);
        en.dots.push(en.dots[0].clone());
        assert!(!covered(&run, &en, target()));
    }
    #[test]
    fn redirected_and_consumed_dots_keep_their_off_target_interactions() {
        for id in [147, 226, 240] {
            let mut run = Run::default();
            run.legendary.equipped.insert(
                id,
                crate::survivors::items::legendary::Roll {
                    unique: id,
                    milestone: None,
                },
            );
            assert!(!covered(&run, &enemy(500.0), target()));
        }
    }
    #[test]
    fn shared_modifier_cache_keeps_target_defenses_and_owners_independent() {
        let mut cache = ModifierCache::default();
        let run = Run::default();
        assert!(covered_with(&run, &enemy(150.0), target(), &mut cache));
        let mut protected = target();
        protected.multiplier = 0.1;
        assert!(!covered_with(&run, &enemy(150.0), protected, &mut cache));
        let mut resistant = target();
        resistant.resistances.fire = 75.0;
        assert!(!covered_with(&run, &enemy(150.0), resistant, &mut cache));
        let mut guest = Run::default();
        guest.owner = Some(42);
        guest.replace_spell_modifiers(
            99,
            [Modifier {
                source: 99,
                filter: Filter::default(),
                change: Change::DamageAgainstBosses(10.0),
            }],
        );
        let mut boss = enemy(50.0);
        boss.boss = true;
        boss.dots[0].owner = guest.owner;
        assert!(covered_with(&guest, &boss, target(), &mut cache));
        boss.dots[0].owner = run.owner;
        assert!(!covered_with(&run, &boss, target(), &mut cache));
    }
}
