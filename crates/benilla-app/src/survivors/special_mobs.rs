//! Ordinary monster variants. Simulation and damage are host-owned; warnings and spell FX replicate.
use bevy::prelude::*;

use super::{
    Run, RunEntity,
    boss_spells::{Spell, Warning, Warnings},
    enemies::{Enemy, Rank},
    hero::{HERO_RADIUS, Hero},
    multiplayer::{Companions, RemoteOwner},
    spell_engine::Tags,
    units::{COLOR_HEAL, COLOR_HURT, Fx, flat},
};

#[derive(Component, Default)]
pub(super) struct Special {
    cooldown: f32,
    protection_cooldown: f32,
    armed: bool,
}

/// Recomputed each simulation tick; overlapping protectors apply only one aura.
#[derive(Component)]
pub(super) struct Protection(pub f32);

#[derive(Component)]
pub(super) struct Bomb {
    bonuses: super::monster_stats::Bonuses,
    source: Entity,
    origin: Vec3,
    radius: f32,
    duration: f32,
    elapsed: f32,
    damage: f32,
}
impl Bomb {
    fn warning(&self) -> Warning {
        Warning {
            spell: Spell::Mortars,
            origin: self.origin.to_array(),
            direction: [0.0; 3],
            mortars: vec![],
            elapsed: self.elapsed,
            circle: Some([self.radius, self.duration]),
            spatial_scale: self.bonuses.area_radius(), windup_seconds: None,
            zones: vec![],
        }
    }
}

