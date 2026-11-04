//! In-game character sheet (pauses combat) and movable damage meter.
use super::{
    data, hud,
    options::{Action, Settings},
    Phase, Run, Selection,
};
use bevy::{prelude::*, window::PrimaryWindow};
mod bags;
mod boss_bars;
mod meter;
mod preview;
mod sheet;
pub(super) mod skin;
pub(super) use meter::Meter;
const GOLD: Color = Color::srgb(1.0, 0.82, 0.2);
const GREY: Color = Color::srgb(0.65, 0.65, 0.65);
const METER_WIDTH: f32 = 306.0;
#[derive(Clone, Copy, Debug)]
pub(super) enum Pane {
    Meter,
    Sheet,
}
#[derive(Clone, Copy)]
struct Drag {
    pane: Pane,
    cursor: Vec2,
    origin: Vec2,
}
#[derive(Resource)]
pub(super) struct View {
    meter: bool,
    position: Vec2,
    sheet: Vec2,
    pub selected: usize,
    pub yaw: f32,
    bag_page: usize,
    selected_item: Option<u64>,
    stats_collapsed: u8,
    stats_scroll: f32,
    meter_scroll: f32,
    drag: Option<Drag>,
}
impl Default for View {
    fn default() -> Self {
        let mut view = Self {
            meter: true,
            position: Vec2::new(1000.0, 440.0),
            sheet: Vec2::new(24.0, 30.0),
            selected: 0,
            yaw: 0.35,
            bag_page: 0,
            selected_item: None,
            stats_collapsed: 0b111100,
            stats_scroll: 0.0,
            meter_scroll: 0.0,
            drag: None,
        };
        if let Some(text) = crate::local_state::home().and_then(|home| {
            crate::local_state::read_to_string(&home.join("survivors-panels.txt")).ok()
        }) {
            let fields: Vec<_> = text.split_whitespace().collect();
            if fields.len() == 6 && fields[0] == "v1" {
                let numbers: Vec<f32> = fields[2..]
                    .iter()
                    .filter_map(|s| s.parse::<f32>().ok())
                    .collect();
                if numbers.len() == 4 && numbers.iter().all(|v| v.is_finite() && *v >= 0.0) {
                    view.meter = fields[1] != "false";
                    view.position = Vec2::new(numbers[0], numbers[1]);
                    view.sheet = Vec2::new(numbers[2], numbers[3]);
                }
            }
        }
        view
    }
}
impl View {
    fn save(&self) {
        if let Some(home) = crate::local_state::home() {
            let text = format!(
                "v1 {} {} {} {} {}",
                self.meter, self.position.x, self.position.y, self.sheet.x, self.sheet.y
            );
            if let Err(error) =
                crate::local_state::write_atomic(&home.join("survivors-panels.txt"), &text)
            {
                warn!("Could not save panel positions: {error}");
            }
        }
    }
}
#[derive(Component, Clone, Copy)]
pub(super) enum Click {
    Character,
    Meter,
    CloseSheet,
    CloseBags,
    ToggleStats(u8),
    CloseMeter,
    Skill(usize),
    Rotate(f32),
    Bags,
}
#[derive(Component)]
pub(super) struct DragTitle(Pane);
#[derive(Component)]
struct MeterRoot;
#[derive(Component)]
struct MeterRows;
#[derive(Component)]
struct Launcher;
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<View>()
        .add_message::<bevy::input::mouse::MouseWheel>()
        .add_systems(
            Update,
            scroll_meter
                .after(input)
                .before(render_meter)
                .before(benilla_world::schedule::WorldStage::Input),
        )
        .add_systems(
            Update,
            input.before(benilla_world::schedule::WorldStage::Input),
        )
        .add_systems(
            Update,
            (render_meter, launchers, position)
                .chain()
                .after(input)
                .after(super::abilities::apply_hits),
        );
    sheet::plugin(app);
    skin::plugin(app);
    preview::plugin(app);
    bags::plugin(app);
    boss_bars::plugin(app);
}
fn in_world(phase: Phase) -> bool {
    matches!(phase, Phase::Hub | Phase::Playing | Phase::CharacterSheet)
}
#[allow(clippy::too_many_arguments)]
pub(super) fn input(
    phase: Res<State<Phase>>,
    mut next: ResMut<NextState<Phase>>,
    settings: Res<Settings>,
    options: Res<super::options::Panel>,
    mut selection: ResMut<Selection>,
    mut roster: ResMut<super::characters::Roster>,
    mut view: ResMut<View>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    scale: Option<Res<UiScale>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    clicks: Query<(&Interaction, &Click), Changed<Interaction>>,
    titles: Query<(&Interaction, &DragTitle)>,
) {
    selection.ui_captured = false;
    if *phase.get() == Phase::Menu {
        selection.sheet_open = false;
        selection.bags_open = false;
        view.drag = None;
        return;
    }
    let active = in_world(*phase.get());
    if !active {
        selection.sheet_open = false;
        selection.bags_open = false;
        if view
            .drag
            .is_some_and(|drag| matches!(drag.pane, Pane::Sheet))
        {
            view.drag = None;
        }
    }
    if options.open {
        return;
    }
    let toggle = settings.just_pressed(Action::Character, &keys)
        || clicks
            .iter()
            .any(|(i, c)| *i == Interaction::Pressed && matches!(c, Click::Character));
    if active && toggle {
        selection.sheet_open = !selection.sheet_open;
        if selection.sheet_open {
            selection.talents_open = false;
            roster.expeditions = false;
        }
    }
    let bags = settings.just_pressed(Action::Inventory, &keys)
        || clicks
            .iter()
            .any(|(i, c)| *i == Interaction::Pressed && matches!(c, Click::Bags));
    if active && bags {
        selection.bags_open = !selection.bags_open;
        roster.expeditions = false;
    }
    if settings.just_pressed(Action::DamageMeter, &keys) {
        view.meter = !view.meter;
        view.save();
    }
    if (selection.sheet_open || selection.bags_open)
        && (keys.just_pressed(KeyCode::Escape) || settings.just_pressed(Action::Pause, &keys))
    {
        selection.sheet_open = false;
        selection.bags_open = false;
        keys.clear_just_pressed(KeyCode::Escape);
    }
    for (interaction, click) in &clicks {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *click {
            Click::Character => {}
            Click::Meter => {
                view.meter = !view.meter;
                view.save();
            }
            Click::CloseMeter => {
                view.meter = false;
                view.save();
            }
            Click::CloseSheet => selection.sheet_open = false,
            Click::CloseBags => selection.bags_open = false,
            Click::ToggleStats(category) => {
                if category < 6 {
                    view.stats_collapsed ^= 1 << category;
                }
            }
            Click::Skill(index) => view.selected = index,
            Click::Rotate(delta) => view.yaw += delta,
            Click::Bags => {}
        }
    }
    if (selection.sheet_open || selection.bags_open || selection.talents_open)
        && matches!(*phase.get(), Phase::Hub | Phase::Playing)
    {
        next.set(Phase::CharacterSheet);
    } else if !selection.sheet_open
        && !selection.bags_open
        && !selection.talents_open
        && *phase.get() == Phase::CharacterSheet
    {
        next.set(if selection.in_hub {
            Phase::Hub
        } else {
            Phase::Playing
        });
    }
    let ui_scale = scale.as_deref().map_or(1.0, |s| s.0).max(0.01);
    let cursor = windows
        .single()
        .ok()
        .and_then(Window::cursor_position)
        .map(|cursor| cursor / ui_scale);
    if mouse.just_pressed(MouseButton::Left) {
        if let Some(cursor) = cursor {
            if let Some((_, title)) = titles.iter().find(|(i, title)| {
                **i == Interaction::Pressed && (active || matches!(title.0, Pane::Meter))
            }) {
                let origin = match title.0 {
                    Pane::Meter => view.position,
                    Pane::Sheet => view.sheet,
                };
                view.drag = Some(Drag {
                    pane: title.0,
                    cursor,
                    origin,
                });
            }
        }
    }
    if let Some(drag) = view.drag {
        if mouse.pressed(MouseButton::Left) {
            selection.ui_captured = true;
            if let Some(cursor) = cursor {
                let point = drag.origin + cursor - drag.cursor;
                match drag.pane {
                    Pane::Meter => view.position = point,
                    Pane::Sheet => view.sheet = point,
                }
            }
        } else {
            view.drag = None;
            view.save();
        }
    }
    if let Ok(window) = windows.single() {
        let size = Vec2::new(window.width(), window.height()) / ui_scale;
        view.position = view.position.clamp(
            Vec2::ZERO,
            (size - Vec2::new(METER_WIDTH, 300.0)).max(Vec2::ZERO),
        );
        view.sheet = view.sheet.clamp(
            Vec2::ZERO,
            (size - Vec2::new(824.0, sheet::HEIGHT)).max(Vec2::ZERO),
        );
    }
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
        TextShadow {
            offset: Vec2::splat(1.0),
            ..default()
        },
    )
}
fn button(
    ui: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    click: Click,
    label: impl Into<String>,
    width: f32,
) {
    skin::button(ui, font, label, click, width);
}
fn position(
    view: Res<View>,
    mut roots: Query<
        (&mut Node, Option<&MeterRoot>, Option<&sheet::Root>),
        Or<(With<MeterRoot>, With<sheet::Root>)>,
    >,
) {
    for (mut node, meter, sheet) in &mut roots {
        let point = if meter.is_some() {
            view.position
        } else if sheet.is_some() {
            view.sheet
        } else {
            continue;
        };
        node.left = Val::Px(point.x);
        node.top = Val::Px(point.y);
    }
}
fn launchers(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    font: Option<Res<hud::HudFont>>,
    roots: Query<Entity, With<Launcher>>,
    mut cache: Local<Option<Phase>>,
) {
    let Some(font) = font else {
        return;
    };
    if *cache == Some(*phase.get()) {
        return;
    }
    *cache = Some(*phase.get());
    for e in &roots {
        commands.entity(e).despawn();
    }
    if !in_world(*phase.get()) {
        return;
    }
    commands
        .spawn((
            Launcher,
            GlobalZIndex(445),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(16.0),
                bottom: Val::Px(85.0),
                column_gap: Val::Px(6.0),
                ..default()
            },
        ))
        .with_children(|ui| {
            button(ui, &font.0, Click::Character, "Character", 105.0);
            button(ui, &font.0, Click::Meter, "Damage meter", 120.0);
            button(ui, &font.0, Click::Bags, "Bags", 80.0);
        });
}
#[allow(clippy::too_many_arguments)]
fn render_meter(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    run: Res<Run>,
    view: Res<View>,
    font: Option<Res<hud::HudFont>>,
    spells: Option<Res<crate::ui_action::Spells>>,
    roots: Query<Entity, With<MeterRoot>>,
    mut cache: Local<String>,
) {
    let Some(font) = font else {
        return;
    };
    let show = view.meter && *phase.get() != Phase::Menu;
    let key = format!("{} {:?} {}", show, phase.get(), (run.elapsed * 4.0) as u64);
    if *cache == key {
        return;
    }
    *cache = key;
    for e in &roots {
        commands.entity(e).despawn();
    }
    if !show {
        return;
    }
    let rows = run.meter.rows(run.elapsed);
    let total: f64 = rows.iter().map(|r| r.dps).sum();
    let max = rows.first().map_or(1.0, |r| r.dps.max(1.0));
    let f = &font.0;
    commands
        .spawn((
            MeterRoot,
            GlobalZIndex(465),
            BackgroundColor(Color::srgba(0.025, 0.02, 0.02, 0.94)),
            BorderColor::all(GOLD),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(view.position.x),
                top: Val::Px(view.position.y),
                width: Val::Px(METER_WIDTH),
                padding: UiRect::all(Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                ..default()
            },
        ))
        .with_children(|ui| {
            ui.spawn((
                Button,
                DragTitle(Pane::Meter),
                BackgroundColor(Color::srgb(0.18, 0.10, 0.035)),
                Node {
                    height: Val::Px(28.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::horizontal(Val::Px(4.0)),
                    ..default()
                },
            ))
            .with_children(|header| {
                header.spawn(text(f, "Damage / s - 5s | drag / scroll", 14.0, GOLD));
                button(header, f, Click::CloseMeter, "X", 24.0);
            });
            if rows.is_empty() {
                ui.spawn(text(f, "Waiting for damage...", 13.0, GREY));
            }
            ui.spawn((
                MeterRows,
                Interaction::default(),
                bevy::ui::RelativeCursorPosition::default(),
                ScrollPosition(Vec2::new(0.0, view.meter_scroll)),
                Node {
                    width: Val::Percent(100.0),
                    max_height: Val::Px(240.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(3.0),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
            ))
            .with_children(|ui| {
                for row in &rows {
                    let def = data::ABILITIES.iter().find(|d| d.key == row.key);
                    let name = def.map_or_else(
                        || row.key.replace('_', " "),
                        |d| hud::spell_name(spells.as_deref(), d.spell, d.key),
                    );
                    ui.spawn(Node {
                        height: Val::Px(23.0),
                        flex_shrink: 0.0,
                        width: Val::Percent(100.0),
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|line| {
                        line.spawn((
                            BackgroundColor(hud::rgb(run.class.color).with_alpha(0.40)),
                            Node {
                                position_type: PositionType::Absolute,
                                width: Val::Percent((row.dps / max * 100.0) as f32),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                        ));
                        line.spawn((
                            text(
                                f,
                                format!("{}  {:.1} | {:.0}", name, row.dps, row.total),
                                13.0,
                                Color::WHITE,
                            ),
                            Node {
                                padding: UiRect::horizontal(Val::Px(4.0)),
                                ..default()
                            },
                        ));
                    });
                }
            });
            ui.spawn(text(
                f,
                format!("Total: {total:.1} DPS | damage this run"),
                12.0,
                GOLD,
            ));
        });
}

fn scroll_meter(
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    mut view: ResMut<View>,
    mut selection: ResMut<Selection>,
    mut rows: Query<
        (
            &ComputedNode,
            &bevy::ui::RelativeCursorPosition,
            &mut ScrollPosition,
        ),
        With<MeterRows>,
    >,
) {
    for event in wheel.read() {
        if !view.meter {
            continue;
        }
        let amount = match event.unit {
            bevy::input::mouse::MouseScrollUnit::Line => event.y * 28.0,
            bevy::input::mouse::MouseScrollUnit::Pixel => event.y,
        };
        for (node, cursor, mut scroll) in &mut rows {
            if !cursor.cursor_over() {
                continue;
            }
            selection.ui_captured = true;
            scroll.0.y = (scroll.0.y - amount).clamp(
                0.0,
                ((node.content_size.y - node.size.y) * node.inverse_scale_factor).max(0.0),
            );
            view.meter_scroll = scroll.0.y;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn low_damage_milestones_remain_in_the_scrollable_meter_after_the_tenth_source() {
        let mut run = Run {
            elapsed: 5.0,
            ..default()
        };
        for key in [
            "cleave",
            "rend",
            "heroic_strike",
            "whirlwind",
            "execute",
            "mortal_strike",
            "thunder_clap",
            "fireball",
            "frostbolt",
            "arcane_explosion",
            "Bloodbath",
            "Windfury Weapon",
        ] {
            run.meter.record(key, 5.0, 100.0);
        }
        run.meter.record("Deep Wounds", 5.0, 1.0);
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<Phase>()
            .insert_resource(run)
            .insert_resource(View {
                meter: true,
                meter_scroll: 46.0,
                ..default()
            })
            .insert_resource(hud::HudFont(Handle::default()))
            .add_systems(Update, render_meter);
        app.world_mut()
            .resource_mut::<NextState<Phase>>()
            .set(Phase::Playing);
        app.update();
        let world = app.world_mut();
        assert!(world
            .query::<&Text>()
            .iter(world)
            .any(|text| text.0.starts_with("Deep Wounds")));
        assert_eq!(
            world
                .query_filtered::<&ScrollPosition, With<MeterRows>>()
                .single(world)
                .unwrap()
                .0
                .y,
            46.0
        );
        world.resource_mut::<Run>().elapsed += 0.3;
        app.update();
        let world = app.world_mut();
        assert_eq!(
            world
                .query_filtered::<&ScrollPosition, With<MeterRows>>()
                .single(world)
                .unwrap()
                .0
                .y,
            46.0
        );
    }
    #[test]
    fn bags_and_character_open_and_close_independently_and_resume_only_after_both_close() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<Phase>()
            .init_resource::<Settings>()
            .init_resource::<super::super::options::Panel>()
            .init_resource::<Selection>()
            .init_resource::<super::super::characters::Roster>()
            .init_resource::<View>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_systems(OnEnter(Phase::CharacterSheet), super::super::pause)
            .add_systems(OnEnter(Phase::Playing), super::super::unpause)
            .add_systems(Update, (input, hud::keys).chain());
        app.world_mut()
            .resource_mut::<NextState<Phase>>()
            .set(Phase::Playing);
        app.update();
        for (click, sheet, bags) in [
            (Click::Bags, false, true),
            (Click::Character, true, true),
            (Click::CloseSheet, false, true),
            (Click::Character, true, true),
            (Click::CloseBags, true, false),
            (Click::CloseSheet, false, false),
        ] {
            let button = app.world_mut().spawn((Interaction::Pressed, click)).id();
            app.update();
            app.world_mut().despawn(button);
            app.update();
            let selection = app.world().resource::<Selection>();
            assert_eq!((selection.sheet_open, selection.bags_open), (sheet, bags));
            assert_eq!(
                *app.world().resource::<State<Phase>>().get(),
                if sheet || bags {
                    Phase::CharacterSheet
                } else {
                    Phase::Playing
                }
            );
            assert_eq!(
                app.world().resource::<Time<Virtual>>().is_paused(),
                sheet || bags
            );
        }
    }
    #[test]
    fn character_sheet_pauses_and_all_close_controls_resume_the_run() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<Phase>()
            .init_resource::<Settings>()
            .init_resource::<super::super::options::Panel>()
            .init_resource::<Selection>()
            .init_resource::<super::super::characters::Roster>()
            .init_resource::<View>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_systems(OnEnter(Phase::CharacterSheet), super::super::pause)
            .add_systems(OnEnter(Phase::Playing), super::super::unpause)
            .add_systems(Update, (input, hud::keys).chain());
        app.world_mut().resource_mut::<Selection>().in_hub = false;
        app.world_mut()
            .resource_mut::<NextState<Phase>>()
            .set(Phase::Playing);
        app.update();
        for (open, close) in [
            (KeyCode::KeyC, Some(KeyCode::KeyC)),
            (KeyCode::KeyB, Some(KeyCode::KeyB)),
            (KeyCode::KeyB, Some(KeyCode::Escape)),
            (KeyCode::KeyC, None),
        ] {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(open);
            app.update();
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .reset_all();
            app.update();
            assert_eq!(
                *app.world().resource::<State<Phase>>().get(),
                Phase::CharacterSheet
            );
            assert_eq!(
                app.world().resource::<Selection>().sheet_open,
                open == KeyCode::KeyC
            );
            assert_eq!(
                app.world().resource::<Selection>().bags_open,
                open == KeyCode::KeyB
            );
            assert!(app.world().resource::<Time<Virtual>>().is_paused());
            let button = if let Some(key) = close {
                app.world_mut()
                    .resource_mut::<ButtonInput<KeyCode>>()
                    .press(key);
                None
            } else {
                Some(
                    app.world_mut()
                        .spawn((Interaction::Pressed, Click::CloseSheet))
                        .id(),
                )
            };
            app.update();
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .reset_all();
            if let Some(button) = button {
                app.world_mut().despawn(button);
            }
            app.update();
            assert_eq!(
                *app.world().resource::<State<Phase>>().get(),
                Phase::Playing
            );
            assert!(!app.world().resource::<Selection>().sheet_open);
            assert!(!app.world().resource::<Selection>().bags_open);
            assert!(!app.world().resource::<Time<Virtual>>().is_paused());
        }
    }
    #[test]
    fn dragging_uses_ui_coordinates_and_escape_closes_the_sheet_without_pausing() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<Phase>()
            .init_resource::<Settings>()
            .init_resource::<super::super::options::Panel>()
            .init_resource::<Selection>()
            .init_resource::<super::super::characters::Roster>()
            .init_resource::<View>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .insert_resource(UiScale(2.0))
            .add_systems(Update, (input, hud::keys).chain());
        let mut window = Window::default();
        window.set_cursor_position(Some(Vec2::new(100.0, 100.0)));
        let window = app.world_mut().spawn((PrimaryWindow, window)).id();
        app.world_mut().resource_mut::<View>().position = Vec2::new(20.0, 20.0);
        app.world_mut()
            .spawn((Interaction::Pressed, DragTitle(Pane::Meter)));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.world_mut()
            .resource_mut::<NextState<Phase>>()
            .set(Phase::Hub);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear_just_pressed(MouseButton::Left);
        app.world_mut()
            .entity_mut(window)
            .get_mut::<Window>()
            .unwrap()
            .set_cursor_position(Some(Vec2::new(160.0, 140.0)));
        app.update();
        assert_eq!(
            app.world().resource::<View>().position,
            Vec2::new(50.0, 40.0)
        );
        assert!(app.world().resource::<Selection>().ui_captured);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.update();
        assert!(!app.world().resource::<Selection>().ui_captured);
        app.world_mut().resource_mut::<Selection>().sheet_open = true;
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        app.update();
        assert!(!app.world().resource::<Selection>().sheet_open);
        assert_eq!(*app.world().resource::<State<Phase>>().get(), Phase::Hub);
        app.world_mut()
            .resource_mut::<NextState<Phase>>()
            .set(Phase::LevelUp);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F6);
        app.update();
        assert!(!app.world().resource::<View>().meter);
        assert_eq!(
            *app.world().resource::<State<Phase>>().get(),
            Phase::LevelUp
        );
    }
}
