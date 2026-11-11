//! Stateful milestones live on each player's Run, including on the host for guests.
use super::procs;
use super::{
    abilities::{Hit, PendingHits},
    data,
    enemies::{Dot, Enemy},
    hero::Hero,
    multiplayer::{Companions, RemoteOwner},
    spell_engine::{tags, Tags},
    talent_design::{self, Mechanic as M},
    units::{self, Fx, UnitSpec},
    Owned, Rng, Run,
};
use bevy::prelude::*;

#[derive(Default)]
pub(super) struct Runtime {
    pub minions: super::minions::Runtime,
    enabled: Vec<M>,
    target: Option<Entity>,
    streak: u32,
    impacts: [u32; 4],
    pub shield_capacity: f32,
    pub shield_burst: f32,
    pub blocked: bool,
    last_block: f32,
    pulse: [f32; 3],
    summon: f32,
    healing: f32,
    party_pulse: bool,
    bursts: Vec<Burst>,
}
struct Burst {
    created: f32,
    parent: u64,
    chain: u64,
    pos: Vec3,
    damage: f32,
    exclude: Option<Entity>,
    radius: f32,
    targets: usize,
    dots: Vec<Dot>,
    freeze: bool,
    skill: &'static str,
    meter_source: Option<&'static str>,
    spread_source: Option<&'static str>,
}
#[derive(Component)]
pub(super) struct Taunted {
    pub owner: Option<u64>,
    pub until: f32,
}
impl Taunted {
    pub(super) fn damage_multiplier(&self, now: f32) -> f32 {
        if self.until > now { 1.25 } else { 1.0 }
    }
}
#[derive(Component)]
pub(super) struct Summon {
    family: u8,
    owner: Option<u64>,
    slot: u8,
    cooldown: f32,
    stationary: bool,
    target: Option<Entity>,
    retarget: f32,
    carried: Option<Dot>,
    carry_until: f32,
    harvested: Option<Entity>,
    charge: Option<(Entity, super::spell_engine::Resolved)>,
    protecting: bool,
}
impl Summon {
    pub(super) fn owned_pet(&self, owner: Option<u64>) -> bool {
        (self.family == 0 || self.family == 3) && self.owner == owner
    }
    pub(super) fn owned_totem(&self, owner: Option<u64>) -> bool {
        self.family == 2 && self.stationary && self.owner == owner
    }
}

const PET_LEASH: f32 = 100.0;
fn within_pet_leash(position: Vec3, owner: Vec3) -> bool {
    position.distance_squared(owner) <= PET_LEASH * PET_LEASH
}

/// Bosses first, then proximity biased toward the owner, within the owner's leash.
fn pet_target(
    foes: impl Iterator<Item = (Entity, Vec3, bool)>,
    origin: Vec3,
    owner_pos: Vec3,
) -> Option<Entity> {
    let owner_direction = units::flat(owner_pos - origin).normalize_or_zero();
    let score = |position: Vec3| {
        let delta = units::flat(position - origin);
        let alignment = delta.normalize_or_zero().dot(owner_direction).max(0.0);
        // A target toward the owner counts as up to 35% closer. Nearby foes still matter.
        delta.length() * (1.0 - 0.35 * alignment)
    };
    foes.filter(|(_, position, _)| within_pet_leash(*position, owner_pos))
        .min_by(|(a_entity, a, a_boss), (b_entity, b, b_boss)| {
            b_boss
                .cmp(a_boss)
                .then_with(|| score(*a).total_cmp(&score(*b)))
                .then(a_entity.to_bits().cmp(&b_entity.to_bits()))
        })
        .map(|(entity, _, _)| entity)
}
pub(super) fn active(run: &Run, m: M) -> bool {
    run.talent_runtime.enabled.contains(&m) || super::items::legendary::milestones::active(run, m)
}
fn chain_bit(m: M) -> u64 {
    1u64 << (m as u32)
}
/// Only authored damage tags decide hit eligibility, independently of ATTACK/SPELL or visuals.
fn hit_tags(m: M) -> Tags {
    Tags::DAMAGE
        | match m {
            M::Bleed => Tags::PHYSICAL,
            M::Ignite => Tags::FIRE,
            M::Freeze | M::Shatter => Tags::COLD,
            M::HolyHeal | M::HolyJudgement => Tags::HOLY,
            M::ShadowPlague | M::Plague | M::ShadowDrain => Tags::SHADOW,
            M::ArcaneFocus | M::ArcaneBlast | M::Starfall | M::Moonfire => Tags::ARCANE,
            _ => Tags(0),
        }
}
fn accepts(run: &Run, h: &Hit, m: M, tags: Tags) -> bool {
    h.can_proc() && active(run, m) && tags.contains(hit_tags(m))
}
fn meter_source(run: &Run, m: M) -> Option<&'static str> {
    if !active(run, m) {
        return None;
    }
    if !run.talent_runtime.enabled.contains(&m) {
        if let Some(grant) = run
            .legendary
            .grants
            .iter()
            .find(|g| g.milestone().mechanic == m)
        {
            return Some(grant.milestone().name);
        }
    }
    let class = data::CLASSES.iter().position(|c| c.id == run.class.id)?;
    talent_design::DESIGNS[class * 3..class * 3 + 3]
        .iter()
        .flat_map(|s| s.milestones.iter())
        .find(|milestone| milestone.mechanic == m)
        .map(|milestone| milestone.name)
}
pub(super) fn ailment_source(run: &Run, spell: u32, _roll: f32) -> Option<&'static str> {
    match spell {
        1943 => meter_source(run, M::Bleed),
        133 => meter_source(run, M::Ignite),
        _ => None,
    }
}
pub(super) fn configure(run: &mut Run) {
    run.equipment_version = None;
    let Some(class) = data::CLASSES.iter().position(|c| c.id == run.class.id) else {
        return;
    };
    run.talent_runtime.enabled.clear();
    for tree in 0..3 {
        for (i, node) in [8, 12, 18].into_iter().enumerate() {
            if run.talent_build[tree][node] > 0 {
                let mechanic = talent_design::DESIGNS[class * 3 + tree].milestones[i].mechanic;
                if mechanic != M::None {
                    run.talent_runtime.enabled.push(mechanic);
                }
            }
        }
    }
    super::starter_spells::sync(run, class);
    let mut unlocks = Vec::new();
    if active(run, M::Taunt) {
        unlocks.push("taunt");
        unlocks.push(if class == 0 {
            "thunder_clap"
        } else {
            "consecration"
        });
    }
    if active(run, M::TrapLaunch) {
        unlocks.push("explosive_trap");
    }
    if class == 1 && run.talent_build[0][8] > 0 {
        unlocks.extend(["holy_shock", "exorcism", "holy_light"]);
    }
    if class == 4 && run.talent_build[0][8] > 0 {
        unlocks.push("power_word_shield");
    }
    if class == 4 && run.talent_build[1][8] > 0 {
        unlocks.extend(["flash_heal", "holy_nova", "holy_fire"]);
    }
    if class == 8 && run.talent_build[0][8] > 0 {
        unlocks.extend(["starfire", "moonfire"]);
    }
    if class == 8 && run.talent_build[1][8] > 0 {
        unlocks.extend(["claw", "swipe"]);
    }
    if class == 8 && run.talent_build[2][8] > 0 {
        unlocks.push("rejuvenation");
    }
    if class == 5 && run.talent_build[2][8] > 0 {
        unlocks.extend(["healing_stream_totem", "totemic_recall"]);
    }
    if class == 5 && run.talent_build[2][12] > 0 {
        unlocks.push("strength_of_earth_totem");
    }
    for key in unlocks {
        if !run.abilities.iter().any(|a| a.def.key == key) {
            run.abilities.push(Owned {
                def: data::ability(key),
                level: 1,
                cd: 0.5,
            });
        }
    }
}
pub(super) fn block_chance(run: &Run) -> f32 {
    if active(run, M::Block) {
        0.2 * super::items::legendary::milestones::sources(run, M::Block)
            .len()
            .max(1) as f32
    } else {
        0.0
    }
}
pub(super) fn refreshed_shield(run: &mut Run, capacity: f32) {
    if run.shield > 0.0
        && run.shield_t > 0.0
        && active(run, M::Shieldburst)
        && run.legendary.events.shield_origin == super::items::legendary::events::Origin::Skill
    {
        run.talent_runtime.shield_burst += run.talent_runtime.shield_capacity;
    }
    run.talent_runtime.shield_capacity = capacity;
}
pub(super) fn before_hit(run: &mut Run, h: &Hit, en: &Enemy) -> f32 {
    if run.talent_runtime.enabled.is_empty() && run.legendary.grants.is_empty() {
        return 1.0;
    }
    let t = h
        .tags
        .unwrap_or_else(|| tags::for_spell(data::ability(h.skill)));
    let focus = accepts(run, h, M::Focus, t) || accepts(run, h, M::ArcaneFocus, t);
    if focus || accepts(run, h, M::Execution, t) {
        if run.talent_runtime.target != Some(h.target) {
            run.talent_runtime.target = Some(h.target);
            run.talent_runtime.streak = 0;
        }
        let bonus = if focus {
            run.talent_runtime.streak.min(10) as f32 * 0.08
        } else {
            0.0
        };
        run.talent_runtime.streak += 1;
        return 1.0 + bonus;
    }
    if accepts(run, h, M::Shatter, t) && en.root_t > 0.0 {
        1.6
    } else {
        1.0
    }
}
pub(super) fn applies_dots_on_hit(run: &Run) -> bool {
    [M::Poison, M::Moonfire, M::Plague, M::ShadowPlague].into_iter().any(|m| active(run, m))
}

