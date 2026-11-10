//! The horde: spawning on a ring outside the view, the chase, contact damage, death, and the
//! experience gems the dead leave.

use std::collections::HashMap;

mod unstuck;
mod motion;

use benilla_world::rig_anim::GlobalSeqDrive;
use crate::survivors::geometry::Surface as WorldPoint;
use bevy::prelude::*;

use super::abilities::Hit;
use super::data::{self, EnemyDef};
use super::hero::{self, Hero};
use super::units::{self, flat, yaw_toward, Fx, Marks, UnitSpec, COLOR_HURT};
use super::{Phase, Rng, Run, Selection};
use crate::creature_anim::{move_flags, MovementState};
use crate::net::{FieldChanged, Guid, ObjectStore};

fn targetable(run: &Run, boss: bool) -> bool {
    run.hp > 0.0
        && run.dead_t.is_none()
        && run.pending_levels == 0
        && (boss || run.mobile.vanish_t <= 0.0)
}

#[derive(Component)]
pub(super) struct Enemy {
    pub def: &'static EnemyDef,
    pub hp: f32,
    pub max_hp: f32,
    pub speed: f32,
    pub damage: f32,
    pub radius: f32,
    pub xp: u32,
    pub boss: bool,
    pub attack_cd: f32,
    pub slow: f32,
    pub slow_t: f32,
    pub root_t: f32,
    pub stun_t: f32,
    pub fear_t: f32,
    pub weaken: f32,
    pub weaken_t: f32,
    pub dots: Vec<Dot>,
    pub dead: bool,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) enum Rank {
    Normal,
    Elite,
    Sprinter,
    Explosive,
    Healer,
    Protector,
    Boss,
}
impl Enemy {
    /// Retain root/freeze timers for spell interactions without immobilizing major enemies.
    pub(super) fn ignores_freeze(&self, rank: Option<&Rank>) -> bool {
        self.boss || matches!(rank, Some(Rank::Elite | Rank::Boss))
    }

    pub(super) fn cap_slow(&mut self, rank: Option<&Rank>) {
        if self.ignores_freeze(rank) {
            self.slow = self.slow.max(0.5);
        }
    }
}
fn roll_rank(roll: f32, config: &super::expedition_config::Config, map: usize) -> Rank {
    let zone = data::CAMPAIGN
        .iter()
        .position(|&m| m == map)
        .map_or(if data::dungeon(map) { data::CAMPAIGN.len() } else { 0 }, |i| i + 1);
    let mut threshold = 0.0;
    for (rank, chance, first_zone) in [
        (Rank::Elite, config.elites.chance, config.elites.first_zone),
        (
            Rank::Sprinter,
            config.sprinters.chance,
            config.sprinters.first_zone,
        ),
        (
            Rank::Explosive,
            config.explosives.chance,
            config.explosives.first_zone,
        ),
        (
            Rank::Healer,
            config.healers.chance,
            config.healers.first_zone,
        ),
        (
            Rank::Protector,
            config.protectors.chance,
            config.protectors.first_zone,
        ),
    ] {
        if zone < first_zone {
            continue;
        }
        // Locked variants leave their probability to normal monsters, not to other variants.
        threshold += chance;
        if roll < threshold {
            return rank;
        }
    }
    Rank::Normal
}

#[derive(Component)]
pub(super) struct ExpeditionScaling {
    damage: f32,
}

pub(super) fn scale_enemies(
    mut run: ResMut<Run>,
    players: Option<Res<super::multiplayer::Companions>>,
    mut enemies: Query<(&mut Enemy, &ExpeditionScaling)>,
) {
    run.expedition_players = players.as_ref().map_or(1, |p| p.0.len().saturating_add(1));
    let factor = run.expeditions.factor(run.elapsed);
    for (mut enemy, base) in &mut enemies {
        if enemy.dead {
            continue;
        }
        enemy.damage = base.damage * factor;
    }
}

#[derive(Clone)]
pub(super) struct Dot {
    pub item_source: Option<u16>,
    pub meta: super::items::legendary::periodic::Meta,
    pub origin: super::items::legendary::events::Origin,
    pub owner: Option<u64>,
    pub skill: &'static str,
    pub meter_source: Option<&'static str>,
    pub tags: Option<super::spell_engine::Tags>,
    pub dps: f32,
    pub left: f32,
    pub tick: f32,
}

/// A corpse playing its death before it fades.
#[derive(Component)]
pub(super) struct Dying(f32);

