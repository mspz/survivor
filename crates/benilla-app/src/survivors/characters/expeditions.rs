//! Destination browser: aligned campaign rows, a selection summary and a fixed action footer.
use super::*;
use bevy::ui::FocusPolicy;
const MUTED: Color = Color::srgb(0.62, 0.61, 0.57);
const EDGE: Color = Color::srgb(0.29, 0.25, 0.17);
#[derive(Resource)]
pub(in crate::survivors) struct View {
    pub scroll: f32,
    pub open: [bool; 3],
}
impl Default for View {
    fn default() -> Self {
        Self {
            scroll: 0.0,
            open: [true, false, false],
        }
    }
}
#[derive(Component)]
pub(super) struct GroupHeader(usize);
pub(super) fn toggle(
    phase: Res<State<Phase>>,
    roster: Res<Roster>,
    mut view: ResMut<View>,
    buttons: Query<(&Interaction, &GroupHeader), Changed<Interaction>>,
) {
    if *phase.get() != Phase::Hub || !roster.expeditions {
        return;
    }
    for (interaction, group) in &buttons {
        if *interaction == Interaction::Pressed {
            view.open[group.0] = !view.open[group.0];
            view.scroll = 0.0;
        }
    }
}
fn group(
    ui: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    index: usize,
    title: &str,
    open: bool,
) {
    ui.spawn((
        Button,
        GroupHeader(index),
        BackgroundColor(Color::srgb(0.12, 0.095, 0.055)),
        BorderColor::all(EDGE),
        Node {
            width: Val::Percent(100.0),
            min_height: Val::Px(40.0),
            flex_shrink: 0.0,
            padding: UiRect::all(Val::Px(10.0)),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
    ))
    .with_children(|header| {
        header.spawn(label(
            font,
            format!("{} {title}", if open { "[-]" } else { "[+]" }),
            16.0,
            hud::GOLD_TONE,
        ));
    });
}
#[derive(Component)]
pub(super) struct Destination {
    selected: bool,
}
#[derive(Component)]
pub(super) struct ModifierHint(usize);
#[derive(Component)]
pub(super) struct ModifierTooltip;
#[derive(Component)]
pub(super) struct ModifierPanel;

pub(super) fn modifier_tooltip(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    roster: Res<Roster>,
    selection: Res<Selection>,
    font: Option<Res<hud::HudFont>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    scale: Option<Res<UiScale>>,
    hints: Query<(&Interaction, &ModifierHint)>,
    roots: Query<Entity, With<ModifierTooltip>>,
) {
    for root in &roots {
        commands.entity(root).despawn();
    }
    if *phase.get() != Phase::Hub || !roster.expeditions || selection.endless || !data::dungeon(selection.map) {
        return;
    }
    let Some((_, hint)) = hints.iter().find(|(interaction, _)| **interaction != Interaction::None) else {
        return;
    };
    let Some(font) = font else { return; };
    let mods = selection.dungeon_mods.active(selection.map);
    let modifier = &super::super::dungeon_mods::DEFINITIONS[hint.0];
    let (viewport, cursor) = windows.single().map_or(
        (Vec2::new(1365.0, 768.0), Vec2::new(960.0, 300.0)),
        |window| {
            let scale = scale.as_deref().map_or(1.0, |s| s.0).max(0.01);
            (Vec2::new(window.width(), window.height()) / scale,
             window.cursor_position().unwrap_or(Vec2::ZERO) / scale)
        },
    );
    let width = 300.0_f32.min((viewport.x - 16.0).max(1.0));
    let x = if cursor.x > viewport.x * 0.5 { cursor.x - width - 16.0 } else { cursor.x + 16.0 };
    commands.spawn((
        ModifierTooltip,
        GlobalZIndex(800),
        FocusPolicy::Pass,
        BackgroundColor(Color::srgb(0.035, 0.03, 0.02)),
        BorderColor::all(hud::GOLD_TONE),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(x.clamp(8.0, (viewport.x - width - 8.0).max(8.0))),
            top: Val::Px((cursor.y + 14.0).clamp(8.0, (viewport.y - 116.0).max(8.0))),
            width: Val::Px(width),
            padding: UiRect::all(Val::Px(12.0)),
            border: UiRect::all(Val::Px(1.0)),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(5.0),
            ..default()
        },
    )).with_children(|tip| {
        tip.spawn(label(&font.0, modifier.name, 16.0, hud::GOLD_TONE));
        tip.spawn(label(&font.0, format!("{} per rank", mods.description(hint.0)), 13.0, Color::WHITE));
        tip.spawn(label(&font.0, format!("Rank {}/{} · +{} ilvl per rank · XP x{:.2} per rank",
            mods.rank(hint.0), super::super::dungeon_mods::MAX_RANK,
            mods.rank_bonus(hint.0), mods.rank_xp()), 12.0, MUTED));
    });
}
pub(super) fn hover(
    mut rows: Query<
        (
            &Interaction,
            &Destination,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        Changed<Interaction>,
    >,
) {
    for (interaction, row, mut background, mut border) in &mut rows {
        let hovered = *interaction != Interaction::None;
        background.0 = if row.selected {
            Color::srgb(0.19, 0.14, 0.065)
        } else if hovered {
            Color::srgb(0.12, 0.105, 0.065)
        } else {
            Color::srgb(0.065, 0.06, 0.045)
        };
        *border = BorderColor::all(if row.selected || hovered {
            hud::GOLD_TONE
        } else {
            EDGE
        });
    }
}

fn duration(index: usize) -> f32 {
    if data::dungeon(index) { super::super::dungeon_config::get(index).duration_seconds } else { data::CLEAR_SECS }
}
fn levels(index: usize) -> String {
    if index == data::TRAINING_MAP {
        return "Level 60 test area".into();
    }
    if data::dungeon(index) {
        let level = super::super::dungeon_config::get(index).item_level;
        return format!("Endgame 60  |  Loot ilvl {} (+{})", level, level.saturating_sub(60));
    }
    if let Some((low, high)) = data::campaign_levels(index) {
        return format!("Levels {low}-{high}");
    }
    let roster = data::roster(index);
    let low = roster.enemies.iter().map(|e| e.level).min().unwrap_or(1);
    let high = roster.enemies.iter().map(|e| e.level).max().unwrap_or(low);
    format!("Enemies: level {low}-{high}")
}
fn row(
    ui: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    selection: &Selection,
    index: usize,
    number: Option<usize>,
) {
    let map = &data::MAPS[index];
    let available = selection.allows_map(index);
    let selected = !selection.endless && available && selection.map == index;
    let mut entity = ui.spawn((
        Destination { selected },
        BorderColor::all(if selected { hud::GOLD_TONE } else { EDGE }),
        BackgroundColor(if selected {
            Color::srgb(0.19, 0.14, 0.065)
        } else {
            Color::srgb(0.065, 0.06, 0.045)
        }),
        Node {
            width: Val::Percent(100.0),
            min_height: Val::Px(66.0),
            flex_shrink: 0.0,
            padding: UiRect::all(Val::Px(10.0)),
            border: UiRect::all(Val::Px(1.0)),
            column_gap: Val::Px(10.0),
            align_items: AlignItems::Center,
            ..default()
        },
    ));
    if available {
        entity.insert((Button, Click::Map(index)));
    }
    entity.with_children(|item| {
        if let Some(number) = number {
            item.spawn((
                label(
                    font,
                    format!("{number:02}"),
                    14.0,
                    if selected { hud::GOLD_TONE } else { MUTED },
                ),
                Node {
                    width: Val::Px(23.0),
                    flex_shrink: 0.0,
                    ..default()
                },
            ));
        }
        item.spawn(Node {
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            min_width: Val::Px(0.0),
            row_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|body| {
            body.spawn(label(
                font,
                map.name,
                16.0,
                if selected {
                    hud::GOLD_TONE
                } else if available {
                    Color::WHITE
                } else {
                    MUTED
                },
            ));
            body.spawn(label(
                font,
                format!("{}  |  {}", map.zone, levels(index)),
                12.0,
                MUTED,
            ));
        });
        item.spawn((
            label(
                font,
                if selected {
                    "SELECTED"
                } else if available {
                    "OPEN"
                } else {
                    "LOCKED"
                },
                10.0,
                if selected {
                    hud::GOLD_TONE
                } else if available {
                    Color::srgb(0.52, 0.77, 0.5)
                } else {
                    MUTED
                },
            ),
            Node {
                flex_shrink: 0.0,
                ..default()
            },
        ));
    });
}
fn detail(
    ui: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    selection: &Selection,
    compact: bool,
) {
    if selection.endless {
        ui.spawn(label(font, "Endless Expeditions", 24.0, hud::GOLD_TONE));
        ui.spawn(label(font, "Level 60 | Random campaign maps", 14.0, Color::WHITE));
        ui.spawn(label(font, "Start at 6 minutes. Each cleared map offers two different destinations. Duration decreases by 1 minute per map, down to 2 minutes. Defeat every boss and monster to continue.", 14.0, MUTED));
        ui.spawn(label(font, "Per map change: monsters move 20% faster and gain 20% health/damage; bosses gain 40% health/damage. Boss cooldowns and warning times shrink by 20%. Gold and XP gain +30% per map (additive).", 13.0, MUTED));
        return;
    }
    let index = selection.map;
    let Some(map) = data::MAPS
        .get(index)
        .filter(|_| selection.allows_map(index))
    else {
        ui.spawn(label(
            font,
            "Select an available expedition",
            18.0,
            hud::GOLD_TONE,
        ));
        return;
    };
    if !compact {
        ui.spawn(label(font, "SELECTED DESTINATION", 11.0, MUTED));
    }
    ui.spawn(label(
        font,
        map.name,
        if compact { 20.0 } else { 26.0 },
        hud::GOLD_TONE,
    ));
    ui.spawn(label(font, map.zone, 14.0, Color::WHITE));
    if compact {
        ui.spawn(label(
            font,
            if index == data::TRAINING_MAP {
                format!("60 yd triangle / {} mobs / Boss: 3 sec / Mobs: 1 sec", super::super::training::PACK_SIZE)
            } else {
                format!(
                    "{}  |  Survive {} minutes, then clear",
                    levels(index),
                    duration(index) as u32 / 60
                )
            },
            12.0,
            MUTED,
        ));
        if data::dungeon(index) {
            let mods = selection.dungeon_mods.active(index);
            ui.spawn(label(font, format!("Loot ilvl {} (+{})  |  Character XP x{:.2}",
                super::super::dungeon_config::get(index).item_level + mods.bonus(),
                mods.bonus(), mods.xp_multiplier()), 12.0, hud::GOLD_TONE));
        }
        return;
    }
    ui.spawn((
        BackgroundColor(EDGE),
        Node {
            height: Val::Px(1.0),
            width: Val::Percent(100.0),
            margin: UiRect::vertical(Val::Px(8.0)),
            flex_shrink: 0.0,
            ..default()
        },
    ));
    ui.spawn(label(font, levels(index), 15.0, Color::WHITE));
    if data::dungeon(index) {
        let mods = selection.dungeon_mods.active(index);
        ui.spawn(label(font, format!("Loot ilvl {} (+{})  |  Character XP x{:.2}",
            super::super::dungeon_config::get(index).item_level + mods.bonus(),
            mods.bonus(), mods.xp_multiplier()), 13.0, hud::GOLD_TONE));
    }
    if index == data::TRAINING_MAP {
        ui.spawn(label(font, "60 yd triangle. No time limit.", 15.0, MUTED));
        for (heading, body) in [
            (
                "Immortal dummy",
                "Near the entrance. All damage is counted.",
            ),
            ("Boss", "60 yd from dummy. Respawn: 3 seconds."),
            ("Mob pack", "40 passive targets. Respawn: 1 second."),
        ] {
            ui.spawn((
                label(font, heading, 15.0, hud::GOLD_TONE),
                Node {
                    margin: UiRect::top(Val::Px(6.0)),
                    ..default()
                },
            ));
            ui.spawn(label(font, body, 13.0, MUTED));
        }
        ui.spawn(label(
            font,
            "Passive targets. No attacks or movement.",
            13.0,
            MUTED,
        ));
    } else {
        ui.spawn(label(font, "OBJECTIVE", 11.0, MUTED));
        ui.spawn(label(
            font,
            format!("Survive {} minutes, then kill every boss and monster", duration(index) as u32 / 60),
            19.0,
            Color::WHITE,
        ));
        ui.spawn(label(
            font,
            if data::dungeon(index) {
                format!("Defeat every boss wave. Health phases: {:.0}% and {:.0}%. Minimum duration: {:.1} minutes.",
                    super::super::dungeon_config::get(index).spells.phase_thresholds[0]*100.0,
                    super::super::dungeon_config::get(index).spells.phase_thresholds[1]*100.0, duration(index)/60.0)
            } else {
                "Fight growing waves of enemies and defeat the bosses along the way.".to_owned()
            },
            14.0,
            MUTED,
        ));
        ui.spawn((
            label(font, "VICTORY", 11.0, MUTED),
            Node {
                margin: UiRect::top(Val::Px(12.0)),
                ..default()
            },
        ));
        ui.spawn(label(font, map.victory, 15.0, hud::GOLD_TONE));
        ui.spawn(label(
            font,
            if selection.test_character {
                "All campaign destinations are unlocked for this test character."
            } else {
                "Complete this expedition to unlock the next destination."
            },
            13.0,
            MUTED,
        ));
    }
}

fn modifiers(ui: &mut ChildSpawnerCommands, font: &Handle<Font>, selection: &Selection) {
    let mods = selection.dungeon_mods.active(selection.map);
    let unlocked = selection.allows_map(selection.map);
    for (index, modifier) in super::super::dungeon_mods::DEFINITIONS.iter().enumerate() {
        ui.spawn((
            BackgroundColor(Color::srgb(0.08, 0.065, 0.045)),
            Node { width: Val::Percent(100.0), min_height: Val::Px(36.0), flex_shrink: 0.0,
                align_items: AlignItems::Center, column_gap: Val::Px(5.0),
                padding: UiRect::horizontal(Val::Px(7.0)), ..default() },
        )).with_children(|row| {
            row.spawn((
                ModifierHint(index), Interaction::default(),
                Node { flex_grow: 1.0, min_width: Val::Px(0.0), height: Val::Px(36.0),
                    align_items: AlignItems::Center, ..default() },
            )).with_children(|name| {
                name.spawn(label(font, modifier.name, 12.0, Color::WHITE));
            });
            modifier_button(row, font, index, false, "−", unlocked && mods.rank(index) > 0);
            row.spawn((label(font, mods.rank(index).to_string(), 13.0, hud::GOLD_TONE),
                Node { width: Val::Px(15.0), justify_content: JustifyContent::Center, ..default() }));
            modifier_button(row, font, index, true, "+",
                unlocked && mods.rank(index) < super::super::dungeon_mods::MAX_RANK);
        });
    }
}

fn modifier_button(ui: &mut ChildSpawnerCommands, font: &Handle<Font>, index: usize,
    increase: bool, title: &str, enabled: bool) {
    let mut button = ui.spawn((
        BackgroundColor(Color::srgb(0.16, 0.12, 0.06)),
        BorderColor::all(EDGE),
        Node { width: Val::Px(28.0), height: Val::Px(28.0), border: UiRect::all(Val::Px(1.0)),
            align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() },
    ));
    if enabled { button.insert((Button, Click::DungeonMod(index, increase))); }
    button.with_children(|button| {
        button.spawn(label(font, title, 16.0, if enabled { hud::GOLD_TONE } else { MUTED }));
    });
}
pub(super) fn draw(
    commands: &mut Commands,
    font: &Handle<Font>,
    selection: &Selection,
    viewport: Vec2,
    scroll: f32,
    open: [bool; 3],
    endless_available: bool,
) {
    let show_modifiers = !selection.endless && data::dungeon(selection.map);
    let beside = show_modifiers && viewport.x >= 720.0;
    let side_width = 280.0;
    let side_height = 326.0;
    let width = (viewport.x - 32.0 - if beside { side_width + 12.0 } else { 0.0 })
        .clamp(240.0, 980.0);
    let height = (viewport.y - 32.0 - if show_modifiers && !beside { side_height + 12.0 } else { 0.0 })
        .clamp(320.0, 650.0);
    let compact = width < 720.0;
    commands
        .spawn((
            Root,
            Button,
            GlobalZIndex(470),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.78)),
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
                .spawn(Node {
                    flex_direction: if beside { FlexDirection::Row } else { FlexDirection::Column },
                    align_items: AlignItems::FlexStart,
                    column_gap: Val::Px(12.0), row_gap: Val::Px(12.0),
                    ..default()
                })
                .with_children(|windows| {
            windows
                .spawn((
                    Node {
                        width: Val::Px(width),
                        height: Val::Px(height),
                        padding: UiRect::all(Val::Px(20.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(14.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.035, 0.03, 0.02)),
                ))
                .with_children(|panel| {
                    skin::backdrop(panel);
                    skin::close(panel, Click::Back, width - 44.0, 12.0);
                    panel
                        .spawn(Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(4.0),
                            flex_shrink: 0.0,
                            padding: UiRect::right(Val::Px(30.0)),
                            ..default()
                        })
                        .with_children(|header| {
                            header.spawn(label(font, "Expeditions", 26.0, hud::GOLD_TONE));
                            header.spawn(label(font, "Choose your next destination", 13.0, MUTED));
                        });
                    panel
                        .spawn(Node {
                            flex_direction: if compact {
                                FlexDirection::Column
                            } else {
                                FlexDirection::Row
                            },
                            flex_grow: 1.0,
                            min_height: Val::Px(0.0),
                            column_gap: Val::Px(20.0),
                            row_gap: Val::Px(12.0),
                            ..default()
                        })
                        .with_children(|body| {
                            body.spawn(Node {
                                width: if compact {
                                    Val::Percent(100.0)
                                } else {
                                    Val::Px(410.0)
                                },
                                flex_grow: 1.0,
                                flex_direction: FlexDirection::Column,
                                min_height: Val::Px(0.0),
                                min_width: Val::Px(0.0),
                                row_gap: Val::Px(7.0),
                                ..default()
                            })
                            .with_children(|left| {
                                left.spawn((
                                    Scroll,
                                    Interaction::default(),
                                    ScrollPosition(Vec2::new(0.0, scroll)),
                                    Node {
                                        flex_grow: 1.0,
                                        min_height: Val::Px(0.0),
                                        overflow: Overflow::scroll_y(),
                                        flex_direction: FlexDirection::Column,
                                        row_gap: Val::Px(6.0),
                                        padding: UiRect::right(Val::Px(6.0)),
                                        ..default()
                                    },
                                ))
                                .with_children(|list| {
                                    group(list, font, 0, "Expeditions", open[0]);
                                    if open[0] {
                                        if selection.test_character {
                                            list.spawn(label(font, "PRACTICE", 11.0, MUTED));
                                            row(list, font, selection, data::TRAINING_MAP, None);
                                        }
                                        list.spawn((
                                            label(font, "CAMPAIGN", 11.0, MUTED),
                                            Node {
                                                margin: UiRect::top(Val::Px(
                                                    if selection.test_character {
                                                        12.0
                                                    } else {
                                                        0.0
                                                    },
                                                )),
                                                ..default()
                                            },
                                        ));
                                        for (rank, index) in
                                            progression::zone_order().into_iter().enumerate()
                                        {
                                            row(list, font, selection, index, Some(rank + 1));
                                        }
                                        let mut endless = list.spawn((
                                            BackgroundColor(if selection.endless { Color::srgb(0.19, 0.14, 0.065) } else { Color::srgb(0.065, 0.06, 0.045) }),
                                            BorderColor::all(if selection.endless { hud::GOLD_TONE } else { EDGE }),
                                            Node { width: Val::Percent(100.0), padding: UiRect::all(Val::Px(12.0)), border: UiRect::all(Val::Px(1.0)), flex_shrink: 0.0, flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() },
                                        ));
                                        if endless_available { endless.insert((Button, Click::Endless)); }
                                        endless.with_children(|row| {
                                            row.spawn(label(font, "Endless Expeditions", 16.0, if endless_available { hud::GOLD_TONE } else { MUTED }));
                                            row.spawn(label(font, if endless_available { "Level 60 | 6 min start | Endless progression" } else { "Requires character level 60" }, 12.0, MUTED));
                                        });
                                    }
                                    group(list, font, 1, "Donjons", open[1]);
                                    if open[1] {
                                        for (rank, &index) in data::DUNGEONS.iter().enumerate() {
                                            row(list, font, selection, index, Some(rank + 1));
                                        }
                                    }
                                    group(list, font, 2, "Raids", open[2]);
                                    if open[2] {
                                        list.spawn(label(
                                            font,
                                            "No raids available.",
                                            13.0,
                                            MUTED,
                                        ));
                                    }
                                });
                                left.spawn(label(
                                    font,
                                    "Complete all expeditions to unlock every dungeon.",
                                    11.0,
                                    MUTED,
                                ));
                            });
                            body.spawn((
                                BorderColor::all(EDGE),
                                BackgroundColor(Color::srgb(0.06, 0.05, 0.035)),
                                Node {
                                    width: if compact {
                                        Val::Percent(100.0)
                                    } else {
                                        Val::Px(350.0)
                                    },
                                    height: if compact {
                                        Val::Px(112.0)
                                    } else {
                                        Val::Percent(100.0)
                                    },
                                    flex_shrink: 0.0,
                                    padding: UiRect::all(Val::Px(14.0)),
                                    border: UiRect::all(Val::Px(1.0)),
                                    flex_direction: FlexDirection::Column,
                                    row_gap: Val::Px(7.0),
                                    overflow: Overflow::clip(),
                                    ..default()
                                },
                            ))
                            .with_children(|right| {
                                detail(right, font, selection, compact || height < 610.0)
                            });
                        });
                    panel
                        .spawn(Node {
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            flex_shrink: 0.0,
                            column_gap: Val::Px(12.0),
                            ..default()
                        })
                        .with_children(|footer| {
                            skin::button_sized(
                                footer,
                                font,
                                "Back",
                                Click::Back,
                                if width < 500.0 { 80.0 } else { 120.0 },
                                40.0,
                                14.0,
                                false,
                            );
                            if selection.endless || selection.allows_map(selection.map) {
                                skin::button_sized(
                                    footer,
                                    font,
                                    if width < 500.0 {
                                        "Start"
                                    } else {
                                        "Start expedition"
                                    },
                                    Click::Launch,
                                    if width < 500.0 { width - 132.0 } else { 220.0 },
                                    40.0,
                                    16.0,
                                    true,
                                );
                            } else {
                                footer.spawn(label(
                                    font,
                                    "Choose an available expedition",
                                    13.0,
                                    MUTED,
                                ));
                            }
                        });
                });
            if show_modifiers {
                windows.spawn((
                    ModifierPanel,
                    BackgroundColor(Color::srgb(0.035, 0.03, 0.02)),
                    Node { width: Val::Px(if beside { side_width } else { width }),
                        height: Val::Px(side_height), padding: UiRect::all(Val::Px(16.0)),
                        flex_direction: FlexDirection::Column, row_gap: Val::Px(7.0), ..default() },
                )).with_children(|side| {
                    skin::backdrop(side);
                    side.spawn(label(font, "Dungeon modifiers", 17.0, hud::GOLD_TONE));
                    modifiers(side, font, selection);
                });
            }
                });
        });
}
