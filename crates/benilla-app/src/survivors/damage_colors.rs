//! The colours of outgoing damage numbers, by type and school, tunable while the game runs.
use super::spell_engine::Tags;
use super::units::{COLOR_HEAL, COLOR_HURT, COLOR_MELEE};
use bevy::prelude::*;
use serde::Deserialize;
use std::{collections::BTreeMap, path::Path, path::PathBuf};

const FILE: &str = include_str!("../../../../config/survivors-damage-colors.toml");
const LOCAL_FILE: &str = "survivors-damage-colors.toml";
const RELOAD_SECONDS: f32 = 1.0;
/// Lightning spells are also Nature and some spells are also Physical: most specific first.
const SCHOOLS: [(&str, Tags); 8] = [
    ("lightning", Tags::LIGHTNING),
    ("fire", Tags::FIRE),
    ("cold", Tags::COLD),
    ("arcane", Tags::ARCANE),
    ("holy", Tags::HOLY),
    ("shadow", Tags::SHADOW),
    ("nature", Tags::NATURE),
    ("physical", Tags::PHYSICAL),
];

/// The colour of a direct hit when no palette is loaded, as in a bare test world.
pub(super) const FALLBACK: u32 = COLOR_MELEE;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    critical: Option<String>,
    direct: BTreeMap<String, String>,
    dot: BTreeMap<String, String>,
}

struct Palette {
    default: u32,
    schools: [Option<u32>; SCHOOLS.len()],
}

impl Palette {
    fn parse(mut raw: BTreeMap<String, String>, table: &str) -> Result<Self, String> {
        let default = raw
            .remove("default")
            .ok_or_else(|| format!("[{table}] needs a default colour"))?;
        let mut schools = [None; SCHOOLS.len()];
        for (slot, (name, _)) in schools.iter_mut().zip(SCHOOLS) {
            *slot = raw.remove(name).map(|c| color(&c)).transpose()?;
        }
        if let Some(name) = raw.keys().next() {
            return Err(format!("[{table}] has an unknown school: {name}"));
        }
        Ok(Self {
            default: color(&default)?,
            schools,
        })
    }

    fn get(&self, tags: Tags) -> u32 {
        SCHOOLS
            .iter()
            .position(|(_, school)| tags.intersects(*school))
            .and_then(|i| self.schools[i])
            .unwrap_or(self.default)
    }
}

/// `RRGGBB`, with an optional `#`, as the 0xAARRGGBB of a floating number.
fn color(text: &str) -> Result<u32, String> {
    let hex = text.trim().trim_start_matches('#');
    let rgb = u32::from_str_radix(hex, 16)
        .ok()
        .filter(|_| hex.len() == 6)
        .ok_or_else(|| format!("invalid colour {text:?}, expected RRGGBB"))?;
    let argb = 0xFF00_0000 | rgb;
    // The number filters recognise healing and damage taken by these exact values.
    Ok(if argb == COLOR_HEAL || argb == COLOR_HURT {
        argb ^ 1
    } else {
        argb
    })
}

#[derive(Resource)]
pub(super) struct DamageColors {
    critical: Option<u32>,
    direct: Palette,
    dot: Palette,
    source: String,
    reload: f32,
}

impl Default for DamageColors {
    fn default() -> Self {
        Self::parse(FILE).expect("the bundled damage colours are valid")
    }
}

impl DamageColors {
    fn parse(text: &str) -> Result<Self, String> {
        let raw: Raw = toml::from_str(text).map_err(|e| e.to_string())?;
        Ok(Self {
            critical: raw.critical.as_deref().map(color).transpose()?,
            direct: Palette::parse(raw.direct, "direct")?,
            dot: Palette::parse(raw.dot, "dot")?,
            source: text.to_owned(),
            reload: 0.0,
        })
    }

    pub(super) fn direct(&self, tags: Tags, crit: bool) -> u32 {
        self.critical
            .filter(|_| crit)
            .unwrap_or_else(|| self.direct.get(tags))
    }

