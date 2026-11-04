//! Character-selection lobby intent and direct IPv4 connection UI.
use bevy::prelude::*;

#[derive(Clone, Debug)]
pub(super) enum Request {
    Solo,
    Host,
    Join(String),
    Leave,
    Cancel,
}

#[derive(Resource, Default)]
pub(in crate::survivors) struct Lobby {
    pub joining: bool,
    pub busy: bool,
    pub input: String,
    pub active: bool,
    pub error: String,
    pub status: String,
    pub members: usize,
    pub revision: u64,
    pub(super) command_line: bool,
    pub(super) request: Option<Request>,
}

impl Lobby {
    pub(in crate::survivors) fn solo(&mut self) {
        if !self.command_line {
            self.request = Some(Request::Solo);
        }
    }
    pub(in crate::survivors) fn host(&mut self) {
        self.busy = true;
        self.error.clear();
        self.request = Some(Request::Host);
        self.revision += 1;
    }
    pub(in crate::survivors) fn open_join(&mut self) {
        self.joining = true;
        self.error.clear();
        self.status.clear();
        self.revision += 1;
    }
    pub(super) fn fail(&mut self, error: impl Into<String>) {
        self.busy = false;
        self.error = error.into();
        self.revision += 1;
    }
    pub(super) fn members(&mut self, members: usize) {
        if self.members != members {
            self.members = members;
            self.revision += 1;
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Lobby>();
    #[cfg(not(target_arch = "wasm32"))]
    app.add_systems(Update, input.after(super::super::characters::input))
        .add_systems(Update, draw.after(input))
        .add_systems(
            Update,
            capture
                .after(super::super::combat_ui::input)
                .before(benilla_world::schedule::WorldStage::Input),
        );
}

#[cfg(not(target_arch = "wasm32"))]
fn capture(
    buttons: Query<&Interaction, With<Action>>,
    mut selection: ResMut<super::super::Selection>,
) {
    selection.ui_captured |= buttons.iter().any(|i| *i != Interaction::None);
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Component, Clone, Copy)]
pub(super) enum Action {
    Join,
    Cancel,
    Paste,
    Leave,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Component)]
