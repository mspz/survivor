//! In-game character sheet (pauses combat) and movable damage meter.
use super::{
    data, hud,
    options::{Action, Settings},
    Phase, Run, Selection,
};
use bevy::{prelude::*, window::PrimaryWindow};
pub(super) mod bags;
mod boss_bars;
mod meter;
mod buff_meter;
pub(super) use buff_meter::EffectMeter;
pub(super) use buff_meter::{control_changes as track_control_changes, track as track_effects};
mod meter_panel;
mod meter_debug;
use meter_panel::render_meter;
mod preview;
mod sheet;
pub(super) mod skin;
pub(super) use meter::Meter;
pub(in crate::survivors) use meter::DamageEvent;
pub(super) use meter_panel::{PartyDamage, PartyRow};
const GOLD: Color = Color::srgb(1.0, 0.82, 0.2);
const GREY: Color = Color::srgb(0.65, 0.65, 0.65);
const METER_WIDTH: f32 = 280.0;
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
    details_scroll: f32,
    debug_scroll: f32,
    debug_open: bool,
    debug_tab: meter_debug::Tab,
    meter_selected: Option<String>,
    meter_tab: meter_panel::Tab,
    drag: Option<Drag>,
    /// The character sheet shows the Runes tab instead of the paper doll.
    rune_page: bool,
    /// The item whose Delete button was pressed once, awaiting confirmation.
    delete_armed: Option<u64>,
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
            details_scroll: 0.0,
            debug_scroll: 0.0,
            debug_open: false,
            debug_tab: default(),
            meter_selected: None,
            meter_tab: default(),
            drag: None,
            rune_page: false,
            delete_armed: None,
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
                crate::local_state::write_queued(&home.join("survivors-panels.txt"), &text)
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
    CloseMeterDetails,
    Skill(usize),
    Rotate(f32),
    Bags,
    /// Character sheet tab: false = Character, true = Runes.
    SheetPage(bool),
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
            meter_panel::interact
                .after(input)
                .before(render_meter)
                .before(benilla_world::schedule::WorldStage::Input),
        )
        .add_systems(
            Update,
            (render_meter, launchers, position)
                .chain()
                .after(input)
                .after(super::abilities::apply_hits)
                .after(track_effects),
        );
    meter_debug::plugin(app);
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
    selection.pointer_captured = false;
    if *phase.get() == Phase::Menu {
        selection.sheet_open = false;
        selection.bags_open = false;
        view.drag = None;
        view.meter_selected = None;
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
    if view.meter_selected.is_some() && keys.just_pressed(KeyCode::Escape) {
        if view.debug_open {
            view.debug_open = false;
        } else {
            view.meter_selected = None;
        }
        keys.clear_just_pressed(KeyCode::Escape);
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
            Click::CloseMeterDetails => view.meter_selected = None,
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
            Click::SheetPage(runes) => view.rune_page = runes,
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
            match drag.pane {
                Pane::Meter => selection.pointer_captured = true,
                Pane::Sheet => selection.ui_captured = true,
            }
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
            (size - Vec2::new(METER_WIDTH, 400.0)).max(Vec2::ZERO),
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
    lobby: Option<Res<super::multiplayer::lobby::Lobby>>,
    selection: Res<Selection>,
    account: Option<Res<super::talents::Account>>,
    mut cache: Local<Option<(Phase, bool, bool, u32)>>,
) {
    let Some(font) = font else {
        return;
    };
    let multiplayer = lobby.as_ref().is_some_and(|l| l.active);
    let points = account
        .as_ref()
        .map_or(0, |a| selection.class.map_or(0, |c| a.available(c)));
    let key = (*phase.get(), multiplayer, selection.in_hub, points);
    if *cache == Some(key) {
        return;
    }
    *cache = Some(key);
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
                left: Val::Px(0.0),
                bottom: Val::Px(0.0),
                padding: UiRect::all(Val::Px(6.0)),
                column_gap: Val::Px(6.0),
                flex_wrap: FlexWrap::Wrap,
                row_gap: Val::Px(6.0),
                max_width: Val::Percent(100.0),
                ..default()
            },
        ))
        .with_children(|ui| {
            skin::backdrop(ui);
            button(ui, &font.0, Click::Character, "Character", 105.0);
            button(ui, &font.0, Click::Meter, "Damage", 90.0);
            button(ui, &font.0, Click::Bags, "Bags", 80.0);
            skin::button(
                ui,
                &font.0,
                format!("Talents ({points})"),
                super::talent_ui::Action::Toggle,
                115.0,
            );
            if selection.in_hub {
                skin::button(
                    ui,
                    &font.0,
                    "Expeditions",
                    super::characters::Click::Expeditions,
                    110.0,
                );
                skin::button(
                    ui,
                    &font.0,
                    "Runes [R]",
                    super::characters::Click::Runes,
                    100.0,
                );
            }
            skin::button(
                ui,
                &font.0,
                "Menu [Esc]",
                super::characters::Click::Options,
                110.0,
            );
            if selection.in_hub {
                skin::button(
                    ui,
                    &font.0,
                    "Log out",
                    super::characters::Click::Logout,
                    115.0,
                );
            }
            #[cfg(not(target_arch = "wasm32"))]
            if multiplayer {
                skin::button(
                    ui,
                    &font.0,
                    "Join a game",
                    super::multiplayer::lobby::Action::Toggle,
                    180.0,
                );
            }
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
            Option<&MeterRows>,
            Option<&meter_debug::DebugRows>,
        ),
        Or<(With<MeterRows>, With<meter_panel::DetailsRows>, With<meter_debug::DebugRows>)>,
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
        for (node, cursor, mut scroll, meter, debug) in &mut rows {
            if view.debug_open && debug.is_none() {
                continue;
            }
            if !cursor.cursor_over() {
                continue;
            }
            selection.pointer_captured = true;
            scroll.0.y = (scroll.0.y - amount).clamp(
                0.0,
                ((node.content_size.y - node.size.y) * node.inverse_scale_factor).max(0.0),
            );
            if meter.is_some() {
                view.meter_scroll = scroll.0.y;
            } else if debug.is_some() {
                view.debug_scroll = scroll.0.y;
            } else {
                view.details_scroll = scroll.0.y;
            }
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
        window.resolution.set(1280.0, 1080.0);
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
        assert!(app.world().resource::<Selection>().pointer_captured);
        assert!(!app.world().resource::<Selection>().ui_captured);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.update();
        assert!(!app.world().resource::<Selection>().pointer_captured);
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
