//! Hidden milestone spells: shared modifier rules, independent gameplay tags and DBC visuals.
mod view;
use super::{
    data::{AbilityDef, Effect, Kind},
    spell_engine::{self, Resolved, Tags},
    talent_design::Mechanic as M,
    Run,
};
pub(super) use view::{views, ViewSpell};

type Key = (
    &'static str,
    Option<u16>,
    Option<&'static str>,
    super::items::legendary::events::Origin,
);
#[derive(Default)]
pub(super) struct Runtime {
    pub parent: u64,
    serial: u64,
    clean_at: f32,
    trigger_cooldowns: std::collections::HashMap<Key, f32>,
    geometry: std::collections::HashMap<(Key, u64), f32>,
    geometry_clean_at: f32,
    cooldowns: std::collections::HashMap<Key, f32>,
    activations: std::collections::HashMap<
        (Key, u64),
        (f32, bool, std::collections::HashSet<bevy::prelude::Entity>),
    >,
}
pub(super) fn cooldown(run: &Run, key: &str, tags: Tags, base: f32) -> f32 {
    let recovery: f32 = run
        .spell_modifiers
        .entries()
        .iter()
        .filter(|m| m.filter.matches(key, tags))
        .filter_map(|m| match m.change {
            spell_engine::Change::IncreasedCooldownRecovery(v) if v.is_finite() => Some(v),
            _ => None,
        })
        .sum();
    base.max(0.5) * (1.0 - run.stats().haste).max(0.35) / (1.0 + recovery).max(0.05)
}
pub(super) fn ready(run: &mut Run, key: &'static str, tags: Tags) -> bool {
    trigger_ready(run, key, None, find(key).map_or(tags, |s| s.tags))
}
pub(super) fn ready_milestone(run: &mut Run, mechanic: M, tags: Tags) -> bool {
    let key = key(run, mechanic);
    ready(run, key, tags)
}
pub(super) fn ready_effect(
    run: &mut Run,
    key: &'static str,
    item: Option<u16>,
    tags: Tags,
) -> bool {
    let parent = run.proc_runtime.parent;
    let token = (
        (
            key,
            item,
            None,
            super::items::legendary::events::Origin::Talent,
        ),
        parent,
    );
    if run.elapsed >= run.proc_runtime.geometry_clean_at {
        let now = run.elapsed;
        run.proc_runtime.geometry.retain(|_, at| now - *at < 310.0);
        run.proc_runtime.geometry_clean_at = now + 1.0;
    }
    if parent != 0 && run.proc_runtime.geometry.contains_key(&token) {
        return true;
    }
    let ready = trigger_ready(run, key, item, tags);
    if ready && parent != 0 {
        run.proc_runtime.geometry.insert(token, run.elapsed);
    }
    ready
}
fn trigger_ready(run: &mut Run, key: &'static str, item: Option<u16>, tags: Tags) -> bool {
    let cd = if key == "proc_arcane_companion" {
        0.75
    } else {
        cooldown(run, key, tags, 0.5)
    };
    let id = (
        key,
        item,
        None,
        super::items::legendary::events::Origin::Talent,
    );
    if run
        .proc_runtime
        .trigger_cooldowns
        .get(&id)
        .is_some_and(|at| *at > run.elapsed)
    {
        return false;
    }
    run.proc_runtime
        .trigger_cooldowns
        .insert(id, run.elapsed + cd);
    true
}
pub(super) fn fresh(run: &mut Run) -> u64 {
    run.proc_runtime.serial = run.proc_runtime.serial.wrapping_add(1);
    (1 << 63) | run.proc_runtime.serial
}
pub(super) fn parent(run: &mut Run, cast: u64) -> u64 {
    let id = if cast != 0 { cast } else { fresh(run) };
    run.proc_runtime.parent = id;
    id
}
/// Admit an activation once, then keep its target copies together across delayed missiles.
/// Area copies from the same parent share their victim set, independently of landing time.
pub(super) fn admit(run: &mut Run, hit: &super::abilities::Hit, area: bool) -> bool {
    let key = (hit.skill, hit.item_source, hit.meter_source, hit.origin);
    let cd = cooldown(run, hit.skill, hit.tags.unwrap_or(Tags::DAMAGE), 0.5);
    let now = run.elapsed;
    let state = &mut run.proc_runtime;
    if now >= state.clean_at {
        state.activations.retain(|_, (at, _, _)| now - *at < 310.0);
        state.clean_at = now + 1.0;
    }
    let parent = if hit.cast_id == 0 {
        (1 << 62) | (f64::from(now) * 10000.0) as u64
    } else {
        hit.cast_id
    };
    let token = (key, parent);
    if !state.activations.contains_key(&token) {
        let allowed =
            hit.proc_admitted || state.cooldowns.get(&key).is_none_or(|until| *until <= now);
        if allowed && !hit.proc_admitted {
            state.cooldowns.insert(key, now + cd);
        }
        state
            .activations
            .insert(token, (now, allowed, Default::default()));
    }
    let (_, allowed, victims) = state.activations.get_mut(&token).unwrap();
    *allowed && (!area || victims.insert(hit.target))
}

