//! Bag grid and explicit destination slots (rings/trinkets may occupy either slot).
use super::super::items::{Inventory, Item, SLOTS};
use super::*;
use bevy::ui::RelativeCursorPosition;
mod gestures;
mod grid;
#[derive(Component, Clone, Copy)]
pub(super) enum DropTarget {
    Equipment(&'static str),
    Bag(Option<u64>),
    BagBackground,
}
#[derive(Component, Clone, Copy)]
enum Action {
    Select(u64),
    Equip(u64, &'static str),
    Unequip(&'static str),
    Page(i32),
}
#[derive(Component)]
struct ItemTip(u64);
#[derive(Component)]
struct Tooltip;
const BAG_COLUMNS: usize = 14;
const BAG_ROWS: usize = 8;
const BAG_PAGE_SIZE: usize = BAG_COLUMNS * BAG_ROWS;
const BAG_GRID_WIDTH: f32 = 474.0;
const BAG_GRID_HEIGHT: f32 = 270.0;
const BAG_WIDTH: f32 = 498.0;
const BAG_HEIGHT: f32 = 340.0;
#[derive(Component)]
struct BagRoot;
pub(super) fn plugin(app: &mut App) {
    gestures::plugin(app);
    app.add_systems(
        Update,
        input
            .after(gestures::input)
            .after(super::input)
            .before(super::sheet::render),
    )
    .add_systems(Update, render.after(input).after(super::sheet::render))
    .add_systems(Update, tooltip.after(render));
}
fn input(
    gesture: Option<Res<gestures::DragState>>,
    mut inventory: Option<ResMut<Inventory>>,
    mut view: ResMut<View>,
    selection: Res<Selection>,
    mut run: ResMut<Run>,
    clicks: Query<(&Interaction, &Action), Changed<Interaction>>,
) {
    if (!selection.sheet_open && !selection.bags_open)
        || gesture.as_ref().is_some_and(|state| state.consumed_click())
    {
        return;
    }
    let Some(inventory) = inventory.as_mut() else {
        return;
    };
    for (interaction, action) in &clicks {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let result = match *action {
            Action::Select(id) => {
                view.selected_item = Some(id);
                Ok(())
            }
            Action::Equip(id, slot) => inventory.equip(id, slot),
            Action::Unequip(slot) => inventory.unequip(slot),
            Action::Page(delta) => {
                let last = inventory.bag().count().max(1).div_ceil(BAG_PAGE_SIZE) - 1;
                view.bag_page = view
                    .bag_page
                    .min(last)
                    .saturating_add_signed(delta as isize)
                    .min(last);
                Ok(())
            }
        };
        inventory.notice = match result {
            Ok(()) => String::new(),
            Err(e) => e,
        };
        inventory.apply(&mut run);
    }
}
pub(super) fn icon(ui: &mut ChildSpawnerCommands, item: &Item, size: f32) {
    ui.spawn((
        ImageNode::default(),
        hud::UiArt::item(&item.icon),
        Node {
            width: Val::Px(size),
            height: Val::Px(size),
            ..default()
        },
    ));
}
pub(super) fn equipped(ui: &mut ChildSpawnerCommands, item: &Item, slot: &'static str) {
    // Absolute bounds keep the original slot art and its frame underneath the item.
    ui.spawn((
        Button,
        Action::Select(item.id),
        ItemTip(item.id),
        DropTarget::Equipment(slot),
        RelativeCursorPosition::default(),
        BorderColor::all(item.rarity.color()),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(1.0),
            top: Val::Px(1.0),
            width: Val::Px(36.0),
            height: Val::Px(36.0),
            border: UiRect::all(Val::Px(2.0)),
            ..default()
        },
    ))
    .with_children(|b| icon(b, item, 32.0));
}
fn action(ui: &mut ChildSpawnerCommands, font: &Handle<Font>, label: &str, click: Action) {
    skin::button(
        ui,
        font,
        label,
        click,
        (label.len() as f32 * 6.5 + 16.0).max(46.0),
    );
}
pub(super) fn panel(
    ui: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    view: &View,
    inventory: Option<&Inventory>,
) {
    grid::panel(ui, font, view, inventory);
}
#[allow(clippy::too_many_arguments)]
fn render(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    selection: Res<Selection>,
    view: Res<View>,
    inventory: Option<Res<Inventory>>,
    font: Option<Res<hud::HudFont>>,
    roster: Option<Res<super::super::characters::Roster>>,
    roots: Query<Entity, With<BagRoot>>,
    mut cache: Local<String>,
) {
    let owner = roster
        .as_ref()
        .and_then(|r| r.current())
        .map_or("Adventurer", |c| c.name.as_str());
    let show = selection.bags_open && in_world(*phase.get());
    let stamp = format!(
        "{show} {owner} {} {:?} {:?}",
        view.bag_page,
        view.selected_item,
        inventory.as_ref().map(|i| (i.revision, &i.notice))
    );
    if *cache == stamp {
        return;
    }
    *cache = stamp;
    for entity in &roots {
        commands.entity(entity).despawn();
    }
    let Some(font) = font else { return };
    if !show {
        return;
    }
    commands
        .spawn((
            BagRoot,
            GlobalZIndex(625),
            BackgroundColor(Color::srgba(0.012, 0.012, 0.015, 0.98)),
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(18.0),
                bottom: Val::Px(120.0),
                width: Val::Px(BAG_WIDTH),
                height: Val::Px(BAG_HEIGHT),
                ..default()
            },
        ))
        .with_children(|root| {
            skin::frame(root, BAG_WIDTH, BAG_HEIGHT);
            skin::art(
                root,
                "Interface\\Buttons\\Button-Backpack-Up",
                skin::rect(11.0, 6.0, 18.0, 18.0),
                [0.0, 1.0, 0.0, 1.0],
            );
            root.spawn((
                text(&font.0, format!("{owner}'s Inventory"), 13.0, GOLD),
                skin::rect(34.0, 6.0, 410.0, 20.0),
            ));
            skin::close(root, Click::CloseBags, BAG_WIDTH - 34.0, 1.0);
            root.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..skin::rect(12.0, 30.0, BAG_GRID_WIDTH, 302.0)
            })
            .with_children(|body| panel(body, &font.0, &view, inventory.as_deref()));
        });
}
fn tooltip(
    mut commands: Commands,
    inventory: Option<Res<Inventory>>,
    selection: Res<Selection>,
    font: Option<Res<hud::HudFont>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    scale: Option<Res<UiScale>>,
    items: Query<(&Interaction, &ItemTip)>,
    roots: Query<Entity, With<Tooltip>>,
    mut cache: Local<Option<(u64, u64, bool)>>,
    gesture: Res<gestures::DragState>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
) {
    let id = if (selection.sheet_open || selection.bags_open) && !gesture.dragging() {
        items
            .iter()
            .find(|(i, _)| **i == Interaction::Hovered)
            .map(|(_, tip)| tip.0)
    } else {
        None
    };
    let compare = keys
        .as_ref()
        .is_some_and(|keys| keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]));
    let key = id.map(|id| (id, inventory.as_ref().map_or(0, |i| i.revision), compare));
    if key == *cache {
        return;
    }
    *cache = key;
    for e in &roots {
        commands.entity(e).despawn();
    }
    let (Some(item), Some(font)) = (
        inventory
            .as_ref()
            .and_then(|i| id.and_then(|id| i.item(id))),
        font,
    ) else {
        return;
    };
    let (viewport, cursor) = windows.single().map_or(
        (Vec2::new(1365.0, 768.0), Vec2::new(800.0, 400.0)),
        |window| {
            let scale = scale.as_deref().map_or(1.0, |s| s.0).max(0.01);
            (
                Vec2::new(window.width(), window.height()) / scale,
                window.cursor_position().unwrap_or(Vec2::ZERO) / scale,
            )
        },
    );
    let mut lines = item.tooltip_lines(inventory.as_ref().unwrap());
    if compare {
        lines.extend(item.comparison_lines(inventory.as_ref().unwrap()));
    }
    let estimated_height = lines
        .iter()
        .map(|(line, _)| (line.chars().count().max(1).div_ceil(34)) as f32 * 17.0)
        .sum::<f32>()
        + 24.0;
    let x = if cursor.x > viewport.x * 0.5 {
        cursor.x - 320.0
    } else {
        cursor.x + 18.0
    };
    let y = (cursor.y + 14.0).min((viewport.y - estimated_height - 8.0).max(8.0));
    commands
        .spawn((
            Tooltip,
            GlobalZIndex(650),
            BackgroundColor(Color::srgb(0.02, 0.025, 0.04)),
            BorderColor::all(Color::srgb(0.45, 0.45, 0.45)),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(x.clamp(8.0, (viewport.x - 308.0).max(8.0))),
                top: Val::Px(y.max(8.0)),
                width: Val::Px(300.0),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
        ))
        .with_children(|ui| {
            ui.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(1.0),
                ..default()
            })
            .with_children(|body| {
                for (line, color) in lines {
                    body.spawn(text(&font.0, line, 14.0, color));
                }
            });
        });
}

