//! Monster-only ground pressure. The host owns locks, impacts and player movement history.
use super::{
    Run, RunEntity,
    boss_spells::{Spell, Warning, Warnings},
    enemies::Enemy,
    hero::Hero,
    monster_mods::Monster,
    monster_stats::Bonuses,
    multiplayer,
    spell_engine::Tags,
    units::{COLOR_HURT, Fx, flat},
};
use bevy::prelude::*;
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct PlayerState {
    pub casts: u64,
    pub silenced_until: f32,
}
#[derive(Component)]
pub(super) struct LeapLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Effect {
    Bombardier,
    Aftermath,
    Flusher,
    Leap,
    Breaker,
    Lane,
    Siege,
    Volley,
    Desecrate,
}
impl Effect {
    fn source(self) -> &'static str {
        match self {
            Self::Bombardier => "Monster: bombardier",
            Self::Aftermath => "Monster: remanence",
            Self::Flusher => "Monster: flusher",
            Self::Leap => "Monster: mage hunter",
            Self::Breaker => "Monster: concentration breaker",
            Self::Lane => "Monster: beater",
            Self::Siege => "Monster: besieger",
            Self::Volley => "Monster: siege volley",
            Self::Desecrate => "Monster: desecrator",
        }
    }
}
#[derive(Component)]
pub(super) struct Ground {
    source: Entity,
    effect: Effect,
    warning: Warning,
    raw: f32,
    tags: Tags,
    bonuses: Bonuses,
    delay: f32,
    duration: f32,
    next: f32,
    // A field's ramp is personal, and resets on leaving it.
    exposure: HashMap<Entity, f32>,
    jump_from: Option<Vec3>,
    critical: bool,
}
#[allow(clippy::too_many_arguments)]
fn ground(
    source: Entity,
    effect: Effect,
    pos: Vec3,
    radius: f32,
    delay: f32,
    duration: f32,
    raw: f32,
    tags: Tags,
    rank: Monster,
) -> Ground {
    Ground {
        source,
        effect,
        raw,
        tags,
        bonuses: Bonuses(rank),
        delay,
        duration,
        next: delay,
        exposure: default(),
        jump_from: None,
        critical: false,
        warning: Warning {
            spell: Spell::Mortars,
            origin: pos.to_array(),
            direction: [0.0; 3],
            mortars: vec![],
            elapsed: 0.0,
            circle: Some([radius, delay]),
            zones: vec![],
            windup_seconds: None,
            spatial_scale: Bonuses(rank).area_radius(),
        },
    }
}

/// Conversion happens instead of missile launch; the circle is locked at launch time.
#[allow(clippy::too_many_arguments)]
pub(super) fn bombard(
    commands: &mut Commands,
    source: Entity,
    pos: Vec3,
    raw: f32,
    tags: Tags,
    rank: Monster,
    delay: f32,
    critical: bool,
) {
    let mut impact = ground(
        source,
        Effect::Bombardier,
        pos,
        3.0,
        1.2 + delay,
        0.0,
        raw * 4.0,
        Tags(tags.0 & !Tags::PROJECTILE.0),
        rank,
    );
    impact.critical = critical;
    commands.spawn((RunEntity, impact));
}

pub(super) fn aftermath(
    commands: &mut Commands,
    source: Entity,
    pos: Vec3,
    raw: f32,
    rank: Monster,
) {
    if rank.has(Monster::Aftermath) {
        commands.spawn((
            RunEntity,
            ground(
                source,
                Effect::Aftermath,
                pos,
                2.5,
                0.5,
                3.0,
                raw * 0.25,
                Tags::FIRE,
                rank,
            ),
        ));
    }
}

