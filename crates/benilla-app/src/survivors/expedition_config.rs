//! Host-owned expedition tuning. Training actors deliberately bypass these rules.
use super::data;
use serde::{Deserialize, Serialize};
const FILE: &str = include_str!("../../../../config/survivors-expeditions.toml");
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Config {
    #[serde(skip)]
    dungeon_map: Option<usize>,
    #[serde(skip)]
    pub endless: Option<super::endless::Stage>,
    version: u32,
    #[serde(skip)]
    pub elites: super::monster_mod_effects::Elites,
    #[serde(skip)]
    pub sprinters: super::monster_mod_effects::Sprinters,
    #[serde(skip)]
    pub explosives: super::monster_mod_effects::Explosives,
    #[serde(skip)]
    pub healers: super::monster_mod_effects::Healers,
    #[serde(skip)]
    pub protectors: super::monster_mod_effects::Protectors,
    pub end_multiplier: f32,
    #[serde(default = "default_party_health")]
    multiplayer_health_per_extra_player: f32,
    #[serde(skip)]
    pub ranged: super::monster_mod_effects::Ranged,
    monster_health: f32,
    monster_damage: f32,
    #[serde(default = "default_level_scaling")]
    health_per_level: f32,
    #[serde(default = "default_level_scaling")]
    damage_per_level: f32,
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
pub(super) fn default_level_scaling() -> f32 { 0.02 }
#[derive(Clone, Copy, Deserialize, Serialize)]
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
    pub(super) fn set_monster_mod_effects(&mut self, types: &super::monster_mod_effects::Config) {
        self.elites = types.elites.clone();
        self.sprinters = types.sprinters.clone();
        self.explosives = types.explosives.clone();
        self.healers = types.healers.clone();
        self.protectors = types.protectors.clone();
        self.ranged = types.ranged.clone();
    }
    #[cfg(test)]
    pub(super) fn without_resistances(mut self) -> Self {
        self.monster_resistance_per_level = Default::default();
        self.boss_resistance_per_level = Default::default();
        self
    }
    pub(super) fn parse(text: &str) -> Result<Self, String> {
        let mut c: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        c.set_monster_mod_effects(super::monster_mod_effects::get());
        let valid = |v: f32| v.is_finite() && v > 0.0 && v <= 1000.0;
        if c.version != 1
            || ![c.health_per_level, c.damage_per_level].into_iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(&v))
            || !c.monster_resistance_per_level.valid()
            || !c.boss_resistance_per_level.valid()
            || ![c.monster_armour_per_level, c.boss_armour_per_level]
                .into_iter()
                .all(|v| v.is_finite() && (0.0..=1000.0).contains(&v))
            || !valid(c.end_multiplier)
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
        if data::dungeon(map) {
            return super::dungeon_config::get(map).zone;
        }
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
        1.0 + (self.end_multiplier - 1.0) * self.progress(elapsed)
    }
    pub fn for_map(self, map: usize) -> Self {
        if !data::dungeon(map) { return self; }
        let mut config = super::dungeon_config::get(map).expedition().expect("validated dungeon combat");
        config.dungeon_map = Some(map);
        config
    }
    pub fn elapsed(&self, elapsed: f32) -> f32 {
        (elapsed - self.endless.map_or(0.0, |s| s.started)).max(0.0)
    }
    pub fn boss_offset(&self) -> u32 {
        self.endless.map_or(0, |s| s.boss_offset)
    }
    pub fn reward_multiplier(&self) -> f32 {
        self.endless.map_or(1.0, |s| super::endless::config().reward(s.index))
    }
    pub fn gold_multiplier(&self) -> f32 {
        self.endless.map_or(1.0, |s| super::endless::config().gold(s.index))
    }
    pub fn movement_multiplier(&self) -> f32 {
        self.endless.map_or(1.0, |s| super::endless::config().movement(s.index))
    }
    pub fn duration(&self) -> f32 {
        if let Some(stage) = self.endless {
            return super::endless::config().duration(stage.index);
        }
        self.dungeon_map.map_or(data::VICTORY_SECS, |map| super::dungeon_config::get(map).duration_seconds)
    }
    pub fn progress(&self, elapsed: f32) -> f32 { (self.elapsed(elapsed) / self.duration()).clamp(0.0, 1.0) }
    pub fn boss_waves_due(&self, elapsed: f32) -> u32 {
        self.boss_offset() + boss_waves_due(self.elapsed(elapsed), self.duration())
    }
    pub fn boss_interval(&self) -> f32 {
        self.duration() / data::BOSS_WAVES as f32
    }
    pub fn party_health(&self, players: usize) -> f32 {
        if players <= 1 {
            1.0
        } else {
            self.multiplayer_health_per_extra_player * players.saturating_sub(1) as f32
        }
    }
    pub fn ranged_chance(&self) -> f32 {
        self.ranged.chance
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
        let multiplier = self.endless.map_or(1.0, |s| super::endless::config().stats(s.index, boss));
        let (health, damage) = if boss {
            (
                self.boss_health * z.boss_health,
                self.boss_damage * z.boss_damage,
            )
        } else {
            (
                self.monster_health * z.monster_health,
                self.monster_damage * z.monster_damage,
            )
        };
        (health * multiplier, damage * multiplier)
    }
    pub fn stats_at_level(&self, map: usize, boss: bool, level: u32) -> (f32, f32) {
        let (health, damage) = self.stats(map, boss);
        let levels = level.saturating_sub(1) as f32;
        (health * (1.0 + self.health_per_level * levels),
            damage * (1.0 + self.damage_per_level * levels))
    }
    pub fn population(&self, map: usize, elapsed: f32) -> usize {
        let p = self.progress(elapsed);
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
    #[test]
    fn level_growth_is_linear_configurable_and_compatible_with_older_configs() {
        let config = Config::parse(FILE).unwrap();
        for boss in [false, true] {
            let (health, damage) = config.stats(data::CAMPAIGN[0], boss);
            for (level, multiplier) in [(0, 1.0), (1, 1.0), (60, 2.18), (110, 3.18)] {
                let actual = config.stats_at_level(data::CAMPAIGN[0], boss, level);
                assert!((actual.0 / health - multiplier).abs() < 0.00001);
                assert!((actual.1 / damage - multiplier).abs() < 0.00001);
            }
        }
        let legacy = FILE.lines().filter(|line| !line.starts_with("health_per_level =")
            && !line.starts_with("damage_per_level =")).collect::<Vec<_>>().join("\n");
        assert_eq!(Config::parse(&legacy).unwrap().stats_at_level(0, false, 110), config.stats_at_level(0, false, 110));
        let custom = FILE.replace("health_per_level = 0.02", "health_per_level = 0.0")
            .replace("damage_per_level = 0.02", "damage_per_level = 0.04");
        let custom = Config::parse(&custom).unwrap();
        let base = custom.stats(0, false);
        let scaled = custom.stats_at_level(0, false, 11);
        assert_eq!(scaled.0, base.0);
        assert!((scaled.1 / base.1 - 1.4).abs() < 0.00001);
        for field in ["health_per_level", "damage_per_level"] {
            for invalid in ["-0.01", "nan", "inf", "1.01"] {
                assert!(Config::parse(&FILE.replace(&format!("{field} = 0.02"), &format!("{field} = {invalid}"))).is_err());
            }
        }
    }
    #[test]
    fn all_dungeons_are_harder_than_the_final_expedition_at_the_same_time() {
        let config = Config::default();
        let campaign = data::CAMPAIGN[5];
        for &map in data::DUNGEONS {
            for boss in [false, true] {
                let before = config.stats(campaign, boss);
                let after = config.stats(map, boss);
                assert_eq!(after.0, before.0 * if boss { 6.0 } else { 2.0 });
                assert_eq!(after.1, before.1 * 1.5);
            }
            assert!(config.population(map, 0.0) > config.population(campaign, 0.0));
            assert_eq!(config.zone(map).damage_reduction, config.zone(campaign).damage_reduction);
        }
    }
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
        assert_eq!(c.factor(c.duration() / 2.0), 2.0);
        assert_eq!(c.factor(c.duration()), 3.0);
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
            let (health, damage) = config.stats_at_level(data::CAMPAIGN[0], enemy.boss, enemy.def.level);
            assert_eq!(enemy.max_hp, health);
            assert_eq!(enemy.hp, health * 0.25);
            assert_eq!(enemy.damage, damage * config.end_multiplier);
        }
        world.resource_mut::<Run>().elapsed = 9000.0;
        world.run_system_once(enemies::scale_enemies).unwrap();
        for enemy in world.query::<&enemies::Enemy>().iter(&world) {
            let config = &world.resource::<Run>().expeditions;
            assert_eq!(enemy.max_hp, config.stats_at_level(data::CAMPAIGN[0], enemy.boss, enemy.def.level).0);
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
            let enemy = world.get::<enemies::Enemy>(entity).unwrap();
            let expected = config.stats_at_level(data::CAMPAIGN[0], boss, enemy.def.level).0 * config.end_multiplier;
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
                config.stats_at_level(data::CAMPAIGN[0], enemy.boss, enemy.def.level).0 * 2.5
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
                    let (health, damage) = config.stats_at_level(data::CAMPAIGN[0], enemy.boss, enemy.def.level);
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
                    config.stats_at_level(data::CAMPAIGN[0], boss, enemy.def.level).0
                        * config.factor(450.0)
                        * config.party_health(total)
                );
                world.despawn(entity);
            }
        }
    }
}

