//! Stateful milestones live on each player's Run, including on the host for guests.
use super::procs;
use super::{
    abilities::{Hit, PendingHits},
    data,
    enemies::{Dot, Enemy},
    hero::Hero,
    multiplayer::{Companions, RemoteOwner},
    spell_engine::{tags, Change, Filter, Modifier, Tags},
    talent_design::{self, Mechanic as M},
    units::{self, Fx, UnitSpec},
    Owned, Rng, Run,
};
use bevy::prelude::*;

#[derive(Default)]
pub(super) struct Runtime {
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
    party_buff: f32,
    bursts: Vec<Burst>,
}
struct Burst {
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
}

const PET_LEASH: f32 = 100.0;
fn within_pet_leash(position: Vec3, owner: Vec3) -> bool {
    position.distance_squared(owner) <= PET_LEASH * PET_LEASH
}

/// Stay near the owner's current fight, prefer reachable enemies, then vary between three.
/// The caller retains the result between decisions, avoiding frame-to-frame wandering.
fn pet_target(
    foes: impl Iterator<Item = (Entity, Vec3)>,
    origin: Vec3,
    owner_pos: Vec3,
    range: f32,
    rng: &mut Rng,
) -> Option<Entity> {
    let mut candidates: Vec<_> = foes
        .filter(|(_, position)| within_pet_leash(*position, owner_pos))
        .map(|(entity, pos)| {
            (
                entity,
                units::flat(pos - owner_pos).length(),
                units::flat(pos - origin).length(),
            )
        })
        .collect();
    candidates.sort_by(|a, b| {
        a.1.total_cmp(&b.1)
            .then_with(|| a.0.to_bits().cmp(&b.0.to_bits()))
    });
    let nearest = candidates.first()?.1;
    let limit = nearest + 6.0;
    let reachable = candidates
        .iter()
        .any(|(_, distance, pet_distance)| *distance <= limit && *pet_distance <= range);
    candidates.retain(|(_, distance, pet_distance)| {
        *distance <= limit && (!reachable || *pet_distance <= range)
    });
    let count = candidates.len().min(3);
    if count == 0 {
        return None;
    }
    Some(candidates[(rng.next_u64() % count as u64) as usize].0)
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
    active(run, m) && h.proc_chain & chain_bit(m) == 0 && tags.contains(hit_tags(m))
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
pub(super) fn ailment_source(run: &Run, spell: u32, roll: f32) -> Option<&'static str> {
    match spell {
        1943 => meter_source(run, M::Bleed).or_else(|| {
            if roll < 0.5 {
                meter_source(run, M::Pet)
            } else {
                None
            }
        }),
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
    if class == 2 && run.talent_build[2][8] > 0 {
        unlocks.push("raptor_strike");
    }
    if class == 2 && run.talent_build[2][12] > 0 {
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
    if run.shield > 0.0 && run.shield_t > 0.0 && active(run, M::Shieldburst) {
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
pub(super) fn on_hit(
    run: &mut Run,
    h: &Hit,
    en: &mut Enemy,
    pos: Vec3,
    rng: &mut Rng,
    pending: &mut PendingHits,
) {
    if h.damage <= 0.0 || (run.talent_runtime.enabled.is_empty() && run.legendary.grants.is_empty())
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
    let plague = accepts(run, h, M::ShadowPlague, t) || accepts(run, h, M::Plague, t) || arcane;
    if (poison || plague) && en.hp > 0.0 {
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
        if !run.legendary.has(230) {
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
    if accepts(run, h, M::Freeze, t) {
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
    let now = run.elapsed;
    let mut extra = |key, damage, splash, source, mechanic| {
        let mut hit = Hit::direct(key, h.target, damage, false);
        hit.owner = h.owner;
        hit.triggered = true;
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
    if accepts(run, h, M::Seal, t) {
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
    if accepts(run, h, M::Windfury, t) && rng.f32() < 0.2 {
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
    if accepts(run, h, M::Execution, t) && run.talent_runtime.streak >= 10 {
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
    if active(run, M::ShadowDrain) && tags.contains(Tags::SHADOW) {
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
    let spread = (active(run, M::Proliferate) && poison)
        || (active(run, M::IgniteSpread) && fire)
        || (active(run, M::PlagueSpread) && !owned.is_empty());
    let freeze = active(run, M::FrostNova) && en.root_t > 0.0;
    let explode = bleed
        || active(run, M::Poisonburst) && poison
        || active(run, M::Fireburst) && fire
        || active(run, M::Plagueburst) && !owned.is_empty()
        || super::items::legendary::milestones::sources(run, M::PlagueSpread).contains(&5)
            && active(run, M::PlagueSpread)
            && !owned.is_empty()
        || freeze;
    if spread || explode {
        run.talent_runtime.bursts.push(Burst {
            chain: 0,
            pos,
            damage: if explode {
                (en.max_hp * 0.3).min(300.0)
            } else {
                0.0
            },
            exclude: None,
            radius: 6.0,
            targets: 64,
            dots: if spread { owned } else { vec![] },
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
                } else {
                    M::PlagueSpread
                },
            ),
        });
    }
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
    if dt <= 0.0 || run.dead_t.is_some() {
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
        let p = procs::resolve(
            &run,
            skill,
            burst.damage,
            0.0,
            if burst.freeze { 2.0 } else { 0.0 },
            burst.radius,
            burst.targets,
        );
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
                hit.proc_resolved = true;
                hit.tags = Some(p.tags);
                hit.direct_dot = p.direct_dot.map(|mut dot| {
                    dot.dps /= count;
                    dot
                });
                hit.proc_chain = burst.chain;
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
                    dot.left = dot
                        .left
                        .max(procs::resolve(&run, spread_key, 0.0, 0.0, 3.0, 0.0, 1).duration);
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
                hit.proc_resolved = true;
                hit.direct_dot = p.direct_dot;
                hit.tags = Some(p.tags);
                hit.proc_chain = burst.chain;
                hit.meter_source = burst.meter_source;
                for repeat in 0..repeats {
                    pending.0.push((
                        now + super::spell_engine::RECAST_DELAY * (repeat + 1) as f32,
                        hit.clone(),
                    ));
                }
                pending.0.push((now, hit));
                fx.impact(entity, data::ability(burst.skill).spell);
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
        u8::from(active(&run, M::HealTotem)),
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
                });
                if let Some(owner) = owner {
                    commands
                        .entity(entity)
                        .insert(super::multiplayer::OwnedEffect(owner));
                }
            }
        }
    }
    for (entity, mut tf, mut pet, mut mv) in &mut pets {
        if pet.owner != owner {
            continue;
        }
        let demon = pet.family == 1;
        if pet.carry_until < now {
            pet.carried = None;
        }
        if pet.slot >= counts[pet.family as usize] {
            commands.entity(entity).despawn();
            continue;
        }
        if !within_pet_leash(tf.translation, pos) {
            tf.translation = pos + Vec3::X * (2.0 + f32::from(pet.slot));
            pet.target = None;
            pet.retarget = 0.0;
            pet.cooldown -= dt;
            mv.speed = 0.0;
            mv.flags = 0;
            continue;
        }
        let ranged = demon || pet.stationary;
        let range = if ranged { 20.0 } else { 3.0 };
        pet.retarget -= dt;
        let valid = pet
            .target
            .and_then(|e| enemies.get(e).ok())
            .is_some_and(|(_, tf, en)| !en.dead && within_pet_leash(tf.translation, pos));
        if !valid || pet.retarget <= 0.0 {
            pet.target = pet_target(
                enemies
                    .iter()
                    .filter(|(_, _, en)| !en.dead)
                    .map(|(e, tf, _)| (e, tf.translation)),
                tf.translation,
                pos,
                range,
                &mut rng,
            );
            pet.retarget = rng.range(1.2, 2.4);
        }
        let target = pet.target.and_then(|e| enemies.get(e).ok());
        if !pet.stationary && run.legendary.has(185) {
            if let Some((e, _, _)) = enemies
                .iter()
                .filter(|(_, _, en)| {
                    !en.dead
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
        let destination = if pet.stationary {
            pos + Vec3::X * 2.0
        } else {
            target.as_ref().map_or(pos, |t| t.1.translation)
        };
        let destination = if run.legendary.has(188) && !pet.stationary {
            run.legendary
                .casting
                .spirit_destination
                .unwrap_or(destination)
        } else {
            destination
        };
        let delta = units::flat(destination - tf.translation);
        let len = delta.length();
        let move_by = (dt * 8.0).min((len - 1.5).max(0.0));
        tf.translation += delta.normalize_or_zero() * move_by;
        tf.translation.y = pos.y;
        mv.speed = if move_by > 0.0 { 8.0 } else { 0.0 };
        mv.flags = if move_by > 0.0 { 1 } else { 0 };
        pet.cooldown -= dt;
        if !pet.stationary {
            run.legendary.casting.pet = Some(tf.translation);
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
                let damage = (if pet.stationary { 24.0 } else { 18.0 })
                    * if demon && active(&run, M::DemonLord) {
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
                pet.cooldown = procs::resolve(&run, skill, damage, 0.0, 0.0, 0.0, 1).cooldown;
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
                if (!demon && !pet.stationary && active(&run, M::Pack))
                    || (demon && active(&run, M::Demons))
                {
                    hit.dot = damage * 0.2;
                    hit.dot_dur = 4.0;
                }
                if demon && active(&run, M::DemonLord) {
                    hit.splash = 4.0;
                }
                pending.0.push((now, hit.clone()));
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
        pos: Vec3,
        rate: (f32, f32),
        pulse: (f32, f32),
        flat: (f32, f32),
        buff: (f32, f32),
    }
    fn source(run: &mut Run, pos: Vec3) -> Source {
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
        let buff = procs::resolve(
            run,
            "proc_ally_buff",
            if active(run, M::AllyBuff) { 0.15 } else { 0.0 },
            0.0,
            0.0,
            12.0,
            1,
        );
        Source {
            pos,
            rate: (rate.dmg, rate.radius),
            pulse: (pulse.dmg, pulse.radius),
            flat: (healing, procs::radius(run, "proc_holy_heal", 12.0)),
            buff: (buff.dmg, buff.radius),
        }
    }
    if run.dead_t.is_none() {
        sources.push(source(&mut run, tf.translation));
    }
    if let Some(players) = players.as_deref_mut() {
        for (tf, owner) in &remotes {
            if let Some(p) = players.0.get_mut(&owner.0) {
                if p.run.dead_t.is_none() {
                    sources.push(source(&mut p.run, tf.translation));
                }
            }
        }
    }
    fn receive(run: &mut Run, pos: Vec3, dt: f32, sources: &[Source]) {
        let mut buff = 0.0_f32;
        for source in sources {
            let distance = units::flat(source.pos - pos).length();
            let in_range = |(amount, radius)| if distance <= radius { amount } else { 0.0 };
            let amount = run.stats().max_hp * (in_range(source.rate) * dt + in_range(source.pulse))
                + in_range(source.flat);
            run.heal(amount);
            buff = buff.max(in_range(source.buff));
        }
        if run.talent_runtime.party_buff == buff {
            return;
        }
        run.talent_runtime.party_buff = buff;
        run.replace_spell_modifiers(
            0x7000_0000,
            if buff > 0.0 {
                vec![
                    Change::IncreasedDamage(buff),
                    Change::IncreasedAttackSpeed(buff),
                ]
                .into_iter()
                .map(|change| Modifier {
                    source: 0,
                    filter: Filter::default(),
                    change,
                })
                .collect::<Vec<_>>()
            } else {
                vec![]
            },
        );
    }
    receive(&mut run, tf.translation, dt, &sources);
    if let Some(players) = players.as_deref_mut() {
        for (tf, owner) in &remotes {
            if let Some(p) = players.0.get_mut(&owner.0) {
                receive(&mut p.run, tf.translation, dt, &sources);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn pet_targets_vary_between_nearby_enemies_without_ignoring_reach() {
        let mut rng = Rng(42);
        let foes = [
            (Entity::from_bits(1), Vec3::X),
            (Entity::from_bits(2), Vec3::X * 2.0),
            (Entity::from_bits(3), Vec3::X * 3.0),
            (Entity::from_bits(4), Vec3::X * 50.0),
        ];
        let chosen: std::collections::HashSet<_> = (0..64)
            .map(|_| pet_target(foes.into_iter(), Vec3::ZERO, Vec3::ZERO, 3.0, &mut rng).unwrap())
            .collect();
        assert_eq!(chosen.len(), 3);
        assert!(!chosen.contains(&foes[3].0));
        let ranged = [
            (Entity::from_bits(1), Vec3::X * 19.0),
            (Entity::from_bits(2), Vec3::X * 21.0),
        ];
        for _ in 0..16 {
            assert_eq!(
                pet_target(ranged.into_iter(), Vec3::ZERO, Vec3::ZERO, 20.0, &mut rng),
                Some(ranged[0].0)
            );
        }
        assert_eq!(
            pet_target(std::iter::empty(), Vec3::ZERO, Vec3::ZERO, 3.0, &mut rng),
            None
        );
        let follow = [
            (Entity::from_bits(1), Vec3::X),
            (Entity::from_bits(2), Vec3::X * 51.0),
        ];
        assert_eq!(
            pet_target(
                follow.into_iter(),
                Vec3::ZERO,
                Vec3::X * 50.0,
                3.0,
                &mut rng
            ),
            Some(follow[1].0)
        );
        let edge = Entity::from_bits(9);
        assert_eq!(
            pet_target(
                [(edge, Vec3::X * 100.0)].into_iter(),
                Vec3::X * 99.0,
                Vec3::ZERO,
                3.0,
                &mut rng
            ),
            Some(edge)
        );
        assert_eq!(
            pet_target(
                [(edge, Vec3::X * 100.1)].into_iter(),
                Vec3::X * 99.0,
                Vec3::ZERO,
                3.0,
                &mut rng
            ),
            None
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
            .filter(|s| tags::for_spell(s).contains(Tags::DAMAGE))
        {
            let mut poison = run(3, 0, &[8]);
            let mut enemy = enemy();
            let mut pending = PendingHits::default();
            let mut h = Hit::direct(skill.key, Entity::from_bits(1), 10.0, false);
            h.triggered = true; // Also covers compatible impacts produced by another spell proc.
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
    fn distinct_procs_can_chain_but_cannot_cycle_back_to_a_previous_milestone() {
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
        let splash = paladin.talent_runtime.bursts.pop().unwrap();
        assert_eq!(splash.chain, chain_bit(M::Seal) | chain_bit(M::Splash));
        let mut child = Hit::direct("proc_splash", root.target, splash.damage, false);
        child.triggered = true;
        child.proc_chain = splash.chain;
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
        for (class, tree, compatible, other, count) in [
            (6, 0, "arcane_missiles", "fireball", 5),
            (8, 0, "starfire", "fireball", 5),
        ] {
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
        let mut h = Hit::direct("proc_bloodburst", Entity::from_bits(1), 10.0, false);
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
    fn poison_refreshes_by_owner_and_cannot_reenter_its_own_proc_chain() {
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
            "Refreshing must not postpone the next damage tick"
        );
        assert_eq!(enemy.dots[0].left, 6.0);
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
                origin: crate::survivors::items::legendary::events::Origin::Talent,
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
