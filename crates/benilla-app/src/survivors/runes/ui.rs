//! The rune panel: the dropped rune and the account's slots, shown at the results after a
//! victory and in the hub on R. Rebuilt whenever the runes or the message change.
use super::{parse_color, Rune, RuneAction, Runes};
use crate::survivors::{combat_ui::skin, hud, Phase, Rng, Selection};
use bevy::prelude::*;

const EDGE: Color = Color::srgb(0.55, 0.45, 0.25);
const EQUIPPED: Color = Color::srgb(0.25, 0.9, 0.35);

#[derive(Component)]
struct PanelRoot;

fn spawn_panel(mut commands: Commands) {
    commands.spawn((
        PanelRoot,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: Val::Px(8.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.82)),
        GlobalZIndex(460),
        Visibility::Hidden,
    ));
}

fn visible(runes: &Runes, phase: Phase, training: bool) -> bool {
    match phase {
        Phase::GameOver => runes.save.pending.is_some() || !runes.message.is_empty(),
        Phase::Hub => runes.open,
        // The training ground is the rune test bench.
        Phase::Playing => training && runes.open,
        _ => false,
    }
}

fn rarity_color(runes: &Runes, rune: &Rune) -> Color {
    parse_color(&runes.config.rarity(rune.rarity).color).unwrap_or(Color::WHITE)
}

/// Hovering a rune icon lists every stat its rarity can have and the roll ranges.
#[derive(Component, Clone, Copy)]
pub(in crate::survivors) struct RuneTip(pub u8);

#[derive(Component)]
struct TipRoot;

/// Upgrade bonuses, shown in yellow after the roll.
const BONUS: Color = Color::srgb(1.0, 0.82, 0.0);

/// The rune's client icon in a frame of its rarity colour.
pub(in crate::survivors) fn rune_icon(ui: &mut ChildSpawnerCommands, runes: &Runes, rune: &Rune) {
    ui.spawn((
        Node {
            width: Val::Px(44.0),
            height: Val::Px(44.0),
            border: UiRect::all(Val::Px(2.0)),
            flex_shrink: 0.0,
            ..default()
        },
        BorderColor::all(rarity_color(runes, rune)),
        Interaction::default(),
        RuneTip(rune.rarity),
    ))
    .with_children(|frame| {
        frame.spawn((
            ImageNode::default(),
            hud::UiArt::item(&runes.config.rarity(rune.rarity).icon),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
        ));
    });
}

/// "+20 +2% Damage": the roll in white, the upgrade bonus in yellow, then the label.
pub(in crate::survivors) fn stat_text(
    ui: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    runes: &Runes,
    stat: &super::RuneStat,
    size: f32,
) {
    let (roll, bonus, label) = runes.stat_parts(stat);
    let span = |s: String, color: Color| {
        (
            TextSpan::new(s),
            TextFont {
                font: font.clone(),
                font_size: size,
                ..default()
            },
            TextColor(color),
        )
    };
    ui.spawn((
        hud::centered_bundle(font, roll, size, Color::WHITE),
        children![span(bonus, BONUS), span(label, Color::WHITE)],
    ));
}

pub(in crate::survivors) fn rune_lines(
    ui: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    runes: &Runes,
    rune: &Rune,
) {
    rune_icon(ui, runes, rune);
    ui.spawn(hud::centered_bundle(
        font,
        runes.name(rune),
        14.0,
        rarity_color(runes, rune),
    ));
    for stat in &rune.stats {
        stat_text(ui, font, runes, stat, 11.0);
    }
}

