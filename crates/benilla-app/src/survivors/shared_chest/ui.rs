use super::super::combat_ui::skin;
use super::*;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};

#[derive(Component)]
pub(super) struct Root;
#[derive(Component, Clone, Copy)]
pub(super) enum Click {
    Open,
    Close,
    Deposit(u64),
    Withdraw(u64),
}
#[derive(Component)]
pub(super) struct List(bool);
#[derive(Component)]
pub(super) struct Description;
#[derive(Component)]
pub(super) struct Inspect(Item);

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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chest_window_opens_near_the_chest_and_closes_outside_the_hub() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<Phase>()
            .insert_resource(Selection {
                in_hub: true,
                ..default()
            })
            .init_resource::<View>()
            .insert_resource(Inventory::fixture(vec![super::super::fixture_item()]))
            .insert_resource(hud::HudFont(Handle::default()))
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<MouseWheel>()
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
        assert_eq!(world.query::<&List>().iter(world).count(), 2);
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
pub(super) fn input(
    phase: Res<State<Phase>>,
    mut selection: ResMut<Selection>,
    hero: Query<&Transform, With<hero::Hero>>,
    mut view: ResMut<View>,
    mut inventory: ResMut<Inventory>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    clicks: Query<(&Interaction, &Click), Changed<Interaction>>,
    mut scroll: MessageReader<MouseWheel>,
    mut lists: Query<
        (
            &ComputedNode,
            &bevy::ui::RelativeCursorPosition,
            &mut ScrollPosition,
            &List,
        ),
        With<List>,
    >,
) {
    let accessible = selection.in_hub
        && *phase.get() == Phase::Hub
        && hero.single().is_ok_and(|t| near(t.translation));
    if !accessible {
        view.open = false;
        return;
    }
    if view.open && keys.just_pressed(KeyCode::Escape) {
        view.open = false;
        keys.clear_just_pressed(KeyCode::Escape);
    }
    for (interaction, click) in &clicks {
        if *interaction != Interaction::Pressed {
            continue;
        }
        view.notice.clear();
        match *click {
            Click::Open => view.open = true,
            Click::Close => view.open = false,
            Click::Deposit(id) if view.open => {
                if let Err(e) = inventory.begin_deposit(id, &view.replica.identity) {
                    view.notice = e;
                }
            }
            Click::Withdraw(entry) if view.open => match inventory.ensure_trade_identity() {
                Ok(_) => {
                    let vault = view.replica.identity.clone();
                    view.requests.push(Request::Withdraw { vault, entry });
                }
                Err(e) => view.notice = e,
            },
            _ => {}
        }
    }
    if view.open {
        selection.ui_captured = true;
        for wheel in scroll.read() {
            for (node, cursor, mut position, list) in &mut lists {
                if cursor.cursor_over() {
                    position.0.y = (position.0.y
                        - wheel.y
                            * if wheel.unit == MouseScrollUnit::Line {
                                36.0
                            } else {
                                1.0
                            })
                    .clamp(
                        0.0,
                        ((node.content_size.y - node.size.y) * node.inverse_scale_factor).max(0.0),
                    );
                    view.scroll[usize::from(list.0)] = position.0.y;
                }
            }
        }
    } else {
        scroll.clear();
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
    hovered: Query<(&Interaction, &Inspect)>,
    mut inspected_cache: Local<Option<Item>>,
    mut descriptions: Query<&mut Text, With<Description>>,
) {
    let Some(font) = font else { return };
    let accessible = selection.in_hub
        && *phase.get() == Phase::Hub
        && hero.single().is_ok_and(|t| near(t.translation));
    if !view.open {
        *inspected_cache = None;
    }
    if let Some((_, item)) = hovered.iter().find(|(i, _)| **i == Interaction::Hovered) {
        *inspected_cache = Some(item.0.clone());
    }
    let description = inspected_cache.as_ref().map(|item| {
        item.tooltip_lines(&inventory)
            .into_iter()
            .map(|(s, _)| s)
            .collect::<Vec<_>>()
            .join(" · ")
    });
    for mut text in &mut descriptions {
        let value = description
            .as_deref()
            .unwrap_or("Survolez un objet pour afficher ses caractéristiques.");
        if text.0 != value {
            text.0 = value.into();
        }
    }
    let key = format!(
        "{} {} {} {:?} {} {}",
        accessible,
        view.open,
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
    for root in &roots {
        commands.entity(root).despawn();
    }
    if !accessible {
        return;
    }
    let f = &font.0;
    if !view.open {
        commands
            .spawn((
                Root,
                GlobalZIndex(455),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(50.0),
                    bottom: Val::Px(58.0),
                    margin: UiRect::left(Val::Px(-115.0)),
                    ..default()
                },
            ))
            .with_children(|ui| {
                skin::button(ui, f, "Ouvrir le coffre partagé", Click::Open, 230.0);
            });
        return;
    }
    commands
        .spawn((
            Root,
            GlobalZIndex(610),
            BackgroundColor(Color::srgba(0.025, 0.02, 0.015, 0.98)),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Px(70.0),
                margin: UiRect::left(Val::Px(-380.0)),
                width: Val::Px(760.0),
                padding: UiRect::all(Val::Px(14.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                ..default()
            },
        ))
        .with_children(|ui| {
            skin::backdrop(ui);
            ui.spawn(Node {
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|header| {
                header.spawn(text(f, "Coffre partagé · Hurlevent", 22.0, hud::GOLD_TONE));
                skin::button(header, f, "Fermer", Click::Close, 80.0);
            });
            ui.spawn(text(
                f,
                "Les objets déposés peuvent être récupérés par les membres du salon.",
                13.0,
                Color::WHITE,
            ));
            ui.spawn(Node {
                column_gap: Val::Px(14.0),
                ..default()
            })
            .with_children(|row| {
                for storage in [false, true] {
                    row.spawn(Node {
                        width: Val::Percent(50.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(5.0),
                        ..default()
                    })
                    .with_children(|column| {
                        column.spawn(text(
                            f,
                            if storage {
                                "Coffre · récupérer"
                            } else {
                                "Votre sac · déposer"
                            },
                            15.0,
                            hud::GOLD_TONE,
                        ));
                        column
                            .spawn((
                                List(storage),
                                Interaction::default(),
                                bevy::ui::RelativeCursorPosition::default(),
                                ScrollPosition(Vec2::new(0.0, view.scroll[usize::from(storage)])),
                                Node {
                                    width: Val::Percent(100.0),
                                    height: Val::Px(310.0),
                                    overflow: Overflow::scroll_y(),
                                    flex_direction: FlexDirection::Column,
                                    row_gap: Val::Px(3.0),
                                    ..default()
                                },
                            ))
                            .with_children(|list| {
                                let entries: Vec<_> = if storage {
                                    view.replica
                                        .entries
                                        .iter()
                                        .map(|e| (e.id, &e.item))
                                        .collect()
                                } else {
                                    inventory
                                        .bag_cells()
                                        .into_iter()
                                        .flatten()
                                        .map(|i| (i.id, i))
                                        .collect()
                                };
                                if entries.is_empty() {
                                    list.spawn(text(
                                        f,
                                        "Aucun objet",
                                        13.0,
                                        Color::srgb(0.6, 0.6, 0.6),
                                    ));
                                }
                                for (id, item) in entries {
                                    let mut inspected = item.clone();
                                    if storage {
                                        inspected.id = 0;
                                    }
                                    list.spawn((
                                        Button,
                                        Inspect(inspected),
                                        if storage {
                                            Click::Withdraw(id)
                                        } else {
                                            Click::Deposit(id)
                                        },
                                        BackgroundColor(Color::srgb(0.08, 0.065, 0.04)),
                                        Node {
                                            height: Val::Px(38.0),
                                            flex_shrink: 0.0,
                                            width: Val::Percent(100.0),
                                            column_gap: Val::Px(7.0),
                                            align_items: AlignItems::Center,
                                            padding: UiRect::all(Val::Px(3.0)),
                                            ..default()
                                        },
                                    ))
                                    .with_children(|button| {
                                        button.spawn((
                                            ImageNode::default(),
                                            hud::UiArt::item(&item.icon),
                                            Node {
                                                width: Val::Px(30.0),
                                                height: Val::Px(30.0),
                                                ..default()
                                            },
                                        ));
                                        button.spawn((
                                            text(
                                                f,
                                                format!("{} · niv. {}", item.name, item.level),
                                                12.0,
                                                Color::WHITE,
                                            ),
                                            TextLayout::new_with_no_wrap(),
                                        ));
                                    });
                                }
                            });
                    });
                }
            });
            ui.spawn(text(
                f,
                format!(
                    "{} dépôt(s) en attente · {}",
                    inventory.pending_deposits().len(),
                    view.notice
                ),
                12.0,
                hud::GOLD_TONE,
            ));
            ui.spawn((
                Description,
                text(
                    f,
                    description
                        .as_deref()
                        .unwrap_or("Survolez un objet pour afficher ses caractéristiques."),
                    11.0,
                    Color::WHITE,
                ),
            ));
        });
}
