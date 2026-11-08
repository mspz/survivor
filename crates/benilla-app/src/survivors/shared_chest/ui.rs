//! The shared vault uses the same compact item cells as the player's bag.
use super::super::combat_ui::{
    bags::{DropTarget, ItemTip},
    skin,
};
use super::*;
use bevy::{
    ui::{FocusPolicy, RelativeCursorPosition},
    window::PrimaryWindow,
};
const PAGE_SIZE: usize = 112;
#[derive(Component)]
pub(super) struct Root;
#[derive(Component, Clone, Copy)]
pub(super) enum Click {
    Open,
    Close,
    Page(i32),
}
#[derive(Component)]
pub(super) struct List;
#[derive(Component)]
pub(super) struct Inspect(Item);
#[derive(Component, Clone, Copy)]
pub(super) struct VaultItem(u64);
#[derive(Component)]
pub(super) struct Tooltip;
#[derive(Component)]
pub(super) struct Ghost;
#[derive(Clone, Copy)]
enum Source {
    Bag(u64),
    Vault(u64),
}
#[derive(Clone, Copy)]
pub(super) struct Held {
    source: Source,
    start: Vec2,
    dragging: bool,
}
fn text(font: &Handle<Font>, value: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(value),
        TextFont {
            font: font.clone(),
            font_size: size,
            ..default()
        },
        TextColor(color),
    )
}
pub(super) fn input(
    phase: Res<State<Phase>>,
    mut selection: ResMut<Selection>,
    hero: Query<&Transform, With<hero::Hero>>,
    mut view: ResMut<View>,
    mut other: Option<ResMut<super::OtherView>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    clicks: Query<(&Interaction, &Click), Changed<Interaction>>,
) {
    let accessible = selection.in_hub
        && matches!(*phase.get(), Phase::Hub | Phase::CharacterSheet)
        && hero.single().is_ok_and(|t| near(t.translation));
    if !accessible {
        view.open = false;
        return;
    }
    if view.open && keys.just_pressed(KeyCode::Escape) {
        view.open = false;
        keys.clear_just_pressed(KeyCode::Escape);
    }
    for (i, c) in &clicks {
        if *i != Interaction::Pressed {
            continue;
        }
        match *c {
            Click::Open => {
                if let Some(other) = other.as_deref_mut() {
                    other.open = false;
                }
                view.open = true;
                selection.bags_open = true;
                selection.talents_open = false;
                selection.sheet_open = false;
            }
            Click::Close => view.open = false,
            Click::Page(d) => {
                view.page = view
                    .page
                    .saturating_add_signed(d as isize)
                    .min(view.replica.entries.len().max(1).div_ceil(PAGE_SIZE) - 1)
            }
        }
    }
    if view.open {
        selection.ui_captured = true;
    }
}
fn transfer(source: Source, inventory: &mut Inventory, view: &mut View) {
    view.notice.clear();
    match source {
        Source::Bag(id) => {
            // Equipped items are never deposited by a gesture on the paper doll.
            if !inventory.bag_cells().iter().flatten().any(|i| i.id == id) {
                return;
            }
            if let Err(e) = inventory.begin_deposit(id, &view.replica.identity) {
                view.notice = e;
            }
        }
        Source::Vault(entry) => {
            if !view.replica.entries.iter().any(|e| e.id == entry) {
                return;
            }
            match inventory.ensure_trade_identity() {
                Ok(_) => view.requests.push(Request::Withdraw {
                    vault: view.replica.identity.clone(),
                    entry,
                }),
                Err(e) => view.notice = e,
            }
        }
    }
}
#[allow(clippy::too_many_arguments)]
pub(super) fn gestures(
    mut commands: Commands,
    mut held: Local<Option<Held>>,
    mut view: ResMut<View>,
    mut inventory: ResMut<Inventory>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    keys: Res<ButtonInput<KeyCode>>,
    scale: Option<Res<UiScale>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut selection: ResMut<Selection>,
    bags: Query<(&Interaction, &ItemTip, &RelativeCursorPosition)>,
    vault: Query<(&Interaction, &VaultItem, &RelativeCursorPosition)>,
    grids: Query<&RelativeCursorPosition, With<List>>,
    targets: Query<(&DropTarget, &RelativeCursorPosition)>,
    ghosts: Query<Entity, With<Ghost>>,
) {
    for e in &ghosts {
        commands.entity(e).despawn();
    }
    let cursor = windows
        .single()
        .ok()
        .filter(|w| w.focused)
        .and_then(Window::cursor_position)
        .map(|p| p / scale.as_deref().map_or(1.0, |s| s.0).max(0.01));
    let Some(mouse) = mouse else {
        *held = None;
        return;
    };
    let Some(cursor) = cursor.filter(|_| view.open && selection.in_hub) else {
        *held = None;
        return;
    };
    if keys.just_pressed(KeyCode::Escape) {
        *held = None;
        return;
    }
    let source = vault
        .iter()
        .find(|(i, _, p)| **i != Interaction::None && p.cursor_over())
        .map(|(_, s, _)| Source::Vault(s.0))
        .or_else(|| {
            bags.iter()
                .find(|(i, _, p)| **i != Interaction::None && p.cursor_over())
                .map(|(_, s, _)| Source::Bag(s.0))
        });
    if mouse.just_pressed(MouseButton::Right) {
        if held.take().is_none() {
            if let Some(s) = source {
                transfer(s, &mut inventory, &mut view);
            }
        }
        return;
    }
    if mouse.just_pressed(MouseButton::Left) {
        *held = source.map(|source| Held {
            source,
            start: cursor,
            dragging: false,
        });
    }
    let Some(mut press) = *held else {
        return;
    };
    press.dragging |= press.start.distance(cursor) >= 4.0;
    selection.ui_captured = true;
    if mouse.just_released(MouseButton::Left) {
        *held = None;
        let destination = match press.source {
            Source::Bag(_) => grids.iter().any(|p| p.cursor_over()),
            Source::Vault(_) => targets.iter().any(|(t, p)| {
                matches!(t, DropTarget::Bag(_) | DropTarget::BagBackground) && p.cursor_over()
            }),
        };
        if press.dragging && destination {
            transfer(press.source, &mut inventory, &mut view);
        }
    } else if mouse.pressed(MouseButton::Left) {
        *held = Some(press);
        if press.dragging {
            let item = match press.source {
                Source::Bag(id) => inventory.item(id),
                Source::Vault(id) => view
                    .replica
                    .entries
                    .iter()
                    .find(|e| e.id == id)
                    .map(|e| &e.item),
            };
            if let Some(item) = item {
                commands.spawn((
                    Ghost,
                    GlobalZIndex(900),
                    FocusPolicy::Pass,
                    ImageNode::default(),
                    hud::UiArt::item(&item.icon),
                    skin::rect(cursor.x + 8.0, cursor.y + 8.0, 32.0, 32.0),
                ));
            }
        }
    } else {
        *held = None;
    }
}
pub(super) fn render(
    mut commands: Commands,
    view: Res<View>,
    inventory: Res<Inventory>,
    selection: Res<Selection>,
    phase: Res<State<Phase>>,
    hero: Query<&Transform, With<hero::Hero>>,
    font: Option<Res<hud::HudFont>>,
    roots: Query<Entity, With<Root>>,
    mut cache: Local<String>,
) {
    let Some(font) = font else { return };
    let accessible = selection.in_hub
        && matches!(*phase.get(), Phase::Hub | Phase::CharacterSheet)
        && hero.single().is_ok_and(|t| near(t.translation));
    let key = format!(
        "{accessible} {} {} {} {:?} {} {}",
        view.open,
        view.page,
        inventory.revision,
        view.replica
            .entries
            .iter()
            .map(|e| e.id)
            .collect::<Vec<_>>(),
        view.notice,
        view.replica.identity
    );
    if *cache == key {
        return;
    }
    *cache = key;
    for e in &roots {
        commands.entity(e).despawn();
    }
    if !accessible {
        return;
    }
    if !view.open {
        commands
            .spawn((
                Root,
                GlobalZIndex(455),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(50.0),
                    bottom: Val::Px(58.0),
                    margin: UiRect::left(Val::Px(PROMPT_OFFSET)),
                    ..default()
                },
            ))
            .with_children(|ui| skin::button(ui, &font.0, OPEN_LABEL, Click::Open, 230.0));
        return;
    }
    let pages = view.replica.entries.len().max(1).div_ceil(PAGE_SIZE);
    let page = view.page.min(pages - 1);
    commands
        .spawn((
            Root,
            GlobalZIndex(610),
            BackgroundColor(Color::srgba(0.012, 0.012, 0.015, 0.98)),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(18.0),
                bottom: Val::Px(120.0),
                width: Val::Px(498.0),
                height: Val::Px(364.0),
                ..default()
            },
        ))
        .with_children(|ui| {
            skin::frame(ui, 498.0, 364.0);
            ui.spawn((
                text(&font.0, TITLE, 13.0, hud::GOLD_TONE),
                skin::rect(14.0, 6.0, 420.0, 20.0),
            ));
            skin::close(ui, Click::Close, 464.0, 1.0);
            ui.spawn((
                List,
                RelativeCursorPosition::default(),
                Interaction::default(),
                Node {
                    flex_wrap: FlexWrap::Wrap,
                    align_content: AlignContent::Start,
                    column_gap: Val::Px(2.0),
                    row_gap: Val::Px(2.0),
                    ..skin::rect(12.0, 30.0, 474.0, 270.0)
                },
            ))
            .with_children(|grid| {
                for index in 0..PAGE_SIZE {
                    let entry = view.replica.entries.get(page * PAGE_SIZE + index);
                    let mut cell = grid.spawn((
                        Button,
                        RelativeCursorPosition::default(),
                        BackgroundColor(Color::srgb(0.015, 0.015, 0.015)),
                        BorderColor::all(
                            entry.map_or(Color::srgb(0.18, 0.18, 0.18), |e| e.item.rarity.color()),
                        ),
                        Node {
                            width: Val::Px(32.0),
                            height: Val::Px(32.0),
                            border: UiRect::all(Val::Px(1.0)),
                            flex_shrink: 0.0,
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                    ));
                    if let Some(entry) = entry {
                        cell.insert((VaultItem(entry.id), Inspect(entry.item.clone())))
                            .with_children(|b| {
                                b.spawn((
                                    ImageNode::default(),
                                    hud::UiArt::item(&entry.item.icon),
                                    Node {
                                        width: Val::Px(28.0),
                                        height: Val::Px(28.0),
                                        ..default()
                                    },
                                ));
                            });
                    } else {
                        cell.with_children(|b| {
                            skin::art(
                                b,
                                "Interface\\PaperDoll\\UI-Backpack-EmptySlot",
                                skin::rect(0.0, 0.0, 30.0, 30.0),
                                [0.0, 1.0, 0.0, 1.0],
                            )
                        });
                    }
                }
            });
            ui.spawn(Node {
                align_items: AlignItems::Center,
                column_gap: Val::Px(7.0),
                ..skin::rect(12.0, 305.0, 474.0, 25.0)
            })
            .with_children(|footer| {
                footer.spawn(text(
                    &font.0,
                    format!("{} items", view.replica.entries.len()),
                    11.0,
                    Color::WHITE,
                ));
                if pages > 1 {
                    skin::button(footer, &font.0, "<", Click::Page(-1), 28.0);
                    footer.spawn(text(
                        &font.0,
                        format!("{} / {pages}", page + 1),
                        11.0,
                        hud::GOLD_TONE,
                    ));
                    skin::button(footer, &font.0, ">", Click::Page(1), 28.0);
                }
            });
            ui.spawn((
                text(
                    &font.0,
                    if view.notice.is_empty() {
                        "Right-click or drag items between your bags and the chest."
                    } else {
                        &view.notice
                    },
                    11.0,
                    hud::GOLD_TONE,
                ),
                skin::rect(12.0, 335.0, 474.0, 22.0),
            ));
        });
}
pub(super) fn tooltip(
    mut commands: Commands,
    view: Res<View>,
    inventory: Res<Inventory>,
    font: Option<Res<hud::HudFont>>,
    hovered: Query<(&Interaction, &Inspect)>,
    roots: Query<Entity, With<Tooltip>>,
) {
    for e in &roots {
        commands.entity(e).despawn();
    }
    if !view.open {
        return;
    }
    let Some(font) = font else { return };
    let Some((_, item)) = hovered.iter().find(|(i, _)| **i == Interaction::Hovered) else {
        return;
    };
    commands
        .spawn((
            Tooltip,
            GlobalZIndex(680),
            BackgroundColor(Color::srgb(0.02, 0.025, 0.04)),
            BorderColor::all(hud::GOLD_TONE),
            Node {
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                position_type: PositionType::Absolute,
                left: Val::Px(526.0),
                bottom: Val::Px(120.0),
                width: Val::Px(280.0),
                ..default()
            },
        ))
        .with_children(|tip| {
            for (line, color) in item.0.tooltip_lines(&inventory) {
                tip.spawn(text(&font.0, line, 13.0, color));
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn right_click_and_drag_transfer_in_both_directions_without_moving_on_mouse_down() {
        for withdraw in [false, true] {
            for drag in [false, true] {
                let mut app = App::new();
                app.init_resource::<ButtonInput<MouseButton>>()
                    .init_resource::<ButtonInput<KeyCode>>()
                    .insert_resource(Selection {
                        in_hub: true,
                        bags_open: true,
                        ..default()
                    })
                    .insert_resource(Inventory::fixture(vec![super::super::fixture_item()]))
                    .insert_resource(View {
                        open: true,
                        replica: Replica {
                            identity: "test-vault".into(),
                            entries: vec![Entry {
                                id: 7,
                                item: super::super::fixture_item(),
                            }],
                            ..default()
                        },
                        ..default()
                    })
                    .add_systems(Update, gestures);
                let mut window = Window::default();
                window.focused = true;
                window.set_cursor_position(Some(Vec2::new(100.0, 100.0)));
                let window = app.world_mut().spawn((PrimaryWindow, window)).id();
                let mut source = app.world_mut().spawn((
                    Interaction::Hovered,
                    RelativeCursorPosition {
                        cursor_over: true,
                        normalized: Some(Vec2::ZERO),
                    },
                ));
                if withdraw {
                    source.insert(VaultItem(7));
                } else {
                    source.insert(ItemTip(1));
                }
                if drag {
                    app.world_mut()
                        .resource_mut::<ButtonInput<MouseButton>>()
                        .press(MouseButton::Left);
                    app.update();
                    assert!(app
                        .world()
                        .resource::<Inventory>()
                        .pending_deposits()
                        .is_empty());
                    assert!(app.world().resource::<View>().requests.is_empty());
                    app.world_mut()
                        .resource_mut::<ButtonInput<MouseButton>>()
                        .clear_just_pressed(MouseButton::Left);
                    app.world_mut()
                        .get_mut::<Window>(window)
                        .unwrap()
                        .set_cursor_position(Some(Vec2::new(140.0, 100.0)));
                    let mut target = app.world_mut().spawn(RelativeCursorPosition {
                        cursor_over: true,
                        normalized: Some(Vec2::ZERO),
                    });
                    if withdraw {
                        target.insert(DropTarget::BagBackground);
                    } else {
                        target.insert(List);
                    }
                    app.world_mut()
                        .resource_mut::<ButtonInput<MouseButton>>()
                        .release(MouseButton::Left);
                } else {
                    app.world_mut()
                        .resource_mut::<ButtonInput<MouseButton>>()
                        .press(MouseButton::Right);
                }
                app.update();
                if withdraw {
                    assert!(matches!(
                        app.world().resource::<View>().requests.as_slice(),
                        [Request::Withdraw { entry: 7, .. }]
                    ));
                } else {
                    assert_eq!(
                        app.world().resource::<Inventory>().pending_deposits().len(),
                        1
                    );
                    assert!(app.world().resource::<Inventory>().item(1).is_none());
                }
            }
        }
    }
    #[test]
    fn chest_window_opens_near_the_chest_and_closes_outside_the_hub() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<Phase>()
            .insert_resource(Selection {
                in_hub: true,
                ..default()
            })
            .insert_resource(View {
                replica: Replica {
                    entries: vec![Entry {
                        id: 1,
                        item: super::super::fixture_item(),
                    }],
                    ..default()
                },
                ..default()
            })
            .insert_resource(Inventory::fixture(vec![super::super::fixture_item()]))
            .insert_resource(hud::HudFont(Handle::default()))
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Update, (input, render).chain());
        app.world_mut().spawn((
            hero::Hero { vy: 0.0, air: 0.0 },
            Transform::from_translation(position()),
        ));
        app.world_mut()
            .resource_mut::<NextState<Phase>>()
            .set(Phase::Hub);
        app.update();
        let world = app.world_mut();
        let open = world
            .query::<(Entity, &Click)>()
            .iter(world)
            .find(|(_, c)| matches!(c, Click::Open))
            .unwrap()
            .0;
        world.entity_mut(open).insert(Interaction::Pressed);
        app.update();
        assert!(app.world().resource::<View>().open);
        assert!(app.world().resource::<Selection>().ui_captured);
        let world = app.world_mut();
        assert_eq!(world.query::<&List>().iter(world).count(), 1);
        let lists: Vec<_> = world
            .query_filtered::<Entity, With<List>>()
            .iter(world)
            .collect();
        let item = world
            .query_filtered::<Entity, With<Inspect>>()
            .single(world)
            .unwrap();
        world.entity_mut(item).insert(Interaction::Hovered);
        app.update();
        let world = app.world_mut();
        assert_eq!(
            world
                .query_filtered::<Entity, With<List>>()
                .iter(world)
                .collect::<Vec<_>>(),
            lists,
            "inspecting an item must preserve the lists and their scroll positions"
        );
        world.resource_mut::<Selection>().in_hub = false;
        world.resource_mut::<NextState<Phase>>().set(Phase::Playing);
        app.update();
        let world = app.world_mut();
        assert!(!world.resource::<View>().open);
        assert_eq!(
            world
                .query_filtered::<Entity, With<Root>>()
                .iter(world)
                .count(),
            0
        );
    }
}
