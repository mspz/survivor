//! Host-owned expedition tuning. Training actors deliberately bypass these rules.
use super::data;
use serde::Deserialize;
const FILE: &str = include_str!("../../../../config/survivors-expeditions.toml");
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Config {
    version: u32,
    pub end_multiplier: f32,
    monster_health: f32,
    monster_damage: f32,
    boss_health: f32,
    boss_damage: f32,
    population_start: usize,
    population_end: usize,
    pub max_monsters: usize,
    zones: [Zone; 6],
}
#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Zone {
    pub monster_quantity: f32,
    monster_health: f32,
    monster_damage: f32,
    pub boss_quantity: usize,
    boss_health: f32,
    boss_damage: f32,
}
impl Config {
    fn parse(text: &str) -> Result<Self, String> {
        let c: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        let valid = |v: f32| v.is_finite() && v > 0.0 && v <= 1000.0;
        if c.version != 1
            || !valid(c.end_multiplier)
            || c.end_multiplier < 1.0
            || ![
                c.monster_health,
                c.monster_damage,
                c.boss_health,
                c.boss_damage,
            ]
            .into_iter()
            .all(valid)
            || c.population_start > c.population_end
            || c.population_end > c.max_monsters
            || c.max_monsters > 5000
            || c.zones.iter().any(|z| {
                z.boss_quantity > 32
                    || ![
                        z.monster_quantity,
                        z.monster_health,
                        z.monster_damage,
                        z.boss_health,
                        z.boss_damage,
                    ]
                    .into_iter()
                    .all(valid)
            })
        {
            return Err("invalid expedition settings".into());
        }
        Ok(c)
    }
    pub fn zone(&self, map: usize) -> Zone {
        self.zones[data::CAMPAIGN.iter().position(|&i| i == map).unwrap_or(0)]
    }
    pub fn factor(&self, elapsed: f32) -> f32 {
        1.0 + (self.end_multiplier - 1.0) * data::progress(elapsed).clamp(0.0, 1.0)
    }
    pub fn stats(&self, map: usize, boss: bool) -> (f32, f32) {
        let z = self.zone(map);
        if boss {
            (
                self.boss_health * z.boss_health,
                self.boss_damage * z.boss_damage,
            )
        } else {
            (
                self.monster_health * z.monster_health,
                self.monster_damage * z.monster_damage,
            )
        }
    }
    pub fn population(&self, map: usize, elapsed: f32) -> usize {
        let p = data::progress(elapsed).clamp(0.0, 1.0);
        (((self.population_start as f32 + p * (self.population_end - self.population_start) as f32)
            * self.zone(map).monster_quantity)
            .ceil() as usize)
            .min(self.max_monsters)
    }
}
impl Default for Config {
    fn default() -> Self {
        let builtin = || Self::parse(FILE).expect("valid embedded expedition config");
        if cfg!(test) {
            return builtin();
        }
        let explicit = crate::webenv::var("WOW_EXPEDITION_CONFIG");
        let path = explicit
            .as_deref()
            .unwrap_or("config/survivors-expeditions.toml");
        match crate::local_state::read_to_string(std::path::Path::new(path)) {
            Ok(text) => Self::parse(&text).unwrap_or_else(|e| {
                bevy::log::warn!("Invalid expedition config {path}: {e}; using defaults");
                builtin()
            }),
            Err(_) => builtin(),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn campaign_and_time_scaling() {
        let c = Config::parse(FILE).unwrap();
        assert_eq!(c.factor(0.0), 1.0);
        assert_eq!(c.factor(450.0), 1.5);
        assert_eq!(c.factor(900.0), 2.0);
        assert_eq!(c.factor(9000.0), 2.0);
        for pair in data::CAMPAIGN.windows(2) {
            for boss in [false, true] {
                let a = c.stats(pair[0], boss);
                let b = c.stats(pair[1], boss);
                assert!(b.0 > a.0 && b.1 > a.1);
            }
        }
        assert!(
            Config::parse(&FILE.replace("end_multiplier = 2.0", "end_multiplier = -1.0")).is_err()
        );
    }

    #[test]
    fn population_and_boss_tuning_are_independent() {
        let mut c = Config::parse(FILE).unwrap();
        let map = data::CAMPAIGN[0];
        let boss = c.stats(map, true);
        c.zones[0].monster_quantity = 2.0;
        c.zones[0].monster_health = 3.0;
        c.zones[0].monster_damage = 4.0;
        assert_eq!(c.population(map, 0.0), 28);
        assert_eq!(c.population(map, 9000.0), c.max_monsters);
        assert_eq!(c.stats(map, false), (144.0, 12.0));
        assert_eq!(c.stats(map, true), boss);
        c.zones[0].boss_quantity = 3;
        assert_eq!(c.zone(map).boss_quantity, 3);
        assert_eq!(c.population(map, 0.0), 28);
    }

    #[test]
    fn alive_enemies_keep_damage_taken_while_scaling() {
        use super::super::{enemies, Run};
        use bevy::ecs::system::RunSystemOnce;
        use bevy::prelude::*;
        let mut world = World::new();
        world.insert_resource(Run::default());
        world
            .run_system_once(|mut commands: Commands, mut run: ResMut<Run>| {
                for boss in [false, true] {
                    enemies::spawn_enemy(
                        &mut commands,
                        &mut run,
                        &data::roster(data::CAMPAIGN[0]).enemies[0],
                        Vec3::ZERO,
                        Vec3::ZERO,
                        boss,
                    );
                }
            })
            .unwrap();
        for mut enemy in world.query::<&mut enemies::Enemy>().iter_mut(&mut world) {
            enemy.hp *= 0.25;
        }
        world.resource_mut::<Run>().elapsed = 900.0;
        world.run_system_once(enemies::scale_enemies).unwrap();
        for enemy in world.query::<&enemies::Enemy>().iter(&world) {
            assert_eq!(enemy.hp / enemy.max_hp, 0.25);
            assert_eq!(enemy.max_hp, if enemy.boss { 1800.0 } else { 96.0 });
            assert_eq!(enemy.damage, if enemy.boss { 20.0 } else { 6.0 });
        }
        world.resource_mut::<Run>().elapsed = 9000.0;
        world.run_system_once(enemies::scale_enemies).unwrap();
        for enemy in world.query::<&enemies::Enemy>().iter(&world) {
            assert_eq!(enemy.max_hp, if enemy.boss { 1800.0 } else { 96.0 });
        }
    }
}