pub(super) fn on_hit(
    run: &mut Run,
    h: &Hit,
    en: &mut Enemy,
    pos: Vec3,
    rng: &mut Rng,
    pending: &mut PendingHits,
) {
    if !h.can_proc()
        || h.damage <= 0.0
        || (run.talent_runtime.enabled.is_empty() && run.legendary.grants.is_empty())
    {
        return;
    }
    let t = h
        .tags
        .unwrap_or_else(|| tags::for_spell(data::ability(h.skill)));
    // Count only eligible hits: e.g. a Fire hit cannot advance an Arcane milestone.
    for (index, qualifies) in [
        accepts(run, h, M::Flurry, t),
        accepts(run, h, M::Stormstrike, t),
        accepts(run, h, M::ArcaneBlast, t) || accepts(run, h, M::Starfall, t),
        accepts(run, h, M::Cataclysm, t),
    ]
    .into_iter()
    .enumerate()
    {
        if qualifies {
            run.talent_runtime.impacts[index] = run.talent_runtime.impacts[index].wrapping_add(1);
        }
    }
    let count = run.talent_runtime.impacts;
    let poison = accepts(run, h, M::Poison, t);
    let arcane = accepts(run, h, M::Moonfire, t);
    let affliction = accepts(run, h, M::Plague, t);
    let plague = accepts(run, h, M::ShadowPlague, t) || affliction || arcane;
    if run.proc_runtime.blocked_dot_target != Some(h.target)
        && (poison || plague)
        && en.hp > 0.0
        && procs::ready(
            run,
            if poison {
                "proc_poison"
            } else if arcane {
                "proc_lunar_flare"
            } else {
                "proc_plague"
            },
            t,
        )
    {
        let key = if poison {
            "proc_poison"
        } else if arcane {
            "proc_lunar_flare"
        } else {
            "proc_plague"
        };
        let tick = en
            .dots
            .iter()
            .find(|d| d.owner == h.owner && d.skill == key)
            .map_or(1.0, |d| d.tick);
        if !poison && !affliction && !run.legendary.has(230) {
            en.dots.retain(|d| !(d.owner == h.owner && d.skill == key));
        }
        let p = procs::resolve(run, key, 0.0, h.damage * 0.18, 6.0, 0.0, 1);
        super::items::legendary::periodic::install(
            run,
            en,
            h.target,
            pos,
            Dot {
                owner: h.owner,
                skill: key,
                tags: Some(p.tags),
                meter_source: meter_source(
                    run,
                    if poison {
                        M::Poison
                    } else if arcane {
                        M::Moonfire
                    } else if active(run, M::ShadowPlague) {
                        M::ShadowPlague
                    } else {
                        M::Plague
                    },
                ),
                item_source: None,
                origin: if h.origin == super::items::legendary::events::Origin::Item {
                    h.origin
                } else {
                    super::items::legendary::events::Origin::Talent
                },
                meta: Default::default(),
                dps: p.dot,
                left: p.duration,
                tick,
            },
        );
    }
    if accepts(run, h, M::Freeze, t) && procs::ready(run, "proc_frostbite", t) {
        let duration = procs::resolve(run, "proc_frostbite", 0.0, 0.0, 3.0, 0.0, 1).duration;
        en.slow = 0.6;
        en.slow_t = duration;
        if rng.f32() < 0.2 {
            en.root_t = duration * if en.boss { 0.15 } else { 0.5 };
        }
    }
    let holy = t.contains(Tags::HOLY);
    let healing = if accepts(run, h, M::HolyHeal, t) {
        0.04
    } else {
        0.0
    } + if accepts(run, h, M::HolyJudgement, t) {
        0.08
    } else {
        0.0
    };
    if healing > 0.0 && procs::ready(run, "proc_holy_heal", t) {
        run.talent_runtime.healing += procs::resolve(
            run,
            "proc_holy_heal",
            h.damage.min(en.max_hp) * healing,
            0.0,
            0.0,
            0.0,
            1,
        )
        .dmg;
    }
    let now = run.elapsed;
    let mut extra = |key, damage, splash, source, mechanic| {
        let mut hit = Hit::direct(key, h.target, damage, false);
        hit.owner = h.owner;
        hit.cast_id = h.cast_id;
        hit.triggered = true;
        hit.proc_admitted = true;
        hit.origin = if h.origin == super::items::legendary::events::Origin::Item {
            h.origin
        } else {
            super::items::legendary::events::Origin::Talent
        };
        hit.proc_chain = h.proc_chain | chain_bit(mechanic);
        hit.splash = splash;
        hit.meter_source = source;
        pending.0.push((now + 0.01, hit));
    };
    if accepts(run, h, M::Seal, t) && procs::ready_milestone(run, M::Seal, t) {
        for donor in super::items::legendary::milestones::sources(run, M::Seal) {
            let key = if donor == 7 {
                "flame_shock"
            } else {
                "holy_shock"
            };
            extra(
                key,
                h.damage * 0.25,
                if active(run, M::SealStorm) { 6.0 } else { 0.0 },
                super::items::legendary::milestones::source_name(donor, M::Seal),
                M::Seal,
            );
        }
        if active(run, M::SealStorm) {
            let amount =
                procs::resolve(run, "proc_holy_heal", h.damage * 0.05, 0.0, 0.0, 0.0, 1).dmg;
            run.heal(amount);
        }
    }
    if accepts(run, h, M::Splash, t) {
        run.talent_runtime.bursts.push(Burst {
            created: run.elapsed,
            parent: h.cast_id,
            chain: h.proc_chain | chain_bit(M::Splash),
            pos,
            damage: h.damage * 0.4,
            exclude: Some(h.target),
            radius: 4.0,
            targets: 64,
            dots: vec![],
            freeze: false,
            skill: h.skill,
            meter_source: meter_source(run, M::Splash),
            spread_source: None,
        });
    }
    if holy
        && super::items::legendary::milestones::sources(run, M::HolyJudgement).contains(&2)
        && accepts(run, h, M::HolyJudgement, t)
    {
        run.talent_runtime.bursts.push(Burst {
            created: run.elapsed,
            parent: h.cast_id,
            chain: h.proc_chain | chain_bit(M::HolyJudgement),
            pos,
            damage: h.damage * 0.5,
            exclude: Some(h.target),
            radius: 6.0,
            targets: 2,
            dots: vec![],
            freeze: false,
            skill: "holy_shock",
            meter_source: meter_source(run, M::HolyJudgement),
            spread_source: None,
        });
    }
    if accepts(run, h, M::Windfury, t)
        && rng.f32() < 0.2
        && procs::ready_milestone(run, M::Windfury, t)
    {
        extra(
            h.skill,
            h.damage * 0.5,
            0.0,
            meter_source(run, M::Windfury),
            M::Windfury,
        );
        extra(
            h.skill,
            h.damage * 0.5,
            0.0,
            meter_source(run, M::Windfury),
            M::Windfury,
        );
    }
    if accepts(run, h, M::Execution, t)
        && run.talent_runtime.streak >= 10
        && procs::ready_milestone(run, M::Execution, t)
    {
        extra(
            h.skill,
            h.damage * 2.0,
            0.0,
            meter_source(run, M::Execution),
            M::Execution,
        );
        run.talent_runtime.streak = 0;
    }
    let burst = if accepts(run, h, M::Flurry, t) && count[0] % 3 == 0 {
        Some((0.8, "blade_flurry"))
    } else if accepts(run, h, M::Stormstrike, t) && count[1] % 5 == 0 {
        Some((1.0, "chain_lightning"))
    } else if (accepts(run, h, M::ArcaneBlast, t) || accepts(run, h, M::Starfall, t))
        && count[2] % 5 == 0
    {
        Some((1.5, "arcane_explosion"))
    } else if accepts(run, h, M::Cataclysm, t)
        && count[3] % 3 == 0
        && run.elapsed - run.talent_runtime.last_block >= 1.0
    {
        run.talent_runtime.last_block = run.elapsed;
        Some((2.0, "hellfire"))
    } else {
        None
    };
    if let Some((factor, skill)) = burst {
        let mechanic = match skill {
            "blade_flurry" => M::Flurry,
            "chain_lightning" => M::Stormstrike,
            "hellfire" => M::Cataclysm,
            _ if active(run, M::Starfall) => M::Starfall,
            _ => M::ArcaneBlast,
        };
        let (radius, targets) = match skill {
            "blade_flurry" => (6.0, 64),
            "chain_lightning" => (8.0, 4),
            "hellfire" => (8.0, 64),
            _ => (7.0, 64),
        };
        run.talent_runtime.bursts.push(Burst {
            created: run.elapsed,
            parent: h.cast_id,
            chain: h.proc_chain | chain_bit(mechanic),
            pos,
            damage: h.damage * factor,
            exclude: None,
            radius,
            targets,
            dots: vec![],
            freeze: false,
            skill,
            meter_source: meter_source(run, mechanic),
            spread_source: None,
        });
    }
}
pub(super) fn periodic_leech(run: &mut Run, tags: Tags, damage: f32) {
    if active(run, M::ShadowDrain)
        && tags.contains(Tags::SHADOW)
        && procs::ready(run, "proc_shadow_leech", tags)
    {
        let amount = procs::resolve(run, "proc_shadow_leech", damage * 0.12, 0.0, 0.0, 0.0, 1).dmg;
        run.heal(amount);
    }
}
pub(super) fn on_death(run: &mut Run, en: &Enemy, pos: Vec3) {
    if run.talent_runtime.enabled.is_empty() && run.legendary.grants.is_empty() {
        return;
    }
    let owned: Vec<_> = en
        .dots
        .iter()
        .filter(|d| d.owner == run.owner)
        .cloned()
        .collect();
    let has = |key| owned.iter().any(|d| d.skill == key);
    let bleed = active(run, M::Bloodburst)
        && owned
            .iter()
            .any(|d| tags::school(d.skill).is_some_and(|t| t.contains(Tags::PHYSICAL)));
    let poison = has("deadly_poison") || has("proc_poison");
    let fire = owned
        .iter()
        .any(|d| tags::school(d.skill).is_some_and(|t| t.contains(Tags::FIRE)));
    let trap_spread = active(run, M::DeathTrap) && has("explosive_trap");
    let spread = (active(run, M::Proliferate) && poison)
        || (active(run, M::IgniteSpread) && fire)
        || (active(run, M::PlagueSpread) && !owned.is_empty());
    let freeze = active(run, M::FrostNova) && en.root_t > 0.0;
    let explode = bleed
        || active(run, M::Poisonburst) && poison
        || active(run, M::Fireburst) && fire
        || (active(run, M::Plagueburst) || active(run, M::DeathTrap)) && !owned.is_empty()
        || super::items::legendary::milestones::sources(run, M::PlagueSpread).contains(&5)
            && active(run, M::PlagueSpread)
            && !owned.is_empty()
        || freeze;
    if spread || trap_spread || explode {
        run.talent_runtime.bursts.push(Burst {
            created: run.elapsed,
            parent: run.proc_runtime.parent,
            chain: 0,
            pos,
            damage: if explode {
                death_explosion_base(en.max_hp)
            } else {
                0.0
            },
            exclude: None,
            radius: if explode { DEATH_EXPLOSION_RADIUS } else { 6.0 },
            targets: 64,
            dots: owned.into_iter().filter(|d| d.left > 0.0 && d.left.is_finite() && (spread || trap_spread && d.skill == "explosive_trap")).collect(),
            freeze,
            skill: if poison {
                "deadly_poison"
            } else if fire {
                "fireball"
            } else {
                "arcane_explosion"
            },
            meter_source: meter_source(
                run,
                if bleed {
                    M::Bloodburst
                } else if active(run, M::Poisonburst) && poison {
                    M::Poisonburst
                } else if active(run, M::Fireburst) && fire {
                    M::Fireburst
                } else if active(run, M::DeathTrap) {
                    M::DeathTrap
                } else if active(run, M::Plagueburst) {
                    M::Plagueburst
                } else if freeze {
                    M::FrostNova
                } else {
                    M::PlagueSpread
                },
            ),
            spread_source: meter_source(
                run,
                if active(run, M::Proliferate) && poison {
                    M::Proliferate
                } else if active(run, M::IgniteSpread) && fire {
                    M::IgniteSpread
                } else if trap_spread {
                    M::DeathTrap
                } else {
                    M::PlagueSpread
                },
            ),
        });
    }
}

/// Only the health-derived base is capped; the source's damage modifiers and crit follow.
pub(super) const DEATH_EXPLOSION_CAP: f32 = 1000.0;
pub(super) const DEATH_EXPLOSION_RADIUS: f32 = 6.0 * std::f32::consts::SQRT_2;

fn death_explosion_base(max_hp: f32) -> f32 {
    (max_hp.max(0.0) * 0.3).min(DEATH_EXPLOSION_CAP)
}