#[derive(Resource, Default)]
pub(super) struct Spawner {
    timer: f32,
    center: usize,
    // Reserve the entire Endless ring, but spread costly unit/model creation across frames.
    ring: std::collections::VecDeque<(&'static EnemyDef, Vec3, Vec3)>,
}
impl Spawner {
    fn next_center(&mut self, centers: &[Vec3]) -> Vec3 {
        let position = centers[self.center % centers.len()];
        self.center = self.center.wrapping_add(1);
        position
    }
}

/// An enemy died here, worth this much.
#[derive(Message, Clone, Copy)]
pub(super) struct Loot {
    pub item_level: u32,
    pub pos: Vec3,
    pub xp: u32,
    pub boss: bool,
}

#[derive(Clone, Copy)]
pub(super) enum GemKind {
    Xp(u32),
    Heal(f32),
    Vacuum,
    Shield(Option<u64>, u64),
}

#[derive(Component)]
pub(super) struct Gem {
    kind: GemKind,
    pulled: bool,
    speed: f32,
}

#[derive(Component)]
pub(super) struct StillPickup;

/// The `GameObjectDisplayInfo` row and scale a pickup wears: a wisp per experience tier, the
/// bigger tiers rarer, and a turkey leg for health.
fn look_for(kind: GemKind) -> (u32, f32) {
    match kind {
        // `TurkeyLeg.mdx`.
        GemKind::Heal(_) => (114, 2.5),
        // A large purple wisp, distinct from the small experience motes.
        GemKind::Vacuum => (1307, 3.0),
        GemKind::Shield(..) => (1267, 2.0),
        // `BFD_WispSmallPurple.mdx`.
        GemKind::Xp(xp) if xp >= 20 => (1307, 1.0),
        // `BFD_WispSmallGreen.mdx`.
        GemKind::Xp(xp) if xp >= 5 => (1267, 1.0),
        // `BFD_WispSmall.mdx`.
        GemKind::Xp(_) => (426, 1.0),
    }
}

/// Body radius: the big beasts need more room.
fn radius_for(def: &EnemyDef) -> f32 {
    (if def.big { 1.3 } else { 0.6 }) * def.scale
}

pub(super) fn spawn_enemy(
    commands: &mut Commands,
    run: &mut Run,
    def: &'static EnemyDef,
    pos: Vec3,
    hero: Vec3,
    boss: bool,
) -> Entity {
    let e = spawn_rank(
        commands,
        run,
        def,
        pos,
        hero,
        if boss { Rank::Boss } else { Rank::Normal },
    );
    if boss {
        commands
            .entity(e)
            .insert(super::achievements::BossArrival(run.elapsed));
    }
    e
}
pub(super) fn spawn_mob(
    commands: &mut Commands,
    run: &mut Run,
    rng: &mut Rng,
    def: &'static EnemyDef,
    pos: Vec3,
    hero: Vec3,
) -> Entity {
    let rank = if super::training::actor(def) {
        Rank::Normal
    } else {
        roll_rank(rng.f32(), &run.expeditions, run.expedition_map)
    };
    spawn_rank(commands, run, def, pos, hero, rank)
}
fn spawn_rank(
    commands: &mut Commands,
    run: &mut Run,
    def: &'static EnemyDef,
    pos: Vec3,
    hero: Vec3,
    rank: Rank,
) -> Entity {
    let boss = rank == Rank::Boss;
    let elite = rank == Rank::Elite;
    let training = super::training::actor(def);
    let (base_health, base_damage) = if training {
        (def.hp, def.damage)
    } else {
        run.expeditions.stats(run.expedition_map, boss)
    };
    let modifiers = if training { Default::default() } else { run.dungeon_modifiers() };
    let tuning = data::dungeon(run.expedition_map).then(|| super::dungeon_config::get(run.expedition_map));
    let actor = tuning.and_then(|c| c.actor(def.name, boss));
    let base_health = base_health * actor.map_or(1.0, |a| a.health_multiplier);
    let base_damage = base_damage * actor.map_or(1.0, |a| a.damage_multiplier) * modifiers.damage();
    let scale = if training {
        1.0
    } else {
        run.expeditions.factor(run.elapsed)
    };
    let size_multiplier = if boss {
        tuning.map_or(2.0, |c| c.boss_size_multiplier)
    } else if elite {
        run.expeditions.elites.size_multiplier
    } else {
        1.0
    };
    let serial = run.guid();
    let e = units::spawn_unit(
        commands,
        &UnitSpec {
            serial,
            display: def.display,
            scale: def.scale * size_multiplier,
            level: def.level,
            faction: units::FACTION_MONSTER,
            bytes0: 1 << 8,
            weapons: &[],
            sheath: 0,
        },
        pos,
        yaw_toward(hero - pos),
    );
    let party = if training {
        1.0
    } else {
        run.expeditions.party_health(run.expedition_players)
    };
    let hp = base_health
        * scale
        * party
        * if elite {
            run.expeditions.elites.health_multiplier
        } else {
            1.0
        };
    let movement = modifiers.movement() * run.expeditions.movement_multiplier()
        * if rank == Rank::Sprinter { run.expeditions.sprinters.speed_multiplier } else { 1.0 };
    let movement = if run.endless.is_some() { super::endless::config().cap_movement(movement) } else { movement };
    commands.entity(e).insert((
        Enemy {
            def,
            hp,
            max_hp: hp,
            speed: def.speed * movement,
            damage: base_damage * scale,
            radius: radius_for(def) * size_multiplier,
            xp: if elite {
                def.xp.saturating_mul(run.expeditions.elites.xp_multiplier)
            } else {
                def.xp
            },
            boss,
            attack_cd: tuning.map_or(0.5, |c| c.initial_attack_delay),
            slow: 1.0,
            slow_t: 0.0,
            root_t: 0.0,
            stun_t: 0.0,
            fear_t: 0.0,
            weaken: 1.0,
            weaken_t: 0.0,
            dots: Vec::new(),
            dead: false,
        },
        Marks::debuffs(),
        rank,
    ));
    if elite {
        commands
            .entity(e)
            .insert(super::unit_frames::Label(format!("{} · Elite", def.name)));
    }
    let special_label = match rank {
        Rank::Explosive => Some("Explosif"),
        Rank::Healer => Some("Soigneur"),
        Rank::Protector => Some("Protecteur"),
        _ => None,
    };
    if let Some(label) = special_label {
        commands.entity(e).insert((
            super::special_mobs::Special::default(),
            super::unit_frames::Label(format!("{} - {label}", def.name)),
        ));
    }
    if !training {
        commands.entity(e).insert(ExpeditionScaling {
            damage: base_damage,
        });
    }
    e
}

pub(super) fn ground(world: &WorldPoint, mut p: Vec3, fallback: f32) -> Vec3 {
    p.y = world.terrain_height_under(p).unwrap_or(fallback);
    p
}

/// The final clear starts at the configured duration and requires no living enemies.
#[derive(Resource, Default)]
pub(super) struct ExpeditionClear {
    pub done: bool,
}

impl ExpeditionClear {
    /// Fold in this frame's living enemies once the clear has begun.
    fn update(&mut self, bosses_pending: bool, bosses_alive: usize, mobs_alive: usize) {
        self.done = !bosses_pending && bosses_alive == 0 && mobs_alive == 0;
    }
}

const SPAWN_CLEARANCE: f32 = 20.0;

fn safe_spawn_position(mut position: Vec3, players: &[Vec3]) -> Vec3 {
    let clear = |p: Vec3| players.iter().all(|player| flat(p - *player).length_squared() >= SPAWN_CLEARANCE * SPAWN_CLEARANCE);
    if clear(position) { return position; }
    // Push out of overlapping player exclusion circles, then use an enclosing-circle fallback.
    for _ in 0..8 {
        for player in players {
            let delta = flat(position - *player);
            if delta.length_squared() < SPAWN_CLEARANCE * SPAWN_CLEARANCE {
                let direction = if delta.length_squared() > 0.0001 { delta.normalize() } else { Vec3::X };
                let y = position.y;
                position = *player + direction * (SPAWN_CLEARANCE + 0.01);
                position.y = y;
            }
        }
        if clear(position) { return position; }
    }
    let center = players.iter().copied().sum::<Vec3>() / players.len() as f32;
    let radius = players.iter().map(|p| flat(*p - center).length()).fold(0.0_f32, f32::max) + SPAWN_CLEARANCE + 0.01;
    let direction = flat(position - center).normalize_or_zero();
    let direction = if direction == Vec3::ZERO { Vec3::X } else { direction };
    let y = position.y;
    position = center + direction * radius;
    position.y = y;
    position
}

/// Runs after every spawn source and before movement, which seats relocated mobs on terrain.
pub(super) fn protect_spawns(
    players: Query<&Transform, (Or<(With<Hero>, With<super::multiplayer::RemoteOwner>)>, Without<Enemy>)>,
    mut spawned: Query<(&mut Transform, &Enemy), Added<Enemy>>,
) {
    let players: Vec<_> = players.iter().map(|tf| tf.translation).collect();
    for (mut transform, enemy) in &mut spawned {
        if !super::training::actor(enemy.def) {
            transform.translation = safe_spawn_position(transform.translation, &players);
        }
    }
}

/// Keep the horde topped up on a ring outside the view, with the bosses and the encircling waves
/// on the clock.
pub(super) fn spawn_enemies(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut rng: ResMut<Rng>,
    mut spawner: ResMut<Spawner>,
    selection: Res<Selection>,
    hero: Query<&Transform, With<Hero>>,
    enemies: Query<&Enemy>,
    world: WorldPoint,
    players: Option<Res<super::multiplayer::Companions>>,
    clear: Option<ResMut<ExpeditionClear>>,
    remotes: Query<(&Transform, &super::multiplayer::RemoteOwner), Without<Hero>>,
) {
    if super::training::active(&selection) {
        spawner.ring.clear();
        return;
    }
    // Spawner is a persistent resource; an abandoned run must not leak its queued ring.
    if run.rings == 0 {
        spawner.ring.clear();
    }
    let Ok(h) = hero.single() else { return };
    let mut centers = Vec::new();
    if run.hp > 0.0 && run.dead_t.is_none() {
        centers.push(h.translation);
    }
    let mut guests: Vec<_> = remotes
        .iter()
        .filter(|(_, owner)| {
            players.as_ref().is_some_and(|p| {
                p.0.get(&owner.0)
                    .is_some_and(|p| p.run.hp > 0.0 && p.run.dead_t.is_none())
            })
        })
        .map(|(tf, owner)| (owner.0, tf.translation))
        .collect();
    guests.sort_by_key(|(owner, _)| *owner);
    centers.extend(guests.into_iter().map(|(_, position)| position));
    if centers.is_empty() {
        return;
    }
    let elapsed = run.elapsed;
    run.expedition_players = players.as_ref().map_or(1, |p| p.0.len().saturating_add(1));
    run.expedition_map = selection.map;
    let zone = run.expeditions.zone(selection.map);
    let roster = if run.endless.is_some() { super::endless::roster(selection.map) } else { data::roster(selection.map) };
    let pool = data::wave_pool(run.expeditions.progress(elapsed) * data::VICTORY_SECS, roster.enemies.len());

    let dungeon_balance = data::dungeon(selection.map).then(|| super::dungeon_config::get(selection.map));
    let boss_interval = run.expeditions.boss_interval();
    let clearing = run.expeditions.elapsed(elapsed) >= run.expeditions.duration();
    let due_bosses = run.expeditions.boss_waves_due(elapsed);
    let bosses_before = run.bosses;
    while run.bosses < due_bosses {
        let hpos = spawner.next_center(&centers);
        let def = &roster.bosses[(run.bosses - run.expeditions.boss_offset()) as usize % roster.bosses.len()];
        let a = rng.range(0.0, std::f32::consts::TAU);
        let pos = ground(
            &world,
            hpos + Vec3::new(a.cos(), 0.0, a.sin()) * dungeon_balance.map_or(30.0, |c| c.boss_spawn_radius),
            hpos.y,
        );
        for i in 0..zone.boss_quantity + run.dungeon_modifiers().extra_bosses() {
            let offset = Vec3::new((i % 4) as f32 * 3.0, 0.0, (i / 4) as f32 * 3.0);
            let position = ground(&world, pos + offset, hpos.y);
            spawn_enemy(&mut commands, &mut run, def, position, hpos, true);
        }
        run.bosses += 1;
        run.banner = Some((format!("{} approaches!", def.name), 4.0));
    }

    let mut ring_spawned = 0;
    for _ in 0..8 {
        let Some((def, position, center)) = spawner.ring.pop_front() else { break };
        let position = ground(&world, position, center.y);
        super::ranged_enemies::spawn(&mut commands, &mut run, &mut rng, def, position, center);
        ring_spawned += 1;
    }
    if let Some(mut clear) = clear {
        if clearing {
            let alive = |boss: bool| enemies.iter().filter(|e| !e.dead && e.boss == boss).count();
            clear.update(bosses_before < due_bosses, alive(true), alive(false) + ring_spawned + spawner.ring.len());
        } else {
            *clear = ExpeditionClear::default();
        }
    }
    if clearing {
        return;
    }
    // Half way between bosses, a closing ring of the moment's weakest.
    let due_rings = ((run.expeditions.elapsed(elapsed) + boss_interval / 2.0) / boss_interval) as u32;
    if run.rings < due_rings {
        run.rings = due_rings;
        let def = &roster.enemies[pool[0]];
        let alive = enemies.iter().filter(|e| !e.dead && !e.boss).count() + ring_spawned + spawner.ring.len();
        let n = (((dungeon_balance.map_or(28.0, |c| c.ring_start)
            + run.expeditions.progress(elapsed) * dungeon_balance.map_or(60.0, |c| c.ring_growth)) * zone.monster_quantity).ceil() as usize)
            .min(run.expeditions.max_monsters.saturating_sub(alive));
        for i in 0..n {
            let hpos = centers[i % centers.len()];
            let count = (n + centers.len() - 1 - i % centers.len()) / centers.len();
            let a = (i / centers.len()) as f32 / count as f32 * std::f32::consts::TAU;
            let pos = hpos + Vec3::new(a.cos(), 0.0, a.sin()) * dungeon_balance.map_or(26.0, |c| c.ring_radius);
            if run.endless.is_some() {
                spawner.ring.push_back((def, pos, hpos));
            } else {
                let pos = ground(&world, pos, hpos.y);
                super::ranged_enemies::spawn(&mut commands, &mut run, &mut rng, def, pos, hpos);
                ring_spawned += 1;
            }
        }
        run.banner = Some(("They're surrounding you!".into(), 3.0));
    }

    spawner.timer -= time.delta_secs();
    if spawner.timer > 0.0 {
        return;
    }
    spawner.timer = 0.12;
    let alive = enemies.iter().filter(|e| !e.dead && !e.boss).count() + ring_spawned + spawner.ring.len();
    let target = run.expeditions.population(selection.map, elapsed);
    if alive >= target {
        return;
    }
    let n = (((target - alive) as f32) / dungeon_balance.map_or(6.0, |c| c.replenish_divisor)).ceil().min(dungeon_balance.map_or(4, |c| c.replenish_max_per_frame) as f32) as usize;
    for _ in 0..n {
        let hpos = spawner.next_center(&centers);
        let def = &roster.enemies[pool[rng.index(pool.len())]];
        let a = rng.range(0.0, std::f32::consts::TAU);
        let r = rng.range(dungeon_balance.map_or(34.0, |c| c.spawn_radius_min), dungeon_balance.map_or(42.0, |c| c.spawn_radius_max));
        let pos = ground(&world, hpos + Vec3::new(a.cos(), 0.0, a.sin()) * r, hpos.y);
        super::ranged_enemies::spawn(&mut commands, &mut run, &mut rng, def, pos, hpos);
    }
}

/// The chase: straight at the hero, pushed apart from each other, stopped by crowd control, and
/// swinging when in reach.
#[allow(clippy::type_complexity)]
pub(super) fn move_enemies(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut rng: ResMut<Rng>,
    world: WorldPoint,
    hero: Query<(Entity, &Transform), (With<Hero>, Without<Enemy>)>,
    mut enemies: Query<
        (
            Entity,
            &mut Transform,
            &mut Enemy,
            &mut MovementState,
            Option<&super::talent_runtime::Taunted>,
            Option<&mut super::ranged_enemies::Caster>,
            Option<&super::boss_spells::Skills>,
            Option<&Rank>,
        ),
        Without<Hero>,
    >,
    mut fx: Fx,
    mut hits: MessageWriter<Hit>,
    mut companions: Option<ResMut<super::multiplayer::Companions>>,
    remotes: Query<(Entity, &Transform, &super::multiplayer::RemoteOwner), Without<Enemy>>,
    practice: Query<&super::training::Target>,
    mut missiles: Local<super::ranged_enemies::Pending>,
    mut unstuck: Local<unstuck::Tracker>,
    spells: Option<Res<crate::ui_action::Spells>>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    unstuck.retain(|entity| enemies.contains(entity));
    let Ok((hero_e, htf)) = hero.single() else {
        return;
    };
    let hpos = htf.translation;
    let contact_intervals = if data::dungeon(run.expedition_map) {
        let c = super::dungeon_config::get(run.expedition_map);
        [c.monster_attack_interval, c.boss_attack_interval]
    } else { [1.3, 1.0] };
    let attack_rate = run.dungeon_modifiers().attack_rate();
    for missile in missiles.advance(run.elapsed, dt) {
        if let Some(owner) = missile.owner {
            if remotes
                .get(missile.target)
                .is_ok_and(|(_, _, id)| id.0 == owner)
            {
                if let Some(player) = companions.as_deref_mut().and_then(|p| p.0.get_mut(&owner)) {
                    super::ranged_enemies::impact(missile, &mut player.run, &mut fx);
                }
            }
        } else if missile.target == hero_e {
            super::ranged_enemies::impact(missile, &mut run, &mut fx);
        }
    }

    // Separation over a coarse grid: only neighbours in the same or adjacent cells push.
    const CELL: f32 = 3.0;
    let bodies: Vec<(Entity, Vec3, f32)> = enemies
        .iter()
        .filter(|(_, _, e, ..)| !e.dead)
        .map(|(ent, tf, e, ..)| (ent, tf.translation, e.radius))
        .collect();
    let key = |p: Vec3| ((p.x / CELL).floor() as i32, (p.z / CELL).floor() as i32);
    let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    for (i, b) in bodies.iter().enumerate() {
        grid.entry(key(b.1)).or_default().push(i);
    }
    let mut push: HashMap<Entity, Vec3> = HashMap::with_capacity(bodies.len());
    for (i, &(ent, pos, r)) in bodies.iter().enumerate() {
        let (cx, cz) = key(pos);
        let mut acc = Vec3::ZERO;
        for dx in -1..=1 {
            for dz in -1..=1 {
                let Some(cell) = grid.get(&(cx + dx, cz + dz)) else {
                    continue;
                };
                for &j in cell {
                    if j == i {
                        continue;
                    }
                    let (_, other, or) = bodies[j];
                    let d = flat(pos - other);
                    let min = r + or;
                    let distance_squared = d.length_squared();
                    // Most grid neighbours do not touch: avoid a square root for those pairs.
                    if distance_squared >= min * min && distance_squared > 1e-8 {
                        continue;
                    }
                    let len = distance_squared.sqrt();
                    if len < min && len > 1e-4 {
                        acc += d / len * (min - len);
                    } else if len <= 1e-4 {
                        acc += Vec3::new((i as f32).sin(), 0.0, (i as f32).cos()) * 0.1;
                    }
                }
            }
        }
        push.insert(ent, acc);
    }

    for (ent, mut tf, mut e, mut mv, taunt, mut caster, skills, rank) in &mut enemies {
        let e = &mut *e;
        if e.dead {
            unstuck.reset(ent);
            if mv.speed != 0.0 || mv.flags != 0 {
                mv.speed = 0.0;
                mv.flags = 0;
            }
            continue;
        }
        // Debuffs still expire on passive training targets; combat AI never runs for them.
        for t in [
            &mut e.slow_t,
            &mut e.root_t,
            &mut e.stun_t,
            &mut e.fear_t,
            &mut e.weaken_t,
        ] {
            *t = (*t - dt).max(0.0);
        }
        if e.slow_t <= 0.0 {
            e.slow = 1.0;
        }
        if e.weaken_t <= 0.0 {
            e.weaken = 1.0;
        }
        if let Ok(target) = practice.get(ent) {
            unstuck.reset(ent);
            tf.translation = ground(&world, target.home, target.home.y);
            mv.speed = 0.0;
            mv.flags = 0;
            continue;
        }
        if skills.is_some_and(|s| s.busy()) {
            unstuck.reset(ent);
            if let Some(caster) = caster.as_deref_mut() { caster.cancel(); }
            continue;
        }
        let host_visible = targetable(&run, e.boss);
        let mut target = host_visible.then_some((None, hero_e, hpos));
        let mut nearest = if !host_visible {
            f32::INFINITY
        } else {
            flat(hpos - tf.translation).length_squared()
        };
        if let Some(players) = companions.as_deref() {
            for (entity, transform, owner) in &remotes {
                let Some(player) = players.0.get(&owner.0) else {
                    continue;
                };
                if !targetable(&player.run, e.boss) {
                    continue;
                }
                let distance = flat(transform.translation - tf.translation).length_squared();
                if distance < nearest {
                    nearest = distance;
                    target = Some((Some(owner.0), entity, transform.translation));
                }
            }
        }
        if let Some(taunt) = taunt.filter(|t| t.until > run.elapsed) {
            if let Some(id) = taunt.owner {
                if companions
                    .as_deref()
                    .is_some_and(|p| p.0.get(&id).is_some_and(|p| targetable(&p.run, e.boss)))
                {
                    if let Some((entity, tf, _)) = remotes.iter().find(|(_, _, o)| o.0 == id) {
                        target = Some((Some(id), entity, tf.translation));
                    }
                }
            } else if host_visible {
                target = Some((None, hero_e, hpos));
            }
        }
        let Some((owner, hero_e, hpos)) = target else {
            unstuck.reset(ent);
            if let Some(caster) = caster.as_deref_mut() { caster.cancel(); }
            mv.speed = 0.0;
            mv.flags = 0;
            e.attack_cd -= dt * attack_rate;
            continue;
        };
        let target_run = if let Some(owner) = owner {
            &mut companions
                .as_deref_mut()
                .unwrap()
                .0
                .get_mut(&owner)
                .unwrap()
                .run
        } else {
            &mut *run
        };
        let hero_down = target_run.dead_t.is_some();
        e.attack_cd -= dt * attack_rate;
        let pos = tf.translation;
        let to_hero = flat(hpos - pos);
        let dist = to_hero.length();

        // Under Fade the horde has lost the hero: it mills where it stands.
        let lost = target_run.mobile.vanish_t > 0.0 && !e.boss;
        let cast_ready = caster.as_deref_mut().is_some_and(|caster| {
            let eligible = dist <= caster.range && !lost && e.stun_t <= 0.0
                && e.fear_t <= 0.0 && e.attack_cd <= 0.0 && !hero_down;
            caster.ready(eligible.then_some(hero_e), dt)
        });
        e.cap_slow(rank);
        let held = e.stun_t > 0.0 || (e.root_t > 0.0 && !e.ignores_freeze(rank)) || lost;
        let contact = dist <= super::spell_engine::targeting::melee_contact_range(e.radius);
        let mut step = Vec3::ZERO;
        let mut speed = 0.0;
        if !held && !hero_down {
            let dir = if e.fear_t > 0.0 {
                -to_hero.normalize_or_zero()
            } else {
                to_hero.normalize_or_zero()
            };
            if (caster.as_deref().map_or(!contact, |c| dist > c.stand_off)
                && !caster.as_deref().is_some_and(|c| c.casting())) || e.fear_t > 0.0 {
                speed = e.speed * e.slow;
                step = dir * speed;
            }
            if dir != Vec3::ZERO {
                let target = Quat::from_rotation_y(yaw_toward(dir));
                tf.rotation = tf.rotation.slerp(target, (dt * 10.0).min(1.0));
            }
        }
        let shove = push.get(&ent).copied().unwrap_or_default();
        let stand_off = caster.as_deref().map_or(
            super::spell_engine::targeting::melee_contact_range(e.radius), |c| c.stand_off);
        let travel = motion::travel(step, dt, dist, stand_off, e.fear_t > 0.0);
        let mut next = pos + travel + shove * (dt * 8.0).min(0.5);
        next = ground(&world, next, pos.y);
        let walking = step.length_squared() > 1.0e-10
            && !cast_ready && !caster.as_deref().is_some_and(|c| c.casting());
        next = unstuck.resolve(ent, pos, next, walking, dt, |from, to| world.constrain(from, to));
        tf.translation = next;
        if speed > 0.0 {
            mv.speed = speed;
            mv.flags = move_flags::FORWARD;
        } else if mv.speed != 0.0 || mv.flags != 0 {
            mv.speed = 0.0;
            mv.flags = 0;
        }

        if let Some(caster) = caster.as_deref() {
            if cast_ready {
                e.attack_cd = caster.interval;
                let speed = spells
                    .as_deref()
                    .and_then(|s| s.catalog.get(caster.spell))
                    .map_or(24.0, |s| if s.speed > 0.0 { s.speed } else { 24.0 });
                missiles.launch(
                    ent,
                    hero_e,
                    owner,
                    *caster,
                    e.damage * e.weaken,
                    dist,
                    speed,
                );
                fx.cast(ent, caster.spell, &[hero_e], None);
            }
        }
        if caster.is_none()
            && contact
            && !lost
            && e.stun_t <= 0.0
            && e.fear_t <= 0.0
            && e.attack_cd <= 0.0
            && !hero_down
        {
            e.attack_cd = contact_intervals[usize::from(e.boss)];
            let raw = e.damage * e.weaken;
            let roll = if target_run.equipment.block > 0.0
                || super::talent_runtime::block_chance(target_run) > 0.0
                || super::extra_spells::block_bonus(target_run) > 0.0
                || super::extra_spells::dodge_chance(target_run) > 0.0
            {
                rng.f32()
            } else {
                1.0
            };
            target_run.legendary.events.attacker = Some(ent);
            target_run.taken_source = e.def.name;
            let taken = hero::hurt_hero_with_block(target_run, raw, roll);
            if target_run.extra_spells.ice_until > target_run.elapsed {
                e.slow = e.slow.min(0.6);
                e.slow_t = e.slow_t.max(2.0);
            }
            target_run.legendary.events.attacker = None;
            fx.swing(ent, hero_e, taken);
            if taken > 0.0 {
                fx.number(target_run.owner, hero_e, taken, false, COLOR_HURT);
            }
            if target_run.thorns_t > 0.0
                && target_run.legendary.has(220)
                && super::procs::ready(
                    target_run,
                    "thorns",
                    super::spell_engine::Tags::NATURE | super::spell_engine::Tags::SPELL,
                )
            {
                let amount = target_run.thorns * target_run.stats().might;
                if target_run.legendary.effects.allies.len() > 1 {
                    target_run.legendary.effects.heals.push((
                        target_run.legendary.events.position,
                        amount,
                        24.0,
                        false,
                        true,
                    ));
                } else {
                    let serial = target_run.guid();
                    spawn_gem(
                        &mut commands,
                        serial,
                        target_run.legendary.events.position,
                        GemKind::Heal(amount),
                    );
                }
            } else if target_run.thorns_t > 0.0
                && !target_run.legendary.has(220)
                && super::procs::ready(
                    target_run,
                    "thorns",
                    super::spell_engine::Tags::NATURE | super::spell_engine::Tags::SPELL,
                )
            {
                let source = data::ABILITIES
                    .iter()
                    .find(|def| def.spell == target_run.thorns_spell)
                    .map_or("thorns", |def| def.key);
                let mut hit = Hit::direct(
                    source,
                    ent,
                    target_run.thorns * target_run.stats().might,
                    false,
                );
                hit.direct_dot = target_run.thorns_dot;
                hit.owner = target_run.owner;
                hit.triggered = true;
                hit.proc_admitted = true;
                hit.origin = super::items::legendary::events::Origin::Talent;
                hit.cast_id = super::procs::parent(target_run, 0);
                hits.write(hit);
            }
        }
    }
}

/// Mark timers run down; the state kits reap with them.
pub(super) fn tick_marks(
    time: Res<Time>,
    mut units: Query<(Entity, &Guid, &mut ObjectStore, &mut Marks), With<Enemy>>,
    mut fields: MessageWriter<FieldChanged>,
) {
    let dt = time.delta_secs();
    for (entity, guid, mut store, mut marks) in &mut units {
        marks.tick(dt);
        marks.flush(entity, guid.0, &mut store, &mut fields);
    }
}

/// Corpses fade once their death has played.
pub(super) fn reap_dead(
    mut commands: Commands,
    time: Res<Time>,
    mut dying: Query<(Entity, &mut Dying)>,
) {
    for (e, mut d) in &mut dying {
        d.0 += time.delta_secs();
        if d.0 > 2.5 {
            commands
                .entity(e)
                .remove::<Dying>()
                .insert(benilla_world::model_fade::DespawnFade::default());
        }
    }
}

pub(super) fn mark_dying(commands: &mut Commands, e: Entity) {
    commands.entity(e).insert(Dying(0.0));
}

/// Most gems on the ground at once; past it, a kill's worth joins an existing gem.
const MAX_GEMS: usize = 350;
const MEAT_DROP_CHANCE: f32 = 0.0075;
const VACUUM_DROP_CHANCE: f32 = MEAT_DROP_CHANCE / 5.0;

pub(super) fn drop_loot(
    mut commands: Commands,
    mut drops: MessageReader<Loot>,
    mut rng: ResMut<Rng>,
    mut run: ResMut<Run>,
    mut gems: Query<&mut Gem>,
) {
    let mut count = gems.iter().count();
    for d in drops.read() {
        if d.boss || rng.f32() < MEAT_DROP_CHANCE {
            let heal = GemKind::Heal(if d.boss { 1000.0 } else { 30.0 });
            spawn_gem(&mut commands, run.guid(), d.pos + Vec3::X * 0.8, heal);
        }
        if rng.f32() < VACUUM_DROP_CHANCE {
            spawn_gem(
                &mut commands,
                run.guid(),
                d.pos - Vec3::X * 0.8,
                GemKind::Vacuum,
            );
        }
        if count >= MAX_GEMS && !d.boss {
            if let Some(mut g) = gems.iter_mut().find(|g| matches!(g.kind, GemKind::Xp(_))) {
                if let GemKind::Xp(xp) = &mut g.kind {
                    *xp += d.xp;
                }
                continue;
            }
        }
        spawn_gem(&mut commands, run.guid(), d.pos, GemKind::Xp(d.xp));
        count += 1;
    }
}

pub(super) fn spawn_gem(commands: &mut Commands, serial: u64, pos: Vec3, kind: GemKind) {
    let (display, scale) = look_for(kind);
    let e = units::spawn_object(commands, serial, display, scale, pos + Vec3::Y * 0.5);
    commands.entity(e).insert((
        StillPickup,
        Gem {
            kind,
            pulled: false,
            speed: 6.0,
        },
    ));
}

fn bank_levels(run: &mut Run) {
    while run.xp >= data::xp_to_next(run.level) {
        run.xp -= data::xp_to_next(run.level);
        run.gain_expedition_level();
        super::card_draw::bank(run);
        if run.level == super::mobility::LEARN_LEVEL {
            if let Some(m) = super::mobility::for_class(run.class.id) {
                run.banner = Some((
                    format!("You have learned {}! Use your movement skill key.", m.name),
                    4.0,
                ));
            }
        }
    }
}

pub(super) fn bank_endless_xp(
    mut commands: Commands,
    mut run: ResMut<Run>,
    gems: Query<(Entity, &Gem)>,
    mut players: Option<ResMut<super::multiplayer::Companions>>,
) {
    if !run.won || run.endless.is_none() { return; }
    let mut total = 0_u64;
    for (entity, gem) in &gems {
        if let GemKind::Xp(xp) = gem.kind {
            total = total.saturating_add(u64::from(xp));
            commands.entity(entity).despawn();
        }
    }
    grant_pickup_xp(&mut run, total);
    if let Some(players) = players.as_deref_mut() {
        for player in players.0.values_mut() { grant_pickup_xp(&mut player.run, total); }
    }
}

fn grant_pickup_xp(run: &mut Run, xp: u64) {
    run.xp += xp as f32 * data::XP_RATE * run.stats().growth * run.expeditions.reward_multiplier();
    bank_levels(run);
}

/// XP, meat and magnets are collected without travelling; XP is shared with the party.
pub(super) fn collect_gems(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut next: ResMut<NextState<Phase>>,
    hero: Query<&Transform, (With<Hero>, Without<Gem>)>,
    mut gems: Query<(Entity, &mut Transform, &mut Gem)>,
    phase: Option<Res<State<Phase>>>,
    mut players: Option<ResMut<super::multiplayer::Companions>>,
    mut host: Option<ResMut<super::multiplayer::HostPlayer>>,
) {
    let Ok(h) = hero.single() else { return };
    if run.dead_t.is_some() || run.hp <= 0.0 {
        return;
    }
    let dt = time.delta_secs();
    let stats = run.stats();
    let target = h.translation + Vec3::Y * 1.0;
    let mut vacuum = false;
    let mut collected = Vec::new();
    let mut collected_xp = 0u64;
    for (e, mut tf, mut g) in &mut gems {
        if matches!(g.kind,GemKind::Shield(owner,_) if owner!=run.owner) {
            continue;
        }
        let d = target - tf.translation;
        let dist = d.length();
        let pickup = if matches!(g.kind, GemKind::Xp(_)) {
            super::items::legendary::events::pickup(&run)
        } else {
            1.0
        };
        if let GemKind::Xp(xp) = g.kind {
            if flat(d).length() <= stats.magnet * pickup {
                collected_xp += u64::from(xp);
                collected.push(e);
                commands.entity(e).despawn();
            }
            continue;
        }
        let instant = matches!(g.kind, GemKind::Heal(_) | GemKind::Vacuum);
        if instant && flat(d).length() > stats.magnet {
            continue;
        }
        if !g.pulled && flat(d).length() <= stats.magnet * pickup {
            g.pulled = true;
        }
        if !g.pulled {
            continue;
        }
        if instant || dist < 0.9 {
            match g.kind {
                GemKind::Xp(_) => unreachable!("XP is collected without travelling"),
                GemKind::Heal(hp) => {
                    run.heal(hp);
                }
                GemKind::Vacuum => vacuum = true,
                GemKind::Shield(_, generation) => {
                    if generation == run.legendary.effects.shield_generation {
                        run.legendary.effects.shield_lock = 0.0;
                    }
                }
            }
            collected.push(e);
            commands.entity(e).despawn();
            continue;
        }
        g.speed += 30.0 * dt;
        tf.translation += d / dist * (g.speed * dt).min(dist);
    }
    if vacuum {
        // Commands are deferred: XP collected in the first pass still appears in this query.
        for (entity, _, gem) in &mut gems {
            if collected.contains(&entity) {
                continue;
            }
            if let GemKind::Xp(xp) = gem.kind {
                collected_xp += u64::from(xp);
                commands.entity(entity).despawn();
            }
        }
        run.banner = Some(("Experience Magnet: collected all map XP!".to_owned(), 3.0));
    }
    grant_pickup_xp(&mut run, collected_xp);
    if collected_xp > 0 {
        if let Some(host) = host.as_deref_mut() {
            grant_pickup_xp(&mut host.run, collected_xp);
        }
        if let Some(players) = players.as_deref_mut() {
            for player in players.0.values_mut() {
                grant_pickup_xp(&mut player.run, collected_xp);
            }
        }
    }
    if run.pending_levels > 0 && phase.is_none_or(|p| *p.get() == Phase::Playing) {
        next.set(Phase::LevelUp);
    }
}

/// Pickups hang still: a wisp's own loop (its circling motes and their trails) stops on its first
/// frame, clip and global sequences alike, as soon as the model attaches.
pub(super) fn still_gems(
    mut players: Query<
        &mut AnimationPlayer,
        (Or<(With<Gem>, With<StillPickup>)>, Added<AnimationPlayer>),
    >,
    mut drives: Query<
        &mut GlobalSeqDrive,
        (Or<(With<Gem>, With<StillPickup>)>, Added<GlobalSeqDrive>),
    >,
    pickups: Query<Entity, Or<(With<Gem>, With<StillPickup>)>>,
    parents: Query<&ChildOf>,
    mut emitters: Query<
        &mut benilla_world::particles::ParticleEmitter,
        Added<benilla_world::particles::ParticleEmitter>,
    >,
) {
    for mut player in &mut players {
        for (_, active) in player.playing_animations_mut() {
            active.set_speed(0.0);
        }
    }
    for mut drive in &mut drives {
        drive.set_paused(true);
    }
    for mut emitter in &mut emitters {
        let mut anchor = emitter.anchor().or(emitter.emit_host());
        for _ in 0..32 {
            let Some(entity) = anchor else { break };
            if pickups.contains(entity) {
                emitter.set_frozen(true);
                break;
            }
            anchor = parents.get(entity).ok().map(|parent| parent.parent());
        }
    }
}

#[cfg(test)]
mod elite_tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    #[test]
    fn endless_ring_creation_is_bounded_and_pending_monsters_block_completion() {
        let mut world = World::new();
        crate::survivors::geometry::init_test_world(&mut world);
        world.init_resource::<Time>();
        world.init_resource::<Spawner>();
        world.init_resource::<ExpeditionClear>();
        world.insert_resource(Rng(42));
        let map = data::CAMPAIGN[0];
        let mut run = Run { expedition_map: map, ..default() };
        let stage = super::super::endless::Stage::default();
        run.endless = Some(stage);
        run.expeditions.endless = Some(stage);
        run.elapsed = run.expeditions.boss_interval() * 0.5;
        world.insert_resource(run);
        world.insert_resource(Selection { map, ..default() });
        world.spawn((Hero { vy: 0.0, air: 0.0 }, Transform::default()));
        world.run_system_once(spawn_enemies).unwrap();
        let queued = world.resource::<Spawner>().ring.len();
        assert!(queued > 8);
        let before = world.query::<&Enemy>().iter(&world).filter(|e| !e.boss).count();
        // No more replenishment at the deadline; already reserved enemies must still arrive.
        let duration = world.resource::<Run>().expeditions.duration();
        world.resource_mut::<Run>().elapsed = duration;
        world.resource_mut::<Run>().bosses = data::BOSS_WAVES;
        let mut spawned = 0;
        while !world.resource::<Spawner>().ring.is_empty() {
            for mut enemy in world.query::<&mut Enemy>().iter_mut(&mut world) { enemy.dead = true; }
            world.run_system_once(spawn_enemies).unwrap();
            let alive = world.query::<&Enemy>().iter(&world).filter(|e| !e.dead).count();
            assert!(alive > 0 && alive <= 8);
            spawned += alive;
            assert!(!world.resource::<ExpeditionClear>().done);
        }
        assert_eq!(spawned, queued);
        assert_eq!(world.query::<&Enemy>().iter(&world).count(), before + queued);
        for mut enemy in world.query::<&mut Enemy>().iter_mut(&mut world) { enemy.dead = true; }
        world.run_system_once(spawn_enemies).unwrap();
        assert!(world.resource::<ExpeditionClear>().done);
        // Starting a fresh run also discards a ring left behind by abandoning the old one.
        world.resource_mut::<Spawner>().ring.push_back((
            &data::roster(map).enemies[0], Vec3::X * 26.0, Vec3::ZERO,
        ));
        world.resource_mut::<Spawner>().timer = 100.0;
        *world.resource_mut::<Run>() = Run { expedition_map: map, ..default() };
        world.run_system_once(spawn_enemies).unwrap();
        assert!(world.resource::<Spawner>().ring.is_empty());
    }
    #[test]
    fn dungeon_modifiers_apply_to_real_spawns_and_add_a_boss_per_wave() {
        let mut world = World::new();
        crate::survivors::geometry::init_test_world(&mut world);
        world.init_resource::<Time>();
        world.init_resource::<Spawner>();
        world.insert_resource(Rng(42));
        let map = data::DUNGEONS[0];
        let mut run = Run { expedition_map: map, elapsed: data::BOSS_EVERY_SECS, ..default() };
        for index in [0, 1, 3] { run.dungeon_mods.toggle(index); }
        world.insert_resource(run);
        world.insert_resource(Selection { map, ..default() });
        world.spawn((Hero { vy: 0.0, air: 0.0 }, Transform::default()));
        world.run_system_once(spawn_enemies).unwrap();
        let run = world.resource::<Run>();
        let expected_damage = run.expeditions.stats(map, true).1 * 1.25
            * run.expeditions.factor(run.elapsed);
        let bosses: Vec<_> = world.query::<&Enemy>().iter(&world).filter(|e| e.boss)
            .map(|e| (e.speed, e.def.speed, e.damage)).collect();
        assert_eq!(bosses.len(), 2);
        for &(speed, base_speed, damage) in &bosses {
            assert!((speed - base_speed * 1.3).abs() < 0.001);
            assert!((damage - expected_damage).abs() < 0.001);
        }
        world.run_system_once(scale_enemies).unwrap();
        assert!(world.query::<&Enemy>().iter(&world).filter(|e| e.boss)
            .all(|boss| (boss.damage - expected_damage).abs() < 0.001));
        world.resource_mut::<Run>().elapsed = data::VICTORY_SECS;
        world.run_system_once(spawn_enemies).unwrap();
        assert_eq!(world.resource::<Run>().bosses, data::BOSS_WAVES, "every scheduled wave arrives even with living bosses");
        for mut enemy in world.query::<&mut Enemy>().iter_mut(&mut world) {
            if enemy.boss { enemy.dead = true; }
        }
        world.run_system_once(spawn_enemies).unwrap();
        assert_eq!(world.resource::<Run>().bosses, data::BOSS_WAVES);
        assert_eq!(world.query::<&Enemy>().iter(&world).filter(|e| e.boss && !e.dead).count(), 0);
    }
    #[test]
    fn variants_unlock_across_the_campaign_and_accumulate_without_changing_their_chances() {
        let config = super::super::expedition_config::Config::default();
        let variants = [
            (Rank::Elite, 2, 200),
            (Rank::Sprinter, 3, 300),
            (Rank::Explosive, 4, 300),
            (Rank::Healer, 5, 200),
            (Rank::Protector, 6, 200),
        ];
        for (index, &map) in data::CAMPAIGN.iter().enumerate() {
            let mut special_total = 0;
            for &(rank, first_zone, frequency) in &variants {
                let expected = if index + 1 >= first_zone {
                    frequency
                } else {
                    0
                };
                let actual = (0..10_000)
                    .filter(|i| roll_rank((*i as f32 + 0.5) / 10_000.0, &config, map) == rank)
                    .count();
                assert_eq!(actual, expected, "zone {}: {rank:?}", index + 1);
                special_total += actual;
            }
            let normals = (0..10_000)
                .filter(|i| roll_rank((*i as f32 + 0.5) / 10_000.0, &config, map) == Rank::Normal)
                .count();
            assert_eq!(normals + special_total, 10_000);
        }
    }

