//! Classic instance entries and CreatureDisplayInfo IDs from MaNGOS Zero's world tables.
//! Sources: github.com/mangoszero/database, World/Setup/FullDB/{areatrigger_teleport,creature_template}.sql.
use super::{MapDef, foe, giant};

// Append-only destination IDs: 0..16, including Training Grounds, belong to existing saves.
pub(crate) const DUNGEONS: &[usize] = &[17, 18, 19, 20, 21, 22];
#[cfg(test)]
pub(crate) const DUNGEON_ITEM_LEVEL: u32 = 65;
pub(crate) fn dungeon(map: usize) -> bool {
    DUNGEONS.contains(&map)
}
pub(crate) fn dungeon_cleared(map: usize, spawned: u32, living_bosses: usize) -> bool {
    !dungeon(map) || (spawned as usize >= super::roster(map).bosses.len() && living_bosses == 0)
}

pub(super) const BRD: MapDef = MapDef {
    name: "Blackrock Depths",
    zone: "Blackrock Mountain — Dungeon",
    map: 230,
    center: [456.929, 34.0923, -69.5617],
    leash: 60.0,
    victory: "Emperor Dagran Thaurissan's reign is over.",
    foes: &[
        foe("Anvilrage Overseer", 8754, 60, 3.8, 1.0),
        foe("Anvilrage Warden", 8758, 60, 3.8, 1.0),
        foe("Anvilrage Guardsman", 8750, 60, 4.0, 1.0),
        foe("Anvilrage Footman", 8718, 60, 4.2, 1.0),
        foe("Anvilrage Soldier", 8757, 60, 4.2, 1.0),
        foe("Anvilrage Medic", 8752, 60, 3.8, 1.0),
        foe("Anvilrage Officer", 8753, 60, 4.4, 1.0),
        foe("Anvilrage Marshal", 8751, 60, 4.4, 1.0),
        foe("Anvilrage Captain", 8749, 60, 4.6, 1.0),
    ],
    bosses: &[
        foe("High Interrogator Gerstahn", 8761, 63, 4.0, 1.3),
        foe("Pyromancer Loregrain", 8762, 63, 4.0, 1.3),
        foe("General Angerforge", 8756, 63, 4.2, 1.4),
        foe("Golem Lord Argelmach", 8759, 63, 4.2, 1.4),
        foe("Hurley Blackbreath", 8658, 63, 4.4, 1.4),
        foe("Emperor Dagran Thaurissan", 8807, 63, 4.6, 1.5),
    ],
};
pub(super) const LBRS: MapDef = MapDef {
    name: "Lower Blackrock Spire",
    zone: "Blackrock Mountain — Dungeon",
    map: 229,
    center: [78.3534, -226.841, 49.7662],
    leash: 60.0,
    victory: "Overlord Wyrmthalak's forces are broken.",
    foes: &[
        foe("Spirestone Enforcer", 11577, 60, 3.8, 1.0),
        foe("Spirestone Reaver", 11581, 60, 4.0, 1.0),
        foe("Spirestone Ogre Magus", 11562, 60, 3.8, 1.0),
        foe("Smolderthorn Mystic", 9709, 60, 3.8, 1.0),
        foe("Smolderthorn Shadow Priest", 9728, 60, 3.8, 1.0),
        foe("Smolderthorn Headhunter", 9682, 60, 4.4, 1.0),
        foe("Smolderthorn Berserker", 7803, 60, 4.8, 1.0),
        foe("Spirestone Battle Lord", 11576, 60, 4.4, 1.0),
    ],
    bosses: &[
        giant("Highlord Omokk", 11565, 63, 4.0, 1.2),
        foe("Shadow Hunter Vosh'gajin", 9732, 63, 4.4, 1.4),
        foe("War Master Voone", 9733, 63, 4.4, 1.4),
        giant("Mother Smolderweb", 9929, 63, 4.6, 1.2),
        giant("Spirestone Butcher", 11574, 63, 4.2, 1.2),
        giant("Overlord Wyrmthalak", 8711, 63, 4.6, 1.2),
    ],
};
pub(super) const DIRE_MAUL_EAST: MapDef = MapDef {
    name: "Dire Maul East",
    zone: "Feralas — Dungeon",
    map: 429,
    center: [47.4501, -153.665, -2.71439],
    leash: 60.0,
    victory: "Alzzin the Wildshaper's corruption is cleansed.",
    foes: &[
        foe("Warpwood Treant", 10621, 60, 3.8, 1.0),
        foe("Warpwood Tangler", 9592, 60, 3.8, 1.0),
        foe("Wildspawn Satyr", 11345, 60, 4.2, 1.0),
        foe("Wildspawn Rogue", 2021, 60, 4.6, 1.0),
        foe("Wildspawn Trickster", 2020, 60, 4.4, 1.0),
        foe("Wildspawn Betrayer", 10032, 60, 4.4, 1.0),
        foe("Wildspawn Hellcaller", 11340, 60, 3.8, 1.0),
        giant("Warpwood Crusher", 12929, 60, 4.0, 1.0),
    ],
    bosses: &[
        foe("Pusillin", 7552, 63, 4.8, 1.5),
        foe("Lethtendris", 14378, 63, 4.2, 1.4),
        giant("Hydrospawn", 5489, 63, 4.4, 1.2),
        foe("Zevrim Thornhoof", 11335, 63, 4.4, 1.4),
        giant("Alzzin the Wildshaper", 14416, 63, 4.6, 1.2),
    ],
};
pub(super) const DIRE_MAUL_WEST: MapDef = MapDef {
    name: "Dire Maul West",
    zone: "Feralas — Dungeon",
    map: 429,
    center: [-65.6559, 159.561, -3.4647],
    leash: 60.0,
    victory: "Prince Tortheldrin's prison is shattered.",
    foes: &[
        foe("Eldreth Apparition", 14365, 60, 4.0, 1.0),
        foe("Eldreth Seether", 11213, 60, 4.4, 1.0),
        foe("Eldreth Sorcerer", 11205, 60, 3.8, 1.0),
        foe("Mana Remnant", 14272, 60, 4.2, 1.0),
        giant("Warpwood Guardian", 13173, 60, 3.8, 1.0),
        giant("Warpwood Stomper", 6350, 60, 4.0, 1.0),
        giant("Residual Monstrosity", 14254, 60, 4.2, 1.0),
        giant("Ironbark Protector", 13489, 60, 4.2, 1.0),
    ],
    bosses: &[
        giant("Tendris Warpwood", 14383, 63, 4.0, 1.2),
        foe("Magister Kalendris", 14384, 63, 4.2, 1.4),
        foe("Illyanna Ravenoak", 11270, 63, 4.4, 1.4),
        giant("Immol'thar", 14173, 63, 4.4, 1.0),
        foe("Prince Tortheldrin", 11256, 63, 4.6, 1.5),
    ],
};
pub(super) const SCHOLOMANCE: MapDef = MapDef {
    name: "Scholomance",
    zone: "Western Plaguelands — Dungeon",
    map: 289,
    center: [190.819, 126.329, 137.227],
    leash: 60.0,
    victory: "Darkmaster Gandling's school of necromancy falls.",
    foes: &[
        foe("Scholomance Neophyte", 11164, 60, 3.8, 1.0),
        foe("Scholomance Adept", 11161, 60, 3.8, 1.0),
        foe("Scholomance Acolyte", 11157, 60, 4.0, 1.0),
        foe("Scholomance Necromancer", 11163, 60, 3.8, 1.0),
        foe("Risen Warrior", 7847, 60, 4.4, 1.0),
        foe("Risen Protector", 11489, 60, 4.2, 1.0),
        giant("Risen Construct", 12074, 60, 4.0, 1.0),
        foe("Risen Bonewarder", 11397, 60, 4.4, 1.0),
    ],
    bosses: &[
        giant("Kirtonos the Herald", 7534, 63, 4.2, 1.2),
        foe("Jandice Barov", 11073, 63, 4.2, 1.4),
        giant("Rattlegore", 12073, 63, 4.2, 1.2),
        foe("Ras Frostwhisper", 7919, 63, 4.2, 1.4),
        foe("Vectus", 2606, 63, 4.4, 1.4),
        foe("Darkmaster Gandling", 11070, 63, 4.6, 1.5),
    ],
};
pub(super) const STRATHOLME: MapDef = MapDef {
    name: "Stratholme — Undead",
    zone: "Eastern Plaguelands — Dungeon",
    map: 329,
    center: [3590.87, -3643.22, 138.491],
    leash: 60.0,
    victory: "Baron Rivendare's undead army is defeated.",
    foes: &[
        foe("Plague Ghoul", 559, 60, 4.2, 1.0),
        foe("Ghoul Ravener", 10626, 60, 4.6, 1.0),
        foe("Thuzadin Shadowcaster", 10631, 60, 3.8, 1.0),
        foe("Thuzadin Necromancer", 10539, 60, 3.8, 1.0),
        foe("Shrieking Banshee", 10728, 60, 4.6, 1.0),
        foe("Crypt Crawler", 6841, 60, 4.4, 1.0),
        giant("Patchwork Horror", 1693, 60, 3.8, 1.0),
        giant("Venom Belcher", 12819, 60, 4.0, 1.0),
    ],
    bosses: &[
        foe("Baroness Anastari", 10698, 63, 4.4, 1.5),
        giant("Nerub'enkan", 9793, 63, 4.2, 1.2),
        foe("Maleki the Pallid", 10546, 63, 4.2, 1.4),
        giant("Ramstein the Gorger", 12818, 63, 4.2, 1.2),
        foe("Baron Rivendare", 10729, 63, 4.6, 1.2),
    ],
};

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn six_instanced_dungeons_have_native_rosters_and_finishable_boss_waves() {
        assert_eq!(super::super::TRAINING_MAP, 16);
        assert_eq!(super::super::MAPS[16].name, "Training Grounds");
        for &map in DUNGEONS {
            let destination = &super::super::MAPS[map];
            assert!(matches!(destination.map, 229 | 230 | 289 | 329 | 429));
            let roster = super::super::roster(map);
            assert!(roster.enemies.len() >= 8);
            assert!(
                roster
                    .enemies
                    .iter()
                    .all(|e| e.level == 60 && e.display > 0)
            );
            assert!((5..=6).contains(&roster.bosses.len()));
            assert!(roster.bosses.iter().all(|e| e.level == 63 && e.display > 0));
            assert!(
                roster.bosses.len() as f32 * super::super::BOSS_EVERY_SECS
                    < super::super::VICTORY_SECS
            );
            assert!(!dungeon_cleared(map, 0, 0));
            assert!(!dungeon_cleared(map, roster.bosses.len() as u32, 1));
            assert!(dungeon_cleared(map, roster.bosses.len() as u32, 0));
        }
    }
}