pub(super) fn tick(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    hero: Query<(Entity, &Transform), (With<Hero>, Without<Enemy>, Without<Summon>)>,
    mut enemies: Query<(Entity, &Transform, &mut Enemy), Without<Summon>>,
    mut pets: Query<
        (
            Entity,
            &mut Transform,
            &mut Summon,
            &mut crate::creature_anim::MovementState,
        ),
        Without<Enemy>,
    >,
    mut pending: ResMut<PendingHits>,
    mut rng: ResMut<Rng>,
    mut fx: Fx,
) {
    let dt = time.delta_secs();
    super::minions::tick(&mut run);
    if dt <= 0.0 || run.dead_t.is_some() || run.pending_levels > 0 {
        return;
    }
    let Ok((me, tf)) = hero.single() else {
        return;
    };
    let pos = tf.translation;
    let owner = run.owner;
    let now = run.elapsed;
    let bursts = std::mem::take(&mut run.talent_runtime.bursts);
    for mut burst in bursts {
        let delay = (now - burst.created).max(0.0);
        for dot in &mut burst.dots {
            dot.left -= delay;
            dot.tick -= delay;
        }
        burst.dots.retain(|dot| dot.left > 0.0);
        if run.legendary.has(238) && !burst.dots.is_empty() {
            let mut gathered = Vec::new();
            for (_, tf, mut en) in &mut enemies {
                if !en.dead && units::flat(tf.translation - burst.pos).length() <= burst.radius {
                    en.dots.retain(|dot| {
                        if dot.owner == owner {
                            gathered.push(dot.clone());
                            false
                        } else {
                            true
                        }
                    });
                }
            }
            for dot in gathered {
                super::items::legendary::periodic::field(
                    &mut run,
                    238,
                    burst.pos,
                    super::items::legendary::periodic::budget(&dot),
                    dot.left,
                    dot.tags.unwrap_or(Tags::SHADOW),
                    false,
                );
            }
            burst.dots.clear();
        }
        let skill = procs::source_key(&run, burst.meter_source)
            .filter(|key| !key.ends_with("_spread"))
            .unwrap_or("proc_plagueburst");
        let mut p = procs::resolve(
            &run,
            skill,
            burst.damage,
            0.0,
            if burst.freeze { 2.0 } else { 0.0 },
            burst.radius,
            burst.targets,
        );
        p.roll_extras(std::array::from_fn(|i| {
            if p.extra_chances[i] > 0.0 {
                rng.f32()
            } else {
                1.0
            }
        }));
        let death_trap = active(&run, M::DeathTrap)
            && burst.meter_source.is_some()
            && burst.meter_source == meter_source(&run, M::DeathTrap);
        if !death_trap && !procs::ready(&mut run, skill, p.tags) {
            continue;
        }
        if skill == "proc_starfall" && run.legendary.has(88) {
            let mut targets: Vec<_> = enemies
                .iter()
                .filter(|(_, tf, en)| {
                    !en.dead && units::flat(tf.translation - burst.pos).length() <= 24.0
                })
                .map(|(e, tf, _)| (e, units::flat(tf.translation - burst.pos).length()))
                .collect();
            targets.sort_by(|a, b| a.1.total_cmp(&b.1));
            targets.truncate(3);
            let count = targets.len().max(1) as f32;
            for (target, dist) in targets {
                let (damage, crit) = super::spell_engine::critical_with_multiplier(
                    p.dmg / count,
                    p.crit,
                    p.crit_multiplier,
                    rng.f32(),
                );
                let mut hit = Hit::direct(skill, target, damage, crit);
                hit.owner = owner;
                hit.origin = super::items::legendary::events::Origin::Talent;
                hit.triggered = true;
                hit.proc_admitted = true;
                hit.proc_resolved = true;
                hit.tags = Some(p.tags);
                hit.direct_dot = p.direct_dot.map(|mut dot| {
                    dot.dps /= count;
                    dot
                });
                hit.proc_chain = burst.chain;
                hit.cast_id = burst.parent;
                hit.meter_source = burst.meter_source;
                pending.0.push((now + dist / 25.0, hit));
            }
            continue;
        }
        let spread_key =
            procs::source_key(&run, burst.spread_source).unwrap_or("proc_plague_spread");
        let spread_radius = if burst.dots.is_empty() {
            0.0
        } else {
            procs::radius(&run, spread_key, burst.radius)
        };
        let repeats = super::spell_engine::recast_count(p.recast, rng.f32(), false).min(64);
        let mut affected = 0;
        for (entity, tf, mut en) in &mut enemies {
            if en.dead
                || burst.exclude == Some(entity)
                || units::flat(tf.translation - burst.pos).length() > p.radius.max(spread_radius)
            {
                continue;
            }
            if affected >= p.count {
                break;
            }
            affected += 1;
            let distance = units::flat(tf.translation - burst.pos).length();
            if burst.freeze && distance <= p.radius {
                en.root_t = en.root_t.max(p.duration * if en.boss { 0.3 } else { 1.0 });
            }
            for dot in burst.dots.iter().filter(|_| distance <= spread_radius) {
                if !en
                    .dots
                    .iter()
                    .any(|d| d.owner == dot.owner && d.skill == dot.skill)
                {
                    let mut dot = dot.clone();
                    dot.meter_source = burst.spread_source.or(dot.meter_source);
                    en.dots.push(dot);
                }
            }
            if p.dmg > 0.0 && distance <= p.radius {
                let (damage, crit) = super::spell_engine::critical_with_multiplier(
                    p.dmg,
                    p.crit,
                    p.crit_multiplier,
                    rng.f32(),
                );
                let mut hit = Hit::direct(skill, entity, damage, crit);
                hit.owner = owner;
                hit.triggered = true;
                hit.proc_admitted = true;
                hit.proc_resolved = true;
                hit.direct_dot = p.direct_dot;
                hit.tags = Some(p.tags);
                hit.proc_chain = burst.chain;
                hit.cast_id = burst.parent;
                hit.origin = super::items::legendary::events::Origin::Talent;
                hit.meter_source = burst.meter_source;
                let spell = data::ability(burst.skill).spell;
                for repeat in 0..repeats {
                    let at = now + super::spell_engine::RECAST_DELAY * (repeat + 1) as f32;
                    pending.0.push((at, hit.clone()));
                    // Same unit at both ends: the impact alone, on the echo's landing.
                    pending.1.push((at, entity, entity, spell));
                }
                pending.0.push((now, hit));
                fx.impact(entity, spell);
            }
        }
    }
    let shield = std::mem::take(&mut run.talent_runtime.shield_burst);
    let blocked = std::mem::take(&mut run.talent_runtime.blocked);
    if shield > 0.0 && active(&run, M::Shieldburst)
        || blocked && active(&run, M::Bulwark) && now - run.talent_runtime.last_block >= 1.0
    {
        let nova = active(&run, M::ShieldNova);
        let radius = procs::radius(
            &run,
            if shield > 0.0 {
                "proc_shieldburst"
            } else {
                procs::key(&run, M::Bulwark)
            },
            if nova { 10.0 } else { 6.0 },
        );
        let damage = if shield > 0.0 {
            shield * if nova { 1.2 } else { 0.6 }
        } else {
            45.0
        };
        let donors = if shield > 0.0 {
            vec![run.class.id]
        } else {
            super::items::legendary::milestones::sources(&run, M::Bulwark)
        };
        for donor in donors {
            let skill = if donor == 1 {
                "thunder_clap"
            } else {
                "holy_nova"
            };
            for (entity, tf, en) in &enemies {
                if !en.dead && units::flat(tf.translation - pos).length() <= radius {
                    let mut hit = Hit::direct(skill, entity, damage, false);
                    hit.owner = owner;
                    hit.triggered = true;
                    hit.meter_source = meter_source(
                        &run,
                        if shield > 0.0 {
                            if nova {
                                M::ShieldNova
                            } else {
                                M::Shieldburst
                            }
                        } else {
                            M::Bulwark
                        },
                    );
                    if shield <= 0.0 {
                        hit.meter_source =
                            super::items::legendary::milestones::source_name(donor, M::Bulwark);
                    }
                    pending.0.push((now, hit));
                }
            }
            fx.cast(me, data::ability(skill).spell, &[me], None);
        }
        if active(&run, M::ShieldHeal) {
            let max = run.stats().max_hp;
            let amount = procs::resolve(&run, "proc_holy_heal", max * 0.1, 0.0, 0.0, 0.0, 1).dmg;
            run.heal(amount);
        }
        if blocked {
            run.talent_runtime.last_block = now;
            if super::items::legendary::milestones::sources(&run, M::Bulwark).contains(&2) {
                let max = run.stats().max_hp;
                let amount =
                    procs::resolve(&run, "proc_holy_heal", max * 0.02, 0.0, 0.0, 0.0, 1).dmg;
                run.heal(amount);
            }
        }
    }
    for (index, mechanic, key) in [
        (0, M::Taunt, "proc_taunt"),
        (1, M::HolyBurst, "proc_holy_burst"),
        (2, M::Grove, "proc_grove"),
    ] {
        if !active(&run, mechanic) || now < run.talent_runtime.pulse[index] {
            continue;
        }
        run.talent_runtime.pulse[index] = now + procs::interval(&run, key, 6.0);
        if mechanic != M::Taunt {
            run.talent_runtime.party_pulse = true;
        }
        if mechanic == M::Taunt {
            let p = procs::resolve(&run, "proc_taunt", 0.0, 0.0, 3.0, 12.0, 1);
            for (entity, tf, mut en) in &mut enemies {
                if !en.dead && units::flat(tf.translation - pos).length() <= p.radius {
                    if run.legendary.has(159) && !en.boss {
                        en.fear_t = en.fear_t.max(p.duration);
                    } else {
                        commands.entity(entity).insert(Taunted {
                            owner,
                            until: now + p.duration,
                        });
                    }
                    fx.impact(entity, 355);
                }
            }
        }
        if mechanic == M::HolyBurst {
            let radius = procs::radius(&run, "proc_holy_burst", 6.0);
            for (entity, tf, en) in &enemies {
                if !en.dead && units::flat(tf.translation - pos).length() <= radius {
                    let mut h = Hit::direct("holy_nova", entity, 40.0, false);
                    h.owner = owner;
                    h.triggered = true;
                    h.meter_source = meter_source(&run, M::HolyBurst);
                    pending.0.push((now, h));
                }
            }
        }
    }
    let stomp = run.talent_runtime.minions.stomp.take();
    let command = run.talent_runtime.minions.command.take();
    let counts = [
        if active(&run, M::Pack) {
            2
        } else {
            u8::from(active(&run, M::Pet))
        },
        if active(&run, M::Demons) {
            3
        } else {
            u8::from(active(&run, M::Demon))
        },
        if active(&run, M::SpiritTotem) {
            2
        } else {
            u8::from(active(&run, M::HealTotem))
        },
        if run.talent_runtime.minions.tame_active {
            run.abilities
                .iter()
                .find(|a| a.def.key == "tame_beast" && !run.disabled_spells.contains(a.def.key))
                .map_or(0, |a| if a.level >= 6 { 2 } else { 1 })
        } else {
            0
        },
    ];
    if now >= run.talent_runtime.summon {
        run.talent_runtime.summon = now + 1.0;
        for (family, count) in counts.into_iter().enumerate() {
            for slot in 0..count {
                let totem = family == 2;
                let demon = family == 1;
                if pets.iter().any(|(_, _, p, _)| {
                    p.owner == owner && p.family == family as u8 && p.slot == slot
                }) {
                    continue;
                }
                let serial = run.guid();
                let entity = units::spawn_unit(
                    &mut commands,
                    &UnitSpec {
                        serial,
                        display: if totem {
                            4587
                        } else if demon {
                            4449
                        } else {
                            247
                        },
                        scale: 1.0,
                        level: 60,
                        faction: units::FACTION_FRIENDLY,
                        bytes0: 0,
                        weapons: &[],
                        sheath: 0,
                    },
                    pos + Vec3::X * (2.0 + f32::from(slot)),
                    0.0,
                );
                commands.entity(entity).insert(Summon {
                    family: family as u8,
                    owner,
                    slot,
                    cooldown: rng.range(0.05, 0.55),
                    stationary: totem,
                    target: None,
                    retarget: 0.0,
                    carried: None,
                    carry_until: 0.0,
                    harvested: None,
                    charge: None,
                    protecting: false,
                });
                if let Some(owner) = owner {
                    commands
                        .entity(entity)
                        .insert(super::multiplayer::OwnedEffect(owner));
                }
            }
        }
    }
    let owner_has_boss = enemies
        .iter()
        .any(|(_, tf, en)| !en.dead && en.boss && within_pet_leash(tf.translation, pos));
    let mut owner_dangers = if pets
        .iter()
        .any(|(_, _, pet, _)| pet.owner == owner && !pet.stationary)
    {
        enemies
            .iter()
            .filter(|(_, tf, en)| {
                !en.dead
                    && within_pet_leash(tf.translation, pos)
                    && super::spell_engine::targeting::threatens_owner(
                        units::flat(tf.translation - pos).length(),
                        en.radius,
                    )
            })
            .map(|(e, tf, _)| (e, units::flat(tf.translation - pos).length_squared()))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    owner_dangers.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.to_bits().cmp(&b.0.to_bits())));
    // Preserve valid existing assignments, then reserve new targets as pets choose them.
    // Bosses are deliberately shared; totems are not mobile pets.
    let mut reserved = std::collections::HashMap::new();
    for (entity, tf, pet, _) in &pets {
        if pet.owner != owner
            || pet.stationary
            || pet.slot >= counts[pet.family as usize]
            || !within_pet_leash(tf.translation, pos)
        {
            continue;
        }
        if let Some(target) = pet.target {
            if enemies.get(target).is_ok_and(|(_, tf, en)| {
                !en.dead && !en.boss && within_pet_leash(tf.translation, pos)
            }) {
                reserved.entry(target).or_insert(entity);
            }
        }
    }
    for (entity, mut tf, mut pet, mut mv) in &mut pets {
        if pet.owner != owner {
            continue;
        }
        let demon = pet.family == 1;
        let hunter_pet = pet.family == 0 || pet.family == 3;
        if pet.carry_until < now {
            pet.carried = None;
        }
        if pet.slot >= counts[pet.family as usize] {
            commands.entity(entity).despawn();
            continue;
        }
        reserved.retain(|_, holder| *holder != entity);
        let exclusive = !pet.stationary;
        let available = |target: Entity, enemy: &Enemy| {
            !exclusive || enemy.boss || !reserved.contains_key(&target)
        };
        let danger = if exclusive {
            owner_dangers
                .iter()
                .map(|(e, _)| *e)
                .find(|e| enemies.get(*e).is_ok_and(|(_, _, en)| available(*e, en)))
        } else {
            None
        };
        if hunter_pet {
            if let Some(p) = stomp {
                let mut targets = Vec::new();
                for (target, target_tf, enemy) in &enemies {
                    if !enemy.dead
                        && units::flat(target_tf.translation - tf.translation).length() <= p.radius
                    {
                        let hit = super::minions::attack(&run, "stomp", p, target, &mut rng);
                        pending.0.push((now, hit));
                        targets.push(target);
                    }
                }
                fx.area_scale(
                    entity,
                    data::ability("stomp").spell,
                    (p.radius / 6.0).clamp(0.25, 8.0),
                );
                fx.cast(entity, data::ability("stomp").spell, &targets, None);
            }
            if let Some(p) = command {
                // Share the basic-attack priorities, including the exact owner threat.
                let target = danger
                    .or_else(|| {
                        pet_target(
                            enemies
                                .iter()
                                .filter(|(e, _, en)| !en.dead && available(*e, en))
                                .map(|(e, target_tf, en)| (e, target_tf.translation, en.boss)),
                            tf.translation,
                            pos,
                        )
                    })
                    .filter(|target| {
                        enemies.get(*target).is_ok_and(|(_, target_tf, _)| {
                            units::flat(target_tf.translation - tf.translation).length() <= p.range
                        })
                    });
                if let Some(target) = target {
                    pet.target = Some(target);
                    pet.charge = Some((target, p));
                    pet.retarget = 2.0;
                }
            }
        }
        if !within_pet_leash(tf.translation, pos) {
            tf.translation = pos + Vec3::X * (2.0 + f32::from(pet.slot));
            pet.target = None;
            pet.charge = None;
            pet.retarget = 0.0;
            pet.cooldown -= dt;
            mv.speed = 0.0;
            mv.flags = 0;
            continue;
        }
        let ranged = demon || pet.stationary;
        let range = if ranged { 20.0 } else { 3.0 };
        pet.retarget -= dt;
        let was_protecting = pet.protecting;
        pet.protecting = danger.is_some();
        if let Some(target) = danger {
            if pet.charge.is_some_and(|(old, _)| old != target) {
                pet.charge = None;
            }
            pet.target = Some(target);
        }
        let valid = pet
            .target
            .and_then(|e| enemies.get(e).ok())
            .is_some_and(|(e, tf, en)| {
                !en.dead && within_pet_leash(tf.translation, pos) && available(e, en)
            });
        if !valid {
            pet.charge = None;
        }
        let focused_boss = pet
            .target
            .and_then(|e| enemies.get(e).ok())
            .is_some_and(|(_, _, en)| en.boss);
        if !pet.protecting
            && (was_protecting
                || !valid
                || pet.retarget <= 0.0
                || (owner_has_boss && !focused_boss))
        {
            pet.target = pet_target(
                enemies
                    .iter()
                    .filter(|(e, _, en)| !en.dead && available(*e, en))
                    .map(|(e, tf, en)| (e, tf.translation, en.boss)),
                tf.translation,
                pos,
            );
            if pet.charge.is_some_and(|(old, _)| Some(old) != pet.target) {
                pet.charge = None;
            }
            pet.retarget = rng.range(1.2, 2.4);
        }
        if !pet.stationary && !pet.protecting && !owner_has_boss && run.legendary.has(185) {
            if let Some((e, _, _)) = enemies
                .iter()
                .filter(|(e, _, en)| {
                    !en.dead
                        && available(*e, en)
                        && en.dots.iter().any(|d| {
                            d.owner == owner
                                && super::items::legendary::periodic::family(d)
                                    == super::items::legendary::periodic::Family::Bleed
                        })
                })
                .min_by(|(_, a, _), (_, b, _)| {
                    units::flat(a.translation - tf.translation)
                        .length_squared()
                        .total_cmp(&units::flat(b.translation - tf.translation).length_squared())
                })
            {
                if pet.carried.is_none() {
                    pet.target = Some(e);
                }
            }
        }
        if exclusive {
            if let Some(target) = pet.target {
                if enemies.get(target).is_ok_and(|(_, _, en)| !en.boss) {
                    reserved.insert(target, entity);
                }
            }
        }
        let target = pet.target.and_then(|e| enemies.get(e).ok());
        let destination = if pet.stationary {
            tf.translation
        } else {
            target.as_ref().map_or(pos, |t| t.1.translation)
        };
        let destination =
            if run.legendary.has(188) && !pet.stationary && !pet.protecting && !owner_has_boss {
                run.legendary
                    .casting
                    .spirit_destination
                    .unwrap_or(destination)
            } else {
                destination
            };
        let delta = units::flat(destination - tf.translation);
        let len = delta.length();
        let profile_key = if demon {
            "proc_demon_bolt"
        } else if pet.stationary {
            if active(&run, M::SpiritTotem) {
                "proc_spirit_bolt"
            } else {
                "proc_totem_bolt"
            }
        } else {
            "proc_pet_bite"
        };
        let profile = procs::resolve(&run, profile_key, 18.0, 0.0, 0.0, 0.0, 1);
        let speed = if pet.charge.is_some() { 30.0 } else { 8.0 }
            * (run.stats().speed / super::server::BASE_SPEED)
            * super::monster_specials::player::pet_movement(&run, tf.translation)
            * profile.movement_speed;
        let move_by = (dt * speed).min((len - 1.5).max(0.0));
        tf.translation += delta.normalize_or_zero() * move_by;
        if !pet.stationary {
            tf.translation.y = pos.y;
            let facing = target.map_or(delta, |(_, target_tf, _)| {
                units::flat(target_tf.translation - tf.translation)
            });
            if facing.length_squared() > 0.0001 {
                let rotation = Quat::from_rotation_y(units::yaw_toward(facing));
                tf.rotation = tf.rotation.slerp(rotation, (dt * 20.0).min(1.0));
            }
        }
        mv.speed = if move_by > 0.0 { speed } else { 0.0 };
        mv.flags = if move_by > 0.0 { 1 } else { 0 };
        pet.cooldown -= dt * super::monster_specials::player::pet_action(&run, tf.translation);
        if !pet.stationary {
            run.legendary.casting.pet = Some(tf.translation);
        }
        if let Some((target, p)) = pet.charge {
            if let Ok((_, target_tf, enemy)) = enemies.get(target) {
                if !enemy.dead
                    && units::flat(target_tf.translation - tf.translation).length() <= 3.0
                {
                    let hit = super::minions::attack(&run, "kill_command", p, target, &mut rng);
                    pending.0.push((now, hit));
                    fx.cast(entity, data::ability("kill_command").spell, &[target], None);
                    pet.charge = None;
                }
            }
        }
        if let Some((target, target_tf, _)) = target {
            if pet.cooldown <= 0.0
                && units::flat(target_tf.translation - tf.translation).length() <= range
                && (!pet.stationary || active(&run, M::DualTotem) || run.legendary.has(194))
            {
                let target_pos = target_tf.translation;
                if !pet.stationary && run.legendary.has(188) {
                    pet.cooldown = 0.5;
                    continue;
                }
                if !pet.stationary && run.legendary.has(185) {
                    if pet.harvested != Some(target) {
                        if let Some(dot) = pet.carried.take() {
                            if let Ok((_, _, mut en)) = enemies.get_mut(target) {
                                en.dots.push(dot);
                            }
                            pet.harvested = None;
                        } else if let Ok((_, _, mut en)) = enemies.get_mut(target) {
                            if let Some(dot) = en.dots.iter_mut().find(|d| {
                                d.owner == owner
                                    && super::items::legendary::periodic::family(d)
                                        == super::items::legendary::periodic::Family::Bleed
                            }) {
                                let mut portion = dot.clone();
                                portion.dps *= 0.3;
                                dot.dps *= 0.7;
                                pet.carried = Some(portion);
                                pet.carry_until = now + 6.0;
                                pet.harvested = Some(target);
                                pet.target = None;
                            }
                        }
                    }
                }
                if !pet.stationary {
                    super::items::legendary::effects::pet_mark(&mut run, target, target_pos);
                }
                if !pet.stationary && run.legendary.has(191) {
                    pet.cooldown = 1.5;
                    continue;
                }
                if !pet.stationary && run.legendary.has(181) {
                    if let Some((key, _)) = run.legendary.casting.last_melee {
                        super::items::legendary::casting::copy(
                            &mut run,
                            181,
                            key,
                            tf.translation,
                            Some(target),
                            0.0,
                        );
                        pet.cooldown = 1.5;
                        continue;
                    }
                }
                let damage = (if pet.family == 3 {
                    let level = run
                        .abilities
                        .iter()
                        .find(|a| a.def.key == "tame_beast")
                        .map_or(1, |a| a.level);
                    super::minions::tame_damage(level)
                } else if pet.stationary {
                    24.0
                } else {
                    18.0
                }) * if demon && active(&run, M::DemonLord) {
                    2.0
                } else {
                    1.0
                };
                let skill = if demon {
                    "proc_demon_bolt"
                } else if pet.stationary {
                    if active(&run, M::SpiritTotem) {
                        "proc_spirit_bolt"
                    } else {
                        "proc_totem_bolt"
                    }
                } else {
                    "proc_pet_bite"
                };
                pet.cooldown = profile.cooldown;
                let mut hit = Hit::direct(skill, target, damage, false);
                hit.owner = owner;
                hit.triggered = true;
                hit.meter_source = meter_source(
                    &run,
                    if demon {
                        if active(&run, M::DemonLord) {
                            M::DemonLord
                        } else if active(&run, M::Demons) {
                            M::Demons
                        } else {
                            M::Demon
                        }
                    } else if pet.stationary {
                        if active(&run, M::SpiritTotem) {
                            M::SpiritTotem
                        } else {
                            M::DualTotem
                        }
                    } else if active(&run, M::Pack) {
                        M::Pack
                    } else {
                        M::Pet
                    },
                );
                if !demon && !pet.stationary && active(&run, M::Pack) {
                    hit.meter_source = Some(super::minions::PACK_ATTACK_SOURCE);
                }
                if demon && active(&run, M::Demons) {
                    hit.dot = damage * 0.2;
                    hit.dot_dur = 4.0;
                }
                if demon && active(&run, M::DemonLord) {
                    hit.splash = 4.0;
                }
                if pet.family == 3 {
                    let p = procs::resolve(&run, skill, damage, hit.dot, hit.dot_dur, 0.0, 1);
                    let mut single = super::minions::attack(&run, skill, p, target, &mut rng);
                    single.meter_source = Some("Tame Beast");
                    let repeats =
                        super::spell_engine::recast_count(p.recast, rng.f32(), false).min(64);
                    for repeat in 0..repeats {
                        let at = now + super::spell_engine::RECAST_DELAY * (repeat + 1) as f32;
                        pending.0.push((at, single.clone()));
                    }
                    pending.0.push((now, single));
                } else {
                    pending.0.push((now, hit.clone()));
                }
                fx.cast(entity, data::ability(skill).spell, &[target], None);
            }
        }
    }
}