    #[test]
    fn spawning_uses_the_current_expedition_and_configured_introduction() {
        let mut world = World::new();
        let mut run = Run::default();
        run.expeditions.elites.chance = 1.0;
        run.expeditions.sprinters.chance = 0.0;
        run.expeditions.explosives.chance = 0.0;
        run.expeditions.healers.chance = 0.0;
        run.expeditions.protectors.chance = 0.0;
        run.expeditions.elites.first_zone = 4;
        world.insert_resource(run);
        world.insert_resource(Rng(42));
        for (zone, expected) in [
            (0, Rank::Normal),
            (2, Rank::Normal),
            (3, Rank::Elite),
            (5, Rank::Elite),
        ] {
            world.resource_mut::<Run>().expedition_map = data::CAMPAIGN[zone];
            let entity = world
                .run_system_once(
                    |mut commands: Commands, mut run: ResMut<Run>, mut rng: ResMut<Rng>| {
                        let def = &data::roster(run.expedition_map).enemies[0];
                        spawn_mob(
                            &mut commands,
                            &mut run,
                            &mut rng,
                            def,
                            Vec3::ZERO,
                            Vec3::ZERO,
                        )
                    },
                )
                .unwrap();
            assert_eq!(*world.get::<Rank>(entity).unwrap(), expected);
        }
    }

