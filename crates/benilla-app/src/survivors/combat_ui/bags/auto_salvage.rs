//! Modal editor for per-character automatic item sales.
use super::*;
use crate::survivors::items::{AutoSalvageRule, Rarity};
use bevy::input::keyboard::KeyboardInput;

#[derive(Resource, Default)]
pub(super) struct Editor {
    pub open: bool,
    rules: [AutoSalvageRule; 4],
    fields: [String; 4],
    focus: Option<usize>,
    replace: bool,
    error: String,
}
#[derive(Component)]
struct Root;
#[derive(Component, Clone, Copy)]
enum Click {
    Open,
    Toggle(usize),
    Field(usize),
    Save,
    Cancel,
}

pub(super) fn button(ui: &mut ChildSpawnerCommands, font: &Handle<Font>) {
    skin::button_sized(
        ui,
        font,
        "Auto-salvage",
        Click::Open,
        118.0,
        24.0,
        12.0,
        false,
    );
}
fn input(
    mut editor: ResMut<Editor>,
    mut inventory: Option<ResMut<Inventory>>,
    selection: Res<Selection>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut typing: MessageReader<KeyboardInput>,
    clicks: Query<(&Interaction, &Click), Changed<Interaction>>,
) {
    if !selection.bags_open {
        editor.open = false;
        editor.focus = None;
    }
    let Some(inventory) = inventory.as_deref_mut() else {
        return;
    };
    for (interaction, click) in &clicks {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *click {
            Click::Open if selection.bags_open => {
                editor.rules = inventory.auto_salvage();
                editor.fields = editor.rules.map(|r| r.below_ilvl.to_string());
                editor.open = true;
                editor.focus = None;
                editor.error.clear();
            }
            Click::Toggle(i) if editor.open => editor.rules[i].enabled = !editor.rules[i].enabled,
            Click::Field(i) if editor.open => {
                editor.focus = Some(i);
                editor.replace = true;
            }
            Click::Cancel => {
                editor.open = false;
                editor.focus = None;
            }
            Click::Save if editor.open => {
                let mut rules = editor.rules;
                let mut valid = true;
                for (i, field) in editor.fields.iter().enumerate() {
                    match field.parse::<u32>() {
                        Ok(value) if value <= AutoSalvageRule::MAX_BELOW_ILVL => {
                            rules[i].below_ilvl = value
                        }
                        _ => valid = false,
                    }
                }
                if !valid {
                    editor.error = format!(
                        "Enter a threshold between 0 and {} for each rarity.",
                        AutoSalvageRule::MAX_BELOW_ILVL
                    );
                } else {
                    match inventory.set_auto_salvage(rules) {
                        Ok(()) => {
                            editor.open = false;
                            editor.focus = None;
                        }
                        Err(error) => editor.error = error,
                    }
                }
            }
            _ => {}
        }
    }
    if !editor.open {
        typing.clear();
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        editor.open = false;
        editor.focus = None;
    }
    if let Some(index) = editor.focus {
        if keys.just_pressed(KeyCode::Backspace) {
            if editor.replace {
                editor.fields[index].clear();
                editor.replace = false;
            } else {
                editor.fields[index].pop();
            }
        }
        for event in typing.read().filter(|e| e.state.is_pressed()) {
            if let Some(text) = &event.text {
                for ch in text.chars().filter(char::is_ascii_digit) {
                    if editor.replace {
                        editor.fields[index].clear();
                        editor.replace = false;
                    }
                    if editor.fields[index].len() < 3 {
                        editor.fields[index].push(ch);
                    }
                }
            }
        }
        if keys.just_pressed(KeyCode::Tab) {
            editor.focus = Some((index + 1) % 4);
            editor.replace = true;
        }
    } else {
        typing.clear();
    }
    // Numeric entry must not trigger movement, inventory hotkeys or run controls.
    keys.reset_all();
}
fn capture(editor: Res<Editor>, mut selection: ResMut<Selection>) {
    if editor.open {
        selection.ui_captured = true;
        selection.pointer_captured = true;
    }
}
fn render(
    mut commands: Commands,
    editor: Res<Editor>,
    font: Option<Res<hud::HudFont>>,
    roots: Query<Entity, With<Root>>,
    mut cache: Local<String>,
) {
    let key = format!(
        "{} {:?} {:?} {:?} {}",
        editor.open, editor.rules, editor.fields, editor.focus, editor.error
    );
    if *cache == key {
        return;
    }
    *cache = key;
    for root in &roots {
        commands.entity(root).despawn();
    }
    let Some(font) = font.filter(|_| editor.open) else {
        return;
    };
    let f = &font.0;
    commands
        .spawn((
            Root,
            GlobalZIndex(950),
            Button,
            bevy::ui::FocusPolicy::Block,
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    BackgroundColor(Color::srgb(0.025, 0.022, 0.018)),
                    Node {
                        width: Val::Px(460.0),
                        max_width: Val::Percent(95.0),
                        padding: UiRect::all(Val::Px(20.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(10.0),
                        ..default()
                    },
                ))
                .with_children(|panel| {
                    skin::backdrop(panel);
                    panel.spawn(text(f, "Auto-salvage", 22.0, GOLD));
                    panel.spawn(text(
                        f,
                        "Sell items with an item level below the threshold.",
                        13.0,
                        Color::WHITE,
                    ));
                    panel.spawn(text(
                        f,
                        "Applies to bag items and new loot. Equipped items are kept.",
                        12.0,
                        GREY,
                    ));
                    for (i, (name, rarity)) in [
                        ("Normal", Rarity::Normal),
                        ("Magic", Rarity::Magic),
                        ("Epic", Rarity::Epic),
                        ("Legendary", Rarity::Legendary),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        panel
                            .spawn(Node {
                                width: Val::Percent(100.0),
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(10.0),
                                ..default()
                            })
                            .with_children(|row| {
                                row.spawn((
                                    Button,
                                    Click::Toggle(i),
                                    Node {
                                        width: Val::Px(28.0),
                                        height: Val::Px(28.0),
                                        ..default()
                                    },
                                ))
                                .with_children(|check| {
                                    skin::art(
                                        check,
                                        "Interface\\Buttons\\UI-CheckBox-Up",
                                        skin::rect(0.0, 0.0, 28.0, 28.0),
                                        [0.0, 1.0, 0.0, 1.0],
                                    );
                                    if editor.rules[i].enabled {
                                        skin::art(
                                            check,
                                            "Interface\\Buttons\\UI-CheckBox-Check",
                                            skin::rect(0.0, 0.0, 28.0, 28.0),
                                            [0.0, 1.0, 0.0, 1.0],
                                        );
                                    }
                                });
                                row.spawn((
                                    text(f, name, 16.0, rarity.color()),
                                    Node {
                                        flex_grow: 1.0,
                                        ..default()
                                    },
                                ));
                                row.spawn((
                                    Button,
                                    Click::Field(i),
                                    BackgroundColor(Color::BLACK),
                                    BorderColor::all(if editor.focus == Some(i) {
                                        GOLD
                                    } else {
                                        GREY
                                    }),
                                    Node {
                                        width: Val::Px(82.0),
                                        height: Val::Px(30.0),
                                        border: UiRect::all(Val::Px(1.0)),
                                        padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                                        ..default()
                                    },
                                ))
                                .with_children(|field| {
                                    field.spawn(text(
                                        f,
                                        format!(
                                            "{}{}",
                                            editor.fields[i],
                                            if editor.focus == Some(i) { "|" } else { "" }
                                        ),
                                        16.0,
                                        Color::WHITE,
                                    ));
                                });
                            });
                    }
                    if !editor.error.is_empty() {
                        panel.spawn(text(f, &editor.error, 12.0, Color::srgb(1.0, 0.3, 0.2)));
                    }
                    panel.spawn(text(
                        f,
                        "Saving also applies these rules to items already in your bags.",
                        12.0,
                        GREY,
                    ));
                    panel
                        .spawn(Node {
                            justify_content: JustifyContent::SpaceBetween,
                            ..default()
                        })
                        .with_children(|footer| {
                            skin::button(footer, f, "Cancel", Click::Cancel, 125.0);
                            skin::button(footer, f, "Save", Click::Save, 145.0);
                        });
                });
        });
}
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Editor>()
        .add_message::<KeyboardInput>()
        .add_systems(Update, input.before(crate::survivors::combat_ui::input))
        .add_systems(
            Update,
            capture
                .after(crate::survivors::combat_ui::input)
                .before(crate::survivors::client::input),
        )
        .add_systems(Update, render.after(input));
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        ecs::system::RunSystemOnce,
        input::{keyboard::Key, ButtonState},
    };
    fn world() -> World {
        let mut world = World::new();
        world.init_resource::<Editor>();
        world.insert_resource(Inventory::fixture(vec![]));
        world.insert_resource(Selection {
            bags_open: true,
            ..default()
        });
        world.init_resource::<ButtonInput<KeyCode>>();
        world.init_resource::<Messages<KeyboardInput>>();
        world
    }
    fn click(world: &mut World, action: Click) {
        let entity = world.spawn((Interaction::Pressed, action)).id();
        world.run_system_once(input).unwrap();
        world.despawn(entity);
    }
    #[test]
    fn editor_replaces_numeric_field_and_saves_checked_thresholds() {
        let mut world = world();
        click(&mut world, Click::Open);
        click(&mut world, Click::Toggle(0));
        click(&mut world, Click::Field(0));
        world.write_message(KeyboardInput {
            key_code: KeyCode::Digit4,
            logical_key: Key::Character("999".into()),
            state: ButtonState::Pressed,
            text: Some("999".into()),
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        world.run_system_once(input).unwrap();
        assert_eq!(world.resource::<Editor>().fields[0], "999");
        world.resource_mut::<Messages<KeyboardInput>>().clear();
        click(&mut world, Click::Save);
        assert!(!world.resource::<Editor>().open);
        assert_eq!(
            world.resource::<Inventory>().auto_salvage()[0],
            AutoSalvageRule {
                enabled: true,
                below_ilvl: 999
            }
        );
    }
    #[test]
    fn cancel_discards_changes_and_invalid_threshold_keeps_editor_open() {
        let mut world = world();
        click(&mut world, Click::Open);
        world.resource_mut::<Editor>().fields[0] = "1000".into();
        click(&mut world, Click::Save);
        assert!(world.resource::<Editor>().open);
        assert!(!world.resource::<Editor>().error.is_empty());
        click(&mut world, Click::Cancel);
        assert_eq!(
            world.resource::<Inventory>().auto_salvage(),
            [AutoSalvageRule::default(); 4]
        );
    }
}
