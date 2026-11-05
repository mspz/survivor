//! Marks, linked targets and healing reserves are scoped to one equipped owner.
use super::*;
use super::{
    events::{self, Origin, Shape},
    periodic::{self, Family},
};
use crate::survivors::{
    abilities::Hit,
    data::{self, Effect},
    enemies::{Dot, Enemy},
    spell_engine::Resolved,
    units::flat,
};
#[derive(Clone)]
struct Mark {
    value: f32,
    until: f32,
    school: Tags,
    skill: &'static str,
    pos: Vec3,
    link: Option<Entity>,
    cast: u64,
}
#[derive(Default)]
pub(in crate::survivors) struct State {
    marks: BTreeMap<(Entity, u16), Mark>,
    pub overheal: f32,
    pub drain: f32,
    pub drain_until: f32,
    heal_seed: f32,
    seed_until: f32,
    seed_charges: u8,
    seal: f32,
    seal_until: f32,
    nova_until: f32,
    block: f32,
    block_target: Option<Entity>,
    cooldowns: BTreeMap<u16, f32>,
    pub heals: Vec<(Vec3, f32, f32, bool, bool)>,
    pub allies: Vec<Vec3>,
    pub pushes: Vec<(Vec3, f32)>,
    sides: u8,
    overlap: BTreeMap<(u64, Entity), u8>,
    pub active_heals: Vec<(f32, f32)>,
    pub shield_lock: f32,
    pub shield_generation: u64,
    pub shards: Vec<Vec3>,
    pub immunity_shield: f32,
}
impl State {
    pub(in crate::survivors) fn clear_area_mark(&mut self) {
        self.marks.retain(|(_, id), _| *id != 168);
    }
}
pub(in crate::survivors) fn seed(run: &mut Run, amount: f32, duration: f32) {
    run.legendary.effects.heal_seed = amount;
    run.legendary.effects.seed_charges = 3;
    run.legendary.effects.seed_until = run.elapsed + duration.max(1.0);
}
pub(in crate::survivors) fn pet_mark(run: &mut Run, target: Entity, pos: Vec3) {
    for id in [191, 198] {
        if run.legendary.has(id) {
            mark(run, target, id, 30.0, Tags::COLD, "proc_pet_bite", pos);
        }
    }
}
pub(in crate::survivors) fn equip(run: &mut Run) {
    let worn: BTreeSet<_> = run.legendary.equipped.keys().copied().collect();
    run.legendary
        .effects
        .marks
        .retain(|(_, id), _| worn.contains(id));
    if !worn.contains(&305) {
        run.legendary.effects.seed_charges = 0;
    }
    if !worn.contains(&310) {
        run.legendary.effects.drain = 0.0;
    }
    if !worn.contains(&78) {
        run.legendary.effects.overheal = 0.0;
    }
    if !worn.contains(&213) {
        run.legendary.effects.block = 0.0;
        run.legendary.effects.nova_until = 0.0;
    }
    if !worn.contains(&212) {
        run.legendary.effects.shield_lock = 0.0;
    }
}
pub(in crate::survivors) fn pack_center(run: &mut Run) -> Option<Vec3> {
    let target = run
        .legendary
        .effects
        .marks
        .iter()
        .filter(|((_, id), m)| *id == 198 && m.until > run.elapsed)
        .max_by(|a, b| a.1.until.total_cmp(&b.1.until))
        .map(|(k, _)| *k);
    if let Some(key) = target {
        run.legendary.effects.marks.remove(&key);
        run.legendary
            .periodic
            .foes
            .iter()
            .find(|(entity, _)| *entity == key.0)
            .map(|(_, pos)| *pos)
    } else {
        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn drain_reserve_expires_two_seconds_after_the_last_drain_tick() {
        let mut run = Run::default();
        run.legendary.equipped.insert(
            310,
            Roll {
                unique: 310,
                milestone: None,
            },
        );
        run.elapsed = 1.0;
        periodic_hit(
            &mut run,
            Entity::from_bits(1),
            Vec3::ZERO,
            "drain_life",
            Tags::SHADOW,
            20.0,
            5.0,
        );
        assert_eq!(run.legendary.effects.drain_until, 8.0);
        run.elapsed = 6.0;
        periodic_hit(
            &mut run,
            Entity::from_bits(1),
            Vec3::ZERO,
            "drain_life",
            Tags::SHADOW,
            20.0,
            0.0,
        );
        assert_eq!(run.legendary.effects.drain_until, 8.0);
        assert_eq!(run.legendary.effects.drain, 12.0);
    }
    #[test]
    fn pack_cone_uses_the_marked_victims_current_position_and_consumes_the_mark() {
        let mut run = Run::default();
        run.legendary.equipped.insert(
            198,
            Roll {
                unique: 198,
                milestone: None,
            },
        );
        let target = Entity::from_bits(1);
        pet_mark(&mut run, target, Vec3::X);
        run.legendary.periodic.foes = vec![(target, Vec3::X * 8.0)];
        assert_eq!(pack_center(&mut run), Some(Vec3::X * 8.0));
        assert_eq!(pack_center(&mut run), None);
        pet_mark(&mut run, target, Vec3::X);
        run.legendary.periodic.foes.clear();
        assert_eq!(pack_center(&mut run), None);
    }
}
fn ready(run: &mut Run, id: u16, seconds: f32) -> bool {
    let now = run.elapsed;
    if run
        .legendary
        .effects
        .cooldowns
        .get(&id)
        .is_some_and(|at| *at > now)
    {
        false
    } else {
        run.legendary.effects.cooldowns.insert(id, now + seconds);
        true
    }
}
fn mark(
    run: &mut Run,
    target: Entity,
    id: u16,
    value: f32,
    school: Tags,
    skill: &'static str,
    pos: Vec3,
) {
    run.legendary.effects.marks.insert(
        (target, id),
        Mark {
            value,
            school,
            skill,
            pos,
            until: run.elapsed + 6.0,
            link: None,
            cast: 0,
        },
    );
}
fn take(run: &mut Run, target: Entity, id: u16) -> Option<Mark> {
    run.legendary
        .effects
        .marks
        .remove(&(target, id))
        .filter(|m| m.until >= run.elapsed)
}
fn burst(run: &mut Run, id: u16, pos: Vec3, damage: f32, school: Tags) {
    events::queue(
        run,
        id,
        pos,
        None,
        None,
        Shape::Circle(5.0),
        damage,
        school,
        0.0,
    );
}
pub(in crate::survivors) fn fear_boss(run: &mut Run, target: Entity, pos: Vec3) {
    if ready(run, 159, 2.0) {
        events::queue(
            run,
            159,
            pos,
            Some(target),
            None,
            Shape::Guided(1, 0.0),
            30.0,
            Tags::SHADOW,
            0.0,
        );
    }
}
pub(in crate::survivors) fn before_hit(
    run: &mut Run,
    h: &mut Hit,
    en: &mut Enemy,
    pos: Vec3,
    tags: Tags,
) {
    if h.origin != Origin::Skill || h.triggered {
        return;
    }
    if run.legendary.has(191) && tags.contains(Tags::PHYSICAL) {
        if let Some(m) = take(run, h.target, 191) {
            if en.boss {
                events::queue(
                    run,
                    191,
                    pos,
                    Some(h.target),
                    None,
                    Shape::Guided(1, 0.0),
                    m.value,
                    Tags::COLD,
                    0.0,
                );
            } else {
                en.root_t = en.root_t.max(2.0);
            }
        }
    }
    if run.legendary.has(176) && h.skill == "execute" {
        h.effect = Effect::None;
    }
    if run.legendary.has(176) && h.skill != "execute" && tags.contains(Tags::MELEE) {
        if let Some(sentence) = take(run, h.target, 176) {
            if !en.boss && en.hp <= en.max_hp * sentence.value {
                h.effect = Effect::Execute(1.0);
            }
        }
    }
    if h.damage <= 0.0 {
        return;
    }
    if run.legendary.has(285) {
        let school = events::school(tags);
        if let Some(previous) = take(run, h.target, 285) {
            if previous.school.0 == school {
                mark(
                    run,
                    h.target,
                    285,
                    previous.value + h.damage,
                    Tags(school),
                    h.skill,
                    pos,
                );
                h.damage = 0.0;
            } else {
                if previous.value > 0.0 {
                    events::queue(
                        run,
                        285,
                        pos,
                        Some(h.target),
                        None,
                        Shape::Guided(1, 0.0),
                        previous.value,
                        previous.school,
                        0.0,
                    );
                }
                mark(run, h.target, 285, 0.0, Tags(school), h.skill, pos);
            }
        } else {
            mark(run, h.target, 285, 0.0, Tags(school), h.skill, pos);
        }
    }
    if run.legendary.has(213) && tags.contains(Tags::MELEE) && run.legendary.effects.block > 0.0 {
        let damage = std::mem::take(&mut run.legendary.effects.block);
        let target = run
            .legendary
            .effects
            .block_target
            .take()
            .filter(|attacker| {
                run.legendary.periodic.foes.iter().any(|(e, p)| {
                    e == attacker && flat(*p - run.legendary.events.position).length() <= 4.0
                })
            })
            .unwrap_or(h.target);
        events::queue(
            run,
            213,
            run.legendary.events.position,
            Some(target),
            None,
            Shape::Guided(1, 4.0),
            damage,
            Tags::PHYSICAL,
            0.0,
        );
    }
}
pub(in crate::survivors) fn hit(
    run: &mut Run,
    h: &Hit,
    en: &mut Enemy,
    pos: Vec3,
    tags: Tags,
    effective: f32,
    feared: bool,
    rng: &mut Rng,
) {
    if h.origin != Origin::Skill || h.triggered || effective <= 0.0 {
        return;
    }
    if run.legendary.has(163) && tags.contains(Tags::MELEE) {
        if let Some(link) = take(run, h.target, 163).and_then(|m| m.link) {
            run.legendary.effects.marks.remove(&(link, 163));
            events::queue(
                run,
                163,
                pos,
                Some(link),
                None,
                Shape::Guided(1, 20.0),
                h.damage,
                Tags(tags.0 & super::SCHOOLS),
                0.0,
            );
        } else if let Some((other, other_pos)) = run
            .legendary
            .periodic
            .foes
            .iter()
            .filter(|(e, p)| *e != h.target && flat(*p - pos).length() <= 12.0)
            .min_by(|a, b| {
                flat(a.1 - pos)
                    .length_squared()
                    .total_cmp(&flat(b.1 - pos).length_squared())
            })
            .copied()
        {
            mark(run, h.target, 163, 0.0, tags, h.skill, pos);
            mark(run, other, 163, 0.0, tags, h.skill, other_pos);
            run.legendary
                .effects
                .marks
                .get_mut(&(h.target, 163))
                .unwrap()
                .link = Some(other);
            run.legendary
                .effects
                .marks
                .get_mut(&(other, 163))
                .unwrap()
                .link = Some(h.target);
        }
    }
    if run.legendary.has(168) && tags.contains(Tags::MELEE) {
        run.legendary.effects.marks.retain(|(_, id), _| *id != 168);
        mark(run, h.target, 168, 0.0, tags, h.skill, pos);
    }
    if run.legendary.has(126) && tags.contains(Tags::AREA) {
        if let Some(old) = take(run, h.target, 126) {
            if old.cast != h.cast_id {
                burst(
                    run,
                    126,
                    pos,
                    old.value,
                    Tags(old.school.0 & super::SCHOOLS),
                );
            } else {
                run.legendary.effects.marks.insert((h.target, 126), old);
            }
        }
        if h.item_source == Some(126) {
            let count = run
                .legendary
                .effects
                .overlap
                .entry((h.cast_id, h.target))
                .or_default();
            *count += 1;
            if *count >= 2 {
                mark(run, h.target, 126, h.damage * 0.5, tags, h.skill, pos);
                if let Some(m) = run.legendary.effects.marks.get_mut(&(h.target, 126)) {
                    m.cast = h.cast_id;
                }
            }
        }
    }
    if run.legendary.has(169) && h.item_source == Some(169) {
        let side = *run.legendary.casting.sides.get(&h.cast_id).unwrap_or(&0);
        run.legendary.effects.sides |= if side < 0 { 1 } else { 2 };
        if run.legendary.effects.sides == 3 {
            run.legendary.effects.sides = 0;
            let behind = run.legendary.events.position
                - run.legendary.casting.last_heading.normalize_or(Vec3::NEG_Z) * 3.0;
            burst(run, 169, behind, h.damage, tags);
        }
    }
    if run.legendary.has(111)
        && tags.contains(Tags::PROJECTILE)
        && rng.f32() < 0.25
        && ready(run, 111, 1.0)
    {
        if let Some(other) = run
            .legendary
            .periodic
            .affected
            .iter()
            .filter(|(e, _)| **e != h.target)
            .filter_map(|(e, _)| {
                run.legendary
                    .periodic
                    .foes
                    .iter()
                    .find(|(target, _)| target == e)
            })
            .filter(|(_, p)| flat(*p - pos).length() <= 12.0)
            .min_by(|a, b| {
                flat(a.1 - pos)
                    .length_squared()
                    .total_cmp(&flat(b.1 - pos).length_squared())
            })
            .copied()
        {
            run.legendary.periodic.swaps.push((h.target, other.0));
            events::queue(
                run,
                111,
                pos,
                Some(other.0),
                None,
                Shape::Guided(1, 12.0),
                h.damage,
                tags,
                0.1,
            );
        }
    }
    if run.legendary.has(234) && tags.contains(Tags::ATTACK) && run.elapsed % 6.0 >= 3.0 {
        let next = run
            .legendary
            .periodic
            .foes
            .iter()
            .filter(|(e, p)| *e != h.target && flat(*p - pos).length() <= 12.0)
            .min_by(|a, b| {
                flat(a.1 - pos)
                    .length_squared()
                    .total_cmp(&flat(b.1 - pos).length_squared())
            })
            .map(|p| p.0);
        if let Some(target) = next {
            if let Some(dot) = en
                .dots
                .iter_mut()
                .find(|d| d.owner == h.owner && periodic::budget(d) >= d.dps && d.dps > 0.0)
            {
                dot.left = (dot.left - 1.0).max(0.0);
                events::queue(
                    run,
                    234,
                    pos,
                    Some(target),
                    None,
                    Shape::Guided(1, 12.0),
                    dot.dps,
                    dot.tags.unwrap_or(tags),
                    0.1,
                );
            }
        }
    }
    if run.legendary.has(239) {
        let mut schools = BTreeSet::new();
        let mut total = 0.0;
        let mut duration = 0.0_f32;
        for dot in en
            .dots
            .iter()
            .filter(|d| d.owner == h.owner && d.item_source != Some(239))
        {
            schools.insert(events::school(dot.tags.unwrap_or(Tags(0))));
            total += periodic::budget(dot);
            duration = duration.max(dot.left);
        }
        if schools.len() >= 3 && duration > 0.0 {
            en.dots
                .retain(|d| d.owner != h.owner || d.item_source == Some(239));
            let cycle: Vec<_> = schools.into_iter().map(Tags).collect();
            let school = cycle[0];
            let dot = Dot {
                owner: h.owner,
                skill: "proc_plague",
                meter_source: super::definition(239).map(|d| d.name),
                tags: Some(Tags::SPELL | Tags::DAMAGE | Tags::DOT | school),
                item_source: Some(239),
                origin: Origin::Legendary,
                meta: periodic::Meta {
                    family: Family::Other,
                    cycle,
                    ..default()
                },
                dps: total / duration.floor().max(1.0),
                left: duration,
                tick: duration.min(1.0),
            };
            periodic::install(run, en, h.target, pos, dot);
        }
    }
    let school = Tags(events::school(tags));
    if run.legendary.has(147) && tags.contains(Tags::HOLY) {
        if let Some(index) = en
            .dots
            .iter()
            .position(|d| d.owner == h.owner && periodic::family(d) == Family::Poison)
        {
            let dot = en.dots.remove(index);
            periodic::field(
                run,
                147,
                pos,
                periodic::budget(&dot),
                dot.left,
                Tags::NATURE,
                true,
            );
        }
    }
    if run.legendary.has(13) && h.skill == "chain_lightning" {
        let aim = flat(pos - run.legendary.events.position).normalize_or(Vec3::NEG_Z);
        events::queue(
            run,
            13,
            pos,
            None,
            Some(h.target),
            Shape::Cone(aim, 7.0),
            h.damage,
            Tags::LIGHTNING | Tags::NATURE,
            0.0,
        );
    }
    if run.legendary.has(43)
        && h.skill == "cleave"
        && en
            .dots
            .iter()
            .any(|d| d.owner == h.owner && periodic::family(d) == Family::Bleed)
    {
        let mut targets: Vec<_> = run
            .legendary
            .periodic
            .affected
            .iter()
            .filter(|(e, keys)| {
                **e != h.target
                    && keys
                        .iter()
                        .any(|key| matches!(*key, "rend" | "rupture" | "hemorrhage" | "proc_bleed"))
            })
            .filter_map(|(e, _)| {
                run.legendary
                    .periodic
                    .foes
                    .iter()
                    .find(|(target, _)| target == e)
            })
            .copied()
            .collect();
        targets.sort_by(|a, b| {
            flat(a.1 - pos)
                .length_squared()
                .total_cmp(&flat(b.1 - pos).length_squared())
        });
        for (target, _) in targets.into_iter().take(2) {
            events::queue(
                run,
                43,
                pos,
                Some(target),
                None,
                Shape::Guided(1, f32::INFINITY),
                h.damage,
                school,
                0.0,
            );
        }
    }
    if run.legendary.has(18)
        && tags.contains(Tags::COLD)
        && (en.root_t > 0.0 || en.boss && en.slow_t > 0.0)
    {
        events::queue(
            run,
            18,
            pos,
            None,
            Some(h.target),
            Shape::Guided(3, 12.0),
            h.damage * 0.25,
            Tags::COLD,
            0.0,
        );
    }
    if run.legendary.has(26) && h.skill == "arcane_missiles" {
        let mut rune = take(run, h.target, 26).map_or(0.0, |m| m.value) + 1.0;
        if rune >= 3.0 {
            burst(run, 26, pos, h.damage, Tags::ARCANE);
            rune = 0.0;
        }
        mark(run, h.target, 26, rune, Tags::ARCANE, h.skill, pos);
    }
    if run.legendary.has(50) && tags.contains(Tags::MELEE) {
        events::queue(
            run,
            50,
            pos,
            None,
            None,
            Shape::Circle(4.0),
            h.damage * 0.5,
            school,
            0.25,
        );
    }
    if run.legendary.has(56) {
        if tags.contains(Tags::HOLY) {
            let seals = take(run, h.target, 56).map_or(0.0, |m| m.value) + h.damage * 0.25;
            mark(run, h.target, 56, seals, Tags::HOLY, h.skill, pos);
        } else if tags.contains(Tags::PHYSICAL) {
            if let Some(seals) = take(run, h.target, 56) {
                burst(run, 56, pos, seals.value, Tags::HOLY);
            }
        }
    }
    if run.legendary.has(95) {
        if let Some(cold) = take(run, h.target, 95) {
            burst(run, 95, pos, cold.value, Tags::COLD);
        }
    }
    if run.legendary.has(97) {
        if tags.contains(Tags::AREA) {
            if let Some(shadow) = take(run, h.target, 97) {
                burst(run, 97, pos, shadow.value, Tags::SHADOW);
            }
        } else {
            mark(
                run,
                h.target,
                97,
                h.damage * 0.5,
                Tags::SHADOW,
                h.skill,
                pos,
            );
        }
    }
    if run.legendary.has(141) {
        if let Some(old) = take(run, h.target, 141) {
            if old.school != school {
                burst(run, 141, pos, old.value, old.school);
            }
        }
        mark(run, h.target, 141, h.damage * 0.3, school, h.skill, pos);
    }
    if run.legendary.has(146) {
        if tags.contains(Tags::PHYSICAL) {
            if let Some(brittle) = take(run, h.target, 146) {
                if brittle.skill == "frostbolt" {
                    burst(run, 146, pos, brittle.value, Tags::PHYSICAL);
                } else {
                    mark(
                        run,
                        h.target,
                        146,
                        brittle.value + h.damage * 0.25,
                        school,
                        h.skill,
                        pos,
                    );
                }
            } else {
                mark(run, h.target, 146, h.damage * 0.25, school, h.skill, pos);
            }
        } else if tags.contains(Tags::COLD) {
            if let Some(heat) = take(run, h.target, 146) {
                mark(
                    run,
                    h.target,
                    146,
                    heat.value,
                    Tags::PHYSICAL,
                    "frostbolt",
                    pos,
                );
            }
        }
    }
    if run.legendary.has(148) {
        if tags.contains(Tags::AREA) {
            if let Some(weight) = take(run, h.target, 148) {
                burst(run, 148, pos, weight.value, Tags::PHYSICAL);
            }
        } else if spell_engine_tags(h.skill).contains(Tags::COLD) {
            let value = take(run, h.target, 148).map_or(0.0, |m| m.value) + h.damage * 0.25;
            mark(run, h.target, 148, value, Tags::PHYSICAL, h.skill, pos);
        }
    }
    if run.legendary.has(151) {
        if tags.contains(Tags::COLD) && h.crit {
            mark(
                run,
                h.target,
                151,
                h.damage * 0.5,
                Tags::LIGHTNING | Tags::NATURE,
                h.skill,
                pos,
            );
        } else if tags.contains(Tags::LIGHTNING) && take(run, h.target, 151).is_some() {
            let conductors: Vec<_> = run
                .legendary
                .effects
                .marks
                .keys()
                .filter(|(_, id)| *id == 151)
                .copied()
                .collect();
            for (target, id) in conductors {
                if let Some(m) = take(run, target, id) {
                    events::queue(
                        run,
                        151,
                        m.pos,
                        Some(target),
                        None,
                        Shape::Guided(1, 0.0),
                        h.damage,
                        Tags::LIGHTNING | Tags::NATURE,
                        0.0,
                    );
                }
            }
        }
    }
    if run.legendary.has(155) {
        if tags.contains(Tags::NATURE) && en.slow_t > 0.0 {
            en.root_t = en.root_t.max(if en.boss { 0.3 } else { 1.0 });
            mark(
                run,
                h.target,
                155,
                h.damage * 0.5,
                Tags::NATURE,
                h.skill,
                pos,
            );
        } else if tags.contains(Tags::MELEE) {
            if let Some(root) = take(run, h.target, 155) {
                en.root_t = 0.0;
                burst(run, 155, pos, root.value, Tags::NATURE);
            }
        }
    }
    if run.legendary.has(160) {
        if let Some(old) = take(run, h.target, 160) {
            if old.school != school {
                events::queue(
                    run,
                    160,
                    pos,
                    None,
                    Some(h.target),
                    Shape::Guided(1, 20.0),
                    h.damage * 0.5,
                    old.school,
                    0.0,
                );
            } else {
                mark(run, h.target, 160, h.damage * 0.5, school, h.skill, pos);
            }
        } else {
            mark(run, h.target, 160, h.damage * 0.5, school, h.skill, pos);
        }
    }
    if run.legendary.has(170) {
        let orientation = flat(pos - run.legendary.events.position).normalize_or(Vec3::NEG_Z);
        let mut consumed = false;
        if tags.contains(Tags::MELEE) {
            if let Some(old) = take(run, h.target, 170) {
                consumed = true;
                let dir = old.pos.normalize_or(Vec3::NEG_Z);
                events::queue(
                    run,
                    170,
                    pos,
                    None,
                    None,
                    Shape::Cone(dir, 8.0),
                    h.damage * 0.5,
                    school,
                    0.0,
                );
            }
        } else if tags.contains(Tags::PHYSICAL) {
            mark(
                run,
                h.target,
                170,
                h.damage * 0.5,
                school,
                h.skill,
                orientation,
            );
        }
        if !consumed
            && tags.contains(Tags::MELEE | Tags::PHYSICAL)
            && !run.legendary.effects.marks.contains_key(&(h.target, 170))
        {
            mark(
                run,
                h.target,
                170,
                h.damage * 0.5,
                school,
                h.skill,
                orientation,
            );
        }
    }
    if run.legendary.has(171) && tags.contains(Tags::MELEE) {
        if let Some((key, mut p)) = run.legendary.casting.last_ranged.take() {
            p.range = 4.0;
            p.count = 1;
            p.radius = 0.0;
            p.cast_id = 0;
            super::casting::plan(
                run,
                171,
                data::ability(key),
                p,
                pos,
                None,
                Some(h.target),
                0.0,
                Origin::Legendary,
                Vec::new(),
            );
            if let Some(plan) = run.legendary.casting.plans.last_mut() {
                plan.guided = true;
            }
        }
    }
    if run.legendary.has(176) && h.skill == "execute" && !en.boss && en.hp > 0.0 {
        let threshold = if let Effect::Execute(t) = data::ability("execute").effect {
            t
        } else {
            0.2
        };
        mark(run, h.target, 176, threshold, school, h.skill, pos);
    }
    if run.legendary.has(235) && feared {
        en.fear_t = 0.0;
        run.legendary
            .casting
            .plans
            .retain(|p| !(p.unique == 235 && p.primary == Some(h.target)));
        let mut p = run.resolve_spell(data::ability(h.skill), 1);
        p.dmg = effective;
        p.dot = 0.0;
        p.radius = 0.0;
        p.count = 1;
        p.range = f32::INFINITY;
        p.crit = 0.0;
        p.cast_id = 0;
        p.tags = tags;
        super::casting::plan(
            run,
            235,
            data::ability(h.skill),
            p,
            pos,
            None,
            Some(h.target),
            3.0,
            Origin::Legendary,
            Vec::new(),
        );
        if let Some(plan) = run.legendary.casting.plans.last_mut() {
            plan.guided = true;
            plan.effect = Effect::None;
        }
    }
    if run.legendary.has(273) && tags.contains(Tags::COLD) {
        let dot = Dot {
            owner: h.owner,
            skill: "proc_bleed",
            meter_source: super::definition(273).map(|d| d.name),
            tags: Some(Tags::DOT | Tags::DAMAGE | Tags::PHYSICAL),
            item_source: None,
            origin: Origin::Legendary,
            meta: periodic::Meta {
                family: Family::Bleed,
                ..default()
            },
            dps: h.damage * 0.2,
            left: 4.0,
            tick: 1.0,
        };
        periodic::install(run, en, h.target, pos, dot);
    }
    if run.legendary.has(276) && tags.contains(Tags::LIGHTNING) && rng.f32() < 0.2 {
        events::queue(
            run,
            276,
            pos,
            None,
            Some(h.target),
            Shape::Guided(2, 8.0),
            h.damage * 0.5,
            Tags::LIGHTNING | Tags::NATURE,
            0.0,
        );
    }
    if run.legendary.has(286) && tags.contains(Tags::SPELL) {
        if let Some(old) = take(run, h.target, 286) {
            if old.skill != h.skill {
                super::casting::copy(run, 286, old.skill, pos, Some(h.target), 0.1);
            } else {
                mark(run, h.target, 286, old.value, old.school, old.skill, pos);
            }
        } else {
            mark(run, h.target, 286, h.damage, school, h.skill, pos);
        }
    }
    if run.legendary.has(304) {
        if h.skill == "claw" {
            mark(run, h.target, 304, h.damage, Tags::PHYSICAL, h.skill, pos);
        } else if h.skill == "swipe" {
            if let Some(claw) = take(run, h.target, 304) {
                events::queue(
                    run,
                    304,
                    pos,
                    None,
                    None,
                    Shape::Circle(4.0),
                    claw.value,
                    Tags::PHYSICAL,
                    0.3,
                );
            }
        }
    }
    if run.legendary.has(305)
        && h.skill == "claw"
        && run.legendary.effects.seed_until >= run.elapsed
        && run.legendary.effects.seed_charges > 0
    {
        let amount = run.legendary.effects.heal_seed / 3.0;
        run.legendary.effects.seed_charges -= 1;
        run.legendary.effects.heals.push((
            run.legendary.events.position,
            amount,
            6.0,
            false,
            false,
        ));
    }
}
pub(in crate::survivors) fn area_center(run: &Run) -> Option<Vec3> {
    let target = run
        .legendary
        .effects
        .marks
        .iter()
        .find(|((_, id), mark)| *id == 168 && mark.until >= run.elapsed)?
        .0
         .0;
    let pos = run
        .legendary
        .periodic
        .foes
        .iter()
        .find(|(e, _)| *e == target)?
        .1;
    Some((run.legendary.events.position + pos) * 0.5)
}
fn spell_engine_tags(key: &str) -> Tags {
    crate::survivors::spell_engine::tags::for_spell(data::ability(key))
}
pub(in crate::survivors) fn periodic_hit(
    run: &mut Run,
    target: Entity,
    pos: Vec3,
    key: &'static str,
    tags: Tags,
    damage: f32,
    remaining: f32,
) {
    if run.legendary.has(95) {
        let held = take(run, target, 95).map_or(0.0, |m| m.value) + damage * 0.25;
        mark(run, target, 95, held, Tags::COLD, key, pos);
    }
    if run.legendary.has(310) && key == "drain_life" {
        run.legendary.effects.drain =
            (run.legendary.effects.drain + damage * 0.3).min(run.stats().max_hp * 0.25);
        run.legendary.effects.drain_until = run.elapsed + remaining.max(0.0) + 2.0;
    }
    let _ = tags;
}
pub(in crate::survivors) fn cast(
    run: &mut Run,
    def: &AbilityDef,
    p: &Resolved,
    primary: Option<(Entity, Vec3)>,
    repeated: bool,
) {
    if repeated {
        return;
    }
    if run.legendary.has(213) && def.kind == Kind::Nova {
        run.legendary.effects.nova_until = run.elapsed + 6.0;
    }
    if run.legendary.has(78)
        && p.tags.contains(Tags::DAMAGE)
        && run.legendary.effects.overheal > 0.0
    {
        let amount = std::mem::take(&mut run.legendary.effects.overheal);
        events::queue(
            run,
            78,
            run.legendary.events.position,
            primary.map(|p| p.0),
            None,
            Shape::Guided(1, 24.0),
            amount,
            Tags::HOLY,
            0.0,
        );
    }
    if run.legendary.has(203) && p.tags.contains(Tags::MELEE) && ready(run, 203, 4.0) {
        if let Some(heal) = run.abilities.iter().find(|o| o.def.kind == Kind::Heal) {
            let amount = run.resolve_spell(heal.def, heal.level).dmg * 0.25;
            run.legendary.effects.heals.push((
                run.legendary.events.position,
                amount,
                6.0,
                false,
                false,
            ));
        }
    }
    if def.kind == Kind::Heal {
        if run.legendary.has(77) {
            burst(
                run,
                77,
                run.legendary.events.position,
                p.dmg * 0.25,
                Tags::HOLY,
            );
        }
        if run.legendary.has(207) {
            run.legendary.effects.seal = p.dmg * 0.25;
            run.legendary.effects.seal_until = run.elapsed + 6.0;
        }
    }
    if run.legendary.has(310)
        && def.key == "death_coil"
        && run.legendary.effects.drain_until >= run.elapsed
    {
        let amount = std::mem::take(&mut run.legendary.effects.drain);
        run.shield += amount;
        run.shield_t = run.shield_t.max(3.0);
        run.legendary.events.shield_origin = Origin::Legendary;
    }
}
pub(in crate::survivors) fn active_heal(run: &mut Run, amount: f32) -> bool {
    let effective = amount.max(0.0) * (1.0 + run.equipment.healing).max(0.0);
    if run.legendary.has(78) {
        run.legendary.effects.overheal = (run.legendary.effects.overheal
            + (effective - (run.stats().max_hp - run.hp).max(0.0)).max(0.0))
        .min(run.stats().max_hp * 0.3);
    }
    if run.legendary.has(217) && run.immune_t > 0.0 {
        run.shield += effective;
        run.legendary.effects.immunity_shield += effective;
        run.shield_t = run.shield_t.max(run.immune_t);
        run.legendary.events.shield_origin = Origin::Legendary;
        return true;
    }
    false
}
pub(in crate::survivors) fn blocked(run: &mut Run, raw: f32) {
    if run.legendary.has(213) && run.legendary.effects.nova_until > run.elapsed {
        run.legendary.effects.block = raw.min(run.stats().max_hp * 0.2);
        run.legendary.effects.block_target = run.legendary.events.attacker;
        run.legendary.effects.nova_until = 0.0;
    }
    if run.legendary.has(99) && ready(run, 99, 1.0) {
        if let Some((key, _)) = run.legendary.casting.last_ranged {
            let origin = run
                .legendary
                .events
                .attacker
                .and_then(|a| {
                    run.legendary
                        .periodic
                        .foes
                        .iter()
                        .find(|(e, _)| *e == a)
                        .map(|(_, p)| *p)
                })
                .unwrap_or(run.legendary.events.position);
            super::casting::copy(run, 99, key, origin, None, 0.0);
        }
    }
    if run.legendary.has(277) && ready(run, 277, 1.0) {
        let school = if run.legendary.casting.last_school.0 == 0 {
            Tags::PHYSICAL
        } else {
            run.legendary.casting.last_school
        };
        burst(run, 277, run.legendary.events.position, 45.0, school);
    }
}
pub(in crate::survivors) fn hurt(run: &mut Run, taken: f32) {
    if taken > 0.0
        && run.legendary.has(207)
        && run.legendary.effects.seal_until >= run.elapsed
        && run.legendary.effects.seal > 0.0
    {
        let amount = std::mem::take(&mut run.legendary.effects.seal);
        run.legendary.effects.heals.push((
            run.legendary.events.position,
            amount,
            12.0,
            false,
            true,
        ));
    }
}
pub(in crate::survivors) fn shield_expired(run: &mut Run, remaining: f32, broken: bool) {
    if broken && run.legendary.has(212) {
        run.legendary.effects.shield_generation =
            run.legendary.effects.shield_generation.wrapping_add(1);
        run.legendary.effects.shield_lock = run.elapsed + 8.0;
        run.legendary
            .effects
            .shards
            .push(run.legendary.events.position);
    }
    if run.legendary.has(58) {
        if broken {
            run.legendary
                .effects
                .pushes
                .push((run.legendary.events.position, 6.0));
        } else if remaining > 0.0 {
            burst(
                run,
                58,
                run.legendary.events.position,
                remaining,
                Tags::HOLY,
            );
        }
    }
}
pub(in crate::survivors) fn death(
    run: &mut Run,
    target: Entity,
    pos: Vec3,
    key: &'static str,
    damage: f32,
) {
    if run.legendary.has(44) && key == "execute" {
        events::queue(
            run,
            44,
            pos,
            None,
            Some(target),
            Shape::Guided(1, 20.0),
            damage,
            Tags::PHYSICAL,
            0.0,
        );
    }
    run.legendary
        .effects
        .marks
        .retain(|(entity, _), _| *entity != target);
}
pub(in crate::survivors) fn tick(
    mut commands: Commands,
    mut run: ResMut<Run>,
    mut players: Option<ResMut<crate::survivors::multiplayer::Companions>>,
    actors: Query<
        (
            &Transform,
            Option<&crate::survivors::multiplayer::RemoteOwner>,
            Has<crate::survivors::hero::Hero>,
        ),
        Without<Enemy>,
    >,
    mut enemies: Query<(Entity, &mut Transform, &mut Enemy)>,
) {
    let positions: Vec<_> = actors
        .iter()
        .filter_map(|(tf, owner, local)| {
            owner
                .map(|o| (Some(o.0), tf.translation))
                .or_else(|| local.then_some((run.owner, tf.translation)))
        })
        .collect();
    let mut heals = Vec::new();
    let mut pushes = Vec::new();
    let mut collect = |run: &mut Run| {
        for pos in std::mem::take(&mut run.legendary.effects.shards) {
            let serial = run.guid();
            crate::survivors::enemies::spawn_gem(
                &mut commands,
                serial,
                pos,
                crate::survivors::enemies::GemKind::Shield(
                    run.owner,
                    run.legendary.effects.shield_generation,
                ),
            );
        }
        if run.legendary.effects.immunity_shield > 0.0 && run.immune_t <= 0.0 {
            let amount = std::mem::take(&mut run.legendary.effects.immunity_shield).min(run.shield);
            run.shield -= amount;
            run.heal_rate += amount / 3.0;
            run.heal_t = run.heal_t.max(3.0);
        }
        for (amount, duration) in std::mem::take(&mut run.legendary.effects.active_heals) {
            let center = run.legendary.events.position;
            let kind = if run.legendary.has(152) {
                Some(Family::Bleed)
            } else if run.legendary.has(237) {
                Some(Family::Disease)
            } else {
                None
            };
            let candidate = kind.and_then(|family| {
                enemies
                    .iter()
                    .filter(|(_, tf, en)| {
                        !en.dead
                            && flat(tf.translation - center).length() <= 12.0
                            && en
                                .dots
                                .iter()
                                .any(|d| d.owner == run.owner && periodic::family(d) == family)
                    })
                    .min_by(|(_, a, ae), (_, b, be)| {
                        let oldest = |en: &Enemy| {
                            en.dots
                                .iter()
                                .filter(|d| d.owner == run.owner && periodic::family(d) == family)
                                .map(|d| d.meta.started)
                                .min_by(f32::total_cmp)
                                .unwrap_or(f32::INFINITY)
                        };
                        (if family == Family::Disease {
                            oldest(ae).total_cmp(&oldest(be))
                        } else {
                            std::cmp::Ordering::Equal
                        })
                        .then_with(|| {
                            flat(a.translation - center)
                                .length_squared()
                                .total_cmp(&flat(b.translation - center).length_squared())
                        })
                    })
                    .map(|(e, tf, _)| (e, tf.translation))
            });
            let destination = candidate.and_then(|(source, pos)| {
                enemies
                    .iter()
                    .filter(|(e, tf, en)| {
                        *e != source && !en.dead && flat(tf.translation - pos).length() <= 12.0
                    })
                    .min_by(|(_, a, _), (_, b, _)| {
                        flat(a.translation - pos)
                            .length_squared()
                            .total_cmp(&flat(b.translation - pos).length_squared())
                    })
                    .map(|(e, _, _)| e)
            });
            if let Some((target, pos)) =
                candidate.filter(|_| kind == Some(Family::Bleed) || destination.is_some())
            {
                if let Ok((_, _, mut en)) = enemies.get_mut(target) {
                    if let Some(index) = en
                        .dots
                        .iter()
                        .position(|d| d.owner == run.owner && Some(periodic::family(d)) == kind)
                    {
                        let dot = en.dots.remove(index);
                        if kind == Some(Family::Bleed) {
                            events::queue(
                                run,
                                152,
                                pos,
                                Some(target),
                                None,
                                Shape::Guided(1, 0.0),
                                periodic::budget(&dot),
                                Tags::HOLY,
                                0.0,
                            );
                        } else if let Some(next) = destination {
                            run.legendary.periodic.transfers.push((next, dot));
                        }
                        continue;
                    }
                }
            }
            if duration > 0.0 {
                run.heal_rate = amount / duration.max(1.0);
                run.heal_t = duration;
            } else {
                events::active_heal(run, amount);
            }
        }
        run.legendary
            .effects
            .marks
            .retain(|_, m| m.until >= run.elapsed);
        if run.legendary.effects.drain > 0.0
            && !enemies.iter().any(|(_, _, en)| {
                !en.dead
                    && en
                        .dots
                        .iter()
                        .any(|d| d.owner == run.owner && d.skill == "drain_life" && d.left > 0.0)
            })
        {
            run.legendary.effects.drain_until =
                run.legendary.effects.drain_until.min(run.elapsed + 2.0);
        }
        if run.legendary.effects.drain_until < run.elapsed {
            run.legendary.effects.drain = 0.0;
        }
        run.legendary.effects.allies = positions.iter().map(|(_, pos)| *pos).collect();
        heals.extend(
            std::mem::take(&mut run.legendary.effects.heals)
                .into_iter()
                .map(|p| (run.owner, p)),
        );
        pushes.extend(std::mem::take(&mut run.legendary.effects.pushes));
    };
    collect(&mut run);
    if let Some(players) = players.as_deref_mut() {
        for player in players.0.values_mut() {
            collect(&mut player.run);
        }
    }
    for (source, (pos, amount, radius, percent, nearest)) in heals {
        let mut targets = Vec::new();
        for (owner, p) in &positions {
            if flat(*p - pos).length() <= radius
                && (!nearest || positions.len() <= 1 || *owner != source)
            {
                if let Some(player) = crate::survivors::multiplayer::player_run(
                    &mut run,
                    players.as_deref_mut(),
                    *owner,
                ) {
                    if player.hp > 0.0 && (!nearest || player.hp < player.stats().max_hp) {
                        targets.push((*owner, flat(*p - pos).length()));
                    }
                }
            }
        }
        targets.sort_by(|a, b| a.1.total_cmp(&b.1));
        for (owner, _) in targets
            .into_iter()
            .take(if nearest { 1 } else { usize::MAX })
        {
            if let Some(player) =
                crate::survivors::multiplayer::player_run(&mut run, players.as_deref_mut(), owner)
            {
                let value = if percent {
                    amount * player.stats().max_hp
                } else {
                    amount
                };
                player.heal(value);
            }
        }
    }
    for (pos, radius) in pushes {
        for (_, mut tf, en) in &mut enemies {
            if !en.dead && !en.boss && flat(tf.translation - pos).length() <= radius {
                let direction = flat(tf.translation - pos).normalize_or(Vec3::X);
                tf.translation += direction * 2.0;
            }
        }
    }
}