    #[test]
    fn special_spawn_frequencies_are_disjoint_and_configurable() {
        let mut config = super::super::expedition_config::Config::default();
        for (rank, count) in [
            (Rank::Elite, 200),
            (Rank::Sprinter, 300),
            (Rank::Explosive, 300),
            (Rank::Healer, 200),
            (Rank::Protector, 200),
            (Rank::Normal, 8800),
        ] {
            assert_eq!(
                (0..10_000)
                    .filter(
                        |i| roll_rank((*i as f32 + 0.5) / 10_000.0, &config, data::CAMPAIGN[5])
                            == rank
                    )
                    .count(),
                count
            );
        }
        config.elites.chance = 0.0;
        config.sprinters.chance = 0.0;
        config.explosives.chance = 0.0;
        config.healers.chance = 0.0;
        config.protectors.chance = 1.0;
        assert_eq!(roll_rank(0.99, &config, data::CAMPAIGN[5]), Rank::Protector);
    }
    #[test]
    fn two_percent_of_regular_spawn_rolls_are_elite() {
        let config = super::super::expedition_config::Config::default();
        assert_eq!(roll_rank(0.0, &config, data::CAMPAIGN[5]), Rank::Elite);
        assert_eq!(roll_rank(0.02, &config, data::CAMPAIGN[5]), Rank::Sprinter);
        assert_eq!(
            (0..10_000)
                .filter(
                    |i| roll_rank(*i as f32 / 10_000.0, &config, data::CAMPAIGN[5]) == Rank::Elite
                )
                .count(),
            200
        );
    }
    #[test]
    fn three_percent_of_spawns_are_sprinters_and_variants_never_overlap() {
        let config = super::super::expedition_config::Config::default();
        assert_eq!(
            (0..10_000)
                .filter(
                    |i| roll_rank(*i as f32 / 10_000.0, &config, data::CAMPAIGN[5])
                        == Rank::Sprinter
                )
                .count(),
            300
        );
        assert_eq!(roll_rank(0.05, &config, data::CAMPAIGN[5]), Rank::Explosive);
        assert_eq!(roll_rank(0.15, &config, data::CAMPAIGN[5]), Rank::Normal);
    }
    #[test]
    fn final_wave_arrives_before_the_clear_can_finish_for_campaign_and_dungeons() {
        for map in [data::CAMPAIGN[0], data::DUNGEONS[0]] {
            let mut world = World::new();
            crate::survivors::geometry::init_test_world(&mut world);
            world.init_resource::<Time>();
            world.init_resource::<Spawner>();
            world.init_resource::<ExpeditionClear>();
            world.insert_resource(Rng(42));
            let expeditions = super::super::expedition_config::Config::default().for_map(map);
            let duration = expeditions.duration();
            world.insert_resource(Run {
                expedition_map: map, expeditions, elapsed: duration,
                bosses: data::BOSS_WAVES - 1, ..default()
            });
            world.insert_resource(Selection { map, ..default() });
            world.spawn((Hero { vy: 0.0, air: 0.0 }, Transform::default()));
            world.run_system_once(spawn_enemies).unwrap();
            assert_eq!(world.resource::<Run>().bosses, data::BOSS_WAVES);
            assert!(!world.resource::<ExpeditionClear>().done);
            assert_eq!(world.query::<&Enemy>().iter(&world).filter(|e| !e.boss).count(), 0);
            world.run_system_once(spawn_enemies).unwrap();
            assert!(!world.resource::<ExpeditionClear>().done);
            for mut enemy in world.query::<&mut Enemy>().iter_mut(&mut world) {
                enemy.dead = true;
            }
            world.run_system_once(spawn_enemies).unwrap();
            assert!(world.resource::<ExpeditionClear>().done);
        }
    }

