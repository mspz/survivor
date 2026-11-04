//! Character-select composition using the client's own Classic glue artwork.
use super::*;
use crate::glue::{
    art::GlueArt,
    widgets::{Hilight, LockHighlight},
};
use bevy::ui_render::ui_material::MaterialNode;

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn textured_menu_buttons_keep_their_actions_without_plain_background_components() {
        for (phase, in_hub, action, target) in [
            (Phase::Paused, true, hud::UiAction::Resume, Phase::Hub),
            (Phase::Paused, false, hud::UiAction::Resume, Phase::Playing),
            (Phase::GameOver, true, hud::UiAction::MainMenu, Phase::Hub),
            (Phase::Paused, true, hud::UiAction::Logout, Phase::Menu),
        ] {
            let mut world = World::new();
            world.insert_resource(State::new(phase));
            world.init_resource::<NextState<Phase>>();
            world.init_resource::<Messages<AppExit>>();
            world.init_resource::<crate::survivors::options::Panel>();
            world.insert_resource(Selection {
                in_hub,
                ..default()
            });
            world.commands().spawn(Node::default()).with_children(|ui| {
                skin::button(ui, &Handle::default(), "Action", action, 200.0);
            });
            world.flush();
            let entity = world
                .query_filtered::<Entity, With<hud::UiAction>>()
                .single(&world)
                .unwrap();
            *world.get_mut::<Interaction>(entity).unwrap() = Interaction::Pressed;
            world.run_system_once(hud::buttons).unwrap();
            assert!(
                matches!(world.resource::<NextState<Phase>>(), NextState::Pending(phase) if *phase == target)
            );
        }
    }
}

