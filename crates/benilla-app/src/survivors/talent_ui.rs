//! Three simultaneous trees, ranked nodes, prerequisite links and free class respecs.
use super::{
    data,
    talents::{self, Account, NODES, TREES},
    Phase, Selection,
};
use bevy::prelude::*;

mod classic;

const GOLD: Color = Color::srgb(1.0, 0.82, 0.2);
#[derive(Component)]
struct Root;
#[derive(Component, Clone, Copy)]
pub(super) enum Action {
    Toggle,
    Buy(usize, usize),
    Reset,
}
#[derive(Resource, Default)]
pub(super) struct View {
    class: Option<usize>,
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<super::options::Settings>()
        .init_resource::<View>()
        .init_resource::<classic::Art>()
        .add_systems(
            Update,
            (
                classic::load_art,
                input
                    .after(super::combat_ui::input)
                    .before(benilla_world::schedule::WorldStage::Input),
                classic::render,
                classic::tooltip,
            )
                .chain(),
        );
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
fn button(action: Action, color: Color) -> impl Bundle {
    (
        Button,
        action,
        BackgroundColor(color),
        BorderColor::all(GOLD),
    )
}

pub(super) fn input(
    phase: Res<State<Phase>>,
    mut next: ResMut<NextState<Phase>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    settings: Res<super::options::Settings>,
    mut selection: ResMut<Selection>,
    mut view: ResMut<View>,
    mut account: ResMut<Account>,
    mut run: ResMut<super::Run>,
    buttons: Query<(&Interaction, &Action), Changed<Interaction>>,
) {
    if !matches!(
        *phase.get(),
        Phase::Hub | Phase::Playing | Phase::CharacterSheet
    ) {
        selection.talents_open = false;
        return;
    }
    if settings.just_pressed(super::options::Action::Talents, &keys) {
        selection.sheet_open = false;
        selection.talents_open = !selection.talents_open;
        if selection.talents_open {
            view.class = selection.class;
        }
    }
    view.class = selection.class;
    if selection.talents_open
        && (keys.just_pressed(KeyCode::Escape)
            || settings.just_pressed(super::options::Action::Pause, &keys))
    {
        selection.talents_open = false;
        keys.clear_just_pressed(KeyCode::Escape);
    }
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *action {
            Action::Toggle => {
                selection.sheet_open = false;
                selection.talents_open = !selection.talents_open;
                if selection.talents_open {
                    view.class = selection.class;
                }
            }
            Action::Buy(tree, node)
                if selection.talents_open && editable(*phase.get(), &selection) =>
            {
                if account.buy(view.class.unwrap_or(0), tree, node) {
                    account.save();
                    account.apply(view.class.unwrap_or(0), &mut run);
                }
            }
            Action::Reset if selection.talents_open && editable(*phase.get(), &selection) => {
                account.reset(view.class.unwrap_or(0));
                let start = run.class.start;
                run.abilities.retain(|a| a.def.key == start);
                account.apply(view.class.unwrap_or(0), &mut run);
                account.save();
            }
            _ => {}
        }
    }
    if selection.talents_open && matches!(*phase.get(), Phase::Hub | Phase::Playing) {
        next.set(Phase::CharacterSheet);
    } else if !selection.talents_open
        && !selection.sheet_open
        && !selection.bags_open
        && *phase.get() == Phase::CharacterSheet
    {
        next.set(if selection.in_hub {
            Phase::Hub
        } else {
            Phase::Playing
        });
    }
}