    #[test]
    fn the_clear_needs_every_boss_and_every_monster() {
        let mut clear = ExpeditionClear::default();
        clear.update(false, 1, 100);
        assert!(!clear.done);
        clear.update(false, 1, 5);
        assert!(!clear.done, "a boss is alive");
        clear.update(false, 0, 1);
        assert!(!clear.done, "too many mobs are left");
        clear.update(true, 0, 0);
        assert!(!clear.done, "a boss has yet to arrive");
        clear.update(false, 0, 0);
        assert!(clear.done);
    }

    #[test]
    fn elites_have_ten_times_health_and_triple_xp_without_boss_damage_or_rewards() {
        let mut world = World::new();
        world.insert_resource(Run {
            elapsed: 120.0,
            expedition_players: 3,
            ..default()
        });
        world
            .run_system_once(|mut commands: Commands, mut run: ResMut<Run>| {
                let def = &data::roster(run.expedition_map).enemies[0];
                for rank in [Rank::Normal, Rank::Elite, Rank::Sprinter, Rank::Boss] {
                    spawn_rank(&mut commands, &mut run, def, Vec3::ZERO, Vec3::ZERO, rank);
                }
            })
            .unwrap();
        let mut query = world.query::<(
            &Rank,
            &Enemy,
            &crate::net::NetEntity,
            Option<&super::super::unit_frames::Label>,
        )>();
        let (_, normal, normal_model, _) = query
            .iter(&world)
            .find(|(r, ..)| **r == Rank::Normal)
            .unwrap();
        let (_, elite, elite_model, label) = query
            .iter(&world)
            .find(|(r, ..)| **r == Rank::Elite)
            .unwrap();
        assert_eq!(elite.max_hp, normal.max_hp * 10.0);
        assert_eq!(elite.hp, elite.max_hp);
        assert_eq!(elite.damage, normal.damage);
        assert_eq!(elite.speed, normal.speed);
        assert_eq!(elite.xp, normal.xp * 3);
        assert!(!elite.boss);
        assert!((elite_model.scale - normal_model.scale * 1.5).abs() < 0.001);
        let (_, sprinter, sprinter_model, _) = query
            .iter(&world)
            .find(|(r, ..)| **r == Rank::Sprinter)
            .unwrap();
        assert_eq!(sprinter.max_hp, normal.max_hp);
        assert_eq!(sprinter.speed, normal.speed * 3.0);
        assert_eq!(sprinter.damage, normal.damage);
        assert_eq!(sprinter.xp, normal.xp);
        assert_eq!(sprinter_model.scale, normal_model.scale);
        assert!(!sprinter.boss);
        assert!(label.unwrap().0.ends_with("Elite"));
        assert!(
            query
                .iter(&world)
                .find(|(r, ..)| **r == Rank::Boss)
                .unwrap()
                .1
                .boss
        );
    }
}

