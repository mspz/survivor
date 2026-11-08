//! Reference catalogue: independent of the current build's learned spells and passive eligibility.
use super::{cards, combat_ui::skin, data, hud, Phase, Run};
use bevy::{prelude::*, ui::RelativeCursorPosition, window::PrimaryWindow};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Tab {
    #[default]
    Spells,
    Passives,
}
#[derive(Resource, Default)]
pub(super) struct Panel {
    pub open: bool,
    tab: Tab,
    class: Option<usize>,
    scroll: f32,
}
#[derive(Component)]
struct Root;
#[derive(Component)]
struct Body;
#[derive(Component)]
struct Tooltip;
#[derive(Component)]
struct Tip {
    name: String,
    lines: Vec<String>,
}
#[derive(Component, Clone, Copy)]
enum Click {
    Tab(Tab),
    Class(Option<usize>),
    Close,
}
const RARITIES: [(cards::Rarity, &str); 4] = [
    (cards::Rarity::Common, "Common"),
    (cards::Rarity::Uncommon, "Uncommon"),
    (cards::Rarity::Epic, "Epic"),
    (cards::Rarity::Legendary, "Legendary"),
];
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Panel>()
        .add_message::<bevy::input::mouse::MouseWheel>()
        .add_systems(Update, input.after(hud::buttons).before(hud::keys))
        .add_systems(Update, scroll.after(input).before(render))
        .add_systems(Update, render.after(input))
        .add_systems(Update, tooltip.after(render));
}
fn input(
    phase: Res<State<Phase>>,
    mut panel: ResMut<Panel>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    open: Query<(&Interaction, &hud::UiAction), Changed<Interaction>>,
    clicks: Query<(&Interaction, &Click), Changed<Interaction>>,
) {
    if *phase.get() != Phase::Paused {
        panel.open = false;
        return;
    }
    if open
        .iter()
        .any(|(i, a)| *i == Interaction::Pressed && matches!(a, hud::UiAction::Codex))
    {
        panel.open = true;
    }
    if !panel.open {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        panel.open = false;
        keys.clear_just_pressed(KeyCode::Escape);
        return;
    }
    for (i, c) in &clicks {
        if *i != Interaction::Pressed {
            continue;
        }
        match *c {
            Click::Close => panel.open = false,
            Click::Tab(tab) => {
                panel.tab = tab;
                panel.scroll = 0.0;
            }
            Click::Class(class) => {
                panel.class = class;
                panel.scroll = 0.0;
            }
        }
    }
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
fn spell_keys(class: &data::ClassDef) -> Vec<&'static str> {
    let mut keys = vec![class.start];
    for &key in class.pool {
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
    keys
}
fn passive_rarities(
    p: &'static data::PassiveDef,
    run: &Run,
    config: &super::reward_config::Config,
) -> Vec<usize> {
    let weights = config.rarity_weights();
    if super::passives::legendary(p) {
        if weights[3] > 0.0 && config.card_weight(hud::Choice::NewPassive(p), run) > 0.0 {
            vec![3]
        } else {
            vec![]
        }
    } else {
        (0..4)
            .filter(|i| {
                weights[*i] > 0.0
                    && config.card_weight(hud::Choice::NumericPassive(p, (*i + 1) as u8), run) > 0.0
            })
            .collect()
    }
}
fn entry(
    ui: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    spell: u32,
    name: String,
    lines: Vec<String>,
    badges: Vec<(String, Color)>,
) {
    ui.spawn((
        Button,
        Tip {
            name: name.clone(),
            lines,
        },
        BackgroundColor(Color::srgba(0.08, 0.065, 0.04, 0.9)),
        Node {
            width: Val::Percent(100.0),
            min_height: Val::Px(49.0),
            flex_shrink: 0.0,
            padding: UiRect::all(Val::Px(6.0)),
            column_gap: Val::Px(10.0),
            align_items: AlignItems::Center,
            ..default()
        },
    ))
    .with_children(|row| {
        row.spawn((
            ImageNode::default(),
            hud::SpellIcon(spell),
            Node {
                width: Val::Px(34.0),
                height: Val::Px(34.0),
                flex_shrink: 0.0,
                ..default()
            },
        ));
        row.spawn((
            text(font, name, 16.0, Color::WHITE),
            Node {
                flex_grow: 1.0,
                ..default()
            },
        ));
        row.spawn(Node {
            column_gap: Val::Px(9.0),
            row_gap: Val::Px(3.0),
            flex_wrap: FlexWrap::Wrap,
            ..default()
        })
        .with_children(|r| {
            for (label, color) in badges {
                r.spawn(text(font, label, 13.0, color));
            }
        });
    });
}
#[allow(clippy::too_many_arguments)]
fn render(
    mut commands: Commands,
    panel: Res<Panel>,
    phase: Res<State<Phase>>,
    run: Res<Run>,
    offer: Res<hud::Offer>,
    font: Option<Res<hud::HudFont>>,
    spells: Option<Res<crate::ui_action::Spells>>,
    roots: Query<Entity, With<Root>>,
    mut cache: Local<Option<(bool, Phase, Tab, Option<usize>)>>,
) {
    let Some(font) = font else { return };
    let key = (panel.open, *phase.get(), panel.tab, panel.class);
    if *cache == Some(key) {
        return;
    }
    *cache = Some(key);
    for e in &roots {
        commands.entity(e).despawn();
    }
    if !panel.open || *phase.get() != Phase::Paused {
        return;
    }
    commands
        .spawn((
            Root,
            Button,
            GlobalZIndex(810),
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
                .spawn(Node {
                    width: Val::Px(980.0),
                    max_width: Val::Percent(94.0),
                    height: Val::Px(700.0),
                    max_height: Val::Vh(90.0),
                    padding: UiRect::all(Val::Px(18.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(10.0),
                    ..default()
                })
                .with_children(|frame| {
                    skin::backdrop(frame);
                    frame
                        .spawn(Node {
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            ..default()
                        })
                        .with_children(|head| {
                            head.spawn(text(&font.0, "CODEX", 26.0, hud::GOLD_TONE));
                            skin::button(head, &font.0, "Back [Esc]", Click::Close, 180.0);
                        });
                    frame
                        .spawn(Node {
                            column_gap: Val::Px(10.0),
                            ..default()
                        })
                        .with_children(|tabs| {
                            for (tab, label) in [(Tab::Spells, "Spells"), (Tab::Passives, "Passives")]
                            {
                                skin::button_sized(
                                    tabs,
                                    &font.0,
                                    label,
                                    Click::Tab(tab),
                                    180.0,
                                    34.0,
                                    16.0,
                                    panel.tab == tab,
                                );
                            }
                        });
                    if panel.tab == Tab::Spells {
                        frame
                            .spawn(Node {
                                column_gap: Val::Px(5.0),
                                row_gap: Val::Px(5.0),
                                flex_wrap: FlexWrap::Wrap,
                                ..default()
                            })
                            .with_children(|classes| {
                                skin::button_sized(
                                    classes,
                                    &font.0,
                                    "All",
                                    Click::Class(None),
                                    100.0,
                                    28.0,
                                    12.0,
                                    panel.class.is_none(),
                                );
                                for (index, class) in data::CLASSES.iter().enumerate() {
                                    skin::button_sized(
                                        classes,
                                        &font.0,
                                        class.name,
                                        Click::Class(Some(index)),
                                        100.0,
                                        28.0,
                                        12.0,
                                        panel.class == Some(index),
                                    );
                                }
                            });
                    }
                    frame.spawn(text(
                        &font.0,
                        if panel.tab == Tab::Spells {
                            "Rank 1 reference values · Hover over a spell to view its effects"
                        } else {
                            "One row per card · All available rarities are grouped together"
                        },
                        13.0,
                        hud::GOLD_TONE,
                    ));
                    frame
                        .spawn((
                            Body,
                            Interaction::default(),
                            RelativeCursorPosition::default(),
                            ScrollPosition(Vec2::new(0.0, panel.scroll)),
                            Node {
                                width: Val::Percent(100.0),
                                flex_grow: 1.0,
                                flex_basis: Val::Px(0.0),
                                min_height: Val::Px(0.0),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(4.0),
                                overflow: Overflow::scroll_y(),
                                ..default()
                            },
                        ))
                        .with_children(|list| {
                            if panel.tab == Tab::Spells {
                                let mut reference = Run::default();
                                reference.balance = run.balance.clone();
                                for (index, class) in data::CLASSES
                                    .iter()
                                    .enumerate()
                                    .filter(|(i, _)| panel.class.is_none_or(|c| c == *i))
                                {
                                    let _ = index;
                                    reference.class = class;
                                    list.spawn((
                                        text(&font.0, class.name, 19.0, hud::GOLD_TONE),
                                        Node {
                                            margin: UiRect::top(Val::Px(8.0)),
                                            flex_shrink: 0.0,
                                            ..default()
                                        },
                                    ));
                                    for key in spell_keys(class) {
                                        let def = data::ability(key);
                                        let p = reference.resolve_spell(def, 1);
                                        let card = hud::describe_card(
                                            hud::Choice::NewAbility(def),
                                            &reference,
                                            spells.as_deref(),
                                        );
                                        let lines = vec![
                                            class.name.into(),
                                            p.tags.labels(),
                                            card.desc,
                                            format!("Cooldown: {:.1} s", p.cooldown),
                                        ];
                                        entry(list, &font.0, def.spell, card.name, lines, vec![]);
                                    }
                                }
                            } else {
                                for p in data::PASSIVES {
                                    let rarities = passive_rarities(p, &run, &offer.config);
                                    if rarities.is_empty() {
                                        continue;
                                    }
                                    let lines = rarities
                                        .iter()
                                        .map(|&i| {
                                            format!(
                                                "{}: {}",
                                                RARITIES[i].1,
                                                hud::passive_effect_text(
                                                    p,
                                                    if super::passives::legendary(p) {
                                                        1
                                                    } else {
                                                        (i + 1) as u64
                                                    }
                                                )
                                            )
                                        })
                                        .collect();
                                    let badges = rarities
                                        .iter()
                                        .map(|&i| (RARITIES[i].1.to_owned(), RARITIES[i].0.color()))
                                        .collect();
                                    entry(list, &font.0, p.spell, p.name.into(), lines, badges);
                                }
                            }
                        });
                });
        });
}
fn scroll(
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    mut panel: ResMut<Panel>,
    mut rows: Query<(&ComputedNode, &RelativeCursorPosition, &mut ScrollPosition), With<Body>>,
) {
    if !panel.open {
        wheel.clear();
        return;
    }
    for event in wheel.read() {
        for (node, cursor, mut p) in &mut rows {
            if cursor.cursor_over() {
                let step = if event.unit == bevy::input::mouse::MouseScrollUnit::Line {
                    36.0
                } else {
                    1.0
                };
                p.0.y = (p.0.y - event.y * step).clamp(
                    0.0,
                    ((node.content_size.y - node.size.y) * node.inverse_scale_factor).max(0.0),
                );
                panel.scroll = p.0.y;
            }
        }
    }
}
fn tooltip(
    mut commands: Commands,
    panel: Res<Panel>,
    font: Option<Res<hud::HudFont>>,
    hover: Query<(&Interaction, &Tip)>,
    roots: Query<Entity, With<Tooltip>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    scale: Option<Res<UiScale>>,
) {
    for e in &roots {
        commands.entity(e).despawn();
    }
    if !panel.open {
        return;
    }
    let Some(font) = font else { return };
    let Some((_, tip)) = hover.iter().find(|(i, _)| **i != Interaction::None) else {
        return;
    };
    let inv = scale.as_deref().map_or(1.0, |s| s.0).max(0.01);
    let (viewport, cursor) =
        windows
            .single()
            .map_or((Vec2::new(1280.0, 768.0), Vec2::new(700.0, 120.0)), |w| {
                (
                    Vec2::new(w.width(), w.height()) / inv,
                    w.cursor_position().unwrap_or_default() / inv,
                )
            });
    let x = if cursor.x + 380.0 > viewport.x {
        cursor.x - 370.0
    } else {
        cursor.x + 16.0
    };
    let height = 60.0
        + tip
            .lines
            .iter()
            .map(|l| (l.chars().count() / 42 + 1 + l.matches('\n').count()) as f32 * 18.0)
            .sum::<f32>();
    commands
        .spawn((
            Tooltip,
            GlobalZIndex(850),
            bevy::ui::FocusPolicy::Pass,
            BackgroundColor(Color::srgb(0.02, 0.025, 0.04)),
            BorderColor::all(hud::GOLD_TONE),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(x.max(4.0)),
                top: Val::Px((cursor.y + 16.0).min((viewport.y - height - 8.0).max(4.0))),
                width: Val::Px(350.0),
                max_height: Val::Vh(90.0),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(7.0),
                ..default()
            },
        ))
        .with_children(|body| {
            body.spawn(text(&font.0, &tip.name, 18.0, hud::GOLD_TONE));
            for line in &tip.lines {
                body.spawn(text(&font.0, line, 14.0, Color::WHITE));
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalogue_contains_every_class_spell_and_groups_all_passive_rarities() {
        let run = Run::default();
        let config = super::super::reward_config::Config::builtin();
        for class in data::CLASSES {
            let keys = spell_keys(class);
            assert!(keys.contains(&class.start));
            assert!(class.pool.iter().all(|key| keys.contains(key)));
            assert_eq!(
                keys.iter().collect::<std::collections::BTreeSet<_>>().len(),
                keys.len()
            );
        }
        for passive in data::PASSIVES {
            let rarities = passive_rarities(passive, &run, &config);
            if config.card_weight(hud::Choice::NewPassive(passive), &run) == 0.0 {
                assert!(rarities.is_empty());
                continue;
            }
            let expected = if super::super::passives::legendary(passive) {
                vec![3]
            } else {
                vec![0, 1, 2, 3]
            };
            assert_eq!(rarities, expected, "{}", passive.key);
        }
    }
    #[test]
    fn escape_menu_opens_codex_and_both_pages_build_and_escape_returns_to_pause() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(State::new(Phase::Paused))
            .init_resource::<NextState<Phase>>()
            .init_resource::<Panel>()
            .init_resource::<Run>()
            .init_resource::<hud::Offer>()
            .init_resource::<super::super::Selection>()
            .init_resource::<super::super::options::Panel>()
            .init_resource::<super::super::options::Settings>()
            .init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(hud::HudFont(Handle::default()))
            .add_systems(Update, (input, render, tooltip, hud::keys).chain());
        let button = app
            .world_mut()
            .spawn((Interaction::Pressed, hud::UiAction::Codex))
            .id();
        app.update();
        assert!(app.world().resource::<Panel>().open);
        let world = app.world_mut();
        let spells = world.query::<&Tip>().iter(world).count();
        assert_eq!(
            spells,
            data::CLASSES
                .iter()
                .map(|c| spell_keys(c).len())
                .sum::<usize>()
        );
        world.despawn(button);
        world.resource_mut::<Panel>().tab = Tab::Passives;
        app.update();
        let world = app.world_mut();
        let expected = data::PASSIVES
            .iter()
            .filter(|p| {
                !passive_rarities(
                    p,
                    world.resource::<Run>(),
                    &world.resource::<hud::Offer>().config,
                )
                .is_empty()
            })
            .count();
        assert_eq!(world.query::<&Tip>().iter(world).count(), expected);
        let row = world
            .query_filtered::<Entity, With<Tip>>()
            .iter(world)
            .next()
            .unwrap();
        world.entity_mut(row).insert(Interaction::Hovered);
        app.update();
        let world = app.world_mut();
        assert_eq!(
            world
                .query_filtered::<Entity, With<Tooltip>>()
                .iter(world)
                .count(),
            1
        );
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        assert!(!app.world().resource::<Panel>().open);
        assert!(matches!(
            app.world().resource::<NextState<Phase>>(),
            NextState::Unchanged
        ));
        let world = app.world_mut();
        assert_eq!(
            world
                .query_filtered::<Entity, With<Root>>()
                .iter(world)
                .count(),
            0
        );
    }
}
