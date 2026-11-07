//! Character-select composition using the client's own Classic glue artwork.
use super::*;
use crate::glue::{
    art::GlueArt,
    widgets::{Hilight, LockHighlight},
};
use bevy::ui_render::ui_material::MaterialNode;
use bevy::window::PrimaryWindow;

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    #[test]
    fn creation_spell_tooltip_shows_effects_and_disappears_after_hover_or_menu_exit() {
        let mut app = App::new();
        app.insert_resource(State::new(Phase::Menu))
            .insert_resource(Selection {
                class: Some(4),
                ..default()
            })
            .insert_resource(hud::HudFont(Handle::default()))
            .add_systems(Update, spell_hover);
        let icon = app
            .world_mut()
            .spawn((
                Interaction::Hovered,
                CreationSpell(
                    "Power Word: Shield".into(),
                    data::ability("power_word_shield"),
                ),
            ))
            .id();
        app.update();
        let world = app.world_mut();
        assert_eq!(
            world
                .query_filtered::<Entity, With<CreationTooltip>>()
                .iter(world)
                .count(),
            1
        );
        assert!(world
            .query::<&Text>()
            .iter(world)
            .any(|t| t.0.contains("Absorbs")));
        assert!(world
            .query::<&Text>()
            .iter(world)
            .any(|t| t.0.contains("Recharge")));
        world.entity_mut(icon).insert(Interaction::None);
        app.update();
        let world = app.world_mut();
        assert_eq!(
            world
                .query_filtered::<Entity, With<CreationTooltip>>()
                .iter(world)
                .count(),
            0
        );
        world.entity_mut(icon).insert(Interaction::Hovered);
        world.insert_resource(State::new(Phase::Hub));
        app.update();
        let world = app.world_mut();
        assert_eq!(
            world
                .query_filtered::<Entity, With<CreationTooltip>>()
                .iter(world)
                .count(),
            0
        );
    }

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
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(24.0),
                    bottom: Val::Px(24.0),
                    ..default()
                },
            ))
            .with_children(|ui| {
                skin::button_sized(
                    ui,
                    font,
                    "Quitter le jeu",
                    hud::UiAction::Quit,
                    180.0,
                    36.0,
                    15.0,
                    false,
                );
            });
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
                    ui.spawn(label(font, "Hub", 14.0, Color::srgb(0.7, 0.7, 0.7)));
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
                                        "Hub",
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
                        left: Val::Px(viewport.x * 0.5 - 152.0),
                        bottom: Val::Px(20.0),
                        width: Val::Px(304.0),
                        align_items: AlignItems::Center,
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(8.0),
                        ..default()
                    },
                ))
                .with_children(|ui| {
                    ui.spawn((
                        label(font, &character.name, 24.0, hud::GOLD_TONE),
                        TextShadow {
                            offset: Vec2::new(1.0, 2.0),
                            color: Color::BLACK,
                        },
                    ));
                    ui.spawn(Node {
                        width: Val::Percent(100.0),
                        padding: UiRect::all(Val::Px(16.0)),
                        align_items: AlignItems::Center,
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(8.0),
                        ..default()
                    })
                    .with_children(|actions| {
                        skin::backdrop(actions);
                        skin::button_sized(
                            actions,
                            font,
                            "Entrer dans le monde",
                            Click::Enter,
                            272.0,
                            42.0,
                            17.0,
                            false,
                        );
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            actions.spawn((
                                BackgroundColor(hud::GOLD_TONE.with_alpha(0.22)),
                                Node {
                                    width: Val::Percent(90.0),
                                    height: Val::Px(1.0),
                                    margin: UiRect::vertical(Val::Px(3.0)),
                                    ..default()
                                },
                            ));
                            actions.spawn(label(
                                font,
                                "MULTIJOUEUR",
                                11.0,
                                Color::srgb(0.7, 0.64, 0.48),
                            ));
                            actions
                                .spawn(Node {
                                    flex_direction: FlexDirection::Column,
                                    row_gap: Val::Px(6.0),
                                    ..default()
                                })
                                .with_children(|lobby| {
                                    skin::button_sized(
                                        lobby,
                                        font,
                                        "Créer un salon",
                                        Click::HostLobby,
                                        272.0,
                                        34.0,
                                        14.0,
                                        false,
                                    );
                                    skin::button_sized(
                                        lobby,
                                        font,
                                        "Rejoindre un salon",
                                        Click::JoinLobby,
                                        272.0,
                                        34.0,
                                        14.0,
                                        false,
                                    );
                                });
                        }
                    });
                    ui.spawn(Node {
                        column_gap: Val::Px(10.0),
                        ..default()
                    })
                    .with_children(|arrows| {
                        skin::button_sized(
                            arrows,
                            font,
                            "<",
                            Click::Rotate(-0.35),
                            38.0,
                            28.0,
                            16.0,
                            false,
                        );
                        skin::button_sized(
                            arrows,
                            font,
                            ">",
                            Click::Rotate(0.35),
                            38.0,
                            28.0,
                            16.0,
                            false,
                        );
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
pub(super) struct CreationSpell(String, &'static data::AbilityDef);
#[derive(Component)]
pub(super) struct SpellHover;
#[derive(Component)]
pub(super) struct CreationTooltip;

#[allow(clippy::too_many_arguments)]
pub(super) fn spell_hover(
    mut commands: Commands,
    font: Option<Res<hud::HudFont>>,
    phase: Res<State<Phase>>,
    selection: Res<Selection>,
    spells: Option<Res<crate::ui_action::Spells>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    scale: Option<Res<UiScale>>,
    icons: Query<(&Interaction, &CreationSpell)>,
    mut captions: Query<&mut Text, With<SpellHover>>,
    roots: Query<Entity, With<CreationTooltip>>,
    mut cached: Local<Option<(usize, &'static str, Vec<String>)>>,
) {
    let hovered = icons
        .iter()
        .find(|(interaction, _)| **interaction != Interaction::None)
        .map(|(_, spell)| spell);
    let name = hovered.map_or("", |spell| spell.0.as_str());
    for mut caption in &mut captions {
        if caption.0 != name {
            caption.0 = name.to_owned();
        }
    }
    for root in &roots {
        commands.entity(root).despawn();
    }
    if *phase.get() != Phase::Menu {
        *cached = None;
        return;
    }
    let (Some(spell), Some(font), Some(class)) = (hovered, font, selection.class) else {
        *cached = None;
        return;
    };
    let Some(class_def) = data::CLASSES.get(class) else {
        return;
    };
    if cached
        .as_ref()
        .is_none_or(|(c, key, _)| *c != class || *key != spell.1.key)
    {
        let reference = Run {
            class: class_def,
            ..default()
        };
        let p = reference.resolve_spell(spell.1, 1);
        let card = hud::describe_card(
            hud::Choice::NewAbility(spell.1),
            &reference,
            spells.as_deref(),
        );
        *cached = Some((
            class,
            spell.1.key,
            vec![
                class_def.name.into(),
                p.tags.labels(),
                card.desc,
                format!("Recharge : {:.1} s · Rang 1", p.cooldown),
            ],
        ));
    }
    let lines = &cached.as_ref().unwrap().2;
    let inv = scale.as_deref().map_or(1.0, |s| s.0).max(0.01);
    let (viewport, cursor) =
        windows
            .single()
            .map_or((Vec2::new(1280.0, 768.0), Vec2::new(850.0, 250.0)), |w| {
                (
                    Vec2::new(w.width(), w.height()) / inv,
                    w.cursor_position().unwrap_or_default() / inv,
                )
            });
    let height = 55.0
        + lines
            .iter()
            .map(|l| (l.chars().count() / 40 + 1 + l.matches('\n').count()) as f32 * 18.0)
            .sum::<f32>();
    let x = if cursor.x + 360.0 > viewport.x {
        cursor.x - 350.0
    } else {
        cursor.x + 16.0
    };
    commands
        .spawn((
            CreationTooltip,
            GlobalZIndex(800),
            bevy::ui::FocusPolicy::Pass,
            BackgroundColor(Color::srgb(0.02, 0.025, 0.04)),
            BorderColor::all(hud::GOLD_TONE),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(x.max(4.0)),
                top: Val::Px((cursor.y + 16.0).min((viewport.y - height - 8.0).max(4.0))),
                width: Val::Px(330.0),
                max_height: Val::Vh(90.0),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(7.0),
                ..default()
            },
        ))
        .with_children(|tip| {
            tip.spawn(label(&font.0, name, 18.0, hud::GOLD_TONE));
            for line in lines {
                tip.spawn(label(&font.0, line, 14.0, Color::WHITE));
            }
        });
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
                        CreationSpell(name, def),
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