#[cfg(test)]
mod pickup_tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    fn world() -> World {
        let mut world = World::new();
        world.init_resource::<Time>();
        world.init_resource::<Run>();
        world.init_resource::<NextState<Phase>>();
        world.spawn((Transform::default(), Hero { vy: 0.0, air: 0.0 }));
        world
    }
    fn gem(world: &mut World, kind: GemKind, pos: Vec3) -> Entity {
        world
            .spawn((
                Transform::from_translation(pos),
                Gem {
                    kind,
                    pulled: false,
                    speed: 6.0,
                },
            ))
            .id()
    }
    #[test]
    fn endless_gems_receive_the_stage_bonus_once_including_the_last_kill() {
        let mut world = world();
        {
            let mut run = world.resource_mut::<Run>();
            super::super::endless::begin(&mut run);
            let stage = super::super::endless::Stage { index: 2, ..default() };
            run.endless = Some(stage);
            run.expeditions = super::super::endless::config().expedition(stage);
            run.level = 50;
        }
        gem(&mut world, GemKind::Xp(10), Vec3::Y);
        world.run_system_once(collect_gems).unwrap();
        let expected = 10.0 * data::XP_RATE * 1.6;
        assert!((world.resource::<Run>().xp - expected).abs() < 0.001);
        world.resource_mut::<Run>().won = true;
        gem(&mut world, GemKind::Xp(5), Vec3::X * 10000.0);
        world.run_system_once(bank_endless_xp).unwrap();
        assert!((world.resource::<Run>().xp - expected * 1.5).abs() < 0.001);
        world.run_system_once(bank_endless_xp).unwrap();
        assert!((world.resource::<Run>().xp - expected * 1.5).abs() < 0.001);
    }

    #[test]
    fn endless_victory_sweeps_all_xp_including_late_drops_without_moving_or_double_credit() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<Phase>();
        let mut run = Run { won: true, level: 50, ..default() };
        super::super::endless::begin(&mut run);
        app.insert_resource(run)
            .add_systems(OnEnter(Phase::GameOver), bank_endless_xp)
            .add_systems(Update, bank_endless_xp.run_if(in_state(Phase::GameOver)));
        let first = gem(app.world_mut(), GemKind::Xp(10), Vec3::X * 10000.0);
        let second = gem(app.world_mut(), GemKind::Xp(20), -Vec3::X * 10000.0);
        let meat = gem(app.world_mut(), GemKind::Heal(10.0), Vec3::Z * 10000.0);
        app.world_mut().resource_mut::<NextState<Phase>>().set(Phase::GameOver);
        app.update();
        let expected = 30.0 * data::XP_RATE;
        assert!((app.world().resource::<Run>().xp - expected).abs() < 0.001);
        assert!(app.world().get_entity(first).is_err());
        assert!(app.world().get_entity(second).is_err());
        assert!(app.world().get_entity(meat).is_ok());
        let late = gem(app.world_mut(), GemKind::Xp(5), Vec3::Z * 10000.0);
        app.update();
        assert!(app.world().get_entity(late).is_err());
        assert!((app.world().resource::<Run>().xp - expected - 5.0 * data::XP_RATE).abs() < 0.001);
        app.update();
        assert!((app.world().resource::<Run>().xp - expected - 5.0 * data::XP_RATE).abs() < 0.001);
    }

    #[test]
    fn meat_and_magnets_are_collected_without_travel_only_inside_pickup_range() {
        let mut world = world();
        world.resource_mut::<Run>().hp = 1.0;
        let radius = world.resource::<Run>().stats().magnet;
        let near = Vec3::X * (radius * 0.9) + Vec3::Y;
        let far = Vec3::Z * (radius + 10.0) + Vec3::Y;
        let meat = gem(&mut world, GemKind::Heal(10.0), near);
        let magnet = gem(&mut world, GemKind::Vacuum, -near);
        let distant_meat = gem(&mut world, GemKind::Heal(10.0), far);
        let distant_magnet = gem(&mut world, GemKind::Vacuum, -far);
        let xp = gem(&mut world, GemKind::Xp(1), Vec3::X * 10_000.0);
        world.run_system_once(collect_gems).unwrap();
        assert!(world.get_entity(meat).is_err());
        assert!(world.get_entity(magnet).is_err());
        assert!(world.get_entity(xp).is_err());
        assert!(world.resource::<Run>().hp > 1.0);
        assert_eq!(world.resource::<Run>().xp, data::XP_RATE);
        for (entity, position) in [(distant_meat, far), (distant_magnet, -far)] {
            assert_eq!(
                world.get::<Transform>(entity).unwrap().translation,
                position
            );
            assert!(!world.get::<Gem>(entity).unwrap().pulled);
        }
        world.run_system_once(collect_gems).unwrap();
        assert_eq!(world.resource::<Run>().xp, data::XP_RATE);
    }

    #[test]
    fn an_old_shield_fragment_cannot_unlock_a_later_broken_shield() {
        let mut world = world();
        {
            let mut run = world.resource_mut::<Run>();
            run.legendary.effects.shield_generation = 2;
            run.legendary.effects.shield_lock = 8.0;
        }
        gem(&mut world, GemKind::Shield(None, 1), Vec3::Y);
        world.run_system_once(collect_gems).unwrap();
        assert_eq!(world.resource::<Run>().legendary.effects.shield_lock, 8.0);
        gem(&mut world, GemKind::Shield(None, 2), Vec3::Y);
        world.run_system_once(collect_gems).unwrap();
        assert_eq!(world.resource::<Run>().legendary.effects.shield_lock, 0.0);
    }
    #[test]
    fn pickups_share_xp_once_from_host_or_guest_without_travel_or_reviving_fallen_players() {
        use super::super::multiplayer::{Companion, Companions, HostPlayer};
        for guest_collects in [false, true] {
            let mut world = world();
            world.resource_mut::<Run>().level = 50;
            if guest_collects {
                world.resource_mut::<Run>().owner = Some(42);
                let host = world.spawn(Transform::from_xyz(100.0, 0.0, 0.0)).id();
                world.insert_resource(HostPlayer {
                    entity: host,
                    run: Run {
                        level: 50,
                        ..default()
                    },
                });
            }
            let entity = world.spawn(Transform::from_xyz(100.0, 0.0, 0.0)).id();
            let mut fallen = Run {
                owner: Some(77),
                level: 50,
                hp: 0.0,
                dead_t: Some(1.0),
                ..default()
            };
            fallen.passives.push((
                data::PASSIVES.iter().find(|p| p.key == "wisdom").unwrap(),
                1,
            ));
            let growth = fallen.stats().growth;
            let mut players = Companions::default();
            players.0.insert(
                77,
                Companion {
                    entity,
                    run: fallen,
                    physics: Hero { vy: 0.0, air: 0.0 },
                    offer: default(),
                    offer_id: 0,
                },
            );
            world.insert_resource(players);
            let xp = gem(&mut world, GemKind::Xp(3), Vec3::X * 2.0 + Vec3::Y);
            world.run_system_once(collect_gems).unwrap();
            assert!(
                world.get_entity(xp).is_err(),
                "XP is collected immediately in magnet range"
            );
            world.run_system_once(collect_gems).unwrap();
            assert_eq!(world.resource::<Run>().xp, 3.0 * data::XP_RATE);
            let players = world.resource::<Companions>();
            assert_eq!(players.0[&77].run.xp, 3.0 * data::XP_RATE * growth);
            assert_eq!(players.0[&77].run.hp, 0.0);
            if guest_collects {
                assert_eq!(world.resource::<HostPlayer>().run.xp, 3.0 * data::XP_RATE);
            }
            gem(&mut world, GemKind::Vacuum, Vec3::Y);
            gem(&mut world, GemKind::Xp(2), Vec3::X * 10_000.0);
            world.run_system_once(collect_gems).unwrap();
            assert_eq!(world.resource::<Run>().xp, 5.0 * data::XP_RATE);
            assert!(
                (world.resource::<Companions>().0[&77].run.xp - 5.0 * data::XP_RATE * growth).abs()
                    < 0.001
            );
            if guest_collects {
                assert_eq!(world.resource::<HostPlayer>().run.xp, 5.0 * data::XP_RATE);
            }
        }
    }
    #[test]
    fn vacuum_collects_near_and_distant_xp_once_with_growth_but_leaves_other_pickups() {
        let mut world = world();
        {
            let mut run = world.resource_mut::<Run>();
            run.level = 50;
            run.hp = 10.0;
            run.passives.push((
                data::PASSIVES.iter().find(|p| p.key == "wisdom").unwrap(),
                1,
            ));
        }
        gem(&mut world, GemKind::Xp(1), Vec3::Y);
        gem(&mut world, GemKind::Vacuum, Vec3::Y);
        gem(&mut world, GemKind::Vacuum, Vec3::Y);
        gem(&mut world, GemKind::Xp(2), Vec3::X * 10_000.0);
        gem(&mut world, GemKind::Xp(3), -Vec3::X * 10_000.0);
        let meat = gem(&mut world, GemKind::Heal(30.0), Vec3::Z * 10_000.0);
        let distant_vacuum = gem(&mut world, GemKind::Vacuum, -Vec3::Z * 10_000.0);
        world.run_system_once(collect_gems).unwrap();
        assert!((world.resource::<Run>().xp - 6.0 * data::XP_RATE * 1.1).abs() < 0.001);
        assert_eq!(world.resource::<Run>().hp, 10.0);
        assert!(world.get_entity(meat).is_ok());
        assert!(world.get_entity(distant_vacuum).is_ok());
        assert_eq!(world.query::<&Gem>().iter(&world).count(), 2);
    }
    #[test]
    fn vacuum_banks_all_level_ups_and_is_consumed_when_no_xp_is_present() {
        let mut world = world();
        gem(&mut world, GemKind::Vacuum, Vec3::Y);
        gem(&mut world, GemKind::Xp(100), Vec3::X * 10_000.0);
        world.run_system_once(collect_gems).unwrap();
        assert_eq!(world.resource::<Run>().level, 8);
        assert_eq!(world.resource::<Run>().pending_levels, 7);
        let remainder = 100.0 * data::XP_RATE - (1..8).map(data::xp_to_next).sum::<f32>();
        assert_eq!(world.resource::<Run>().xp, remainder);
        assert!(matches!(
            world.resource::<NextState<Phase>>(),
            NextState::Pending(Phase::LevelUp)
        ));
        let empty = gem(&mut world, GemKind::Vacuum, Vec3::Y);
        world.run_system_once(collect_gems).unwrap();
        assert!(world.get_entity(empty).is_err());
        assert_eq!(world.resource::<Run>().xp, remainder);
    }
    #[test]
    fn manual_rewards_bank_xp_levels_without_opening_the_draft_or_hiding_the_player() {
        let mut world = world();
        world.resource_mut::<Run>().auto_draw_cards = false;
        world.resource_mut::<Run>().hp = 100.0;
        gem(&mut world, GemKind::Vacuum, Vec3::Y);
        gem(&mut world, GemKind::Xp(100), Vec3::X * 10_000.0);
        world.run_system_once(collect_gems).unwrap();
        let run = world.resource::<Run>();
        assert_eq!((run.level,run.pending_levels,run.stored_cards),(8,0,7));
        assert!(targetable(run,false));
        assert!(!matches!(world.resource::<NextState<Phase>>(),NextState::Pending(Phase::LevelUp)));
    }
    #[test]
    fn actual_drop_table_makes_vacuums_five_times_rarer_than_meat() {
        let mut world = World::new();
        world.init_resource::<Run>();
        world.init_resource::<Rng>();
        world.init_resource::<Messages<Loot>>();
        world.spawn(Gem {
            kind: GemKind::Xp(0),
            pulled: false,
            speed: 6.0,
        });
        for _ in 0..100_000 {
            world.resource_mut::<Messages<Loot>>().write(Loot {
                item_level: 1,
                pos: Vec3::ZERO,
                xp: 1,
                boss: false,
            });
        }
        world.run_system_once(drop_loot).unwrap();
        let mut meats = 0;
        let mut vacuums = 0;
        for gem in world.query::<&Gem>().iter(&world) {
            match gem.kind {
                GemKind::Heal(_) => meats += 1,
                GemKind::Vacuum => vacuums += 1,
                _ => {}
            }
        }
        assert!((650..850).contains(&meats), "{meats}");
        assert!((110..190).contains(&vacuums), "{vacuums}");
        assert!((4.0..6.0).contains(&(meats as f32 / vacuums as f32)));
    }
}

