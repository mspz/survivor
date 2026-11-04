//! Private, repeatable combat expedition for TEST_ characters; simulated by the host.
use super::{
    data::{self, EnemyDef},
    enemies::{self, Enemy},
    hero, Run, Selection,
};
use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;

pub(super) static DUMMY: EnemyDef = EnemyDef {
    name: "Immortal Training Dummy",
    display: 367,
    hp: 100_000.0,
    speed: 0.0,
    damage: 0.0,
    scale: 1.3,
    xp: 0,
    level: 60,
    big: false,
};
static BOSS: EnemyDef = EnemyDef {
    name: "Training Boss",
    display: 16174,
    hp: 20_000.0,
    speed: 3.0,
    damage: 15.0,
    scale: 1.0,
    xp: 100,
    level: 63,
    big: true,
};
static MOB: EnemyDef = EnemyDef {
    name: "Training Soldier",
    display: 1218,
    hp: 1000.0,
    speed: 3.5,
    damage: 5.0,
    scale: 1.0,
    xp: 10,
    level: 60,
    big: false,
};
pub(super) fn dummy(enemy: &Enemy) -> bool {
    std::ptr::eq(enemy.def, &DUMMY)
}
pub(super) fn actor(def: &EnemyDef) -> bool {
    [&DUMMY, &BOSS, &MOB]
        .into_iter()
        .any(|d| std::ptr::eq(d, def))
}
pub(super) fn active(selection: &Selection) -> bool {
    !selection.in_hub && selection.stage == data::TRAINING_MAP
}

#[derive(Component)]
pub(super) struct Target {
    pub home: Vec3,
}
#[derive(Default)]
struct Slot {
    entity: Option<Entity>,
    respawn_at: Option<f32>,
}
#[derive(Resource, Default)]
pub(super) struct Arena {
    slots: Vec<Slot>,
}
pub(super) fn reset(mut arena: ResMut<Arena>) {
    *arena = Arena::default();
}

