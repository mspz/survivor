//! One immutable balance document per dungeon; restart to reload disk edits.
use super::{data, expedition_config as expedition, resistances::Resistances};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

pub(super) const MAX_REWARD_LEVEL: u32 = 200;
const FILES: [(&str, &str); 6] = [
    (
        "blackrock-depths.toml",
        include_str!("../../../../config/dungeons/blackrock-depths.toml"),
    ),
    (
        "lower-blackrock-spire.toml",
        include_str!("../../../../config/dungeons/lower-blackrock-spire.toml"),
    ),
    (
        "dire-maul-east.toml",
        include_str!("../../../../config/dungeons/dire-maul-east.toml"),
    ),
    (
        "dire-maul-west.toml",
        include_str!("../../../../config/dungeons/dire-maul-west.toml"),
    ),
    (
        "scholomance.toml",
        include_str!("../../../../config/dungeons/scholomance.toml"),
    ),
    (
        "stratholme.toml",
        include_str!("../../../../config/dungeons/stratholme.toml"),
    ),
];

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Config {
    version: u32,
    pub duration_seconds: f32,
    pub item_level: u32,
    pub character_xp_multiplier: f64,
    pub boss_size_multiplier: f32,
    pub monster_attack_interval: f32,
    pub boss_attack_interval: f32,
    pub ring_start: f32,
    pub ring_growth: f32,
    pub ring_radius: f32,
    pub spawn_radius_min: f32,
    pub spawn_radius_max: f32,
    pub boss_spawn_radius: f32,
    pub replenish_divisor: f32,
    pub replenish_max_per_frame: usize,
    pub initial_attack_delay: f32,
    pub combat: Combat,
    pub zone: expedition::Zone,
    pub modifiers: Modifiers,
    pub spells: Spells,
    pub monsters: Vec<Actor>,
    pub bosses: Vec<Actor>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Combat {
    end_multiplier: f32,
    multiplayer_health_per_extra_player: f32,
    monster_health: f32,
    monster_damage: f32,
    boss_health: f32,
    boss_damage: f32,
    monster_armour_per_level: f32,
    boss_armour_per_level: f32,
    monster_resistance_per_level: Resistances,
    boss_resistance_per_level: Resistances,
    population_start: usize,
    population_end: usize,
    max_monsters: usize,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Actor {
    pub name: String,
    pub level: u32,
    pub speed: f32,
    pub scale: f32,
    pub health_multiplier: f32,
    pub damage_multiplier: f32,
    pub xp_multiplier: f32,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Modifiers {
    pub item_level_per_rank: [u32; 6],
    pub xp_per_rank: f64,
    pub movement_per_rank: f32,
    pub damage_per_rank: f32,
    pub attack_rate_per_rank: f32,
    pub bosses_per_rank: usize,
    pub spells_per_rank: usize,
    pub warning_multiplier_per_rank: f32,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Spells {
    pub cast_range: f32,
    pub pulse_interval: f32,
    pub phase_thresholds: [f32; 2],
    pub windup: [f32; 5],
    pub cooldown: [f32; 5],
    pub duration: [f32; 5],
    pub damage: [f32; 5],
    pub initial_delay: f32,
    pub initial_stagger: f32,
    pub recovery: f32,
    pub phase_pause: f32,
    pub phase_stagger: f32,
    pub boss_rotation: f32,
    pub phase_rotation_degrees: f32,
    pub target_radius: [f32; 3],
    pub ring_inner: [f32; 3],
    pub ring_outer: [f32; 3],
    pub cross_width: [f32; 3],
    pub closing_angle_degrees: [f32; 3],
    pub forge_lane_count: [usize; 3],
    pub arcane_lane_count: [usize; 3],
    pub lane_width: [f32; 3],
    pub lane_spacing: f32,
    pub lane_half_length: f32,
    pub stone_rings: [[f32; 4]; 3],
    pub root_count: [usize; 3],
    pub root_radius: [f32; 3],
    pub root_orbit: f32,
    pub sector_inner: [f32; 3],
    pub sector_outer: f32,
    pub plague_angle_degrees: f32,
    pub necrotic_angle_degrees: f32,
}

impl Config {
    fn parse_for_map(text: &str, map: usize) -> Result<Self, String> {
        let config = Self::parse(text)?;
        for boss in [false, true] {
            let actors = if boss {
                &config.bosses
            } else {
                &config.monsters
            };
            let names: Vec<_> = data::MAPS[map].creature_names(boss).collect();
            if actors.len() != names.len()
                || actors.iter().any(|a| !names.contains(&a.name.as_str()))
            {
                return Err("dungeon creature names must match its roster".into());
            }
        }
        Ok(config)
    }
    pub(super) fn actor(&self, name: &str, boss: bool) -> Option<&Actor> {
        (if boss { &self.bosses } else { &self.monsters })
            .iter()
            .find(|a| a.name == name)
    }
    pub(super) fn expedition(&self) -> Result<expedition::Config, String> {
        let value = toml::Value::try_from(&self.combat).map_err(|e| e.to_string())?;
        let mut table = value.as_table().expect("combat table").clone();
        table.insert("version".into(), toml::Value::Integer(1));
        table.insert(
            "zones".into(),
            toml::Value::Array(vec![
                toml::Value::try_from(self.zone)
                    .map_err(|e| e.to_string())?;
                6
            ]),
        );
        expedition::Config::parse(
            &toml::to_string(&toml::Value::Table(table)).map_err(|e| e.to_string())?,
        )
    }
    pub(super) fn parse(text: &str) -> Result<Self, String> {
        let c: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        c.expedition()?;
        let positive = |v: f32| v.is_finite() && v > 0.0 && v <= 1.0e6;
        let s = &c.spells;
        let m = &c.modifiers;
        if c.version != 1
            || ![
                c.duration_seconds,
                c.monster_attack_interval,
                c.boss_attack_interval,
                c.boss_size_multiplier,
                c.ring_start,
                c.ring_growth,
                c.ring_radius,
                c.spawn_radius_min,
                c.spawn_radius_max,
                c.boss_spawn_radius,
                c.replenish_divisor,
                c.initial_attack_delay,
                s.cast_range,
                s.pulse_interval,
                s.initial_delay,
                s.initial_stagger,
                s.recovery,
                s.phase_pause,
                s.phase_stagger,
                s.lane_spacing,
                s.lane_half_length,
                s.root_orbit,
                s.sector_outer,
            ]
            .into_iter()
            .all(positive)
            || c.spawn_radius_min > c.spawn_radius_max
            || c.replenish_max_per_frame == 0
            || c.replenish_max_per_frame > 100
            || !(1..=MAX_REWARD_LEVEL).contains(&c.item_level)
            || m.item_level_per_rank
                .iter()
                .map(|&v| u64::from(v) * 3)
                .sum::<u64>()
                + u64::from(c.item_level)
                > u64::from(MAX_REWARD_LEVEL)
            || !c.character_xp_multiplier.is_finite()
            || !(1.0..=1000.0).contains(&c.character_xp_multiplier)
            || !m.xp_per_rank.is_finite()
            || !(1.0..=4.0).contains(&m.xp_per_rank)
            || ![
                m.movement_per_rank,
                m.damage_per_rank,
                m.attack_rate_per_rank,
            ]
            .into_iter()
            .all(|v| v.is_finite() && (0.0..=10.0).contains(&v))
            || m.bosses_per_rank > 8
            || m.spells_per_rank > 1
            || !m.warning_multiplier_per_rank.is_finite()
            || !(0.1..=1.0).contains(&m.warning_multiplier_per_rank)
            || !(0.0 < s.phase_thresholds[1]
                && s.phase_thresholds[1] < s.phase_thresholds[0]
                && s.phase_thresholds[0] < 1.0)
            || !s.phase_thresholds.iter().all(|v| v.is_finite())
            || !s
                .windup
                .iter()
                .chain(&s.cooldown)
                .chain(&s.duration)
                .chain(&s.damage)
                .chain(&s.target_radius)
                .chain(&s.ring_outer)
                .chain(&s.cross_width)
                .chain(&s.lane_width)
                .chain(&s.root_radius)
                .all(|&v| positive(v))
            || !s
                .ring_inner
                .iter()
                .chain(&s.sector_inner)
                .all(|&v| v.is_finite() && v >= 0.0)
            || (0..3).any(|i| {
                s.ring_inner[i] >= s.ring_outer[i]
                    || s.sector_inner[i] >= s.sector_outer
                    || !(1..=32).contains(&s.forge_lane_count[i])
                    || !(1..=32).contains(&s.arcane_lane_count[i])
                    || !(1..=32).contains(&s.root_count[i])
                    || s.stone_rings[i].iter().any(|&v| !positive(v))
                    || !(s.stone_rings[i][0] < s.stone_rings[i][1]
                        && s.stone_rings[i][1] < s.stone_rings[i][2]
                        && s.stone_rings[i][2] < s.stone_rings[i][3])
            })
            || !s
                .closing_angle_degrees
                .iter()
                .chain([&s.plague_angle_degrees, &s.necrotic_angle_degrees])
                .all(|&v| v.is_finite() && v > 0.0 && v < 360.0)
            || !s.boss_rotation.is_finite()
            || !s.phase_rotation_degrees.is_finite()
            || [&c.monsters, &c.bosses].into_iter().any(|actors| {
                let mut names = std::collections::BTreeSet::new();
                actors.is_empty()
                    || actors.iter().any(|a| {
                        a.name.is_empty()
                            || !names.insert(&a.name)
                            || !(1..=100).contains(&a.level)
                            || ![
                                a.speed,
                                a.scale,
                                a.health_multiplier,
                                a.damage_multiplier,
                                a.xp_multiplier,
                            ]
                            .into_iter()
                            .all(positive)
                    })
            })
        {
            return Err("invalid dungeon balance values".into());
        }
        Ok(c)
    }
}

pub(super) fn get(map: usize) -> &'static Config {
    static CONFIGS: OnceLock<Vec<Config>> = OnceLock::new();
    let configs = CONFIGS.get_or_init(|| {
        FILES
            .iter()
            .enumerate()
            .map(|(index, (name, embedded))| {
                if !cfg!(test) {
                    let relative = std::path::Path::new("config/dungeons").join(name);
                    let candidates = [
                        Some(relative.clone()),
                        benilla_formats::project_folder().map(|p| p.join(&relative)),
                        std::env::current_exe()
                            .ok()
                            .and_then(|p| p.parent().map(|p| p.join(&relative))),
                    ];
                    for path in candidates.into_iter().flatten() {
                        if let Ok(text) = crate::local_state::read_to_string(&path) {
                            match Config::parse_for_map(&text, data::DUNGEONS[index]) {
                                Ok(config) => return config,
                                Err(error) => bevy::log::warn!(
                                    "Invalid dungeon config {}: {error}; using embedded defaults",
                                    path.display()
                                ),
                            }
                            break;
                        }
                    }
                }
                Config::parse_for_map(embedded, data::DUNGEONS[index])
                    .expect("valid embedded dungeon balance")
            })
            .collect()
    });
    &configs[data::DUNGEONS.iter().position(|&i| i == map).unwrap_or(0)]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_six_documents_validate_and_preserve_current_defaults() {
        for (index, (_, text)) in FILES.iter().enumerate() {
            let map = data::DUNGEONS[index];
            let c = Config::parse_for_map(text, map).unwrap();
            assert_eq!(c.item_level, 65);
            assert_eq!(c.duration_seconds, 720.0);
            assert_eq!(
                c.expedition().unwrap().for_map(map).stats(map, true),
                (108000.0, 1425.0)
            );
            assert_eq!(
                c.expedition().unwrap().for_map(map).stats(map, false),
                (640.0, 127.5)
            );
            assert_eq!(c.spells.windup, [8.0, 4.5, 6.0, 6.0, 6.0]);
            assert_eq!(data::roster(map).bosses.len(), c.bosses.len());
        }
    }
    #[test]
    fn malformed_balance_and_unknown_creatures_are_rejected() {
        for text in [
            FILES[0].1.replace("item_level = 65", "item_level = 199"),
            FILES[0].1.replace(
                "phase_thresholds = [0.70, 0.35]",
                "phase_thresholds = [0.30, 0.80]",
            ),
            FILES[0].1.replace("level = 60", "level = 0"),
            FILES[0].1.replace("Anvilrage Overseer", "Typo Monster"),
            FILES[0].1.replace(
                "lane_width = [4.5, 5.5, 6.5]",
                "lane_width = [-1.0, 5.5, 6.5]",
            ),
        ] {
            assert!(Config::parse_for_map(&text, data::DUNGEONS[0]).is_err());
        }
    }
    #[test]
    fn independent_combat_fields_keep_shared_monster_types() {
        let text = FILES[0]
            .1
            .replace("monster_health = 40", "monster_health = 80");
        let custom = Config::parse(&text).unwrap();
        let engine = custom.expedition().unwrap();
        assert_eq!(engine.stats(0, false).0, 1280.0);
        assert_eq!(engine.elites.chance, super::super::monster_types::get().elites.chance);
        assert_eq!(
            get(data::DUNGEONS[1])
                .expedition()
                .unwrap()
                .stats(0, false)
                .0,
            640.0
        );
    }
}