pub(super) struct Spell {
    pub def: AbilityDef,
    pub tags: Tags,
}
const fn spell(key: &'static str, visual: u32, kind: Kind, tags: Tags) -> Spell {
    Spell {
        def: AbilityDef {
            key,
            spell: visual,
            kind,
            damage: 1.0,
            dot: 0.0,
            cooldown: 1.0,
            range: if matches!(kind, Kind::Melee) {
                3.0
            } else {
                20.0
            },
            radius: 0.0,
            count: 1,
            duration: 0.0,
            effect: Effect::None,
            mark: 0,
            totem: (0, 0),
            overhead: false,
        },
        tags,
    }
}
macro_rules! hidden {
    ($key:literal, $visual:literal, $kind:ident, $($tag:ident),+) => {
        spell($key, $visual, Kind::$kind, Tags($(Tags::$tag.0)|+))
    };
}
pub(super) static SPELLS: &[Spell] = &[
    hidden!(
        "proc_bleed",
        1943,
        Dot,
        ATTACK,
        PHYSICAL,
        DAMAGE,
        DOT,
        DURATION
    ),
    hidden!("proc_ignite", 133, Dot, SPELL, FIRE, DAMAGE, DOT, DURATION),
    hidden!(
        "proc_poison",
        2818,
        Dot,
        SPELL,
        NATURE,
        DAMAGE,
        DOT,
        DURATION
    ),
    hidden!(
        "proc_plague",
        172,
        Dot,
        SPELL,
        SHADOW,
        DAMAGE,
        DOT,
        DURATION
    ),
    hidden!(
        "proc_lunar_flare",
        8921,
        Dot,
        SPELL,
        ARCANE,
        DAMAGE,
        DOT,
        DURATION
    ),
    hidden!("proc_bloodburst", 1449, Nova, SPELL, PHYSICAL, DAMAGE, AREA),
    hidden!("proc_poisonburst", 2818, Nova, SPELL, NATURE, DAMAGE, AREA),
    hidden!("proc_fireburst", 133, Nova, SPELL, FIRE, DAMAGE, AREA),
    hidden!("proc_plagueburst", 1449, Nova, SPELL, SHADOW, DAMAGE, AREA),
    hidden!(
        "proc_frostnova",
        122,
        Nova,
        SPELL,
        COLD,
        DAMAGE,
        AREA,
        DURATION
    ),
    hidden!(
        "proc_poison_spread",
        2818,
        Nova,
        SPELL,
        NATURE,
        AREA,
        DOT,
        DURATION
    ),
    hidden!(
        "proc_ignite_spread",
        133,
        Nova,
        SPELL,
        FIRE,
        AREA,
        DOT,
        DURATION
    ),
    hidden!(
        "proc_plague_spread",
        172,
        Nova,
        SPELL,
        SHADOW,
        AREA,
        DOT,
        DURATION
    ),
    hidden!(
        "proc_splash",
        13877,
        Nova,
        ATTACK,
        MELEE,
        PHYSICAL,
        DAMAGE,
        AREA
    ),
    hidden!(
        "proc_flurry",
        13877,
        Nova,
        ATTACK,
        MELEE,
        PHYSICAL,
        DAMAGE,
        AREA
    ),
    hidden!(
        "proc_execution",
        2098,
        Melee,
        ATTACK,
        MELEE,
        PHYSICAL,
        DAMAGE
    ),
    hidden!(
        "proc_windfury",
        17364,
        Melee,
        ATTACK,
        MELEE,
        PHYSICAL,
        DAMAGE
    ),
    hidden!("proc_seal", 20473, Strike, SPELL, HOLY, DAMAGE),
    hidden!("proc_seal_storm", 20473, Strike, SPELL, HOLY, DAMAGE, AREA),
    hidden!("proc_flametongue", 8050, Strike, SPELL, FIRE, DAMAGE),
    hidden!(
        "proc_stormstrike",
        421,
        Chain,
        SPELL,
        LIGHTNING,
        NATURE,
        DAMAGE,
        CHAIN,
        AREA
    ),
    hidden!("proc_arcane_blast", 1449, Nova, SPELL, ARCANE, DAMAGE, AREA),
    hidden!("proc_starfall", 1449, Nova, SPELL, ARCANE, DAMAGE, AREA),
    hidden!("proc_cataclysm", 1949, Nova, SPELL, FIRE, DAMAGE, AREA),
    hidden!(
        "proc_thunder_fortress",
        6343,
        Nova,
        ATTACK,
        PHYSICAL,
        DAMAGE,
        AREA
    ),
    hidden!("proc_sanctuary", 15237, Nova, SPELL, HOLY, DAMAGE, AREA),
    hidden!("proc_shieldburst", 15237, Nova, SPELL, HOLY, DAMAGE, AREA),
    hidden!("proc_holy_burst", 15237, Nova, SPELL, HOLY, DAMAGE, AREA),
    hidden!(
        "proc_holy_judgement",
        20473,
        Chain,
        SPELL,
        HOLY,
        DAMAGE,
        AREA,
        CHAIN
    ),
    hidden!(
        "proc_pet_bite",
        772,
        Melee,
        ATTACK,
        MELEE,
        PHYSICAL,
        DAMAGE,
        DOT,
        DURATION,
        MINION
    ),
    hidden!(
        "proc_demon_bolt",
        133,
        Bolt,
        SPELL,
        FIRE,
        DAMAGE,
        PROJECTILE,
        DOT,
        DURATION,
        AREA,
        MINION
    ),
    hidden!(
        "proc_totem_bolt",
        403,
        Bolt,
        SPELL,
        LIGHTNING,
        NATURE,
        DAMAGE,
        PROJECTILE,
        TOTEM,
        BUFF
    ),
    hidden!(
        "proc_spirit_bolt",
        403,
        Bolt,
        SPELL,
        LIGHTNING,
        NATURE,
        DAMAGE,
        PROJECTILE,
        TOTEM,
        CHAIN
    ),
    hidden!(
        "proc_frostbite",
        116,
        Strike,
        SPELL,
        COLD,
        UTILITY,
        DURATION
    ),
    hidden!(
        "proc_taunt",
        355,
        Nova,
        SPELL,
        PHYSICAL,
        UTILITY,
        AREA,
        DURATION
    ),
    hidden!("proc_holy_heal", 635, Heal, SPELL, HOLY, HEAL, AREA),
    hidden!("proc_shadow_leech", 15407, Heal, SPELL, SHADOW, HEAL),
    hidden!(
        "proc_totem_heal",
        5394,
        Heal,
        SPELL,
        NATURE,
        HEAL,
        AREA,
        TOTEM,
        DURATION,
        BUFF
    ),
    hidden!("proc_bloom", 774, Heal, SPELL, NATURE, HEAL, AREA, DURATION),
    hidden!("proc_grove", 774, Heal, SPELL, NATURE, HEAL, AREA, DURATION),
    hidden!(
        "proc_ally_buff",
        1126,
        Heal,
        SPELL,
        NATURE,
        UTILITY,
        AURA,
        AREA,
        DURATION
    ),
];

