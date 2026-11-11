//! A journey starting at level 60, gaining one monster level per campaign-map transition.
use super::*;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

const FILE: &str = include_str!("../../../../config/survivors-endless.toml");

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Config {
    version: u32,
    initial_seconds: f32,
    shorter_per_map: f32,
    minimum_seconds: f32,
    movement_per_map: f32,
    #[serde(default = "default_movement_cap")]
    maximum_movement_multiplier: f32,
    monster_stats_per_map: f32,
    boss_stats_per_map: f32,
    boss_cooldown_per_map: f32,
    minimum_boss_cooldown: f32,
    safe_time_per_map: f32,
    reward_per_map: f32,
    #[serde(default = "default_gold_per_map")]
    gold_per_map: f32,
    combat: expedition_config::Config,
}

impl Config {
    fn parse(text: &str) -> Result<Self, String> {
        let mut document: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;
        let combat = document
            .get_mut("combat")
            .and_then(toml::Value::as_table_mut)
            .ok_or("Endless requires a combat table")?;
        if combat.contains_key("zones") {
            return Err("Endless uses one shared combat.zone, not per-map combat.zones".into());
        }
        let zone = combat
            .remove("zone")
            .ok_or("Endless requires a shared combat.zone")?;
        // Expand only in memory for the expedition engine's campaign-map lookup.
        combat.insert(
            "zones".into(),
            toml::Value::Array(vec![zone; data::CAMPAIGN.len()]),
        );
        let mut value: Self = document
            .try_into()
            .map_err(|e: toml::de::Error| e.to_string())?;
        value.combat = expedition_config::Config::parse(
            &toml::to_string(&value.combat).map_err(|e| e.to_string())?,
        )?;
        if value.version != 1
            || ![
                value.initial_seconds,
                value.shorter_per_map,
                value.minimum_seconds,
                value.movement_per_map,
                value.monster_stats_per_map,
                value.boss_stats_per_map,
                value.minimum_boss_cooldown,
            ]
            .into_iter()
            .all(|v| v.is_finite() && v > 0.0)
            || value.minimum_seconds > value.initial_seconds
            || !value.maximum_movement_multiplier.is_finite()
            || value.maximum_movement_multiplier < 1.0
            || ![value.boss_cooldown_per_map, value.safe_time_per_map]
                .into_iter()
                .all(|v| v.is_finite() && v > 0.0 && v <= 1.0)
            || !value.reward_per_map.is_finite()
            || value.reward_per_map < 0.0
            || !value.gold_per_map.is_finite()
            || value.gold_per_map < 0.0
        {
            return Err("invalid Endless progression".into());
        }
        Ok(value)
    }
    pub fn duration(&self, index: u32) -> f32 {
        (self.initial_seconds - index as f32 * self.shorter_per_map).max(self.minimum_seconds)
    }
    pub fn movement(&self, index: u32) -> f32 {
        self.cap_movement(power(self.movement_per_map, index))
    }
    pub fn cap_movement(&self, multiplier: f32) -> f32 {
        multiplier.min(self.maximum_movement_multiplier)
    }
    pub fn stats(&self, index: u32, boss: bool) -> f32 {
        power(
            if boss {
                self.boss_stats_per_map
            } else {
                self.monster_stats_per_map
            },
            index,
        )
    }
    pub fn cooldown(&self, base: f32, index: u32) -> f32 {
        (base * power(self.boss_cooldown_per_map, index)).max(self.minimum_boss_cooldown)
    }
    pub fn safe_time(&self, base: f32, index: u32) -> f32 {
        (base * power(self.safe_time_per_map, index)).max(0.001)
    }
    pub fn reward(&self, index: u32) -> f32 {
        1.0 + index as f32 * self.reward_per_map
    }
    pub fn gold(&self, index: u32) -> f32 {
        1.0 + index as f32 * self.gold_per_map
    }
    pub fn expedition(&self, stage: Stage) -> expedition_config::Config {
        let mut config = self.combat.clone();
        config.endless = Some(stage);
        config
    }
}

fn default_movement_cap() -> f32 {
    5.0
}
fn default_gold_per_map() -> f32 { 0.6 }

fn power(base: f32, index: u32) -> f32 {
    // Keep very long journeys finite, including integer-to-float protocol conversion.
    base.powf(index as f32).clamp(1.0e-20, 1.0e12)
}