#[cfg(test)]
mod tests {
    use super::super::super::items::{Config, Rarity};
    use super::*;
    #[derive(Component)]
    struct TestRoot;
    #[test]
    fn bag_window_is_right_anchored_and_does_not_require_the_character_sheet() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(State::new(Phase::Hub))
            .insert_resource(Selection {
                bags_open: true,
                ..default()
            })
            .init_resource::<View>()
            .insert_resource(Inventory::fixture(Vec::new()))
            .insert_resource(hud::HudFont(Handle::default()))
            .add_systems(Update, render);
        app.update();
        let world = app.world_mut();
        let node = world
            .query_filtered::<&Node, With<BagRoot>>()
            .single(world)
            .unwrap();
        assert_eq!(node.position_type, PositionType::Absolute);
        assert_eq!(node.right, Val::Px(18.0));
        assert_eq!(node.width, Val::Px(BAG_WIDTH));
        assert_eq!(
            world
                .query::<&DropTarget>()
                .iter(world)
                .filter(|target| matches!(target, DropTarget::Bag(_)))
                .count(),
            112
        );
        let mut selection = world.resource_mut::<Selection>();
        selection.bags_open = false;
        selection.sheet_open = true;
        app.update();
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<BagRoot>>()
                .iter(app.world())
                .count(),
            0
        );
    }
    fn draw(
        mut commands: Commands,
        view: Res<View>,
        inventory: Res<Inventory>,
        roots: Query<Entity, With<TestRoot>>,
    ) {
        for e in &roots {
            commands.entity(e).despawn();
        }
        commands
            .spawn((TestRoot, Node::default()))
            .with_children(|ui| panel(ui, &Handle::default(), &view, Some(&inventory)));
    }
    #[test]
    fn populated_bag_builds_and_buttons_equip_and_unequip_without_losing_items() {
        let mut rng = super::super::super::Rng(42);
        let item = Config::builtin().generate(10, &mut rng).unwrap();
        let slot = SLOTS.iter().find(|(_, kind)| *kind == item.slot).unwrap().0;
        assert_ne!(item.rarity, Rarity::Legendary);
        let mut app = App::new();
        app.insert_resource(Inventory::fixture(vec![item]))
            .init_resource::<View>()
            .insert_resource(Selection {
                sheet_open: true,
                ..default()
            })
            .init_resource::<Run>()
            .add_systems(Update, (input, draw).chain());
        app.world_mut()
            .spawn((Interaction::Pressed, Action::Select(1)));
        app.update();
        assert_eq!(app.world().resource::<View>().selected_item, Some(1));
        assert_eq!(
            app.world_mut()
                .query::<&DropTarget>()
                .iter(app.world())
                .filter(|target| matches!(target, DropTarget::Bag(_)))
                .count(),
            BAG_PAGE_SIZE
        );
        let click = app
            .world_mut()
            .query::<(Entity, &Action)>()
            .iter(app.world())
            .find(|(_, a)| matches!(a,Action::Equip(1,s) if *s==slot))
            .unwrap()
            .0;
        *app.world_mut().get_mut::<Interaction>(click).unwrap() = Interaction::Pressed;
        app.update();
        assert_eq!(
            app.world()
                .resource::<Inventory>()
                .equipped(slot)
                .unwrap()
                .id,
            1
        );
        assert_eq!(app.world().resource::<Inventory>().bag().count(), 0);
        let click = app
            .world_mut()
            .query::<(Entity, &Action)>()
            .iter(app.world())
            .find(|(_, a)| matches!(a,Action::Unequip(s) if *s==slot))
            .unwrap()
            .0;
        *app.world_mut().get_mut::<Interaction>(click).unwrap() = Interaction::Pressed;
        app.update();
        assert!(app.world().resource::<Inventory>().equipped(slot).is_none());
        assert_eq!(app.world().resource::<Inventory>().bag().count(), 1);
    }
}