#[cfg(test)]
mod spawn_clearance_tests {
    use super::*;
    #[test]
    fn new_mobs_avoid_host_and_guest_but_existing_mobs_can_approach() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.init_resource::<Run>().add_systems(Update, protect_spawns);
        app.world_mut().spawn((Hero { vy: 0.0, air: 0.0 }, Transform::default()));
        app.world_mut().spawn((super::super::multiplayer::RemoteOwner(42), Transform::from_xyz(35.0, 0.0, 0.0)));
        let entity = app.world_mut().run_system_once(|mut commands: Commands, mut run: ResMut<Run>| {
            spawn_enemy(&mut commands, &mut run, &data::roster(0).enemies[0], Vec3::X * 35.0, Vec3::ZERO, false)
        }).unwrap();
        app.update();
        let pos = app.world().get::<Transform>(entity).unwrap().translation;
        assert!(flat(pos).length() >= 20.0);
        assert!(flat(pos - Vec3::X * 35.0).length() >= 20.0);
        app.world_mut().get_mut::<Transform>(entity).unwrap().translation = Vec3::ZERO;
        app.update();
        assert_eq!(app.world().get::<Transform>(entity).unwrap().translation, Vec3::ZERO);
    }

    #[test]
    fn spawn_positions_clear_every_player_including_overlapping_exclusion_zones() {
        for players in [vec![Vec3::ZERO], vec![Vec3::ZERO, Vec3::X * 35.0], vec![Vec3::ZERO, Vec3::X * 10.0, Vec3::Z * 10.0, -Vec3::X * 10.0]] {
            for x in -40..80 {
                for z in -30..30 {
                    let original = Vec3::new(x as f32, 7.0, z as f32);
                    let position = safe_spawn_position(original, &players);
                    assert_eq!(position.y, original.y);
                    assert!(players.iter().all(|p| flat(position - *p).length() >= 20.0));
                    if players.iter().all(|p| flat(original - *p).length() >= 20.0) { assert_eq!(position, original); }
                }
            }
        }
        assert_eq!(safe_spawn_position(Vec3::ONE, &[]), Vec3::ONE);
    }
}
