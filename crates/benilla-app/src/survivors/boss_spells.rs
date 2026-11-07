//! Boss windups lock their ground footprint; only the host resolves their impacts.
use super::{
    enemies::{self, Enemy},
    hero::{self, Hero, HERO_RADIUS},
    multiplayer,
    spell_engine::Tags,
    units::{flat, yaw_toward, Fx, COLOR_HURT},
    Phase, Rng, Run,
};
use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

mod visuals;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum Spell {
    Cone,
    Charge,
    Mortars,
    Rings,
}
impl Spell {
    fn windup(self) -> f32 {
        match self {
            Self::Cone => 1.6,
            Self::Charge => 1.8,
            Self::Mortars => 2.0,
            Self::Rings => 2.4,
        }
    }
    fn cooldown(self) -> f32 {
        match self {
            Self::Cone => 8.0,
            Self::Charge => 12.0,
            Self::Mortars => 10.0,
            Self::Rings => 14.0,
        }
    }
    fn range(self) -> f32 {
        match self {
            Self::Cone => 13.0,
            Self::Charge => 28.0,
            Self::Mortars => 45.0,
            Self::Rings => 19.0,
        }
    }
    fn multiplier(self) -> f32 {
        match self {
            Self::Cone => 1.5,
            Self::Charge => 1.6,
            Self::Mortars => 1.4,
            Self::Rings => 1.5,
        }
    }
    fn source(self) -> &'static str {
        match self {
            Self::Cone => "Boss: frontal cleave",
            Self::Charge => "Boss: charge",
            Self::Mortars => "Boss: mortar barrage",
            Self::Rings => "Boss: concentric rings",
        }
    }
    fn tags(self) -> Tags {
        match self {
            Self::Mortars => Tags::FIRE | Tags::SPELL | Tags::AREA,
            _ => Tags::PHYSICAL | Tags::AREA,
        }
    }
}

/// Also sent in snapshots so every client sees the same locked zone and fill progress.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Warning {
    pub spell: Spell,
    pub origin: [f32; 3],
    pub direction: [f32; 3],
    pub mortars: Vec<[f32; 3]>,
    pub elapsed: f32,
    /// Optional circular footprint: radius and windup seconds for monster death explosions.
    #[serde(default)]
    pub circle: Option<[f32; 2]>,
}
impl Warning {
    pub fn progress(&self) -> f32 {
        (self.elapsed / self.circle.map_or(self.spell.windup(), |c| c[1])).clamp(0.0, 1.0)
    }
    fn new(spell: Spell, origin: Vec3, targets: &[Player], rng: &mut Rng) -> Self {
        let nearest = targets
            .iter()
            .min_by(|a, b| {
                origin
                    .distance_squared(a.pos)
                    .total_cmp(&origin.distance_squared(b.pos))
            })
            .unwrap();
        let direction = flat(nearest.pos - origin)
            .try_normalize()
            .unwrap_or(Vec3::Z);
        let mortars = if spell == Spell::Mortars {
            (0..3)
                .map(|i| {
                    let player = &targets[i % targets.len()];
                    let offset = if i == 0 {
                        Vec3::ZERO
                    } else {
                        let a = rng.range(0.0, std::f32::consts::TAU);
                        Vec3::new(a.cos(), 0.0, a.sin()) * rng.range(4.0, 7.0)
                    };
                    (player.pos + offset).to_array()
                })
                .collect()
        } else {
            vec![]
        };
        Self {
            spell,
            origin: origin.to_array(),
            direction: direction.to_array(),
            mortars,
            elapsed: 0.0,
            circle: None,
        }
    }
    pub(super) fn contains(&self, pos: Vec3, radius: f32) -> bool {
        if let Some([area, _]) = self.circle {
            return flat(pos - Vec3::from_array(self.origin)).length() <= area + radius;
        }
        let delta = flat(pos - Vec3::from_array(self.origin));
        let distance = delta.length();
        match self.spell {
            Spell::Cone => {
                let direction = Vec3::from_array(self.direction);
                let forward = delta.dot(direction);
                let side = delta.cross(direction).y.abs();
                distance <= 13.0 + radius
                    && forward >= -radius
                    && side <= forward + radius * std::f32::consts::SQRT_2
            }
            Spell::Charge => {
                let direction = Vec3::from_array(self.direction);
                let forward = delta.dot(direction);
                forward >= -radius
                    && forward <= 24.0 + radius
                    && delta.cross(direction).y.abs() <= 2.0 + radius
            }
            Spell::Mortars => self
                .mortars
                .iter()
                .any(|p| flat(pos - Vec3::from_array(*p)).length() <= 3.5 + radius),
            Spell::Rings => [(4.0, 7.0), (10.0, 13.0), (16.0, 19.0)]
                .iter()
                .any(|&(inner, outer)| distance + radius >= inner && distance - radius <= outer),
        }
    }
}