#[derive(Clone)]
struct Track {
    pos: Vec3,
    anchor: Vec3,
    velocity: Vec3,
    still: f32,
    casts: u64,
    last_casts: u64,
}
impl Track {
    fn new(pos: Vec3, casts: u64) -> Self {
        Self {
            pos,
            anchor: pos,
            velocity: Vec3::ZERO,
            still: 0.0,
            casts: 0,
            last_casts: casts,
        }
    }
    fn update(&mut self, pos: Vec3, casts: u64, dt: f32) {
        let delta = flat(pos - self.pos);
        // Teleports do not predict impacts far outside the fight.
        self.velocity = if delta.length() <= 20.0 {
            (delta / dt).clamp_length_max(12.0)
        } else {
            Vec3::ZERO
        };
        self.pos = pos;
        if flat(pos - self.anchor).length() > 3.0 {
            self.anchor = pos;
            self.still = 0.0;
            self.casts = 0;
        } else {
            self.still += dt;
            self.casts = self
                .casts
                .saturating_add(casts.wrapping_sub(self.last_casts));
        }
        self.last_casts = casts;
    }
}
#[derive(Default, Resource)]
pub(super) struct State {
    elapsed: f32,
    players: HashMap<Entity, Track>,
    cooldowns: HashMap<Entity, [f32; 7]>,
    salvos: Vec<Salvo>,
}
struct Salvo {
    source: Entity,
    target: Entity,
    next: f32,
    remaining: u8,
    rank: Monster,
    raw: f32,
}