    pub(super) fn periodic(&self, tags: Tags) -> u32 {
        self.dot.get(tags)
    }
}

fn path() -> Option<PathBuf> {
    crate::local_state::home().map(|home| home.join(LOCAL_FILE))
}

fn load(mut colors: ResMut<DamageColors>) {
    let Some(path) = path() else {
        return;
    };
    if crate::local_state::read_to_string(&path).is_err() {
        if let Err(error) = crate::local_state::write_atomic(&path, FILE) {
            warn!("Could not write {}: {error}", path.display());
        }
    }
    reload(&mut colors, &path);
}

fn watch(time: Res<Time<Real>>, mut colors: ResMut<DamageColors>) {
    colors.reload -= time.delta_secs();
    if colors.reload > 0.0 {
        return;
    }
    colors.reload = RELOAD_SECONDS;
    if let Some(path) = path() {
        reload(&mut colors, &path);
    }
}

/// A broken edit keeps the last valid colours and is reported once, until the file changes.
fn reload(colors: &mut DamageColors, path: &Path) {
    let Ok(text) = crate::local_state::read_to_string(path) else {
        return;
    };
    if text == colors.source {
        return;
    }
    match DamageColors::parse(&text) {
        Ok(next) => {
            *colors = next;
            info!("Damage colours reloaded");
        }
        Err(error) => {
            warn!("Ignoring {}: {error}", path.display());
            colors.source = text;
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<DamageColors>()
        .add_systems(Startup, load)
        .add_systems(Update, watch);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_keep_hits_white_crits_yellow_and_dots_by_school() {
        let colors = DamageColors::default();
        assert_eq!(colors.direct(Tags::FIRE, false), 0xFFFF_FFFF);
        assert_eq!(colors.direct(Tags::FIRE, true), 0xFFFF_DE00);
        assert_eq!(colors.periodic(Tags::NATURE), 0xFF7C_DB4E);
        assert_eq!(colors.periodic(Tags::PHYSICAL), 0xFFE0_473C);
        assert_eq!(colors.periodic(Tags::FIRE), 0xFFFF_8C2A);
        assert_eq!(colors.periodic(Tags::SHADOW), 0xFFB4_6CFF);
        assert_eq!(colors.periodic(Tags::COLD), 0xFF7C_DB4E);
    }

    #[test]
    fn the_most_specific_school_wins_and_crits_can_keep_their_type() {
        let colors = DamageColors::parse(
            "[direct]\ndefault = \"#000001\"\nlightning = \"000002\"\nnature = \"000003\"\n\
             physical = \"000004\"\n[dot]\ndefault = \"000005\"\n",
        )
        .unwrap();
        assert_eq!(
            colors.direct(Tags::LIGHTNING | Tags::NATURE, true),
            0xFF00_0002
        );
        assert_eq!(
            colors.direct(Tags::NATURE | Tags::PHYSICAL, false),
            0xFF00_0003
        );
        assert_eq!(colors.direct(Tags::SPELL, false), 0xFF00_0001);
    }

    #[test]
    fn mistakes_are_rejected() {
        for text in [
            "[direct]\n[dot]\ndefault = \"FFFFFF\"\n",
            "[direct]\ndefault = \"FFF\"\n[dot]\ndefault = \"FFFFFF\"\n",
            "[direct]\ndefault = \"GGGGGG\"\n[dot]\ndefault = \"FFFFFF\"\n",
            "[direct]\ndefault = \"FFFFFF\"\nfrost = \"FFFFFF\"\n[dot]\ndefault = \"FFFFFF\"\n",
            "not toml",
        ] {
            assert!(DamageColors::parse(text).is_err(), "{text}");
        }
    }

    #[test]
    fn a_damage_colour_is_never_mistaken_for_healing_or_damage_taken() {
        assert_ne!(color("30FF30").unwrap(), COLOR_HEAL);
        assert_ne!(color("FF3030").unwrap(), COLOR_HURT);
    }
}
