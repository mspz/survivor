//! Owner-local proc counters and payloads. Payload ancestry survives periodic snapshots.
use super::*;
use crate::survivors::{abilities::Hit, data, enemies::Enemy, spell_engine::Resolved, units::flat};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(in crate::survivors) enum Origin {
    #[default]
    Skill,
    Talent,
    Item,
    Legendary,
}
#[derive(Clone, Copy, Debug)]
pub(in crate::survivors) enum Shape {
    Circle(f32),
    Guided(usize, f32),
    Chain(usize, f32),
    Cone(Vec3, f32),
}
#[derive(Clone)]
pub(in crate::survivors) struct Payload {
    pub parent: u64,
    pub unique: u16,
    pub at: f32,
    pub pos: Vec3,
    pub primary: Option<Entity>,
    pub exclude: Option<Entity>,
    pub shape: Shape,
    pub damage: f32,
    pub school: Tags,
    pub dot: f32,
    pub duration: f32,
}
#[derive(Default)]
pub(in crate::survivors) struct State {
    pub serial: u64,
    pub position: Vec3,
    pub attacker: Option<Entity>,
    pub shield_origin: Origin,
    pub queue: Vec<Payload>,
    cooldowns: BTreeMap<u16, f32>,
    buffs: BTreeMap<u16, f32>,
    counters: BTreeMap<u16, u32>,
    names: BTreeMap<&'static str, f32>,
    victims: BTreeSet<Entity>,
    schools: BTreeMap<u64, f32>,
    victim_schools: BTreeMap<Entity, (u64, f32)>,
    last_mode: Option<(bool, f32)>,
    pub moving: bool,
    last_walk: f32,
    stationary_since: f32,
    walked: f32,
    seen: BTreeMap<(u16, u64), f32>,
    random: u64,
}
impl State {
    fn first(&mut self, id: u16, cast: u64, now: f32) -> bool {
        self.seen.retain(|_, at| now - *at < 310.0);
        self.seen.insert((id, cast), now).is_none()
    }
    pub fn buff(&self, id: u16, now: f32) -> bool {
        self.buffs.get(&id).is_some_and(|until| *until > now)
    }
    pub(in crate::survivors) fn active_buffs(
        &self,
        now: f32,
    ) -> impl Iterator<Item = (u16, f32)> + '_ {
        self.buffs
            .iter()
            .filter_map(move |(&id, &until)| (until > now).then_some((id, until - now)))
    }
    fn ready(&mut self, id: u16, now: f32, cooldown: f32) -> bool {
        if self.cooldowns.get(&id).is_some_and(|until| *until > now) {
            return false;
        }
        self.cooldowns.insert(id, now + cooldown);
        true
    }
    fn count(&mut self, id: u16, threshold: u32) -> bool {
        let value = self.counters.entry(id).or_default();
        *value += 1;
        if *value < threshold {
            false
        } else {
            *value = 0;
            true
        }
    }
}
pub(in crate::survivors) fn school(tags: Tags) -> u64 {
    let bits = tags.0 & super::SCHOOLS;
    if bits & (Tags::LIGHTNING | Tags::NATURE).0 != 0 {
        Tags::NATURE.0
    } else {
        bits
    }
}
pub(in crate::survivors) fn dot_conversion(run: &Run, key: &str, tags: Tags, damage: f32) -> f32 {
    let mut increased = run.stats().might - 1.0;
    let mut dot = 0.0;
    let mut more = 1.0;
    for m in run
        .spell_modifiers
        .entries()
        .iter()
        .filter(|m| m.filter.matches(key, tags))
    {
        match m.change {
            Change::IncreasedDamage(v) | Change::IncreasedEffect(v) => increased += v,
            Change::IncreasedDotDamage(v) => dot += v,
            Change::MoreDotDamage(v) => more *= (1.0 + v).max(0.0),
            _ => {}
        }
    }
    damage * ((1.0 + increased + dot).max(0.0) / (1.0 + increased).max(0.0001)) * more
}
pub(in crate::survivors) fn queue(
    run: &mut Run,
    unique: u16,
    pos: Vec3,
    primary: Option<Entity>,
    exclude: Option<Entity>,
    mut shape: Shape,
    mut damage: f32,
    mut school: Tags,
    delay: f32,
) {
    if run.legendary.events.queue.len() >= 256 {
        return;
    }
    if !crate::survivors::procs::ready_effect(
        run,
        "legendary_proc",
        Some(unique),
        Tags::SPELL | Tags::DAMAGE | school,
    ) {
        return;
    }
    if run.legendary.has(70) {
        school = Tags::SHADOW;
    }
    school = if run.legendary.has(94) {
        Tags::FIRE
    } else if run.legendary.has(95) {
        Tags::COLD
    } else if run.legendary.has(96) {
        Tags::PHYSICAL
    } else if run.legendary.has(97) {
        Tags::SHADOW
    } else if run.legendary.has(154) {
        if school.contains(Tags::PHYSICAL) {
            Tags::ARCANE
        } else {
            Tags::PHYSICAL
        }
    } else {
        school
    };
    if (311..=325).contains(&unique) && unique != 321 {
        let key = if school.contains(Tags::PHYSICAL) {
            "proc_bloodburst"
        } else if school.contains(Tags::FIRE) {
            "proc_fireburst"
        } else if school.contains(Tags::COLD) {
            "proc_frostnova"
        } else if school.contains(Tags::HOLY) {
            "proc_holy_burst"
        } else if school.contains(Tags::SHADOW) {
            "proc_plagueburst"
        } else if school.contains(Tags::ARCANE) {
            "proc_arcane_blast"
        } else {
            "proc_totem_bolt"
        };
        let mut def = data::ability(key).clone();
        def.damage = damage
            * super::super::item_level::factor(
                run.legendary.levels.get(&unique).copied().unwrap_or(1),
            );
        let form = match shape {
            Shape::Circle(r) | Shape::Cone(_, r) => {
                def.radius = r;
                Tags::AREA
            }
            Shape::Guided(..) => Tags::PROJECTILE,
            Shape::Chain(..) => Tags::CHAIN,
        };
        let p = crate::survivors::spell_engine::resolve_tagged(
            &def,
            1,
            &run.stats(),
            run.spell_modifiers.entries(),
            0.0,
            Tags::SPELL | Tags::DAMAGE | school | form,
        );
        damage = p.dmg * run.balance.multiplier_tagged(run.class, key, p.tags);
        shape = match shape {
            Shape::Circle(_) => Shape::Circle(p.radius),
            Shape::Cone(d, _) => Shape::Cone(d, p.radius),
            other => other,
        };
    }
    let converted = run.legendary.has(70) && damage > 0.0;
    let dot = if converted {
        dot_conversion(
            run,
            "proc_plague",
            Tags::SPELL | Tags::DAMAGE | Tags::DOT | school,
            damage,
        ) / 3.0
    } else {
        0.0
    };
    let parent = if run.proc_runtime.parent == 0 {
        crate::survivors::procs::fresh(run)
    } else {
        run.proc_runtime.parent
    };
    run.legendary.events.queue.push(Payload {
        parent,
        unique,
        at: run.elapsed + delay,
        pos,
        primary,
        exclude,
        shape,
        damage: if converted { 0.0 } else { damage },
        school,
        dot,
        duration: if converted { 3.0 } else { 0.0 },
    });
}
fn ready(run: &mut Run, id: u16, base: f32) -> bool {
    let cd =
        crate::survivors::procs::cooldown(run, "legendary_proc", Tags::SPELL | Tags::DAMAGE, base);
    run.legendary.events.ready(id, run.elapsed, cd)
}
fn arm(run: &mut Run, id: u16, duration: f32, cooldown: f32) {
    if run.legendary.has(id) && ready(run, id, cooldown) {
        run.legendary
            .events
            .buffs
            .insert(id, run.elapsed + duration);
    }
}
pub(in crate::survivors) fn direct(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Bolt | Kind::Strike | Kind::Melee | Kind::Nova | Kind::Cone | Kind::Chain
    )
}
/// Preview charge effects; a failed cast cannot consume them.
pub(in crate::survivors) fn modify(run: &Run, def: &AbilityDef, p: &mut Resolved) {
    let s = &run.legendary.events;
    let now = run.elapsed;
    if s.buff(328, now) && p.tags.contains(Tags::DAMAGE) {
        p.cooldown /= 1.25;
    }
    if s.buff(332, now) && p.tags.contains(Tags::AREA) {
        p.radius = (p.radius * 1.4_f32.sqrt()).min(100.0);
    }
    if s.buff(333, now) {
        p.crit_multiplier += 0.5;
    }
    if s.buff(334, now) {
        p.dot *= 1.25;
    }
    if s.buff(335, now) {
        p.crit = (p.crit + 0.15).min(1.0);
    }
    if s.buff(340, now) && p.tags.contains(Tags::GUARD) {
        p.cooldown /= 1.25;
    }
    p.primary_critical = s.buff(326, now) && p.tags.contains(Tags::PROJECTILE | Tags::DAMAGE);
    if s.buff(329, now)
        && matches!(def.kind, Kind::Melee | Kind::Strike | Kind::Dot)
        && p.tags.contains(Tags::ATTACK | Tags::MELEE)
    {
        p.count = (p.count + 2).min(64);
    }
    if s.buff(336, now) && direct(def.kind) && p.tags.contains(Tags::DAMAGE) {
        p.recast += 1.0;
    }
    if s.buff(339, now) && p.tags.intersects(Tags::PROJECTILE | Tags::CHAIN) {
        p.chains = (p.chains + 1).min(32);
        if def.kind == Kind::Chain {
            p.count = (p.count + 1).min(32);
        }
    }
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
    crate::survivors::procs::parent(run, p.cast_id);
    super::effects::cast(run, def, p, primary, repeated);
    let now = run.elapsed;
    let pos = primary.map_or(run.legendary.events.position, |p| p.1);
    let victim = primary.map(|p| p.0);
    let tags = p.tags;
    if p.dmg > 0.0
        && run.legendary.has(311)
        && direct(def.kind)
        && tags.contains(Tags::DAMAGE)
        && run.legendary.events.count(311, 5)
    {
        queue(
            run,
            311,
            pos,
            victim,
            None,
            Shape::Circle(5.0),
            35.0,
            Tags::PHYSICAL,
            0.0,
        );
    }
    if run.legendary.events.buff(326, now) && p.tags.contains(Tags::PROJECTILE | Tags::DAMAGE) {
        run.legendary.events.buffs.remove(&326);
    }
    if run.legendary.has(326) && tags.contains(Tags::ATTACK) && run.legendary.events.count(326, 5) {
        run.legendary.events.buffs.insert(326, now + 6.0);
    }
    if run.legendary.has(328) && tags.contains(Tags::DAMAGE) {
        run.legendary.events.names.retain(|_, at| now - *at <= 6.0);
        run.legendary.events.names.insert(def.key, now);
        if run.legendary.events.names.len() >= 3 && ready(run, 328, 8.0) {
            run.legendary.events.buffs.insert(328, now + 4.0);
            run.legendary.events.names.clear();
        }
    }
    if run.legendary.has(332) && tags.contains(Tags::DAMAGE) {
        let attack = tags.contains(Tags::ATTACK);
        let spell = tags.contains(Tags::SPELL);
        if attack != spell {
            if run
                .legendary
                .events
                .last_mode
                .is_some_and(|(previous, at)| previous != attack && now - at <= 4.0)
            {
                arm(run, 332, 4.0, 8.0);
            }
            run.legendary.events.last_mode = Some((attack, now));
        }
    }
    if run.legendary.has(336)
        && matches!(def.kind, Kind::Shield | Kind::Immune | Kind::Thorns)
        && run.legendary.events.count(336, 5)
    {
        run.legendary.events.buffs.insert(336, now + 6.0);
    } else if direct(def.kind) && tags.contains(Tags::DAMAGE) {
        run.legendary.events.buffs.remove(&336);
    }
    if matches!(def.kind, Kind::Melee | Kind::Strike | Kind::Dot)
        && tags.contains(Tags::ATTACK | Tags::MELEE)
    {
        run.legendary.events.buffs.remove(&329);
    }
    if tags.intersects(Tags::PROJECTILE | Tags::CHAIN) {
        run.legendary.events.buffs.remove(&339);
    }
    if run.legendary.has(324) && tags.contains(Tags::MELEE) && run.legendary.events.buff(324, now) {
        run.legendary.events.buffs.remove(&324);
        let direction = flat(pos - run.legendary.events.position).normalize_or(Vec3::NEG_Z);
        queue(
            run,
            324,
            pos,
            victim,
            None,
            Shape::Cone(direction, 8.0),
            45.0,
            Tags::FIRE,
            0.0,
        );
    }
    run.proc_runtime.parent = 0;
}
/// Primary hit counters are keyed by the successful activation, never by volley size.
pub(in crate::survivors) fn hit(
    run: &mut Run,
    h: &Hit,
    tags: Tags,
    pos: Vec3,
    effective: f32,
    full: bool,
    newly_rooted: bool,
    rng: &mut Rng,
) {
    if !h.can_proc() {
        return;
    }
    let now = run.elapsed;
    if newly_rooted && run.legendary.has(318) && ready(run, 318, 3.0) {
        queue(
            run,
            318,
            pos,
            None,
            None,
            Shape::Circle(4.0),
            30.0,
            Tags::PHYSICAL,
            0.0,
        );
    }
    if effective <= 0.0 {
        return;
    }
    if run.legendary.has(312) && h.crit && rng.f32() < 0.25 && ready(run, 312, 2.0) {
        queue(
            run,
            312,
            pos,
            None,
            Some(h.target),
            Shape::Guided(3, 20.0),
            18.0,
            Tags::COLD,
            0.0,
        );
    }
    if h.primary
        && h.cast_id != 0
        && tags.contains(Tags::PROJECTILE)
        && run.legendary.has(317)
        && run.legendary.events.first(317, h.cast_id, now)
    {
        run.legendary.events.victims.insert(h.target);
        if run.legendary.events.victims.len() >= 5 {
            run.legendary.events.victims.clear();
            queue(
                run,
                317,
                run.legendary.events.position,
                None,
                None,
                Shape::Guided(1, 24.0),
                60.0,
                Tags::ARCANE,
                0.0,
            );
        }
    }
    if run.legendary.has(319) {
        run.legendary
            .events
            .schools
            .retain(|_, at| now - *at <= 8.0);
        run.legendary.events.schools.insert(school(tags), now);
        if run.legendary.events.schools.len() >= 3 && ready(run, 319, 6.0) {
            run.legendary.events.schools.clear();
            queue(
                run,
                319,
                pos,
                None,
                None,
                Shape::Circle(6.0),
                45.0,
                Tags::HOLY,
                0.0,
            );
        }
    }
    if run.legendary.has(320)
        && h.primary
        && h.cast_id != 0
        && tags.contains(Tags::AREA)
        && run.legendary.events.first(320, h.cast_id, now)
        && rng.f32() < 0.15
        && ready(run, 320, 3.0)
    {
        queue(
            run,
            320,
            pos,
            None,
            None,
            Shape::Circle(4.0),
            30.0,
            Tags::ARCANE,
            1.0,
        );
    }
    if run.legendary.has(323) && full && ready(run, 323, 2.0) {
        let index = run.legendary.events.queue.len();
        queue(
            run,
            323,
            pos,
            Some(h.target),
            None,
            Shape::Guided(1, 0.0),
            0.0,
            Tags::NATURE,
            0.0,
        );
        if let Some(payload) = run.legendary.events.queue.get(index) {
            let tags = Tags::SPELL | Tags::DOT | Tags::DAMAGE | Tags::DURATION | payload.school;
            let mut def = data::ability("proc_poison").clone();
            def.damage = 0.0;
            def.dot = 8.0
                * super::super::item_level::factor(
                    run.legendary.levels.get(&323).copied().unwrap_or(1),
                );
            def.duration = 4.0;
            let p = crate::survivors::spell_engine::resolve_tagged(
                &def,
                1,
                &run.stats(),
                run.spell_modifiers.entries(),
                0.0,
                tags,
            );
            let factor = run
                .balance
                .multiplier_tagged(run.class, "proc_poison", p.tags);
            let payload = &mut run.legendary.events.queue[index];
            payload.dot = p.dot * factor;
            payload.duration = p.duration;
        }
    }
    if h.primary
        && h.cast_id != 0
        && direct(data::ability(h.skill).kind)
        && run.legendary.has(333)
        && run.legendary.events.first(333, h.cast_id, now)
    {
        if h.crit {
            run.legendary.events.counters.insert(333, 0);
            run.legendary.events.buffs.remove(&333);
        } else if run.legendary.events.count(333, 3) {
            run.legendary.events.buffs.insert(333, now + 4.0);
        }
    }
    if run.legendary.has(339) {
        let current = school(tags);
        if run
            .legendary
            .events
            .victim_schools
            .insert(h.target, (current, now))
            .is_some_and(|(old, at)| old != current && now - at <= 3.0)
        {
            arm(run, 339, 6.0, 5.0);
        }
    }
}
pub(in crate::survivors) fn dot_applied(
    run: &mut Run,
    _victim: Entity,
    _tags: Tags,
    origin: Origin,
    other_school: bool,
) {
    if origin != Origin::Skill || !run.legendary.has(334) {
        return;
    }
    if other_school {
        arm(run, 334, 6.0, 8.0);
    }
}
pub(in crate::survivors) fn expired(run: &mut Run, pos: Vec3, origin: Origin) {
    if origin != Origin::Skill {
        return;
    }
    if run.legendary.has(314) && ready(run, 314, 4.0) {
        queue(
            run,
            314,
            pos,
            None,
            None,
            Shape::Circle(5.0),
            30.0,
            Tags::SHADOW,
            0.0,
        );
    }
    arm(run, 327, 4.0, 8.0);
}
pub(in crate::survivors) fn killed(
    run: &mut Run,
    victim: Entity,
    pos: Vec3,
    origin: Origin,
    rng: &mut Rng,
) {
    run.legendary.events.victim_schools.remove(&victim);
    if origin != Origin::Skill {
        return;
    }
    if run.legendary.has(313) && rng.f32() < 0.3 && ready(run, 313, 2.0) {
        queue(
            run,
            313,
            pos,
            None,
            None,
            Shape::Circle(5.0),
            35.0,
            Tags::FIRE,
            0.6,
        );
    }
    if run.legendary.has(325) && run.hp < run.stats().max_hp * 0.35 && ready(run, 325, 5.0) {
        queue(
            run,
            325,
            pos,
            None,
            Some(victim),
            Shape::Guided(3, 20.0),
            22.0,
            Tags::SHADOW,
            0.0,
        );
    }
}
pub(in crate::survivors) fn blocked(run: &mut Run) {
    if run.legendary.has(315) && ready(run, 315, 3.0) {
        queue(
            run,
            315,
            run.legendary.events.position,
            run.legendary.events.attacker,
            None,
            Shape::Chain(3, 20.0),
            24.0,
            Tags::LIGHTNING | Tags::NATURE,
            0.0,
        );
    }
    if run.legendary.has(329) {
        run.legendary.events.buffs.insert(329, run.elapsed + 5.0);
    }
}
pub(in crate::survivors) fn hurt(run: &mut Run, before: f32, absorbed: f32, taken: f32) {
    super::effects::hurt(run, taken);
    if taken <= 0.0 {
        if absorbed > 0.0 {
            arm(run, 335, 5.0, 8.0);
        }
        return;
    }
    let max = run.stats().max_hp;
    if before > max * 0.35 && run.hp < max * 0.35 {
        arm(run, 331, 3.0, 20.0);
    }
    if taken > max * 0.15 && run.legendary.has(337) && ready(run, 337, 15.0) {
        run.mobile.vanish_t = run.mobile.vanish_t.max(1.0);
    }
    // Deterministic stream lives on the run, separate from the global damage-roll stream.
    let mut random = if run.legendary.events.random == 0 {
        0x9e3779b97f4a7c15
    } else {
        run.legendary.events.random
    };
    random ^= random >> 12;
    random ^= random << 25;
    random ^= random >> 27;
    run.legendary.events.random = random;
    let roll = (random.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 32) as u32;
    if run.legendary.has(316) && roll < u32::MAX / 4 && ready(run, 316, 3.0) {
        if let Some(attacker) = run.legendary.events.attacker {
            queue(
                run,
                316,
                run.legendary.events.position,
                Some(attacker),
                None,
                Shape::Guided(1, 20.0),
                24.0,
                Tags::NATURE,
                0.0,
            );
        }
    }
}
pub(in crate::survivors) fn shield_expired(run: &mut Run, remaining: f32) {
    super::effects::shield_expired(run, remaining, false);
    if remaining <= 0.0 || run.legendary.events.shield_origin != Origin::Skill {
        return;
    }
    if run.legendary.has(321) && ready(run, 321, 5.0) {
        queue(
            run,
            321,
            run.legendary.events.position,
            None,
            None,
            Shape::Guided(3, 24.0),
            remaining.min(run.stats().max_hp * 0.25),
            Tags::PHYSICAL,
            0.0,
        );
    }
}
pub(in crate::survivors) fn mobility(run: &mut Run, pos: Vec3) {
    run.legendary.events.position = pos;
    if run.legendary.has(322) && ready(run, 322, 10.0) {
        queue(
            run,
            322,
            pos,
            None,
            None,
            Shape::Circle(6.0),
            40.0,
            Tags::SHADOW,
            0.3,
        );
    }
    if run.shield <= 0.0 || run.shield_t <= 0.0 {
        if run.legendary.has(330) && ready(run, 330, 10.0) {
            run.shield = run.stats().max_hp * 0.15;
            run.shield_t = 3.0;
            run.legendary.events.shield_origin = Origin::Legendary;
        }
    }
}
pub(in crate::survivors) fn walk(run: &mut Run, from: Vec3, to: Vec3, dt: f32) {
    let distance = flat(to - from).length();
    let now = run.elapsed;
    run.legendary.events.position = to;
    let moving =
        distance > 0.0001 && distance <= run.stats().speed * run.haste_mult() * dt.max(0.0) + 0.1;
    if moving {
        let heading = flat(to - from).normalize_or_zero();
        if run.legendary.periodic.heading.length_squared() < 0.1 {
            run.legendary.periodic.heading = heading;
            run.legendary.casting.last_heading = heading;
        } else if run.legendary.periodic.heading.dot(heading) < 0.5 {
            run.legendary.periodic.turn = true;
            run.legendary.periodic.heading = heading;
            run.legendary.casting.last_heading = heading;
        }
        if !run.legendary.events.moving
            && now - run.legendary.events.stationary_since >= 3.0
            && run.legendary.has(338)
        {
            run.legendary.events.buffs.insert(338, now + 4.0);
        }
        run.legendary.events.walked += distance;
        if run.legendary.has(324) && run.legendary.events.walked >= 10.0 {
            run.legendary.events.walked = 0.0;
            run.legendary.events.buffs.insert(324, f32::INFINITY);
        }
        run.legendary.events.last_walk = now;
    } else if run.legendary.events.moving {
        run.legendary.events.stationary_since = now;
    }
    run.legendary.events.moving = moving;
}
pub(in crate::survivors) fn active_heal(run: &mut Run, amount: f32) -> f32 {
    let healed = receive_active_heal(run, amount);
    run.healing_meter
        .record_hit("self_healing", run.elapsed, healed, false, false);
    healed
}
pub(in crate::survivors) fn receive_active_heal(run: &mut Run, amount: f32) -> f32 {
    if super::effects::active_heal(run, amount) {
        return 0.0;
    }
    let before = run.hp;
    let healed = run.recover(amount);
    let max = run.stats().max_hp;
    if before < max * 0.8 && run.hp >= max * 0.8 {
        arm(run, 340, 6.0, 10.0);
    }
    healed
}
pub(in crate::survivors) fn reduction(run: &Run) -> f32 {
    if run.legendary.events.buff(327, run.elapsed) {
        0.25
    } else {
        0.0
    }
}
pub(in crate::survivors) fn regeneration(run: &Run) -> f32 {
    if run.legendary.events.buff(331, run.elapsed) {
        run.stats().max_hp * 0.08
    } else {
        0.0
    }
}
pub(in crate::survivors) fn pickup(run: &Run) -> f32 {
    if run.legendary.events.buff(338, run.elapsed) {
        3.0
    } else {
        1.0
    }
}
pub(in crate::survivors) fn equip(run: &mut Run) {
    let worn: BTreeSet<_> = run.legendary.equipped.keys().copied().collect();
    let state = &mut run.legendary.events;
    state.buffs.retain(|id, _| worn.contains(id));
    state.cooldowns.retain(|id, _| worn.contains(id));
    state.counters.retain(|id, _| worn.contains(id));
    state.queue.retain(|p| worn.contains(&p.unique));
    state.names.clear();
    state.victims.clear();
    state.schools.clear();
    state.victim_schools.clear();
}

