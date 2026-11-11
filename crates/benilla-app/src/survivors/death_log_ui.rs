//! Personal death recap, including immutable attacker-mod snapshots.
use super::*;
use crate::survivors::{Phase, Selection, combat_ui, hud};
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};

#[derive(Resource, Default)]
pub(in crate::survivors) struct View {
    pub open: bool,
    seen: Option<(u64, u32)>,
    rendered: Option<(bool, bool, Option<(u64, u32)>, (u32, u32))>,
}
#[derive(Component)]
struct Root;
#[derive(Component)]
struct Launcher;
#[derive(Component)]
struct Rows;
#[derive(Component)]
enum Action {
    Open,
    Close,
}
const GOLD: Color = Color::srgb(1.0, 0.82, 0.3);
const GREY: Color = Color::srgb(0.72, 0.73, 0.77);
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
pub(in crate::survivors) fn plugin(app: &mut App) {
    app.init_resource::<View>()
        .add_message::<MouseWheel>()
        .add_systems(
            Update,
            interact
                .after(combat_ui::input)
                .before(benilla_world::schedule::WorldStage::Input),
        )
        .add_systems(
            Update,
            render
                .after(super::super::hero::hero_vitals)
                .after(interact),
        );
}
fn interact(
    mut view: ResMut<View>,
    phase: Res<State<Phase>>,
    run: Res<Run>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mut selection: ResMut<Selection>,
    clicks: Query<(&Action, &Interaction), Changed<Interaction>>,
    mut wheel: MessageReader<MouseWheel>,
    mut rows: Query<
        (
            &ComputedNode,
            &bevy::ui::RelativeCursorPosition,
            &mut ScrollPosition,
        ),
        With<Rows>,
    >,
) {
    let available = *phase.get() == Phase::GameOver && !run.won && run.death_log.last.is_some();
    if !available
        || keys
            .as_deref()
            .is_some_and(|k| k.just_pressed(KeyCode::Escape))
    {
        view.open = false;
    }
    for (action, interaction) in &clicks {
        if *interaction == Interaction::Pressed {
            match action {
                Action::Open if available => view.open = true,
                Action::Open => {}
                Action::Close => view.open = false,
            }
        }
        if *interaction != Interaction::None {
            selection.pointer_captured = true;
        }
    }
    if view.open {
        selection.ui_captured = true;
        selection.pointer_captured = true;
    }
    for event in wheel.read() {
        if !view.open {
            continue;
        }
        let amount = match event.unit {
            MouseScrollUnit::Line => event.y * 32.0,
            MouseScrollUnit::Pixel => event.y,
        };
        for (node, cursor, mut scroll) in &mut rows {
            if cursor.cursor_over() {
                scroll.0.y = (scroll.0.y - amount).clamp(
                    0.0,
                    ((node.content_size.y - node.size.y) * node.inverse_scale_factor).max(0.0),
                );
            }
        }
    }
}
pub(in crate::survivors) fn launch_button(ui: &mut ChildSpawnerCommands, font: &Handle<Font>) {
    ui.spawn((
        Launcher,
        Node {
            display: Display::None,
            ..default()
        },
    ))
    .with_children(|row| {
        combat_ui::skin::button_sized(
            row,
            font,
            "Death Recap",
            Action::Open,
            260.0,
            42.0,
            18.0,
            false,
        );
    });
}
fn close_button(ui: &mut ChildSpawnerCommands, font: &Handle<Font>) {
    ui.spawn((
        Button,
        Action::Close,
        BackgroundColor(Color::srgb(0.075, 0.065, 0.045)),
        BorderColor::all(Color::srgb(0.32, 0.26, 0.15)),
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
    .with_children(|button| {
        button.spawn(text(font, "Close", 13.0, Color::srgb(0.88, 0.85, 0.75)));
    });
}
pub(super) fn school(tags: u64) -> String {
    let tags = Tags(tags);
    let mut names = Vec::new();
    for (school, name) in [
        (Tags::PHYSICAL, "Physical"),
        (Tags::FIRE, "Fire"),
        (Tags::COLD, "Cold"),
        (Tags::SHADOW, "Shadow"),
        (Tags::ARCANE, "Arcane"),
        (Tags::HOLY, "Holy"),
    ] {
        if tags.intersects(school) {
            names.push(name);
        }
    }
    if tags.intersects(Tags::LIGHTNING) {
        names.push("Lightning / Nature");
    } else if tags.intersects(Tags::NATURE) {
        names.push("Nature");
    }
    if names.is_empty() {
        "Unknown school".into()
    } else {
        names.join(" + ")
    }
}
fn description(event: &Event, death: f32) -> (String, String, String) {
    let result = match event.outcome {
        Outcome::Blocked => "BLOCKED".into(),
        Outcome::Dodged => "DODGED".into(),
        Outcome::Absorbed => "ABSORBED".into(),
        Outcome::Hit => format!("{:.0} damage ({})", event.damage, school(event.tags)),
    };
    let flags = format!(
        "{}{}{}",
        if event.lethal { " · FATAL HIT" } else { "" },
        if event.source.critical {
            " · CRITICAL"
        } else {
            ""
        },
        if Tags(event.tags).contains(Tags::DOT) {
            " · DoT"
        } else {
            ""
        }
    );
    let ability = if event.ability == event.source.name {
        if Tags(event.tags).contains(Tags::MELEE) {
            "Melee attack"
        } else if Tags(event.tags).contains(Tags::PROJECTILE) {
            "Projectile"
        } else {
            "Direct hit"
        }
    } else {
        &event.ability
    };
    let identity = event
        .source
        .id
        .map_or(String::new(), |id| format!(" #{id:X}"));
    let headline = format!(
        "−{:.2} s · {result}{flags}\n{}{} · Type: {} · {ability}",
        (death - event.at).max(0.0),
        event.source.name,
        identity,
        event.source.kind()
    );
    let mods = match event.source.mods {
        Some(rank) if rank.count() == 0 => "Monster modifiers: None".into(),
        Some(rank) => format!("Monster modifiers: {}", rank.labels().join(" · ")),
        None => "Monster modifiers: Unavailable for this source".into(),
    };
    let detail = format!(
        "Health {:.0} → {:.0} · Raw {:.0} · Absorbed {:.0}{}",
        event.before,
        event.after,
        event.raw,
        event.absorbed,
        if event.overkill > 0.0 {
            format!(" · Overkill {:.0}", event.overkill)
        } else {
            String::new()
        }
    );
    (headline, mods, detail)
}
#[allow(clippy::too_many_arguments)]
fn render(
    mut commands: Commands,
    run: Res<Run>,
    phase: Res<State<Phase>>,
    font: Option<Res<hud::HudFont>>,
    mut view: ResMut<View>,
    roots: Query<Entity, With<Root>>,
    mut launchers: Query<&mut Node, With<Launcher>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    scale: Option<Res<UiScale>>,
) {
    let Some(font) = font else {
        return;
    };
    let report = run.death_log.last.as_ref();
    let key = report.map(|r| (r.id, r.at.to_bits()));
    if view.seen != key {
        view.seen = key;
        view.open = false;
    }
    let available = report.is_some() && *phase.get() == Phase::GameOver && !run.won;
    if !available {
        view.open = false;
    }
    for mut node in &mut launchers {
        node.display = if available {
            Display::Flex
        } else {
            Display::None
        };
    }
    let viewport = windows.single().map_or(Vec2::new(1365.0, 768.0), |w| {
        Vec2::new(w.width(), w.height()) / scale.as_deref().map_or(1.0, |s| s.0).max(0.01)
    });
    let state = (
        available,
        view.open,
        key,
        (viewport.x.to_bits(), viewport.y.to_bits()),
    );
    if view.rendered == Some(state) {
        return;
    }
    view.rendered = Some(state);
    for root in &roots {
        commands.entity(root).despawn();
    }
    let Some(report) = report.filter(|_| available && view.open) else {
        return;
    };
    let width = (viewport.x - 32.0).max(1.0).min(1100.0);
    let height = (viewport.y - 32.0).max(1.0).min(780.0);
    commands.spawn((Root,GlobalZIndex(1900),Interaction::default(),
        BackgroundColor(Color::srgba(0.0,0.0,0.0,0.72)),Node {
            position_type:PositionType::Absolute,width:Val::Percent(100.0),height:Val::Percent(100.0),
            align_items:AlignItems::Center,justify_content:JustifyContent::Center,..default()
        })).with_children(|overlay| {
        overlay.spawn((BackgroundColor(Color::srgb(0.035,0.03,0.025)),
            BorderColor::all(Color::srgb(0.65,0.49,0.17)),Node {
                width:Val::Px(width),height:Val::Px(height),max_width:Val::Percent(100.0),
                padding:UiRect::all(Val::Px(16.0)),border:UiRect::all(Val::Px(2.0)),
                flex_direction:FlexDirection::Column,row_gap:Val::Px(12.0),overflow:Overflow::clip(),..default()
            })).with_children(|ui| {
            ui.spawn(Node {width:Val::Percent(100.0),align_items:AlignItems::Center,
                column_gap:Val::Px(12.0),flex_shrink:0.0,..default()}).with_children(|header| {
                header.spawn(Node {flex_grow:1.0,min_width:Val::Px(0.0),flex_direction:FlexDirection::Column,
                    row_gap:Val::Px(3.0),..default()}).with_children(|title| {
                    title.spawn(text(&font.0,"Death Recap",21.0,GOLD));
                    title.spawn(text(&font.0,"Last 10 seconds · Most recent events first",12.0,GREY));
                });
                close_button(header,&font.0);
            });
            ui.spawn(Node {width:Val::Percent(100.0),flex_wrap:FlexWrap::Wrap,
                column_gap:Val::Px(8.0),row_gap:Val::Px(8.0),flex_shrink:0.0,..default()}).with_children(|cards| {
                for (label,value,color) in [
                    ("Damage taken",format!("{:.0}",report.total_damage()),GOLD),
                    ("Damage absorbed",format!("{:.0}",report.total_absorbed()),Color::srgb(0.78,0.6,1.0)),
                    ("Events",report.events.len().to_string(),Color::WHITE),
                ] {
                    cards.spawn((BackgroundColor(Color::srgb(0.065,0.055,0.045)),
                        BorderColor::all(Color::srgb(0.26,0.22,0.14)),Node {flex_grow:1.0,flex_basis:Val::Px(0.0),
                            min_width:Val::Px(100.0),padding:UiRect::all(Val::Px(10.0)),border:UiRect::all(Val::Px(1.0)),
                            flex_direction:FlexDirection::Column,row_gap:Val::Px(5.0),..default() })).with_children(|card| {
                        card.spawn(text(&font.0,label,11.0,GREY));
                        card.spawn(text(&font.0,value,19.0,color));
                    });
                }
            });
            if report.truncated {
                ui.spawn(text(&font.0,"Dense combat: some of the oldest events could not be retained.",12.0,GOLD));
            }
            ui.spawn((Rows,Interaction::default(),bevy::ui::RelativeCursorPosition::default(),ScrollPosition::default(),
                Node {width:Val::Percent(100.0),min_height:Val::Px(0.0),flex_grow:1.0,flex_basis:Val::Px(0.0),
                    flex_direction:FlexDirection::Column,row_gap:Val::Px(10.0),overflow:Overflow::scroll_y(),
                    padding:UiRect::right(Val::Px(8.0)),..default()})).with_children(|rows| {
                if report.events.is_empty() {
                    rows.spawn(text(&font.0,"No damage recorded in the 10 seconds before this death.",14.0,GREY));
                }
                for event in report.events.iter().rev() {
                    let (headline,mods,detail)=description(event,report.at);
                    rows.spawn((BackgroundColor(if event.lethal {Color::srgb(0.16,0.045,0.03)} else {Color::srgb(0.055,0.047,0.037)}),
                        BorderColor::all(if event.lethal {Color::srgb(0.55,0.18,0.10)} else {Color::srgb(0.23,0.19,0.12)}),
                        Node {width:Val::Percent(100.0),min_width:Val::Px(0.0),flex_shrink:0.0,
                            padding:UiRect::all(Val::Px(12.0)),border:UiRect::all(Val::Px(1.0)),
                            flex_direction:FlexDirection::Column,row_gap:Val::Px(6.0),..default()})).with_children(|row| {
                        row.spawn(text(&font.0,headline,15.0,if event.lethal {Color::srgb(1.0,0.5,0.4)} else {GOLD}));
                        row.spawn(text(&font.0,mods,13.0,Color::srgb(0.94,0.92,0.85)));
                        row.spawn(text(&font.0,detail,12.0,GREY));
                    });
                }
            });
            ui.spawn(text(&font.0,"Personal recap · Press Esc to return to the death screen",11.0,GREY));
        });
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    fn press(app: &mut App, open: bool) {
        let actions: Vec<_> = app
            .world_mut()
            .query::<(Entity, &Action)>()
            .iter(app.world())
            .map(|(entity, action)| (entity, matches!(action, Action::Open)))
            .collect();
        for (entity, _) in &actions {
            app.world_mut()
                .entity_mut(*entity)
                .insert(Interaction::None);
        }
        let entity = actions
            .into_iter()
            .find(|(_, is_open)| *is_open == open)
            .unwrap()
            .0;
        app.world_mut()
            .entity_mut(entity)
            .insert(Interaction::Pressed);
        app.update();
    }
    #[test]
    fn recap_opens_only_from_the_death_screen_button_and_can_be_closed_and_reopened() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<Run>()
            .init_resource::<View>()
            .init_resource::<Selection>()
            .add_message::<MouseWheel>()
            .insert_resource(State::new(Phase::GameOver))
            .insert_resource(hud::HudFont(Handle::default()))
            .add_systems(
                Startup,
                |mut commands: Commands, font: Res<hud::HudFont>| {
                    commands
                        .spawn(hud::OverRoot)
                        .with_children(|ui| launch_button(ui, &font.0));
                },
            )
            .add_systems(Update, (interact, render).chain());
        let actor = Entity::from_raw_u32(123).unwrap();
        {
            let mut run = app.world_mut().resource_mut::<Run>();
            run.death_log
                .remember(actor, "Anvilrage Captain", Rank::Brutal.with(Rank::Echo));
            run.death_log.record(
                20.0,
                Some(actor),
                "Melee",
                Tags::PHYSICAL | Tags::MELEE,
                200.0,
                200.0,
                10.0,
                100.0,
                Outcome::Hit,
            );
        }
        app.update();
        assert!(
            !app.world().resource::<View>().open,
            "death must not open the recap automatically"
        );
        assert_eq!(
            app.world_mut().query::<&Root>().iter(app.world()).count(),
            0
        );
        let launcher = app
            .world_mut()
            .query_filtered::<Entity, With<Launcher>>()
            .single(app.world())
            .unwrap();
        let parent = app.world().get::<ChildOf>(launcher).unwrap().parent();
        assert!(
            app.world().get::<hud::OverRoot>(parent).is_some(),
            "button belongs to the death screen"
        );
        assert_eq!(
            app.world().get::<Node>(launcher).unwrap().display,
            Display::Flex
        );
        press(&mut app, true);
        assert!(app.world().resource::<View>().open);
        let texts: Vec<_> = app
            .world_mut()
            .query::<&Text>()
            .iter(app.world())
            .map(|t| t.0.clone())
            .collect();
        assert!(texts.iter().any(|t| t.contains("FATAL HIT")
            && t.contains("Anvilrage Captain")
            && t.contains("Type: Melee")));
        assert!(
            texts
                .iter()
                .any(|t| t.contains("Brutal") && t.contains("Echo"))
        );
        press(&mut app, false);
        assert_eq!(
            app.world_mut().query::<&Root>().iter(app.world()).count(),
            0
        );
        app.update();
        assert!(!app.world().resource::<View>().open);
        press(&mut app, true);
        assert_eq!(
            app.world_mut().query::<&Root>().iter(app.world()).count(),
            1
        );
        app.world_mut()
            .resource_mut::<Run>()
            .death_log
            .last
            .as_mut()
            .unwrap()
            .id += 1;
        app.update();
        assert!(
            !app.world().resource::<View>().open,
            "a new death must also wait for a click"
        );
        *app.world_mut().resource_mut::<State<Phase>>() = State::new(Phase::Playing);
        app.update();
        assert_eq!(
            app.world().get::<Node>(launcher).unwrap().display,
            Display::None
        );
        assert_eq!(
            school((Tags::LIGHTNING | Tags::NATURE).0),
            "Lightning / Nature"
        );
    }
}
