//! The expedition clock shares the simulation's normalized boss schedule.
use super::*;

// Match the damage meter's diagnostic panels: warm black, bronze edges and ivory text.
const PANEL: Color = Color::srgb(0.035, 0.03, 0.025);
const TRACK: Color = Color::srgb(0.065, 0.055, 0.045);
const BORDER: Color = Color::srgb(0.65, 0.49, 0.17);
const INNER_BORDER: Color = Color::srgb(0.26, 0.22, 0.14);
const IVORY: Color = Color::srgb(0.88, 0.85, 0.75);
const UPCOMING: Color = Color::srgb(0.17, 0.14, 0.095);
const REACHED: Color = Color::srgb(0.48, 0.35, 0.13);
const PROGRESS: Color = Color::srgb(0.38, 0.26, 0.10);
const COMPLETE: Color = Color::srgb(0.50, 0.38, 0.15);

#[derive(Component)]
pub(in crate::survivors) struct Fill;

#[derive(Component)]
pub(in crate::survivors) struct Milestone(u32);

#[derive(Component)]
pub(in crate::survivors) struct TimeText;

#[derive(Component)]
pub(in crate::survivors) struct RewardRow;

#[derive(Component)]
pub(in crate::survivors) struct RewardText(&'static str);

pub(super) fn objective(progress: f32) -> String {
    if progress >= 1.0 {
        "Kill remaining monsters".into()
    } else {
        format!("Survive \u{00b7} {:.0}%", (progress * 100.0).floor())
    }
}

pub(super) fn spawn(col: &mut ChildSpawnerCommands, font: &Handle<Font>) {
    col.spawn((
        Node {
            width: Val::Px(360.0),
            padding: UiRect::axes(Val::Px(12.0), Val::Px(10.0)),
            margin: UiRect::bottom(Val::Px(6.0)),
            border: UiRect::all(Val::Px(1.0)),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            ..default()
        },
        BackgroundColor(PANEL),
        BorderColor::all(BORDER),
    ))
    .with_children(|panel| {
        panel
            .spawn(Node {
                width: Val::Percent(100.0),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|header| {
                header
                    .spawn(Node {
                        width: Val::Px(70.0),
                        flex_shrink: 0.0,
                        ..default()
                    })
                    .with_children(|left| {
                        left.spawn((LevelText, text(font, "Level 1", 13.0, GOLD)));
                    });
                header
                    .spawn(Node {
                        flex_grow: 1.0,
                        min_width: Val::Px(0.0),
                        justify_content: JustifyContent::Center,
                        ..default()
                    })
                    .with_children(|center| {
                        center.spawn((
                            ExpeditionObjective,
                            centered(font, "Survive \u{00b7} 0%", 14.0, IVORY),
                        ));
                    });
                header
                    .spawn(Node {
                        width: Val::Px(70.0),
                        flex_shrink: 0.0,
                        justify_content: JustifyContent::FlexEnd,
                        ..default()
                    })
                    .with_children(|right| {
                        right.spawn((KillText, text(font, "Kills 0", 13.0, IVORY)));
                    });
            });
        panel
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(26.0),
                    border: UiRect::all(Val::Px(1.0)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(TRACK),
                BorderColor::all(INNER_BORDER),
            ))
            .with_children(|bar| {
                bar.spawn((
                    Fill,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px(0.0),
                        width: Val::Percent(0.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(PROGRESS),
                ));
                for wave in 1..=data::BOSS_WAVES {
                    bar.spawn((
                        Milestone(wave),
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Percent(wave as f32 / data::BOSS_WAVES as f32 * 100.0),
                            width: Val::Px(1.0),
                            height: Val::Percent(100.0),
                            margin: UiRect::left(Val::Px(-1.0)),
                            ..default()
                        },
                        BackgroundColor(UPCOMING),
                    ));
                }
                // A separate overlay keeps the clock above both the fill and boss ticks.
                bar.spawn((
                    TimeText,
                    ZIndex(1),
                    text(font, "00:00 / 12:00", 14.0, IVORY),
                ));
            });
        panel
            .spawn((
                RewardRow,
                Node {
                    display: Display::None,
                    width: Val::Percent(100.0),
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                },
            ))
            .with_children(|row| {
                for (label, initial, align, color) in [
                    ("Gold", "Gold x1.00", JustifyContent::FlexStart, GOLD),
                    (
                        "Expeditions",
                        "Expeditions 0",
                        JustifyContent::Center,
                        IVORY,
                    ),
                    ("XP", "XP x1.00", JustifyContent::FlexEnd, IVORY),
                ] {
                    row.spawn(Node {
                        width: Val::Percent(100.0 / 3.0),
                        justify_content: align,
                        ..default()
                    })
                    .with_children(|cell| {
                        cell.spawn((RewardText(label), text(font, initial, 12.0, color)));
                    });
                }
            });
    });
}

pub(in crate::survivors) fn refresh(
    run: Res<Run>,
    mut fill: Query<(&mut Node, &mut BackgroundColor), With<Fill>>,
    mut markers: Query<(&Milestone, &mut BackgroundColor), Without<Fill>>,
    mut clock: Query<&mut Text, With<TimeText>>,
    mut rewards: Query<&mut Node, (With<RewardRow>, Without<Fill>)>,
    mut reward_text: Query<(&RewardText, &mut Text), Without<TimeText>>,
) {
    for mut row in &mut rewards {
        row.display = if run.endless.is_some() {
            Display::Flex
        } else {
            Display::None
        };
    }
    if let Some(stage) = run.endless {
        let completed = stage.index.saturating_add(u32::from(run.won));
        let multiplier = run.expeditions.reward_multiplier();
        for (label, mut text) in &mut reward_text {
            let value = if label.0 == "Expeditions" {
                format!("Expeditions {completed}")
            } else {
                format!("{} x{multiplier:.2}", label.0)
            };
            if **text != value {
                **text = value;
            }
        }
    }
    let progress = run.expeditions.progress(run.elapsed);
    if let Ok((mut node, mut color)) = fill.single_mut() {
        node.width = Val::Percent(progress * 100.0);
        color.0 = if progress >= 1.0 { COMPLETE } else { PROGRESS };
    }
    let due = run.expeditions.boss_waves_due(run.elapsed) - run.expeditions.boss_offset();
    for (milestone, mut color) in &mut markers {
        color.0 = if milestone.0 <= due {
            REACHED
        } else {
            UPCOMING
        };
    }
    if let Ok(mut text) = clock.single_mut() {
        let elapsed = run.expeditions.elapsed(run.elapsed) as u32;
        let duration = run.expeditions.duration() as u32;
        let value = format!(
            "{:02}:{:02} / {:02}:{:02}",
            elapsed / 60,
            elapsed % 60,
            duration / 60,
            duration % 60,
        );
        if **text != value {
            **text = value;
        }
    }
}