fn active(run: &Run) -> bool {
    run.hp > 0.0 && run.dead_t.is_none() && run.pending_levels == 0 && run.mobile.vanish_t <= 0.0
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn tick(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut state: Local<State>,
    mut companions: Option<ResMut<multiplayer::Companions>>,
    players: Query<
        (
            Entity,
            &Transform,
            Option<&multiplayer::RemoteOwner>,
            Has<Hero>,
        ),
        (
            Without<Enemy>,
            Or<(With<Hero>, With<multiplayer::RemoteOwner>)>,
        ),
    >,
    mut enemies: Query<(Entity, &mut Transform, &Enemy, &Monster), Without<Hero>>,
    mut fields: Query<(Entity, &mut Ground)>,
    mut warnings: ResMut<Warnings>,
    surface: super::geometry::Surface,
    mut fx: Fx,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let now = run.elapsed;
    if now < state.elapsed {
        *state = State::default();
    }
    state.elapsed = now;
    let targets: Vec<_> = players
        .iter()
        .filter_map(|(entity, tf, owner, local)| {
            let owner = if local { None } else { Some(owner?.0) };
            let player = if let Some(id) = owner {
                &companions.as_deref()?.0.get(&id)?.run
            } else {
                &run
            };
            active(player).then_some((entity, owner, tf.translation, player.siege.casts))
        })
        .collect();
    state
        .players
        .retain(|entity, _| targets.iter().any(|t| t.0 == *entity));
    for &(entity, _, pos, casts) in &targets {
        state
            .players
            .entry(entity)
            .or_insert_with(|| Track::new(pos, casts))
            .update(pos, casts, dt);
    }
    state
        .cooldowns
        .retain(|e, _| enemies.get(*e).is_ok_and(|(_, _, en, _)| !en.dead));

    // Each follow-up locks independently using the player's position and velocity at that time.
    let tracks = state.players.clone();
    state.salvos.retain_mut(|salvo| {
        if !enemies
            .get(salvo.source)
            .is_ok_and(|(_, _, e, _)| !e.dead && e.hp > 0.0)
        {
            return false;
        }
        let Some(track) = tracks.get(&salvo.target) else {
            return false;
        };
        if now >= salvo.next {
            let lead = f32::from(3 - salvo.remaining) * 0.6;
            let pos = track.pos + track.velocity * lead;
            commands.spawn((
                RunEntity,
                ground(
                    salvo.source,
                    Effect::Volley,
                    pos,
                    3.0,
                    1.0,
                    0.0,
                    salvo.raw,
                    Tags::FIRE,
                    salvo.rank,
                ),
            ));
            salvo.remaining -= 1;
            salvo.next = now + 0.6;
        }
        salvo.remaining > 0
    });

    for (source, tf, enemy, rank) in &mut enemies {
        if enemy.dead || enemy.hp <= 0.0 || enemy.stun_t > 0.0 || enemy.fear_t > 0.0 {
            continue;
        }
        let pos = tf.translation;
        let farthest = targets
            .iter()
            .filter(|t| flat(t.2 - pos).length() <= 45.0)
            .max_by(|a, b| {
                flat(a.2 - pos)
                    .length_squared()
                    .total_cmp(&flat(b.2 - pos).length_squared())
            });
        let Some(&(target, _, point, _)) = farthest else {
            continue;
        };
        let mut cds = state
            .cooldowns
            .get(&source)
            .copied()
            .unwrap_or([now + (source.to_bits() % 11) as f32 * 0.15; 7]);
        let raw = enemy.damage * enemy.weaken;
        let mods = [
            Monster::Flusher,
            Monster::MageHunter,
            Monster::ConcentrationBreaker,
            Monster::Beater,
            Monster::Besieger,
            Monster::SiegeVolley,
            Monster::Desecrator,
        ];
        for (index, modifier) in mods.into_iter().enumerate() {
            if !rank.has(modifier) || now < cds[index] {
                continue;
            }
            let mut zone = match index {
                0 | 2 => {
                    let candidate = targets
                        .iter()
                        .filter(|t| flat(t.2 - pos).length() <= 45.0)
                        .find(|t| {
                            state.players.get(&t.0).is_some_and(|p| {
                                if index == 0 {
                                    p.still >= 2.0
                                } else {
                                    p.casts >= 2
                                }
                            })
                        });
                    let Some(&(entity, _, point, _)) = candidate else {
                        continue;
                    };
                    let track = state.players.get_mut(&entity).unwrap();
                    if index == 0 {
                        track.still = 0.0;
                    } else {
                        track.casts = 0;
                    }
                    ground(
                        source,
                        if index == 0 {
                            Effect::Flusher
                        } else {
                            Effect::Breaker
                        },
                        point,
                        3.0,
                        1.0,
                        0.0,
                        raw * 1.5,
                        Tags::SHADOW,
                        *rank,
                    )
                }
                1 => {
                    if flat(point - pos).length() < 8.0 {
                        continue;
                    }
                    let mut g = ground(
                        source,
                        Effect::Leap,
                        point,
                        3.5,
                        1.2,
                        0.0,
                        raw * 1.5,
                        Tags::PHYSICAL,
                        *rank,
                    );
                    g.jump_from = Some(pos);
                    commands.entity(source).insert(LeapLock);
                    g
                }
                3 => {
                    let direction = flat(point - pos).normalize_or_zero();
                    for angle in [-0.5_f32, 0.0, 0.5] {
                        let mut lane = ground(
                            source,
                            Effect::Lane,
                            pos,
                            0.0,
                            1.4,
                            0.0,
                            raw * 1.5,
                            Tags::PHYSICAL,
                            *rank,
                        );
                        lane.warning.circle = None;
                        lane.warning.spell = Spell::Charge;
                        lane.warning.direction =
                            (Quat::from_rotation_y(angle) * direction).to_array();
                        lane.warning.windup_seconds = Some(1.4);
                        commands.spawn((RunEntity, lane));
                    }
                    cds[index] = now + 8.0;
                    continue;
                }
                4 | 6 => {
                    let effect = if index == 4 {
                        Effect::Siege
                    } else {
                        Effect::Desecrate
                    };
                    if fields
                        .iter()
                        .filter(|(_, f)| f.source == source && f.effect == effect)
                        .count()
                        >= 2
                    {
                        continue;
                    }
                    ground(
                        source,
                        effect,
                        point,
                        if index == 4 { 4.0 } else { 7.0 },
                        1.5,
                        if index == 4 { 5.0 } else { 8.0 },
                        raw * 0.25,
                        Tags::SHADOW,
                        *rank,
                    )
                }
                _ => {
                    state.salvos.push(Salvo {
                        source,
                        target,
                        next: now + 0.6,
                        remaining: 2,
                        rank: *rank,
                        raw: raw * 1.5,
                    });
                    ground(
                        source,
                        Effect::Volley,
                        point,
                        3.0,
                        1.0,
                        0.0,
                        raw * 1.5,
                        Tags::FIRE,
                        *rank,
                    )
                }
            };
            zone.warning.origin =
                super::enemies::ground(&surface, Vec3::from_array(zone.warning.origin), point.y)
                    .to_array();
            commands.spawn((RunEntity, zone));
            cds[index] = now
                + match index {
                    0 => 4.0,
                    1 => 8.0,
                    2 => 5.0,
                    4 => 6.0,
                    5 => 9.0,
                    _ => 10.0,
                };
        }
        state.cooldowns.insert(source, cds);
    }

    for (entity, mut field) in &mut fields {
        let f = &mut *field;
        if !enemies
            .get(f.source)
            .is_ok_and(|(_, _, e, _)| !e.dead && e.hp > 0.0)
        {
            if f.effect == Effect::Leap {
                commands.entity(f.source).try_remove::<LeapLock>();
            }
            commands.entity(entity).despawn();
            continue;
        }
        let before = f.warning.elapsed;
        f.warning.elapsed += dt;
        warnings.0.push(f.warning.clone());
        if f.warning.elapsed < f.delay {
            continue;
        }
        let persistent = f.duration > 0.0;
        let through = f.warning.elapsed.min(f.delay + f.duration - 0.0001);
        let pulses = if persistent {
            if f.next <= through {
                ((through - f.next).floor() as u32 + 1).min(16)
            } else {
                0
            }
        } else {
            u32::from(f.next <= f.delay)
        };
        if let Some(start) = f.jump_from {
            let destination = Vec3::from_array(f.warning.origin);
            if let Ok((_, mut tf, _, _)) = enemies.get_mut(f.source) {
                let progress = ((f.warning.elapsed - f.delay) / 0.3).clamp(0.0, 1.0);
                tf.translation = super::enemies::ground(
                    &surface,
                    surface.constrain(tf.translation, start.lerp(destination, progress)),
                    tf.translation.y,
                );
                if progress < 1.0 {
                    continue;
                }
            }
            f.jump_from = None;
            commands.entity(f.source).remove::<LeapLock>();
        }
        for &(player, owner, pos, _) in &targets {
            if !f.warning.contains(pos, super::hero::HERO_RADIUS) {
                f.exposure.remove(&player);
                continue;
            }
            let exposure = f.exposure.entry(player).or_default();
            let ramp = if f.effect == Effect::Desecrate {
                // Sum the ramp at each due pulse, including pulses crossed by a slow frame.
                (0..pulses)
                    .map(|i| 1.0 + *exposure + (f.next + i as f32 - before.max(f.delay)).max(0.0))
                    .sum::<f32>()
            } else {
                pulses as f32
            };
            *exposure +=
                f.warning.elapsed.min(f.delay + f.duration).max(f.delay) - before.max(f.delay);
            if pulses == 0 {
                continue;
            }
            let Some(target) = multiplayer::player_run(&mut run, companions.as_deref_mut(), owner)
            else {
                continue;
            };
            target.legendary.events.attacker = Some(f.source);
            target.taken_source = f.effect.source();
            let tags = f.tags
                | Tags::SPELL
                | Tags::AREA
                | if persistent { Tags::DOT } else { Tags::DAMAGE };
            let raw = f.raw
                * super::monster_mod_effects::get().monster_ground_damage_multiplier
                * ramp
                * (1.0 - target.support.power(target.elapsed, "smoke_bomb"));
            let damage = if f.effect == Effect::Bombardier {
                super::monster_stats::deal(target, raw, tags, f.bonuses, f.critical, None)
            } else {
                super::monster_stats::deal_secondary(target, raw, tags, f.bonuses)
            };
            target.legendary.events.attacker = None;
            if damage > 0.0 {
                fx.number(owner, player, damage, f.critical, COLOR_HURT);
                if f.effect == Effect::Bombardier {
                    super::monster_stats::leech(&mut commands, f.source, damage, f.bonuses);
                }
                fx.impact(player, 2120);
                if f.effect == Effect::Breaker {
                    target.siege.silenced_until = target.elapsed + 1.0;
                }
            }
        }
        f.next += pulses as f32;
        if !persistent && pulses > 0 && f.bonuses.0.has(Monster::Aftermath) {
            let mut remnant = ground(
                f.source,
                Effect::Aftermath,
                Vec3::from_array(f.warning.origin),
                2.5,
                0.5,
                3.0,
                f.raw * 0.25,
                Tags::FIRE,
                f.bonuses.0,
            );
            remnant.warning = f.warning.clone();
            remnant.warning.elapsed = 0.0;
            remnant.warning.windup_seconds = Some(0.5);
            if let Some(circle) = &mut remnant.warning.circle {
                circle[1] = 0.5;
            }
            commands.spawn((RunEntity, remnant));
        }
        if (!persistent && pulses > 0) || (persistent && f.warning.elapsed >= f.delay + f.duration)
        {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    fn arena(rank: Monster) -> (World, Schedule, Entity, Entity) {
        let mut world = World::new();
        super::super::geometry::init_test_world(&mut world);
        world.init_resource::<Time>();
        world.init_resource::<Warnings>();
        world.init_resource::<crate::creature_anim::PlaySeq>();
        world.init_resource::<Messages<crate::creature_anim::CastEvent>>();
        world.init_resource::<Messages<crate::creature_anim::SpellGoTargets>>();
        world.init_resource::<Messages<crate::creature_anim::SwingMessage>>();
        world.init_resource::<Messages<crate::combat_text::CombatTextSpawn>>();
        world.init_resource::<Messages<crate::net::FieldChanged>>();
        world.insert_resource(Run {
            hp: 10000.0,
            elapsed: 1.0,
            ..default()
        });
        let player = world
            .spawn((
                Hero { vy: 0.0, air: 0.0 },
                Transform::from_xyz(12.0, 0.0, 0.0),
            ))
            .id();
        let source = world
            .run_system_once(|mut commands: Commands, mut run: ResMut<Run>| {
                super::super::enemies::spawn_enemy(
                    &mut commands,
                    &mut run,
                    &super::super::data::roster(super::super::data::CAMPAIGN[1]).enemies[0],
                    Vec3::ZERO,
                    Vec3::X * 12.0,
                    false,
                )
            })
            .unwrap();
        world.entity_mut(source).insert(rank);
        world.get_mut::<Enemy>(source).unwrap().damage = 20.0;
        let mut schedule = Schedule::default();
        schedule.add_systems(tick);
        (world, schedule, player, source)
    }
    fn step(world: &mut World, schedule: &mut Schedule, dt: f32) {
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(dt));
        world.resource_mut::<Run>().elapsed += dt;
        world.resource_mut::<Warnings>().0.clear();
        schedule.run(world);
    }
    #[test]
    fn all_pressure_mods_are_unstealable_and_only_casters_roll_bombardier() {
        for modifier in Monster::MODS.into_iter().skip(50) {
            assert!(!modifier.stealable());
            let mut run = Run::default();
            run.monster_mods.grant(modifier, 0.0, None, Vec3::ZERO);
            assert_eq!(run.monster_mods.rank(0.0).count(), 0);
            assert!(
                super::super::monster_specials::player::mod_aura(modifier, None)
                    .name
                    .contains(modifier.labels()[0])
            );
        }
        assert!(!Monster::Normal.eligible(Monster::Bombardier, 1, false));
        assert!(!Monster::Boss.eligible(Monster::Bombardier, 4, true));
        let caster: Monster = serde_json::from_str(r#"{"base":"Caster","mods":0}"#).unwrap();
        assert!(caster.eligible(Monster::Bombardier, 1, false));
    }
    #[test]
    fn bombardment_is_locked_dodgeable_and_four_times_base_damage() {
        for dodge in [false, true] {
            let (mut world, mut schedule, player, source) = arena(Monster::Bombardier);
            world
                .run_system_once(move |mut commands: Commands| {
                    bombard(
                        &mut commands,
                        source,
                        Vec3::X * 12.0,
                        20.0,
                        Tags::FIRE,
                        Monster::Bombardier,
                        0.0,
                        false,
                    );
                })
                .unwrap();
            step(&mut world, &mut schedule, 0.5);
            assert_eq!(world.resource::<Run>().hp, 10000.0);
            assert_eq!(world.resource::<Warnings>().0.len(), 1);
            if dodge {
                world.get_mut::<Transform>(player).unwrap().translation.z = 10.0;
            }
            step(&mut world, &mut schedule, 0.8);
            assert_eq!(
                world.resource::<Run>().hp,
                if dodge { 10000.0 } else { 9880.0 }
            );
            step(&mut world, &mut schedule, 1.0);
            assert_eq!(
                world.resource::<Run>().hp,
                if dodge { 10000.0 } else { 9880.0 }
            );
        }
    }
    #[test]
    fn caster_attack_is_converted_without_a_hidden_direct_missile() {
        let (mut world, mut schedule, _, source) = arena(Monster::Bombardier);
        world.insert_resource(super::super::Rng(42));
        world.init_resource::<Messages<super::super::abilities::Hit>>();
        world.entity_mut(source).insert(
            super::super::ranged_enemies::profile(super::super::data::CAMPAIGN[1]).unwrap(),
        );
        world.get_mut::<Enemy>(source).unwrap().attack_cd = 0.0;
        schedule.add_systems(super::super::enemies::move_enemies.after(tick));
        step(&mut world, &mut schedule, 0.1);
        step(&mut world, &mut schedule, 1.1);
        assert_eq!(world.query::<&Ground>().iter(&world).count(), 1);
        step(&mut world, &mut schedule, 0.5);
        assert_eq!(world.resource::<Run>().hp, 10000.0);
        assert_eq!(world.resource::<Warnings>().0.len(), 1);
        step(&mut world, &mut schedule, 0.8);
        assert_eq!(world.resource::<Run>().hp, 9880.0);
    }
    #[test]
    fn stationary_tracking_resets_and_casts_count_only_since_last_movement() {
        let mut track = Track::new(Vec3::ZERO, 10);
        track.update(Vec3::X, 12, 2.0);
        assert_eq!(track.casts, 2);
        assert_eq!(track.still, 2.0);
        track.update(Vec3::X * 4.0, 14, 0.1);
        assert_eq!(track.casts, 0);
        assert_eq!(track.still, 0.0);
        track.update(Vec3::X * 4.0, 16, 0.1);
        assert_eq!(track.casts, 2);
    }
    #[test]
    fn every_autonomous_mod_warns_before_damaging_and_despawns_with_source() {
        for modifier in [
            Monster::Flusher,
            Monster::MageHunter,
            Monster::ConcentrationBreaker,
            Monster::Beater,
            Monster::Besieger,
            Monster::SiegeVolley,
            Monster::Desecrator,
        ] {
            let (mut world, mut schedule, _, source) = arena(modifier);
            step(&mut world, &mut schedule, 0.1);
            world.resource_mut::<Run>().siege.casts = 2;
            for _ in 0..24 {
                step(&mut world, &mut schedule, 0.1);
                if world.query::<&Ground>().iter(&world).count() > 0 {
                    break;
                }
            }
            assert!(
                world.query::<&Ground>().iter(&world).count() > 0,
                "{modifier:?}"
            );
            world.get_mut::<Enemy>(source).unwrap().dead = true;
            step(&mut world, &mut schedule, 0.1);
            assert_eq!(world.query::<&Ground>().iter(&world).count(), 0);
        }
    }
    #[test]
    fn remanence_ticks_without_recursion_and_breaker_blocks_casts() {
        let (mut world, mut schedule, _, source) = arena(Monster::Aftermath);
        world
            .run_system_once(move |mut commands: Commands| {
                aftermath(
                    &mut commands,
                    source,
                    Vec3::X * 12.0,
                    20.0,
                    Monster::Aftermath,
                );
                commands.spawn((
                    RunEntity,
                    ground(
                        source,
                        Effect::Breaker,
                        Vec3::X * 12.0,
                        3.0,
                        1.0,
                        0.0,
                        10.0,
                        Tags::SHADOW,
                        Monster::Normal,
                    ),
                ));
            })
            .unwrap();
        step(&mut world, &mut schedule, 1.1);
        assert!(world.resource::<Run>().siege.silenced_until > world.resource::<Run>().elapsed);
        assert_eq!(world.query::<&Ground>().iter(&world).count(), 1);
        step(&mut world, &mut schedule, 4.0);
        assert_eq!(world.query::<&Ground>().iter(&world).count(), 0);
    }
    #[test]
    fn desecration_ramps_per_player_and_resets_on_leaving_even_between_pulses() {
        let (mut world, mut schedule, player, source) = arena(Monster::Normal);
        world
            .run_system_once(move |mut commands: Commands| {
                commands.spawn((
                    RunEntity,
                    ground(
                        source,
                        Effect::Desecrate,
                        Vec3::X * 12.0,
                        7.0,
                        1.0,
                        8.0,
                        10.0,
                        Tags::SHADOW,
                        Monster::Normal,
                    ),
                ));
            })
            .unwrap();
        step(&mut world, &mut schedule, 1.0);
        let first = world.resource::<Run>().hp;
        step(&mut world, &mut schedule, 1.0);
        let second = world.resource::<Run>().hp;
        assert_eq!(10000.0 - first, 15.0);
        assert_eq!(first - second, 30.0);
        world.get_mut::<Transform>(player).unwrap().translation.z = 20.0;
        step(&mut world, &mut schedule, 0.1);
        world.get_mut::<Transform>(player).unwrap().translation.z = 0.0;
        step(&mut world, &mut schedule, 0.9);
        assert!(second - world.resource::<Run>().hp < 30.0);
    }
    #[test]
    fn salvo_locks_followups_separately_and_leaves_rabatteur_gaps() {
        let (mut world, mut schedule, player, _) = arena(Monster::SiegeVolley);
        for _ in 0..20 {
            step(&mut world, &mut schedule, 0.1);
            if world.query::<&Ground>().iter(&world).count() > 0 {
                break;
            }
        }
        let first = world
            .query::<&Ground>()
            .iter(&world)
            .next()
            .unwrap()
            .warning
            .origin;
        world.get_mut::<Transform>(player).unwrap().translation.z = 3.0;
        step(&mut world, &mut schedule, 0.6);
        let origins: Vec<_> = world
            .query::<&Ground>()
            .iter(&world)
            .map(|g| g.warning.origin)
            .collect();
        assert!(origins.contains(&first));
        assert!(origins.iter().any(|p| p[2] > 3.0));
        let (mut world, mut schedule, _, _) = arena(Monster::Beater);
        for _ in 0..20 {
            step(&mut world, &mut schedule, 0.1);
            if world.query::<&Ground>().iter(&world).count() > 0 {
                break;
            }
        }
        let zones: Vec<_> = world
            .query::<&Ground>()
            .iter(&world)
            .map(|g| g.warning.clone())
            .collect();
        assert_eq!(zones.len(), 3);
        let gap = Vec3::new(20.0 * 0.25_f32.cos(), 0.0, 20.0 * 0.25_f32.sin());
        assert!(
            zones
                .iter()
                .all(|w| !w.contains(gap, super::super::hero::HERO_RADIUS))
        );
    }
    #[test]
    fn mage_hunter_locks_on_a_guest_and_cannot_follow_their_dodge() {
        let (mut world, mut schedule, _, source) = arena(Monster::MageHunter);
        let guest = world
            .spawn((
                multiplayer::RemoteOwner(42),
                Transform::from_xyz(35.0, 0.0, 0.0),
            ))
            .id();
        let mut companions = multiplayer::Companions::default();
        companions.0.insert(
            42,
            multiplayer::Companion {
                entity: guest,
                run: Run {
                    owner: Some(42),
                    hp: 10000.0,
                    ..default()
                },
                physics: Hero { vy: 0.0, air: 0.0 },
                offer: default(),
                offer_id: 0,
            },
        );
        world.insert_resource(companions);
        for _ in 0..20 {
            step(&mut world, &mut schedule, 0.1);
            if world.query::<&Ground>().iter(&world).count() > 0 {
                break;
            }
        }
        let warning = world
            .query::<&Ground>()
            .iter(&world)
            .next()
            .unwrap()
            .warning
            .clone();
        assert_eq!(warning.origin[0], 35.0);
        let copy: Warning =
            serde_json::from_str(&serde_json::to_string(&warning).unwrap()).unwrap();
        assert_eq!(copy.origin, warning.origin);
        world.get_mut::<Transform>(guest).unwrap().translation.z = 12.0;
        step(&mut world, &mut schedule, 1.6);
        assert!((world.get::<Transform>(source).unwrap().translation.x - 35.0).abs() < 0.01);
        assert_eq!(
            world.resource::<multiplayer::Companions>().0[&42].run.hp,
            10000.0
        );
        assert!(world.get::<LeapLock>(source).is_none());
    }
}
