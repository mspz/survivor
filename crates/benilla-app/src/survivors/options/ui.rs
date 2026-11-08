use super::super::{hero::CamRig, hud::HudFont, Phase};
use super::*;
use crate::cvars::Cvars;

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
                        settings.auto_draw_cards = true;
                        settings.damage = true;
                        settings.healing = true;
                        settings.incoming = true;
                        settings.critical_only = false;
                        settings.grade = true;
                        settings.show_fps = false;
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
    super::super::combat_ui::skin::button(ui, f, label, click, width);
}
fn row(
    ui: &mut ChildSpawnerCommands,
    f: &Handle<Font>,
    label: &str,
    build: impl FnOnce(&mut ChildSpawnerCommands),
) {
    ui.spawn(Node {
        width: Val::Percent(100.0),
        column_gap: Val::Px(8.0),
        align_items: AlignItems::Center,
        ..default()
    })
    .with_children(|r| {
        r.spawn((
            text(f, label, 15.0, Color::WHITE),
            Node {
                width: Val::Px(280.0),
                ..default()
            },
        ));
        build(r);
    });
}
fn key_label(key: Option<KeyCode>) -> String {
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
    let stamp = format!(
        "{:?} {} {:?} {:?} {:?} {:?} {:?}",
        phase.get(),
        panel.open,
        panel.tab,
        panel.capture,
        *settings,
        (rig.sensitivity, rig.dist),
        audio
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
    let f = &font.0;
    commands.spawn((Root,Button,GlobalZIndex(800),BackgroundColor(Color::srgba(0.0,0.0,0.0,0.8)),Node{position_type:PositionType::Absolute,width:Val::Percent(100.0),height:Val::Percent(100.0),justify_content:JustifyContent::Center,align_items:AlignItems::Center,..default()})).with_children(|overlay| {
        overlay.spawn(Node{width:Val::Px(740.0),max_width:Val::Percent(98.0),height:Val::Px(710.0),max_height:Val::Vh(94.0),padding:UiRect::all(Val::Px(20.0)),flex_direction:FlexDirection::Column,row_gap:Val::Px(12.0),..default()}).with_children(|frame| {
            super::super::combat_ui::skin::backdrop(frame);
            frame.spawn(text(f,"OPTIONS",26.0,GOLD));
            frame.spawn(Node{column_gap:Val::Px(5.0),row_gap:Val::Px(5.0),flex_wrap:FlexWrap::Wrap,..default()}).with_children(|tabs| {
                for (tab,label) in [(Tab::Controls,"Key bindings"),(Tab::Mouse,"Camera"),(Tab::Sound,"Audio"),(Tab::Display,"Interface"),(Tab::Video,"Video"),(Tab::Help,"Help")] {
                    button(tabs,f,Click::Tab(tab),if tab==panel.tab{format!("[{label}]")}else{label.into()},104.0);
                }
            });
            frame.spawn((Body,Interaction::default(),bevy::ui::RelativeCursorPosition::default(),ScrollPosition(Vec2::new(0.0,panel.scroll)),Node{width:Val::Percent(100.0),min_height:Val::Px(0.0),flex_direction:FlexDirection::Column,row_gap:Val::Px(8.0),flex_grow:1.0,flex_basis:Val::Px(0.0),overflow:Overflow::scroll_y(),..default()})).with_children(|body| {
                match panel.tab {
                    Tab::Controls => {
                        body.spawn(text(f,"Choose a primary or secondary key. Conflicts swap the bindings.",13.0,GOLD));
                        for action in Action::ALL { row(body,f,action.name(),|r| {
                            for slot in 0..2 { button(r,f,Click::Bind(action,slot),if panel.capture==Some((action,slot)){"Press a key…".into()}else{key_label(settings.bindings[action as usize][slot])},164.0); }
                        }); }
                        body.spawn(text(f,"Esc cancels binding. Backspace clears the key.\nBindings use physical key positions.",12.0,Color::WHITE));
                    }
                    Tab::Sound => {
                        for (name,label) in [("MasterVolume","Master volume"),("SoundVolume","Sound effects"),("MusicVolume","Music"),("AmbienceVolume","Ambience")] {
                            row(body,f,label,|r| { button(r,f,Click::Volume(name,-1),"−",45.0);r.spawn(text(f,format!("{:3.0} %",value(cv,name)*100.0),16.0,GOLD));button(r,f,Click::Volume(name,1),"+",45.0); });
                        }
                        for (name,label) in [("MasterSoundEffects","Enable sound"),("EnableMusic","Enable music"),("EnableAmbience","Enable ambience"),("SoundOutputLimiter","Audio limiter")] {
                            row(body,f,label,|r|button(r,f,Click::Cvar(name),on_off(value(cv,name)>0.0),160.0));
                        }
                        body.spawn(text(f,"The limiter reduces distortion when many sounds overlap. Settings apply immediately.",13.0,GOLD));
                    }
                    Tab::Mouse => {
                        row(body,f,"Sensitivity",|r|{button(r,f,Click::Sensitivity(-1),"−",45.0);r.spawn(text(f,format!("{:.1}×",rig.sensitivity),16.0,GOLD));button(r,f,Click::Sensitivity(1),"+",45.0);});
                        for(flag,label,on)in[(5,"Invert horizontal axis",settings.invert_x),(6,"Invert vertical axis",settings.invert_y)]{row(body,f,label,|r|button(r,f,Click::Flag(flag),on_off(on),160.0));}
                        row(body,f,"Zoom speed",|r|{button(r,f,Click::Zoom(-1),"−",45.0);r.spawn(text(f,format!("{:.2}×",settings.zoom_speed),16.0,GOLD));button(r,f,Click::Zoom(1),"+",45.0);});
                        row(body,f,"Current distance",|r|{button(r,f,Click::Distance(-1),"−",45.0);r.spawn(text(f,format!("{:.1} m",rig.dist),16.0,GOLD));button(r,f,Click::Distance(1),"+",45.0);});
                        button(body,f,Click::DistanceReset,"Reset camera height and zoom",320.0);
                        body.spawn(text(f,"Drag with either mouse button to rotate the camera. Scroll to zoom. Hold both buttons to move forward.\nYou can adjust the current distance with the mouse wheel.",13.0,Color::WHITE));
                    }
                    Tab::Display => {
                        for(flag,label,on)in[(9,"Automatically draw level-up cards",settings.auto_draw_cards),(0,"Damage dealt",settings.damage),(1,"Healing received and done",settings.healing),(2,"Damage taken",settings.incoming),(3,"Show only outgoing critical hits",settings.critical_only),(4,"Color grading and sharpening",settings.grade),(7,"FPS counter",settings.show_fps),(8,"Performance diagnostics",settings.perf_diag)] {row(body,f,label,|r|button(r,f,Click::Flag(flag),on_off(on),160.0));}
                        body.spawn(text(f,"Filters apply to new floating numbers. Combat calculations are unchanged.\nDrag the Character and Combat meter windows by their title bars to move them.\nPerformance diagnostics identify expensive work and write a report to benilla-config/Diagnostics. They turn off on the next launch.\nPer-system details require warcraft-survivors-perf.exe.",13.0,GOLD));
                    }
                    Tab::Video => {
                        for(name,label)in[("gxWindow","Windowed mode"),("gxMaximize","Maximized window"),("gxVSync","Vertical sync")] {row(body,f,label,|r|button(r,f,Click::Cvar(name),on_off(value(cv,name)>0.0),160.0));}
                        row(body,f,"View distance",|r|{button(r,f,Click::ViewDistance(-1),"−",45.0);r.spawn(text(f,format!("{:.0} m",value(cv,"farclip")),16.0,GOLD));button(r,f,Click::ViewDistance(1),"+",45.0);});
                        body.spawn(text(f,"A shorter view distance reduces GPU load. Vertical sync reduces tearing.\nFullscreen uses borderless mode. The browser manages its own window.",13.0,GOLD));
                    }
                    Tab::Help => {
                        for(title,description)in[
                            ("Hub and progression","Expeditions: choose a zone and start a run. Talents: permanent character progression. Runes: prepare your equipment in the hub."),
                            ("Items and shared chest","Right-click in your bags to equip or unequip an item. With the chest open, right-click to transfer. You can also drag items between the windows. Dropping elsewhere cancels the transfer."),
                            ("Gheed","Approach the blacksmith and select a slot, item type, affix profile, rarity and item level. Choose random or custom affixes, inspect the preview, then click Craft. The training blacksmith also offers legendary effects."),
                            ("Multiplayer","Host or join a game from character selection. Use the host's Hamachi IPv4 address. Shared chest items are available to everyone in the game."),
                            ("Pause","Menus pause the run in solo play. In multiplayer, the world keeps running: open menus in a safe place."),
                        ]{body.spawn(text(f,title,17.0,GOLD));body.spawn(text(f,description,14.0,Color::WHITE));}
                    }
                }
            });
            frame.spawn(Node{column_gap:Val::Px(10.0),flex_wrap:FlexWrap::Wrap,..default()}).with_children(|footer| {
                if panel.tab!=Tab::Help {button(footer,f,Click::Reset,"Reset this tab",240.0);}
                button(footer,f,Click::Back,"Back [Esc]",200.0);
            });
            frame.spawn(text(f,"Preferences save automatically · Scroll to browse",12.0,GOLD));
        });
    });
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
        for tab in [
            Tab::Controls,
            Tab::Sound,
            Tab::Mouse,
            Tab::Display,
            Tab::Video,
            Tab::Help,
        ] {
            app.world_mut().resource_mut::<Panel>().tab = tab;
            app.update();
            let world = app.world_mut();
            assert_eq!(world.query::<&Root>().iter(world).count(), 1);
        }
    }
}