fn ally(enemy: &Enemy) -> bool {
    !enemy.dead && enemy.hp > 0.0 && !enemy.boss && !super::training::actor(enemy.def)
}
fn in_range(a: Vec3, b: Vec3, radius: f32) -> bool {
    flat(a - b).length_squared() <= radius * radius
}
fn explosion_hit(run: &mut Run, player: Entity, bomb: &Bomb, fx: &mut Fx) {
    if run.hp <= 0.0 || run.dead_t.is_some() || run.pending_levels > 0 {
        return;
    }
    run.legendary.events.attacker = Some(bomb.source);
    run.taken_source = "Monster: death explosion";
    let raw = bomb.damage * (1.0 - run.support.power(run.elapsed, "smoke_bomb"));
    let damage = super::monster_stats::deal(run, raw, Tags::FIRE | Tags::SPELL | Tags::AREA, bomb.bonuses, false, None);
    run.legendary.events.attacker = None;
    fx.impact(player, 13812);
    if damage > 0.0 {
        fx.number(run.owner, player, damage, false, COLOR_HURT);
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn tick(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut companions: Option<ResMut<Companions>>,
    mut enemies: Query<(Entity, &Transform, &mut Enemy, &Rank, Option<&mut Special>, Option<&mut super::monster_specials::Runtime>)>,
    protected: Query<Entity, With<Protection>>,
    mut bombs: Query<(Entity, &mut Bomb)>,
    players: Query<(Entity, &Transform, Option<&RemoteOwner>, Has<Hero>), Without<Enemy>>,
    mut warnings: ResMut<Warnings>,
    mut fx: Fx,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let config = run.expeditions.clone();
    let mut heals = Vec::new();
    let mut protectors = Vec::new();
    for (entity, tf, enemy, rank, state, _) in &mut enemies {
        let Some(mut state) = state else {
            continue;
        };
        if enemy.dead {
            if rank.has(Rank::Explosive) && !state.armed {
                state.armed = true;
                commands.spawn((
                    RunEntity,
                    Bomb {
                        bonuses: super::monster_stats::Bonuses(*rank),
                        source: entity,
                        origin: tf.translation,
                        radius: config.explosives.radius,
                        duration: config.explosives.delay,
                        elapsed: 0.0,
                        damage: enemy.damage * enemy.weaken * config.explosives.damage_multiplier,
                    },
                ));
                // Publish immediately, before the deferred bomb entity becomes visible.
                warnings.0.push(Warning {
                    spell: Spell::Mortars,
                    origin: tf.translation.to_array(),
                    direction: [0.0; 3],
                    mortars: vec![],
                    elapsed: 0.0,
                    circle: Some([config.explosives.radius, config.explosives.delay]),
                    spatial_scale: super::monster_stats::Bonuses(*rank).area_radius(), windup_seconds: None,
                    zones: vec![],
                });
            }
            continue;
        }
        if enemy.hp <= 0.0 || super::training::actor(enemy.def) {
            continue;
        }
        state.cooldown = (state.cooldown - dt).max(0.0);
        state.protection_cooldown = (state.protection_cooldown - dt).max(0.0);
        if rank.has(Rank::Protector) {
            protectors.push((entity, tf.translation, state.protection_cooldown <= 0.0));
            if state.protection_cooldown <= 0.0 {
                state.protection_cooldown = 4.0;
            }
        }
        if rank.has(Rank::Healer)
            && state.cooldown <= 0.0
            && enemy.stun_t <= 0.0
            && enemy.fear_t <= 0.0
        {
            state.cooldown = config.healers.interval;
            heals.push((entity, tf.translation));
        }
    }
    for entity in &protected {
        commands.entity(entity).remove::<Protection>();
    }
    for (entity, tf, mut enemy, _, _, mut state) in &mut enemies {
        if enemy.dead || enemy.hp <= 0.0 || super::training::actor(enemy.def) {
            continue;
        }
        let mut shield = false;
        let mut flash = false;
        for &(source, pos, pulse) in &protectors {
            if (source == entity || ally(&enemy)) && in_range(pos, tf.translation, config.protectors.radius) {
                shield = true;
                flash |= pulse;
            }
        }
        if shield {
            commands
                .entity(entity)
                .insert(Protection(config.protectors.damage_reduction));
            if flash {
                fx.impact(entity, 17);
            }
        }
        for &(source, pos) in &heals {
            if (source != entity && !ally(&enemy)) || !in_range(pos, tf.translation, config.healers.radius) {
                continue;
            }
            let restored = (enemy.max_hp * config.healers.heal_fraction)
                .min((enemy.max_hp - enemy.hp).max(0.0));
            if restored > 0.0 {
                super::monster_specials::heal(&mut enemy, state.as_deref_mut(), restored, true);
                fx.cast(source, 635, &[entity], None);
                fx.number(None, entity, restored, false, COLOR_HEAL);
            }
        }
    }
    for (entity, mut bomb) in &mut bombs {
        bomb.elapsed += dt;
        let warning = bomb.warning();
        if bomb.elapsed >= bomb.duration {
            for (player, tf, owner, local) in &players {
                if !warning.contains(tf.translation, HERO_RADIUS) {
                    continue;
                }
                if local {
                    explosion_hit(&mut run, player, &bomb, &mut fx);
                } else if let Some(owner) = owner {
                    if let Some(companion) = companions
                        .as_deref_mut()
                        .and_then(|p| p.0.get_mut(&owner.0))
                    {
                        explosion_hit(&mut companion.run, player, &bomb, &mut fx);
                    }
                }
            }
            commands.entity(entity).despawn();
        }
        warnings.0.push(warning);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        combat_text::CombatTextSpawn,
        creature_anim::{CastEvent, PlaySeq, SpellGoTargets, SwingMessage},
    };
    use bevy::ecs::system::RunSystemOnce;

    fn arena() -> World {
        let mut world = World::new();
        world.init_resource::<Time>();
        world.init_resource::<Run>();
        world.init_resource::<Warnings>();
        world.init_resource::<PlaySeq>();
        world.init_resource::<Messages<CastEvent>>();
        world.init_resource::<Messages<SpellGoTargets>>();
        world.init_resource::<Messages<SwingMessage>>();
        world.init_resource::<Messages<CombatTextSpawn>>();
        world
    }
    fn spawn(world: &mut World, rank: Rank, x: f32) -> Entity {
        let entity = world
            .run_system_once(move |mut commands: Commands, mut run: ResMut<Run>| {
                super::super::enemies::spawn_enemy(
                    &mut commands,
                    &mut run,
                    &super::super::data::roster(0).enemies[0],
                    Vec3::X * x,
                    Vec3::ZERO,
                    rank.is_boss(),
                )
            })
            .unwrap();
        world.entity_mut(entity).insert((rank, Special::default()));
        {
            let mut enemy = world.get_mut::<Enemy>(entity).unwrap();
            enemy.max_hp = 100.0;
            enemy.hp = 50.0;
            enemy.damage = 20.0;
        }
        entity
    }
    fn advance(world: &mut World, dt: f32) {
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(dt));
        world.resource_mut::<Warnings>().0.clear();
        world.run_system_once(tick).unwrap();
    }
    #[test]
    fn combined_boss_mods_heal_protect_and_explode_independently() {
        let mut world = arena();
        let rank = Rank::Boss
            .with(Rank::Healer)
            .with(Rank::Protector)
            .with(Rank::Explosive);
        let source = spawn(&mut world, rank, 0.0);
        let near = spawn(&mut world, Rank::Normal, 3.0);
        advance(&mut world, 0.1);
        assert!(world.get::<Enemy>(source).unwrap().boss);
        assert_eq!(world.get::<Enemy>(source).unwrap().hp, 60.0);
        assert_eq!(world.get::<Protection>(source).unwrap().0, 0.25);
        assert_eq!(world.get::<Enemy>(near).unwrap().hp, 60.0);
        assert_eq!(world.get::<Protection>(near).unwrap().0, 0.25);
        // A protector pulse must not delay the healer's independent cooldown.
        world.resource_mut::<Run>().expeditions.healers.interval = 1.0;
        advance(&mut world, 4.0);
        advance(&mut world, 1.0);
        assert_eq!(world.get::<Enemy>(near).unwrap().hp, 80.0);
        world.get_mut::<Enemy>(source).unwrap().dead = true;
        advance(&mut world, 0.1);
        assert_eq!(world.query::<&Bomb>().iter(&world).count(), 1);
        assert!(world.get::<Protection>(near).is_none());
        advance(&mut world, 0.1);
        assert_eq!(world.query::<&Bomb>().iter(&world).count(), 1);
    }
    #[test]
    fn healers_include_self_but_exclude_other_bosses_and_corpses() {
        let mut world = arena();
        let healer = spawn(&mut world, Rank::Healer, 0.0);
        let near = spawn(&mut world, Rank::Normal, 3.0);
        let far = spawn(&mut world, Rank::Normal, 20.0);
        let boss = spawn(&mut world, Rank::Boss, 1.0);
        let dead = spawn(&mut world, Rank::Normal, 2.0);
        world.get_mut::<Enemy>(dead).unwrap().dead = true;
        advance(&mut world, 0.1);
        assert_eq!(world.get::<Enemy>(near).unwrap().hp, 60.0);
        assert_eq!(world.get::<Enemy>(healer).unwrap().hp, 60.0);
        for entity in [far, boss, dead] {
            assert_eq!(world.get::<Enemy>(entity).unwrap().hp, 50.0);
        }
        advance(&mut world, 1.0);
        assert_eq!(world.get::<Enemy>(near).unwrap().hp, 60.0);
        world.get_mut::<Enemy>(near).unwrap().hp = 97.0;
        advance(&mut world, 4.0);
        assert_eq!(world.get::<Enemy>(near).unwrap().hp, 100.0);
        world.get_mut::<Enemy>(near).unwrap().hp = 60.0;
        world.get_mut::<Enemy>(healer).unwrap().stun_t = 10.0;
        advance(&mut world, 4.0);
        assert_eq!(world.get::<Enemy>(near).unwrap().hp, 60.0);
    }
    #[test]
    fn protection_includes_self_never_stacks_and_ends_when_sources_die_or_move_away() {
        let mut world = arena();
        let first = spawn(&mut world, Rank::Protector, 0.0);
        let near = spawn(&mut world, Rank::Normal, 3.0);
        let far = spawn(&mut world, Rank::Normal, 30.0);
        advance(&mut world, 0.1);
        assert_eq!(world.get::<Protection>(first).unwrap().0, 0.25);
        assert_eq!(world.get::<Protection>(near).unwrap().0, 0.25);
        assert!(world.get::<Protection>(far).is_none());
        let second = spawn(&mut world, Rank::Protector, 2.0);
        advance(&mut world, 0.1);
        assert_eq!(world.get::<Protection>(near).unwrap().0, 0.25);
        world.get_mut::<Enemy>(first).unwrap().dead = true;
        world.get_mut::<Transform>(second).unwrap().translation = Vec3::X * 100.0;
        advance(&mut world, 0.1);
        assert!(world.get::<Protection>(near).is_none());
    }
    #[test]
    fn explosions_warn_wait_survive_corpse_removal_hit_guests_and_do_not_repeat() {
        let mut world = arena();
        let source = spawn(&mut world, Rank::Explosive, 0.0);
        let player = world
            .spawn((
                Hero { vy: 0.0, air: 0.0 },
                Transform::from_xyz(1.0, 0.0, 0.0),
            ))
            .id();
        let guest = world
            .spawn((RemoteOwner(7), Transform::from_xyz(2.0, 0.0, 0.0)))
            .id();
        let far = world
            .spawn((RemoteOwner(8), Transform::from_xyz(20.0, 0.0, 0.0)))
            .id();
        let mut companions = Companions::default();
        for (id, entity) in [(7, guest), (8, far)] {
            companions.0.insert(
                id,
                super::super::multiplayer::Companion {
                    entity,
                    run: Run {
                        owner: Some(id),
                        ..default()
                    },
                    physics: Hero { vy: 0.0, air: 0.0 },
                    offer: default(),
                    offer_id: 0,
                },
            );
        }
        world.insert_resource(companions);
        let hp = world.resource::<Run>().hp;
        world.get_mut::<Enemy>(source).unwrap().dead = true;
        advance(&mut world, 0.1);
        let warning = &world.resource::<Warnings>().0[0];
        assert_eq!(warning.circle, Some([3.5, 1.5]));
        let decoded: Warning =
            serde_json::from_slice(&serde_json::to_vec(warning).unwrap()).unwrap();
        assert_eq!(decoded.circle, warning.circle);
        assert_eq!(decoded.progress(), warning.progress());
        assert!(warning.contains(Vec3::X, HERO_RADIUS));
        assert!(!warning.contains(Vec3::X * 20.0, HERO_RADIUS));
        world.despawn(source);
        advance(&mut world, 1.0);
        assert_eq!(world.resource::<Run>().hp, hp);
        advance(&mut world, 0.5);
        assert!(world.resource::<Run>().hp < hp);
        assert!(world.resource::<Companions>().0[&7].run.hp < hp);
        assert_eq!(world.resource::<Companions>().0[&8].run.hp, hp);
        let after = world.resource::<Run>().hp;
        advance(&mut world, 5.0);
        assert_eq!(world.resource::<Run>().hp, after);
        assert!(world.resource::<Warnings>().0.is_empty());
        assert!(world.get_entity(player).is_ok());
    }
}
