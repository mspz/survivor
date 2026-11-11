use super::*;
use bevy::ecs::system::RunSystemOnce;

#[test]
fn all_bases_scale_from_the_resolved_level_and_keep_the_bonus_during_time_updates() {
    use crate::survivors::{endless, monster_mods::Base};
    for map in [data::CAMPAIGN[0], data::DUNGEONS[0]] {
        for endless_level in [None, Some(60), Some(110)] {
            if data::dungeon(map) && endless_level.is_some() {
                continue;
            }
            let mut world = World::new();
            let mut run = Run {
                expedition_map: map,
                expedition_players: 2,
                ..default()
            };
            run.expeditions = run.expeditions.for_map(map);
            if let Some(level) = endless_level {
                let stage = endless::Stage {
                    index: level - 60,
                    ..default()
                };
                run.endless = Some(stage);
                run.expeditions = endless::config().expedition(stage);
            }
            world.insert_resource(run);
            world
                .run_system_once(|mut commands: Commands, mut run: ResMut<Run>| {
                    for base in [Base::Melee, Base::Caster, Base::Boss] {
                        let mut rank = Rank::Normal;
                        rank.base = base;
                        let roster = data::roster(run.expedition_map);
                        let def = if base == Base::Boss {
                            &roster.bosses[0]
                        } else {
                            &roster.enemies[0]
                        };
                        spawn_rank(&mut commands, &mut run, def, Vec3::ZERO, Vec3::X, rank);
                    }
                })
                .unwrap();
            for enemy in world.query::<&Enemy>().iter(&world) {
                let run = world.resource::<Run>();
                let base = run.expeditions.stats(map, enemy.boss);
                let actor = data::dungeon(map)
                    .then(|| crate::survivors::dungeon_config::get(map))
                    .and_then(|config| config.actor(enemy.def.name, enemy.boss));
                let factor = 1.0 + 0.02 * enemy.def.level.saturating_sub(1) as f32;
                let health = base.0
                    * factor
                    * actor.map_or(1.0, |a| a.health_multiplier)
                    * run.expeditions.party_health(2);
                let damage = base.1 * factor * actor.map_or(1.0, |a| a.damage_multiplier);
                assert!((enemy.max_hp / health - 1.0).abs() < 0.00001);
                assert!((enemy.damage / damage - 1.0).abs() < 0.00001);
                if let Some(level) = endless_level {
                    assert_eq!(enemy.def.level, level);
                }
            }
            for mut enemy in world.query::<&mut Enemy>().iter_mut(&mut world) {
                enemy.hp *= 0.5;
            }
            world.resource_mut::<Run>().elapsed = 10000.0;
            world.run_system_once(scale_enemies).unwrap();
            let first: Vec<_> = world
                .query::<&Enemy>()
                .iter(&world)
                .map(|e| (e.hp, e.max_hp, e.damage))
                .collect();
            world.run_system_once(scale_enemies).unwrap();
            let second: Vec<_> = world
                .query::<&Enemy>()
                .iter(&world)
                .map(|e| (e.hp, e.max_hp, e.damage))
                .collect();
            assert_eq!(
                first, second,
                "level bonuses must not compound every update"
            );
            assert!(second.iter().all(|(hp, max, _)| *hp == *max * 0.5));
        }
    }
}