const PACK_SIZE: usize = 40;
fn site_center(site: usize) -> Vec3 {
    match site {
        0 => Vec3::new(20.0, 0.0, 0.0),
        1 => Vec3::new(80.0, 0.0, 0.0),
        _ => Vec3::new(50.0, 0.0, 30.0 * 3.0_f32.sqrt()),
    }
}
fn position(slot: usize) -> Vec3 {
    if slot < 2 {
        return site_center(slot);
    }
    // An 8 x 5 formation keeps forty passive targets compact without overlapping bodies.
    let index = slot - 2;
    site_center(2) + Vec3::new((index % 8) as f32 - 3.5, 0.0, (index / 8) as f32 - 2.0) * 1.8
}
pub(super) fn tick(
    mut commands: Commands,
    mut arena: ResMut<Arena>,
    mut run: ResMut<Run>,
    selection: Res<Selection>,
    targets: Query<&Enemy>,
    world: WorldPoint,
) {
    if !active(&selection) {
        return;
    }
    if arena.slots.is_empty() {
        run.banner = Some((
            "Training: 60 yd triangle | Immortal dummy | Boss | 40-mob pack".into(),
            12.0,
        ));
        arena.slots = (0..PACK_SIZE + 2).map(|_| Slot::default()).collect();
    }
    let center = hero::arena(&data::MAPS[data::TRAINING_MAP]);
    for (index, slot) in arena.slots.iter_mut().enumerate() {
        if let Some(entity) = slot.entity {
            if targets.get(entity).is_ok_and(|e| !e.dead) {
                continue;
            }
            let at = *slot
                .respawn_at
                .get_or_insert(run.elapsed + if index == 1 { 3.0 } else { 1.0 });
            if run.elapsed < at {
                continue;
            }
        }
        let home = enemies::ground(&world, center + position(index), center.y);
        let def = match index {
            0 => &DUMMY,
            1 => &BOSS,
            _ => &MOB,
        };
        let entity = enemies::spawn_enemy(&mut commands, &mut run, def, home, center, index == 1);
        commands.entity(entity).insert(Target { home });
        slot.entity = Some(entity);
        slot.respawn_at = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    #[test]
    fn practice_never_auto_wins_but_normal_expeditions_still_do() {
        use crate::{
            creature_anim::{CastEvent, PlaySeq, SpellGoTargets, SwingMessage},
            net::{FieldChanged, Guid, ObjectStore},
        };
        let mut world = World::new();
        world.init_resource::<Run>();
        world.init_resource::<Time>();
        world.init_resource::<NextState<super::super::Phase>>();
        world.init_resource::<PlaySeq>();
        world.init_resource::<Messages<CastEvent>>();
        world.init_resource::<Messages<SpellGoTargets>>();
        world.init_resource::<Messages<SwingMessage>>();
        world.init_resource::<Messages<FieldChanged>>();
        world.init_resource::<Messages<crate::combat_text::CombatTextSpawn>>();
        world.insert_resource(Selection {
            stage: data::TRAINING_MAP,
            ..default()
        });
        world.spawn((
            hero::Hero { vy: 0.0, air: 0.0 },
            Guid(1),
            ObjectStore(benilla_protocol::messages::ObjectFields::from_pairs(&[])),
            super::super::units::Marks::buffs(),
        ));
        world.resource_mut::<Run>().elapsed = data::VICTORY_SECS + 1.0;
        world.run_system_once(hero::hero_vitals).unwrap();
        assert!(!world.resource::<Run>().won);
        world.resource_mut::<Selection>().stage = 0;
        world.run_system_once(hero::hero_vitals).unwrap();
        assert!(world.resource::<Run>().won);
    }
    #[test]
    fn training_has_separate_areas_fixed_stats_and_exact_respawn_delays() {
        let mut world = World::new();
        benilla_world::world_point::init_world_point_resources(&mut world);
        world.init_resource::<Run>();
        world.init_resource::<Arena>();
        world.insert_resource(Selection {
            stage: data::TRAINING_MAP,
            map: data::TRAINING_MAP,
            test_character: true,
            ..default()
        });
        world.run_system_once(tick).unwrap();
        let ids: Vec<_> = world
            .resource::<Arena>()
            .slots
            .iter()
            .map(|s| s.entity.unwrap())
            .collect();
        assert_eq!(ids.len(), PACK_SIZE + 2);
        assert_eq!(ids[2..].len(), 40);
        for (a, b) in [(0, 1), (0, 2), (1, 2)] {
            assert!(((site_center(a) - site_center(b)).length() - 60.0).abs() < 0.001);
        }
        let average = (2..PACK_SIZE + 2).map(position).sum::<Vec3>() / PACK_SIZE as f32;
        assert!((average - site_center(2)).length() < 0.001);
        for a in 2..PACK_SIZE + 2 {
            for b in a + 1..PACK_SIZE + 2 {
                assert!((position(a) - position(b)).length() > 1.7);
            }
        }
        assert!(dummy(world.get::<Enemy>(ids[0]).unwrap()));
        assert!(world.get::<Enemy>(ids[1]).unwrap().boss);
        // Ordinary enemies teleport toward distant players; practice actors must not.
        use crate::creature_anim::{CastEvent, PlaySeq, SpellGoTargets, SwingMessage};
        world.init_resource::<super::super::Rng>();
        world.init_resource::<Time>();
        world.init_resource::<PlaySeq>();
        world.init_resource::<Messages<super::super::abilities::Hit>>();
        world.init_resource::<Messages<CastEvent>>();
        world.init_resource::<Messages<SpellGoTargets>>();
        world.init_resource::<Messages<SwingMessage>>();
        world.init_resource::<Messages<crate::combat_text::CombatTextSpawn>>();
        let center = hero::arena(&data::MAPS[data::TRAINING_MAP]);
        let player = world
            .spawn((
                hero::Hero { vy: 0.0, air: 0.0 },
                Transform::from_translation(center + position(1)),
            ))
            .id();
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(100));
        let hp = world.resource::<Run>().hp;
        for id in &ids {
            let mut enemy = world.get_mut::<Enemy>(*id).unwrap();
            enemy.attack_cd = 0.0;
            enemy.root_t = 0.05;
        }
        for offset in [Vec3::ZERO, Vec3::X * 8.0] {
            for id in &ids {
                let home = world.get::<Target>(*id).unwrap().home;
                world.get_mut::<Transform>(player).unwrap().translation = home + offset;
                world.run_system_once(enemies::move_enemies).unwrap();
                assert_eq!(
                    world.resource::<Run>().hp,
                    hp,
                    "training enemies must never attack"
                );
                for id in &ids {
                    let target = world.get::<Target>(*id).unwrap();
                    let tf = world.get::<Transform>(*id).unwrap();
                    assert!(
                        (super::super::units::flat(tf.translation - target.home)).length() < 0.01
                    );
                    let mv = world
                        .get::<crate::creature_anim::MovementState>(*id)
                        .unwrap();
                    assert_eq!(mv.speed, 0.0);
                    assert_eq!(mv.flags, 0);
                    assert_eq!(world.get::<Enemy>(*id).unwrap().root_t, 0.0);
                }
            }
        }
        for i in [1, 2] {
            world.get_mut::<Enemy>(ids[i]).unwrap().dead = true;
        }
        world.resource_mut::<Run>().elapsed = 10.0;
        world.run_system_once(tick).unwrap();
        assert_eq!(world.resource::<Arena>().slots[1].respawn_at, Some(13.0));
        assert_eq!(world.resource::<Arena>().slots[2].respawn_at, Some(11.0));
        world.resource_mut::<Run>().elapsed = 11.0;
        world.run_system_once(tick).unwrap();
        assert_ne!(world.resource::<Arena>().slots[2].entity, Some(ids[2]));
        assert_eq!(world.resource::<Arena>().slots[1].entity, Some(ids[1]));
        world.resource_mut::<Run>().elapsed = 12.99;
        world.run_system_once(tick).unwrap();
        assert_eq!(world.resource::<Arena>().slots[1].entity, Some(ids[1]));
        world.resource_mut::<Run>().elapsed = 13.0;
        world.run_system_once(tick).unwrap();
        let boss = world.resource::<Arena>().slots[1].entity.unwrap();
        assert_ne!(boss, ids[1]);
        assert_eq!(world.get::<Enemy>(boss).unwrap().max_hp, BOSS.hp);
        world.get_mut::<Enemy>(boss).unwrap().dead = true;
        world.resource_mut::<Run>().elapsed = 5000.0;
        world.run_system_once(tick).unwrap();
        world.resource_mut::<Run>().elapsed = 5003.0;
        world.run_system_once(tick).unwrap();
        let boss = world.resource::<Arena>().slots[1].entity.unwrap();
        assert_eq!(world.get::<Enemy>(boss).unwrap().max_hp, BOSS.hp);
        world.run_system_once(reset).unwrap();
        assert!(world.resource::<Arena>().slots.is_empty());
    }
}
