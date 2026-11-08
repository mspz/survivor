//! Manual level ups: with the option on, the levels gained wait in [`Run::held_levels`] and the
//! cards are dealt only when the player asks, by key or by clicking the prompt.
use super::hud::{self, HudFont};
use super::multiplayer::{GuestActions, Role};
use super::options::{Action, Settings};
use super::{Phase, Run, Selection};
use bevy::prelude::*;

#[derive(Component)]
struct Prompt;
#[derive(Component)]
struct PromptLabel;
#[derive(Component)]
struct PromptButton;

/// The player's preference, and their request to open the cards: held levels are released at
/// once, and the option turned off releases what was waiting.
pub(super) fn hold_levels(run: &mut Run, hold: bool, release: bool) {
    run.hold_levels = hold;
    if release || !hold {
        run.pending_levels += std::mem::take(&mut run.held_levels);
    }
}

fn spawn(mut commands: Commands, font: Option<Res<HudFont>>, prompts: Query<(), With<Prompt>>) {
    let Some(font) = font else {
        return;
    };
    if !prompts.is_empty() {
        return;
    }
    commands
        .spawn((
            Prompt,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(24.0),
                bottom: Val::Px(24.0),
                ..default()
            },
            bevy::ui::FocusPolicy::Pass,
            GlobalZIndex(400),
            Visibility::Hidden,
        ))
        .with_children(|row| {
            row.spawn((
                PromptButton,
                Button,
                Node {
                    padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BorderColor::all(Color::srgb(0.85, 0.65, 0.2)),
                BackgroundColor(Color::srgba(0.1, 0.07, 0.02, 0.85)),
            ))
            .with_children(|button| {
                button.spawn((
                    PromptLabel,
                    hud::text_bundle(&font.0, "", 18.0, Color::srgb(1.0, 0.82, 0.25)),
                ));
            });
        });
}

#[allow(clippy::too_many_arguments)]
fn update(
    settings: Res<Settings>,
    keys: Res<ButtonInput<KeyCode>>,
    phase: Res<State<Phase>>,
    selection: Res<Selection>,
    role: Option<Res<Role>>,
    mut run: ResMut<Run>,
    mut actions: ResMut<GuestActions>,
    auto: Option<Res<super::auto::Auto>>,
    clicks: Query<&Interaction, (Changed<Interaction>, With<PromptButton>)>,
    mut prompt: Query<&mut Visibility, With<Prompt>>,
    mut label: Query<&mut Text, With<PromptLabel>>,
) {
    let playing = *phase.get() == Phase::Playing && !selection.in_hub;
    // The automatic test runs pick their own cards.
    let manual = settings.manual_levels && auto.is_none();
    let release = playing
        && run.held_levels > 0
        && (settings.just_pressed(Action::LevelUp, &keys)
            || clicks.iter().any(|i| *i == Interaction::Pressed));
    if role.is_some_and(|r| *r == Role::Guest) {
        // The host owns the run: it hears the preference and the request with every input.
        actions.hold_levels = manual;
        actions.release_levels |= release;
    } else if run.hold_levels != manual || release {
        hold_levels(&mut run, manual, release);
    }
    let shown = playing && run.held_levels > 0;
    for mut visibility in &mut prompt {
        visibility.set_if_neq(if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
    if shown {
        let key = settings
            .key_name(Action::LevelUp)
            .map_or_else(String::new, |key| format!("  [{key}]"));
        let levels = run.held_levels;
        let text = if levels > 1 {
            format!("{levels} niveaux à choisir{key}")
        } else {
            format!("Niveau à choisir{key}")
        };
        for mut label in &mut label {
            if label.0 != text {
                label.0 = text.clone();
            }
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, (spawn, update).chain());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_levels_wait_for_the_request_or_for_the_option_to_be_turned_off() {
        let mut run = Run::default();
        hold_levels(&mut run, true, false);
        run.held_levels = 2;
        hold_levels(&mut run, true, false);
        assert_eq!((run.held_levels, run.pending_levels), (2, 0));
        hold_levels(&mut run, true, true);
        assert_eq!((run.held_levels, run.pending_levels), (0, 2));
        run.held_levels = 1;
        hold_levels(&mut run, false, false);
        assert_eq!((run.held_levels, run.pending_levels), (0, 3));
        assert!(!run.hold_levels);
    }
}