pub(super) fn menu(
    commands: &mut Commands,
    roster: &Roster,
    font: &Handle<Font>,
    viewport: Vec2,
    art: Option<&GlueArt>,
    selection: &Selection,
    spells: Option<&crate::ui_action::Spells>,
) {
    let panel_height = (viewport.y - 145.0).clamp(380.0, 700.0);
    if !roster.creating {
        commands
            .spawn((
                Root,
                GlobalZIndex(455),
                skin::rect(24.0, 12.0, 256.0, 158.0),
            ))
            .with_children(|ui| {
                skin::art(
                    ui,
                    "Interface\\Glues\\Common\\Glues-WoW-Logo",
                    skin::rect(0.0, 0.0, 256.0, 128.0),
                    [0.0, 1.0, 0.0, 1.0],
                );
                ui.spawn((
                    label(font, "SURVIVORS", 18.0, hud::GOLD_TONE),
                    skin::rect(69.0, 130.0, 180.0, 24.0),
                ));
            });
    }
    commands
        .spawn((
            Root,
            GlobalZIndex(455),
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(24.0),
                top: Val::Px(22.0),
                width: Val::Px(300.0),
                height: Val::Px(panel_height),
                ..default()
            },
        ))
        .with_children(|panel| {
            skin::frame(panel, 300.0, panel_height);
            panel
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(10.0),
                    ..skin::rect(18.0, 18.0, 264.0, panel_height - 36.0)
                })
                .with_children(|ui| {
                    ui.spawn(label(
                        font,
                        if roster.creating {
                            "Create Character"
                        } else {
                            "Azeroth Survivors"
                        },
                        20.0,
                        hud::GOLD_TONE,
                    ));
                    ui.spawn(label(
                        font,
                        "Stormwind City",
                        14.0,
                        Color::srgb(0.7, 0.7, 0.7),
                    ));
                    if roster.creating {
                        ui.spawn(label(
                            font,
                            "Choose your race and class.",
                            15.0,
                            Color::WHITE,
                        ));
                        ui.spawn(label(font, "Character name", 16.0, hud::GOLD_TONE));
                        ui.spawn(Node {
                            padding: UiRect::all(Val::Px(12.0)),
                            min_height: Val::Px(48.0),
                            ..default()
                        })
                        .with_children(|entry| {
                            skin::backdrop(entry);
                            entry.spawn(label(
                                font,
                                format!("{}|", roster.name),
                                22.0,
                                Color::WHITE,
                            ));
                        });
                        ui.spawn(label(
                            font,
                            "Type a name, then choose Create Character below.",
                            14.0,
                            Color::WHITE,
                        ));
                        if !roster.error.is_empty() {
                            ui.spawn(label(font, &roster.error, 14.0, Color::srgb(1.0, 0.3, 0.2)));
                        }
                        spell_window(ui, font, selection, spells);
                        ui.spawn(Node {
                            flex_grow: 1.0,
                            ..default()
                        });
                        button(ui, font, Click::Cancel, "Back", false);
                    } else {
                        if roster.save.characters.is_empty() {
                            ui.spawn(label(
                                font,
                                "Create your first adventurer.",
                                16.0,
                                Color::WHITE,
                            ));
                        }
                        ui.spawn((
                            Scroll,
                            Interaction::default(),
                            ScrollPosition::default(),
                            Node {
                                height: Val::Px(panel_height - 155.0),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(4.0),
                                overflow: Overflow::scroll_y(),
                                ..default()
                            },
                        ))
                        .with_children(|list| {
                            for character in &roster.save.characters {
                                let selected = roster.selected == Some(character.id);
                                list.spawn((
                                    Button,
                                    Click::Select(character.id),
                                    LockHighlight(selected),
                                    BackgroundColor(if selected {
                                        Color::srgba(0.45, 0.32, 0.02, 0.25)
                                    } else {
                                        Color::NONE
                                    }),
                                    Node {
                                        height: Val::Px(72.0),
                                        min_height: Val::Px(72.0),
                                        flex_direction: FlexDirection::Column,
                                        padding: UiRect::axes(Val::Px(10.0), Val::Px(7.0)),
                                        ..default()
                                    },
                                ))
                                .with_children(|row| {
                                    if let Some(highlight) =
                                        art.and_then(|art| art.select_highlight.as_ref())
                                    {
                                        row.spawn((
                                            Hilight,
                                            Visibility::Hidden,
                                            MaterialNode(highlight.clone()),
                                            skin::rect(-10.0, -3.0, 284.0, 78.0),
                                            bevy::ui::FocusPolicy::Pass,
                                        ));
                                    }
                                    row.spawn(label(font, &character.name, 18.0, hud::GOLD_TONE));
                                    row.spawn(label(
                                        font,
                                        format!(
                                            "Level {} {}",
                                            talents::level(character.xp),
                                            data::CLASSES[character.class].name
                                        ),
                                        14.0,
                                        Color::WHITE,
                                    ));
                                    row.spawn(label(
                                        font,
                                        "Stormwind City",
                                        13.0,
                                        Color::srgb(0.65, 0.65, 0.65),
                                    ));
                                });
                            }
                        });
                        if !roster.error.is_empty() {
                            ui.spawn(label(font, &roster.error, 13.0, Color::srgb(1.0, 0.3, 0.2)));
                        }
                        if !roster.blocked && roster.save.characters.len() < 10 {
                            button(ui, font, Click::Create, "Create New Character", false);
                        }
                    }
                });
        });
    if !roster.creating {
        if let Some(character) = roster
            .selected
            .and_then(|id| roster.save.characters.iter().find(|c| c.id == id))
        {
            commands
                .spawn((
                    Root,
                    GlobalZIndex(455),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(viewport.x * 0.5 - 140.0),
                        bottom: Val::Px(20.0),
                        width: Val::Px(280.0),
                        align_items: AlignItems::Center,
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(10.0),
                        ..default()
                    },
                ))
                .with_children(|ui| {
                    ui.spawn((
                        label(font, &character.name, 26.0, hud::GOLD_TONE),
                        TextShadow::default(),
                    ));
                    skin::button_sized(
                        ui,
                        font,
                        "Entrer dans le monde",
                        Click::Enter,
                        240.0,
                        44.0,
                        19.0,
                        false,
                    );
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        skin::button(ui, font, "Créer un salon", Click::HostLobby, 240.0);
                        skin::button(ui, font, "Rejoindre un salon", Click::JoinLobby, 240.0);
                    }
                    ui.spawn(Node {
                        column_gap: Val::Px(8.0),
                        ..default()
                    })
                    .with_children(|arrows| {
                        skin::button(arrows, font, "<", Click::Rotate(-0.35), 32.0);
                        skin::button(arrows, font, ">", Click::Rotate(0.35), 32.0);
                    });
                });
            if !roster.blocked {
                commands
                    .spawn((
                        Root,
                        GlobalZIndex(455),
                        Node {
                            position_type: PositionType::Absolute,
                            right: Val::Px(42.0),
                            bottom: Val::Px(36.0),
                            ..default()
                        },
                    ))
                    .with_children(|ui| button(ui, font, Click::Delete, "Delete Character", false));
            }
        }
    }
}