fn card(ui: &mut ChildSpawnerCommands, edge: Color, body: impl FnOnce(&mut ChildSpawnerCommands)) {
    ui.spawn((
        Node {
            width: Val::Px(200.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: UiRect::all(Val::Px(10.0)),
            row_gap: Val::Px(3.0),
            border: UiRect::all(Val::Px(2.0)),
            border_radius: BorderRadius::all(Val::Px(8.0)),
            ..default()
        },
        BackgroundGradient::from(LinearGradient::to_bottom(vec![
            ColorStop::percent(Color::srgba(0.2, 0.14, 0.05, 0.95), 0.0),
            ColorStop::percent(Color::srgba(0.04, 0.03, 0.02, 0.97), 100.0),
        ])),
        BorderColor::all(edge),
    ))
    .with_children(|c| {
        skin::backdrop(c);
        body(c);
    });
}

fn build(ui: &mut ChildSpawnerCommands, font: &Handle<Font>, runes: &Runes) {
    let pending = runes.save.pending.as_ref();
    ui.spawn(hud::centered_bundle(font, "Runes", 26.0, hud::GOLD_TONE));
    ui.spawn(hud::centered_bundle(
        font,
        if pending.is_some() {
            "New rune: place it, swap it in, or spend it to upgrade a slotted rune. \
             A higher rarity raises the upgrade chance."
        } else {
            "Shared by all your characters. Only the equipped rune is active."
        },
        12.0,
        hud::DIM_GREY,
    ));
    // One row: the new rune, then the slots; cards in a row share the tallest height.
    ui.spawn(Node {
        column_gap: Val::Px(12.0),
        margin: UiRect::top(Val::Px(4.0)),
        ..default()
    })
    .with_children(|row| {
        if let Some(rune) = pending {
            card(row, rarity_color(runes, rune), |c| {
                c.spawn(hud::centered_bundle(
                    font,
                    "New rune",
                    12.0,
                    Color::srgb(0.85, 0.78, 0.6),
                ));
                rune_lines(c, font, runes, rune);
            });
            row.spawn((
                Node {
                    align_self: AlignSelf::Center,
                    ..default()
                },
                hud::centered_bundle(font, "»", 28.0, hud::GOLD_TONE),
            ));
        }
        for slot in 0..runes.save.slots.len() {
            let rune = runes.save.rune(slot);
            let equipped = runes.save.equipped == Some(slot);
            let edge = if equipped { EQUIPPED } else { EDGE };
            card(row, edge, |c| {
                c.spawn(hud::centered_bundle(
                    font,
                    format!(
                        "Slot {}{}",
                        slot + 1,
                        if equipped { " — equipped" } else { "" }
                    ),
                    12.0,
                    if equipped { EQUIPPED } else { hud::DIM_GREY },
                ));
                match rune {
                    Some(rune) => rune_lines(c, font, runes, rune),
                    None => {
                        c.spawn(hud::centered_bundle(font, "Empty", 13.0, hud::DIM_GREY));
                    }
                }
                // Push the buttons to the bottom of the card.
                c.spawn(Node {
                    flex_grow: 1.0,
                    ..default()
                });
                match (pending, rune) {
                    (Some(_), None) => {
                        skin::button(c, font, "Place here", RuneAction::Place(slot), 150.0)
                    }
                    (Some(spent), Some(r)) => {
                        skin::button(c, font, "Swap", RuneAction::Swap(slot), 150.0);
                        let label = match runes.config.upgrade_chance(r, spent.rarity) {
                            Some(p) => {
                                // The chance, spelled out: base x rarity multiplier = final.
                                let base = runes.config.upgrade_chances[r.level as usize];
                                let bonus = runes.config.rarity_bonus(spent.rarity);
                                let (text, color) = if bonus > 1.0 {
                                    (
                                        format!(
                                            "Chance {} x{bonus} = {}",
                                            percent(base),
                                            percent(p)
                                        ),
                                        EQUIPPED,
                                    )
                                } else {
                                    (format!("Chance {} (x1)", percent(p)), hud::DIM_GREY)
                                };
                                c.spawn(hud::centered_bundle(font, text, 11.0, color));
                                format!("Upgrade to +{}", r.level + 1)
                            }
                            None => "Max level".to_string(),
                        };
                        skin::button(c, font, label, RuneAction::Upgrade(slot), 150.0);
                    }
                    _ => {}
                }
                if rune.is_some() {
                    // Side by side, to keep the card height.
                    c.spawn(Node {
                        column_gap: Val::Px(6.0),
                        ..default()
                    })
                    .with_children(|buttons| {
                        let label = if equipped { "Unequip" } else { "Equip" };
                        skin::button(buttons, font, label, RuneAction::Equip(slot), 80.0);
                        let label = if runes.delete_armed == Some(slot) {
                            "Confirm"
                        } else {
                            "Delete"
                        };
                        skin::button(buttons, font, label, RuneAction::Delete(slot), 80.0);
                    });
                }
            });
        }
    });
    ui.spawn(hud::centered_bundle(
        font,
        bonus_table(runes),
        11.0,
        hud::DIM_GREY,
    ));
    if !runes.message.is_empty() {
        ui.spawn(hud::centered_bundle(
            font,
            runes.message.clone(),
            14.0,
            hud::GOLD_TONE,
        ));
    }
    ui.spawn(Node {
        column_gap: Val::Px(12.0),
        margin: UiRect::top(Val::Px(4.0)),
        ..default()
    })
    .with_children(|row| {
        if pending.is_some() {
            skin::button(row, font, "Discard rune", RuneAction::Discard, 160.0);
        } else {
            skin::button(row, font, "Close", RuneAction::Close, 160.0);
        }
    });
}

/// "Upgrade chance by the rarity of the spent rune: Bronze x1 · Silver x2 · ...".
fn bonus_table(runes: &Runes) -> String {
    let steps: Vec<String> = runes
        .config
        .rarities
        .iter()
        .enumerate()
        .map(|(rarity, def)| {
            format!(
                "{} x{}",
                def.name.trim_end_matches(" Rune"),
                runes.config.rarity_bonus(rarity as u8)
            )
        })
        .collect();
    format!(
        "Upgrade chance by the rarity of the spent rune:  {}",
        steps.join("  ·  ")
    )
}

/// "30%", "1.5%", "0.1%".
fn percent(p: f32) -> String {
    let v = p * 100.0;
    if v >= 10.0 || v.fract() == 0.0 {
        format!("{v:.0}%")
    } else {
        format!("{v:.1}%")
    }
}

#[allow(clippy::type_complexity)]
fn refresh(
    mut commands: Commands,
    runes: Res<Runes>,
    phase: Res<State<Phase>>,
    selection: Res<Selection>,
    font: Option<Res<hud::HudFont>>,
    mut root: Query<(Entity, &mut Visibility), With<PanelRoot>>,
    mut shown: Local<Option<(u64, String, bool)>>,
) {
    let Ok((entity, mut visibility)) = root.single_mut() else {
        return;
    };
    let Some(font) = font else { return };
    let training = crate::survivors::training::active(&selection);
    let show = visible(&runes, *phase.get(), training);
    let key = (runes.revision, runes.message.clone(), show);
    if shown.as_ref() == Some(&key) {
        return;
    }
    *shown = Some(key);
    *visibility = if show {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    commands.entity(entity).despawn_related::<Children>();
    if show {
        commands
            .entity(entity)
            .with_children(|ui| build(ui, &font.0, &runes));
    }
}

fn clicks(
    presses: Query<(&Interaction, &RuneAction), Changed<Interaction>>,
    mut runes: ResMut<Runes>,
    mut rng: ResMut<Rng>,
) {
    for (interaction, action) in &presses {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let result = runes.act(*action, &mut rng);
        if *action == RuneAction::Close {
            continue;
        }
        runes.message = match result {
            Ok(message) | Err(message) => message,
        };
    }
}

/// R opens the panel in the hub and on the training ground; a rune still waiting to be
/// placed opens it on arrival in the hub.
fn hub_keys(
    keys: Res<ButtonInput<KeyCode>>,
    phase: Res<State<Phase>>,
    selection: Res<Selection>,
    mut runes: ResMut<Runes>,
) {
    let training = *phase.get() == Phase::Playing && crate::survivors::training::active(&selection);
    if !(*phase.get() == Phase::Hub || training) || selection.ui_captured {
        return;
    }
    if keys.just_pressed(KeyCode::KeyR) {
        runes.open = !runes.open;
        if !runes.open {
            runes.message.clear();
        }
    }
}

/// The range tooltip beside the cursor while a rune icon is hovered.
fn tooltip(
    mut commands: Commands,
    runes: Res<Runes>,
    font: Option<Res<hud::HudFont>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    scale: Option<Res<UiScale>>,
    icons: Query<(&Interaction, &RuneTip, &InheritedVisibility)>,
    mut roots: Query<(Entity, &mut Node), With<TipRoot>>,
    mut shown: Local<Option<u8>>,
) {
    let hovered = icons
        .iter()
        .find(|(i, _, v)| **i != Interaction::None && v.get())
        .map(|(_, tip, _)| tip.0);
    let (Some(rarity), Some(font)) = (hovered, font) else {
        for (entity, _) in &mut roots {
            commands.entity(entity).despawn();
        }
        *shown = None;
        return;
    };
    let (viewport, cursor) = windows.single().map_or(
        (Vec2::new(1365.0, 768.0), Vec2::new(800.0, 400.0)),
        |window| {
            let scale = scale.as_deref().map_or(1.0, |s| s.0).max(0.01);
            (
                Vec2::new(window.width(), window.height()) / scale,
                window.cursor_position().unwrap_or(Vec2::ZERO) / scale,
            )
        },
    );
    let width = 250.0_f32.min((viewport.x - 16.0).max(1.0));
    let x = if cursor.x > viewport.x * 0.5 {
        cursor.x - width - 18.0
    } else {
        cursor.x + 18.0
    };
    let left = Val::Px(x.clamp(8.0, (viewport.x - width - 8.0).max(8.0)));
    let top = Val::Px((cursor.y + 14.0).clamp(8.0, (viewport.y - 320.0).max(8.0)));
    if *shown == Some(rarity) && !roots.is_empty() {
        for (_, mut node) in &mut roots {
            node.left = left;
            node.top = top;
        }
        return;
    }
    for (entity, _) in &mut roots {
        commands.entity(entity).despawn();
    }
    *shown = Some(rarity);
    let f = &font.0;
    let def = runes.config.rarity(rarity);
    let color = parse_color(&def.color).unwrap_or(Color::WHITE);
    commands
        .spawn((
            TipRoot,
            GlobalZIndex(650),
            BackgroundColor(Color::srgb(0.02, 0.025, 0.04)),
            BorderColor::all(color),
            Node {
                position_type: PositionType::Absolute,
                left,
                top,
                width: Val::Px(width),
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::all(Val::Px(2.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                ..default()
            },
        ))
        .with_children(|tip| {
            tip.spawn(hud::text_bundle(f, def.name.clone(), 15.0, color));
            tip.spawn(hud::text_bundle(
                f,
                "Possible stats and roll ranges",
                11.0,
                hud::DIM_GREY,
            ));
            for (level, line) in runes.possible_stats(rarity) {
                let (line, color) = if level == 0 {
                    (line, Color::WHITE)
                } else {
                    (format!("{line}  (unlocks at +{level})"), hud::DIM_GREY)
                };
                tip.spawn(hud::text_bundle(f, line, 12.0, color));
            }
        });
}

fn open_for_pending(mut runes: ResMut<Runes>) {
    if runes.save.pending.is_some() {
        runes.open = true;
    }
}

fn close_on_leave(mut runes: ResMut<Runes>) {
    runes.open = false;
    runes.message.clear();
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn_panel)
        .add_systems(OnEnter(Phase::Hub), open_for_pending)
        .add_systems(OnExit(Phase::Hub), close_on_leave)
        .add_systems(Update, (hub_keys, clicks, refresh, tooltip).chain());
}
