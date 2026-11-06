//! Scheduled spell geometry stores positions and reselects living targets when it fires.
use super::events::Origin;
use super::*;
use crate::survivors::{
    abilities::{self, Hit},
    data::{self, Effect},
    enemies::Enemy,
    spell_engine::{
        self,
        targeting::{self, Target},
        Resolved,
    },
    units::{flat, Fx},
};

#[derive(Clone)]
pub(in crate::survivors) struct Plan {
    pub unique: u16,
    pub skill: &'static str,
    pub params: Resolved,
    pub effect: Effect,
    pub at: f32,
    pub until: f32,
    pub every: f32,
    pub center: Vec3,
    pub direction: Option<Vec3>,
    pub guided: bool,
    pub primary: Option<Entity>,
    pub exclude: Vec<Entity>,
    pub origin: Origin,
    pub first: bool,
    splash: f32,
}
#[derive(Default)]
pub(in crate::survivors) struct State {
    pub counts: BTreeMap<&'static str, u64>,
    pub last_used: BTreeMap<&'static str, f32>,
    pub elemental: u64,
    pub last: Option<&'static str>,
    pub previous: Option<&'static str>,
    last_direct: Option<&'static str>,
    prior_direct: BTreeMap<&'static str, &'static str>,
    pub last_school: Tags,
    pub eclipse: bool,
    pub last_melee: Option<(&'static str, Resolved)>,
    pub last_ranged: Option<(&'static str, Resolved)>,
    pub last_heading: Vec3,
    pub pair: Option<(&'static str, &'static str)>,
    pub plans: Vec<Plan>,
    impressions: BTreeMap<&'static str, Plan>,
    last_area: BTreeMap<&'static str, Vec3>,
    sequence: Vec<&'static str>,
    pub anchor: Option<Vec3>,
    pub pet: Option<Vec3>,
    pub spirit_destination: Option<Vec3>,
    pub sides: BTreeMap<u64, i8>,
    pub melee: u64,
    pub rains: Vec<(Vec3, f32)>,
    pub ground: Vec<(Vec3, f32)>,
    pub auras: BTreeMap<&'static str, (Entity, Vec3, Resolved, f32)>,
}
#[derive(Component)]
pub(in crate::survivors) struct Caster(Option<u64>);
const FORMS: u64 = Tags::MELEE.0
    | Tags::PROJECTILE.0
    | Tags::AREA.0
    | Tags::CHAIN.0
    | Tags::TOTEM.0
    | Tags::TRAP.0
    | Tags::AURA.0;
fn shape_tags(kind: Kind) -> Tags {
    match kind {
        Kind::Bolt => Tags::PROJECTILE,
        Kind::Melee => Tags::MELEE,
        Kind::Cone | Kind::Nova | Kind::Ground | Kind::GroundSelf => Tags::AREA,
        Kind::Chain => Tags::CHAIN,
        _ => Tags(0),
    }
}
fn shape_from(def: &mut AbilityDef, tags: &mut Tags, other: &AbilityDef) {
    def.kind = other.kind;
    def.range = other.range;
    def.radius = other.radius;
    def.count = other.count;
    *tags = Tags((tags.0 & !FORMS) | shape_tags(other.kind).0);
}
pub(in crate::survivors) fn profile(
    run: &Run,
    original: &AbilityDef,
    def: &mut AbilityDef,
    tags: &mut Tags,
) {
    let state = &run.legendary.casting;
    let count = *state.counts.get(original.key).unwrap_or(&0);
    if run.legendary.has(13) && original.key == "chain_lightning" {
        def.kind = Kind::Melee;
        def.range = 3.5;
        def.radius = 0.0;
        def.count = 1;
        *tags = Tags((tags.0 & !(Tags::SPELL.0 | FORMS)) | (Tags::ATTACK | Tags::MELEE).0);
    }
    if run.legendary.has(43) && original.key == "cleave" {
        def.count = 1;
    }
    if run.legendary.has(86) && original.key == "lightning_bolt"
        || run.legendary.has(261)
            && tags.contains(Tags::SPELL | Tags::DAMAGE)
            && events::direct(original.kind)
    {
        *tags = Tags((tags.0 & !Tags::SPELL.0) | Tags::ATTACK.0);
    }
    if run.legendary.has(262) && tags.contains(Tags::MELEE | Tags::ATTACK) {
        *tags = Tags((tags.0 & !(Tags::ATTACK.0 | super::SCHOOLS)) | Tags::SPELL.0 | Tags::HOLY.0);
    }
    if run.legendary.has(269) && !original.key.starts_with("proc_") {
        *tags = Tags(
            (tags.0 & !(Tags::ATTACK.0 | Tags::SPELL.0))
                | if state.counts.values().sum::<u64>() % 2 == 0 {
                    Tags::ATTACK.0
                } else {
                    Tags::SPELL.0
                },
        );
    }
    if run.legendary.has(172) && tags.contains(Tags::MELEE | Tags::DAMAGE) {
        def.kind = [Kind::Melee, Kind::Cone, Kind::Nova][count as usize % 3];
        def.radius = 5.0;
        *tags = Tags((tags.0 & !FORMS) | shape_tags(def.kind).0 | Tags::MELEE.0);
    }
    if run.legendary.has(169) && tags.contains(Tags::MELEE | Tags::DAMAGE) {
        def.kind = Kind::Cone;
        def.radius = 5.0;
        *tags = *tags | Tags::AREA;
    }
    if run.legendary.has(306) && original.key == "hurricane" {
        def.kind = Kind::Chain;
        def.duration = 0.0;
        def.count = 4;
        *tags = Tags((tags.0 & !(FORMS | Tags::DURATION.0)) | Tags::CHAIN.0);
    }
    if run.legendary.has(310) && original.key == "death_coil" {
        def.kind = Kind::Nova;
        def.radius = 7.0;
        *tags = Tags((tags.0 & !FORMS) | Tags::AREA.0);
    }
    if run.legendary.has(88) && original.key == "proc_starfall" {
        def.kind = Kind::Bolt;
        def.count = 3;
        def.radius = 0.0;
        *tags = Tags((tags.0 & !FORMS) | Tags::PROJECTILE.0);
    }
    if run.legendary.has(303) && original.key == "entangling_roots" {
        def.kind = Kind::Ground;
        def.radius = 5.0;
        def.damage = 0.0;
        *tags = *tags | Tags::AREA | Tags::DOT;
    }
    if run.legendary.has(265) && original.kind == Kind::Dot {
        def.kind = Kind::Ground;
        def.radius = 5.0;
        def.damage = 0.0;
        *tags = *tags | Tags::AREA | Tags::DOT;
    }
    if (run.legendary.has(288) || run.legendary.has(300)) && events::direct(original.kind) {
        if let Some((a, b)) = state.pair {
            if original.key == a || original.key == b {
                let other = data::ability(if original.key == a { b } else { a });
                if run.legendary.has(300) || count % 2 == 1 {
                    shape_from(def, tags, other);
                }
                if run.legendary.has(300) {
                    if let Some(school) = spell_engine::tags::school(other.key) {
                        *tags = Tags((tags.0 & !super::SCHOOLS) | school.0);
                    }
                }
            }
        }
    }
    if run.legendary.has(27)
        && !original.key.starts_with("proc_")
        && tags.intersects(Tags::FIRE | Tags::COLD | Tags::LIGHTNING | Tags::NATURE)
    {
        let school =
            [Tags::FIRE, Tags::COLD, Tags::LIGHTNING | Tags::NATURE][state.elemental as usize % 3];
        *tags = Tags((tags.0 & !super::SCHOOLS) | school.0);
    }
    if run.legendary.has(141) && tags.contains(Tags::DAMAGE) {
        let opposite = state.eclipse
            ^ (!original.key.starts_with("proc_")
                && state.last.is_some_and(|key| key != original.key));
        *tags = Tags(
            (tags.0 & !super::SCHOOLS)
                | if opposite {
                    Tags::SHADOW.0
                } else {
                    Tags::FIRE.0
                },
        );
    }
    if run.legendary.has(148) && tags.contains(Tags::COLD) {
        *tags = Tags((tags.0 & !super::SCHOOLS) | Tags::PHYSICAL.0);
        if matches!(def.effect, Effect::Root(_)) {
            def.effect = Effect::None;
        }
    }
    if run.legendary.has(70) && tags.contains(Tags::DAMAGE) && original.damage > 0.0 {
        *tags = Tags((tags.0 & !super::SCHOOLS) | (Tags::SHADOW | Tags::DOT | Tags::DURATION).0);
        if matches!(def.effect, Effect::Execute(_)) {
            def.effect = Effect::None;
        }
    }
    if tags.contains(Tags::DAMAGE) {
        let school = if run.legendary.has(94) {
            Some(Tags::FIRE)
        } else if run.legendary.has(95) {
            Some(Tags::COLD)
        } else if run.legendary.has(97) {
            Some(Tags::SHADOW)
        } else {
            None
        };
        if let Some(school) = school {
            *tags = Tags((tags.0 & !super::SCHOOLS) | school.0);
        }
    }
}
pub(in crate::survivors) fn after_cast(
    run: &mut Run,
    def: &AbilityDef,
    p: &Resolved,
    primary: Option<(Entity, Vec3)>,
    repeated: bool,
) {
    if repeated {
        return;
    }
    if run.legendary.has(281) && matches!(def.kind, Kind::Nova | Kind::Cone) {
        let designated = run
            .abilities
            .iter()
            .filter(|o| matches!(o.def.kind, Kind::Nova | Kind::Cone))
            .min_by(|a, b| {
                run.legendary
                    .casting
                    .last_used
                    .get(a.def.key)
                    .copied()
                    .unwrap_or(-1.0)
                    .total_cmp(
                        &run.legendary
                            .casting
                            .last_used
                            .get(b.def.key)
                            .copied()
                            .unwrap_or(-1.0),
                    )
            })
            .map(|o| o.def.key);
        if designated == Some(def.key) {
            let centers: Vec<_> = run
                .legendary
                .casting
                .ground
                .iter()
                .filter(|(_, until)| *until > run.elapsed)
                .take(3)
                .map(|p| p.0)
                .collect();
            for center in centers {
                plan(
                    run,
                    281,
                    def,
                    *p,
                    center,
                    None,
                    None,
                    0.0,
                    Origin::Legendary,
                    Vec::new(),
                );
            }
        }
    }
    if run.legendary.has(61) && def.key == "sinister_strike" {
        if let Some((_, pos)) = primary {
            let toward = flat(pos - run.legendary.events.position).normalize_or(Vec3::NEG_Z);
            let side = if run
                .legendary
                .casting
                .counts
                .get(def.key)
                .copied()
                .unwrap_or(0)
                % 2
                == 0
            {
                -1.0
            } else {
                1.0
            };
            let left = Vec3::new(-toward.z, 0.0, toward.x);
            run.legendary.moves.push(super::movement::Move {
                cast: p.cast_id,
                point: pos + left * side * 1.5,
            });
        }
    }
    if run.legendary.has(269)
        && run
            .banner
            .as_ref()
            .is_none_or(|(text, _)| text.starts_with("Discipline:"))
    {
        let attack = (run.legendary.casting.counts.values().sum::<u64>() + 1) % 2 == 0;
        run.banner = Some((
            format!(
                "Discipline: next cast {}",
                if attack { "Attack" } else { "Spell" }
            ),
            1.0,
        ));
    }
    if p.tags.contains(Tags::MELEE) {
        run.legendary
            .hands
            .insert(p.cast_id, run.legendary.casting.melee as usize % 2);
        run.legendary.casting.melee += 1;
    }
    if p.tags.contains(Tags::AREA) {
        run.legendary.effects.clear_area_mark();
    }
    if run.legendary.has(141) && run.legendary.casting.last.is_some_and(|key| key != def.key) {
        run.legendary.casting.eclipse = !run.legendary.casting.eclipse;
    }
    run.legendary.casting.previous = run.legendary.casting.last;
    run.legendary.casting.last = Some(def.key);
    run.legendary.casting.last_school = Tags(p.tags.0 & super::SCHOOLS);
    *run.legendary.casting.counts.entry(def.key).or_default() += 1;
    run.legendary.casting.last_used.insert(def.key, run.elapsed);
    if spell_engine::tags::for_spell(data::ability(def.key))
        .intersects(Tags::FIRE | Tags::COLD | Tags::LIGHTNING | Tags::NATURE)
    {
        run.legendary.casting.elemental += 1;
    }
    if p.tags.contains(Tags::DAMAGE) && events::direct(def.kind) {
        if let Some(previous) = run
            .legendary
            .casting
            .last_direct
            .filter(|key| *key != def.key)
        {
            run.legendary.casting.prior_direct.insert(def.key, previous);
        }
        run.legendary.casting.last_direct = Some(def.key);
        if p.tags.contains(Tags::MELEE) {
            run.legendary.casting.last_melee = Some((def.key, *p));
        } else {
            run.legendary.casting.last_ranged = Some((def.key, *p));
        }
    }
    if run.legendary.has(283) && p.tags.contains(Tags::DAMAGE) && events::direct(def.kind) {
        let sequence = &mut run.legendary.casting.sequence;
        if sequence.len() == 3 && sequence[0] == def.key {
            let copies = [sequence[1], sequence[2]];
            sequence.clear();
            for key in copies {
                copy(run, 283, key, run.legendary.events.position, None, 0.18);
            }
        } else if !sequence.contains(&def.key) {
            if sequence.len() >= 3 {
                sequence.clear();
            }
            sequence.push(def.key);
        }
    }
    if run.legendary.has(134) && p.tags.contains(Tags::AREA) {
        if let Some((_, pos)) = primary {
            if let Some(previous) = run.legendary.casting.last_area.insert(def.key, pos) {
                let direction = flat(pos - run.legendary.events.position).normalize_or(Vec3::NEG_Z);
                plan(
                    run,
                    134,
                    def,
                    *p,
                    previous + direction * 3.0,
                    None,
                    None,
                    0.18,
                    Origin::Legendary,
                    Vec::new(),
                );
            }
        }
    }
}
pub(in crate::survivors) fn plan(
    run: &mut Run,
    unique: u16,
    def: &AbilityDef,
    mut p: Resolved,
    center: Vec3,
    direction: Option<Vec3>,
    primary: Option<Entity>,
    delay: f32,
    origin: Origin,
    exclude: Vec<Entity>,
) {
    if run.legendary.casting.plans.len() >= 256 {
        return;
    }
    if origin != Origin::Skill {
        if !crate::survivors::procs::ready_effect(run, "legendary_copy", Some(unique), p.tags) {
            return;
        }
        if p.cast_id == 0 {
            p.cast_id = if run.proc_runtime.parent == 0 {
                crate::survivors::procs::fresh(run)
            } else {
                run.proc_runtime.parent
            };
        }
    }
    let splash = if matches!(def.kind, Kind::Bolt | Kind::Strike) {
        p.radius
    } else {
        0.0
    };
    if p.radius <= 0.0 {
        p.radius = 4.0;
    }
    run.legendary.casting.plans.push(Plan {
        splash,
        unique,
        skill: def.key,
        params: p,
        effect: p.effect,
        at: run.elapsed + delay,
        until: run.elapsed + delay,
        every: f32::INFINITY,
        center,
        direction,
        guided: false,
        primary,
        exclude,
        origin,
        first: true,
    });
}
pub(in crate::survivors) fn copy(
    run: &mut Run,
    unique: u16,
    key: &'static str,
    center: Vec3,
    target: Option<Entity>,
    delay: f32,
) {
    let def = data::ability(key);
    let level = run
        .abilities
        .iter()
        .find(|o| o.def.key == key)
        .map_or(1, |o| o.level);
    let mut p = run.resolve_spell(def, level);
    p.cast_id = 0;
    let direction = if def.kind == Kind::Cone {
        Some(
            target
                .and_then(|e| {
                    run.legendary
                        .periodic
                        .foes
                        .iter()
                        .find(|(id, _)| *id == e)
                        .map(|(_, pos)| flat(*pos - center).normalize_or(Vec3::NEG_Z))
                })
                .unwrap_or(run.legendary.casting.last_heading.normalize_or(Vec3::NEG_Z)),
        )
    } else {
        None
    };
    plan(
        run,
        unique,
        def,
        p,
        center,
        direction,
        target,
        delay,
        Origin::Legendary,
        Vec::new(),
    );
    if let Some(plan) = run.legendary.casting.plans.last_mut() {
        plan.guided = matches!(
            def.kind,
            Kind::Bolt | Kind::Strike | Kind::Melee | Kind::Chain
        );
    }
}
/// Returns `None` for native targeting, otherwise the successful activation and its primary.
pub(in crate::survivors) fn intercept(
    run: &mut Run,
    def: &AbilityDef,
    p: Resolved,
    foes: &[Target],
    hpos: Vec3,
    repeated: bool,
) -> Option<(bool, Option<(Entity, Vec3)>)> {
    if run.legendary.has(188) && p.tags.contains(Tags::PROJECTILE) && events::direct(def.kind) {
        let from_pet = run
            .legendary
            .casting
            .counts
            .iter()
            .filter(|(key, _)| {
                spell_engine::tags::for_spell(data::ability(key)).contains(Tags::PROJECTILE)
            })
            .map(|(_, n)| *n)
            .sum::<u64>()
            % 2
            == 1;
        let source = run
            .legendary
            .casting
            .pet
            .filter(|_| from_pet)
            .unwrap_or(hpos);
        if let Some(first) = foes
            .iter()
            .filter(|f| flat(f.pos - source).length() <= p.range)
            .min_by(|a, b| {
                flat(a.pos - source)
                    .length_squared()
                    .total_cmp(&flat(b.pos - source).length_squared())
            })
        {
            let primary = Some((first.e, first.pos));
            if let Some(pet) = run.legendary.casting.pet.filter(|_| from_pet) {
                plan(
                    run,
                    188,
                    def,
                    p,
                    pet,
                    None,
                    Some(first.e),
                    0.0,
                    if repeated {
                        Origin::Legendary
                    } else {
                        Origin::Skill
                    },
                    Vec::new(),
                );
                run.legendary.casting.plans.last_mut().unwrap().guided = true;
                run.legendary.casting.spirit_destination =
                    Some(first.pos + flat(first.pos - hpos).normalize_or(Vec3::NEG_Z) * 2.0);
                return Some((true, primary));
            }
            run.legendary.casting.spirit_destination =
                Some(first.pos + flat(first.pos - hpos).normalize_or(Vec3::NEG_Z) * 2.0);
        }
    }
    if repeated && run.legendary.has(293) && events::direct(def.kind) {
        if let Some(previous) = run.legendary.casting.prior_direct.get(def.key).copied() {
            copy(run, 293, previous, hpos, None, 0.0);
            return Some((true, None));
        }
    }
    if run.legendary.has(71) && def.key == "holy_nova" {
        let centers: Vec<_> = if run.legendary.effects.allies.len() > 1 {
            run.legendary
                .effects
                .allies
                .iter()
                .copied()
                .filter(|p| flat(*p - hpos).length() <= 24.0)
                .collect()
        } else {
            foes.iter()
                .find(|f| f.dist <= 24.0)
                .map(|f| vec![f.pos])
                .unwrap_or_default()
        };
        let primary = foes
            .iter()
            .filter(|f| {
                centers
                    .iter()
                    .any(|center| flat(f.pos - *center).length() <= p.radius)
            })
            .max_by(|a, b| a.hp.total_cmp(&b.hp))
            .map(|f| (f.e, f.pos));
        if primary.is_none() {
            return Some((false, None));
        }
        for center in centers {
            plan(
                run,
                71,
                def,
                p,
                center,
                None,
                None,
                0.0,
                if repeated {
                    Origin::Legendary
                } else {
                    Origin::Skill
                },
                Vec::new(),
            );
        }
        return Some((true, primary));
    }
    let first = foes.iter().find(|f| f.dist <= p.range.max(p.radius));
    let center = if let Some(center) = super::effects::area_center(run) {
        center
    } else if matches!(def.kind, Kind::Nova | Kind::Cone) {
        hpos
    } else {
        first.map_or(hpos, |f| f.pos)
    };
    let primary = first.map(|f| (f.e, f.pos));
    let direction = first.map_or(Vec3::NEG_Z, |f| {
        flat(f.pos - hpos).normalize_or(Vec3::NEG_Z)
    });
    let origin = if repeated {
        Origin::Legendary
    } else {
        Origin::Skill
    };
    let instant_area = matches!(def.kind, Kind::Nova | Kind::Cone)
        || def.kind == Kind::Ground && def.duration <= 0.0;
    let chosen = if run.legendary.has(175)
        && def.kind == Kind::Cone
        && p.tags.contains(Tags::MELEE)
        && run.legendary.casting.melee % 2 == 1
    {
        Some(175)
    } else if run.legendary.has(42) && def.key == "thunder_clap" {
        Some(42)
    } else if run.legendary.has(3) && instant_area {
        Some(3)
    } else if run.legendary.has(124) && instant_area && def.kind != Kind::Cone {
        Some(124)
    } else if run.legendary.has(126) && def.kind == Kind::Cone {
        Some(126)
    } else if run.legendary.has(98) && instant_area {
        Some(98)
    } else if run.legendary.has(115) && p.tags.contains(Tags::PROJECTILE) {
        Some(115)
    } else if run.legendary.has(263) && p.tags.contains(Tags::PROJECTILE) {
        Some(263)
    } else if run.legendary.has(167) && p.tags.contains(Tags::MELEE) {
        Some(167)
    } else if run.legendary.has(169) && p.tags.contains(Tags::MELEE) {
        Some(169)
    } else if run.legendary.has(264)
        && run
            .abilities
            .iter()
            .find(|o| events::direct(o.def.kind) && o.def.damage > 0.0)
            .is_some_and(|o| o.def.key == def.key)
    {
        Some(264)
    } else {
        None
    };
    let id = chosen?;
    if first.is_none() && chosen != Some(264) {
        return Some((false, None));
    }
    match id {
        175 => {
            let point = first.unwrap().pos - direction * 1.5;
            run.legendary.moves.push(super::movement::Move {
                cast: p.cast_id,
                point,
            });
            plan(
                run,
                175,
                def,
                p,
                hpos,
                Some(direction),
                None,
                0.0,
                origin,
                Vec::new(),
            );
        }
        3 | 42 => {
            let step = p.radius.max(3.0) * 0.8;
            for index in 0..3 {
                let point = if id == 42 {
                    hpos + direction * step * (index + 1) as f32
                } else {
                    center + direction * step * (index as f32 - 1.0)
                };
                plan(
                    run,
                    id,
                    def,
                    p,
                    point,
                    None,
                    None,
                    index as f32 * 0.18,
                    origin,
                    Vec::new(),
                );
            }
        }
        124 => {
            let angle = *run.legendary.casting.counts.get(def.key).unwrap_or(&0) as f32
                * std::f32::consts::FRAC_PI_4;
            let mut small = p;
            small.radius *= 0.35;
            for index in 0..4 {
                let a = angle + index as f32 * std::f32::consts::FRAC_PI_2;
                plan(
                    run,
                    id,
                    def,
                    small,
                    center + Vec3::new(a.cos(), 0.0, a.sin()) * p.radius * 0.75,
                    None,
                    None,
                    0.0,
                    origin,
                    Vec::new(),
                );
            }
        }
        126 => {
            for side in [-1.0, 1.0] {
                let left = Vec3::new(-direction.z, 0.0, direction.x);
                plan(
                    run,
                    id,
                    def,
                    p,
                    first.unwrap().pos + left * side * 2.0,
                    Some(direction),
                    None,
                    0.0,
                    origin,
                    Vec::new(),
                );
            }
        }
        98 => {
            if let Some(mut previous) = run.legendary.casting.impressions.remove(def.key) {
                previous.at = run.elapsed;
                previous.until = run.elapsed;
                run.legendary.casting.plans.push(previous);
            }
            let imprint = Plan {
                splash: 0.0,
                unique: id,
                skill: def.key,
                params: p,
                effect: p.effect,
                at: run.elapsed + 6.0,
                until: run.elapsed + 6.0,
                every: f32::INFINITY,
                center,
                direction: None,
                guided: false,
                primary: None,
                exclude: vec![],
                origin,
                first: true,
            };
            run.legendary.casting.impressions.insert(def.key, imprint);
        }
        115 => {
            for (i, f) in targeting::nearest(foes, p.range, p.count)
                .iter()
                .enumerate()
            {
                let mut params = p;
                if i > 0 {
                    params.cast_id = 0;
                    params.primary_critical = false;
                }
                plan(
                    run,
                    id,
                    def,
                    params,
                    f.pos,
                    None,
                    None,
                    0.4,
                    origin,
                    Vec::new(),
                );
            }
        }
        263 => {
            plan(run, id, def, p, hpos, None, None, 0.6, origin, Vec::new());
            if let Some(plan) = run.legendary.casting.plans.last_mut() {
                plan.guided = true;
            }
        }
        167 => {
            let targets = targeting::nearest(foes, p.range + 0.5, p.count);
            let exclude = targets.iter().map(|f| f.e).collect();
            plan(
                run,
                id,
                def,
                p,
                first.unwrap().pos,
                None,
                Some(first.unwrap().e),
                0.0,
                origin,
                Vec::new(),
            );
            if let Some(plan) = run.legendary.casting.plans.last_mut() {
                plan.guided = true;
            }
            plan(
                run,
                id,
                def,
                p,
                first.unwrap().pos,
                None,
                None,
                0.3,
                Origin::Legendary,
                exclude,
            );
        }
        169 => {
            let side = if run.legendary.casting.melee % 2 == 0 {
                -1.0
            } else {
                1.0
            };
            run.legendary
                .casting
                .sides
                .insert(p.cast_id, if side < 0.0 { -1 } else { 1 });
            let left = Vec3::new(-direction.z, 0.0, direction.x) * side;
            plan(
                run,
                id,
                def,
                p,
                hpos,
                Some(left),
                None,
                0.0,
                origin,
                Vec::new(),
            );
        }
        264 => {
            let old = run.legendary.casting.anchor;
            let anchor = if old.is_none_or(|pos| {
                flat(pos - hpos).length() > p.range * 2.0
                    || !foes.iter().any(|f| flat(f.pos - pos).length() <= p.range)
            }) {
                hpos + Vec3::X * 2.0
            } else {
                old.unwrap()
            };
            run.legendary.casting.anchor = Some(anchor);
            plan(run, id, def, p, anchor, None, None, 0.0, origin, Vec::new());
            if let Some(plan) = run.legendary.casting.plans.last_mut() {
                plan.guided = true;
            }
        }
        _ => unreachable!(),
    }
    Some((true, primary))
}
pub(in crate::survivors) fn equip(run: &mut Run) {
    let pair: Vec<_> = run
        .abilities
        .iter()
        .map(|o| o.def)
        .chain(run.class.pool.iter().map(|key| data::ability(key)))
        .filter(|d| events::direct(d.kind) && d.damage > 0.0)
        .map(|d| d.key)
        .collect();
    if run.legendary.casting.pair.is_none() {
        if let Some(a) = pair.first() {
            if let Some(b) = pair.iter().find(|b| *b != a) {
                run.legendary.casting.pair = Some((*a, *b));
            }
        }
    }
    let worn: BTreeSet<_> = run.legendary.equipped.keys().copied().collect();
    run.legendary
        .casting
        .plans
        .retain(|p| worn.contains(&p.unique));
    run.legendary
        .casting
        .impressions
        .retain(|_, p| worn.contains(&p.unique));
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spirit_projectiles_search_from_the_pet_even_outside_the_owners_range() {
        let mut run = Run::default();
        run.legendary.equipped.insert(
            188,
            Roll {
                unique: 188,
                milestone: None,
            },
        );
        run.legendary.casting.counts.insert("fireball", 1);
        run.legendary.casting.pet = Some(Vec3::X * 90.0);
        let foes = [Target {
            e: Entity::from_bits(1),
            pos: Vec3::X * 92.0,
            hp: 100.0,
            dist: 92.0,
            radius: 0.0,
        }];
        let p = run.resolve_spell(data::ability("fireball"), 1);
        assert!(p.range < 92.0);
        assert!(
            intercept(
                &mut run,
                data::ability("fireball"),
                p,
                &foes,
                Vec3::ZERO,
                false
            )
            .unwrap()
            .0
        );
        assert_eq!(run.legendary.casting.plans[0].center, Vec3::X * 90.0);
    }
}
pub(in crate::survivors) fn tick(
    mut commands: Commands,
    mut run: ResMut<Run>,
    mut players: Option<ResMut<crate::survivors::multiplayer::Companions>>,
    enemies: Query<(Entity, &Transform, &Enemy)>,
    mut hits: MessageWriter<Hit>,
    mut rng: ResMut<Rng>,
    mut fx: Fx,
    mut casters: Query<(Entity, &mut Transform, &Caster), Without<Enemy>>,
) {
    let now = run.elapsed;
    let mut due = Vec::new();
    for (entity, mut tf, caster) in &mut casters {
        if let Some(owner) =
            crate::survivors::multiplayer::player_run(&mut run, players.as_deref_mut(), caster.0)
        {
            if owner.legendary.has(264) {
                if let Some(pos) = owner.legendary.casting.anchor {
                    tf.translation = pos;
                }
            } else {
                commands.entity(entity).despawn();
            }
        }
    }
    let mut spawn = |owner: &mut Run| {
        if owner.legendary.has(264) && !casters.iter().any(|(_, _, c)| c.0 == owner.owner) {
            if let Some(pos) = owner.legendary.casting.anchor {
                let serial = owner.guid();
                let e = crate::survivors::units::spawn_unit(
                    &mut commands,
                    &crate::survivors::units::UnitSpec {
                        serial,
                        display: 4587,
                        scale: 1.0,
                        level: 60,
                        faction: crate::survivors::units::FACTION_FRIENDLY,
                        bytes0: 0,
                        weapons: &[],
                        sheath: 0,
                    },
                    pos,
                    0.0,
                );
                commands.entity(e).insert(Caster(owner.owner));
                if let Some(id) = owner.owner {
                    commands
                        .entity(e)
                        .insert(crate::survivors::multiplayer::OwnedEffect(id));
                }
            }
        }
    };
    spawn(&mut run);
    if let Some(players) = players.as_deref_mut() {
        for p in players.0.values_mut() {
            spawn(&mut p.run);
        }
    }
    let mut collect = |run: &mut Run| {
        let expired: Vec<_> = run
            .legendary
            .casting
            .impressions
            .iter()
            .filter(|(_, p)| p.at <= now)
            .map(|(k, _)| *k)
            .collect();
        for key in expired {
            if let Some(p) = run.legendary.casting.impressions.remove(key) {
                run.legendary.casting.plans.push(p);
            }
        }
        let mut pending = Vec::new();
        for mut plan in std::mem::take(&mut run.legendary.casting.plans) {
            if !run.legendary.has(plan.unique) || run.dead_t.is_some() {
                continue;
            }
            if plan.at <= now {
                due.push((run.owner, plan.clone()));
                plan.first = false;
                plan.at += plan.every;
                if plan.at <= plan.until {
                    pending.push(plan);
                }
            } else {
                pending.push(plan);
            }
        }
        run.legendary.casting.plans = pending;
    };
    collect(&mut run);
    if let Some(players) = players.as_deref_mut() {
        for player in players.0.values_mut() {
            collect(&mut player.run);
        }
    }
    let mut covered = BTreeSet::new();
    for (owner, plan) in due {
        let mut foes: Vec<_> = enemies
            .iter()
            .filter(|(e, _, en)| !en.dead && !plan.exclude.contains(e))
            .map(|(e, tf, en)| Target {
                e,
                pos: tf.translation,
                hp: en.hp,
                dist: flat(tf.translation - plan.center).length(),
                radius: en.radius,
            })
            .collect();
        foes.sort_by(|a, b| {
            a.dist
                .total_cmp(&b.dist)
                .then_with(|| a.e.to_bits().cmp(&b.e.to_bits()))
        });
        let mut targets = if plan.guided {
            if let Some(primary) = plan.primary {
                foes.sort_by_key(|f| f.e != primary);
            }
            foes.into_iter()
                .filter(|f| f.dist <= plan.params.range)
                .take(plan.params.count)
                .collect()
        } else {
            targeting::around(&foes, plan.center, plan.params.radius)
        };
        if let Some(primary) = plan.primary {
            if plan.guided && !targets.iter().any(|f| f.e == primary) {
                targets.clear();
            }
        }
        if let Some(direction) = plan.direction {
            targets.retain(|f| {
                let d = flat(f.pos - plan.center);
                d.length() < 0.5 || d.normalize_or_zero().dot(direction) >= 0.8
            });
        }
        let primary = targets
            .iter()
            .max_by(|a, b| a.hp.total_cmp(&b.hp))
            .map(|f| f.e);
        for f in targets {
            if plan.unique == 71 && !covered.insert((owner, plan.params.cast_id, f.e)) {
                continue;
            }
            let p = plan.params;
            let (damage, crit) = spell_engine::critical_with_multiplier(
                p.dmg,
                if p.primary_critical && Some(f.e) == primary {
                    1.0
                } else {
                    p.crit
                },
                p.crit_multiplier,
                rng.f32(),
            );
            let mut hit = abilities::hit_for(data::ability(plan.skill), &p, f.e, damage, crit);
            hit.owner = owner;
            hit.item_source = Some(plan.unique);
            if plan.origin != Origin::Skill {
                hit.meter_source = super::definition(plan.unique).map(|d| d.name);
            }
            hit.origin = plan.origin;
            hit.primary = plan.first && Some(f.e) == primary;
            hit.cast_id = if plan.first { p.cast_id } else { 0 };
            hit.effect = if plan.unique == 303 && !plan.first {
                Effect::None
            } else {
                plan.effect
            };
            if plan.origin != Origin::Skill {
                hit.triggered = true;
                hit.proc_admitted = true;
                hit.proc_resolved = true;
                hit.proc_chain = u64::MAX;
            }
            hit.splash = if plan.guided { plan.splash } else { 0.0 };
            fx.impact(f.e, data::ability(plan.skill).spell);
            hits.write(hit);
        }
    }
}