pub(super) fn config() -> &'static Config {
    static CONFIG: LazyLock<Config> = LazyLock::new(|| {
        let builtin = || Config::parse(FILE).expect("valid embedded Endless config");
        if cfg!(test) {
            return builtin();
        }
        crate::local_state::read_to_string(std::path::Path::new("config/survivors-endless.toml"))
            .ok()
            .and_then(|text| {
                Config::parse(&text)
                    .map_err(|error| {
                        warn!("Invalid Endless config: {error}; using embedded defaults");
                    })
                    .ok()
            })
            .unwrap_or_else(builtin)
    });
    &CONFIG
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub(super) struct Stage {
    pub index: u32,
    pub started: f32,
    pub boss_offset: u32,
    /// Reward boundaries prevent a later multiplier from repricing earlier maps' XP.
    pub xp_base: u64,
    pub xp_earned: u64,
    pub choices: Option<[usize; 2]>,
}
impl Stage {
    pub fn monster_level(self) -> u32 {
        60_u32.saturating_add(self.index)
    }
}

/// Enemy components retain static definitions. Share one immutable copy per used
/// actor/level, rather than allocating a definition for every spawned monster.
pub(super) fn leveled_actor(stage: Stage, def: &'static data::EnemyDef) -> &'static data::EnemyDef {
    type Actors = std::collections::HashMap<(usize, u32), &'static data::EnemyDef>;
    static ACTORS: LazyLock<std::sync::Mutex<Actors>> = LazyLock::new(Default::default);
    let level = stage.monster_level();
    if def.level == level { return def; }
    let mut actors = ACTORS.lock().unwrap_or_else(|error| error.into_inner());
    *actors.entry((def as *const data::EnemyDef as usize, level))
        .or_insert_with(|| Box::leak(Box::new(data::EnemyDef { level, ..*def })))
}

pub(super) fn begin(run: &mut Run) {
    begin_at(run, diagnostic_stage().unwrap_or(0));
}

fn begin_at(run: &mut Run, index: u32) {
    let stage = Stage { index, ..default() };
    run.endless = Some(stage);
    run.expeditions = config().expedition(stage);
}

// Explicitly opt in for interactive profiling; ordinary launches always start at map one.
fn diagnostic_stage() -> Option<u32> {
    if crate::webenv::var("WOW_PERF_DIAG").as_deref() != Some("1") {
        return None;
    }
    crate::webenv::var("WOW_SURVIVORS_ENDLESS_STAGE")?
        .parse::<u32>()
        .ok()
        .filter(|stage| (1..=100).contains(stage))
        .map(|stage| stage - 1)
}

#[allow(clippy::too_many_arguments)]
fn diagnostic_launch(
    mut step: Local<u8>,
    phase: Res<State<Phase>>,
    mut next: ResMut<NextState<Phase>>,
    mut roster: ResMut<characters::Roster>,
    mut selection: ResMut<Selection>,
    mut account: ResMut<talents::Account>,
    mut run: ResMut<Run>,
    loading: Res<crate::loading_screen::LoadingScreen>,
    hero: Query<(), With<hero::Hero>>,
) {
    if *step >= 2 || diagnostic_stage().is_none() {
        return;
    }
    if *step == 0 && *phase.get() == Phase::Menu {
        match characters::activate_selected(&mut roster, &mut selection, &mut account) {
            Ok(()) => {
                *step = 1;
                next.set(Phase::Hub);
            }
            Err(reason) => {
                warn!("Endless diagnostic launch: {reason}");
                *step = 2;
            }
        }
    } else if *step == 1 && *phase.get() == Phase::Hub && !loading.covering() && !hero.is_empty() {
        run.meta_level = talents::level(account.xp);
        selection.endless = true;
        if hud::start_run(&mut run, &mut selection) {
            // Pin the terrain for comparable interactive profiling runs.
            if let Some(map) = crate::webenv::var("WOW_SURVIVORS_DIAGNOSTIC_MAP")
                .and_then(|value| value.parse::<usize>().ok())
                .filter(|map| data::CAMPAIGN.contains(map))
            {
                selection.map = map;
                selection.stage = map;
                run.expedition_map = map;
            }
            info!(
                "Endless diagnostic: map {} on {}, duration {} s",
                run.endless.unwrap().index + 1,
                data::MAPS[selection.map].name,
                run.expeditions.duration()
            );
            next.set(Phase::Playing);
        } else {
            warn!("Endless diagnostic launch requires a level-60 character");
        }
        *step = 2;
    }
}

pub(super) fn roster(map: usize) -> &'static data::Roster {
    static ROSTERS: LazyLock<Vec<data::Roster>> = LazyLock::new(|| {
        (0..data::MAPS.len())
            .map(|map| {
                let base = data::roster(map);
                let actors = |defs: &[data::EnemyDef]| {
                    defs.iter()
                        .map(|d| data::EnemyDef { level: 60, ..*d })
                        .collect()
                };
                data::Roster {
                    enemies: actors(&base.enemies),
                    bosses: actors(&base.bosses),
                }
            })
            .collect()
    });
    &ROSTERS[map]
}

