//! Hidden milestone spells: shared modifier rules, independent gameplay tags and DBC visuals.
mod view;
use super::{
    data::{AbilityDef, Effect, Kind},
    spell_engine::{self, Resolved, Tags},
    talent_design::Mechanic as M,
    Run,
};
pub(super) use view::{views, ViewSpell};

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
        DURATION
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
        AREA
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
        TOTEM
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
        DURATION
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
    def.damage = damage;
    def.dot = dot;
    def.duration = duration;
    def.radius = radius;
    def.count = count as u32;
    // No second weapon roll: weapon damage is already included in attack-derived payloads.
    let (def, tags) = super::items::legendary::profile(run, &def);
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
