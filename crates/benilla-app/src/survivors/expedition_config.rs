//! Host-owned expedition tuning. Training actors deliberately bypass these rules.
use super::data;
use serde::Deserialize;
const FILE: &str = include_str!("../../../../config/survivors-expeditions.toml");
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Config {
    version: u32,
    pub end_multiplier: f32,
    #[serde(default = "default_party_health")]
    multiplayer_health_per_extra_player: f32,
    #[serde(default = "default_ranged_chance")]
    ranged_monster_chance: f32,
    monster_health: f32,
    monster_damage: f32,
    monster_armour_per_level: f32,
    boss_armour_per_level: f32,
    #[serde(default)]
    monster_resistance_per_level: super::resistances::Resistances,
    #[serde(default)]
    boss_resistance_per_level: super::resistances::Resistances,
    boss_health: f32,
    boss_damage: f32,
    population_start: usize,
    population_end: usize,
    pub max_monsters: usize,
    zones: [Zone; 6],
}
fn default_party_health() -> f32 {
    2.5
}
fn default_ranged_chance() -> f32 {
    0.02
}
#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Zone {
    damage_reduction: f32,
    pub monster_quantity: f32,
    monster_health: f32,
    monster_damage: f32,
    pub boss_quantity: usize,
    boss_health: f32,
    boss_damage: f32,
}
impl Config {
    #[cfg(test)]
    pub(super) fn without_resistances(mut self) -> Self {
        self.monster_resistance_per_level = Default::default();
        self.boss_resistance_per_level = Default::default();
        self
    }
    pub(super) fn parse(text: &str) -> Result<Self, String> {
        let c: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        let valid = |v: f32| v.is_finite() && v > 0.0 && v <= 1000.0;
        if c.version != 1
            || !c.monster_resistance_per_level.valid()
            || !c.boss_resistance_per_level.valid()
            || ![c.monster_armour_per_level, c.boss_armour_per_level]
                .into_iter()
                .all(|v| v.is_finite() && (0.0..=1000.0).contains(&v))
            || !valid(c.end_multiplier)
            || !c.ranged_monster_chance.is_finite()
            || !(0.0..=1.0).contains(&c.ranged_monster_chance)
            || c.end_multiplier < 1.0
            || !c.multiplayer_health_per_extra_player.is_finite()
            || !(1.0..=1000.0).contains(&c.multiplayer_health_per_extra_player)
            || ![
                c.monster_health,
                c.monster_damage,
                c.boss_health,
                c.boss_damage,
            ]
            .into_iter()
            .all(|v| v.is_finite() && v > 0.0 && v <= 1.0e6)
            || c.population_start > c.population_end
            || c.population_end > c.max_monsters
            || c.max_monsters > 5000
            || c.zones.iter().any(|z| {
                z.boss_quantity > 32
                    || !z.damage_reduction.is_finite()
                    || !(0.0..1.0).contains(&z.damage_reduction)
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
    pub fn defenses(&self, map: usize, level: u32, boss: bool, training: bool) -> (f32, f32) {
        if training {
            return (0.0, 0.0);
        }
        let per_level = if boss {
            self.boss_armour_per_level
        } else {
            self.monster_armour_per_level
        };
        (level as f32 * per_level, self.zone(map).damage_reduction)
    }
    pub fn factor(&self, elapsed: f32) -> f32 {
        1.0 + (self.end_multiplier - 1.0) * data::progress(elapsed).clamp(0.0, 1.0)
    }
    pub fn party_health(&self, players: usize) -> f32 {
        if players <= 1 {
            1.0
        } else {
            self.multiplayer_health_per_extra_player * players.saturating_sub(1) as f32
        }
    }
    pub fn ranged_chance(&self) -> f32 {
        self.ranged_monster_chance
    }
    pub fn resistances(
        &self,
        level: u32,
        boss: bool,
        training: bool,
    ) -> super::resistances::Resistances {
        if training {
            return Default::default();
        }
        if boss {
            self.boss_resistance_per_level
        } else {
            self.monster_resistance_per_level
        }
        .scaled(level)
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
    fn fixed_scaling_config() -> String {
        let mut table: toml::Value = toml::from_str(super::FILE).unwrap();
        table["end_multiplier"] = toml::Value::Float(3.0);
        table["multiplayer_health_per_extra_player"] = toml::Value::Float(2.5);
        toml::to_string(&table).unwrap()
    }
    use super::*;
    #[test]
    fn defenses_scale_and_final_reduction_survives_full_penetration() {
        use super::super::spell_engine::{on_hit, Tags};
        let c = Config::parse(FILE).unwrap();
        for &map in data::CAMPAIGN {
            let (armour, reduction) = c.defenses(map, 60, false, false);
            assert_eq!(armour, 60.0 * c.monster_armour_per_level);
            assert_eq!(reduction, c.zone(map).damage_reduction);
            for tags in [Tags::PHYSICAL, Tags::FIRE] {
                let mods = on_hit::HitModifiers {
                    tags,
                    penetration: 1.0,
                    ..Default::default()
                };
                let damage =
                    on_hit::damage(100.0, 1000.0, 1000.0, false, armour, mods) * (1.0 - reduction);
                assert!((damage - 100.0 * (1.0 - reduction)).abs() < 0.001);
            }
            assert_eq!(c.defenses(map, 60, true, true), (0.0, 0.0));
        }
        assert_eq!(
            c.defenses(data::CAMPAIGN[0], 30, false, false).0,
            30.0 * c.monster_armour_per_level
        );
        assert_eq!(
            c.defenses(data::CAMPAIGN[0], 30, true, false).0,
            30.0 * c.boss_armour_per_level
        );
        for invalid in [-0.1, 1.0, f64::NAN] {
            let mut table: toml::Value = toml::from_str(FILE).unwrap();
            table["zones"].as_array_mut().unwrap()[5]["damage_reduction"] =
                toml::Value::Float(invalid);
            assert!(Config::parse(&toml::to_string(&table).unwrap()).is_err());
        }
    }
    #[test]
    fn campaign_and_time_scaling() {
        let file = fixed_scaling_config();
        let c = Config::parse(&file).unwrap();
        assert_eq!(c.factor(0.0), 1.0);
        assert_eq!(c.factor(450.0), 2.0);
        assert_eq!(c.factor(900.0), 3.0);
        assert_eq!(c.factor(9000.0), 3.0);
        for pair in data::CAMPAIGN.windows(2) {
            for boss in [false, true] {
                let a = c.stats(pair[0], boss);
                let b = c.stats(pair[1], boss);
                assert!(b.0 > a.0 && b.1 > a.1);
            }
        }
        assert!(
            Config::parse(&file.replace("end_multiplier = 3.0", "end_multiplier = -1.0")).is_err()
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
        assert_eq!(c.population(map, 0.0), c.population_start * 2);
        assert_eq!(c.population(map, 9000.0), c.max_monsters);
        assert_eq!(
            c.stats(map, false),
            (c.monster_health * 3.0, c.monster_damage * 4.0)
        );
        assert_eq!(c.stats(map, true), boss);
        c.zones[0].boss_quantity = 3;
        assert_eq!(c.zone(map).boss_quantity, 3);
        assert_eq!(c.population(map, 0.0), c.population_start * 2);
    }

    #[test]
    fn existing_enemies_keep_current_and_maximum_health_while_new_spawns_scale_with_time() {
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
            let config = &world.resource::<Run>().expeditions;
            let (health, damage) = config.stats(data::CAMPAIGN[0], enemy.boss);
            assert_eq!(enemy.max_hp, health);
            assert_eq!(enemy.hp, health * 0.25);
            assert_eq!(enemy.damage, damage * config.end_multiplier);
        }
        world.resource_mut::<Run>().elapsed = 9000.0;
        world.run_system_once(enemies::scale_enemies).unwrap();
        for enemy in world.query::<&enemies::Enemy>().iter(&world) {
            let config = &world.resource::<Run>().expeditions;
            assert_eq!(enemy.max_hp, config.stats(data::CAMPAIGN[0], enemy.boss).0);
        }
        for boss in [false, true] {
            let entity = world
                .run_system_once(move |mut commands: Commands, mut run: ResMut<Run>| {
                    enemies::spawn_enemy(
                        &mut commands,
                        &mut run,
                        &data::roster(data::CAMPAIGN[0]).enemies[0],
                        Vec3::ZERO,
                        Vec3::ZERO,
                        boss,
                    )
                })
                .unwrap();
            let config = &world.resource::<Run>().expeditions;
            let expected = config.stats(data::CAMPAIGN[0], boss).0 * config.end_multiplier;
            let enemy = world.get::<enemies::Enemy>(entity).unwrap();
            assert_eq!(enemy.max_hp, expected);
            assert_eq!(enemy.hp, expected);
        }
    }
    #[test]
    fn party_health_follows_the_requested_linear_ladder_and_legacy_config_defaults() {
        let file = fixed_scaling_config();
        let c = Config::parse(&file).unwrap();
        for (players, expected) in [
            (0, 1.0),
            (1, 1.0),
            (2, 2.5),
            (3, 5.0),
            (4, 7.5),
            (5, 10.0),
            (8, 17.5),
        ] {
            assert_eq!(c.party_health(players), expected);
        }
        let legacy = file.replace("multiplayer_health_per_extra_player = 2.5", "");
        assert_eq!(Config::parse(&legacy).unwrap().party_health(3), 5.0);
        for invalid in ["0", "-1", "nan", "1001"] {
            assert!(Config::parse(&file.replace(
                "multiplayer_health_per_extra_player = 2.5",
                &format!("multiplayer_health_per_extra_player = {invalid}")
            ))
            .is_err());
        }
    }
    #[test]
    fn party_changes_never_alter_existing_health_but_apply_to_new_spawns_including_bosses() {
        use super::super::{
            enemies,
            hero::Hero,
            multiplayer::{Companion, Companions},
            training, Run,
        };
        use bevy::{ecs::system::RunSystemOnce, prelude::*};
        let mut world = World::new();
        world.insert_resource(Run {
            expedition_players: 2,
            expeditions: Config::parse(&fixed_scaling_config()).unwrap(),
            ..default()
        });
        world.init_resource::<Companions>();
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
                enemies::spawn_enemy(
                    &mut commands,
                    &mut run,
                    &training::DUMMY,
                    Vec3::ZERO,
                    Vec3::ZERO,
                    false,
                );
            })
            .unwrap();
        let config = world.resource::<Run>().expeditions.clone();
        for mut enemy in world.query::<&mut enemies::Enemy>().iter_mut(&mut world) {
            if training::actor(enemy.def) {
                assert_eq!(enemy.max_hp, training::DUMMY.hp);
                continue;
            }
            assert_eq!(
                enemy.max_hp,
                config.stats(data::CAMPAIGN[0], enemy.boss).0 * 2.5
            );
            enemy.hp *= 0.25;
        }
        world.resource_mut::<Run>().elapsed = 450.0;
        for total in [2, 3, 4, 2, 1] {
            world.resource_mut::<Companions>().0.clear();
            for id in 1..total {
                let entity = world.spawn_empty().id();
                world.resource_mut::<Companions>().0.insert(
                    id as u64,
                    Companion {
                        entity,
                        run: Run {
                            owner: Some(id as u64),
                            dead_t: Some(0.0),
                            ..default()
                        },
                        physics: Hero { vy: 0.0, air: 0.0 },
                        offer: default(),
                        offer_id: 0,
                    },
                );
            }
            for _ in 0..3 {
                world.run_system_once(enemies::scale_enemies).unwrap();
                for enemy in world.query::<&enemies::Enemy>().iter(&world) {
                    if training::actor(enemy.def) {
                        assert_eq!(enemy.max_hp, training::DUMMY.hp);
                        continue;
                    }
                    let (health, damage) = config.stats(data::CAMPAIGN[0], enemy.boss);
                    assert_eq!(enemy.max_hp, health * 2.5);
                    assert_eq!(enemy.hp, health * 2.5 * 0.25);
                    assert_eq!(enemy.damage, damage * config.factor(450.0));
                    assert_eq!(enemy.hp / enemy.max_hp, 0.25);
                }
            }
            assert_eq!(world.resource::<Run>().expedition_players, total);
            for boss in [false, true] {
                let entity = world
                    .run_system_once(move |mut commands: Commands, mut run: ResMut<Run>| {
                        enemies::spawn_enemy(
                            &mut commands,
                            &mut run,
                            &data::roster(data::CAMPAIGN[0]).enemies[0],
                            Vec3::ZERO,
                            Vec3::ZERO,
                            boss,
                        )
                    })
                    .unwrap();
                let enemy = world.get::<enemies::Enemy>(entity).unwrap();
                assert_eq!(
                    enemy.max_hp,
                    config.stats(data::CAMPAIGN[0], boss).0
                        * config.factor(450.0)
                        * config.party_health(total)
                );
                world.despawn(entity);
            }
        }
    }
}