pub(super) fn find(key: &str) -> Option<&'static Spell> {
    SPELLS.iter().find(|s| s.def.key == key)
}
pub(super) fn source_key(_run: &Run, source: Option<&str>) -> Option<&'static str> {
    let name = source?;
    super::talent_design::DESIGNS
        .iter()
        .enumerate()
        .find_map(|(index, d)| {
            d.milestones
                .iter()
                .find(|m| m.name == name)
                .map(|m| key_for_class(super::data::CLASSES[index / 3].id, m.mechanic))
        })
}
pub(super) fn key(run: &Run, m: M) -> &'static str {
    key_for_class(super::items::legendary::milestones::source_class(run, m), m)
}
fn key_for_class(class: u8, m: M) -> &'static str {
    match m {
        M::Bleed | M::Pet => "proc_bleed",
        M::Ignite => "proc_ignite",
        M::Poison => "proc_poison",
        M::ShadowPlague | M::Plague => "proc_plague",
        M::Moonfire => "proc_lunar_flare",
        M::Bloodburst => "proc_bloodburst",
        M::Poisonburst => "proc_poisonburst",
        M::Fireburst => "proc_fireburst",
        M::Plagueburst => "proc_plagueburst",
        M::DeathTrap => "proc_fireburst",
        M::FrostNova => "proc_frostnova",
        M::Proliferate => "proc_poison_spread",
        M::IgniteSpread => "proc_ignite_spread",
        M::PlagueSpread => "proc_plague_spread",
        M::Splash => "proc_splash",
        M::Flurry => "proc_flurry",
        M::Execution => "proc_execution",
        M::Windfury => "proc_windfury",
        M::Seal if class == 7 => "proc_flametongue",
        M::Seal => "proc_seal",
        M::SealStorm => "proc_seal_storm",
        M::Stormstrike => "proc_stormstrike",
        M::ArcaneBlast => "proc_arcane_blast",
        M::Starfall => "proc_starfall",
        M::Cataclysm => "proc_cataclysm",
        M::Bulwark if class == 1 => "proc_thunder_fortress",
        M::Bulwark => "proc_sanctuary",
        M::Shieldburst | M::ShieldNova => "proc_shieldburst",
        M::HolyBurst => "proc_holy_burst",
        M::HolyJudgement => "proc_holy_judgement",
        M::Demon | M::Demons | M::DemonLord => "proc_demon_bolt",
        M::Pack => "proc_pet_bite",
        M::DualTotem => "proc_totem_bolt",
        M::SpiritTotem => "proc_spirit_bolt",
        M::Freeze => "proc_frostbite",
        M::Taunt => "proc_taunt",
        M::HealTotem => "proc_totem_heal",
        M::Hot => "proc_bloom",
        M::Grove => "proc_grove",
        M::AllyBuff => "proc_ally_buff",
        _ => "proc_holy_heal",
    }
}

