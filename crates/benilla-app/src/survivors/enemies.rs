//! The horde: spawning on a ring outside the view, the chase, contact damage, death, and the
//! experience gems the dead leave.

use std::collections::HashMap;

use benilla_world::rig_anim::GlobalSeqDrive;
use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;

use super::abilities::Hit;
use super::data::{self, EnemyDef};
use super::hero::{self, Hero, HERO_RADIUS};
use super::units::{self, flat, yaw_toward, Fx, Marks, UnitSpec, COLOR_HURT};
use super::{Phase, Rng, Run, Selection};
use crate::creature_anim::{move_flags, MovementState};
use crate::net::{FieldChanged, Guid, ObjectStore};

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

#[derive(Component)]
pub(super) struct ExpeditionScaling {
    health: f32,
    damage: f32,
}

pub(super) fn scale_enemies(run: Res<Run>, mut enemies: Query<(&mut Enemy, &ExpeditionScaling)>) {
    let factor = run.expeditions.factor(run.elapsed);
    for (mut enemy, base) in &mut enemies {
        if enemy.dead {
            continue;
        }
        let max_hp = base.health * factor;
        enemy.hp *= max_hp / enemy.max_hp;
        enemy.max_hp = max_hp;
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
    let training = super::training::actor(def);
    let (base_health, base_damage) = if training {
        (def.hp, def.damage)
    } else {
        run.expeditions.stats(run.expedition_map, boss)
    };
    let scale = if training {
        1.0
    } else {
        run.expeditions.factor(run.elapsed)
    };
    let size_multiplier = if boss { 2.0 } else { 1.0 };
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
    let hp = base_health * scale;
    commands.entity(e).insert((
        Enemy {
            def,
            hp,
            max_hp: hp,
            speed: def.speed,
            damage: base_damage * scale,
            radius: radius_for(def) * size_multiplier,
            xp: def.xp,
            boss,
            attack_cd: 0.5,
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
    ));
    if !training {
        commands.entity(e).insert(ExpeditionScaling {
            health: base_health,
            damage: base_damage,
        });
    }
    e
}

pub(super) fn ground(world: &WorldPoint, mut p: Vec3, fallback: f32) -> Vec3 {
    p.y = world.terrain_height_under(p).unwrap_or(fallback);
    p
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
    remotes: Query<(&Transform, &super::multiplayer::RemoteOwner), Without<Hero>>,
) {
    if super::training::active(&selection) {
        return;
    }
    let Ok(h) = hero.single() else { return };
    let hpos = if run.dead_t.is_some() {
        let Some((position, _)) = remotes.iter().find(|(_, owner)| {
            players
                .as_ref()
                .is_some_and(|p| p.0.get(&owner.0).is_some_and(|p| p.run.dead_t.is_none()))
        }) else {
            return;
        };
        position.translation
    } else {
        h.translation
    };
    let elapsed = run.elapsed;
    run.expedition_map = selection.map;
    let zone = run.expeditions.zone(selection.map);
    let roster = data::roster(selection.map);
    let pool = data::wave_pool(elapsed, roster.enemies.len());

    let due_bosses = ((elapsed / data::BOSS_EVERY_SECS) as u32).min(roster.bosses.len() as u32);
    while run.bosses < due_bosses {
        let def = &roster.bosses[run.bosses as usize];
        let a = rng.range(0.0, std::f32::consts::TAU);
        let pos = ground(
            &world,
            hpos + Vec3::new(a.cos(), 0.0, a.sin()) * 30.0,
            hpos.y,
        );
        for i in 0..zone.boss_quantity {
            let offset = Vec3::new((i % 4) as f32 * 3.0, 0.0, (i / 4) as f32 * 3.0);
            let position = ground(&world, pos + offset, hpos.y);
            spawn_enemy(&mut commands, &mut run, def, position, hpos, true);
        }
        run.bosses += 1;
        run.banner = Some((format!("{} approaches!", def.name), 4.0));
    }

    // Half way between bosses, a closing ring of the moment's weakest.
    let mut ring_spawned = 0;
    let due_rings = ((elapsed + data::BOSS_EVERY_SECS / 2.0) / data::BOSS_EVERY_SECS) as u32;
    if run.rings < due_rings {
        run.rings = due_rings;
        let def = &roster.enemies[pool[0]];
        let alive = enemies.iter().filter(|e| !e.dead && !e.boss).count();
        let n = (((28.0 + data::progress(elapsed) * 60.0) * zone.monster_quantity).ceil() as usize)
            .min(run.expeditions.max_monsters.saturating_sub(alive));
        ring_spawned = n;
        for i in 0..n {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            let pos = ground(
                &world,
                hpos + Vec3::new(a.cos(), 0.0, a.sin()) * 26.0,
                hpos.y,
            );
            spawn_enemy(&mut commands, &mut run, def, pos, hpos, false);
        }
        run.banner = Some(("They're surrounding you!".into(), 3.0));
    }

    spawner.timer -= time.delta_secs();
    if spawner.timer > 0.0 {
        return;
    }
    spawner.timer = 0.12;
    let alive = enemies.iter().filter(|e| !e.dead && !e.boss).count() + ring_spawned;
    let target = run.expeditions.population(selection.map, elapsed);
    if alive >= target {
        return;
    }
    let n = (((target - alive) as f32) / 6.0).ceil().min(4.0) as usize;
    for _ in 0..n {
        let def = &roster.enemies[pool[rng.index(pool.len())]];
        let a = rng.range(0.0, std::f32::consts::TAU);
        let r = rng.range(34.0, 42.0);
        let pos = ground(&world, hpos + Vec3::new(a.cos(), 0.0, a.sin()) * r, hpos.y);
        spawn_enemy(&mut commands, &mut run, def, pos, hpos, false);
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
        ),
        Without<Hero>,
    >,
    mut fx: Fx,
    mut hits: MessageWriter<Hit>,
    mut companions: Option<ResMut<super::multiplayer::Companions>>,
    remotes: Query<(Entity, &Transform, &super::multiplayer::RemoteOwner), Without<Enemy>>,
    practice: Query<&super::training::Target>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let Ok((hero_e, htf)) = hero.single() else {
        return;
    };
    let hpos = htf.translation;

    // Separation over a coarse grid: only neighbours in the same or adjacent cells push.
    const CELL: f32 = 3.0;
    let bodies: Vec<(Entity, Vec3, f32)> = enemies
        .iter()
        .filter(|(_, _, e, _, _)| !e.dead)
        .map(|(ent, tf, e, _, _)| (ent, tf.translation, e.radius))
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
                    let len = d.length();
                    let min = r + or;
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

    for (ent, mut tf, mut e, mut mv, taunt) in &mut enemies {
        let e = &mut *e;
        if e.dead {
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
            tf.translation = ground(&world, target.home, target.home.y);
            mv.speed = 0.0;
            mv.flags = 0;
            continue;
        }
        let mut target = (None, hero_e, hpos);
        let host_down = run.dead_t.is_some();
        let mut nearest = if host_down {
            f32::INFINITY
        } else {
            flat(hpos - tf.translation).length_squared()
        };
        if let Some(players) = companions.as_deref() {
            for (entity, transform, owner) in &remotes {
                let Some(player) = players.0.get(&owner.0) else {
                    continue;
                };
                if player.run.dead_t.is_some() || (!e.boss && player.run.mobile.vanish_t > 0.0) {
                    continue;
                }
                let distance = flat(transform.translation - tf.translation).length_squared();
                if distance < nearest {
                    nearest = distance;
                    target = (Some(owner.0), entity, transform.translation);
                }
            }
        }
        if let Some(taunt) = taunt.filter(|t| t.until > run.elapsed) {
            if let Some(id) = taunt.owner {
                if companions
                    .as_deref()
                    .is_some_and(|p| p.0.get(&id).is_some_and(|p| p.run.dead_t.is_none()))
                {
                    if let Some((entity, tf, _)) = remotes.iter().find(|(_, _, o)| o.0 == id) {
                        target = (Some(id), entity, tf.translation);
                    }
                }
            } else if !host_down {
                target = (None, hero_e, hpos);
            }
        }
        let (owner, hero_e, hpos) = target;
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
        e.attack_cd -= dt;
        let pos = tf.translation;
        let to_hero = flat(hpos - pos);
        let dist = to_hero.length();

        // Strays far behind are brought back in front, as the reference genre does.
        if dist > 75.0 {
            let back = hpos + to_hero.normalize_or_zero() * 38.0;
            tf.translation = ground(&world, back, hpos.y);
            continue;
        }

        // Under Fade the horde has lost the hero: it mills where it stands.
        let lost = target_run.mobile.vanish_t > 0.0 && !e.boss;
        let held = e.stun_t > 0.0 || e.root_t > 0.0 || lost;
        let contact = dist <= HERO_RADIUS + e.radius + 0.35;
        let mut step = Vec3::ZERO;
        let mut speed = 0.0;
        if !held && !hero_down {
            let dir = if e.fear_t > 0.0 {
                -to_hero.normalize_or_zero()
            } else {
                to_hero.normalize_or_zero()
            };
            if !contact || e.fear_t > 0.0 {
                speed = e.speed * e.slow;
                step = dir * speed;
            }
            if dir != Vec3::ZERO {
                let target = Quat::from_rotation_y(yaw_toward(dir));
                tf.rotation = tf.rotation.slerp(target, (dt * 10.0).min(1.0));
            }
        }
        let shove = push.get(&ent).copied().unwrap_or_default();
        let mut next = pos + step * dt + shove * (dt * 8.0).min(0.5);
        next = ground(&world, next, pos.y);
        tf.translation = next;
        if speed > 0.0 {
            mv.speed = speed;
            mv.flags = move_flags::FORWARD;
        } else if mv.speed != 0.0 || mv.flags != 0 {
            mv.speed = 0.0;
            mv.flags = 0;
        }

        if contact
            && !lost
            && e.stun_t <= 0.0
            && e.fear_t <= 0.0
            && e.attack_cd <= 0.0
            && !hero_down
        {
            e.attack_cd = if e.boss { 1.0 } else { 1.3 };
            let raw = e.damage * e.weaken;
            let roll = if target_run.equipment.block > 0.0 {
                rng.f32()
            } else {
                1.0
            };
            target_run.legendary.events.attacker = Some(ent);
            let taken = hero::hurt_hero_with_block(target_run, raw, roll);
            target_run.legendary.events.attacker = None;
            fx.swing(ent, hero_e, taken);
            if taken > 0.0 {
                fx.number(hero_e, taken, false, COLOR_HURT);
            }
            if target_run.thorns_t > 0.0 && target_run.legendary.has(220) {
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
            } else if target_run.thorns_t > 0.0 {
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
const MEAT_DROP_CHANCE: f32 = 0.015;
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
    commands.entity(e).insert(Gem {
        kind,
        pulled: false,
        speed: 6.0,
    });
}

/// Gems in the magnet's reach fly to the hero; levels bank as they land.
pub(super) fn collect_gems(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut next: ResMut<NextState<Phase>>,
    hero: Query<&Transform, (With<Hero>, Without<Gem>)>,
    mut gems: Query<(Entity, &mut Transform, &mut Gem)>,
) {
    let Ok(h) = hero.single() else { return };
    if run.dead_t.is_some() {
        return;
    }
    let dt = time.delta_secs();
    let stats = run.stats();
    let target = h.translation + Vec3::Y * 1.0;
    let mut vacuum = false;
    let mut collected = Vec::new();
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
        if !g.pulled && flat(d).length() <= stats.magnet * pickup {
            g.pulled = true;
        }
        if !g.pulled {
            continue;
        }
        if dist < 0.9 {
            match g.kind {
                GemKind::Xp(xp) => run.xp += xp as f32 * data::XP_RATE * stats.growth,
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
                run.xp += xp as f32 * data::XP_RATE * stats.growth;
                commands.entity(entity).despawn();
            }
        }
        run.banner = Some(("Experience Magnet: collected all map XP!".to_owned(), 3.0));
    }
    while run.xp >= data::xp_to_next(run.level) {
        run.xp -= data::xp_to_next(run.level);
        run.level += 1;
        run.pending_levels += 1;
        if run.level == super::mobility::LEARN_LEVEL {
            if let Some(m) = super::mobility::for_class(run.class.id) {
                run.banner = Some((
                    format!("You have learned {}! Use your movement skill key.", m.name),
                    4.0,
                ));
            }
        }
    }
    if run.pending_levels > 0 {
        next.set(Phase::LevelUp);
    }
}

/// Pickups hang still: a wisp's own loop (its circling motes and their trails) stops on its first
/// frame, clip and global sequences alike, as soon as the model attaches.
pub(super) fn still_gems(
    mut players: Query<&mut AnimationPlayer, (With<Gem>, Added<AnimationPlayer>)>,
    mut drives: Query<&mut GlobalSeqDrive, (With<Gem>, Added<GlobalSeqDrive>)>,
) {
    for mut player in &mut players {
        for (_, active) in player.playing_animations_mut() {
            active.set_speed(0.0);
        }
    }
    for mut drive in &mut drives {
        drive.set_paused(true);
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
        assert_eq!(world.resource::<Run>().level, 7);
        assert_eq!(world.resource::<Run>().pending_levels, 6);
        let remainder = 100.0 * data::XP_RATE - (1..7).map(data::xp_to_next).sum::<f32>();
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
        assert!((1300..1700).contains(&meats), "{meats}");
        assert!((220..380).contains(&vacuums), "{vacuums}");
        assert!((4.0..6.0).contains(&(meats as f32 / vacuums as f32)));
    }
}
