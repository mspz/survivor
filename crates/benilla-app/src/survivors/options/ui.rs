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
            }
            Click::Bind(action, slot) => panel.capture = Some((action, slot)),
            Click::Sensitivity(step) => rig.adjust_sensitivity(f32::from(step) * 0.1),
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
                    if name == "gxWindow" {
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
                            ] {
                                c.set(k, v);
                            }
                        }
                    }
                    Tab::Display => {
                        settings.damage = true;
                        settings.healing = true;
                        settings.incoming = true;
                        settings.critical_only = false;
                        settings.grade = true;
                    }
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
    ]
    .map(|k| value(cv, k));
    let stamp = format!(
        "{:?} {} {:?} {:?} {:?} {} {:?}",
        phase.get(),
        panel.open,
        panel.tab,
        panel.capture,
        *settings,
        rig.sensitivity,
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
    commands.spawn((Root,Button,GlobalZIndex(800),BackgroundColor(Color::srgba(0.0,0.0,0.0,0.8)),Node{position_type:PositionType::Absolute,width:Val::Percent(100.0),height:Val::Percent(100.0),justify_content:JustifyContent::Center,align_items:AlignItems::Center,..default()}))
        .with_children(|overlay|{
            overlay.spawn((BackgroundColor(Color::srgb(0.045,0.035,0.03)),BorderColor::all(GOLD),Node{width:Val::Px(720.0),height:Val::Px(710.0),max_height:Val::Vh(95.0),padding:UiRect::all(Val::Px(20.0)),border:UiRect::all(Val::Px(2.0)),flex_direction:FlexDirection::Column,row_gap:Val::Px(12.0),..default()})).with_children(|frame|{ super::super::combat_ui::skin::backdrop(frame);
                frame.spawn(text(f,"OPTIONS",30.0,GOLD));
                frame.spawn(Node{column_gap:Val::Px(8.0),..default()}).with_children(|tabs|{
                    for (tab,label) in [(Tab::Controls,"Key bindings"),(Tab::Sound,"Sound"),(Tab::Mouse,"Mouse"),(Tab::Display,"Display")]{button(tabs,f,Click::Tab(tab),if tab==panel.tab{format!("[{label}]")}else{label.to_owned()},158.0);}
                });
                frame.spawn(Node{width:Val::Percent(100.0),flex_direction:FlexDirection::Column,row_gap:Val::Px(2.0),flex_grow:1.0,..default()}).with_children(|body|{
                    match panel.tab {
                        Tab::Controls=>{
                            body.spawn(text(f,"Click a binding, then press a key. Conflicting keys swap assignments.",13.0,GOLD));
                            for action in Action::ALL {row(body,f,action.name(),|r|{
                                for slot in 0..2 {let label=if panel.capture==Some((action,slot)){"Press a key...".to_owned()}else{key_label(settings.bindings[action as usize][slot])};button(r,f,Click::Bind(action,slot),label,164.0);}
                            });}
                            body.spawn(text(f,"Esc cancels capture / always opens pause. Backspace clears a binding.\nMouse: left drag orbits; right drag turns; both buttons move forward; wheel zooms.\nEnter confirms menu choices. Key bindings use physical key positions.",12.0,Color::srgb(0.7,0.7,0.7)));
                        }
                        Tab::Sound=>{
                            for (name,label) in [("MasterVolume","Master volume"),("SoundVolume","Effects volume"),("MusicVolume","Music volume"),("AmbienceVolume","Ambience volume")] {
                                row(body,f,label,|r|{button(r,f,Click::Volume(name,-1),"-",48.0);r.spawn((text(f,format!("{:.0}%",value(cv,name)*100.0),18.0,GOLD),Node{width:Val::Px(80.0),..default()}));button(r,f,Click::Volume(name,1),"+",48.0);});
                            }
                            for (name,label) in [("MasterSoundEffects","Enable sound"),("EnableMusic","Enable music"),("EnableAmbience","Enable ambience")] {row(body,f,label,|r|button(r,f,Click::Cvar(name),if value(cv,name)>0.0 {"ON"}else{"OFF"},180.0));}
                            body.spawn(text(f,"Changes apply immediately. Volumes are saved in the engine configuration.",13.0,GOLD));
                        }
                        Tab::Mouse=>{
                            row(body,f,"Mouse sensitivity",|r|{button(r,f,Click::Sensitivity(-1),"-",48.0);r.spawn((text(f,format!("{:.1}x",rig.sensitivity),20.0,GOLD),Node{width:Val::Px(80.0),..default()}));button(r,f,Click::Sensitivity(1),"+",48.0);});
                            body.spawn(text(f,"0.1x to 3.0x. Applies to camera orbit and right-button character turning.",14.0,Color::WHITE));
                        }
                        Tab::Display=>{
                            for (flag,label,on) in [(0,"Outgoing damage numbers",settings.damage),(1,"Healing numbers",settings.healing),(2,"Incoming damage numbers",settings.incoming),(3,"Outgoing critical numbers only",settings.critical_only),(4,"Colour / sharpening filter",settings.grade)] {row(body,f,label,|r|button(r,f,Click::Flag(flag),if on{"ON"}else{"OFF"},180.0));}
                            row(body,f,"Windowed mode",|r|button(r,f,Click::Cvar("gxWindow"),if value(cv,"gxWindow")>0.0{"ON"}else{"OFF"},180.0));
                            body.spawn(text(f,"Combat-number filters apply to new floating numbers; damage and healing still resolve normally.",13.0,GOLD));
                        }
                    }
                });
                frame.spawn(Node{column_gap:Val::Px(12.0),..default()}).with_children(|footer|{button(footer,f,Click::Reset,"Reset this tab",200.0);button(footer,f,Click::Back,"Back to pause [Esc]",240.0);});
                frame.spawn(text(f,"Preferences are saved automatically.",12.0,GOLD));
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;
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
        for tab in [Tab::Controls, Tab::Sound, Tab::Mouse, Tab::Display] {
            app.world_mut().resource_mut::<Panel>().tab = tab;
            app.update();
            let world = app.world_mut();
            assert_eq!(world.query::<&Root>().iter(world).count(), 1);
        }
    }
}
