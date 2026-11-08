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
    hovered: Query<(&Interaction, &Action)>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
) {
    if !matches!(
        *phase.get(),
        Phase::Hub | Phase::Playing | Phase::CharacterSheet
    ) {
        selection.talents_open = false;
        return;
    }
    if settings.just_pressed(super::options::Action::Talents, &keys)
        && !settings.just_pressed(super::options::Action::DrawCards, &keys) {
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
    let right_click = mouse
        .as_ref()
        .is_some_and(|m| m.just_pressed(MouseButton::Right));
    if right_click && selection.talents_open && editable(*phase.get(), &selection) {
        if let Some((tree, node)) = hovered
            .iter()
            .find_map(|(interaction, action)| match action {
                Action::Buy(tree, node) if *interaction != Interaction::None => {
                    Some((*tree, *node))
                }
                _ => None,
            })
        {
            let class = view.class.unwrap_or(0);
            if account.refund(class, tree, node) {
                apply_refund(&account, class, &mut run);
                account.save();
            }
        }
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
                if !right_click && selection.talents_open && editable(*phase.get(), &selection) =>
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

fn apply_refund(account: &Account, class: usize, run: &mut super::Run) {
    let previous: std::collections::HashMap<_, _> = run
        .abilities
        .iter()
        .map(|a| (a.def.key, (a.level, a.cd)))
        .collect();
    let starter = run.starter_spell.unwrap_or(run.class.start);
    run.abilities.retain(|a| a.def.key == starter);
    if run.abilities.is_empty() {
        run.starter_spell = None;
        run.abilities.push(super::Owned {
            def: data::ability(run.class.start),
            level: 1,
            cd: 0.5,
        });
    }
    account.apply(class, run);
    for owned in &mut run.abilities {
        if let Some(&(level, cd)) = previous.get(owned.def.key) {
            owned.level = owned.level.max(level);
            owned.cd = owned.cd.min(cd);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn partial_refunds_preserve_starting_spell_and_ranks_and_remove_revoked_unlocks() {
        let mut build = [[0; NODES]; 3];
        build[1][0] = 5;
        build[1][1] = 5;
        build[1][3] = 1;
        build[1][8] = 1;
        let mut account = Account::from_shared(8, u64::MAX, build);
        let mut run = super::super::Run {
            class: &data::CLASSES[8],
            ..Default::default()
        };
        run.abilities.push(super::super::Owned {
            def: data::ability("wrath"),
            level: 4,
            cd: 0.3,
        });
        account.apply(8, &mut run);
        assert_eq!(run.abilities[0].def.key, "claw");
        assert!(account.refund(8, 1, 0));
        apply_refund(&account, 8, &mut run);
        assert_eq!(run.abilities[0].def.key, "claw");
        assert_eq!(run.abilities[0].level, 4);
        assert!(account.refund(8, 1, 8));
        apply_refund(&account, 8, &mut run);
        assert_eq!(run.abilities[0].def.key, "wrath");
        assert_eq!(run.abilities[0].level, 4);
        assert!(!run
            .abilities
            .iter()
            .any(|a| matches!(a.def.key, "claw" | "swipe")));
    }
    #[test]
    fn right_click_refunds_one_point_in_hub_and_keeps_run_talents_read_only() {
        for hub in [false, true] {
            let mut app = pause_app(hub);
            let mut build = [[0; NODES]; 3];
            build[0][0] = 2;
            build[1][0] = 1;
            app.world_mut()
                .insert_resource(Account::from_shared(0, u64::MAX, build));
            key(&mut app, KeyCode::KeyN);
            let available = app.world().resource::<Account>().available(0);
            app.world_mut()
                .spawn((Interaction::Hovered, Action::Buy(0, 0)));
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(MouseButton::Right);
            app.update();
            let account = app.world().resource::<Account>();
            assert_eq!(account.rank(0, 0, 0), if hub { 1 } else { 2 });
            assert_eq!(account.rank(0, 1, 0), 1);
            assert_eq!(account.available(0), available + u32::from(hub));
            if hub {
                assert_eq!(
                    app.world().resource::<super::super::Run>().talent_build[0][0],
                    1
                );
                assert_eq!(
                    app.world().resource::<super::super::Run>().talent_build[1][0],
                    1
                );
            }
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .clear_just_pressed(MouseButton::Right);
            app.update();
            assert_eq!(
                app.world().resource::<Account>().rank(0, 0, 0),
                if hub { 1 } else { 2 }
            );
        }
    }
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
            for close in [Some(KeyCode::KeyN), Some(KeyCode::Escape), None] {
                key(&mut app, KeyCode::KeyN);
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
            KeyCode::KeyN,
            KeyCode::KeyC,
            KeyCode::KeyN,
            KeyCode::KeyB,
            KeyCode::KeyN,
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
        assert_eq!(world.query::<&Root>().iter(world).count(), 1);
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