#[derive(Resource, Default)]
pub(super) struct Warnings(pub Vec<Warning>);
struct Slot {
    spell: Spell,
    left: f32,
}
struct Cast {
    warning: Warning,
    hit: Vec<Entity>,
}
#[derive(Component)]
pub(super) struct Skills {
    slots: Vec<Slot>,
    cast: Option<Cast>,
    recovery: f32,
}
impl Skills {
    fn roll(elapsed: f32, rng: &mut Rng) -> Self {
        let mut pool = vec![Spell::Cone, Spell::Charge, Spell::Mortars, Spell::Rings];
        let count = if elapsed < 600.0 { 1 } else { 2 };
        let slots = (0..count)
            .map(|i| Slot {
                spell: pool.remove(rng.index(pool.len())),
                left: 3.0 + i as f32 * 3.0,
            })
            .collect();
        Self {
            slots,
            cast: None,
            recovery: 0.0,
        }
    }
    pub fn busy(&self) -> bool {
        self.cast.is_some()
    }
}
#[derive(Clone, Copy)]
struct Player {
    entity: Entity,
    owner: Option<u64>,
    pos: Vec3,
}
fn vulnerable(run: &Run) -> bool {
    run.hp > 0.0 && run.dead_t.is_none() && run.pending_levels == 0
}