/// Payloads reselect targets at execution and carry a non-recursive origin through kills/DoTs.
pub(in crate::survivors) fn tick(
    mut run: ResMut<Run>,
    mut players: Option<ResMut<crate::survivors::multiplayer::Companions>>,
    enemies: Query<(Entity, &Transform, &Enemy)>,
    mut hits: MessageWriter<Hit>,
) {
    let now = run.elapsed;
    let mut ready = Vec::new();
    let mut collect = |run: &mut Run| {
        let owner = run.owner;
        let mut pending = Vec::new();
        for payload in std::mem::take(&mut run.legendary.events.queue) {
            if !run.legendary.has(payload.unique) || run.dead_t.is_some() {
                continue;
            }
            if payload.at <= now {
                ready.push((owner, payload));
            } else {
                pending.push(payload);
            }
        }
        run.legendary.events.queue = pending;
    };
    collect(&mut run);
    if let Some(players) = players.as_deref_mut() {
        for player in players.0.values_mut() {
            collect(&mut player.run);
        }
    }
    for (owner, p) in ready {
        let mut candidates: Vec<_> = enemies
            .iter()
            .filter(|(e, _, en)| !en.dead && Some(*e) != p.exclude)
            .map(|(e, tf, en)| (e, tf.translation, en.hp))
            .collect();
        candidates.sort_by(|a, b| {
            flat(a.1 - p.pos)
                .length_squared()
                .total_cmp(&flat(b.1 - p.pos).length_squared())
                .then_with(|| a.0.to_bits().cmp(&b.0.to_bits()))
        });
        let targets: Vec<_> = match p.shape {
            Shape::Circle(radius) => candidates
                .into_iter()
                .filter(|(_, pos, _)| flat(*pos - p.pos).length() <= radius)
                .map(|(e, _, _)| (e, 1.0))
                .collect(),
            Shape::Cone(direction, range) => candidates
                .into_iter()
                .filter(|(_, pos, _)| {
                    let d = flat(*pos - p.pos);
                    d.length() <= range
                        && (d.length() < 0.5 || d.normalize_or_zero().dot(direction) >= 0.8)
                })
                .map(|(e, _, _)| (e, 1.0))
                .collect(),
            Shape::Chain(count, range) => {
                let mut selected = Vec::new();
                let mut center = p.pos;
                for index in 0..count {
                    let next = if index == 0 {
                        candidates
                            .iter()
                            .filter(|(_, pos, _)| flat(*pos - center).length() <= range)
                            .min_by(|a, b| {
                                (Some(a.0) != p.primary)
                                    .cmp(&(Some(b.0) != p.primary))
                                    .then_with(|| {
                                        flat(a.1 - center)
                                            .length_squared()
                                            .total_cmp(&flat(b.1 - center).length_squared())
                                    })
                            })
                    } else {
                        candidates
                            .iter()
                            .filter(|(e, pos, _)| {
                                !selected.iter().any(|(old, _)| old == e)
                                    && flat(*pos - center).length() <= 8.0
                            })
                            .min_by(|a, b| {
                                flat(a.1 - center)
                                    .length_squared()
                                    .total_cmp(&flat(b.1 - center).length_squared())
                            })
                    };
                    if let Some((e, pos, _)) = next {
                        selected.push((*e, 0.85_f32.powi(index as i32)));
                        center = *pos;
                    } else {
                        break;
                    }
                }
                selected
            }
            Shape::Guided(count, range) => {
                if p.primary
                    .is_some_and(|primary| !candidates.iter().any(|(e, _, _)| *e == primary))
                {
                    continue;
                }
                candidates.retain(|(e, pos, _)| {
                    if range == 0.0 {
                        Some(*e) == p.primary
                    } else {
                        flat(*pos - p.pos).length() <= range
                    }
                });
                if p.unique == 317 {
                    candidates.sort_by(|a, b| b.2.total_cmp(&a.2));
                }
                if let Some(primary) = p.primary {
                    candidates.sort_by_key(|(e, _, _)| *e != primary);
                }
                let split = if matches!(p.unique, 143 | 236 | 321) {
                    candidates.len().min(count).max(1) as f32
                } else {
                    1.0
                };
                candidates
                    .into_iter()
                    .take(count)
                    .enumerate()
                    .map(|(i, (e, _, _))| {
                        (
                            e,
                            if matches!(p.shape, Shape::Chain(..)) {
                                0.85_f32.powi(i as i32)
                            } else {
                                1.0 / split
                            },
                        )
                    })
                    .collect()
            }
        };
        let skill = if p.school.contains(Tags::PHYSICAL) {
            "proc_bloodburst"
        } else if p.school.contains(Tags::FIRE) {
            "proc_fireburst"
        } else if p.school.contains(Tags::COLD) {
            "frostbolt"
        } else if p.school.contains(Tags::HOLY) {
            "holy_nova"
        } else if p.school.contains(Tags::SHADOW) {
            "shadow_bolt"
        } else if p.school.contains(Tags::ARCANE) {
            "arcane_missiles"
        } else {
            "lightning_bolt"
        };
        for (target, scale) in targets.into_iter().take(64) {
            let mut hit = Hit::direct(skill, target, p.damage * scale, false);
            hit.owner = owner;
            hit.item_source = Some(p.unique);
            hit.cast_id = p.parent;
            hit.origin = Origin::Legendary;
            hit.tags = Some(
                Tags::SPELL
                    | Tags::DAMAGE
                    | p.school
                    | match p.shape {
                        Shape::Circle(_) | Shape::Cone(..) => Tags::AREA,
                        Shape::Guided(..) => Tags::PROJECTILE,
                        Shape::Chain(..) => Tags::CHAIN,
                    }
                    | if p.dot > 0.0 {
                        Tags::DOT | Tags::DURATION
                    } else {
                        Tags(0)
                    },
            );
            hit.meter_source = definition(p.unique).map(|d| d.name);
            hit.triggered = true;
            hit.proc_admitted = true;
            hit.proc_resolved = true;
            hit.proc_chain = u64::MAX;
            hit.dot = p.dot;
            hit.dot_dur = p.duration;
            hits.write(hit);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn owner(id: u16) -> Run {
        let mut run = Run {
            balance: crate::survivors::balance::Config::parse("version=1\n[classes]\n[tags]\n")
                .unwrap(),
            ..default()
        };
        run.legendary.equipped.insert(
            id,
            Roll {
                unique: id,
                milestone: None,
            },
        );
        run
    }
    fn params(run: &Run, key: &str) -> Resolved {
        run.resolve_spell(data::ability(key), 1)
    }
    fn activation(run: &mut Run, key: &'static str) {
        let def = data::ability(key);
        let p = params(run, key);
        cast(run, def, &p, Some((Entity::from_bits(1), Vec3::X)), false);
    }
    fn primary(cast: u64, crit: bool) -> Hit {
        let mut hit = Hit::direct("fireball", Entity::from_bits(1), 20.0, crit);
        hit.primary = true;
        hit.cast_id = cast;
        hit
    }
    #[test]
    fn every_authored_proc_has_an_observable_combat_effect() {
        for id in 311..=340 {
            let mut run = owner(id);
            let mut rng = Rng(0);
            match id {
                311 => {
                    for _ in 0..5 {
                        activation(&mut run, "heroic_strike");
                    }
                }
                312 => hit(
                    &mut run,
                    &primary(1, true),
                    Tags::SPELL | Tags::FIRE,
                    Vec3::X,
                    20.0,
                    false,
                    false,
                    &mut rng,
                ),
                313 => killed(
                    &mut run,
                    Entity::from_bits(1),
                    Vec3::X,
                    Origin::Skill,
                    &mut rng,
                ),
                314 | 327 => expired(&mut run, Vec3::X, Origin::Skill),
                315 | 329 => blocked(&mut run),
                316 => {
                    run.legendary.events.attacker = Some(Entity::from_bits(2));
                    for _ in 0..128 {
                        run.elapsed += 3.1;
                        let before = run.hp + 5.0;
                        hurt(&mut run, before, 0.0, 5.0);
                    }
                }
                317 => {
                    for i in 1..=5 {
                        let mut h = primary(i, false);
                        h.target = Entity::from_bits(i);
                        hit(
                            &mut run,
                            &h,
                            Tags::SPELL | Tags::PROJECTILE | Tags::FIRE,
                            Vec3::X,
                            20.0,
                            false,
                            false,
                            &mut rng,
                        );
                    }
                }
                318 => hit(
                    &mut run,
                    &primary(1, false),
                    Tags::SPELL | Tags::NATURE,
                    Vec3::X,
                    0.0,
                    false,
                    true,
                    &mut rng,
                ),
                319 => {
                    for school in [Tags::PHYSICAL, Tags::FIRE, Tags::ARCANE] {
                        hit(
                            &mut run,
                            &primary(1, false),
                            Tags::SPELL | school,
                            Vec3::X,
                            20.0,
                            false,
                            false,
                            &mut rng,
                        );
                    }
                }
                320 => hit(
                    &mut run,
                    &primary(1, false),
                    Tags::SPELL | Tags::AREA | Tags::ARCANE,
                    Vec3::X,
                    20.0,
                    false,
                    false,
                    &mut rng,
                ),
                321 => shield_expired(&mut run, 40.0),
                322 | 330 => mobility(&mut run, Vec3::X),
                323 => hit(
                    &mut run,
                    &primary(1, false),
                    Tags::SPELL | Tags::FIRE,
                    Vec3::X,
                    20.0,
                    true,
                    false,
                    &mut rng,
                ),
                324 => {
                    for i in 0..10 {
                        run.elapsed += 1.0;
                        walk(&mut run, Vec3::X * i as f32, Vec3::X * (i + 1) as f32, 1.0);
                    }
                    activation(&mut run, "heroic_strike");
                }
                325 => {
                    run.hp = 20.0;
                    killed(
                        &mut run,
                        Entity::from_bits(1),
                        Vec3::X,
                        Origin::Skill,
                        &mut rng,
                    );
                }
                326 => {
                    for _ in 0..5 {
                        activation(&mut run, "heroic_strike");
                    }
                }
                328 => {
                    for key in ["fireball", "frostbolt", "arcane_missiles"] {
                        activation(&mut run, key);
                    }
                }
                331 => {
                    let max = run.stats().max_hp;
                    run.hp = max * 0.3;
                    hurt(&mut run, max * 0.5, 0.0, max * 0.2);
                }
                332 => {
                    activation(&mut run, "heroic_strike");
                    activation(&mut run, "fireball");
                }
                333 => {
                    for i in 1..=3 {
                        hit(
                            &mut run,
                            &primary(i, false),
                            Tags::SPELL | Tags::FIRE,
                            Vec3::X,
                            20.0,
                            false,
                            false,
                            &mut rng,
                        );
                    }
                }
                334 => dot_applied(
                    &mut run,
                    Entity::from_bits(1),
                    Tags::FIRE,
                    Origin::Skill,
                    true,
                ),
                335 => {
                    let before = run.hp;
                    hurt(&mut run, before, 10.0, 0.0);
                }
                336 => {
                    for _ in 0..5 {
                        activation(&mut run, "ice_barrier");
                    }
                }
                337 => {
                    let max = run.stats().max_hp;
                    let before = run.hp;
                    hurt(&mut run, before, 0.0, max * 0.2);
                }
                338 => {
                    run.elapsed = 3.1;
                    walk(&mut run, Vec3::ZERO, Vec3::X, 1.0);
                }
                339 => {
                    for school in [Tags::FIRE, Tags::COLD] {
                        hit(
                            &mut run,
                            &primary(1, false),
                            Tags::SPELL | school,
                            Vec3::X,
                            20.0,
                            false,
                            false,
                            &mut rng,
                        );
                    }
                }
                340 => {
                    let max = run.stats().max_hp;
                    run.hp = max * 0.7;
                    active_heal(&mut run, max * 0.2);
                }
                _ => unreachable!(),
            }
            assert!(
                !run.legendary.events.queue.is_empty()
                    || !run.legendary.events.buffs.is_empty()
                    || run.shield > 0.0
                    || run.mobile.vanish_t > 0.0,
                "unique {id} produced no effect"
            );
        }
    }
    #[test]
    fn repeated_casts_and_collateral_hits_do_not_fill_activation_counters() {
        let mut run = owner(311);
        let p = params(&run, "heroic_strike");
        for _ in 0..10 {
            cast(&mut run, data::ability("heroic_strike"), &p, None, true);
        }
        assert!(run.legendary.events.queue.is_empty());
        for _ in 0..4 {
            activation(&mut run, "heroic_strike");
        }
        assert!(run.legendary.events.queue.is_empty());
        activation(&mut run, "heroic_strike");
        assert_eq!(run.legendary.events.queue.len(), 1);
        let mut run = owner(317);
        let mut rng = Rng(0);
        for target in 1..20 {
            let mut h = primary(1, false);
            h.primary = false;
            h.target = Entity::from_bits(target);
            hit(
                &mut run,
                &h,
                Tags::SPELL | Tags::PROJECTILE | Tags::FIRE,
                Vec3::ZERO,
                20.0,
                false,
                false,
                &mut rng,
            );
        }
        assert!(run.legendary.events.queue.is_empty());
    }
    #[test]
    fn legendary_damage_and_dot_expirations_cannot_create_another_proc() {
        let mut run = owner(312);
        run.legendary.equipped.insert(
            314,
            Roll {
                unique: 314,
                milestone: None,
            },
        );
        run.legendary.equipped.insert(
            313,
            Roll {
                unique: 313,
                milestone: None,
            },
        );
        let mut rng = Rng(0);
        let mut h = primary(1, true);
        h.origin = Origin::Legendary;
        hit(
            &mut run,
            &h,
            Tags::SPELL | Tags::FIRE,
            Vec3::ZERO,
            20.0,
            true,
            true,
            &mut rng,
        );
        expired(&mut run, Vec3::ZERO, Origin::Legendary);
        killed(&mut run, h.target, Vec3::ZERO, Origin::Legendary, &mut rng);
        assert!(run.legendary.events.queue.is_empty());
        assert!(run.legendary.events.buffs.is_empty());
    }
    #[test]
    fn shield_procs_do_not_replace_an_active_shield_or_retrigger_expiration() {
        let mut run = owner(330);
        run.shield = 99.0;
        run.shield_t = 8.0;
        mobility(&mut run, Vec3::ZERO);
        assert_eq!(run.shield, 99.0);
        assert_eq!(run.shield_t, 8.0);
        run.shield = 0.0;
        run.shield_t = 0.0;
        mobility(&mut run, Vec3::ZERO);
        assert_eq!(run.legendary.events.shield_origin, Origin::Legendary);
        run.legendary.equipped.insert(
            321,
            Roll {
                unique: 321,
                milestone: None,
            },
        );
        let remaining = run.shield;
        shield_expired(&mut run, remaining);
        assert!(run.legendary.events.queue.is_empty());
    }
    #[test]
    fn cooldowns_are_owner_local_and_buffs_expire_on_the_run_clock() {
        let mut a = owner(327);
        let mut b = owner(327);
        expired(&mut a, Vec3::ZERO, Origin::Skill);
        expired(&mut b, Vec3::ZERO, Origin::Skill);
        assert_eq!(reduction(&a), 0.25);
        assert_eq!(reduction(&b), 0.25);
        a.elapsed = 4.1;
        assert_eq!(reduction(&a), 0.0);
        assert_eq!(reduction(&b), 0.25);
        expired(&mut a, Vec3::ZERO, Origin::Skill);
        assert_eq!(reduction(&a), 0.0);
        a.elapsed = 8.0;
        expired(&mut a, Vec3::ZERO, Origin::Skill);
        assert_eq!(reduction(&a), 0.25);
    }
}