fn destinations(current: usize, rng: &mut Rng) -> [usize; 2] {
    let mut pool: Vec<_> = data::CAMPAIGN
        .iter()
        .copied()
        .filter(|&m| m != current)
        .collect();
    let first = pool.swap_remove(rng.index(pool.len()));
    [first, pool.swap_remove(rng.index(pool.len()))]
}

#[derive(Component)]
struct ChoiceRoot;
#[derive(Component)]
struct NextMap(usize);

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        diagnostic_launch.run_if(|| diagnostic_stage().is_some()),
    )
    .add_systems(OnEnter(Phase::GameOver), enemies::bank_endless_xp)
    .add_systems(
        Update,
        enemies::bank_endless_xp
            .after(enemies::drop_loot)
            .before(offer)
            .before(choose)
            .run_if(in_state(Phase::GameOver)),
    )
    .add_systems(OnExit(Phase::GameOver), close)
    .add_systems(
        Update,
        offer
            .after(runes::ui::RunePanel)
            .before(choose)
            .run_if(in_state(Phase::GameOver)),
    )
    .add_systems(
        Update,
        choose
            .after(items::collect_drops)
            .run_if(in_state(Phase::GameOver)),
    );
}

fn offer(
    mut commands: Commands,
    mut run: ResMut<Run>,
    mut rng: ResMut<Rng>,
    font: Option<Res<hud::HudFont>>,
    role: Option<Res<multiplayer::Role>>,
    runes: Option<Res<runes::Runes>>,
    roots: Query<Entity, With<ChoiceRoot>>,
) {
    if !run.won
        || run.endless.is_none()
        || !roots.is_empty()
        || runes.as_deref().is_some_and(runes::Runes::reward_active)
    {
        return;
    }
    let waiting = role.is_some_and(|r| *r == multiplayer::Role::Guest);
    let choices = run
        .endless
        .unwrap()
        .choices
        .unwrap_or_else(|| destinations(run.expedition_map, &mut rng));
    if !waiting {
        run.endless.as_mut().unwrap().choices = Some(choices);
    }
    let Some(font) = font else {
        return;
    };
    let stage = run.endless.unwrap();
    let next = stage.index.saturating_add(1);
    commands
        .spawn((
            ChoiceRoot,
            GlobalZIndex(900),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.85)),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    BackgroundColor(Color::srgb(0.035, 0.03, 0.025)),
                    BorderColor::all(Color::srgb(0.65, 0.49, 0.17)),
                    Node {
                        width: Val::Px(580.0),
                        max_width: Val::Percent(95.0),
                        padding: UiRect::all(Val::Px(24.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(18.0),
                        ..default()
                    },
                ))
                .with_children(|panel| {
                    panel.spawn(hud::centered_bundle(
                        &font.0,
                        format!("Endless - Map {} cleared", stage.index + 1),
                        24.0,
                        hud::GOLD_TONE,
                    ));
                    panel.spawn(hud::centered_bundle(
                        &font.0,
                        format!(
                            "Next: {:.0} min | Monsters ilvl {}\nGold +{:.0}% | XP +{:.0}%\nChoose your next expedition",
                            config().duration(next) / 60.0,
                            Stage { index: next, ..default() }.monster_level(),
                            (config().gold(next) - 1.0) * 100.0,
                            (config().reward(next) - 1.0) * 100.0
                        ),
                        14.0,
                        Color::WHITE,
                    ));
                    if waiting {
                        panel.spawn(hud::centered_bundle(
                            &font.0,
                            "Waiting for the host to choose the next map",
                            16.0,
                            Color::WHITE,
                        ));
                        return;
                    }
                    for map in choices {
                        panel
                            .spawn((
                                Button,
                                NextMap(map),
                                BackgroundColor(Color::srgb(0.12, 0.095, 0.055)),
                                BorderColor::all(Color::srgb(0.32, 0.26, 0.15)),
                                Node {
                                    width: Val::Percent(100.0),
                                    padding: UiRect::all(Val::Px(16.0)),
                                    border: UiRect::all(Val::Px(1.0)),
                                    justify_content: JustifyContent::Center,
                                    ..default()
                                },
                            ))
                            .with_children(|button| {
                                button.spawn(hud::centered_bundle(
                                    &font.0,
                                    data::MAPS[map].name,
                                    19.0,
                                    hud::GOLD_TONE,
                                ));
                            });
                    }
                    panel
                        .spawn((
                            Button,
                            characters::Click::Back,
                            Node {
                                padding: UiRect::all(Val::Px(10.0)),
                                justify_content: JustifyContent::Center,
                                ..default()
                            },
                            Leave,
                        ))
                        .with_children(|button| {
                            button.spawn(hud::centered_bundle(
                                &font.0,
                                "Finish journey and return to hub",
                                13.0,
                                Color::WHITE,
                            ));
                        });
                });
        });
}

