//! Dungeon-only repertoires and health-phase geometry, shared by damage and warning meshes.
use super::*;

#[derive(Clone, Copy)]
enum Style {
    Forge,
    Stone,
    Roots,
    Arcane,
    Necrotic,
    Plague,
}
struct Kit {
    boss: &'static str,
    style: Style,
    school: Tags,
    names: [&'static str; 3],
}
macro_rules! k {
    ($boss:literal, $style:ident, $school:ident, $a:literal, $b:literal, $c:literal) => {
        Kit {
            boss: $boss,
            style: Style::$style,
            school: Tags::$school,
            names: [$a, $b, $c],
        }
    };
}
const KITS: &[&[Kit]] = &[
    &[
        k!(
            "High Interrogator Gerstahn",
            Necrotic,
            SHADOW,
            "Shadow Inquisition",
            "Chains of Torment",
            "Circle of Confession"
        ),
        k!(
            "Pyromancer Loregrain",
            Forge,
            FIRE,
            "Runes of the Firelord",
            "Living Embers",
            "Furnace Ring"
        ),
        k!(
            "General Angerforge",
            Stone,
            PHYSICAL,
            "Iron Phalanx",
            "Siege Bombardment",
            "The General's Encirclement"
        ),
        k!(
            "Golem Lord Argelmach",
            Arcane,
            LIGHTNING,
            "Overloaded Conductors",
            "Golem Discharges",
            "Containment Failure"
        ),
        k!(
            "Hurley Blackbreath",
            Forge,
            FIRE,
            "Burning Ale",
            "Explosive Kegs",
            "Blackbreath's Inferno"
        ),
        k!(
            "Emperor Dagran Thaurissan",
            Forge,
            FIRE,
            "Imperial Furnace",
            "Hands of Ragnaros",
            "The Emperor's Crucible"
        ),
    ],
    &[
        k!(
            "Highlord Omokk",
            Stone,
            PHYSICAL,
            "Omokk's Fault Lines",
            "Crushing Debris",
            "Highlord's Tremor"
        ),
        k!(
            "Shadow Hunter Vosh'gajin",
            Necrotic,
            SHADOW,
            "Shadow Hunter's Sigils",
            "Hexed Ground",
            "Voodoo Encirclement"
        ),
        k!(
            "War Master Voone",
            Arcane,
            PHYSICAL,
            "Crossing Axes",
            "Marked for Execution",
            "War Master's Gauntlet"
        ),
        k!(
            "Mother Smolderweb",
            Roots,
            NATURE,
            "Smolderweb's Brood",
            "Venomous Cocoons",
            "Mother's Hunting Web"
        ),
        k!(
            "Spirestone Butcher",
            Stone,
            PHYSICAL,
            "Butcher's Cleaving Ground",
            "Bloodied Ground",
            "Spirestone Slaughter"
        ),
        k!(
            "Overlord Wyrmthalak",
            Forge,
            FIRE,
            "Overlord's Scorched Earth",
            "Dragonfire Brands",
            "Wyrmthalak's Dominion"
        ),
    ],
    &[
        k!(
            "Pusillin",
            Arcane,
            FIRE,
            "Pusillin's Trick",
            "Mischievous Firebombs",
            "The Imp's Escape"
        ),
        k!(
            "Lethtendris",
            Necrotic,
            SHADOW,
            "Lethtendris's Fel Runes",
            "Sacrificial Brands",
            "Ritual of Binding"
        ),
        k!(
            "Hydrospawn",
            Stone,
            COLD,
            "Tidal Fractures",
            "Erupting Geysers",
            "Hydrospawn's Undertow"
        ),
        k!(
            "Zevrim Thornhoof",
            Roots,
            SHADOW,
            "Thornhoof's Altar",
            "Sacrificial Thorns",
            "Circle of Sacrifice"
        ),
        k!(
            "Alzzin the Wildshaper",
            Roots,
            NATURE,
            "Alzzin's Wild Growth",
            "Seeds of Corruption",
            "The Withering Grove"
        ),
    ],
    &[
        k!(
            "Tendris Warpwood",
            Roots,
            NATURE,
            "Tendris's Root Network",
            "Warpwood Seeds",
            "Ancient's Embrace"
        ),
        k!(
            "Magister Kalendris",
            Arcane,
            ARCANE,
            "Kalendris's Ley Lines",
            "Unstable Focus",
            "Magister's Confinement"
        ),
        k!(
            "Illyanna Ravenoak",
            Roots,
            NATURE,
            "Ravenoak's Hunting Grounds",
            "Marked Prey",
            "The Hunter's Trap"
        ),
        k!(
            "Immol'thar",
            Necrotic,
            SHADOW,
            "Eyes of Immol'thar",
            "Watcher Detonations",
            "Demonic Prison"
        ),
        k!(
            "Prince Tortheldrin",
            Arcane,
            ARCANE,
            "Tortheldrin's Arcane Lattice",
            "Royal Decree",
            "The Prince's Isolation"
        ),
    ],
    &[
        k!(
            "Kirtonos the Herald",
            Necrotic,
            SHADOW,
            "Herald's Shadow",
            "Blood Summons",
            "Kirtonos's Feeding Circle"
        ),
        k!(
            "Jandice Barov",
            Arcane,
            ARCANE,
            "Barov's Hall of Mirrors",
            "Illusory Execution",
            "Jandice's Maze"
        ),
        k!(
            "Rattlegore",
            Stone,
            PHYSICAL,
            "Rattlegore's Bone Quakes",
            "Shattering Graves",
            "The Bone Pit"
        ),
        k!(
            "Ras Frostwhisper",
            Plague,
            COLD,
            "Frostwhisper's Whiteout",
            "Frozen Tombs",
            "The Frozen Sanctuary"
        ),
        k!(
            "Vectus",
            Forge,
            FIRE,
            "Vectus's Burning Lecture",
            "Forbidden Experiments",
            "The Scorched Classroom"
        ),
        k!(
            "Darkmaster Gandling",
            Necrotic,
            SHADOW,
            "Gandling's Examination",
            "Expulsion Sigils",
            "The Final Examination"
        ),
    ],
    &[
        k!(
            "Baroness Anastari",
            Necrotic,
            SHADOW,
            "Anastari's Lament",
            "Possessed Ground",
            "The Baroness's Embrace"
        ),
        k!(
            "Nerub'enkan",
            Roots,
            NATURE,
            "Nerubian Burrows",
            "Infested Ground",
            "The Crypt Web"
        ),
        k!(
            "Maleki the Pallid",
            Plague,
            COLD,
            "Maleki's Frozen Wake",
            "Pallid Tombs",
            "The Cold Ring"
        ),
        k!(
            "Ramstein the Gorger",
            Plague,
            NATURE,
            "Ramstein's Plague Wave",
            "Bile Eruptions",
            "The Slaughterhouse"
        ),
        k!(
            "Baron Rivendare",
            Plague,
            SHADOW,
            "Rivendare's Death March",
            "Runeblade Brands",
            "The Baron's Blight"
        ),
    ],
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(in crate::survivors) struct Ability {
    dungeon: u8,
    boss: u8,
    slot: u8,
}
impl Ability {
    pub(super) fn range(self) -> f32 {
        self.balance().cast_range
    }
    fn balance(self) -> &'static super::super::dungeon_config::Spells {
        &super::super::dungeon_config::get(
            *super::super::data::DUNGEONS
                .get(self.dungeon as usize)
                .unwrap_or(&super::super::data::DUNGEONS[0]),
        )
        .spells
    }
    fn kit(self) -> &'static Kit {
        KITS.get(self.dungeon as usize)
            .and_then(|kits| kits.get(self.boss as usize))
            .unwrap_or(&KITS[0][0])
    }
    pub(super) fn source(self) -> &'static str {
        match self.slot {
            3 => "Crossfire",
            4 => "Closing Sector",
            _ => self.kit().names[self.slot.min(2) as usize],
        }
    }
    pub(super) fn tags(self) -> Tags {
        self.kit().school | Tags::SPELL | Tags::AREA
    }
    pub(super) fn windup(self) -> f32 {
        self.balance().windup[self.slot.min(4) as usize]
    }
    pub(super) fn cooldown(self) -> f32 {
        self.balance().cooldown[self.slot.min(4) as usize]
    }
    pub(super) fn duration(self) -> f32 {
        self.balance().duration[self.slot.min(4) as usize]
    }
    pub(super) fn damage(self) -> f32 {
        self.balance().damage[self.slot.min(4) as usize]
    }
}

