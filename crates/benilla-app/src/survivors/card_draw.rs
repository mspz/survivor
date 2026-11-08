//! Deferred rewards stay outside pending_levels, which protects an active draft in combat.
use super::{hud, multiplayer, options, Phase, Run, Selection};
use bevy::prelude::*;

#[derive(Resource, Default)]
pub(super) struct Request(pub bool);
#[derive(Component)]
struct DrawButton;
#[derive(Component)]
struct DrawLabel;

pub(super) fn bank(run: &mut Run) {
    if run.auto_draw_cards {
        run.pending_levels = run.pending_levels.saturating_add(1);
    } else {
        run.stored_cards = run.stored_cards.saturating_add(1);
    }
}

/// Both local play and remote requests use the same authoritative transition.
pub(super) fn apply(run: &mut Run, automatic: bool, open: bool) -> bool {
    run.auto_draw_cards = automatic;
    if automatic {
        run.pending_levels = run.pending_levels.saturating_add(run.stored_cards);
        run.stored_cards = 0;
    } else if open
        && run.pending_levels == 0
        && run.stored_cards > 0
        && run.dead_t.is_none()
        && run.hp > 0.0
    {
        run.stored_cards -= 1;
        run.pending_levels = 1;
    }
    run.pending_levels > 0 && run.dead_t.is_none() && run.hp > 0.0
}

fn input(
    phase: Res<State<Phase>>,
    settings: Res<options::Settings>,
    keys: Res<ButtonInput<KeyCode>>,
    mut run: ResMut<Run>,
    mut selection: ResMut<Selection>,
    mut next: ResMut<NextState<Phase>>,
    role: Option<Res<multiplayer::Role>>,
    mut request: ResMut<Request>,
    buttons: Query<&Interaction, With<DrawButton>>,
) {
    let playing = *phase.get() == Phase::Playing
        && !selection.in_hub
        && !selection.sheet_open
        && !selection.bags_open
        && !selection.talents_open
        && !selection.ui_captured;
    let clicked = buttons.iter().any(|i| *i == Interaction::Pressed);
    if buttons.iter().any(|i| *i != Interaction::None) {
        selection.pointer_captured = true;
    }
    let open = playing && (clicked || settings.just_pressed(options::Action::DrawCards, &keys));
    if role.as_deref() == Some(&multiplayer::Role::Guest) {
        request.0 |= open;
    } else if apply(&mut run, settings.auto_draw_cards, open) && playing {
        next.set(Phase::LevelUp);
    }
}

