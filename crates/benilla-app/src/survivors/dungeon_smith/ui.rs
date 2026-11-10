use super::super::combat_ui::skin;
use super::*;
use bevy::ui::RelativeCursorPosition;

#[derive(Component)]
pub(super) struct Root;
#[derive(Component)]
pub(super) struct Scroll;
#[derive(Component, Clone, Copy)]
pub(super) enum ExitRoot {
    Footer,
    Confirmation,
}

pub(super) fn money(copper: u64) -> String {
    let (gold, silver, copper) = super::super::currency::split(copper);
    if silver == 0 && copper == 0 {
        format!("{gold} gold")
    } else {
        format!("{gold}g {silver}s {copper}c")
    }
}

pub(super) fn spawn(mut commands: Commands) {
    commands.spawn((
        Root,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(10.0),
            right: Val::Percent(10.0),
            top: Val::Percent(8.0),
            bottom: Val::Percent(15.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(22.0)),
            row_gap: Val::Px(12.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.025, 0.02, 0.015, 0.96)),
        GlobalZIndex(480),
        Visibility::Hidden,
    ));
    commands.spawn((
        ExitRoot::Footer,
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(24.0),
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            ..default()
        },
        GlobalZIndex(490),
        Visibility::Hidden,
    ));
    commands.spawn((
        ExitRoot::Confirmation,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            top: Val::Px(0.0),
            bottom: Val::Px(0.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)),
        GlobalZIndex(700),
        Visibility::Hidden,
    ));
}

pub(super) fn render(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    run: Res<Run>,
    view: Res<View>,
    selection: Res<super::super::Selection>,
    inventory: Res<Inventory>,
    font: Option<Res<hud::HudFont>>,
    mut roots: Query<(Entity, &mut Visibility, &mut Node), With<Root>>,
    mut shown: Local<Option<(bool, u64, u64, Option<u64>, Option<u64>, u32, String)>>,
) {
    let Ok((root, mut visibility, mut node)) = roots.single_mut() else {
        return;
    };
    let Some(font) = font else {
        return;
    };
    let show =
        *phase.get() == Phase::GameOver && available(&run) && view.open && !view.confirm_leave;
    // Dock opposite the source panel without rebuilding the slot during a drag.
    node.left = if selection.sheet_open {
        Val::Auto
    } else {
        Val::Px(18.0)
    };
    node.right = if selection.sheet_open {
        Val::Px(18.0)
    } else {
        Val::Auto
    };
    node.width = Val::Px(430.0);
    node.max_width = Val::Percent(42.0);
    node.top = Val::Px(30.0);
    node.bottom = Val::Px(120.0);
    let key = (
        show,
        inventory.revision,
        inventory.money(),
        inventory.money_owner(),
        view.selected_item(),
        if show { dungeon_level(&run) } else { 0 },
        view.notice.clone(),
    );
    if shown.as_ref() == Some(&key) {
        return;
    }
    *shown = Some(key);
    *visibility = if show {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    commands.entity(root).despawn_related::<Children>();
    if !show {
        return;
    }
    let f = &font.0;
    commands.entity(root).with_children(|panel| {
        skin::backdrop(panel);
        panel.spawn(hud::centered_bundle(f, NAME, 25.0, hud::GOLD_TONE));
        panel.spawn(hud::centered_bundle(f, format!("Dungeon cleared · ilvl {}  |  Your gold: {}", dungeon_level(&run), money(inventory.money())), 16.0, Color::WHITE));
        panel.spawn(hud::centered_bundle(f, "Drag equipment here or right-click it in your bags or character sheet (C).\nFailed attempts consume gold only.", 13.0, Color::srgb(0.8, 0.75, 0.65)));
        panel.spawn(Node { column_gap: Val::Px(8.0), justify_content: JustifyContent::Center, ..default() }).with_children(|row| {
            skin::button(row, f, "Bags (B)", super::super::combat_ui::Click::Bags, 150.0);
            skin::button(row, f, "Character (C)", super::super::combat_ui::Click::Character, 150.0);
        });
        let mut upgrade_slot = panel.spawn((Button, super::super::combat_ui::bags::DropTarget::DungeonUpgrade,
            RelativeCursorPosition::default(), BorderColor::all(hud::GOLD_TONE),
            BackgroundColor(Color::srgb(0.06, 0.05, 0.035)),
            Node { width: Val::Px(64.0), height: Val::Px(64.0), min_height: Val::Px(64.0),
                border: UiRect::all(Val::Px(2.0)), align_self: AlignSelf::Center,
                align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() }));
        let selected_item = view.selected_item().and_then(|id| inventory.item(id));
        if let Some(item) = selected_item {
            upgrade_slot.insert(super::super::combat_ui::bags::ItemHoverTip(item.id));
        }
        upgrade_slot.with_children(|slot| {
                skin::art(slot, "Interface\\PaperDoll\\UI-Backpack-EmptySlot",
                    skin::rect(2.0, 2.0, 60.0, 60.0), [0.0, 1.0, 0.0, 1.0]);
                if let Some(item) = selected_item {
                    super::super::combat_ui::bags::icon(slot, item, 56.0, true);
                }
            });
        if view.selected_item().is_some() { skin::button(panel, f, "Remove item", Action::ClearSlot, 180.0); }
        panel.spawn((Scroll, RelativeCursorPosition::default(), Node { width: Val::Percent(100.0), flex_grow: 1.0, min_height: Val::Px(0.0), overflow: Overflow::scroll_y(), flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), ..default() }))
            .with_children(|details| {
                        let Some(item) = view.selected_item().and_then(|id| inventory.item(id)) else {
                            details.spawn(hud::text_bundle(f, "Select an item to see the price and chances.", 15.0, Color::WHITE));
                            return;
                        };
                        details.spawn(hud::text_bundle(f, item.name.clone(), 20.0, item.rarity.color()));
                        match config::get().quote(item.level, dungeon_level(&run)) {
                            Ok(quote) => {
                                details.spawn(hud::text_bundle(f, format!("Current ilvl {} · Dungeon limit {}\n{}\nPrice per attempt: {}", item.level, dungeon_level(&run), quote.odds(), money(quote.cost)), 16.0, hud::GOLD_TONE));
                                if inventory.money() >= quote.cost {
                                    skin::button(details, f, format!("Pay {} and attempt upgrade", money(quote.cost)), Action::Upgrade, 330.0);
                                } else { details.spawn(hud::text_bundle(f, "Not enough gold for this attempt.", 14.0, Color::srgb(1.0, 0.4, 0.3))); }
                                let mut preview = item.clone();
                                if preview.raise_item_level(item.level + quote.maximum_gain).is_ok() {
                                    details.spawn(hud::text_bundle(f, format!("Preview on best result: ilvl {}", preview.level), 14.0, Color::WHITE));
                                    for (line, color) in preview.tooltip_lines(&inventory) {
                                        details.spawn(hud::text_bundle(f, line, 12.0, color));
                                    }
                                }
                            }
                            Err(reason) => { details.spawn(hud::text_bundle(f, reason, 14.0, Color::srgb(0.8, 0.75, 0.65))); }
                        }
            });
        if !view.notice.is_empty() { panel.spawn(hud::centered_bundle(f, view.notice.clone(), 16.0, hud::GOLD_TONE)); }
        skin::button(panel, f, "Back to dungeon", Action::Close, 200.0);
    });
}

