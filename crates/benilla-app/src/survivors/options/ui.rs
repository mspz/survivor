use super::super::{hero::CamRig, hud::HudFont, Phase};
use super::*;
use crate::cvars::Cvars;
mod layout;

const GOLD: Color = Color::srgb(1.0, 0.82, 0.2);
#[derive(Component)]
pub(super) struct Root;
#[derive(Component, Clone, Copy)]
pub(super) enum Click {
    Tab(Tab),
    Bind(Action, usize),
    Sensitivity(i8),
    Zoom(i8),
    Distance(i8),
    DistanceReset,
    ViewDistance(i8),
    Volume(&'static str, i8),
    Cvar(&'static str),
    Flag(u8),
    Reset,
    Back,
}

fn value(cvars: Option<&Cvars>, name: &str) -> f32 {
    cvars
        .and_then(|c| c.get(name))
        .and_then(|v| v.parse().ok())
        .unwrap_or(match name {
            "MusicVolume" => 0.4,
            "AmbienceVolume" => 0.6,
            "gxWindow" => 0.0,
            "farclip" => 350.0,
            _ => 1.0,
        })
}
fn change_volume(cvars: &mut Cvars, name: &str, steps: i8) {
    let v = (value(Some(cvars), name) + f32::from(steps) * 0.05).clamp(0.0, 1.0);
    cvars.set(name, &format!("{v:.2}"));
}
pub(super) fn input(
    phase: Res<State<Phase>>,
    mut panel: ResMut<Panel>,
    mut settings: ResMut<Settings>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut rig: ResMut<CamRig>,
    mut cvars: Option<ResMut<Cvars>>,
    clicks: Query<(&Interaction, &Click), Changed<Interaction>>,
) {
    if *phase.get() != Phase::Paused {
        panel.open = false;
        panel.capture = None;
        return;
    }
    if !panel.open {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        if panel.capture.take().is_none() {
            panel.open = false;
        }
        keys.clear_just_pressed(KeyCode::Escape);
        return;
    }
    if let Some((action, slot)) = panel.capture {
        let captured = keys
            .get_just_pressed()
            .copied()
            .find(|k| KEY_CODES.contains(k));
        if let Some(key) = captured {
            settings.rebind(
                action,
                slot,
                if key == KeyCode::Backspace {
                    None
                } else {
                    Some(key)
                },
            );
            settings.save();
            panel.capture = None;
            keys.clear_just_pressed(key);
        }
        return;
    }
    if settings.just_pressed(Action::Pause, &keys) {
        panel.open = false;
        let captured: Vec<_> = keys.get_just_pressed().copied().collect();
        for key in captured {
            keys.clear_just_pressed(key);
        }
        return;
    }
    for (interaction, click) in &clicks {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *click {
            Click::Tab(tab) => {
                panel.tab = tab;
                panel.capture = None;
                panel.scroll = 0.0;
            }
            Click::Bind(action, slot) => panel.capture = Some((action, slot)),
            Click::Sensitivity(step) => rig.adjust_sensitivity(f32::from(step) * 0.1),
            Click::Zoom(step) => {
                settings.zoom_speed = ((settings.zoom_speed + f32::from(step) * 0.25) * 4.0)
                    .round()
                    .clamp(1.0, 8.0)
                    / 4.0;
                settings.save();
            }
            Click::Distance(step) => rig.dist = (rig.dist + f32::from(step) * 2.0).clamp(1.5, 70.0),
            Click::DistanceReset => {
                rig.dist = 12.0;
                rig.pitch = 0.3;
            }
            Click::ViewDistance(step) => {
                if let Some(c) = cvars.as_deref_mut() {
                    let distance =
                        (value(Some(c), "farclip") + f32::from(step) * 25.0).clamp(177.0, 777.0);
                    c.set("farclip", &format!("{distance:.0}"));
                }
            }
            Click::Volume(name, step) => {
                if let Some(c) = cvars.as_deref_mut() {
                    change_volume(c, name, step);
                }
            }
            Click::Cvar(name) => {
                if let Some(c) = cvars.as_deref_mut() {
                    let v = if value(Some(c), name) == 0.0 {
                        "1"
                    } else {
                        "0"
                    };
                    c.set(name, v);
                    if matches!(name, "gxWindow" | "gxVSync" | "gxMaximize") {
                        c.commit_latched();
                    }
                }
            }
            Click::Flag(flag) => {
                match flag {
                    0 => settings.damage = !settings.damage,
                    1 => settings.healing = !settings.healing,
                    2 => settings.incoming = !settings.incoming,
                    3 => settings.critical_only = !settings.critical_only,
                    5 => settings.invert_x = !settings.invert_x,
                    6 => settings.invert_y = !settings.invert_y,
                    7 => settings.show_fps = !settings.show_fps,
                    8 => settings.perf_diag = !settings.perf_diag,
                    9 => settings.auto_draw_cards = !settings.auto_draw_cards,
                    10 => settings.ally_plates = !settings.ally_plates,
                    11 => settings.enemy_plates = !settings.enemy_plates,
                    _ => settings.grade = !settings.grade,
                };
                settings.save();
            }
            Click::Back => {
                panel.open = false;
                panel.capture = None;
            }
            Click::Reset => {
                match panel.tab {
                    Tab::Controls => settings.bindings = Settings::defaults().bindings,
                    Tab::Mouse => {
                        let delta = 1.0 - rig.sensitivity;
                        rig.adjust_sensitivity(delta);
                        settings.invert_x = false;
                        settings.invert_y = false;
                        settings.zoom_speed = 1.0;
                        rig.dist = 12.0;
                        rig.pitch = 0.3;
                    }
                    Tab::Sound => {
                        if let Some(c) = cvars.as_deref_mut() {
                            for (k, v) in [
                                ("MasterVolume", "1"),
                                ("SoundVolume", "1"),
                                ("MusicVolume", "0.4"),
                                ("AmbienceVolume", "0.6"),
                                ("MasterSoundEffects", "1"),
                                ("EnableMusic", "1"),
                                ("EnableAmbience", "1"),
                                ("SoundOutputLimiter", "1"),
                            ] {
                                c.set(k, v);
                            }
                        }
                    }
                    Tab::Display => {
                        settings.auto_draw_cards = false;
                        settings.damage = true;
                        settings.healing = true;
                        settings.incoming = true;
                        settings.critical_only = false;
                        settings.grade = true;
                        settings.show_fps = false;
                        settings.ally_plates = false;
                        settings.enemy_plates = false;
                        settings.perf_diag = false;
                    }
                    Tab::Video => {
                        if let Some(c) = cvars.as_deref_mut() {
                            for (k, v) in [
                                ("gxWindow", "0"),
                                ("gxVSync", "1"),
                                ("gxMaximize", "0"),
                                ("farclip", "350"),
                            ] {
                                c.set(k, v);
                            }
                            c.commit_latched();
                        }
                    }
                    Tab::Help => {}
                }
                settings.save();
            }
        }
    }
}

fn text(f: &Handle<Font>, s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(s),
        TextFont {
            font: f.clone(),
            font_size: size,
            ..default()
        },
        TextColor(color),
    )
}
fn button(
    ui: &mut ChildSpawnerCommands,
    f: &Handle<Font>,
    click: Click,
    label: impl Into<String>,
    width: f32,
) {
    ui.spawn((
        Button,
        click,
        BackgroundColor(Color::srgb(0.075, 0.065, 0.045)),
        BorderColor::all(Color::srgb(0.32, 0.26, 0.15)),
        Node {
            width: Val::Px(width),
            max_width: Val::Percent(100.0),
            min_width: Val::Px(0.0),
            min_height: Val::Px(32.0),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
            border: UiRect::all(Val::Px(1.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
    ))
    .with_children(|b| {
        b.spawn(text(f, label, 13.0, GOLD));
    });
}
fn row(
    ui: &mut ChildSpawnerCommands,
    f: &Handle<Font>,
    label: &str,
    build: impl FnOnce(&mut ChildSpawnerCommands),
) {
    ui.spawn((
        BackgroundColor(Color::srgba(1.0, 0.9, 0.6, 0.025)),
        Node {
            width: Val::Percent(100.0),
            min_width: Val::Px(0.0),
            flex_shrink: 0.0,
            padding: UiRect::axes(Val::Px(8.0), Val::Px(8.0)),
            column_gap: Val::Px(12.0),
            row_gap: Val::Px(6.0),
            flex_wrap: FlexWrap::Wrap,
            align_items: AlignItems::Center,
            ..default()
        },
    ))
    .with_children(|r| {
        r.spawn((
            text(f, label, 14.0, Color::srgb(0.78, 0.76, 0.69)),
            Node {
                width: Val::Percent(43.0),
                min_width: Val::Px(150.0),
                ..default()
            },
        ));
        r.spawn(Node {
            flex_grow: 1.0,
            flex_basis: Val::Px(0.0),
            min_width: Val::Px(180.0),
            column_gap: Val::Px(8.0),
            row_gap: Val::Px(6.0),
            flex_wrap: FlexWrap::Wrap,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(build);
    });
}

pub(super) fn key_label(key: Option<KeyCode>) -> String {
    key.map(|k| {
        format!("{k:?}")
            .replace("Key", "")
            .replace("Digit", "")
            .replace("Arrow", "")
    })
    .unwrap_or("Unbound".to_owned())
}
#[allow(clippy::too_many_arguments)]
pub(super) fn render(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    panel: Res<Panel>,
    settings: Res<Settings>,
    rig: Res<CamRig>,
    cvars: Option<Res<Cvars>>,
    font: Option<Res<HudFont>>,
    roots: Query<Entity, With<Root>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    scale: Option<Res<UiScale>>,
    mut cache: Local<String>,
) {
    let Some(font) = font else {
        return;
    };
    let cv = cvars.as_deref();
    let audio = [
        "MasterVolume",
        "SoundVolume",
        "MusicVolume",
        "AmbienceVolume",
        "MasterSoundEffects",
        "EnableMusic",
        "EnableAmbience",
        "gxWindow",
        "gxVSync",
        "gxMaximize",
        "farclip",
        "SoundOutputLimiter",
    ]
    .map(|k| value(cv, k));
    let viewport = windows.single().map_or(Vec2::new(1000.0, 820.0), |window| {
        Vec2::new(window.width(), window.height()) / scale.as_deref().map_or(1.0, |s| s.0)
    });
    let stamp = format!(
        "{:?} {} {:?} {:?} {:?} {:?} {:?} {:?}",
        phase.get(),
        panel.open,
        panel.tab,
        panel.capture,
        *settings,
        (rig.sensitivity, rig.dist),
        audio,
        viewport
    );
    if *cache == stamp {
        return;
    }
    *cache = stamp;
    for root in &roots {
        commands.entity(root).despawn();
    }
    if !panel.open || *phase.get() != Phase::Paused {
        return;
    }
    layout::build(
        &mut commands,
        &font.0,
        &panel,
        &settings,
        &rig,
        cv,
        viewport,
    );
}

fn on_off(on: bool) -> &'static str {
    if on {
        "On"
    } else {
        "Off"
    }
}
#[derive(Component)]
pub(super) struct Body;
pub(super) fn scroll(
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    mut panel: ResMut<Panel>,
    mut bodies: Query<
        (
            &ComputedNode,
            &bevy::ui::RelativeCursorPosition,
            &mut ScrollPosition,
        ),
        With<Body>,
    >,
) {
    if !panel.open {
        wheel.clear();
        return;
    }
    for event in wheel.read() {
        for (node, cursor, mut position) in &mut bodies {
            if cursor.cursor_over() {
                let step = if event.unit == bevy::input::mouse::MouseScrollUnit::Line {
                    32.0
                } else {
                    1.0
                };
                position.0.y = (position.0.y - event.y * step).clamp(
                    0.0,
                    ((node.content_size.y - node.size.y) * node.inverse_scale_factor).max(0.0),
                );
                panel.scroll = position.0.y;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn camera_preferences_and_video_buttons_apply_and_reset_their_real_settings() {
        let mut app = App::new();
        app.insert_resource(State::new(Phase::Paused))
            .init_resource::<Settings>()
            .init_resource::<CamRig>()
            .init_resource::<Cvars>()
            .init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(Panel {
                open: true,
                tab: Tab::Mouse,
                ..default()
            })
            .add_systems(Update, input);
        let click = |app: &mut App, action| {
            let e = app.world_mut().spawn((Interaction::Pressed, action)).id();
            app.update();
            app.world_mut().despawn(e);
        };
        click(&mut app, Click::Flag(6));
        click(&mut app, Click::Zoom(1));
        assert!(app.world().resource::<Settings>().invert_y);
        assert_eq!(app.world().resource::<Settings>().zoom_speed, 1.25);
        click(&mut app, Click::Reset);
        assert!(!app.world().resource::<Settings>().invert_y);
        assert_eq!(app.world().resource::<Settings>().zoom_speed, 1.0);
        app.world_mut().resource_mut::<Panel>().tab = Tab::Video;
        click(&mut app, Click::ViewDistance(1));
        assert_eq!(
            value(Some(app.world().resource::<Cvars>()), "farclip"),
            375.0
        );
        click(&mut app, Click::Cvar("gxVSync"));
        assert_eq!(value(Some(app.world().resource::<Cvars>()), "gxVSync"), 0.0);
        click(&mut app, Click::Reset);
        assert_eq!(
            value(Some(app.world().resource::<Cvars>()), "farclip"),
            350.0
        );
        assert_eq!(value(Some(app.world().resource::<Cvars>()), "gxVSync"), 1.0);
    }
    #[test]
    fn escape_closes_options_without_resuming_and_capture_consumes_the_key() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<Phase>()
            .init_resource::<Settings>()
            .init_resource::<CamRig>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<super::super::super::Selection>()
            .init_resource::<super::super::super::Run>()
            .insert_resource(Panel {
                open: true,
                ..default()
            })
            .add_systems(Update, (input, super::super::super::hud::keys).chain());
        app.world_mut()
            .resource_mut::<NextState<Phase>>()
            .set(Phase::Paused);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        app.update();
        assert!(!app.world().resource::<Panel>().open);
        assert_eq!(*app.world().resource::<State<Phase>>().get(), Phase::Paused);
        {
            let mut panel = app.world_mut().resource_mut::<Panel>();
            panel.open = true;
            panel.capture = Some((Action::Forward, 0));
        }
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyZ);
        app.update();
        assert_eq!(
            app.world().resource::<Settings>().bindings[Action::Forward as usize][0],
            Some(KeyCode::KeyZ)
        );
        assert!(!app
            .world()
            .resource::<ButtonInput<KeyCode>>()
            .just_pressed(KeyCode::KeyZ));
        assert!(app.world().resource::<Panel>().open);
    }
    #[test]
    fn audio_controls_update_real_cvars_and_sound_observers() {
        let mut app = App::new();
        app.init_resource::<Cvars>()
            .init_resource::<crate::sound::SoundConfig>()
            .add_observer(crate::sound::on_cvar);
        change_volume(
            &mut app.world_mut().resource_mut::<Cvars>(),
            "MasterVolume",
            -10,
        );
        let events = app.world_mut().resource_mut::<Cvars>().take_events();
        for event in events {
            app.world_mut().trigger(event);
        }
        assert_eq!(
            app.world().resource::<crate::sound::SoundConfig>().master,
            0.5
        );
    }
    #[test]
    fn all_options_tabs_build_without_duplicate_components() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<Phase>()
            .init_resource::<Settings>()
            .init_resource::<CamRig>()
            .insert_resource(Panel {
                open: true,
                ..default()
            })
            .insert_resource(HudFont(Handle::default()))
            .add_systems(Update, render);
        app.world_mut()
            .resource_mut::<NextState<Phase>>()
            .set(Phase::Paused);
        for (tab, sections, heading) in [
            (Tab::Controls, 3, "Movement & combat"),
            (Tab::Sound, 3, "Volume levels"),
            (Tab::Mouse, 2, "Camera rotation"),
            (Tab::Display, 4, "Cards & progression"),
            (Tab::Video, 2, "Window & synchronization"),
            (Tab::Help, 6, "Hub & progression"),
        ] {
            app.world_mut().resource_mut::<Panel>().tab = tab;
            app.update();
            let world = app.world_mut();
            assert_eq!(world.query::<&Root>().iter(world).count(), 1);
            assert_eq!(
                world.query::<&layout::Section>().iter(world).count(),
                sections
            );
            assert!(world
                .query::<&Text>()
                .iter(world)
                .any(|text| text.0 == heading));
            assert_eq!(
                world
                    .query::<&Click>()
                    .iter(world)
                    .filter(|click| matches!(click, Click::Tab(_)))
                    .count(),
                6
            );
            if tab == Tab::Controls {
                assert_eq!(
                    world
                        .query::<&Click>()
                        .iter(world)
                        .filter(|click| matches!(click, Click::Bind(_, _)))
                        .count(),
                    Action::ALL.len() * 2
                );
            }
            // Changing a setting rebuilds the controls while preserving the browsing position.
            app.world_mut().resource_mut::<Panel>().scroll = 77.0;
            app.world_mut().resource_mut::<Settings>().damage ^= true;
            app.update();
            let world = app.world_mut();
            assert_eq!(
                world
                    .query_filtered::<&ScrollPosition, With<Body>>()
                    .single(world)
                    .unwrap()
                    .0
                    .y,
                77.0
            );
        }
    }
}