#[cfg(test)]
pub(super) fn phase(hp: f32, max_hp: f32) -> u8 {
    phase_for(hp, max_hp, super::super::data::DUNGEONS[0])
}
pub(super) fn phase_for(hp: f32, max_hp: f32, map: usize) -> u8 {
    let fraction = if max_hp > 0.0 { hp / max_hp } else { 1.0 };
    let thresholds = super::super::dungeon_config::get(map)
        .spells
        .phase_thresholds;
    if fraction <= thresholds[1] {
        2
    } else if fraction <= thresholds[0] {
        1
    } else {
        0
    }
}
pub(super) fn loadout(map: usize, name: &str, extra: usize) -> Skills {
    let dungeon = super::super::data::DUNGEONS
        .iter()
        .position(|&m| m == map)
        .unwrap_or(0);
    let boss = KITS[dungeon]
        .iter()
        .position(|kit| kit.boss == name)
        .unwrap_or(0);
    let mut order = vec![0, 1];
    if boss % 2 == 1 {
        order.reverse();
    }
    order.extend(2..2 + extra.min(3) as u8);
    Skills {
        slots: order
            .into_iter()
            .enumerate()
            .map(|(i, slot)| Slot {
                spell: Spell::Dungeon(Ability {
                    dungeon: dungeon as u8,
                    boss: boss as u8,
                    slot,
                }),
                left: super::super::dungeon_config::get(map).spells.initial_delay
                    + i as f32
                        * super::super::dungeon_config::get(map)
                            .spells
                            .initial_stagger,
            })
            .collect(),
        cast: None,
        recovery: 0.0,
        dungeon_phase: 0,
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(in crate::survivors) enum Zone {
    Ring {
        center: [f32; 3],
        inner: f32,
        outer: f32,
    },
    Lane {
        from: [f32; 3],
        to: [f32; 3],
        width: f32,
    },
    Sector {
        center: [f32; 3],
        inner: f32,
        outer: f32,
        start: f32,
        angle: f32,
    },
}
impl Zone {
    pub(super) fn scale_area(&mut self, scale: f32) {
        match self {
            Self::Ring { inner, outer, .. } | Self::Sector { inner, outer, .. } => {
                *inner *= scale; *outer *= scale;
            }
            Self::Lane { from, to, width } => {
                let a = Vec3::from_array(*from); let b = Vec3::from_array(*to);
                let center = (a + b) * 0.5;
                *from = (center + (a - center) * scale).to_array();
                *to = (center + (b - center) * scale).to_array();
                *width *= scale;
            }
        }
    }
    pub(super) fn contains(&self, pos: Vec3, radius: f32) -> bool {
        match *self {
            Self::Ring {
                center,
                inner,
                outer,
            } => {
                let distance = flat(pos - Vec3::from_array(center)).length();
                distance + radius >= inner && distance <= outer + radius
            }
            Self::Lane { from, to, width } => {
                let from = Vec3::from_array(from);
                let delta = flat(Vec3::from_array(to) - from);
                let length = delta.length();
                let direction = delta.normalize_or_zero();
                let point = flat(pos - from);
                let along = point.dot(direction);
                along >= -radius
                    && along <= length + radius
                    && point.cross(direction).y.abs() <= width + radius
            }
            Self::Sector {
                center,
                inner,
                outer,
                start,
                angle,
            } => {
                let point = flat(pos - Vec3::from_array(center));
                let distance = point.length();
                let tolerance = (radius / distance.max(radius).max(0.001)).asin();
                let relative =
                    (point.z.atan2(point.x) - start + tolerance).rem_euclid(std::f32::consts::TAU);
                distance + radius >= inner
                    && distance <= outer + radius
                    && relative <= angle + 2.0 * tolerance
            }
        }
    }
}

pub(super) fn warning(ability: Ability, arena: Vec3, phase: u8, targets: &[Player]) -> Warning {
    use std::f32::consts::{PI, TAU};
    let stage = usize::from(phase.min(2));
    let config = ability.balance();
    let rotation = ability.boss as f32 * config.boss_rotation
        + stage as f32 * config.phase_rotation_degrees.to_radians();
    let point = |angle: f32, radius: f32| arena + Vec3::new(angle.cos(), 0.0, angle.sin()) * radius;
    let mut zones = vec![];
    let ring = |center: Vec3, inner, outer| Zone::Ring {
        center: center.to_array(),
        inner,
        outer,
    };
    if ability.slot == 1 {
        for target in targets {
            zones.push(ring(target.pos, 0.0, config.target_radius[stage]));
        }
    } else if ability.slot == 2 {
        zones.push(ring(
            arena,
            config.ring_inner[stage],
            config.ring_outer[stage],
        ));
    } else if ability.slot == 3 {
        for angle in [rotation, rotation + PI / 2.0] {
            let direction = Vec3::new(angle.cos(), 0.0, angle.sin());
            zones.push(Zone::Lane {
                from: (arena - direction * config.lane_half_length).to_array(),
                to: (arena + direction * config.lane_half_length).to_array(),
                width: config.cross_width[stage],
            });
        }
    } else if ability.slot == 4 {
        zones.push(Zone::Sector {
            center: arena.to_array(),
            inner: 0.0,
            outer: config.sector_outer,
            start: rotation + PI,
            angle: config.closing_angle_degrees[stage].to_radians(),
        });
    } else {
        match ability.kit().style {
            Style::Forge | Style::Arcane => {
                let count = if matches!(ability.kit().style, Style::Arcane) {
                    config.arcane_lane_count[stage]
                } else {
                    config.forge_lane_count[stage]
                };
                let direction = Vec3::new(rotation.cos(), 0.0, rotation.sin());
                let side = direction.cross(Vec3::Y);
                for index in 0..count {
                    let offset = (index as f32 - (count - 1) as f32 * 0.5) * config.lane_spacing;
                    zones.push(Zone::Lane {
                        from: (arena + side * offset - direction * config.lane_half_length)
                            .to_array(),
                        to: (arena + side * offset + direction * config.lane_half_length)
                            .to_array(),
                        width: config.lane_width[stage],
                    });
                }
            }
            Style::Stone => {
                zones.push(ring(
                    arena,
                    config.stone_rings[stage][0],
                    config.stone_rings[stage][1],
                ));
                zones.push(ring(
                    arena,
                    config.stone_rings[stage][2],
                    config.stone_rings[stage][3],
                ));
            }
            Style::Roots => {
                let count = config.root_count[stage];
                for index in 0..count {
                    zones.push(ring(
                        point(
                            rotation + TAU * index as f32 / count as f32,
                            config.root_orbit,
                        ),
                        0.0,
                        config.root_radius[stage],
                    ));
                }
            }
            Style::Necrotic | Style::Plague => zones.push(Zone::Sector {
                center: arena.to_array(),
                inner: config.sector_inner[stage],
                outer: config.sector_outer,
                start: rotation,
                angle: if matches!(ability.kit().style, Style::Plague) {
                    config.plague_angle_degrees.to_radians()
                } else {
                    config.necrotic_angle_degrees.to_radians()
                },
            }),
        }
    }
    Warning {
        spatial_scale: 1.0, windup_seconds: None,
        spell: Spell::Dungeon(ability),
        origin: arena.to_array(),
        direction: Vec3::X.to_array(),
        mortars: vec![],
        elapsed: 0.0,
        circle: None,
        zones,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn arsenal_ranks_add_distinct_spells_up_to_five() {
        let map = super::super::super::data::DUNGEONS[0];
        for rank in 0..=3 {
            let skills = super::loadout(map, "High Interrogator Gerstahn", rank);
            assert_eq!(skills.slots.len(), 2 + rank);
            for (i, slot) in skills.slots.iter().enumerate() {
                assert!(
                    skills.slots[..i]
                        .iter()
                        .all(|other| other.spell != slot.spell)
                );
            }
        }
    }
    use super::*;
    #[test]
    fn every_dungeon_boss_has_its_own_spells_and_each_phase_changes_the_zones() {
        let mut names = std::collections::HashSet::new();
        for (dungeon, &map) in super::super::super::data::DUNGEONS.iter().enumerate() {
            let roster = super::super::super::data::roster(map);
            assert_eq!(KITS[dungeon].len(), roster.bosses.len());
            for boss in &roster.bosses {
                let skills = loadout(map, boss.name, 1);
                assert_eq!(skills.slots.len(), 3);
                for slot in skills.slots {
                    let Spell::Dungeon(ability) = slot.spell else {
                        panic!("expedition spell in a dungeon");
                    };
                    assert_eq!(ability.kit().boss, boss.name);
                    assert!(names.insert(ability.source()));
                    let targets = [Player {
                        entity: Entity::PLACEHOLDER,
                        owner: None,
                        pos: Vec3::X * 20.0,
                    }];
                    let mut previous = vec![];
                    for stage in 0..=2 {
                        let w = warning(ability, Vec3::ZERO, stage, &targets);
                        assert_ne!(w.zones, previous);
                        let mut safe = 0;
                        let mut danger = 0;
                        for x in -10..=10 {
                            for z in -10..=10 {
                                let pos = Vec3::new(x as f32 * 5.0, 0.0, z as f32 * 5.0);
                                if pos.length() > 55.0 {
                                    continue;
                                }
                                if w.contains(pos, HERO_RADIUS) {
                                    danger += 1;
                                } else {
                                    safe += 1;
                                }
                            }
                        }
                        assert!(safe > 0 && danger > 0, "{} phase {stage}", ability.source());
                        let restored: Warning =
                            serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
                        assert_eq!(restored.zones, w.zones);
                        previous = w.zones;
                    }
                }
                assert_eq!(loadout(map, boss.name, 0).slots.len(), 2);
            }
        }
    }
    #[test]
    fn health_thresholds_define_three_phases() {
        assert_eq!(phase(100.0, 100.0), 0);
        assert_eq!(phase(70.01, 100.0), 0);
        assert_eq!(phase(70.0, 100.0), 1);
        assert_eq!(phase(35.01, 100.0), 1);
        assert_eq!(phase(35.0, 100.0), 2);
    }
}
