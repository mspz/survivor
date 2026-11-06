//! Spell-scaled party blessings. A family refreshes instead of stacking with itself.
use super::{
    data::{self, AbilityDef, Kind},
    spell_engine::{self, Change, Filter, Modifier, Tags},
    talent_design::Mechanic as M,
    talent_runtime, units, Run,
};
use bevy::prelude::*;

const MODIFIER_SOURCE: u64 = 0x7100_0000;
const BASE_POWER: f32 = 20.0; // Percentage points, so +6 spell damage adds 6 points, not 600%.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Family {
    Light,
    Inspiration,
    Tides,
    Wild,
}
impl Family {
    fn index(self) -> usize {
        self as usize
    }
    #[cfg(test)]
    fn spell(self) -> &'static AbilityDef {
        data::ability(match self {
            Self::Light => "holy_light",
            Self::Inspiration => "flash_heal",
            Self::Tides => "lesser_healing_wave",
            Self::Wild => "rejuvenation",
        })
    }
    fn aura_spell(self) -> &'static AbilityDef {
        data::ability(match self {
            Self::Light | Self::Inspiration => "proc_holy_heal",
            Self::Tides => "proc_totem_heal",
            Self::Wild => "proc_ally_buff",
        })
    }
}
#[derive(Clone, Copy, Debug)]
struct Blessing {
    family: Family,
    power: f32,
    duration: f32,
    radius: f32,
}
#[derive(Clone, Copy)]
struct Timed {
    owner: Option<u64>,
    blessing: Blessing,
    left: f32,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Bonuses {
    pub damage: f32,
    pub recovery: f32,
    pub repeat: f32,
    pub reduction: f32,
    pub resistance: f32,
    pub speed: f32,
}
#[derive(Default)]
pub(super) struct State {
    pub visible: Vec<super::player_auras::Aura>,
    pending: Vec<Blessing>,
    timed: Vec<Timed>,
    pub bonuses: Bonuses,
}
pub(super) struct Source {
    owner: Option<u64>,
    aura: Option<Blessing>,
    casts: Vec<Blessing>,
}

fn discipline(run: &Run) -> bool {
    let points = |tree: usize| {
        run.talent_build[tree]
            .iter()
            .map(|r| u32::from(*r))
            .sum::<u32>()
    };
    run.class.id == 5 && points(0) > 0 && points(0) >= points(1)
}
fn family(run: &Run, def: &AbilityDef) -> Option<Family> {
    if discipline(run) || def.kind != Kind::Heal {
        return None;
    }
    Some(match def.key {
        "holy_light" => Family::Light,
        "flash_heal" | "renew" => Family::Inspiration,
        "lesser_healing_wave" => Family::Tides,
        "rejuvenation" | "regrowth" => Family::Wild,
        _ => return None,
    })
}
fn resolve(run: &Run, family: Family, def: &AbilityDef, level: u8) -> Blessing {
    let mut def = def.clone();
    def.damage = BASE_POWER;
    def.dot = 0.0;
    def.duration = 8.0;
    def.radius = 12.0;
    // This is a percentage payload, never a hit. Retain the original heal/school/skill tags
    // and opt into spell-damage scaling, without weapons, crits, or outgoing class balance.
    let tags = spell_engine::tags::for_spell(&def)
        | Tags::DAMAGE
        | Tags::HEAL
        | Tags::UTILITY
        | Tags::AURA
        | Tags::AREA
        | Tags::DURATION;
    // Party damage must not feed back into its own next refresh (including the old Nature aura).
    let modifiers: Vec<_> = run
        .spell_modifiers
        .entries()
        .iter()
        .filter(|m| ![MODIFIER_SOURCE, 0x7000_0000].contains(&m.source))
        .cloned()
        .collect();
    let mut stats = run.stats();
    // DAMAGE above enables spell-power modifiers; this percentage payload is not hit damage.
    stats.level_damage = 0.0;
    let p = spell_engine::resolve_tagged(&def, level, &stats, &modifiers, 0.0, tags);
    Blessing {
        family,
        power: (p.dmg / 100.0).clamp(0.0, 2.0),
        duration: p.duration,
        radius: p.radius,
    }
}
pub(super) fn on_heal_cast(run: &mut Run, def: &AbilityDef, level: u8) {
    if let Some(family) = family(run, def) {
        let blessing = resolve(run, family, def, level);
        run.healer_buffs.pending.push(blessing);
    }
}
fn aura(run: &Run) -> Option<Blessing> {
    let active = |mechanics: &[M]| mechanics.iter().any(|m| talent_runtime::active(run, *m));
    let family = if discipline(run) {
        None
    } else {
        match run.class.id {
            2 if active(&[M::HolyHeal, M::HolyBurst, M::HolyJudgement]) => Some(Family::Light),
            5 if active(&[M::HolyHeal, M::HolyBurst, M::HolyJudgement]) => {
                Some(Family::Inspiration)
            }
            7 if active(&[M::HealTotem, M::DualTotem, M::SpiritTotem]) => Some(Family::Tides),
            11 if active(&[M::AllyBuff, M::Grove]) => Some(Family::Wild),
            _ => None,
        }
    };
    family.map(|f| resolve(run, f, f.aura_spell(), 1))
}
pub(super) fn source(run: &mut Run) -> Source {
    Source {
        owner: run.owner,
        aura: aura(run),
        casts: std::mem::take(&mut run.healer_buffs.pending),
    }
}

fn bonuses(powers: [f32; 4]) -> Bonuses {
    let [light, inspiration, tides, wild] = powers;
    Bonuses {
        damage: inspiration + 0.4 * (light + tides + wild),
        reduction: light.min(0.5),
        recovery: tides.min(1.0),
        repeat: (tides * 0.25).min(1.0),
        speed: wild.min(0.6),
        resistance: (wild * 25.0).min(25.0),
    }
}
pub(super) fn receive<'a>(
    run: &mut Run,
    pos: Vec3,
    dt: f32,
    sources: impl Iterator<Item = (Vec3, &'a Source)>,
) {
    let state = &mut run.healer_buffs;
    for buff in &mut state.timed {
        buff.left -= dt;
    }
    state.timed.retain(|buff| buff.left > 0.0);
    let mut powers = [0.0_f32; 4];
    let mut visible = [None; 4];
    if run.dead_t.is_none() {
        for (source_pos, source) in sources {
            let distance = units::flat(source_pos - pos).length();
            if let Some(aura) = source.aura.filter(|b| distance <= b.radius) {
                let value = &mut powers[aura.family.index()];
                if aura.power >= *value {
                    visible[aura.family.index()] = Some((aura, None));
                }
                *value = value.max(aura.power);
            }
            for blessing in source
                .casts
                .iter()
                .filter(|b| distance <= b.radius && b.duration > 0.0)
            {
                let refreshed = Timed {
                    owner: source.owner,
                    blessing: *blessing,
                    left: blessing.duration,
                };
                if let Some(old) = state
                    .timed
                    .iter_mut()
                    .find(|b| b.owner == source.owner && b.blessing.family == blessing.family)
                {
                    *old = refreshed;
                } else {
                    state.timed.push(refreshed);
                }
            }
        }
        for buff in &state.timed {
            let value = &mut powers[buff.blessing.family.index()];
            if buff.blessing.power > *value || visible[buff.blessing.family.index()].is_none() {
                visible[buff.blessing.family.index()] = Some((buff.blessing, Some(buff.left)));
            }
            *value = value.max(buff.blessing.power);
        }
    } else {
        state.timed.clear();
        state.pending.clear();
    }
    state.visible = visible
        .into_iter()
        .flatten()
        .map(|(b, left)| {
            super::player_auras::Aura::buff(
                b.family.aura_spell().spell,
                match b.family {
                    Family::Light => "Bénédiction de lumière",
                    Family::Inspiration => "Inspiration",
                    Family::Tides => "Bénédiction des marées",
                    Family::Wild => "Bienfait sauvage",
                },
                description(b),
                left,
            )
        })
        .collect();
    let next = bonuses(powers);
    apply(run, next);
}
fn apply(run: &mut Run, next: Bonuses) {
    if next == run.healer_buffs.bonuses {
        return;
    }
    run.healer_buffs.bonuses = next;
    let mut modifiers = vec![];
    for (value, change, tags) in [
        (
            next.damage,
            Change::IncreasedDamage(next.damage),
            Tags::DAMAGE,
        ),
        (
            next.recovery,
            Change::IncreasedCooldownRecovery(next.recovery),
            Tags::default(),
        ),
        (
            next.repeat,
            Change::RecastChance(next.repeat),
            Tags::SPELL | Tags::DAMAGE,
        ),
    ] {
        if value > 0.0 {
            modifiers.push(Modifier {
                source: 0,
                filter: Filter::requiring(tags),
                change,
            });
        }
    }
    run.replace_spell_modifiers(MODIFIER_SOURCE, modifiers);
}

pub(super) fn snapshot(run: &Run) -> [f32; 6] {
    let b = run.healer_buffs.bonuses;
    [
        b.damage,
        b.recovery,
        b.repeat,
        b.reduction,
        b.resistance,
        b.speed,
    ]
}
pub(super) fn restore(run: &mut Run, values: [f32; 6]) {
    let clean = |index: usize, max: f32| {
        if values[index].is_finite() {
            values[index].clamp(0.0, max)
        } else {
            0.0
        }
    };
    apply(
        run,
        Bonuses {
            damage: clean(0, 4.4),
            recovery: clean(1, 1.0),
            repeat: clean(2, 1.0),
            reduction: clean(3, 0.5),
            resistance: clean(4, 25.0),
            speed: clean(5, 0.6),
        },
    );
}

fn description(blessing: Blessing) -> String {
    let mut powers = [0.0; 4];
    powers[blessing.family.index()] = blessing.power;
    let b = bonuses(powers);
    let extra = match blessing.family {
        Family::Light => format!(", {:.1}% less damage taken", b.reduction * 100.0),
        Family::Inspiration => String::new(),
        Family::Tides => format!(
            ", +{:.1}% cooldown recovery, +{:.1}% offensive spell repeat chance",
            b.recovery * 100.0,
            b.repeat * 100.0
        ),
        Family::Wild => format!(
            ", +{:.1}% movement speed, +{:.1}% all elemental resistances",
            b.speed * 100.0,
            b.resistance
        ),
    };
    format!("Nearby allies (including you): +{:.1}% damage{extra}. Same blessings refresh; strongest per family applies. Radius {:.1} yd.", b.damage * 100.0, blessing.radius)
}
pub(super) fn aura_tooltip(run: &Run, key: &str) -> Option<String> {
    let blessing = aura(run)?;
    (blessing.family.aura_spell().key == key).then(|| {
        format!(
            "Support aura: {} Active while in range.",
            description(blessing)
        )
    })
}
pub(super) fn tooltip(run: &Run, def: &AbilityDef, level: u8) -> Option<String> {
    let blessing = resolve(run, family(run, def)?, def, level);
    Some(format!(
        "{} Lasts {:.1} sec after casting.",
        description(blessing),
        blessing.duration
    ))
}
pub(super) fn talent_tooltip(spec: &super::talents::Spec, node: usize) -> Option<String> {
    let expected_node = if super::talent_design::index(spec) == 26 {
        12
    } else {
        8
    };
    if node != expected_node {
        return None;
    }
    let family = match super::talent_design::index(spec) {
        3 => Family::Light,
        13 => Family::Inspiration,
        17 => Family::Tides,
        26 => Family::Wild,
        _ => return None,
    };
    Some(format!("Support aura: {} Scales with Intelligence, spell damage and the spell's school; party buffs cannot amplify it.", description(Blessing { family, power: 0.2, duration: 8.0, radius: 12.0 })))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f32, b: f32) {
        assert!((a - b).abs() < 0.0001, "{a} != {b}");
    }
    fn cast(family: Family, owner: u64, power: f32, duration: f32) -> Source {
        Source {
            owner: Some(owner),
            aura: None,
            casts: vec![Blessing {
                family,
                power,
                duration,
                radius: 12.0,
            }],
        }
    }
    fn receive_one(run: &mut Run, source: &Source) {
        receive(run, Vec3::ZERO, 0.1, std::iter::once((Vec3::ZERO, source)));
    }
    #[test]
    fn healer_milestones_grant_the_matching_live_aura_and_tooltip() {
        for (class, tree, node, family) in [
            (1, 0, 8, Family::Light),
            (4, 1, 8, Family::Inspiration),
            (5, 2, 8, Family::Tides),
            (8, 2, 12, Family::Wild),
        ] {
            let mut run = Run {
                class: &data::CLASSES[class],
                ..default()
            };
            run.talent_build[tree][node] = 1;
            talent_runtime::configure(&mut run);
            let blessing = aura(&run).expect("healer milestone aura");
            assert_eq!(blessing.family, family);
            assert!(aura_tooltip(&run, family.aura_spell().key)
                .unwrap()
                .contains("20.0%"));
            run.replace_spell_modifiers(
                321,
                [Modifier {
                    source: 0,
                    filter: Filter::requiring(Tags::SPELL),
                    change: Change::FlatSpellDamage(10.0),
                }],
            );
            assert!(aura(&run).unwrap().power > blessing.power);
            let views = super::super::procs::views(&run);
            let view = views
                .iter()
                .find(|v| v.key == family.aura_spell().key)
                .expect("visible learned support spell");
            assert!(view.lines.iter().any(|l| l.contains("Support aura:")));
        }
    }
    #[test]
    fn base_blessings_have_distinct_roles_and_a_twenty_percent_main_bonus() {
        let mut damages = vec![];
        for family in [
            Family::Light,
            Family::Inspiration,
            Family::Tides,
            Family::Wild,
        ] {
            let mut run = Run::default();
            let blessing = resolve(&run, family, family.spell(), 1);
            near(blessing.power, 0.2);
            on_heal_cast(&mut run, family.spell(), 1);
            let source = source(&mut run);
            receive_one(&mut run, &source);
            let b = run.healer_buffs.bonuses;
            assert!(b.damage > 0.0);
            damages.push(b.damage);
            match family {
                Family::Light => near(b.reduction, 0.2),
                Family::Inspiration => near(b.damage, 0.2),
                Family::Tides => {
                    near(b.recovery, 0.2);
                    near(b.repeat, 0.05);
                }
                Family::Wild => {
                    near(b.speed, 0.2);
                    near(b.resistance, 5.0);
                }
            }
            assert!(tooltip(&run, family.spell(), 1).unwrap().contains("20.0%"));
        }
        assert!(damages[1] > damages[0] && damages[1] > damages[2] && damages[1] > damages[3]);
    }
    #[test]
    fn spell_buffs_scale_with_intelligence_flat_spell_damage_school_rank_area_and_duration() {
        use super::super::items::{self, AffixKind, Effect, SlotKind};
        let config = items::Config::builtin();
        let mut rng = super::super::Rng(567);
        let mut ring = loop {
            let item = config.generate_for_class(1, 5, &mut rng).unwrap();
            if item.slot == SlotKind::Ring {
                break item;
            }
        };
        ring.legendary = None;
        ring.affixes = [
            (Effect::Intelligence, 20.0, AffixKind::Suffix),
            (Effect::SpellDamage, 10.0, AffixKind::Prefix),
        ]
        .into_iter()
        .map(|(effect, value, kind)| items::Affix {
            id: format!("{effect:?}"),
            name: "Test".into(),
            group: format!("{effect:?}"),
            effect,
            value,
            kind,
            tags: vec![],
            proc: None,
        })
        .collect();
        let mut inventory = items::Inventory::fixture(vec![ring]);
        inventory.equip(1, "Ring 1").unwrap();
        let mut run = Run {
            class: &data::CLASSES[4],
            ..default()
        };
        inventory.apply(&mut run);
        run.replace_spell_modifiers(
            321,
            [
                Modifier {
                    source: 0,
                    filter: Filter::requiring(Tags::HOLY),
                    change: Change::IncreasedDamage(0.5),
                },
                Modifier {
                    source: 0,
                    filter: Filter::requiring(Tags::HEAL),
                    change: Change::IncreasedDuration(0.5),
                },
                Modifier {
                    source: 0,
                    filter: Filter::requiring(Tags::AREA),
                    change: Change::IncreasedArea(3.0),
                },
            ],
        );
        let holy = resolve(&run, Family::Inspiration, Family::Inspiration.spell(), 1);
        let nature = resolve(&run, Family::Wild, Family::Wild.spell(), 1);
        near(holy.power, 0.51);
        near(nature.power, 0.36);
        near(holy.duration, 12.0);
        near(holy.radius, 24.0);
        assert!(
            resolve(&run, Family::Inspiration, Family::Inspiration.spell(), 2).power > holy.power
        );
    }
    #[test]
    fn buffs_apply_to_real_hits_dots_cooldowns_repeats_movement_and_incoming_damage() {
        let mut run = Run::default();
        let hit = data::ability("cleave");
        let dot = data::ability("corruption");
        let spell = data::ability("fireball");
        let before = (
            run.resolve_spell(hit, 1),
            run.resolve_spell(dot, 1),
            run.resolve_spell(spell, 1),
            run.stats().speed,
        );
        for family in [
            Family::Light,
            Family::Inspiration,
            Family::Tides,
            Family::Wild,
        ] {
            receive_one(&mut run, &cast(family, family.index() as u64, 0.2, 8.0));
        }
        near(run.resolve_spell(hit, 1).dmg, before.0.dmg * 1.44);
        near(run.resolve_spell(dot, 1).dot, before.1.dot * 1.44);
        near(
            run.resolve_spell(spell, 1).cooldown,
            before.2.cooldown / 1.2,
        );
        near(run.resolve_spell(spell, 1).recast, before.2.recast + 0.05);
        near(run.resolve_spell(hit, 1).recast, before.0.recast);
        near(run.stats().speed, before.3 * 1.2);
        near(run.damage_reduction_tagged(Tags::PHYSICAL), 0.2);
        near(
            run.damage_reduction_tagged(Tags::FIRE | Tags::DOT),
            1.0 - 0.8 * 0.95,
        );
        let values = snapshot(&run);
        let mut guest = Run::default();
        restore(&mut guest, values);
        assert_eq!(snapshot(&guest), values);
        near(
            guest.resolve_spell(spell, 1).dmg,
            run.resolve_spell(spell, 1).dmg,
        );
        near(guest.stats().speed, run.stats().speed);
    }
    #[test]
    fn refreshes_do_not_stack_and_weaker_sources_do_not_keep_expired_stronger_buffs() {
        let mut run = Run::default();
        let strong = cast(Family::Inspiration, 1, 0.4, 1.0);
        let weak = cast(Family::Inspiration, 2, 0.2, 8.0);
        receive_one(&mut run, &strong);
        receive_one(&mut run, &strong);
        receive_one(&mut run, &weak);
        near(run.healer_buffs.bonuses.damage, 0.4);
        assert_eq!(run.healer_buffs.timed.len(), 2);
        receive(&mut run, Vec3::ZERO, 1.0, std::iter::empty());
        near(run.healer_buffs.bonuses.damage, 0.2);
        receive(&mut run, Vec3::ZERO, 8.0, std::iter::empty());
        assert_eq!(run.healer_buffs.bonuses, Bonuses::default());
    }
    #[test]
    fn aura_power_cannot_feed_back_through_received_blessings_and_leaving_range_removes_it() {
        let mut run = Run {
            class: &data::CLASSES[4],
            ..default()
        };
        run.talent_build[1][8] = 1;
        talent_runtime::configure(&mut run);
        let baseline = source(&mut run).aura.unwrap().power;
        for _ in 0..30 {
            let source = source(&mut run);
            near(source.aura.unwrap().power, baseline);
            receive_one(&mut run, &source);
        }
        let version = run.version;
        let aura = source(&mut run);
        receive_one(&mut run, &aura);
        assert_eq!(run.version, version);
        receive(
            &mut run,
            Vec3::ZERO,
            0.1,
            std::iter::once((Vec3::X * 100.0, &aura)),
        );
        assert_eq!(run.healer_buffs.bonuses, Bonuses::default());
    }
    #[test]
    fn discipline_never_emits_blessings_and_dead_or_distant_allies_do_not_receive_them() {
        let mut priest = Run {
            class: &data::CLASSES[4],
            ..default()
        };
        priest.talent_build[0][0] = 1;
        on_heal_cast(&mut priest, data::ability("flash_heal"), 1);
        let source = source(&mut priest);
        assert!(source.casts.is_empty() && source.aura.is_none());
        assert!(tooltip(&priest, data::ability("flash_heal"), 1).is_none());
        let source = cast(Family::Inspiration, 1, 0.2, 8.0);
        let mut run = Run::default();
        receive(
            &mut run,
            Vec3::X * 100.0,
            0.1,
            std::iter::once((Vec3::ZERO, &source)),
        );
        assert_eq!(run.healer_buffs.bonuses, Bonuses::default());
        run.dead_t = Some(0.0);
        receive_one(&mut run, &source);
        assert_eq!(run.healer_buffs.bonuses, Bonuses::default());
        assert!(run.healer_buffs.timed.is_empty());
    }
    #[test]
    fn large_scaling_is_bounded_and_resistances_keep_the_seventy_five_percent_cap() {
        let mut run = Run::default();
        run.replace_spell_modifiers(
            123,
            [Modifier {
                source: 0,
                filter: Filter::default(),
                change: Change::FlatSpellDamage(10000.0),
            }],
        );
        let blessing = resolve(&run, Family::Wild, Family::Wild.spell(), 8);
        near(blessing.power, 2.0);
        run.equipment.resistances.add_all(70.0);
        let source = Source {
            owner: None,
            aura: Some(blessing),
            casts: vec![],
        };
        receive_one(&mut run, &source);
        near(run.healer_buffs.bonuses.speed, 0.6);
        near(
            run.stats().resistances.reduction(Tags::NATURE | Tags::DOT),
            0.75,
        );
    }
}