/// Payloads may be fixed values or a percentage of the triggering hit. Resolve only once;
/// periodic ticks, splash copies and proliferation retain the resulting snapshot.
pub(in crate::survivors) fn gameplay_tags(run: &Run, key: &str, mut tags: Tags) -> Tags {
    if key == "proc_bloodburst"
        && super::items::legendary::milestones::source_class(run, M::Bloodburst) == 3
    {
        tags = Tags(tags.0 & !Tags::SPELL.0) | Tags::MINION | Tags::ATTACK;
    }
    tags
}

pub(super) fn resolve(
    run: &Run,
    key: &str,
    damage: f32,
    dot: f32,
    duration: f32,
    radius: f32,
    count: usize,
) -> Resolved {
    let mut def = find(key).expect("authored hidden spell").def.clone();
    def.damage = damage
        + if damage > 0.0 && matches!(key, "proc_pet_bite" | "proc_demon_bolt") {
            run.minion_weapon_damage()
        } else {
            0.0
        };
    def.dot = dot;
    def.duration = duration;
    def.radius = radius;
    def.count = count as u32;
    // Only independent pet attacks add owner weapon damage; derived payloads retain it once.
    let (def, mut tags) = super::items::legendary::profile(run, &def);
    tags = gameplay_tags(run, key, tags);
    let mut p = spell_engine::resolve_tagged(
        &def,
        1,
        &run.stats(),
        run.spell_modifiers.entries(),
        0.0,
        tags,
    );
    let balance = run.balance.multiplier_tagged(run.class, key, p.tags);
    p.dmg *= balance;
    p.dot *= balance;
    if run.legendary.has(70) && p.tags.contains(Tags::DAMAGE) && p.dmg > 0.0 {
        p.direct_dot = Some(spell_engine::PeriodicSnapshot {
            dps: super::items::legendary::events::dot_conversion(run, key, p.tags, p.dmg) / 3.0,
            duration: 3.0,
            tags: p.tags | Tags::DOT | Tags::DURATION,
        });
        p.crit = 0.0;
    }
    p
}
pub(super) fn radius(run: &Run, key: &str, base: f32) -> f32 {
    resolve(run, key, 0.0, 0.0, 0.0, base, 1).radius
}
pub(super) fn interval(run: &Run, key: &str, base: f32) -> f32 {
    let mut def = find(key).expect("authored hidden spell").def.clone();
    def.cooldown = base;
    spell_engine::resolve(&def, 1, &run.stats(), run.spell_modifiers.entries()).cooldown
}