struct Overlay;

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn input(
    mut lobby: ResMut<Lobby>,
    mut keyboard: MessageReader<bevy::input::keyboard::KeyboardInput>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Query<(&Interaction, &Action), Changed<Interaction>>,
    mut roster: ResMut<super::super::characters::Roster>,
    mut selection: ResMut<super::super::Selection>,
    mut account: ResMut<super::super::talents::Account>,
) {
    let mut actions: Vec<Action> = buttons
        .iter()
        .filter(|(i, _)| **i == Interaction::Pressed)
        .map(|(_, a)| *a)
        .collect();
    if lobby.joining {
        if !lobby.busy {
            for event in keyboard.read().filter(|e| e.state.is_pressed()) {
                if let Some(text) = &event.text {
                    for c in text.chars().filter(|c| c.is_ascii_digit() || *c == '.') {
                        if lobby.input.len() < 15 {
                            lobby.input.push(c);
                            lobby.revision += 1;
                        }
                    }
                }
            }
            if keys.just_pressed(KeyCode::Backspace) {
                lobby.input.pop();
                lobby.revision += 1;
            }
            if keys.just_pressed(KeyCode::Enter) {
                actions.push(Action::Join);
            }
            if keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight])
                && keys.just_pressed(KeyCode::KeyV)
            {
                actions.push(Action::Paste);
            }
        } else {
            keyboard.clear();
        }
        if keys.just_pressed(KeyCode::Escape) {
            actions.push(Action::Cancel);
        }
    } else {
        keyboard.clear();
    }
    for action in actions {
        match action {
            Action::Join if !lobby.busy => {
                if let Err(error) = super::native::address::parse(&lobby.input) {
                    lobby.fail(error);
                    continue;
                }
                if let Err(error) = super::super::characters::activate_selected(
                    &mut roster,
                    &mut selection,
                    &mut account,
                ) {
                    lobby.fail(error);
                    continue;
                }
                lobby.busy = true;
                lobby.error.clear();
                lobby.status = "Connexion au salon…".into();
                lobby.request = Some(Request::Join(lobby.input.clone()));
                lobby.revision += 1;
            }
            Action::Cancel => {
                lobby.request = Some(Request::Cancel);
                lobby.joining = false;
                lobby.revision += 1;
            }
            Action::Paste if !lobby.busy => {
                match arboard::Clipboard::new().and_then(|mut c| c.get_text()) {
                    Ok(text) => {
                        lobby.input = text.trim().chars().take(80).collect();
                        lobby.error.clear();
                        lobby.revision += 1;
                    }
                    Err(_) => lobby.fail("Impossible de lire le presse-papiers."),
                }
            }
            Action::Leave => {
                lobby.request = Some(Request::Leave);
            }
            _ => {}
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn draw(
    mut commands: Commands,
    lobby: Res<Lobby>,
    phase: Res<State<super::super::Phase>>,
    font: Option<Res<super::super::hud::HudFont>>,
    roots: Query<Entity, With<Overlay>>,
    mut shown: Local<Option<(super::super::Phase, u64)>>,
) {
    use super::super::{combat_ui::skin, hud, Phase};
    let Some(font) = font else {
        return;
    };
    let key = (*phase.get(), lobby.revision);
    if *shown == Some(key) {
        return;
    }
    *shown = Some(key);
    for root in &roots {
        commands.entity(root).despawn();
    }
    let f = &font.0;
    if lobby.joining && *phase.get() == Phase::Menu {
        commands
            .spawn((
                Overlay,
                Button,
                GlobalZIndex(900),
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.8)),
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
                        width: Val::Px(560.0),
                        max_width: Val::Percent(94.0),
                        padding: UiRect::all(Val::Px(24.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(16.0),
                        ..default()
                    })
                    .with_children(|ui| {
                        skin::backdrop(ui);
                        ui.spawn((
                            Text::new("Rejoindre un salon"),
                            TextFont {
                                font: f.clone(),
                                font_size: 24.0,
                                ..default()
                            },
                            TextColor(hud::GOLD_TONE),
                        ));
                        ui.spawn((
                            Text::new("Entrez l'IPv4 Hamachi de l'hôte (exemple : 25.12.34.56)."),
                            TextFont {
                                font: f.clone(),
                                font_size: 16.0,
                                ..default()
                            },
                        ));
                        ui.spawn((
                            Text::new(format!("{}|", lobby.input)),
                            TextFont {
                                font: f.clone(),
                                font_size: 20.0,
                                ..default()
                            },
                            BackgroundColor(Color::BLACK),
                            Node {
                                min_height: Val::Px(48.0),
                                padding: UiRect::all(Val::Px(12.0)),
                                ..default()
                            },
                        ));
                        if !lobby.error.is_empty() {
                            ui.spawn((
                                Text::new(&lobby.error),
                                TextFont {
                                    font: f.clone(),
                                    font_size: 15.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(1.0, 0.35, 0.25)),
                            ));
                        }
                        if !lobby.status.is_empty() {
                            ui.spawn((
                                Text::new(&lobby.status),
                                TextFont {
                                    font: f.clone(),
                                    font_size: 15.0,
                                    ..default()
                                },
                            ));
                        }
                        if !lobby.busy {
                            skin::button(ui, f, "Coller l'IPv4", Action::Paste, 240.0);
                            skin::button(ui, f, "Rejoindre", Action::Join, 240.0);
                        }
                        skin::button(ui, f, "Annuler", Action::Cancel, 240.0);
                    });
            });
    } else if lobby.active && *phase.get() != Phase::Menu {
        commands
            .spawn((
                Overlay,
                GlobalZIndex(650),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(18.0),
                    bottom: Val::Px(18.0),
                    width: Val::Px(380.0),
                    padding: UiRect::all(Val::Px(12.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    ..default()
                },
            ))
            .with_children(|ui| {
                skin::backdrop(ui);
                ui.spawn((
                    Text::new(format!("Salon · {}/8 joueurs", lobby.members)),
                    TextFont {
                        font: f.clone(),
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(hud::GOLD_TONE),
                ));
                ui.spawn((
                    Text::new("Connexion directe : IPv4 Hamachi / réseau local"),
                    TextFont {
                        font: f.clone(),
                        font_size: 16.0,
                        ..default()
                    },
                ));
                if !lobby.status.is_empty() {
                    ui.spawn((
                        Text::new(&lobby.status),
                        TextFont {
                            font: f.clone(),
                            font_size: 13.0,
                            ..default()
                        },
                    ));
                }
                ui.spawn(Node {
                    column_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|row| {
                    skin::button(row, f, "Quitter le salon", Action::Leave, 170.0);
                });
            });
    } else if !lobby.error.is_empty() && *phase.get() == Phase::Menu {
        commands.spawn((
            Overlay,
            GlobalZIndex(850),
            Text::new(&lobby.error),
            TextFont {
                font: f.clone(),
                font_size: 16.0,
                ..default()
            },
            TextColor(Color::srgb(1.0, 0.35, 0.25)),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(24.0),
                bottom: Val::Px(24.0),
                max_width: Val::Px(450.0),
                ..default()
            },
        ));
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::survivors::{characters, hud, talents, Phase, Run, Selection};
    use bevy::ecs::system::RunSystemOnce;

    fn world() -> World {
        let mut world = World::new();
        world.init_resource::<Lobby>();
        world.insert_resource(State::new(Phase::Menu));
        world.init_resource::<NextState<Phase>>();
        world.init_resource::<Selection>();
        world.init_resource::<Run>();
        world.init_resource::<characters::Roster>();
        world.init_resource::<talents::Account>();
        world.init_resource::<ButtonInput<KeyCode>>();
        world.init_resource::<Messages<bevy::input::keyboard::KeyboardInput>>();
        world.insert_resource(hud::HudFont(Handle::default()));
        world
    }

    #[test]
    fn join_dialog_has_paste_confirm_and_cancel_and_a_busy_dialog_stays_cancellable() {
        let mut world = world();
        world.resource_mut::<Lobby>().open_join();
        world.run_system_once(draw).unwrap();
        let actions: Vec<_> = world.query::<&Action>().iter(&world).copied().collect();
        assert_eq!(
            actions.iter().filter(|a| matches!(a, Action::Join)).count(),
            1
        );
        assert_eq!(
            actions
                .iter()
                .filter(|a| matches!(a, Action::Paste))
                .count(),
            1
        );
        assert_eq!(
            actions
                .iter()
                .filter(|a| matches!(a, Action::Cancel))
                .count(),
            1
        );
        world.resource_mut::<Lobby>().busy = true;
        world.run_system_once(draw).unwrap();
        let actions: Vec<_> = world.query::<&Action>().iter(&world).copied().collect();
        assert_eq!(actions.len(), 1);
        assert!(matches!(actions[0], Action::Cancel));
    }

    #[test]
    fn enter_in_the_ipv4_dialog_cannot_start_a_solo_game() {
        let mut world = world();
        world.resource_mut::<Lobby>().open_join();
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        world.run_system_once(characters::input).unwrap();
        assert!(matches!(
            world.resource::<NextState<Phase>>(),
            NextState::Unchanged
        ));
        world.run_system_once(input).unwrap();
        assert!(!world.resource::<Lobby>().error.is_empty());
        assert!(!world.resource::<Lobby>().busy);
        assert!(world.resource::<Lobby>().request.is_none());
        assert!(matches!(
            world.resource::<NextState<Phase>>(),
            NextState::Unchanged
        ));
    }
}