fn editable(phase: Phase, selection: &Selection) -> bool {
    phase == Phase::Hub || (phase == Phase::CharacterSheet && selection.in_hub)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pause_app(hub: bool) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<Phase>()
            .insert_resource(Selection {
                class: Some(0),
                in_hub: hub,
                ..default()
            })
            .init_resource::<Account>()
            .init_resource::<super::super::Run>()
            .init_resource::<View>()
            .init_resource::<super::super::combat_ui::View>()
            .init_resource::<super::super::characters::Roster>()
            .init_resource::<super::super::options::Settings>()
            .init_resource::<super::super::options::Panel>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_systems(OnEnter(Phase::CharacterSheet), super::super::pause)
            .add_systems(OnEnter(Phase::Playing), super::super::unpause)
            .add_systems(OnEnter(Phase::Hub), super::super::unpause)
            .add_systems(
                Update,
                (
                    super::super::combat_ui::input,
                    input,
                    super::super::hud::keys,
                )
                    .chain(),
            );
        app.world_mut()
            .resource_mut::<NextState<Phase>>()
            .set(if hub { Phase::Hub } else { Phase::Playing });
        app.update();
        app
    }
    fn key(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.update();
    }
    #[test]
    fn talents_pause_and_resume_with_keyboard_or_close_button_in_run_and_hub() {
        for hub in [false, true] {
            let mut app = pause_app(hub);
            for close in [Some(KeyCode::KeyT), Some(KeyCode::Escape), None] {
                key(&mut app, KeyCode::KeyT);
                assert_eq!(
                    *app.world().resource::<State<Phase>>().get(),
                    Phase::CharacterSheet
                );
                assert!(app.world().resource::<Time<Virtual>>().is_paused());
                assert!(app.world().resource::<Selection>().talents_open);
                assert_eq!(
                    editable(Phase::CharacterSheet, app.world().resource::<Selection>()),
                    hub
                );
                if let Some(close) = close {
                    key(&mut app, close)
                } else {
                    let button = app
                        .world_mut()
                        .spawn((Interaction::Pressed, Action::Toggle))
                        .id();
                    app.update();
                    app.world_mut().despawn(button);
                    app.update();
                }
                assert_eq!(
                    *app.world().resource::<State<Phase>>().get(),
                    if hub { Phase::Hub } else { Phase::Playing }
                );
                assert!(!app.world().resource::<Time<Virtual>>().is_paused());
                assert!(!app.world().resource::<Selection>().talents_open);
            }
        }
    }
    #[test]
    fn switching_between_character_bags_and_talents_keeps_the_game_paused() {
        let mut app = pause_app(false);
        for keycode in [
            KeyCode::KeyT,
            KeyCode::KeyC,
            KeyCode::KeyT,
            KeyCode::KeyB,
            KeyCode::KeyT,
        ] {
            key(&mut app, keycode);
            assert_eq!(
                *app.world().resource::<State<Phase>>().get(),
                Phase::CharacterSheet
            );
            assert!(app.world().resource::<Time<Virtual>>().is_paused());
            let selection = app.world().resource::<Selection>();
            assert!(selection.talents_open || selection.sheet_open || selection.bags_open);
            assert!(!(selection.talents_open && selection.sheet_open));
        }
        key(&mut app, KeyCode::Escape);
        assert_eq!(
            *app.world().resource::<State<Phase>>().get(),
            Phase::Playing
        );
        assert!(!app.world().resource::<Time<Virtual>>().is_paused());
    }
    #[test]
    fn the_panel_builds_three_simultaneous_trees() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<Phase>()
            .insert_resource(Selection {
                talents_open: true,
                in_hub: true,
                ..default()
            })
            .init_resource::<Account>()
            .init_resource::<super::super::Run>()
            .init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(super::super::hud::HudFont(Handle::default()))
            .add_plugins(plugin);
        app.world_mut()
            .resource_mut::<NextState<Phase>>()
            .set(Phase::Hub);
        app.update();
        let world = app.world_mut();
        assert_eq!(
            world
                .query::<&Action>()
                .iter(world)
                .filter(|a| matches!(a, Action::Buy(_, _)))
                .count(),
            NODES * 3
        );
        assert_eq!(world.query::<&Root>().iter(world).count(), 2);
        assert_eq!(
            world
                .query::<&Action>()
                .iter(world)
                .filter(|a| matches!(a, Action::Buy(_, 18)))
                .count(),
            3
        );
        world.resource_mut::<Account>().xp += 1;
        app.update();
        let world = app.world_mut();
        assert_eq!(
            world
                .query::<&Action>()
                .iter(world)
                .filter(|a| matches!(a, Action::Buy(1, _)))
                .count(),
            NODES
        );
        assert!(world
            .query::<&Action>()
            .iter(world)
            .any(|a| matches!(a, Action::Buy(0, _) | Action::Buy(2, _))));
    }
}
