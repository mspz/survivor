//! Grouped settings panels, using the combat debug window's visual hierarchy.
use super::*;

#[derive(Component)]
pub(super) struct Section;

fn section(
    ui: &mut ChildSpawnerCommands,
    f: &Handle<Font>,
    title: &str,
    note: &str,
    build: impl FnOnce(&mut ChildSpawnerCommands),
) {
    ui.spawn((
        Section,
        BackgroundColor(Color::srgb(0.055, 0.047, 0.037)),
        BorderColor::all(Color::srgb(0.23, 0.19, 0.12)),
        Node {
            width: Val::Percent(100.0),
            min_width: Val::Px(0.0),
            flex_shrink: 0.0,
            padding: UiRect::all(Val::Px(12.0)),
            border: UiRect::all(Val::Px(1.0)),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            ..default()
        },
    ))
    .with_children(|card| {
        card.spawn(text(f, title, 16.0, GOLD));
        if !note.is_empty() {
            card.spawn((
                text(f, note, 12.0, Color::srgb(0.64, 0.62, 0.56)),
                Node {
                    width: Val::Percent(100.0),
                    margin: UiRect::bottom(Val::Px(6.0)),
                    ..default()
                },
            ));
        }
        build(card);
    });
}
fn flags(ui: &mut ChildSpawnerCommands, f: &Handle<Font>, rows: &[(u8, &str, bool)]) {
    for &(flag, label, on) in rows {
        row(ui, f, label, |r| {
            button(r, f, Click::Flag(flag), on_off(on), 140.0)
        });
    }
}
fn cvar_flags(
    ui: &mut ChildSpawnerCommands,
    f: &Handle<Font>,
    cv: Option<&Cvars>,
    rows: &[(&'static str, &str)],
) {
    for &(name, label) in rows {
        row(ui, f, label, |r| {
            button(
                r,
                f,
                Click::Cvar(name),
                on_off(value(cv, name) > 0.0),
                140.0,
            )
        });
    }
}
fn step(
    ui: &mut ChildSpawnerCommands,
    f: &Handle<Font>,
    label: &str,
    value: String,
    less: Click,
    more: Click,
) {
    row(ui, f, label, |r| {
        button(r, f, less, "-", 40.0);
        r.spawn((
            text(f, value, 16.0, Color::srgb(0.94, 0.92, 0.85)),
            Node {
                min_width: Val::Px(70.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ));
        button(r, f, more, "+", 40.0);
    });
}
fn tab_button(
    ui: &mut ChildSpawnerCommands,
    f: &Handle<Font>,
    tab: Tab,
    label: &str,
    selected: bool,
) {
    ui.spawn((
        Button,
        Click::Tab(tab),
        BackgroundColor(if selected {
            Color::srgb(0.25, 0.17, 0.06)
        } else {
            Color::srgb(0.075, 0.065, 0.045)
        }),
        BorderColor::all(if selected {
            GOLD
        } else {
            Color::srgb(0.32, 0.26, 0.15)
        }),
        Node {
            min_height: Val::Px(32.0),
            flex_shrink: 0.0,
            padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
            border: UiRect::all(Val::Px(1.0)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
    ))
    .with_children(|b| {
        b.spawn(text(
            f,
            label,
            13.0,
            if selected {
                GOLD
            } else {
                Color::srgb(0.88, 0.85, 0.75)
            },
        ));
    });
}

pub(super) fn build(
    commands: &mut Commands,
    f: &Handle<Font>,
    panel: &Panel,
    settings: &Settings,
    rig: &CamRig,
    cv: Option<&Cvars>,
    viewport: Vec2,
) {
    commands.spawn((Root,Button,GlobalZIndex(800),BackgroundColor(Color::srgba(0.0,0.0,0.0,0.88)),
        Node {position_type:PositionType::Absolute,width:Val::Percent(100.0),height:Val::Percent(100.0),
            justify_content:JustifyContent::Center,align_items:AlignItems::Center,..default()}
    )).with_children(|overlay| {
        overlay.spawn((BackgroundColor(Color::srgb(0.025,0.022,0.018)), BorderColor::all(Color::srgb(0.55,0.43,0.15)),
            Node {width:Val::Px((viewport.x-24.0).clamp(240.0,940.0)),max_width:Val::Percent(98.0),
                height:Val::Px((viewport.y-24.0).clamp(300.0,820.0)),max_height:Val::Vh(94.0),
                padding:UiRect::all(Val::Px(16.0)),border:UiRect::all(Val::Px(1.0)),
                flex_direction:FlexDirection::Column,row_gap:Val::Px(12.0),..default()}
        )).with_children(|frame| {
            frame.spawn(Node {width:Val::Percent(100.0),justify_content:JustifyContent::SpaceBetween,
                align_items:AlignItems::Start,column_gap:Val::Px(8.0),flex_shrink:0.0,..default()
            }).with_children(|header| {
                header.spawn(Node {flex_direction:FlexDirection::Column,flex_grow:1.0,min_width:Val::Px(0.0),row_gap:Val::Px(4.0),..default()}).with_children(|title| {
                    title.spawn(text(f,"Options",25.0,GOLD));
                    title.spawn(text(f,"Customize your controls, presentation and gameplay preferences.",12.0,Color::srgb(0.64,0.62,0.56)));
                });
                button(header,f,Click::Back,"Close",90.0);
            });
            frame.spawn(Node {column_gap:Val::Px(5.0),row_gap:Val::Px(5.0),flex_wrap:FlexWrap::Wrap,flex_shrink:0.0,..default()}).with_children(|tabs| {
                for (tab,label) in [(Tab::Controls,"Key bindings"),(Tab::Mouse,"Camera"),(Tab::Sound,"Audio"),(Tab::Display,"Interface"),(Tab::Video,"Video"),(Tab::Help,"Help")] {
                    tab_button(tabs,f,tab,label,tab==panel.tab);
                }
            });
            frame.spawn((Body,Interaction::default(),bevy::ui::RelativeCursorPosition::default(),ScrollPosition(Vec2::new(0.0,panel.scroll)),
                Node {width:Val::Percent(100.0),min_height:Val::Px(0.0),min_width:Val::Px(0.0),
                    flex_direction:FlexDirection::Column,row_gap:Val::Px(10.0),flex_grow:1.0,
                    flex_basis:Val::Px(0.0),overflow:Overflow::scroll_y(),padding:UiRect::right(Val::Px(8.0)),..default()}
            )).with_children(|body| contents(body,f,panel,settings,rig,cv));
            frame.spawn(Node {justify_content:JustifyContent::SpaceBetween,column_gap:Val::Px(10.0),row_gap:Val::Px(6.0),flex_wrap:FlexWrap::Wrap,flex_shrink:0.0,..default()}).with_children(|footer| {
                if panel.tab!=Tab::Help {button(footer,f,Click::Reset,"Reset this tab",180.0);}
                button(footer,f,Click::Back,"Back [Esc]",150.0);
            });
            frame.spawn(text(f,"Preferences save automatically. Scroll to browse.",11.0,Color::srgb(0.64,0.62,0.56)));
        });
    });
}

fn contents(
    body: &mut ChildSpawnerCommands,
    f: &Handle<Font>,
    panel: &Panel,
    settings: &Settings,
    rig: &CamRig,
    cv: Option<&Cvars>,
) {
    match panel.tab {
        Tab::Controls => {
            for (title,note,actions) in [
                ("Movement & combat","Primary key on the left; secondary key on the right.", &[Action::Forward,Action::Back,Action::TurnLeft,Action::TurnRight,Action::StrafeLeft,Action::StrafeRight,Action::Jump,Action::Mobility][..]),
                ("Cards & rewards","Choose a binding to change it. The draft keys may share a key with movement and windows, which are idle during a draw.", &[Action::DrawCards,Action::Card1,Action::Card2,Action::Card3,Action::AutoPick,Action::Reroll,Action::Skip,Action::Banish][..]),
                ("Menus & overlays","Conflicting keys swap bindings. Esc cancels capture; Backspace clears a key. Keys use physical positions.", &[Action::Talents,Action::Character,Action::Inventory,Action::DamageMeter,Action::Grade,Action::Pause][..]),
            ] {
                section(body,f,title,note,|card| {
                    for &action in actions {
                        row(card,f,action.name(),|r| {
                            for slot in 0..2 {
                                button(r,f,Click::Bind(action,slot),if panel.capture==Some((action,slot)){"Press a key...".into()}else{key_label(settings.bindings[action as usize][slot])},140.0);
                            }
                        });
                    }
                });
            }
        }
        Tab::Sound => {
            section(body,f,"Volume levels","Changes apply immediately.",|card| {
                for (name,label) in [("MasterVolume","Master volume"),("SoundVolume","Sound effects"),("MusicVolume","Music"),("AmbienceVolume","Ambience")] {
                    step(card,f,label,format!("{:.0}%",value(cv,name)*100.0),Click::Volume(name,-1),Click::Volume(name,1));
                }
            });
            section(body,f,"Playback","Choose which parts of the soundscape you hear.",|card| cvar_flags(card,f,cv,&[("MasterSoundEffects","Enable sound"),("EnableMusic","Enable music"),("EnableAmbience","Enable ambience")]));
            section(body,f,"Mix protection","The limiter reduces distortion when many sounds overlap.",|card| cvar_flags(card,f,cv,&[("SoundOutputLimiter","Audio limiter")]));
        }
        Tab::Mouse => {
            section(body,f,"Camera rotation","Drag with either mouse button to rotate the camera. Hold both buttons to move forward.",|card| {
                step(card,f,"Sensitivity",format!("{:.1}x",rig.sensitivity),Click::Sensitivity(-1),Click::Sensitivity(1));
                flags(card,f,&[(5,"Invert horizontal axis",settings.invert_x),(6,"Invert vertical axis",settings.invert_y)]);
            });
            section(body,f,"Zoom & distance","Scroll the mouse wheel to change the current distance.",|card| {
                step(card,f,"Zoom speed",format!("{:.2}x",settings.zoom_speed),Click::Zoom(-1),Click::Zoom(1));
                step(card,f,"Current distance",format!("{:.1} m",rig.dist),Click::Distance(-1),Click::Distance(1));
                button(card,f,Click::DistanceReset,"Reset camera height and zoom",280.0);
            });
        }
        Tab::Display => {
            section(body,f,"Cards & progression","Full Auto opens level-up draws and picks the highest-priority offered card from your active build. Without an active build, your normal draw setting applies.",|card| flags(card,f,&[(9,"Automatically draw level-up cards",settings.auto_draw_cards),(12,"Full Auto: use active build priorities",settings.full_auto_build)]));
            section(body,f,"Floating combat text","Filters apply to new floating numbers and keep combat calculations unchanged.",|card| flags(card,f,&[(0,"Damage dealt",settings.damage),(1,"Healing received and done",settings.healing),(2,"Damage taken",settings.incoming),(3,"Show only outgoing critical hits",settings.critical_only)]));
            section(body,f,"Appearance","Character and Combat meter windows can be moved using their title bars.",|card| flags(card,f,&[(4,"Color grading and sharpening",settings.grade),(10,"Ally health bars",settings.ally_plates),(11,"Enemy health bars",settings.enemy_plates)]));
            section(body,f,"Performance & diagnostics","Diagnostics write reports to benilla-config/Diagnostics and turn off on the next launch. Per-system details require warcraft-survivors-perf.exe.",|card| flags(card,f,&[(7,"FPS counter",settings.show_fps),(8,"Performance diagnostics",settings.perf_diag)]));
        }
        Tab::Video => {
            section(body,f,"Window & synchronization","Fullscreen uses borderless mode. The browser manages its own window. Vertical sync reduces tearing.",|card| cvar_flags(card,f,cv,&[("gxWindow","Windowed mode"),("gxMaximize","Maximized window"),("gxVSync","Vertical sync")]));
            section(body,f,"World rendering","A shorter view distance reduces GPU load.",|card| step(card,f,"View distance",format!("{:.0} m",value(cv,"farclip")),Click::ViewDistance(-1),Click::ViewDistance(1)));
        }
        Tab::Help => {
            for (title,note) in [
                ("Hub & progression","Expeditions start a run in your chosen zone. Talents provide permanent character progression. Runes prepare your equipment in the hub."),
                ("Cards & drafting","In manual mode, T or the draw button opens all stored cards in a chain. During a draw: R rerolls, B activates banishment, S skips (all rebindable). Choose the card to ban, or press B again to cancel."),
                ("Items & shared chest","Right-click in your bags to equip or unequip an item. With the chest open, right-click to transfer. You can also drag items between windows; dropping elsewhere cancels the transfer."),
                ("Blacksmith","Choose a slot, item type, affix profile, rarity and item level. Select random or custom affixes, inspect the preview, then Craft. The training blacksmith also offers legendary effects."),
                ("Multiplayer","Host or join from character selection. Use the host's Hamachi IPv4 address. Shared chest items are available to everyone in the game."),
                ("Pause & menus","Menus pause solo runs. In multiplayer, the world keeps running: open menus in a safe place."),
            ] { section(body,f,title,note,|_| {}); }
        }
    }
}