fn hit(run: &mut Run, attacker: Entity, player: Entity, enemy: &Enemy, spell: Spell, fx: &mut Fx) {
    if !vulnerable(run) {
        return;
    }
    run.legendary.events.attacker = Some(attacker);
    run.taken_source = spell.source();
    let raw = enemy.damage
        * enemy.weaken
        * spell.multiplier()
        * (1.0 - run.support.power(run.elapsed, "smoke_bomb"));
    let damage = hero::hurt_hero_tagged(run, raw, spell.tags());
    run.legendary.events.attacker = None;
    if damage > 0.0 {
        fx.number(run.owner, player, damage, false, COLOR_HURT);
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn tick(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut rng: ResMut<Rng>,
    world: WorldPoint,
    mut warnings: ResMut<Warnings>,
    mut fx: Fx,
    mut companions: Option<ResMut<multiplayer::Companions>>,
    players: Query<
        (
            Entity,
            &Transform,
            Option<&multiplayer::RemoteOwner>,
            Option<&Hero>,
        ),
        Without<Enemy>,
    >,
    mut bosses: Query<
        (
            Entity,
            &Enemy,
            &mut Transform,
            &mut crate::creature_anim::MovementState,
            Option<&mut Skills>,
        ),
        Without<super::training::Target>,
    >,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    warnings.0.clear();
    let targets: Vec<_> = players
        .iter()
        .filter_map(|(entity, tf, owner, hero)| {
            let owner = if hero.is_some() { None } else { Some(owner?.0) };
            let player = if let Some(id) = owner {
                &companions.as_ref()?.0.get(&id)?.run
            } else {
                &run
            };
            vulnerable(player).then_some(Player {
                entity,
                owner,
                pos: tf.translation,
            })
        })
        .collect();
    for (entity, enemy, mut tf, mut movement, skills) in &mut bosses {
        if !enemy.boss {
            continue;
        }
        let Some(mut skills) = skills else {
            commands
                .entity(entity)
                .insert(Skills::roll(run.elapsed, &mut rng));
            continue;
        };
        if enemy.dead
            || enemy.hp <= 0.0
            || targets.is_empty()
            || enemy.stun_t > 0.0
            || enemy.fear_t > 0.0
        {
            if skills.cast.take().is_some() {
                skills.recovery = 2.0;
            }
            continue;
        }
        for slot in &mut skills.slots {
            slot.left = (slot.left - dt).max(0.0);
        }
        skills.recovery = (skills.recovery - dt).max(0.0);
        if skills.cast.is_none() && skills.recovery <= 0.0 {
            let distance = targets
                .iter()
                .map(|p| flat(p.pos - tf.translation).length())
                .fold(f32::INFINITY, f32::min);
            if let Some(slot) = skills
                .slots
                .iter_mut()
                .find(|s| s.left <= 0.0 && distance <= s.spell.range())
            {
                let warning = Warning::new(slot.spell, tf.translation, &targets, &mut rng);
                slot.left = slot.spell.cooldown() + slot.spell.windup();
                tf.rotation =
                    Quat::from_rotation_y(yaw_toward(Vec3::from_array(warning.direction)));
                skills.cast = Some(Cast {
                    warning,
                    hit: vec![],
                });
            }
        }
        let Some(cast) = skills.cast.as_mut() else {
            continue;
        };
        let before = cast.warning.elapsed;
        cast.warning.elapsed += dt;
        let w = &cast.warning;
        let windup = w.spell.windup();
        movement.speed = 0.0;
        movement.flags = 0;
        let charge = w.spell == Spell::Charge;
        let end = windup + if charge { 0.65 } else { 0.2 };
        if w.elapsed >= windup {
            let from = ((before - windup) / 0.65).clamp(0.0, 1.0) * 24.0;
            let to = ((w.elapsed - windup) / 0.65).clamp(0.0, 1.0) * 24.0;
            if charge {
                tf.translation = enemies::ground(
                    &world,
                    Vec3::from_array(w.origin) + Vec3::from_array(w.direction) * to,
                    tf.translation.y,
                );
                movement.speed = 24.0 / 0.65;
                movement.flags = crate::creature_anim::move_flags::FORWARD;
            }
            for target in &targets {
                if cast.hit.contains(&target.entity) || !w.contains(target.pos, HERO_RADIUS) {
                    continue;
                }
                if charge {
                    let along = flat(target.pos - Vec3::from_array(w.origin))
                        .dot(Vec3::from_array(w.direction));
                    if along + HERO_RADIUS < from || along - HERO_RADIUS > to {
                        continue;
                    }
                } else if before >= windup {
                    continue;
                }
                if let Some(player) =
                    multiplayer::player_run(&mut run, companions.as_deref_mut(), target.owner)
                {
                    hit(player, entity, target.entity, enemy, w.spell, &mut fx);
                    cast.hit.push(target.entity);
                }
            }
        }
        warnings.0.push(w.clone());
        if w.elapsed >= end {
            skills.cast = None;
            skills.recovery = 2.0;
            movement.speed = 0.0;
            movement.flags = 0;
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Warnings>()
        .add_plugins(visuals::plugin)
        .add_systems(OnEnter(Phase::Hub), clear)
        .add_systems(OnEnter(Phase::Menu), clear);
}
fn clear(mut warnings: ResMut<Warnings>) {
    warnings.0.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    fn arena(spell: Spell) -> (World, Entity, Entity) {
        use crate::{
            combat_text::CombatTextSpawn,
            creature_anim::{CastEvent, PlaySeq, SpellGoTargets, SwingMessage},
            net::FieldChanged,
        };
        let mut world = World::new();
        benilla_world::world_point::init_world_point_resources(&mut world);
        world.init_resource::<Time>();
        world.init_resource::<Warnings>();
        world.insert_resource(Rng(123));
        world.init_resource::<PlaySeq>();
        world.init_resource::<Messages<CastEvent>>();
        world.init_resource::<Messages<SpellGoTargets>>();
        world.init_resource::<Messages<SwingMessage>>();
        world.init_resource::<Messages<CombatTextSpawn>>();
        world.init_resource::<Messages<FieldChanged>>();
        world.insert_resource(Run {
            hp: 100.0,
            elapsed: 1.0,
            ..default()
        });
        let player = world
            .spawn((
                Hero { vy: 0.0, air: 0.0 },
                Transform::from_xyz(5.0, 0.0, 0.0),
            ))
            .id();
        let boss = world
            .run_system_once(|mut commands: Commands, mut run: ResMut<Run>| {
                enemies::spawn_enemy(
                    &mut commands,
                    &mut run,
                    &super::super::data::roster(0).bosses[0],
                    Vec3::ZERO,
                    Vec3::X,
                    true,
                )
            })
            .unwrap();
        world.get_mut::<Enemy>(boss).unwrap().damage = 20.0;
        world.entity_mut(boss).insert(Skills {
            slots: vec![Slot { spell, left: 0.0 }],
            cast: None,
            recovery: 0.0,
        });
        (world, player, boss)
    }
    fn advance(world: &mut World, dt: f32) {
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(dt));
        world.resource_mut::<Run>().elapsed += dt;
        world.run_system_once(tick).unwrap();
    }
    #[test]
    fn loadouts_switch_at_ten_minutes_and_always_have_distinct_random_skills() {
        let mut seen = vec![];
        for seed in 1..=64 {
            for (elapsed, count) in [(0.0, 1), (599.99, 1), (600.0, 2), (1200.0, 2)] {
                let skills = Skills::roll(elapsed, &mut Rng(seed));
                assert_eq!(skills.slots.len(), count);
                if count == 2 {
                    assert_ne!(skills.slots[0].spell, skills.slots[1].spell);
                }
                if !seen.contains(&skills.slots[0].spell) {
                    seen.push(skills.slots[0].spell);
                }
                assert!(skills
                    .slots
                    .iter()
                    .all(|s| s.left >= 3.0 && s.spell.cooldown() >= 8.0));
            }
        }
        assert_eq!(seen.len(), 4);
    }
    #[test]
    fn real_bosses_receive_loadouts_but_normal_enemies_do_not() {
        let (mut world, _, boss) = arena(Spell::Cone);
        world.entity_mut(boss).remove::<Skills>();
        world.resource_mut::<Run>().elapsed = 600.0;
        advance(&mut world, 0.01);
        assert_eq!(world.get::<Skills>(boss).unwrap().slots.len(), 2);
        world.get_mut::<Enemy>(boss).unwrap().boss = false;
        world.entity_mut(boss).remove::<Skills>();
        advance(&mut world, 0.01);
        assert!(world.get::<Skills>(boss).is_none());
    }
    #[test]
    fn cone_is_frontal_charge_is_a_corridor_and_rings_leave_safe_gaps() {
        let mut w = Warning {
            spell: Spell::Cone,
            origin: [0.0; 3],
            direction: Vec3::X.to_array(),
            mortars: vec![[8.0, 0.0, 3.0]],
            elapsed: 0.0,
            circle: None,
        };
        assert!(w.contains(Vec3::X * 8.0, HERO_RADIUS));
        assert!(!w.contains(Vec3::NEG_X * 8.0, HERO_RADIUS));
        assert!(!w.contains(Vec3::Z * 8.0, HERO_RADIUS));
        w.spell = Spell::Charge;
        assert!(w.contains(Vec3::X * 20.0, HERO_RADIUS));
        assert!(!w.contains(Vec3::new(20.0, 0.0, 5.0), HERO_RADIUS));
        w.spell = Spell::Rings;
        for radius in [5.5, 11.5, 17.5] {
            assert!(w.contains(Vec3::X * radius, HERO_RADIUS));
        }
        for radius in [0.0, 8.5, 14.5, 22.0] {
            assert!(!w.contains(Vec3::X * radius, HERO_RADIUS));
        }
        w.spell = Spell::Mortars;
        assert!(w.contains(Vec3::new(8.0, 0.0, 3.0), HERO_RADIUS));
        assert!(!w.contains(Vec3::ZERO, HERO_RADIUS));
    }
    #[test]
    fn every_attack_warns_before_damage_has_a_cooldown_and_only_hits_once() {
        for spell in [Spell::Cone, Spell::Charge, Spell::Mortars, Spell::Rings] {
            let (mut world, _, boss) = arena(spell);
            advance(&mut world, 0.1);
            assert_eq!(world.resource::<Run>().hp, 100.0);
            assert_eq!(world.resource::<Warnings>().0.len(), 1);
            advance(&mut world, spell.windup() - 0.2);
            assert_eq!(world.resource::<Run>().hp, 100.0);
            advance(&mut world, 0.4);
            assert!(world.resource::<Run>().hp < 100.0, "{spell:?}");
            let hp = world.resource::<Run>().hp;
            advance(&mut world, 0.8);
            assert_eq!(
                world.resource::<Run>().hp,
                hp,
                "{spell:?} must not hit twice"
            );
            assert!(!world.get::<Skills>(boss).unwrap().busy());
            advance(&mut world, 1.0);
            assert!(world.resource::<Warnings>().0.is_empty());
        }
    }
    #[test]
    fn locked_telegraphs_can_be_dodged_and_dead_bosses_cancel_the_attack() {
        for spell in [Spell::Cone, Spell::Charge, Spell::Mortars, Spell::Rings] {
            let (mut world, player, boss) = arena(spell);
            advance(&mut world, 0.1);
            let origin = world.resource::<Warnings>().0[0].origin;
            world.get_mut::<Transform>(player).unwrap().translation = Vec3::new(-30.0, 0.0, 30.0);
            advance(&mut world, spell.windup() + 0.7);
            assert_eq!(world.resource::<Run>().hp, 100.0);
            assert_eq!(world.resource::<Warnings>().0[0].origin, origin);
            world.get_mut::<Skills>(boss).unwrap().slots[0].left = 0.0;
            world.get_mut::<Skills>(boss).unwrap().recovery = 0.0;
            world.get_mut::<Transform>(player).unwrap().translation = Vec3::X * 5.0;
            advance(&mut world, 0.1);
            world.get_mut::<Enemy>(boss).unwrap().dead = true;
            advance(&mut world, spell.windup() + 1.0);
            assert!(world.resource::<Warnings>().0.is_empty());
            assert_eq!(world.resource::<Run>().hp, 100.0);
        }
    }
    #[test]
    fn charge_moves_over_time_and_hits_the_swept_path_once() {
        let (mut world, player, boss) = arena(Spell::Charge);
        world.get_mut::<Transform>(player).unwrap().translation.x = 20.0;
        advance(&mut world, 0.1);
        advance(&mut world, 1.8);
        assert!(world.get::<Transform>(boss).unwrap().translation.x < 5.0);
        assert_eq!(world.resource::<Run>().hp, 100.0);
        advance(&mut world, 0.3);
        assert_eq!(world.resource::<Run>().hp, 100.0);
        advance(&mut world, 0.3);
        assert!(world.resource::<Run>().hp < 100.0);
        assert!((world.get::<Transform>(boss).unwrap().translation.x - 24.0).abs() < 0.01);
    }
    #[test]
    fn aoe_hits_host_and_guests_but_respects_each_players_reward_protection() {
        for protect_guest in [false, true] {
            let (mut world, _, _) = arena(Spell::Cone);
            let guest = world
                .spawn((
                    multiplayer::RemoteOwner(42),
                    Transform::from_xyz(6.0, 0.0, 0.0),
                ))
                .id();
            let mut companions = multiplayer::Companions::default();
            companions.0.insert(
                42,
                multiplayer::Companion {
                    entity: guest,
                    run: Run {
                        owner: Some(42),
                        hp: 100.0,
                        pending_levels: u32::from(protect_guest),
                        ..default()
                    },
                    physics: Hero { vy: 0.0, air: 0.0 },
                    offer: default(),
                    offer_id: 0,
                },
            );
            world.insert_resource(companions);
            advance(&mut world, 0.1);
            advance(&mut world, 1.7);
            assert!(world.resource::<Run>().hp < 100.0);
            let hp = world.resource::<multiplayer::Companions>().0[&42].run.hp;
            assert_eq!(hp < 100.0, !protect_guest);
        }
    }
    #[test]
    fn warning_replication_preserves_locked_areas_and_fill_progress() {
        let w = Warning {
            spell: Spell::Mortars,
            origin: [1.0, 2.0, 3.0],
            direction: Vec3::X.to_array(),
            mortars: vec![[5.0, 6.0, 7.0]],
            elapsed: 1.0,
            circle: None,
        };
        let decoded: Warning = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
        assert_eq!(decoded.progress(), 0.5);
        assert_eq!(decoded.mortars, w.mortars);
        assert_eq!(decoded.origin, w.origin);
    }
}