/// Resolve support once for the whole party, outside the temporary guest simulation context.
pub(super) fn support(
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut players: Option<ResMut<Companions>>,
    hero: Query<&Transform, With<Hero>>,
    remotes: Query<(&Transform, &RemoteOwner)>,
    totems: Query<(&Transform, &Summon)>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let Ok(tf) = hero.single() else {
        return;
    };
    let mut sources = vec![];
    struct Source {
        owner: Option<u64>,
        pos: Vec3,
        rate: (f32, f32),
        healing_centers: Vec<Vec3>,
        pulse: (f32, f32),
        flat: (f32, f32),
        buff: super::healer_buffs::Source,
    }
    fn source(run: &mut Run, pos: Vec3, totems: &Query<(&Transform, &Summon)>) -> Source {
        let totem_healing =
            (active(run, M::HealTotem) || active(run, M::SpiritTotem)) && !run.legendary.has(194);
        let healing_centers = if totem_healing {
            totems
                .iter()
                .filter(|(_, t)| t.owned_totem(run.owner))
                .map(|(tf, _)| tf.translation)
                .collect()
        } else {
            vec![pos]
        };
        let rate = if active(run, M::SpiritTotem) && !run.legendary.has(194) {
            0.04
        } else if active(run, M::HealTotem) && !run.legendary.has(194) {
            0.02
        } else if active(run, M::Grove) {
            0.03
        } else if active(run, M::Hot) {
            0.015
        } else {
            0.0
        };
        let pulse = std::mem::take(&mut run.talent_runtime.party_pulse)
            && (active(run, M::HolyBurst) || active(run, M::Grove));
        let healing = std::mem::take(&mut run.talent_runtime.healing);
        let key = if (active(run, M::HealTotem) || active(run, M::SpiritTotem))
            && !run.legendary.has(194)
        {
            "proc_totem_heal"
        } else if active(run, M::Grove) {
            "proc_grove"
        } else {
            "proc_bloom"
        };
        let rate = procs::resolve(run, key, rate, 0.0, 0.0, 12.0, 1);
        let pulse = procs::resolve(
            run,
            if active(run, M::Grove) {
                "proc_grove"
            } else {
                "proc_holy_heal"
            },
            if pulse { 0.12 } else { 0.0 },
            0.0,
            0.0,
            12.0,
            1,
        );
        let buff = super::healer_buffs::source(run);
        Source {
            owner: run.owner,
            pos,
            rate: (rate.dmg, rate.radius),
            healing_centers,
            pulse: (pulse.dmg, pulse.radius),
            flat: (healing, procs::radius(run, "proc_holy_heal", 12.0)),
            buff,
        }
    }
    if run.dead_t.is_none() {
        sources.push(source(&mut run, tf.translation, &totems));
    }
    if let Some(players) = players.as_deref_mut() {
        for (tf, owner) in &remotes {
            if let Some(p) = players.0.get_mut(&owner.0) {
                if p.run.dead_t.is_none() {
                    sources.push(source(&mut p.run, tf.translation, &totems));
                }
            }
        }
    }
    let mut credits = Vec::new();
    fn receive(
        run: &mut Run,
        pos: Vec3,
        dt: f32,
        sources: &[Source],
        credits: &mut Vec<(Option<u64>, f32)>,
    ) {
        for source in sources {
            let distance = units::flat(source.pos - pos).length();
            let in_range = |(amount, radius)| if distance <= radius { amount } else { 0.0 };
            // Overlapping ancestral totems extend coverage without multiplying the same HoT.
            let rate = if source
                .healing_centers
                .iter()
                .any(|center| units::flat(*center - pos).length() <= source.rate.1)
            {
                source.rate.0
            } else {
                0.0
            };
            let amount =
                run.stats().max_hp * (rate * dt + in_range(source.pulse)) + in_range(source.flat);
            credits.push((source.owner, run.recover(amount)));
        }
        super::healer_buffs::receive(run, pos, dt, sources.iter().map(|s| (s.pos, &s.buff)));
    }
    receive(&mut run, tf.translation, dt, &sources, &mut credits);
    if let Some(players) = players.as_deref_mut() {
        for (tf, owner) in &remotes {
            if let Some(p) = players.0.get_mut(&owner.0) {
                receive(&mut p.run, tf.translation, dt, &sources, &mut credits);
            }
        }
    }
    for (owner, amount) in credits {
        if let Some(source) =
            super::multiplayer::player_run(&mut run, players.as_deref_mut(), owner)
        {
            source
                .healing_meter
                .record_hit("party_healing", source.elapsed, amount, false, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    fn hunter_world(nodes: &[usize], tame: bool) -> World {
        use crate::creature_anim::{CastEvent, PlaySeq, SpellGoTargets, SwingMessage};
        let mut world = World::new();
        let mut hunter = run(2, 0, nodes);
        if tame {
            hunter.abilities.push(Owned {
                def: data::ability("tame_beast"),
                level: 1,
                cd: 0.0,
            });
            hunter.talent_runtime.minions.tame_active = true;
        }
        world.insert_resource(hunter);
        world.insert_resource(Rng(42));
        world.init_resource::<Time>();
        world.init_resource::<PendingHits>();
        world.init_resource::<PlaySeq>();
        world.init_resource::<Messages<CastEvent>>();
        world.init_resource::<Messages<SpellGoTargets>>();
        world.init_resource::<Messages<SwingMessage>>();
        world.init_resource::<Messages<crate::combat_text::CombatTextSpawn>>();
        world.spawn((Hero { vy: 0.0, air: 0.0 }, Transform::default()));
        pet_step(&mut world);
        world
    }
    fn pet_step(world: &mut World) {
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(100));
        world.resource_mut::<Run>().elapsed += 0.1;
        world.run_system_once(tick).unwrap();
    }
    #[test]
    fn minions_immediately_protect_their_owner_then_resume_normal_targeting() {
        for owner_x in [0.0, 50.0] {
            let mut world = hunter_world(&[8], false);
            let hero = world
                .query_filtered::<Entity, With<Hero>>()
                .single(&world)
                .unwrap();
            world.get_mut::<Transform>(hero).unwrap().translation.x = owner_x;
            let pet = world
                .query_filtered::<Entity, With<Summon>>()
                .single(&world)
                .unwrap();
            world.get_mut::<Transform>(pet).unwrap().translation.x = owner_x + 15.0;
            let distant = world
                .spawn((Transform::from_xyz(owner_x + 15.0, 0.0, 0.0), enemy()))
                .id();
            let danger = world
                .spawn((Transform::from_xyz(owner_x + 1.0, 0.0, 0.0), enemy()))
                .id();
            {
                let mut minion = world.get_mut::<Summon>(pet).unwrap();
                minion.target = Some(distant);
                minion.retarget = 999.0;
                minion.cooldown = 999.0;
            }
            pet_step(&mut world);
            assert_eq!(world.get::<Summon>(pet).unwrap().target, Some(danger));
            assert!(world.get::<Summon>(pet).unwrap().protecting);
            assert!(world.get::<Transform>(pet).unwrap().translation.x < owner_x + 15.0);
            world.get_mut::<Transform>(danger).unwrap().translation.x = owner_x + 10.0;
            pet_step(&mut world);
            assert!(!world.get::<Summon>(pet).unwrap().protecting);
            assert_eq!(world.get::<Summon>(pet).unwrap().target, Some(distant));
        }
    }
    #[test]
    fn tame_beast_inherits_bow_damage_after_summoning_and_equipment_swaps() {
        use super::super::items::{weapons::DamageRange, Config, EquipmentType, Inventory};
        let mut world = hunter_world(&[], true);
        let pet = world
            .query_filtered::<Entity, With<Summon>>()
            .single(&world)
            .unwrap();
        world.spawn((Transform::from_xyz(2.0, 0.0, 0.0), enemy()));
        world.resource_mut::<Run>().replace_spell_modifiers(
            906,
            [super::super::spell_engine::Modifier {
                source: 906,
                filter: super::super::spell_engine::Filter::default(),
                change: super::super::spell_engine::Change::CriticalChance(-1.0),
            }],
        );
        let config = Config::builtin();
        let mut rng = Rng(123);
        let mut bow = (0..1000)
            .find_map(|_| {
                let item = config.generate(1, &mut rng).unwrap();
                (item.equipment_type == EquipmentType::Bow).then_some(item)
            })
            .expect("bow in the item pool");
        bow.affixes.clear();
        bow.base_damage = Some(DamageRange {
            min: 20.0,
            max: 30.0,
        });
        let mut inventory = Inventory::fixture(vec![bow]);
        let mut damage = Vec::new();
        for equipped in [false, true, false] {
            if equipped {
                inventory.equip(1, "Ranged").unwrap();
            } else if !damage.is_empty() {
                inventory.unequip("Ranged").unwrap();
            }
            inventory.apply(&mut world.resource_mut::<Run>());
            world.resource_mut::<PendingHits>().0.clear();
            world.get_mut::<Summon>(pet).unwrap().cooldown = 0.0;
            pet_step(&mut world);
            damage.push(world.resource::<PendingHits>().0[0].1.damage);
            assert_eq!(world.query::<&Summon>().iter(&world).count(), 1);
        }
        assert!(damage[1] > damage[0]);
        assert!((damage[2] - damage[0]).abs() < 0.001);
        let multiplier = world.resource::<Run>().balance.multiplier_tagged(
            world.resource::<Run>().class,
            "proc_pet_bite",
            Tags::MINION
                | Tags::PHYSICAL
                | Tags::ATTACK
                | Tags::DAMAGE
                | Tags::MELEE
                | Tags::DOT
                | Tags::DURATION,
        );
        assert!((damage[1] - damage[0] - 25.0 * multiplier).abs() < 0.001);
    }

    #[test]
    fn tame_beast_updates_existing_pet_stats_and_repeats_without_extra_targets() {
        use super::super::spell_engine::{Change, Filter, Modifier};
        let mut world = hunter_world(&[], true);
        let pet = world
            .query_filtered::<Entity, With<Summon>>()
            .single(&world)
            .unwrap();
        for x in [2.0, 3.0, 4.0] {
            world.spawn((Transform::from_xyz(x, 0.0, 0.0), enemy()));
        }
        let update = |world: &mut World, changes: Vec<Change>| {
            world.resource_mut::<Run>().replace_spell_modifiers(
                902,
                changes.into_iter().map(|change| Modifier {
                    source: 902,
                    filter: Filter::default(),
                    change,
                }),
            );
        };
        update(&mut world, vec![Change::CriticalChance(-1.0)]);
        world.get_mut::<Summon>(pet).unwrap().cooldown = 0.0;
        pet_step(&mut world);
        let baseline = world.resource::<PendingHits>().0[0].1.damage;
        let cooldown = world.get::<Summon>(pet).unwrap().cooldown;
        world.resource_mut::<PendingHits>().0.clear();
        update(
            &mut world,
            vec![
                Change::IncreasedDamage(1.0),
                Change::CriticalChance(1.0),
                Change::RecastChance(2.0),
                Change::IncreasedAttackSpeed(1.0),
                Change::IncreasedMovementSpeed(1.0),
                Change::AdditionalMeleeTargets(10),
            ],
        );
        world.get_mut::<Summon>(pet).unwrap().cooldown = 0.0;
        pet_step(&mut world);
        let hits = &world.resource::<PendingHits>().0;
        assert_eq!(hits.len(), 3, "one attack plus two repeats");
        assert!(hits.iter().all(|(_, h)| h.target == hits[0].1.target
            && h.proc_resolved
            && h.crit
            && h.damage >= baseline * 3.9));
        let now = world.resource::<Run>().elapsed;
        assert_eq!(hits.iter().filter(|(at, _)| *at > now).count(), 2);
        assert!(world.get::<Summon>(pet).unwrap().cooldown < cooldown * 0.6);
        world.get_mut::<Transform>(pet).unwrap().translation.x = 20.0;
        pet_step(&mut world);
        assert!(
            world
                .get::<crate::creature_anim::MovementState>(pet)
                .unwrap()
                .speed
                >= 15.9
        );
        assert_eq!(world.query::<&Summon>().iter(&world).count(), 1);
    }

    #[test]
    fn tame_beast_is_untargetable_single_target_and_rank_six_adds_one_wolf() {
        let mut world = hunter_world(&[], true);
        let pet = world
            .query_filtered::<Entity, With<Summon>>()
            .single(&world)
            .unwrap();
        assert!(world.get::<Hero>(pet).is_none());
        assert!(world.get::<Enemy>(pet).is_none());
        assert!(world
            .get::<super::super::multiplayer::RemoteOwner>(pet)
            .is_none());
        for x in [2.0, 3.0, 4.0] {
            world.spawn((Transform::from_xyz(x, 0.0, 0.0), enemy()));
        }
        world.get_mut::<Summon>(pet).unwrap().cooldown = 0.0;
        world.resource_mut::<Run>().replace_spell_modifiers(
            901,
            [super::super::spell_engine::Modifier {
                source: 901,
                filter: super::super::spell_engine::Filter::requiring(Tags::MINION),
                change: super::super::spell_engine::Change::AdditionalMeleeTargets(10),
            }],
        );
        pet_step(&mut world);
        let hits = &world.resource::<PendingHits>().0;
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].1.meter_source, Some("Tame Beast"));
        assert!(hits[0].1.proc_resolved);
        assert_eq!(hits[0].1.splash, 0.0);
        let base_damage = hits[0].1.damage;
        world.resource_mut::<PendingHits>().0.clear();
        world
            .resource_mut::<Run>()
            .abilities
            .iter_mut()
            .find(|a| a.def.key == "tame_beast")
            .unwrap()
            .level = 6;
        world.get_mut::<Summon>(pet).unwrap().cooldown = 0.0;
        pet_step(&mut world);
        assert_eq!(world.resource::<PendingHits>().0.len(), 1);
        assert!(world.resource::<PendingHits>().0[0].1.damage > base_damage * 2.0);
        for _ in 0..20 {
            pet_step(&mut world);
        }
        let pets: Vec<_> = world
            .query::<(Entity, &Summon)>()
            .iter(&world)
            .map(|(entity, summon)| (entity, summon.family, summon.slot))
            .collect();
        assert_eq!(pets.len(), 2);
        assert!(pets.iter().any(|(entity, _, _)| *entity == pet));
        assert!(pets.iter().all(|(_, family, _)| *family == 3));
        assert!(pets.iter().any(|(_, _, slot)| *slot == 0));
        assert!(pets.iter().any(|(_, _, slot)| *slot == 1));
        world
            .resource_mut::<Run>()
            .abilities
            .iter_mut()
            .find(|a| a.def.key == "tame_beast")
            .unwrap()
            .level = 5;
        pet_step(&mut world);
        assert_eq!(world.query::<&Summon>().iter(&world).count(), 1);
    }
    #[test]
    fn stomp_uses_each_owned_pet_origin_instead_of_the_hunter_or_foreign_pets() {
        let mut world = hunter_world(&[8, 12], false);
        let pets: Vec<_> = world
            .query_filtered::<Entity, With<Summon>>()
            .iter(&world)
            .collect();
        for (entity, x) in pets.iter().zip([-20.0, 20.0]) {
            world.get_mut::<Transform>(*entity).unwrap().translation.x = x;
            world.get_mut::<Summon>(*entity).unwrap().cooldown = 999.0;
        }
        let left = world
            .spawn((Transform::from_xyz(-21.0, 0.0, 0.0), enemy()))
            .id();
        let right = world
            .spawn((Transform::from_xyz(21.0, 0.0, 0.0), enemy()))
            .id();
        let center = world.spawn((Transform::default(), enemy())).id();
        let p = world
            .resource::<Run>()
            .resolve_spell(data::ability("stomp"), 6);
        world.resource_mut::<Run>().talent_runtime.minions.stomp = Some(p);
        pet_step(&mut world);
        let hits = &world.resource::<PendingHits>().0;
        assert_eq!(hits.len(), 2);
        for (_, hit) in hits {
            assert!(hit.target == left || hit.target == right);
            assert_ne!(hit.target, center);
            assert!(matches!(hit.effect, data::Effect::Slow(0.5, _)));
        }
        world.resource_mut::<PendingHits>().0.clear();
        world.get_mut::<Summon>(pets[1]).unwrap().owner = Some(99);
        world.resource_mut::<Run>().talent_runtime.minions.stomp = Some(p);
        pet_step(&mut world);
        assert_eq!(world.resource::<PendingHits>().0.len(), 1);
    }
    #[test]
    fn kill_command_charges_and_each_pet_selects_its_own_target() {
        let mut world = hunter_world(&[8, 12], false);
        let pets: Vec<_> = world
            .query_filtered::<Entity, With<Summon>>()
            .iter(&world)
            .collect();
        for (entity, x) in pets.iter().zip([-20.0, 20.0]) {
            world.get_mut::<Transform>(*entity).unwrap().translation.x = x;
            world.get_mut::<Summon>(*entity).unwrap().cooldown = 999.0;
        }
        let left = world
            .spawn((Transform::from_xyz(-35.0, 0.0, 0.0), enemy()))
            .id();
        let right = world
            .spawn((Transform::from_xyz(35.0, 0.0, 0.0), enemy()))
            .id();
        world.spawn((Transform::from_xyz(0.0, 0.0, 10.0), enemy()));
        let p = world
            .resource::<Run>()
            .resolve_spell(data::ability("kill_command"), 1);
        world.resource_mut::<Run>().talent_runtime.minions.command = Some(p);
        pet_step(&mut world);
        assert!(world.resource::<PendingHits>().0.is_empty());
        for entity in &pets {
            assert!(world.get::<Summon>(*entity).unwrap().charge.is_some());
            assert!(
                world
                    .get::<crate::creature_anim::MovementState>(*entity)
                    .unwrap()
                    .speed
                    > 8.0
            );
        }
        for _ in 0..6 {
            pet_step(&mut world);
        }
        let hits = &world.resource::<PendingHits>().0;
        assert_eq!(
            hits.iter()
                .filter(|(_, h)| h.skill == "kill_command")
                .count(),
            2
        );
        assert!(hits.iter().any(|(_, h)| h.target == left));
        assert!(hits.iter().any(|(_, h)| h.target == right));
    }

    #[test]
    fn kill_command_shares_basic_attack_priorities_and_owner_defense_target() {
        let mut world = hunter_world(&[8], false);
        let pet = world
            .query_filtered::<Entity, With<Summon>>()
            .single(&world)
            .unwrap();
        world.get_mut::<Transform>(pet).unwrap().translation.x = 20.0;
        let near = world
            .spawn((Transform::from_xyz(21.0, 0.0, 0.0), enemy()))
            .id();
        let boss = world
            .spawn((
                Transform::from_xyz(30.0, 0.0, 0.0),
                Enemy {
                    boss: true,
                    ..enemy()
                },
            ))
            .id();
        let owner_threat = world
            .spawn((Transform::from_xyz(-1.0, 0.0, 0.0), enemy()))
            .id();
        let other_threat = world
            .spawn((
                Transform::from_xyz(2.0, 0.0, 0.0),
                Enemy {
                    boss: true,
                    ..enemy()
                },
            ))
            .id();
        for expected in [owner_threat, other_threat, boss, near] {
            let p = world
                .resource::<Run>()
                .resolve_spell(data::ability("kill_command"), 1);
            world.resource_mut::<Run>().talent_runtime.minions.command = Some(p);
            world.get_mut::<Summon>(pet).unwrap().cooldown = 999.0;
            pet_step(&mut world);
            let summon = world.get::<Summon>(pet).unwrap();
            assert_eq!(summon.target, Some(expected));
            if let Some((target, _)) = summon.charge {
                assert_eq!(target, expected);
            } else {
                assert!(world
                    .resource::<PendingHits>()
                    .0
                    .iter()
                    .any(|(_, h)| h.skill == "kill_command" && h.target == expected));
            }
            world.get_mut::<Enemy>(expected).unwrap().dead = true;
        }
    }

    fn restoration_world() -> (World, Entity) {
        use crate::creature_anim::{CastEvent, PlaySeq, SpellGoTargets, SwingMessage};
        let mut world = World::new();
        let mut shaman = run(5, 2, &[8, 12, 18]);
        shaman.hp = 10.0;
        world.insert_resource(shaman);
        world.insert_resource(Rng(42));
        world.init_resource::<Time>();
        world.init_resource::<PendingHits>();
        world.init_resource::<PlaySeq>();
        world.init_resource::<Messages<CastEvent>>();
        world.init_resource::<Messages<SpellGoTargets>>();
        world.init_resource::<Messages<SwingMessage>>();
        world.init_resource::<Messages<crate::combat_text::CombatTextSpawn>>();
        world.init_resource::<Messages<Hit>>();
        world.init_resource::<Messages<super::super::enemies::Loot>>();
        world.init_resource::<Messages<crate::net::FieldChanged>>();
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(100));
        let hero = world
            .spawn((Hero { vy: 0.0, air: 0.0 }, Transform::default()))
            .id();
        world.run_system_once(tick).unwrap();
        (world, hero)
    }
    #[test]
    fn spirit_council_has_two_stationary_totems_and_three_targets_per_volley() {
        let (mut world, hero) = restoration_world();
        assert_eq!(world.query::<&Summon>().iter(&world).count(), 2);
        let targets = world
            .run_system_once(|mut commands: Commands, mut run: ResMut<Run>| {
                [4.0, 5.0, 6.0, 70.0].map(|x| {
                    super::super::enemies::spawn_enemy(
                        &mut commands,
                        &mut run,
                        &data::roster(0).enemies[0],
                        Vec3::X * x,
                        Vec3::ZERO,
                        false,
                    )
                })
            })
            .unwrap();
        for entity in targets {
            world.get_mut::<Enemy>(entity).unwrap().hp = 10000.0;
        }
        for mut totem in world.query::<&mut Summon>().iter_mut(&mut world) {
            totem.cooldown = 0.0;
        }
        world.run_system_once(tick).unwrap();
        assert_eq!(world.resource::<PendingHits>().0.len(), 2);
        world
            .run_system_once(super::super::abilities::apply_hits)
            .unwrap();
        for entity in &targets[..3] {
            assert!(world.get::<Enemy>(*entity).unwrap().hp < 10000.0);
        }
        assert_eq!(world.get::<Enemy>(targets[3]).unwrap().hp, 10000.0);
        let original: Vec<_> = world
            .query::<(Entity, &Transform, &Summon)>()
            .iter(&world)
            .map(|(e, tf, _)| (e, tf.translation))
            .collect();
        world.get_mut::<Transform>(hero).unwrap().translation.x = 30.0;
        world.run_system_once(tick).unwrap();
        for (entity, pos) in &original {
            assert_eq!(world.get::<Transform>(*entity).unwrap().translation, *pos);
        }
        world.resource_mut::<Run>().abilities = vec![Owned {
            def: data::ability("totemic_recall"),
            level: 1,
            cd: 0.0,
        }];
        let cooldowns: Vec<_> = original
            .iter()
            .map(|(e, _)| (*e, world.get::<Summon>(*e).unwrap().cooldown))
            .collect();
        world
            .run_system_once(super::super::abilities::cast_abilities)
            .unwrap();
        for (entity, cd) in cooldowns {
            assert!(
                world
                    .get::<Transform>(entity)
                    .unwrap()
                    .translation
                    .distance(Vec3::X * 30.0)
                    <= 3.01
            );
            assert_eq!(world.get::<Summon>(entity).unwrap().cooldown, cd);
        }
    }
    #[test]
    fn restoration_heals_from_totems_without_stacking_and_keeps_party_blessings() {
        let (mut world, hero) = restoration_world();
        world.get_mut::<Transform>(hero).unwrap().translation.x = 30.0;
        let mut players = Companions::default();
        for (id, x) in [(42, 2.0), (43, 31.0)] {
            let entity = world
                .spawn((RemoteOwner(id), Transform::from_xyz(x, 0.0, 0.0)))
                .id();
            players.0.insert(
                id,
                super::super::multiplayer::Companion {
                    entity,
                    run: Run {
                        owner: Some(id),
                        hp: 10.0,
                        ..default()
                    },
                    physics: Hero { vy: 0.0, air: 0.0 },
                    offer: default(),
                    offer_id: 0,
                },
            );
        }
        world.insert_resource(players);
        world.run_system_once(support).unwrap();
        let healing = world.resource::<Companions>().0[&42].run.hp - 10.0;
        assert!(healing > 0.0);
        assert_eq!(world.resource::<Run>().hp, 10.0);
        let players = world.resource::<Companions>();
        assert_eq!(players.0[&43].run.hp, 10.0);
        assert!(players.0[&43].run.healer_buffs.bonuses.recovery > 0.0);
        assert!(players.0[&43].run.healer_buffs.bonuses.damage > 0.0);
        let one = world
            .query_filtered::<Entity, With<Summon>>()
            .iter(&world)
            .next()
            .unwrap();
        world.despawn(one);
        world
            .resource_mut::<Companions>()
            .0
            .get_mut(&42)
            .unwrap()
            .run
            .hp = 10.0;
        world.run_system_once(support).unwrap();
        assert!((world.resource::<Companions>().0[&42].run.hp - 10.0 - healing).abs() < 0.001);
    }

    #[test]
    fn pet_targets_prioritize_bosses_then_distance_from_the_pet_within_owner_leash() {
        let foes = [
            (Entity::from_bits(1), Vec3::X * 5.0, false),
            (Entity::from_bits(2), Vec3::X * 39.0, false),
            (Entity::from_bits(3), Vec3::X * 70.0, true),
            (Entity::from_bits(4), Vec3::X * 101.0, true),
        ];
        assert_eq!(
            pet_target(foes.into_iter(), Vec3::X * 40.0, Vec3::ZERO),
            Some(foes[2].0)
        );
        assert_eq!(
            pet_target(foes[..2].iter().copied(), Vec3::X * 40.0, Vec3::ZERO),
            Some(foes[1].0)
        );
        assert_eq!(pet_target(std::iter::empty(), Vec3::ZERO, Vec3::ZERO), None);
        let edge = Entity::from_bits(9);
        assert_eq!(
            pet_target(
                [(edge, Vec3::X * 100.0, false)].into_iter(),
                Vec3::X * 99.0,
                Vec3::ZERO
            ),
            Some(edge)
        );
        assert_eq!(
            pet_target(
                [(edge, Vec3::X * 100.1, true)].into_iter(),
                Vec3::X * 99.0,
                Vec3::ZERO
            ),
            None
        );
    }

    #[test]
    fn pet_targets_favor_the_owner_direction_without_ignoring_proximity_or_bosses() {
        let away = (Entity::from_bits(1), Vec3::X * 46.0, false);
        let toward = (Entity::from_bits(2), Vec3::X * 32.0, false);
        let distant = (Entity::from_bits(3), Vec3::X * 20.0, false);
        let boss = (Entity::from_bits(4), Vec3::X * 60.0, true);
        let origin = Vec3::X * 40.0;
        assert_eq!(
            pet_target([away, toward].into_iter(), origin, Vec3::ZERO),
            Some(toward.0)
        );
        assert_eq!(
            pet_target([away, distant].into_iter(), origin, Vec3::ZERO),
            Some(away.0)
        );
        assert_eq!(
            pet_target([away, toward, boss].into_iter(), origin, Vec3::ZERO),
            Some(boss.0)
        );
        // Direction follows the owner's current position, rather than a fixed world axis.
        assert_eq!(
            pet_target([away, toward].into_iter(), origin, Vec3::X * 80.0),
            Some(away.0)
        );
        assert_eq!(
            pet_target([away, toward].into_iter(), origin, origin),
            Some(away.0)
        );
    }

    #[test]
    fn pets_focus_bosses_but_interrupt_them_to_defend_the_owner() {
        let mut world = hunter_world(&[8], false);
        let pet = world
            .query_filtered::<Entity, With<Summon>>()
            .single(&world)
            .unwrap();
        world.get_mut::<Transform>(pet).unwrap().translation.x = 20.0;
        let nearby = world
            .spawn((Transform::from_xyz(21.0, 0.0, 0.0), enemy()))
            .id();
        let boss = world
            .spawn((
                Transform::from_xyz(30.0, 0.0, 0.0),
                Enemy {
                    boss: true,
                    ..enemy()
                },
            ))
            .id();
        {
            let mut summon = world.get_mut::<Summon>(pet).unwrap();
            summon.target = Some(nearby);
            summon.retarget = 999.0;
            summon.cooldown = 999.0;
        }
        pet_step(&mut world);
        assert_eq!(world.get::<Summon>(pet).unwrap().target, Some(boss));
        let danger = world
            .spawn((Transform::from_xyz(1.0, 0.0, 0.0), enemy()))
            .id();
        pet_step(&mut world);
        assert_eq!(world.get::<Summon>(pet).unwrap().target, Some(danger));
        assert!(world.get::<Summon>(pet).unwrap().protecting);
        world.get_mut::<Enemy>(danger).unwrap().dead = true;
        pet_step(&mut world);
        assert_eq!(world.get::<Summon>(pet).unwrap().target, Some(boss));
        assert!(!world.get::<Summon>(pet).unwrap().protecting);
        world.get_mut::<Enemy>(boss).unwrap().dead = true;
        pet_step(&mut world);
        assert_eq!(world.get::<Summon>(pet).unwrap().target, Some(nearby));
    }

    #[test]
    fn pets_reserve_distinct_mobs_share_bosses_and_ignore_other_owners_reservations() {
        let mut world = hunter_world(&[8, 12], true);
        let pets: Vec<_> = world
            .query_filtered::<Entity, With<Summon>>()
            .iter(&world)
            .collect();
        assert_eq!(pets.len(), 3);
        let danger = world
            .spawn((Transform::from_xyz(1.0, 0.0, 0.0), enemy()))
            .id();
        let normal = world
            .spawn((Transform::from_xyz(10.0, 0.0, 0.0), enemy()))
            .id();
        for pet in &pets {
            world.get_mut::<Summon>(*pet).unwrap().cooldown = 999.0;
        }
        pet_step(&mut world);
        let targets: Vec<_> = pets
            .iter()
            .filter_map(|pet| world.get::<Summon>(*pet).unwrap().target)
            .collect();
        assert_eq!(targets.len(), 2);
        assert!(targets.contains(&danger) && targets.contains(&normal));
        let p = world
            .resource::<Run>()
            .resolve_spell(data::ability("kill_command"), 1);
        world.resource_mut::<Run>().talent_runtime.minions.command = Some(p);
        pet_step(&mut world);
        let targets: Vec<_> = pets
            .iter()
            .filter_map(|pet| world.get::<Summon>(*pet).unwrap().target)
            .collect();
        assert_eq!(targets.len(), 2);
        assert!(targets.contains(&danger) && targets.contains(&normal));
        // All three can focus a boss, including when it threatens the owner.
        world.get_mut::<Enemy>(danger).unwrap().boss = true;
        pet_step(&mut world);
        assert!(pets
            .iter()
            .all(|pet| world.get::<Summon>(*pet).unwrap().target == Some(danger)));
        world.get_mut::<Enemy>(danger).unwrap().dead = true;
        pet_step(&mut world);
        assert_eq!(
            pets.iter()
                .filter(|pet| world.get::<Summon>(**pet).unwrap().target == Some(normal))
                .count(),
            1
        );
        // A guest's assignment does not reserve this mob for the host.
        {
            let mut foreign = world.get_mut::<Summon>(pets[0]).unwrap();
            foreign.owner = Some(7);
            foreign.target = Some(normal);
        }
        for pet in &pets[1..] {
            let mut summon = world.get_mut::<Summon>(*pet).unwrap();
            summon.target = None;
            summon.retarget = 0.0;
        }
        pet_step(&mut world);
        assert_eq!(
            pets[1..]
                .iter()
                .filter(|pet| world.get::<Summon>(**pet).unwrap().target == Some(normal))
                .count(),
            1
        );
    }

    #[test]
    fn pets_hold_their_selected_target_and_replace_a_dead_target_immediately() {
        use crate::creature_anim::{CastEvent, PlaySeq, SpellGoTargets, SwingMessage};
        let mut world = World::new();
        world.insert_resource(run(2, 0, &[8, 12]));
        world.insert_resource(Rng(42));
        world.init_resource::<Time>();
        world.init_resource::<PendingHits>();
        world.init_resource::<PlaySeq>();
        world.init_resource::<Messages<CastEvent>>();
        world.init_resource::<Messages<SpellGoTargets>>();
        world.init_resource::<Messages<SwingMessage>>();
        world.init_resource::<Messages<crate::combat_text::CombatTextSpawn>>();
        world.spawn((Hero { vy: 0.0, air: 0.0 }, Transform::default()));
        for x in [4.0, 5.0, 6.0] {
            world.spawn((Transform::from_xyz(x, 0.0, 0.0), enemy()));
        }
        fn step(world: &mut World) {
            world
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_millis(100));
            world.resource_mut::<Run>().elapsed += 0.1;
            world.run_system_once(tick).unwrap();
        }
        step(&mut world); // Spawn the two companions.
        let delays: Vec<_> = world
            .query::<&Summon>()
            .iter(&world)
            .map(|p| p.cooldown)
            .collect();
        assert_eq!(delays.len(), 2);
        assert_ne!(delays[0], delays[1]);
        step(&mut world); // Each companion makes its first target decision.
        let before: Vec<_> = world
            .query::<(Entity, &Summon)>()
            .iter(&world)
            .map(|(e, p)| (e, p.target.unwrap()))
            .collect();
        step(&mut world);
        for (entity, target) in &before {
            assert_eq!(world.get::<Summon>(*entity).unwrap().target, Some(*target));
        }
        let dead = before[0].1;
        world.get_mut::<Enemy>(dead).unwrap().dead = true;
        step(&mut world);
        for pet in world.query::<&Summon>().iter(&world) {
            assert!(pet.target.is_some());
            assert_ne!(pet.target, Some(dead));
        }
        let entity = before[0].0;
        world.get_mut::<Transform>(entity).unwrap().translation = Vec3::X * 100.0;
        step(&mut world);
        assert!(
            world.get::<Transform>(entity).unwrap().translation.length() > 90.0,
            "exactly 100 yd must not teleport"
        );
        world.get_mut::<Transform>(entity).unwrap().translation = Vec3::X * 100.1;
        let cooldown = world.get::<Summon>(entity).unwrap().cooldown;
        step(&mut world);
        let pet = world.get::<Summon>(entity).unwrap();
        assert_eq!(
            world.get::<Transform>(entity).unwrap().translation,
            Vec3::X * (2.0 + f32::from(pet.slot))
        );
        assert!(pet.target.is_none());
        assert_eq!(pet.retarget, 0.0);
        assert!((pet.cooldown - (cooldown - 0.1)).abs() < 0.001);
        let mv = world
            .get::<crate::creature_anim::MovementState>(entity)
            .unwrap();
        assert_eq!(mv.speed, 0.0);
        assert_eq!(mv.flags, 0);
        step(&mut world);
        assert!(world.get::<Summon>(entity).unwrap().target.is_some());
        let far = world.get::<Summon>(entity).unwrap().target.unwrap();
        world.get_mut::<Transform>(far).unwrap().translation = Vec3::X * 100.1;
        step(&mut world);
        assert_ne!(
            world.get::<Summon>(entity).unwrap().target,
            Some(far),
            "abandon an out-of-range target without waiting for the retarget timer"
        );
        let enemies: Vec<_> = world
            .query_filtered::<Entity, With<Enemy>>()
            .iter(&world)
            .collect();
        for enemy in enemies {
            world.get_mut::<Transform>(enemy).unwrap().translation = Vec3::X * 101.0;
        }
        step(&mut world);
        assert!(world.get::<Summon>(entity).unwrap().target.is_none());
    }

    #[test]
    fn unchanged_party_auras_do_not_dirty_the_build_every_frame() {
        let mut world = World::new();
        world.insert_resource(Run::default());
        world.init_resource::<Time>();
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(50));
        world.spawn((Hero { vy: 0.0, air: 0.0 }, Transform::default()));
        let mut ally = Run {
            class: &data::CLASSES[8],
            owner: Some(42),
            ..default()
        };
        ally.talent_build[2][12] = 1;
        configure(&mut ally);
        let entity = world.spawn((RemoteOwner(42), Transform::default())).id();
        let mut players = Companions::default();
        players.0.insert(
            42,
            super::super::multiplayer::Companion {
                entity,
                run: ally,
                physics: Hero { vy: 0.0, air: 0.0 },
                offer: default(),
                offer_id: 0,
            },
        );
        world.insert_resource(players);
        let base = world
            .resource::<Run>()
            .resolve_spell(data::ability("cleave"), 1)
            .dmg;
        world.run_system_once(support).unwrap();
        let version = world.resource::<Run>().version;
        assert!(
            world
                .resource::<Run>()
                .resolve_spell(data::ability("cleave"), 1)
                .dmg
                > base
        );
        for _ in 0..5 {
            world.run_system_once(support).unwrap();
            assert_eq!(world.resource::<Run>().version, version);
        }
        world.get_mut::<Transform>(entity).unwrap().translation = Vec3::X * 100.0;
        world.run_system_once(support).unwrap();
        assert_eq!(world.resource::<Run>().version, version + 1);
        assert_eq!(
            world
                .resource::<Run>()
                .resolve_spell(data::ability("cleave"), 1)
                .dmg,
            base
        );
        world.run_system_once(support).unwrap();
        assert_eq!(world.resource::<Run>().version, version + 1);
    }
    fn run(class: usize, tree: usize, nodes: &[usize]) -> Run {
        let mut run = Run {
            class: &data::CLASSES[class],
            balance: super::super::balance::Config::parse("version=1\n[classes]\n[tags]\n")
                .unwrap(),
            ..default()
        };
        for &node in nodes {
            run.talent_build[tree][node] = 1;
        }
        configure(&mut run);
        run
    }
    fn enemy() -> Enemy {
        Enemy {
            def: &data::roster(0).enemies[0],
            hp: 100.0,
            max_hp: 100.0,
            speed: 0.0,
            damage: 0.0,
            radius: 1.0,
            xp: 1,
            boss: false,
            attack_cd: 0.0,
            slow: 1.0,
            slow_t: 0.0,
            root_t: 0.0,
            stun_t: 0.0,
            fear_t: 0.0,
            weaken: 1.0,
            weaken_t: 0.0,
            dots: vec![],
            dead: false,
        }
    }
    #[test]
    fn generic_hit_milestones_accept_all_damage_schools_and_spell_shapes() {
        let mut rng = Rng(42);
        for skill in data::ABILITIES
            .iter()
            .filter(|s| !s.key.starts_with("proc_") && tags::for_spell(s).contains(Tags::DAMAGE))
        {
            let mut poison = run(3, 0, &[8]);
            let mut enemy = enemy();
            let mut pending = PendingHits::default();
            let h = Hit::direct(skill.key, Entity::from_bits(1), 10.0, false);
            on_hit(
                &mut poison,
                &h,
                &mut enemy,
                Vec3::ZERO,
                &mut rng,
                &mut pending,
            );
            assert_eq!(enemy.dots.len(), 1, "{} did not apply poison", skill.key);
        }
        for (class, tree, hits) in [(3, 1, 3), (5, 1, 5), (7, 2, 3)] {
            let mut run = run(class, tree, &[18]);
            run.elapsed = 2.0;
            let mut enemy = enemy();
            let mut pending = PendingHits::default();
            for index in 0..hits {
                let h = Hit::direct(
                    ["cleave", "fireball", "frostbolt", "shadow_bolt", "moonfire"][index],
                    Entity::from_bits(1),
                    10.0,
                    false,
                );
                on_hit(&mut run, &h, &mut enemy, Vec3::ZERO, &mut rng, &mut pending);
            }
            assert_eq!(run.talent_runtime.bursts.len(), 1, "counter {class}/{tree}");
        }
    }
    #[test]
    fn proc_rules_all_proc_origins_are_rejected_as_milestone_sources() {
        use super::super::items::legendary::events::Origin;
        for (origin, triggered, key) in [
            (Origin::Talent, false, "fireball"),
            (Origin::Item, false, "fireball"),
            (Origin::Legendary, false, "fireball"),
            (Origin::Skill, true, "fireball"),
            (Origin::Skill, false, "proc_fireburst"),
        ] {
            let mut run = run(3, 0, &[8]);
            let mut en = enemy();
            let mut pending = PendingHits::default();
            let mut rng = Rng::default();
            let mut hit = Hit::direct(key, Entity::from_bits(1), 100.0, false);
            hit.origin = origin;
            hit.triggered = triggered;
            on_hit(&mut run, &hit, &mut en, Vec3::ZERO, &mut rng, &mut pending);
            assert!(en.dots.is_empty());
            assert!(pending.0.is_empty());
            assert!(run.talent_runtime.bursts.is_empty());
        }
    }

    #[test]
    fn distinct_procs_cannot_trigger_other_milestones() {
        assert!((M::Grove as u32) < 64);
        let mut paladin = run(1, 2, &[8, 12]);
        let mut enemy = enemy();
        let mut rng = Rng(42);
        let mut pending = PendingHits::default();
        let root = Hit::direct("fireball", Entity::from_bits(1), 10.0, false);
        on_hit(
            &mut paladin,
            &root,
            &mut enemy,
            Vec3::ZERO,
            &mut rng,
            &mut pending,
        );
        assert_eq!(pending.0.len(), 1); // Seal accepts this Fire SPELL hit.
        let seal = pending.0.remove(0).1;
        assert_eq!(seal.proc_chain, chain_bit(M::Seal));
        paladin.talent_runtime.bursts.clear();
        on_hit(
            &mut paladin,
            &seal,
            &mut enemy,
            Vec3::ZERO,
            &mut rng,
            &mut pending,
        );
        assert!(pending.0.is_empty(), "Seal recursively triggered itself");
        assert!(paladin.talent_runtime.bursts.is_empty());
        let mut child = Hit::direct("proc_splash", root.target, 10.0, false);
        child.triggered = true;
        child.proc_chain = chain_bit(M::Splash);
        on_hit(
            &mut paladin,
            &child,
            &mut enemy,
            Vec3::ZERO,
            &mut rng,
            &mut pending,
        );
        assert!(pending.0.is_empty());
        assert!(
            paladin.talent_runtime.bursts.is_empty(),
            "Splash cycled back to Seal or itself"
        );
    }
    #[test]
    fn milestone_hit_counters_ignore_incompatible_schools() {
        for (class, tree, compatible, other, count) in [(8, 0, "starfire", "fireball", 5)] {
            let mut run = run(class, tree, &[18]);
            run.elapsed = 2.0;
            let mut enemy = enemy();
            let mut rng = Rng::default();
            let mut pending = PendingHits::default();
            for _ in 0..count - 1 {
                let h = Hit::direct(compatible, Entity::from_bits(1), 10.0, false);
                on_hit(&mut run, &h, &mut enemy, Vec3::ZERO, &mut rng, &mut pending);
                let h = Hit::direct(other, Entity::from_bits(1), 10.0, false);
                on_hit(&mut run, &h, &mut enemy, Vec3::ZERO, &mut rng, &mut pending);
            }
            assert!(
                run.talent_runtime.bursts.is_empty(),
                "wrong school advanced {class}/{tree}"
            );
            let h = Hit::direct(compatible, Entity::from_bits(1), 10.0, false);
            on_hit(&mut run, &h, &mut enemy, Vec3::ZERO, &mut rng, &mut pending);
            assert_eq!(run.talent_runtime.bursts.len(), 1);
        }
    }
    #[test]
    fn physical_spells_and_paladin_holy_skills_trigger_milestones_without_auto_attacks() {
        let mut poison = run(3, 0, &[8]);
        let mut enemy = enemy();
        let mut rng = Rng::default();
        let mut pending = PendingHits::default();
        // A SPELL + PHYSICAL definition proves this condition does not require ATTACK.
        let mut h = Hit::direct("cleave", Entity::from_bits(1), 10.0, false);
        h.tags = Some(Tags::SPELL | Tags::PHYSICAL | Tags::DAMAGE);
        on_hit(
            &mut poison,
            &h,
            &mut enemy,
            Vec3::ZERO,
            &mut rng,
            &mut pending,
        );
        assert_eq!(enemy.dots.len(), 1);
        enemy.dots.clear();
        h.triggered = true;
        h.proc_chain = chain_bit(M::Poison);
        on_hit(
            &mut poison,
            &h,
            &mut enemy,
            Vec3::ZERO,
            &mut rng,
            &mut pending,
        );
        assert!(
            enemy.dots.is_empty(),
            "a milestone already in the ancestry must not trigger again"
        );
        let mut paladin = run(1, 2, &[8, 12]);
        let h = Hit::direct("seal_of_righteousness", Entity::from_bits(1), 10.0, false);
        on_hit(
            &mut paladin,
            &h,
            &mut enemy,
            Vec3::ZERO,
            &mut rng,
            &mut pending,
        );
        assert_eq!(pending.0.len(), 1);
        assert_eq!(paladin.talent_runtime.bursts.len(), 1);
    }
    #[test]
    fn all_specializations_have_three_distinct_authored_milestones() {
        assert_eq!(talent_design::DESIGNS.len(), 27);
        for design in &talent_design::DESIGNS {
            let mut names = std::collections::BTreeSet::new();
            for milestone in design.milestones {
                assert!(names.insert(milestone.name));
                assert!(!milestone.description.is_empty());
                assert!(milestone.mechanic != M::None || !milestone.changes.is_empty());
            }
        }
    }
    #[test]
    fn poison_stacks_without_refreshing_old_applications_and_cannot_reenter_its_proc_chain() {
        let mut run = run(3, 0, &[8]);
        run.owner = Some(7);
        let mut enemy = enemy();
        let mut rng = Rng::default();
        let mut pending = PendingHits::default();
        let mut hit = Hit::direct("sinister_strike", Entity::from_bits(1), 100.0, false);
        hit.owner = Some(7);
        for _ in 0..10 {
            on_hit(
                &mut run,
                &hit,
                &mut enemy,
                Vec3::ZERO,
                &mut rng,
                &mut pending,
            );
        }
        assert_eq!(enemy.dots.len(), 1);
        assert_eq!(enemy.dots[0].skill, "proc_poison");
        assert_eq!(enemy.dots[0].owner, Some(7));
        assert_eq!(enemy.dots[0].dps, 18.0);
        enemy.dots[0].tick = 0.25;
        enemy.dots[0].left = 2.0;
        run.elapsed += 0.5;
        on_hit(
            &mut run,
            &hit,
            &mut enemy,
            Vec3::ZERO,
            &mut rng,
            &mut pending,
        );
        assert_eq!(
            enemy.dots[0].tick, 0.25,
            "Adding a stack must not postpone the next damage tick"
        );
        assert_eq!(enemy.dots.len(), 2);
        assert_eq!(enemy.dots[0].left, 2.0);
        assert_eq!(enemy.dots[1].left, 6.0);
        assert_eq!(enemy.dots[1].tick, 0.25);
        for _ in 0..10 {
            run.elapsed += 0.5;
            on_hit(&mut run, &hit, &mut enemy, Vec3::ZERO, &mut rng, &mut pending);
        }
        assert_eq!(enemy.dots.len(), 12);
        assert!(enemy.dots.iter().all(|d| d.skill == "proc_poison" && d.owner == Some(7) && d.dps == 18.0));
        hit.triggered = true;
        hit.proc_chain = chain_bit(M::Poison);
        enemy.dots.clear();
        on_hit(
            &mut run,
            &hit,
            &mut enemy,
            Vec3::ZERO,
            &mut rng,
            &mut pending,
        );
        assert!(enemy.dots.is_empty());
    }
    #[test]
    fn all_milestone_proliferation_preserves_remaining_lifetime_and_tick_timing() {
        for (class, tree, node, skill) in [
            (3, 0, 12, "deadly_poison"), (6, 1, 12, "proc_ignite"),
            (7, 0, 12, "corruption"), (4, 2, 18, "corruption"),
            (2, 2, 18, "explosive_trap"),
        ] {
            let mut world = hunter_world(&[], false);
            let mut player = run(class, tree, &[node]);
            let mut corpse = enemy();
            corpse.dots.push(Dot {
                owner: None, skill, meter_source: None, tags: None, item_source: None,
                origin: super::super::items::legendary::events::Origin::Skill,
                meta: Default::default(), dps: 7.0, left: 1.2, tick: 0.4,
            });
            on_death(&mut player, &corpse, Vec3::ZERO);
            assert_eq!(player.talent_runtime.bursts[0].dots.len(), 1, "{skill}");
            world.insert_resource(player);
            let target = world.spawn((Transform::from_xyz(2.0, 0.0, 0.0), enemy())).id();
            pet_step(&mut world);
            let copied = &world.get::<Enemy>(target).unwrap().dots[0];
            assert_eq!(copied.skill, skill);
            assert_eq!(copied.dps, 7.0);
            assert!((copied.left - 1.1).abs() < 0.001, "{skill}: {}", copied.left);
            assert!((copied.tick - 0.3).abs() < 0.001, "{skill}");
            let mut next_corpse = enemy();
            next_corpse.dots.push(copied.clone());
            next_corpse.dots[0].left = 0.0;
            world.resource_scope(|_, mut player: Mut<Run>| on_death(&mut player, &next_corpse, Vec3::ZERO));
            assert!(world.resource::<Run>().talent_runtime.bursts.iter().all(|b| b.dots.is_empty()));
        }
    }

    #[test]
    fn proliferation_and_death_bursts_require_the_owners_condition() {
        let mut run = run(3, 0, &[8, 12, 18]);
        run.owner = Some(7);
        let mut enemy = enemy();
        enemy.dots.push(Dot {
            meter_source: None,
            tags: None,
            owner: Some(8),
            skill: "deadly_poison",
            item_source: None,
            origin: crate::survivors::items::legendary::events::Origin::Talent,
            meta: Default::default(),
            dps: 10.0,
            left: 4.0,
            tick: 1.0,
        });
        on_death(&mut run, &enemy, Vec3::ZERO);
        assert!(run.talent_runtime.bursts.is_empty());
        enemy.dots[0].owner = Some(7);
        on_death(&mut run, &enemy, Vec3::ZERO);
        let burst = &run.talent_runtime.bursts[0];
        assert_eq!(burst.damage, 30.000002);
        assert_eq!(burst.dots.len(), 1);
        assert_eq!(burst.dots[0].owner, Some(7));
        assert_eq!(burst.meter_source, Some("Poison Bomb"));
        assert_eq!(burst.spread_source, Some("Toxic Contagion"));
    }

    #[test]
    fn unstable_affliction_stacks_with_independent_durations_without_recursive_procs() {
        let mut run = run(7, 0, &[8]);
        let spec = &super::super::talents::TREES[7][0];
        run.replace_spell_modifiers(908, super::super::talent_effects::effects(spec, 8, 1)
            .into_iter().map(|change| super::super::spell_engine::Modifier {
                source: 908,
                filter: super::super::talent_effects::filter(spec, 8, change),
                change,
            }));
        let mut target = enemy();
        let mut rng = Rng(42);
        let mut pending = PendingHits::default();
        let mut hit = Hit::direct("shadow_bolt", Entity::from_bits(1), 100.0, false);
        on_hit(&mut run, &hit, &mut target, Vec3::ZERO, &mut rng, &mut pending);
        assert_eq!(target.dots.len(), 1);
        target.dots[0].left = 2.0;
        target.dots[0].tick = 0.25;
        run.elapsed += 0.5;
        on_hit(&mut run, &hit, &mut target, Vec3::ZERO, &mut rng, &mut pending);
        assert_eq!(target.dots.len(), 2);
        assert_eq!(target.dots[0].left, 2.0);
        assert_eq!(target.dots[1].left, 6.0);
        assert_eq!(target.dots[1].tick, 0.25);
        for _ in 0..10 {
            run.elapsed += 0.5;
            on_hit(&mut run, &hit, &mut target, Vec3::ZERO, &mut rng, &mut pending);
        }
        assert_eq!(target.dots.len(), 12);
        assert!(target.dots.iter().all(|d| d.skill == "proc_plague" && (d.dps - 22.5).abs() < 0.001));
        target.dots.clear();
        hit.triggered = true;
        run.elapsed += 0.5;
        on_hit(&mut run, &hit, &mut target, Vec3::ZERO, &mut rng, &mut pending);
        assert!(target.dots.is_empty());
    }
    #[test]
    fn bleeding_death_explosions_use_the_specializations_milestone_name() {
        for (class, tree, node, name) in [
            (0, 1, 12, "Bloodbath"),
            (2, 0, 18, "Pack Frenzy"),
            (8, 1, 18, "Savage Carnage"),
        ] {
            let mut run = run(class, tree, &[node]);
            let mut enemy = enemy();
            enemy.dots.push(Dot {
                owner: None,
                skill: "rend",
                meter_source: None,
                tags: None,
                item_source: None,
                origin: crate::survivors::items::legendary::events::Origin::Skill,
                meta: Default::default(),
                dps: 5.0,
                left: 4.0,
                tick: 1.0,
            });
            on_death(&mut run, &enemy, Vec3::ZERO);
            let burst = &run.talent_runtime.bursts[0];
            assert_eq!(burst.meter_source, Some(name));
            assert_eq!(
                burst.skill, "arcane_explosion",
                "Changing the meter label must not replace spell rules or visuals"
            );
        }
    }

    #[test]
    fn death_explosions_cap_health_base_then_apply_source_modifiers_and_crit_once() {
        use super::super::spell_engine::{Change, Filter, Modifier};
        for (class, tree, node, dot, frozen, school, name) in [
            (0, 1, 12, "rend", false, Tags::PHYSICAL, "Bloodbath"),
            (2, 0, 18, "rend", false, Tags::PHYSICAL, "Pack Frenzy"),
            (8, 1, 18, "rend", false, Tags::PHYSICAL, "Savage Carnage"),
            (2, 2, 18, "corruption", false, Tags::FIRE, "Death Trap"),
            (
                3,
                0,
                18,
                "deadly_poison",
                false,
                Tags::NATURE,
                "Poison Bomb",
            ),
            (6, 1, 18, "fireball", false, Tags::FIRE, "Combustion"),
            (7, 0, 18, "corruption", false, Tags::SHADOW, "Doom Eruption"),
            (
                4,
                2,
                18,
                "corruption",
                false,
                Tags::SHADOW,
                "Shadow Pandemic",
            ),
            (6, 2, 18, "corruption", true, Tags::COLD, "Endless Winter"),
        ] {
            for max_hp in [100.0, 10_000.0] {
                let mut world = hunter_world(&[], false);
                let mut player = run(class, tree, &[node]);
                let pack = name == "Pack Frenzy";
                player.replace_spell_modifiers(
                    907,
                    [
                        (Tags(0), Change::IncreasedDamage(1.0)),
                        (school, Change::IncreasedDamage(1.0)),
                        (Tags::ARCANE, Change::MoreDamage(99.0)),
                        (Tags(0), Change::CriticalChance(1.0)),
                        (Tags(0), Change::CriticalMultiplier(1.0)),
                        (Tags::MINION, Change::IncreasedDamage(1.0)),
                    ]
                    .into_iter()
                    .map(|(tags, change)| Modifier {
                        source: 907,
                        filter: Filter::requiring(tags),
                        change,
                    }),
                );
                let mut corpse = enemy();
                corpse.max_hp = max_hp;
                corpse.root_t = if frozen { 1.0 } else { 0.0 };
                corpse.dots.push(Dot {
                    owner: None,
                    skill: dot,
                    meter_source: None,
                    tags: None,
                    item_source: None,
                    origin: super::super::items::legendary::events::Origin::Skill,
                    meta: Default::default(),
                    dps: 5.0,
                    left: 4.0,
                    tick: 1.0,
                });
                on_death(&mut player, &corpse, Vec3::ZERO);
                let base = (max_hp * 0.3).min(1000.0);
                assert_eq!(player.talent_runtime.bursts.len(), 1, "{name}");
                assert!((player.talent_runtime.bursts[0].damage - base).abs() < 0.001);
                assert!((player.talent_runtime.bursts[0].radius.powi(2) - 72.0).abs() < 0.001);
                world.insert_resource(player);
                world.spawn((Transform::from_xyz(8.0, 0.0, 0.0), enemy()));
                world.spawn((Transform::from_xyz(12.0, 0.0, 0.0), enemy()));
                pet_step(&mut world);
                let hits = &world.resource::<PendingHits>().0;
                assert_eq!(hits.len(), 1, "{name}");
                let hit = &hits[0].1;
                assert_eq!(hit.meter_source, Some(name));
                assert!(hit.tags.unwrap().contains(school), "{name}");
                assert_eq!(hit.tags.unwrap().contains(Tags::MINION), pack);
                assert!(hit.crit && hit.proc_resolved, "{name}");
                let expected = base * if pack { 4.0 } else { 3.0 } * 3.0;
                assert!(
                    (hit.damage - expected).abs() < 0.01,
                    "{name}: {} != {expected}",
                    hit.damage
                );
            }
        }
    }
    #[test]
    fn focus_resets_on_switch_and_freeze_increases_damage_only_when_frozen() {
        let mut focused = run(3, 2, &[8]);
        let mut enemy = enemy();
        let mut h = Hit::direct("sinister_strike", Entity::from_bits(1), 10.0, false);
        assert_eq!(before_hit(&mut focused, &h, &enemy), 1.0);
        assert_eq!(before_hit(&mut focused, &h, &enemy), 1.08);
        h.target = Entity::from_bits(2);
        assert_eq!(before_hit(&mut focused, &h, &enemy), 1.0);
        let mut frost = run(6, 2, &[8, 12]);
        h.skill = "frostbolt";
        assert_eq!(before_hit(&mut frost, &h, &enemy), 1.0);
        enemy.root_t = 1.0;
        assert_eq!(before_hit(&mut frost, &h, &enemy), 1.6);
    }
    #[test]
    fn proc_rules_triggered_shields_cannot_trigger_break_or_refresh_bursts() {
        let mut run = run(4, 0, &[8, 12, 18]);
        run.hp = 100.0;
        run.shield = 40.0;
        run.shield_t = 5.0;
        run.talent_runtime.shield_capacity = 40.0;
        run.legendary.events.shield_origin = super::super::items::legendary::events::Origin::Talent;
        super::super::hero::hurt_hero(&mut run, 100.0);
        assert_eq!(run.shield, 0.0);
        assert_eq!(run.talent_runtime.shield_burst, 0.0);
        run.shield = 30.0;
        refreshed_shield(&mut run, 50.0);
        assert_eq!(run.talent_runtime.shield_burst, 0.0);
    }

    #[test]
    fn shield_break_and_refresh_queue_one_explosion_and_tanks_block() {
        let mut run = run(4, 0, &[8, 12, 18]);
        run.hp = 100.0;
        run.shield = 40.0;
        run.shield_t = 5.0;
        run.talent_runtime.shield_capacity = 40.0;
        super::super::hero::hurt_hero(&mut run, 100.0);
        assert_eq!(run.talent_runtime.shield_burst, 40.0);
        super::super::hero::hurt_hero(&mut run, 10.0);
        assert_eq!(run.talent_runtime.shield_burst, 40.0);
        run.shield = 30.0;
        run.shield_t = 5.0;
        refreshed_shield(&mut run, 50.0);
        assert_eq!(run.talent_runtime.shield_burst, 80.0);
        assert_eq!(run.talent_runtime.shield_capacity, 50.0);
        let mut tank = self::run(0, 2, &[8, 12]);
        assert_eq!(
            super::super::hero::hurt_hero_with_block(&mut tank, 10.0, 0.1),
            0.0
        );
        assert!(tank.talent_runtime.blocked);
    }

    #[test]
    fn combat_milestones_generate_their_exact_area_and_target_budget() {
        let mut run = run(3, 1, &[18]);
        let mut enemy = enemy();
        let mut rng = Rng::default();
        let mut pending = PendingHits::default();
        let hit = Hit::direct("sinister_strike", Entity::from_bits(1), 100.0, false);
        for _ in 0..3 {
            on_hit(
                &mut run,
                &hit,
                &mut enemy,
                Vec3::ZERO,
                &mut rng,
                &mut pending,
            );
        }
        let burst = &run.talent_runtime.bursts[0];
        assert_eq!(burst.radius, 6.0);
        assert_eq!(burst.damage, 80.0);
        assert_eq!(burst.skill, "blade_flurry");
        let mut shaman = self::run(5, 1, &[18]);
        for _ in 0..5 {
            on_hit(
                &mut shaman,
                &hit,
                &mut enemy,
                Vec3::ZERO,
                &mut rng,
                &mut pending,
            );
        }
        let burst = &shaman.talent_runtime.bursts[0];
        assert_eq!(burst.radius, 8.0);
        assert_eq!(burst.targets, 4);
        assert_eq!(burst.damage, 100.0);
    }
}
