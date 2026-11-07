//! Periodic families and finite tick reserves belong to an instance and its owner.
use super::events::{self, Origin, Shape};
use super::*;
use crate::survivors::{
    abilities::Hit,
    enemies::{Dot, Enemy},
    units::flat,
};
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::survivors) enum Family {
    #[default]
    Unknown,
    Bleed,
    Poison,
    Burn,
    Disease,
    Curse,
    Other,
}
#[derive(Clone, Default)]
pub(in crate::survivors) struct Meta {
    pub parent: u64,
    pub family: Family,
    pub held: f32,
    pub nails: u8,
    pub transferred: bool,
    pub no_reserve: bool,
    pub last_tick: f32,
    pub started: f32,
    pub frostburn: bool,
    pub previous: Option<Vec3>,
    pub moving: bool,
    pub frozen_until: f32,
    pub cycle: Vec<Tags>,
    pub cycle_index: usize,
}
#[derive(Default)]
pub(in crate::survivors) struct State {
    pub fields: Vec<Field>,
    pub foes: Vec<(Entity, Vec3)>,
    pub reserve: Vec<Dot>,
    pub reserve_until: f32,
    pub poison_target: Option<Entity>,
    pub transfers: Vec<(Entity, Dot)>,
    pub swaps: Vec<(Entity, Entity)>,
    pub last_turn: f32,
    pub heading: Vec3,
    pub turn: bool,
    pub affected: BTreeMap<Entity, Vec<&'static str>>,
}
pub(in crate::survivors) struct Field {
    entity: Option<Entity>,
    pub unique: u16,
    pub pos: Vec3,
    pub damage: f32,
    pub school: Tags,
    pub left: u32,
    pub at: f32,
    pub radius: f32,
    pub targeted: bool,
}
pub(in crate::survivors) fn field(
    run: &mut Run,
    unique: u16,
    pos: Vec3,
    total: f32,
    duration: f32,
    school: Tags,
    targeted: bool,
) {
    let ticks = duration.floor().max(1.0) as u32;
    run.legendary.periodic.fields.push(Field {
        entity: None,
        unique,
        pos,
        damage: total / ticks as f32,
        school,
        left: ticks,
        at: run.elapsed + 1.0,
        radius: 5.0,
        targeted,
    });
}
pub(in crate::survivors) fn family(dot: &Dot) -> Family {
    if dot.meta.family != Family::Unknown {
        return dot.meta.family;
    }
    match dot.skill {
        "rend" | "rupture" | "hemorrhage" | "proc_bleed" | "garrote" | "rake" | "rip" => {
            Family::Bleed
        }
        "proc_poison" | "deadly_poison" => Family::Poison,
        "proc_plague" | "devouring_plague" => Family::Disease,
        "curse_of_agony" => Family::Curse,
        _ if dot.tags.is_some_and(|t| t.contains(Tags::FIRE)) => Family::Burn,
        _ => Family::Other,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn dot() -> Dot {
        Dot {
            owner: None,
            skill: "rend",
            meter_source: None,
            tags: Some(Tags::PHYSICAL | Tags::DOT | Tags::DAMAGE),
            origin: Origin::Skill,
            item_source: None,
            meta: default(),
            dps: 10.0,
            left: 4.0,
            tick: 1.0,
        }
    }
    fn owner(id: u16) -> Run {
        let mut run = Run::default();
        run.legendary.equipped.insert(
            id,
            Roll {
                unique: id,
                milestone: None,
            },
        );
        run
    }
    #[test]
    fn finite_budget_counts_only_future_ticks_and_held_damage() {
        let mut d = dot();
        assert_eq!(budget(&d), 40.0);
        d.left = 2.4;
        d.tick = 0.4;
        d.meta.held = 7.0;
        assert_eq!(budget(&d), 37.0);
        d.left = 0.1;
        d.tick = 0.9;
        assert_eq!(budget(&d), 7.0);
    }
    #[test]
    fn stationary_reserve_releases_once_after_movement() {
        let mut run = owner(64);
        let mut d = dot();
        let e = Entity::from_bits(1);
        assert_eq!(tick(&mut run, e, &mut d, Vec3::ZERO, false, false), 0.0);
        assert_eq!(d.meta.held, 10.0);
        run.legendary.events.moving = true;
        assert_eq!(tick(&mut run, e, &mut d, Vec3::ZERO, false, false), 20.0);
        assert_eq!(tick(&mut run, e, &mut d, Vec3::ZERO, false, false), 10.0);
    }
    #[test]
    fn fixed_field_preserves_consumed_budget() {
        let mut run = owner(69);
        field(&mut run, 69, Vec3::X, 125.0, 4.0, Tags::PHYSICAL, false);
        let f = &run.legendary.periodic.fields[0];
        assert_eq!(f.damage * f.left as f32, 125.0);
        assert_eq!(f.pos, Vec3::X);
    }
    #[test]
    fn fever_changes_school_each_tick_without_changing_budget() {
        let mut run = owner(239);
        let mut d = dot();
        d.meta.cycle = vec![Tags::FIRE, Tags::COLD, Tags::SHADOW];
        for school in [Tags::FIRE, Tags::COLD, Tags::SHADOW, Tags::FIRE] {
            assert_eq!(
                tick(
                    &mut run,
                    Entity::from_bits(1),
                    &mut d,
                    Vec3::ZERO,
                    false,
                    false
                ),
                10.0
            );
            assert!(d.tags.unwrap().contains(school));
        }
    }
}
pub(in crate::survivors) fn budget(dot: &Dot) -> f32 {
    let ticks = if dot.left + 0.0001 >= dot.tick {
        ((dot.left - dot.tick).max(0.0)).floor() + 1.0
    } else {
        0.0
    };
    dot.dps * ticks + dot.meta.held
}
pub(in crate::survivors) fn install(
    run: &mut Run,
    en: &mut Enemy,
    target: Entity,
    pos: Vec3,
    mut dot: Dot,
) {
    dot.meta.started = run.elapsed;
    if dot.meta.parent == 0 {
        dot.meta.parent = run.proc_runtime.parent;
    }
    dot.meta.family = family(&dot);
    if run.legendary.has(94) {
        dot.meta.family = Family::Burn;
    }
    if run.legendary.has(63) && dot.meta.family == Family::Poison {
        let tags = dot.tags.unwrap_or(Tags::SPELL | Tags::DAMAGE | Tags::DOT);
        dot.tags = Some(Tags((tags.0 & !super::SCHOOLS) | Tags::COLD.0));
        en.slow = en.slow.min(if en.boss { 0.75 } else { 0.6 });
        en.slow_t = en.slow_t.max(dot.left);
    }
    if run.legendary.has(226) && dot.meta.family == Family::Poison {
        let chosen = *run.legendary.periodic.poison_target.get_or_insert(target);
        if chosen != target {
            run.legendary.periodic.transfers.push((chosen, dot));
            return;
        }
    }
    if run.legendary.has(230) {
        if let Some(existing) = en
            .dots
            .iter_mut()
            .find(|d| d.owner == dot.owner && d.skill == dot.skill)
        {
            existing.meta.nails = (existing.meta.nails + 1).min(5);
            return;
        }
    }
    if run.legendary.has(222) && dot.meta.family == Family::Bleed {
        let own: Vec<_> = en
            .dots
            .iter()
            .enumerate()
            .filter(|(_, d)| d.owner == dot.owner && family(d) == Family::Bleed)
            .map(|(i, _)| i)
            .collect();
        if own.len() >= 2 {
            en.dots.remove(own[0]);
        }
    }
    if run.legendary.has(240) && dot.meta.family == Family::Bleed {
        let def = crate::survivors::data::ability(dot.skill);
        let mut p = run.resolve_spell(def, 1);
        p.dmg = dot.dps;
        p.dot = 0.0;
        p.radius = 3.0;
        p.crit = 0.0;
        p.cast_id = 0;
        super::casting::plan(
            run,
            240,
            def,
            p,
            pos,
            None,
            None,
            1.0,
            Origin::Legendary,
            Vec::new(),
        );
        if let Some(plan) = run.legendary.casting.plans.last_mut() {
            plan.until = run.elapsed + dot.left;
            plan.every = 1.0;
        }
        return;
    }
    if dot.origin == Origin::Legendary && dot.meter_source == super::definition(323).map(|d| d.name)
    {
        en.dots
            .retain(|d| !(d.owner == dot.owner && d.meter_source == dot.meter_source));
    }
    if en.dots.len() >= 6 {
        en.dots.remove(0);
    }
    let other = en.dots.iter().any(|d| {
        d.owner == dot.owner
            && d.left > 0.0
            && events::school(d.tags.unwrap_or(Tags(0)))
                != events::school(dot.tags.unwrap_or(Tags(0)))
    });
    events::dot_applied(run, target, dot.tags.unwrap_or(Tags(0)), dot.origin, other);
    en.dots.push(dot);
}
fn neighbour(run: &Run, target: Entity, pos: Vec3) -> Option<Entity> {
    run.legendary
        .periodic
        .foes
        .iter()
        .filter(|(e, p)| *e != target && flat(*p - pos).length() <= 20.0)
        .min_by(|a, b| {
            let affected = |e: &Entity| {
                run.legendary
                    .periodic
                    .affected
                    .get(e)
                    .is_some_and(|keys| keys.contains(&"shadow_word_pain"))
            };
            affected(&a.0).cmp(&affected(&b.0)).then_with(|| {
                flat(a.1 - pos)
                    .length_squared()
                    .total_cmp(&flat(b.1 - pos).length_squared())
            })
        })
        .map(|f| f.0)
}
pub(in crate::survivors) fn tick(
    run: &mut Run,
    target: Entity,
    dot: &mut Dot,
    pos: Vec3,
    rooted: bool,
    boss: bool,
) -> f32 {
    let family = family(dot);
    if !dot.meta.cycle.is_empty() {
        let school = dot.meta.cycle[dot.meta.cycle_index % dot.meta.cycle.len()];
        dot.meta.cycle_index += 1;
        let tags = dot.tags.unwrap_or(Tags::SPELL | Tags::DOT | Tags::DAMAGE);
        dot.tags = Some(Tags((tags.0 & !super::SCHOOLS) | school.0));
    }
    let hold = (run.legendary.has(24) && family == Family::Burn)
        || (run.legendary.has(64) && !run.legendary.events.moving)
        || (run.legendary.has(221) && family == Family::Poison && dot.meta.moving)
        || (run.legendary.has(236)
            && family == Family::Bleed
            && rooted
            && (!boss || dot.meta.frozen_until == 0.0 || run.elapsed < dot.meta.frozen_until))
        || (run.legendary.has(302) && dot.skill == "moonfire");
    if run.legendary.has(229)
        && family == Family::Poison
        && !dot.meta.no_reserve
        && budget(dot) <= dot.meta.held
    {
        dot.meta.last_tick += dot.dps;
        return 0.0;
    }
    if hold {
        dot.meta.held += dot.dps;
        if run.legendary.has(236) && family == Family::Bleed && boss && dot.meta.frozen_until == 0.0
        {
            dot.meta.frozen_until = run.elapsed + 2.0;
        }
        return 0.0;
    }
    let damage = dot.dps + dot.meta.held;
    dot.meta.held = 0.0;
    if dot.origin == Origin::Skill && run.legendary.has(72) && dot.skill == "shadow_word_pain" {
        if let Some(next) = neighbour(run, target, pos) {
            let moved = dot.clone();
            run.legendary.periodic.transfers.push((next, moved));
            dot.left = 0.0;
            dot.meta.transferred = true;
        }
    }
    damage
}
pub(in crate::survivors) fn expired(run: &mut Run, target: Entity, dot: &Dot, pos: Vec3) {
    if dot.meta.transferred || dot.origin != Origin::Skill {
        return;
    }
    let fam = family(dot);
    let tags = dot.tags.unwrap_or(Tags::SHADOW);
    if run.legendary.has(236) && fam == Family::Bleed && dot.meta.held > 0.0 {
        events::queue(
            run,
            236,
            pos,
            None,
            Some(target),
            Shape::Guided(3, 20.0),
            dot.meta.held,
            tags,
            0.0,
        );
    }
    if run.legendary.has(24) && fam == Family::Burn && dot.meta.held > 0.0 {
        events::queue(
            run,
            24,
            pos,
            None,
            None,
            Shape::Circle(5.0),
            dot.meta.held,
            tags,
            0.0,
        );
    }
    if run.legendary.has(221) && fam == Family::Poison && dot.meta.held > 0.0 {
        events::queue(
            run,
            221,
            pos,
            Some(target),
            None,
            Shape::Guided(1, 0.0),
            dot.meta.held,
            tags,
            0.0,
        );
    }
    if run.legendary.has(229) && fam == Family::Poison && dot.meta.last_tick > 0.0 {
        let mut reserve = dot.clone();
        reserve.left = 1.0;
        reserve.tick = 1.0;
        reserve.dps = dot.meta.last_tick;
        reserve.meta.held = 0.0;
        reserve.meta.last_tick = 0.0;
        reserve.meta.no_reserve = true;
        if run.legendary.periodic.reserve.len() < 6 {
            run.legendary.periodic.reserve.push(reserve);
        }
        run.legendary.periodic.reserve_until = run.elapsed + 3.0;
    }
    if run.legendary.has(230) && dot.meta.nails > 0 {
        events::queue(
            run,
            230,
            pos,
            None,
            None,
            Shape::Circle(3.0),
            dot.dps * f32::from(dot.meta.nails),
            tags,
            0.0,
        );
    }
    if run.legendary.has(233) {
        if let Some(next) = neighbour(run, target, pos) {
            events::queue(
                run,
                233,
                pos,
                Some(next),
                None,
                Shape::Guided(1, 20.0),
                dot.dps,
                tags,
                0.0,
            );
        }
    }
}
pub(in crate::survivors) fn death(run: &mut Run, target: Entity, en: &Enemy, pos: Vec3) {
    let owned: Vec<_> = en
        .dots
        .iter()
        .filter(|d| d.owner == run.owner)
        .cloned()
        .collect();
    for dot in &owned {
        let fam = family(dot);
        if run.legendary.has(24) && fam == Family::Burn {
            events::queue(
                run,
                24,
                pos,
                None,
                None,
                Shape::Circle(5.0),
                budget(dot),
                dot.tags.unwrap_or(Tags::FIRE),
                0.0,
            );
        }
        if run.legendary.has(66) && fam == Family::Poison {
            if run.legendary.periodic.reserve.len() < 6 {
                run.legendary.periodic.reserve.push(dot.clone());
            }
            run.legendary.periodic.reserve_until = run.elapsed + 6.0;
        }
        if run.legendary.has(226) && fam == Family::Poison {
            if let Some(next) = neighbour(run, target, pos) {
                run.legendary.periodic.transfers.push((next, dot.clone()));
                run.legendary.periodic.poison_target = Some(next);
            }
        }
        if run.legendary.has(280) && dot.tags.is_some_and(|tags| tags.contains(Tags::HOLY)) {
            for (next, p) in run.legendary.periodic.foes.clone() {
                if next != target && flat(p - pos).length() <= 6.0 {
                    run.legendary.periodic.transfers.push((next, dot.clone()));
                }
            }
        }
    }
    if run.legendary.periodic.poison_target == Some(target) {
        run.legendary.periodic.poison_target = None;
    }
}
pub(in crate::survivors) fn before_hit(
    run: &mut Run,
    h: &Hit,
    en: &mut Enemy,
    pos: Vec3,
    tags: Tags,
) {
    if h.triggered || h.origin != Origin::Skill {
        return;
    }
    if run.legendary.periodic.reserve_until < run.elapsed {
        run.legendary.periodic.reserve.clear();
    }
    if h.damage > 0.0
        && (run.legendary.has(66) || run.legendary.has(229) && tags.contains(Tags::MELEE))
    {
        let reserve = std::mem::take(&mut run.legendary.periodic.reserve);
        for mut dot in reserve {
            dot.owner = h.owner;
            dot.meta.transferred = false;
            if en.dots.len() >= 6 {
                en.dots.remove(0);
            }
            en.dots.push(dot);
        }
    }
    if run.legendary.has(142) {
        for dot in &mut en.dots {
            if dot.owner != h.owner {
                continue;
            }
            if family(dot) == Family::Burn && tags.contains(Tags::COLD) {
                let old = dot.tags.unwrap_or(Tags::DOT | Tags::DAMAGE);
                dot.tags = Some(Tags((old.0 & !super::SCHOOLS) | Tags::COLD.0));
                dot.meta.frostburn = true;
                dot.meta.family = Family::Other;
            } else if dot.meta.frostburn && tags.contains(Tags::FIRE) {
                let old = dot.tags.unwrap_or(Tags::DOT | Tags::DAMAGE);
                dot.tags = Some(Tags((old.0 & !super::SCHOOLS) | Tags::FIRE.0));
                dot.meta.family = Family::Burn;
                dot.meta.frostburn = false;
            }
        }
    }
    let consume = if run.legendary.has(302) && h.skill == "wrath" {
        Some((302, None, Some("moonfire"), Tags::ARCANE))
    } else if run.legendary.has(143) && tags.contains(Tags::FIRE) {
        Some((143, Some(Family::Bleed), None, Tags::PHYSICAL))
    } else if run.legendary.has(222) && tags.contains(Tags::AREA) {
        Some((222, Some(Family::Bleed), None, Tags::PHYSICAL))
    } else if run.legendary.has(69) && h.crit && tags.contains(Tags::MELEE) {
        Some((69, Some(Family::Bleed), None, Tags::PHYSICAL))
    } else {
        None
    };
    if let Some((id, fam, key, school)) = consume {
        let mut damage = 0.0;
        en.dots.retain(|d| {
            if d.owner == h.owner
                && fam.is_none_or(|f| family(d) == f)
                && key.is_none_or(|k| d.skill == k)
            {
                damage += budget(d);
                false
            } else {
                true
            }
        });
        if damage > 0.0 {
            let shape = if id == 143 {
                Shape::Guided(3, 20.0)
            } else if id == 222 {
                Shape::Guided(1, 0.0)
            } else {
                Shape::Circle(5.0)
            };
            if id == 69 {
                field(run, id, pos, damage, 4.0, school, false);
            } else {
                events::queue(
                    run,
                    id,
                    pos,
                    if id == 222 { Some(h.target) } else { None },
                    if id == 143 { Some(h.target) } else { None },
                    shape,
                    damage,
                    school,
                    0.0,
                );
            }
        }
    }
    if run.legendary.has(307) && h.skill == "corruption" {
        let mut damage = 0.0;
        en.dots.retain(|d| {
            if d.owner == h.owner && d.skill == "immolate" {
                damage += budget(d);
                false
            } else {
                true
            }
        });
        for index in 0..3 {
            if damage > 0.0 {
                events::queue(
                    run,
                    307,
                    pos,
                    None,
                    None,
                    Shape::Circle(5.0),
                    damage / 3.0,
                    Tags::FIRE,
                    index as f32 * 0.25,
                );
            }
        }
    }
    if run.legendary.has(308) && h.skill == "shadow_bolt" {
        if let Some(next) = neighbour(run, h.target, pos) {
            if let Some(dot) = en
                .dots
                .iter_mut()
                .filter(|d| d.owner == h.owner && d.skill == "curse_of_agony" && budget(d) > 0.0)
                .min_by(|a, b| a.left.total_cmp(&b.left))
            {
                events::queue(
                    run,
                    308,
                    pos,
                    Some(next),
                    None,
                    Shape::Guided(1, 20.0),
                    dot.dps,
                    dot.tags.unwrap_or(Tags::SHADOW),
                    0.1,
                );
                dot.left = (dot.left - 1.0).max(0.0);
            }
        }
    }
}
pub(in crate::survivors) fn tick_world(
    mut commands: Commands,
    mut run: ResMut<Run>,
    mut players: Option<ResMut<crate::survivors::multiplayer::Companions>>,
    mut enemies: Query<(Entity, &Transform, &mut Enemy)>,
) {
    let foes: Vec<_> = enemies
        .iter()
        .filter(|(_, _, en)| !en.dead)
        .map(|(e, tf, _)| (e, tf.translation))
        .collect();
    let mut transfers = Vec::new();
    let mut swaps = Vec::new();
    let mut prepare = |run: &mut Run| {
        let mut fields = std::mem::take(&mut run.legendary.periodic.fields);
        for f in &mut fields {
            if f.entity.is_none() {
                let serial = run.guid();
                let entity = if f.targeted {
                    crate::survivors::units::spawn_unit(
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
                        f.pos,
                        0.0,
                    )
                } else {
                    crate::survivors::units::spawn_area_object(
                        &mut commands,
                        serial,
                        crate::survivors::data::ability("consecration").spell,
                        f.radius,
                        (f.radius
                            / crate::survivors::data::ability("consecration")
                                .radius
                                .max(0.001))
                        .clamp(0.25, 8.0),
                        f.pos,
                    )
                };
                if let Some(owner) = run.owner {
                    commands
                        .entity(entity)
                        .insert(crate::survivors::multiplayer::OwnedEffect(owner));
                }
                f.entity = Some(entity);
            }
            if f.at <= run.elapsed && f.left > 0 && run.legendary.has(f.unique) {
                let targets: Vec<_> = foes
                    .iter()
                    .filter(|(_, pos)| {
                        flat(*pos - f.pos).length() <= if f.targeted { 20.0 } else { f.radius }
                    })
                    .collect();
                if f.targeted {
                    events::queue(
                        run,
                        f.unique,
                        f.pos,
                        None,
                        None,
                        Shape::Guided(1, 20.0),
                        f.damage,
                        f.school,
                        0.0,
                    );
                } else {
                    let count = targets.len().max(1) as f32;
                    if !targets.is_empty() {
                        events::queue(
                            run,
                            f.unique,
                            f.pos,
                            None,
                            None,
                            Shape::Circle(f.radius),
                            f.damage / count,
                            f.school,
                            0.0,
                        );
                    }
                }
                f.left -= 1;
                f.at += 1.0;
            }
        }
        fields.retain(|f| {
            let keep = f.left > 0 && run.legendary.has(f.unique);
            if !keep {
                if let Some(entity) = f.entity {
                    commands.entity(entity).despawn();
                }
            }
            keep
        });
        run.legendary.periodic.fields = fields;
        run.legendary.periodic.foes = foes.clone();
        run.legendary.periodic.affected = enemies
            .iter()
            .filter(|(_, _, en)| !en.dead)
            .filter_map(|(e, _, en)| {
                let keys: Vec<_> = en
                    .dots
                    .iter()
                    .filter(|d| d.owner == run.owner && d.left > 0.0)
                    .map(|d| d.skill)
                    .collect();
                (!keys.is_empty()).then_some((e, keys))
            })
            .collect();
        if run.legendary.has(224)
            && run.legendary.periodic.turn
            && (run.legendary.periodic.last_turn == 0.0
                || run.elapsed - run.legendary.periodic.last_turn >= 1.0)
        {
            let oldest = enemies
                .iter()
                .filter(|(_, _, en)| !en.dead)
                .flat_map(|(e, tf, en)| {
                    en.dots
                        .iter()
                        .filter(|d| d.owner == run.owner && family(d) == Family::Curse)
                        .map(move |d| (e, tf.translation, d.clone()))
                })
                .min_by(|a, b| a.2.meta.started.total_cmp(&b.2.meta.started));
            if let Some((source, pos, dot)) = oldest {
                if let Some(target) = neighbour(run, source, pos) {
                    if let Ok((_, _, mut en)) = enemies.get_mut(source) {
                        if let Some(index) = en.dots.iter().position(|d| {
                            d.owner == dot.owner
                                && d.skill == dot.skill
                                && d.meta.started == dot.meta.started
                        }) {
                            let moved = en.dots.remove(index);
                            run.legendary.periodic.transfers.push((target, moved));
                            run.legendary.periodic.last_turn = run.elapsed;
                        }
                    }
                }
            } else if let Some(index) = run
                .legendary
                .casting
                .plans
                .iter()
                .position(|p| p.unique == 235)
            {
                let from = run.legendary.casting.plans[index].primary;
                if let Some((target, pos)) = foes
                    .iter()
                    .filter(|(e, _)| Some(*e) != from)
                    .min_by(|a, b| {
                        flat(a.1 - run.legendary.events.position)
                            .length_squared()
                            .total_cmp(&flat(b.1 - run.legendary.events.position).length_squared())
                    })
                    .copied()
                {
                    let plan = &mut run.legendary.casting.plans[index];
                    plan.primary = Some(target);
                    plan.center = pos;
                    run.legendary.periodic.last_turn = run.elapsed;
                }
            }
        }
        run.legendary.periodic.turn = false;
        if run.legendary.periodic.reserve_until < run.elapsed {
            run.legendary.periodic.reserve.clear();
        }
        transfers.extend(std::mem::take(&mut run.legendary.periodic.transfers));
        swaps.extend(
            std::mem::take(&mut run.legendary.periodic.swaps)
                .into_iter()
                .map(|p| (run.owner, p)),
        );
    };
    prepare(&mut run);
    if let Some(players) = players.as_deref_mut() {
        for player in players.0.values_mut() {
            prepare(&mut player.run);
        }
    }
    for (target, dot) in transfers {
        if let Ok((_, _, mut en)) = enemies.get_mut(target) {
            if !en.dead {
                if en.dots.len() >= 6 {
                    en.dots.remove(0);
                }
                en.dots.push(dot);
            }
        }
    }
    for (owner, (a, b)) in swaps {
        if let Ok([(_, _, mut first), (_, _, mut second)]) = enemies.get_many_mut([a, b]) {
            if !first.dead && !second.dead {
                let left: Vec<_> = first
                    .dots
                    .iter()
                    .filter(|d| d.owner == owner)
                    .cloned()
                    .collect();
                let right: Vec<_> = second
                    .dots
                    .iter()
                    .filter(|d| d.owner == owner)
                    .cloned()
                    .collect();
                first.dots.retain(|d| d.owner != owner);
                second.dots.retain(|d| d.owner != owner);
                first.dots.extend(right);
                second.dots.extend(left);
            }
        }
    }
    for (e, tf, mut en) in &mut enemies {
        let rooted = en.root_t > 0.0;
        let boss = en.boss;
        for dot in &mut en.dots {
            dot.meta.moving = dot
                .meta
                .previous
                .is_some_and(|old| flat(tf.translation - old).length() > 0.001);
            dot.meta.previous = Some(tf.translation);
            if !rooted {
                dot.meta.frozen_until = 0.0;
            }
            if dot.meta.held <= 0.0 {
                continue;
            }
            if let Some(player) = crate::survivors::multiplayer::player_run(
                &mut run,
                players.as_deref_mut(),
                dot.owner,
            ) {
                let fam = family(dot);
                let hold = (player.legendary.has(24) && fam == Family::Burn)
                    || (player.legendary.has(64) && !player.legendary.events.moving)
                    || (player.legendary.has(221) && fam == Family::Poison && dot.meta.moving)
                    || (player.legendary.has(236)
                        && fam == Family::Bleed
                        && rooted
                        && (!boss
                            || dot.meta.frozen_until == 0.0
                            || player.elapsed < dot.meta.frozen_until))
                    || (player.legendary.has(302) && dot.skill == "moonfire");
                if !hold {
                    let amount = std::mem::take(&mut dot.meta.held);
                    let id = if player.legendary.has(236) && fam == Family::Bleed {
                        236
                    } else if player.legendary.has(64) {
                        64
                    } else {
                        221
                    };
                    let shape = if id == 236 {
                        Shape::Guided(3, 20.0)
                    } else {
                        Shape::Guided(1, 0.0)
                    };
                    events::queue(
                        player,
                        id,
                        tf.translation,
                        if id == 236 { None } else { Some(e) },
                        if id == 236 { Some(e) } else { None },
                        shape,
                        amount,
                        dot.tags.unwrap_or(Tags::SHADOW),
                        0.0,
                    );
                }
            }
        }
    }
}
