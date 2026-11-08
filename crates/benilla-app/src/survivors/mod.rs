//! Warcraft Survivors: a top-down survival mode on the benilla world, started by
//! [`crate::run_survivors`]. The solo client calls an in-process server; its units are local
//! entities dressed by the same attach, animation and spell-visual systems a streamed unit gets,
//! on a battleground picked at the menu ([`data::MAPS`]), with the player controller and FrameXML
//! off ([`crate::run_mode::CaptureMode`]).

mod abilities;
mod arcane_repeats;
mod auto;
mod balance;
mod bank;
mod boss_spells;
mod cards;
mod card_draw;
mod characters;
mod client;
mod combat_ui;
mod currency;
mod damage_colors;
mod data;
mod diagnostics;
mod enemies;
mod enemy_visuals;
mod expedition_config;
mod extra_spells;
mod fill;
mod forms;
mod gambling;
mod codex;
mod player_auras;
mod grade;
mod healer_buffs;
mod healing;
mod health_plates;
mod draft_icons;
mod hero;
mod hub_ground;
mod hub_interact;
mod hud;
mod items;
mod level_stats;
mod menu;
mod minimap;
mod minions;
mod minor_talents;
mod mobility;
mod multiplayer;
mod options;
mod passives;
mod paragon;
mod procs;
mod progression;
mod ranged_enemies;
mod resistances;
mod revival;
mod revival_visuals;
mod reward_config;
mod runes;
mod server;
mod session;
mod shared_chest;
mod specialization;
mod special_mobs;
mod spell_engine;
mod starter_spells;
mod support_spells;
mod talent_design;
mod talent_effects;
mod talent_runtime;
mod talent_ui;
mod talents;
mod totem_limits;
mod training;
mod unit_frames;
mod units;
#[cfg(test)]
mod vanilla_spells_tests;

use benilla_world::schedule::WorldStage;
use bevy::prelude::*;

use data::{AbilityDef, ClassDef, PassiveDef, Stat};

/// Set by [`crate::run_survivors`] before the app builds. wasm32 has no process environment, so
/// the flag lives here; `WOW_SURVIVORS` (`?survivors=1` / `__wenilla_env.survivors` on the web)
/// still turns it on from outside.
pub(crate) static ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) fn active() -> bool {
    ACTIVE.load(std::sync::atomic::Ordering::Relaxed)
        || crate::webenv::var("WOW_SURVIVORS").is_some()
}

pub(crate) struct SurvivorsPlugin;

impl Plugin for SurvivorsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(crate::run_mode::CaptureMode)
            .init_state::<Phase>()
            .init_resource::<Selection>()
            .configure_sets(
                Update,
                (
                    session::Stage::Input,
                    session::Stage::Simulate,
                    session::Stage::Present,
                )
                    .chain()
                    .in_set(WorldStage::Input),
            )
            .add_plugins((server::plugin, client::plugin, multiplayer::plugin));
        if let Some(auto) = auto::from_env() {
            app.insert_resource(auto)
                .add_systems(Update, auto::drive.after(hud::keys))
                .add_systems(Update, auto::plan.before(WorldStage::Input));
        }
    }
}

/// The screen the mode is on.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(super) enum Phase {
    #[default]
    Menu,
    Hub,
    Playing,
    LevelUp,
    Paused,
    /// Paused inspection: character sheet, bags or talents (switching keeps this state).
    CharacterSheet,
    GameOver,
}

/// Everything spawned for a run, despawned when the next begins.
#[derive(Component)]
pub(super) struct RunEntity;

/// The class, look and battleground picked at the menu; the battleground holds for the run.
#[derive(Resource, Default)]
pub(super) struct Selection {
    pub test_character: bool,
    pub sheet_open: bool,
    pub bags_open: bool,
    pub ui_captured: bool,
    /// Non-modal panels consume camera/mouse gestures while keyboard movement continues.
    pub pointer_captured: bool,
    pub in_hub: bool,
    talents_open: bool,
    progress: progression::Progress,
    pub class: Option<usize>,
    /// `(display, race, sex)`, one per race and sex the class's trainers come in.
    pub looks: Vec<(u32, u8, u8)>,
    pub look: Option<usize>,
    /// An index into [`data::MAPS`].
    pub map: usize,
    /// The battleground the world is standing in: the last one entered (or the saved pick at
    /// boot). Picking another at the menu changes only `map`, so the menu hero stays put and no
    /// loading screen rises until Enter World.
    pub stage: usize,
    /// Bumped on every change, so the menu rebuilds once.
    pub version: u32,
}

impl Selection {
    pub fn allows_map(&self, map: usize) -> bool {
        if map == data::TRAINING_MAP {
            self.test_character
        } else {
            (self.test_character && data::CAMPAIGN.contains(&map)) || self.progress.allows(map)
        }
    }
    pub fn class_def(&self) -> Option<&'static ClassDef> {
        self.class.map(|i| &data::CLASSES[i])
    }

    pub fn map_def(&self) -> &'static data::MapDef {
        &data::MAPS[self.map.min(data::MAPS.len() - 1)]
    }

    pub fn stage_def(&self) -> &'static data::MapDef {
        if self.in_hub {
            return &data::HUB;
        }
        &data::MAPS[self.stage.min(data::MAPS.len() - 1)]
    }
}

use server::{Owned, Rng, Run, Stats};

#[cfg(test)]
use server::{pause, reset_run, unpause};