#[derive(Component)]
pub(super) struct CreationSpell(String);
#[derive(Component)]
pub(super) struct SpellHover;

pub(super) fn spell_hover(
    icons: Query<(&Interaction, &CreationSpell)>,
    mut captions: Query<&mut Text, With<SpellHover>>,
) {
    let name = icons
        .iter()
        .find(|(interaction, _)| **interaction != Interaction::None)
        .map_or("", |(_, spell)| spell.0.as_str());
    for mut caption in &mut captions {
        if caption.0 != name {
            caption.0 = name.to_owned();
        }
    }
}

fn spell_window(
    ui: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    selection: &Selection,
    spells: Option<&crate::ui_action::Spells>,
) {
    ui.spawn(Node {
        width: Val::Percent(100.0),
        padding: UiRect::all(Val::Px(12.0)),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(10.0),
        flex_shrink: 0.0,
        ..default()
    })
    .with_children(|panel| {
        skin::backdrop(panel);
        let class = selection.class.and_then(|index| data::CLASSES.get(index));
        panel.spawn(label(
            font,
            class.map_or_else(
                || "Class spells".to_owned(),
                |class| format!("{} spells", class.name),
            ),
            16.0,
            hud::GOLD_TONE,
        ));
        let Some(class) = class else {
            panel.spawn(label(font, "Choose a class", 13.0, Color::WHITE));
            return;
        };
        panel
            .spawn(Node {
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(8.0),
                row_gap: Val::Px(8.0),
                ..default()
            })
            .with_children(|grid| {
                let mut seen = std::collections::BTreeSet::new();
                for key in std::iter::once(&class.start).chain(class.pool.iter()) {
                    if !seen.insert(*key) {
                        continue;
                    }
                    let def = data::ability(key);
                    let display = spells.and_then(|spells| spells.catalog.get(def.spell));
                    let name = display
                        .map_or_else(|| key.replace('_', " "), |display| display.name.clone());
                    let icon = display
                        .and_then(|display| display.icon.as_deref())
                        .unwrap_or("Interface\\Icons\\INV_Misc_QuestionMark");
                    grid.spawn((
                        CreationSpell(name),
                        Button,
                        Node {
                            width: Val::Px(52.0),
                            height: Val::Px(40.0),
                            flex_shrink: 0.0,
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            row_gap: Val::Px(3.0),
                            ..default()
                        },
                    ))
                    .with_children(|cell| {
                        cell.spawn((
                            ImageNode::default(),
                            hud::UiArt::item(icon),
                            Node {
                                width: Val::Px(40.0),
                                height: Val::Px(40.0),
                                flex_shrink: 0.0,
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            BorderColor::all(Color::srgb(0.5, 0.45, 0.3)),
                        ));
                    });
                }
            });
        panel.spawn((
            SpellHover,
            label(font, "", 12.0, Color::WHITE),
            Node {
                height: Val::Px(30.0),
                width: Val::Percent(100.0),
                ..default()
            },
        ));
    });
}
