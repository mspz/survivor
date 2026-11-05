//! Mouse gestures keep only item identity, so rebuilding the sheet cannot lose a held item.
use super::*;
use bevy::{ui::FocusPolicy, window::PrimaryWindow};
#[derive(Clone, Copy)]
struct Press {
    id: u64,
    start: Vec2,
    position: Vec2,
    dragging: bool,
}
#[derive(Resource, Default)]
pub(super) struct DragState {
    press: Option<Press>,
    suppress_click: bool,
}
impl DragState {
    pub(super) fn consumed_click(&self) -> bool {
        self.suppress_click
    }
    pub(super) fn dragging(&self) -> bool {
        self.press.is_some_and(|p| p.dragging)
    }
}
#[derive(Component)]
struct Ghost(u64);
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<DragState>()
        .add_systems(
            Update,
            input
                .after(super::super::input)
                .after(super::super::super::talent_ui::input)
                .before(benilla_world::schedule::WorldStage::Input),
        )
        .add_systems(Update, ghost.after(input));
}
#[allow(clippy::too_many_arguments)]
pub(super) fn input(
    mut state: ResMut<DragState>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    scale: Option<Res<UiScale>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut selection: ResMut<Selection>,
    mut view: ResMut<View>,
    mut inventory: Option<ResMut<Inventory>>,
    mut run: ResMut<Run>,
    sources: Query<(&Interaction, &ItemTip, &RelativeCursorPosition)>,
    targets: Query<(&Interaction, &DropTarget, &RelativeCursorPosition)>,
) {
    state.suppress_click = false;
    let cursor = windows
        .single()
        .ok()
        .filter(|w| w.focused)
        .and_then(Window::cursor_position)
        .map(|p| p / scale.as_deref().map_or(1.0, |s| s.0).max(0.01));
    let (Some(cursor), Some(inventory)) = (cursor, inventory.as_mut()) else {
        state.press = None;
        return;
    };
    if !selection.sheet_open && !selection.bags_open {
        state.press = None;
        return;
    }
    let hovered = sources
        .iter()
        .find(|(interaction, _, position)| {
            **interaction != Interaction::None && position.cursor_over()
        })
        .map(|(_, item, _)| item.0);
    if mouse.just_pressed(MouseButton::Left)
        && keys
            .as_ref()
            .is_some_and(|keys| keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]))
    {
        if let Some(id) = hovered {
            state.press = None;
            state.suppress_click = true;
            selection.ui_captured = true;
            match inventory.delete(id) {
                Ok(()) => {
                    inventory.notice.clear();
                    if view.selected_item == Some(id) {
                        view.selected_item = None;
                    }
                    inventory.apply(&mut run);
                }
                Err(error) => inventory.notice = error,
            }
            return;
        }
    }
    if mouse.just_pressed(MouseButton::Right) {
        if state.press.take().is_none() {
            if let Some(id) = hovered {
                let result = inventory.quick_equip(id);
                inventory.notice = result.err().unwrap_or_default();
                view.selected_item = Some(id);
                inventory.apply(&mut run);
            }
        }
        return;
    }
    if mouse.just_pressed(MouseButton::Left) {
        state.press = hovered.map(|id| Press {
            id,
            start: cursor,
            position: cursor,
            dragging: false,
        });
    }
    let Some(mut press) = state.press else { return };
    if inventory.item(press.id).is_none() {
        state.press = None;
        return;
    }
    press.position = cursor;
    press.dragging |= press.start.distance(cursor) >= 4.0;
    selection.ui_captured = true;
    if mouse.just_released(MouseButton::Left) {
        state.press = None;
        if !press.dragging {
            return;
        }
        let target = targets
            .iter()
            .filter(|(interaction, _, position)| {
                **interaction != Interaction::None && position.cursor_over()
            })
            .min_by_key(|(_, target, _)| match target {
                DropTarget::Equipment(_) => 0,
                DropTarget::Bag(_) => 1,
                DropTarget::BagBackground => 2,
            })
            .map(|(_, target, _)| *target);
        let result = match target {
            Some(DropTarget::Equipment(slot)) => inventory.drop_on_equipment(press.id, slot),
            Some(DropTarget::Bag(target)) => inventory.drop_in_bag(press.id, target),
            Some(DropTarget::BagBackground) => inventory.drop_in_bag(press.id, None),
            None => return,
        };
        inventory.notice = result.err().unwrap_or_default();
        inventory.apply(&mut run);
    } else if mouse.pressed(MouseButton::Left) {
        state.press = Some(press);
    } else {
        state.press = None;
    }
}
fn ghost(
    mut commands: Commands,
    state: Res<DragState>,
    inventory: Option<Res<Inventory>>,
    mut roots: Query<(Entity, &Ghost, &mut Node)>,
) {
    let held = state.press.filter(|p| p.dragging).and_then(|p| {
        inventory
            .as_ref()
            .and_then(|i| i.item(p.id))
            .map(|item| (p, item))
    });
    let mut existing = false;
    for (entity, ghost, mut node) in &mut roots {
        if let Some((press, _)) = held.filter(|(p, _)| p.id == ghost.0) {
            node.left = Val::Px(press.position.x + 8.0);
            node.top = Val::Px(press.position.y + 8.0);
            existing = true;
        } else {
            commands.entity(entity).despawn();
        }
    }
    if !existing {
        if let Some((press, item)) = held {
            commands
                .spawn((
                    Ghost(item.id),
                    GlobalZIndex(900),
                    FocusPolicy::Pass,
                    BackgroundColor(Color::BLACK),
                    BorderColor::all(item.rarity.color()),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(press.position.x + 8.0),
                        top: Val::Px(press.position.y + 8.0),
                        width: Val::Px(44.0),
                        height: Val::Px(44.0),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                ))
                .with_children(|ui| {
                    let usable = inventory.as_ref().is_none_or(|i| i.equippable(item));
                    icon(ui, item, 40.0, usable);
                });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::super::items::{Affix, AffixKind, Effect, Rarity, SlotKind};
    use super::*;
    fn app() -> (App, Entity, Entity, Entity) {
        let item = Item {
            affix_rules: 0,
            legendary: None,
            base_damage: None,
            base_armour: None,
            appearance: None,
            equipment_type: crate::survivors::items::EquipmentType::Legacy,
            base_block: 0.0,
            loot_class: None,
            id: 0,
            base: "ring".into(),
            name: "Test ring".into(),
            slot: SlotKind::Ring,
            icon: "Interface\\Icons\\INV_Jewelry_Ring_01".into(),
            rarity: Rarity::Normal,
            level: 1,
            affixes: vec![Affix {
                id: "life".into(),
                name: "Healthy".into(),
                group: "life".into(),
                kind: AffixKind::Prefix,
                effect: Effect::Life,
                value: 10.0,
                tags: vec![],
                proc: None,
            }],
        };
        let mut app = App::new();
        app.init_resource::<DragState>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<View>()
            .insert_resource(Selection {
                bags_open: true,
                ..default()
            })
            .insert_resource(Inventory::fixture(vec![item.clone(), item]))
            .init_resource::<Run>()
            .add_systems(Update, (input, ghost).chain());
        let mut window = Window::default();
        window.focused = true;
        window.set_cursor_position(Some(Vec2::new(100.0, 100.0)));
        let window = app.world_mut().spawn((PrimaryWindow, window)).id();
        let source = app
            .world_mut()
            .spawn((
                Interaction::Hovered,
                ItemTip(1),
                RelativeCursorPosition {
                    cursor_over: true,
                    normalized: Some(Vec2::ZERO),
                },
            ))
            .id();
        let target = app
            .world_mut()
            .spawn((
                Interaction::None,
                DropTarget::Equipment("Ring 2"),
                RelativeCursorPosition::default(),
            ))
            .id();
        (app, window, source, target)
    }
    fn start(app: &mut App, window: Entity) {
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear_just_pressed(MouseButton::Left);
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .set_cursor_position(Some(Vec2::new(140.0, 100.0)));
        app.update();
        assert!(app.world().resource::<DragState>().dragging());
        assert_eq!(
            app.world_mut().query::<&Ghost>().iter(app.world()).count(),
            1
        );
    }
    #[test]
    fn alt_click_deletes_bag_and_equipped_items_without_dragging_or_reselecting() {
        for alt in [KeyCode::AltLeft, KeyCode::AltRight] {
            for equipped in [false, true] {
                let (mut app, _, source, _) = app();
                let base_hp = app.world().resource::<Run>().stats().max_hp;
                if equipped {
                    app.world_mut()
                        .resource_scope(|world, mut inventory: Mut<Inventory>| {
                            inventory.equip(1, "Ring 1").unwrap();
                            inventory.apply(&mut world.resource_mut::<Run>());
                        });
                    assert!(app.world().resource::<Run>().stats().max_hp > base_hp);
                }
                app.world_mut().resource_mut::<View>().selected_item = Some(1);
                app.world_mut().entity_mut(source).insert(Action::Select(1));
                *app.world_mut().get_mut::<Interaction>(source).unwrap() = Interaction::Pressed;
                app.add_systems(Update, super::super::input.after(input).before(ghost));
                app.world_mut()
                    .resource_mut::<ButtonInput<KeyCode>>()
                    .press(alt);
                app.world_mut()
                    .resource_mut::<ButtonInput<MouseButton>>()
                    .press(MouseButton::Left);
                app.update();
                let inventory = app.world().resource::<Inventory>();
                assert!(inventory.item(1).is_none());
                assert!(inventory.item(2).is_some());
                assert!(inventory.equipped("Ring 1").is_none());
                assert_eq!(app.world().resource::<View>().selected_item, None);
                assert!(!app.world().resource::<DragState>().dragging());
                assert_eq!(
                    app.world_mut().query::<&Ghost>().iter(app.world()).count(),
                    0
                );
                assert_eq!(app.world().resource::<Run>().stats().max_hp, base_hp);
            }
        }
    }
    #[test]
    fn drag_release_equips_to_explicit_slot_and_clears_cursor_ghost() {
        let (mut app, window, source, target) = app();
        start(&mut app, window);
        *app.world_mut().get_mut::<Interaction>(source).unwrap() = Interaction::None;
        app.world_mut()
            .get_mut::<RelativeCursorPosition>(source)
            .unwrap()
            .cursor_over = false;
        *app.world_mut().get_mut::<Interaction>(target).unwrap() = Interaction::Hovered;
        app.world_mut()
            .get_mut::<RelativeCursorPosition>(target)
            .unwrap()
            .cursor_over = true;
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.update();
        assert_eq!(
            app.world()
                .resource::<Inventory>()
                .equipped("Ring 2")
                .unwrap()
                .id,
            1
        );
        assert!(app
            .world()
            .resource::<Inventory>()
            .equipped("Ring 1")
            .is_none());
        assert!(!app.world().resource::<DragState>().dragging());
        assert_eq!(
            app.world_mut().query::<&Ghost>().iter(app.world()).count(),
            0
        );
    }
    #[test]
    fn right_click_toggles_equipment_on_first_slot() {
        let (mut app, _, _, _) = app();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);
        app.update();
        assert_eq!(
            app.world()
                .resource::<Inventory>()
                .equipped("Ring 1")
                .unwrap()
                .id,
            1
        );
        assert!(app
            .world()
            .resource::<Inventory>()
            .equipped("Ring 2")
            .is_none());
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);
        app.update();
        assert!(!app.world().resource::<Inventory>().worn(1));
        assert_eq!(app.world().resource::<Inventory>().all().len(), 2);
    }
    #[test]
    fn invalid_drops_and_focus_loss_cancel_without_losing_items() {
        let (mut app, window, source, target) = app();
        start(&mut app, window);
        *app.world_mut().get_mut::<Interaction>(source).unwrap() = Interaction::None;
        app.world_mut()
            .get_mut::<RelativeCursorPosition>(source)
            .unwrap()
            .cursor_over = false;
        *app.world_mut().get_mut::<DropTarget>(target).unwrap() = DropTarget::Equipment("Head");
        *app.world_mut().get_mut::<Interaction>(target).unwrap() = Interaction::Hovered;
        app.world_mut()
            .get_mut::<RelativeCursorPosition>(target)
            .unwrap()
            .cursor_over = true;
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.update();
        assert_eq!(app.world().resource::<Inventory>().bag().count(), 2);
        assert!(!app.world().resource::<Inventory>().notice.is_empty());
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .reset_all();
        *app.world_mut().get_mut::<Interaction>(source).unwrap() = Interaction::Hovered;
        app.world_mut()
            .get_mut::<RelativeCursorPosition>(source)
            .unwrap()
            .cursor_over = true;
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .set_cursor_position(Some(Vec2::new(100.0, 100.0)));
        start(&mut app, window);
        app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
        app.update();
        assert!(!app.world().resource::<DragState>().dragging());
        assert_eq!(app.world().resource::<Inventory>().bag().count(), 2);
    }
}