#[derive(Component)]
struct Leave;

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn choose(
    mut commands: Commands,
    buttons: Query<(&Interaction, Option<&NextMap>, Has<Leave>), Changed<Interaction>>,
    mut run: ResMut<Run>,
    mut selection: ResMut<Selection>,
    mut next: ResMut<NextState<Phase>>,
    spawned: Query<
        Entity,
        (
            With<RunEntity>,
            Without<hero::Hero>,
            Without<multiplayer::RemoteOwner>,
        ),
    >,
    mut spawner: ResMut<enemies::Spawner>,
    mut clear: ResMut<enemies::ExpeditionClear>,
    mut pending: ResMut<abilities::PendingHits>,
    mut warnings: ResMut<boss_spells::Warnings>,
    mut companions: Option<ResMut<multiplayer::Companions>>,
    runes: Option<Res<runes::Runes>>,
) {
    if runes.as_deref().is_some_and(runes::Runes::reward_active) {
        return;
    }
    for (interaction, destination, leave) in &buttons {
        if *interaction != Interaction::Pressed || !run.won || run.endless.is_none() {
            continue;
        }
        if leave {
            next.set(Phase::Hub);
            return;
        }
        let Some(destination) = destination else {
            continue;
        };
        let Some(stage) = run.endless else {
            continue;
        };
        if !stage
            .choices
            .is_some_and(|choices| choices.contains(&destination.0))
        {
            continue;
        }
        for entity in &spawned {
            commands.entity(entity).despawn();
        }
        advance(&mut run, destination.0);
        if let Some(companions) = companions.as_deref_mut() {
            for player in companions.0.values_mut() {
                advance(&mut player.run, destination.0);
            }
        }
        selection.map = destination.0;
        selection.stage = destination.0;
        selection.version += 1;
        *spawner = default();
        *clear = default();
        pending.0.clear();
        pending.1.clear();
        warnings.0.clear();
        next.set(Phase::Playing);
        return;
    }
}

fn advance(run: &mut Run, map: usize) {
    let Some(old) = run.endless else {
        return;
    };
    let earned = talents::earned(run);
    run.won = false;
    let stage = Stage {
        index: old.index.saturating_add(1),
        started: run.elapsed,
        boss_offset: run.bosses,
        xp_base: talents::base_earned(run),
        xp_earned: earned,
        choices: None,
    };
    run.endless = Some(stage);
    run.expeditions = config().expedition(stage);
    run.expedition_map = map;
    run.rings = 0;
    run.recasts = default();
    run.proc_runtime = default();
    run.talent_runtime = default();
    talent_runtime::configure(run);
    run.banner = Some((format!("Endless - Map {}", stage.index + 1), 4.0));
    run.version += 1;
}

fn close(mut commands: Commands, roots: Query<Entity, With<ChoiceRoot>>) {
    for root in &roots {
        commands.entity(root).despawn();
    }
}

#[cfg(test)]
#[path = "endless_tests.rs"]
mod tests;