pub(super) fn render_exit(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    run: Res<Run>,
    view: Res<View>,
    font: Option<Res<hud::HudFont>>,
    mut roots: Query<(Entity, &ExitRoot, &mut Visibility)>,
    mut shown: Local<Option<(bool, bool)>>,
) {
    let Some(font) = font else {
        return;
    };
    let show = *phase.get() == Phase::GameOver && available(&run);
    let key = (show, view.confirm_leave);
    if shown.as_ref() == Some(&key) {
        return;
    }
    *shown = Some(key);
    for (root, kind, mut visibility) in &mut roots {
        let visible = show && (matches!(kind, ExitRoot::Footer) || view.confirm_leave);
        *visibility = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        commands.entity(root).despawn_related::<Children>();
        if !visible {
            continue;
        }
        commands.entity(root).with_children(|ui| match kind {
            ExitRoot::Footer => {
                ui.spawn(hud::centered_bundle(&font.0, "Dungeon cleared — right-click Aldren to upgrade equipment", 14.0, hud::GOLD_TONE));
                skin::button(ui, &font.0, "Leave dungeon", Action::Leave, 220.0);
            }
            ExitRoot::Confirmation => {
                ui.spawn((Node { width: Val::Px(460.0), max_width: Val::Percent(90.0), padding: UiRect::all(Val::Px(24.0)),
                    flex_direction: FlexDirection::Column, row_gap: Val::Px(16.0), ..default() }, BackgroundColor(Color::srgb(0.035, 0.025, 0.015))))
                    .with_children(|panel| {
                        skin::backdrop(panel);
                        panel.spawn(hud::centered_bundle(&font.0, "Are you sure?", 25.0, hud::GOLD_TONE));
                        panel.spawn(hud::centered_bundle(&font.0, "You haven't spoken to Aldren yet.\nLeave without checking equipment upgrades?", 16.0, Color::WHITE));
                        skin::button(panel, &font.0, "Stay in dungeon", Action::CancelLeave, 220.0);
                        skin::button(panel, &font.0, "Yes, leave dungeon", Action::ConfirmLeave, 220.0);
                    });
            }
        });
    }
}

pub(super) fn scroll(
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    mut lists: Query<(&ComputedNode, &RelativeCursorPosition, &mut ScrollPosition), With<Scroll>>,
) {
    for event in wheel.read() {
        for (node, cursor, mut position) in &mut lists {
            if cursor.cursor_over() {
                let step = if event.unit == bevy::input::mouse::MouseScrollUnit::Line {
                    36.0
                } else {
                    1.0
                };
                let max =
                    ((node.content_size.y - node.size.y) * node.inverse_scale_factor).max(0.0);
                position.0.y = (position.0.y - event.y * step).clamp(0.0, max);
            }
        }
    }
}
