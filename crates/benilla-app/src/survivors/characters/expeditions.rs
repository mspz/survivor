//! Destination browser: aligned campaign rows, a selection summary and a fixed action footer.
use super::*;
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

fn levels(index: usize) -> String {
    if index == data::TRAINING_MAP {
        return "Level 60 test area".into();
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
    let selected = available && selection.map == index;
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
                "60 yd triangle / 40 mobs / Boss: 3 sec / Mobs: 1 sec".into()
            } else {
                format!(
                    "{}  |  Survive {} minutes",
                    levels(index),
                    data::VICTORY_SECS as u32 / 60
                )
            },
            12.0,
            MUTED,
        ));
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
            format!("Survive {} minutes", data::VICTORY_SECS as u32 / 60),
            19.0,
            Color::WHITE,
        ));
        ui.spawn(label(
            font,
            "Fight growing waves of enemies and defeat the bosses along the way.",
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
pub(super) fn draw(
    commands: &mut Commands,
    font: &Handle<Font>,
    selection: &Selection,
    viewport: Vec2,
    scroll: f32,
    open: [bool; 3],
) {
    let width = (viewport.x - 32.0).clamp(240.0, 980.0);
    let height = (viewport.y - 32.0).clamp(300.0, 650.0);
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
                                    group(list, font, 0, "Expéditions", open[0]);
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
                                    }
                                    group(list, font, 1, "Donjons", open[1]);
                                    if open[1] {
                                        list.spawn(label(
                                            font,
                                            "Aucun donjon disponible.",
                                            13.0,
                                            MUTED,
                                        ));
                                    }
                                    group(list, font, 2, "Raids", open[2]);
                                    if open[2] {
                                        list.spawn(label(
                                            font,
                                            "Aucun raid disponible.",
                                            13.0,
                                            MUTED,
                                        ));
                                    }
                                });
                                left.spawn(label(
                                    font,
                                    "Scroll to browse. Complete expeditions in order.",
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
                            if selection.allows_map(selection.map) {
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
        });
}