/// Reward eligibility also considers learned hidden spells; these never occupy a skill slot.
pub(super) fn available(run: &Run) -> Vec<AbilityDef> {
    let Some(class) = super::data::CLASSES
        .iter()
        .position(|c| c.id == run.class.id)
    else {
        return vec![];
    };
    let mut keys = std::collections::BTreeSet::new();
    if run.abilities.iter().any(|a| a.def.key == "tame_beast") {
        keys.insert("proc_pet_bite");
    }
    for tree in 0..3 {
        for (index, node) in [8, 12, 18].into_iter().enumerate() {
            if run.talent_build[tree][node] == 0 {
                continue;
            }
            let m = super::talent_design::DESIGNS[class * 3 + tree].milestones[index].mechanic;
            match m {
                M::None
                | M::Weapon
                | M::Titans
                | M::Focus
                | M::ArcaneFocus
                | M::ArcaneCompanion
                | M::ArcaneCascade
                | M::TrapLaunch
                | M::TrapVolley
                | M::Shatter
                | M::SlowCast
                | M::FeralGuard => continue,
                M::Pet => {
                    keys.insert("proc_pet_bite");
                }
                M::Block if run.class.id != 2 => continue,
                M::ShadowDrain => {
                    keys.insert("proc_shadow_leech");
                    continue;
                }
                M::HolyJudgement if run.class.id != 2 => {
                    keys.insert("proc_holy_heal");
                    continue;
                }
                M::ShieldHeal | M::HolyHeal | M::HolyJudgement | M::SealStorm | M::Bulwark => {
                    keys.insert("proc_holy_heal");
                }
                M::HolyBurst => {
                    keys.insert("proc_holy_heal");
                }
                M::SpiritTotem => {
                    keys.insert("proc_totem_heal");
                }
                M::PlagueSpread if run.class.id == 5 => {
                    keys.insert("proc_plagueburst");
                }
                _ => {}
            }
            keys.insert(key(run, m));
        }
    }
    for grant in &run.legendary.grants {
        let m = grant.milestone().mechanic;
        if matches!(
            m,
            M::None
                | M::Weapon
                | M::Titans
                | M::Focus
                | M::ArcaneFocus
                | M::ArcaneCompanion
                | M::ArcaneCascade
                | M::TrapLaunch
                | M::TrapVolley
                | M::Shatter
                | M::SlowCast
                | M::FeralGuard
        ) {
            continue;
        }
        keys.insert(key_for_class(grant.class(), m));
        if matches!(m, M::Pet | M::Pack) {
            keys.insert("proc_pet_bite");
        }
        if matches!(m, M::DualTotem | M::SpiritTotem) {
            keys.insert("proc_totem_heal");
        }
        if m == M::Grove {
            keys.insert("proc_bloom");
        }
    }
    keys.into_iter()
        .map(|key| {
            let spell = find(key).unwrap();
            let mut def = spell.def.clone();
            if spell.tags.contains(Tags::DOT)
                && (key != "proc_demon_bolt" || super::talent_runtime::active(run, M::Demons))
                && (key != "proc_pet_bite" || super::talent_runtime::active(run, M::Pack))
            {
                def.dot = 1.0;
            }
            // Permanent summons/auras have no expiry for a Duration card to improve.
            // Pet/demon duration applies only once their temporary ailment is unlocked.
            if spell.tags.contains(Tags::DURATION)
                && !matches!(
                    key,
                    "proc_totem_heal" | "proc_bloom" | "proc_grove" | "proc_ally_buff"
                )
                && (!matches!(key, "proc_pet_bite" | "proc_demon_bolt") || def.dot > 0.0)
            {
                def.duration = 1.0;
            }
            if spell.tags.contains(Tags::AREA)
                && (key != "proc_demon_bolt" || super::talent_runtime::active(run, M::DemonLord))
            {
                def.radius = 1.0;
            }
            def
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::{
        balance, data, passives,
        spell_engine::{Change, Filter, Modifier},
    };
    use super::*;

    #[test]
    fn resonant_casting_has_fixed_three_quarter_second_internal_cooldown() {
        let mut run = run();
        add(
            &mut run,
            &[(Tags::SPELL, Change::IncreasedCooldownRecovery(10.0))],
        );
        assert!(ready(&mut run, "proc_arcane_companion", Tags::SPELL));
        for at in [0.0, 0.5, 0.749] {
            run.elapsed = at;
            assert!(!ready(&mut run, "proc_arcane_companion", Tags::SPELL));
        }
        run.elapsed = 0.75;
        assert!(ready(&mut run, "proc_arcane_companion", Tags::SPELL));
    }

    #[test]
    fn proc_rules_keep_cooldowns_per_effect_and_player_and_apply_cdr() {
        let mut run = run();
        assert_eq!(
            cooldown(&run, "proc_fireburst", Tags::FIRE | Tags::SPELL, 0.5),
            0.5
        );
        assert!(ready(&mut run, "proc_fireburst", Tags::FIRE));
        assert!(!ready(&mut run, "proc_fireburst", Tags::FIRE));
        assert!(ready(&mut run, "proc_poisonburst", Tags::NATURE));
        let mut other = Run::default();
        assert!(ready(&mut other, "proc_fireburst", Tags::FIRE));
        run.elapsed = 0.499;
        assert!(!ready(&mut run, "proc_fireburst", Tags::FIRE));
        run.elapsed = 0.5;
        assert!(ready(&mut run, "proc_fireburst", Tags::FIRE));
        add(
            &mut run,
            &[(Tags::FIRE, Change::IncreasedCooldownRecovery(1.0))],
        );
        assert_eq!(
            cooldown(&run, "proc_fireburst", Tags::FIRE | Tags::SPELL, 0.5),
            0.25
        );
        assert_eq!(cooldown(&run, "proc_poisonburst", Tags::NATURE, 0.5), 0.5);
    }

    #[test]
    fn proc_rules_one_activation_keeps_all_geometry_parts_without_bypassing_new_cast_cooldowns() {
        let mut run = run();
        parent(&mut run, 10);
        assert!(ready_effect(
            &mut run,
            "legendary_proc",
            Some(24),
            Tags::FIRE
        ));
        assert!(ready_effect(
            &mut run,
            "legendary_proc",
            Some(24),
            Tags::FIRE
        ));
        parent(&mut run, 11);
        assert!(!ready_effect(
            &mut run,
            "legendary_proc",
            Some(24),
            Tags::FIRE
        ));
        run.elapsed = 0.5;
        assert!(ready_effect(
            &mut run,
            "legendary_proc",
            Some(24),
            Tags::FIRE
        ));
    }

    #[test]
    fn proc_rules_delayed_impacts_do_not_pay_the_activation_cooldown_twice() {
        use super::super::{abilities::Hit, items::legendary::events::Origin};
        use bevy::prelude::Entity;
        let mut run = run();
        assert!(ready(&mut run, "proc_fireburst", Tags::FIRE));
        run.elapsed = 0.5;
        assert!(ready(&mut run, "proc_fireburst", Tags::FIRE));
        let mut hit = Hit::direct("proc_fireburst", Entity::from_bits(1), 10.0, false);
        hit.origin = Origin::Talent;
        hit.triggered = true;
        hit.proc_admitted = true;
        run.elapsed = 1.0;
        hit.cast_id = 1;
        assert!(admit(&mut run, &hit, true));
        run.elapsed = 1.1;
        hit.cast_id = 2;
        assert!(admit(&mut run, &hit, true));
        assert!(!admit(&mut run, &hit, true));
    }

    #[test]
    fn proc_rules_aoe_shares_victims_across_delayed_copies_of_one_parent() {
        use super::super::{abilities::Hit, items::legendary::events::Origin};
        use bevy::prelude::Entity;
        let mut run = run();
        let mut hit = Hit::direct("proc_fireburst", Entity::from_bits(1), 10.0, false);
        hit.triggered = true;
        hit.origin = Origin::Talent;
        hit.cast_id = 10;
        hit.tags = Some(Tags::FIRE | Tags::SPELL | Tags::AREA);
        assert!(admit(&mut run, &hit, true));
        assert!(!admit(&mut run, &hit, true));
        hit.target = Entity::from_bits(2);
        assert!(admit(&mut run, &hit, true));
        run.elapsed = 0.25;
        hit.cast_id = 11;
        assert!(!admit(&mut run, &hit, true));
        run.elapsed = 1.0;
        hit.cast_id = 10;
        assert!(!admit(&mut run, &hit, true));
        hit.cast_id = 12;
        assert!(admit(&mut run, &hit, true));
    }

    fn run() -> Run {
        Run {
            balance: balance::Config::parse("version = 1\n[classes]\n[tags]\n").unwrap(),
            ..Default::default()
        }
    }
    fn add(run: &mut Run, changes: &[(Tags, Change)]) {
        run.replace_spell_modifiers(
            123,
            changes.iter().map(|&(tag, change)| Modifier {
                source: 123,
                filter: Filter::requiring(tag),
                change,
            }),
        );
    }
    #[test]
    fn hidden_spells_have_unique_keys_and_never_enter_the_visible_spell_pool() {
        let mut seen = std::collections::BTreeSet::new();
        for spell in SPELLS {
            assert!(seen.insert(spell.def.key));
            assert!(!data::ABILITIES.iter().any(|a| a.key == spell.def.key));
            assert_eq!(
                spell_engine::tags::for_spell(data::ability(spell.def.key)),
                spell.tags
            );
        }
        assert_eq!(
            spell_engine::tags::school("proc_bloodburst"),
            Some(Tags::PHYSICAL)
        );
        assert!(!find("proc_bloodburst").unwrap().tags.contains(Tags::ARCANE));
    }
    #[test]
    fn poison_and_bleed_resolve_global_dot_duration_and_school_bonuses() {
        let mut run = run();
        add(
            &mut run,
            &[
                (Tags::DAMAGE, Change::IncreasedDamage(0.5)),
                (Tags::NATURE, Change::IncreasedDamage(0.5)),
                (Tags::DOT, Change::IncreasedDotDamage(1.0)),
                (Tags::DURATION, Change::IncreasedDuration(0.5)),
                (Tags::UTILITY, Change::IncreasedCooldownRecovery(1.0)),
            ],
        );
        let poison = resolve(&run, "proc_poison", 0.0, 10.0, 6.0, 0.0, 1);
        assert_eq!(poison.dot, 30.0);
        assert_eq!(poison.duration, 9.0);
        let bleed = resolve(&run, "proc_bleed", 0.0, 10.0, 4.0, 0.0, 1);
        assert_eq!(bleed.dot, 25.0);
        assert_eq!(bleed.duration, 6.0);
        assert_eq!(bleed.dmg, 0.0);
        assert_eq!(interval(&run, "proc_taunt", 6.0), 3.0 * 1.2);
    }
    #[test]
    fn explosions_and_invocations_respect_area_projectile_chain_crit_and_flat_spell_bonuses() {
        let mut run = run();
        add(
            &mut run,
            &[
                (Tags::AREA, Change::IncreasedArea(3.0)),
                (Tags::SPELL, Change::FlatSpellDamage(6.0)),
                (Tags::PROJECTILE, Change::AdditionalProjectiles(2)),
                (Tags::PROJECTILE, Change::AdditionalChains(2)),
                (Tags::PROJECTILE, Change::Fork(1)),
                (Tags::FIRE, Change::CriticalChance(1.0)),
                (Tags::TOTEM, Change::IncreasedCooldownRecovery(1.0)),
            ],
        );
        let explosion = resolve(&run, "proc_bloodburst", 10.0, 0.0, 0.0, 6.0, 64);
        assert_eq!(explosion.radius, 12.0);
        assert_eq!(explosion.dmg, 16.0);
        let demon = resolve(&run, "proc_demon_bolt", 18.0, 0.0, 0.0, 4.0, 1);
        assert_eq!(demon.dmg, 24.0);
        assert_eq!(demon.count, 3);
        assert_eq!(demon.chains, 2);
        assert_eq!(demon.forks, 1);
        assert_eq!(demon.crit, 1.0);
        assert_eq!(demon.radius, 8.0);
        let pet = resolve(&run, "proc_pet_bite", 18.0, 0.0, 0.0, 0.0, 1);
        assert_eq!(pet.dmg, 18.0);
        assert_eq!(pet.count, 1);
        run.balance =
            balance::Config::parse("version=1\n[classes]\n[tags]\nphysical=2.0\narcane=99.0\n")
                .unwrap();
        assert_eq!(
            resolve(&run, "proc_bloodburst", 10.0, 0.0, 0.0, 6.0, 64).dmg,
            32.0
        );
        assert_eq!(
            resolve(&run, "proc_totem_bolt", 24.0, 0.0, 0.0, 0.0, 1).cooldown,
            0.5 * 1.2
        );
    }
    #[test]
    fn dot_cards_become_eligible_for_a_learned_hidden_poison_without_a_visible_dot_spell() {
        let mut run = run();
        run.class = &data::CLASSES[3];
        run.abilities.clear();
        let card = data::PASSIVES.iter().find(|p| p.key == "dot").unwrap();
        assert!(!passives::eligible(card, &run));
        run.talent_build[0][8] = 1;
        assert!(passives::eligible(card, &run));
        run.passives.push((card, 1));
        passives::sync(&mut run);
        assert!((resolve(&run, "proc_poison", 0.0, 10.0, 6.0, 0.0, 1).dot - 11.5).abs() < 0.001);
    }
}