fn boss_waves_due(elapsed: f32, duration: f32) -> u32 {
    ((elapsed / duration).clamp(0.0, 1.0) * data::BOSS_WAVES as f32).floor() as u32
}

#[cfg(test)]
mod timing_tests {
    use super::*;

    #[test]
    fn eight_boss_waves_scale_with_duration_and_stop_at_the_endpoint() {
        for duration in [360.0_f32, 720.0, 1440.0] {
            // The normalized schedule is shared by campaign and dungeon configurations.
            for wave in 1..=data::BOSS_WAVES {
                let at = duration * wave as f32 / data::BOSS_WAVES as f32;
                let due = |elapsed: f32| boss_waves_due(elapsed, duration);
                assert_eq!(due(at - 0.01), wave - 1);
                assert_eq!(due(at), wave);
                assert_eq!(due(duration * 2.0), data::BOSS_WAVES);
            }
        }
        for map in [data::CAMPAIGN[0], data::DUNGEONS[0]] {
            let config = Config::default().for_map(map);
            assert_eq!(config.duration(), 720.0);
            assert_eq!(config.boss_interval(), 90.0);
            for wave in 0..=data::BOSS_WAVES {
                assert_eq!(config.boss_waves_due(wave as f32 * 90.0), wave);
            }
            assert_eq!(config.boss_waves_due(1440.0), data::BOSS_WAVES);
        }
    }
}