fn display(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    run: Res<Run>,
    selection: Res<Selection>,
    settings: Res<options::Settings>,
    font: Res<hud::HudFont>,
    mut buttons: Query<&mut Visibility, With<DrawButton>>,
    mut labels: Query<&mut Text, With<DrawLabel>>,
) {
    let visible = *phase.get() == Phase::Playing
        && !selection.in_hub
        && !selection.bags_open
        && !selection.sheet_open
        && !selection.talents_open
        && !selection.ui_captured
        && run.stored_cards > 0
        && run.dead_t.is_none()
        && run.hp > 0.0;
    let key = settings.bindings[options::Action::DrawCards as usize]
        .iter()
        .flatten()
        .next()
        .map(|key| format!("{key:?}").trim_start_matches("Key").to_owned())
        .unwrap_or_else(|| "Click".into());
    let label = format!("Draw card [{key}] - {} available", run.stored_cards);
    for mut text in &mut labels {
        if text.0 != label {
            text.0 = label.clone();
        }
    }
    if let Ok(mut visibility) = buttons.single_mut() {
        *visibility = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        return;
    }
    if !visible {
        return;
    }
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(140.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            GlobalZIndex(440),
        ))
        .with_children(|row| {
            row.spawn((
                DrawButton,
                Button,
                Visibility::Inherited,
                BackgroundColor(Color::srgb(0.15, 0.10, 0.03)),
                BorderColor::all(Color::srgb(1.0, 0.82, 0.2)),
                Node {
                    padding: UiRect::axes(Val::Px(18.0), Val::Px(10.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
            ))
            .with_children(|button| {
                button.spawn((
                    DrawLabel,
                    Text::new(label),
                    TextFont {
                        font: font.0.clone(),
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.82, 0.2)),
                ));
            });
        });
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Request>()
        .add_systems(
            Update,
            input
                .after(super::talent_ui::input)
                .after(super::combat_ui::input)
                .before(benilla_world::schedule::WorldStage::Input),
        )
        .add_systems(Update, display.after(hud::show_panels));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn confirming_a_manual_choice_returns_to_combat_and_preserves_the_reserve() {
        let mut run = Run {
            hp: 100.0,
            stored_cards: 3,
            auto_draw_cards: false,
            ..default()
        };
        assert!(apply(&mut run, false, true));
        let mut offer = hud::Offer {
            choices: vec![hud::Choice::Heal],
            ..default()
        };
        let mut next = NextState::Unchanged;
        hud::pick(
            0,
            &mut run,
            &mut offer,
            &mut super::super::Rng(42),
            &mut next,
        );
        assert_eq!((run.pending_levels, run.stored_cards), (0, 2));
        assert!(offer.choices.is_empty());
        assert!(matches!(next, NextState::Pending(Phase::Playing)));
    }

    #[test]
    fn keyboard_and_button_open_the_same_single_card_and_counter_updates() {
        for click in [false, true] {
            let mut app = App::new();
            let mut settings = options::Settings::defaults();
            settings.auto_draw_cards = false;
            app.insert_resource(State::new(Phase::Playing))
                .init_resource::<NextState<Phase>>()
                .insert_resource(Run {
                    hp: 100.0,
                    stored_cards: 3,
                    auto_draw_cards: false,
                    ..default()
                })
                .init_resource::<Selection>()
                .insert_resource(settings)
                .init_resource::<ButtonInput<KeyCode>>()
                .init_resource::<Request>()
                .insert_resource(hud::HudFont(Handle::default()))
                .add_systems(Update, (input, display).chain());
            app.world_mut().resource_mut::<Selection>().in_hub = false;
            app.update();
            let world = app.world_mut();
            assert!(world
                .query_filtered::<&Text, With<DrawLabel>>()
                .single(world)
                .unwrap()
                .0
                .contains("3 available"));
            if click {
                let entity = world
                    .query_filtered::<Entity, With<DrawButton>>()
                    .single(world)
                    .unwrap();
                world.entity_mut(entity).insert(Interaction::Pressed);
            } else {
                world
                    .resource_mut::<ButtonInput<KeyCode>>()
                    .press(KeyCode::KeyT);
            }
            app.update();
            let world = app.world_mut();
            assert_eq!(
                (
                    world.resource::<Run>().pending_levels,
                    world.resource::<Run>().stored_cards
                ),
                (1, 2)
            );
            assert!(matches!(
                world.resource::<NextState<Phase>>(),
                NextState::Pending(Phase::LevelUp)
            ));
            assert!(world
                .query_filtered::<&Text, With<DrawLabel>>()
                .single(world)
                .unwrap()
                .0
                .contains("2 available"));
        }
    }

    #[test]
    fn manual_cards_accumulate_without_draft_protection_and_open_one_at_a_time() {
        let mut run = Run {
            hp: 100.0,
            ..default()
        };
        assert!(!apply(&mut run, false, false));
        for _ in 0..3 {
            bank(&mut run);
        }
        assert_eq!((run.pending_levels, run.stored_cards), (0, 3));
        assert!(apply(&mut run, false, true));
        assert_eq!((run.pending_levels, run.stored_cards), (1, 2));
        apply(&mut run, false, true);
        assert_eq!(run.stored_cards, 2);
        run.pending_levels = 0;
        run.dead_t = Some(0.0);
        assert!(!apply(&mut run, false, true));
        assert_eq!(run.stored_cards, 2);
        run.dead_t = None;
        assert!(apply(&mut run, true, false));
        assert_eq!((run.pending_levels, run.stored_cards), (2, 0));
    }
}
