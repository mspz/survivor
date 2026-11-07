//! Compact Bagnon-style grid. The bag owns its window, independently of the paper doll.
use super::*;
pub(super) fn panel(
    ui: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    view: &View,
    inventory: Option<&Inventory>,
) {
    let Some(inventory) = inventory else { return };
    let cells = inventory.bag_cells();
    let pages = cells.len().max(1).div_ceil(BAG_PAGE_SIZE);
    let page = view.bag_page.min(pages - 1);
    ui.spawn((
        DropTarget::BagBackground,
        RelativeCursorPosition::default(),
        Interaction::default(),
        Node {
            width: Val::Px(BAG_GRID_WIDTH),
            height: Val::Px(BAG_GRID_HEIGHT),
            min_height: Val::Px(BAG_GRID_HEIGHT),
            flex_shrink: 0.0,
            flex_wrap: FlexWrap::Wrap,
            align_content: AlignContent::Start,
            column_gap: Val::Px(2.0),
            row_gap: Val::Px(2.0),
            ..default()
        },
    ))
    .with_children(|grid| {
        for index in 0..BAG_PAGE_SIZE {
            let position = page * BAG_PAGE_SIZE + index;
            let item = cells.get(position).copied().flatten();
            let mut cell = grid.spawn((
                Button,
                DropTarget::Bag(position),
                RelativeCursorPosition::default(),
                BackgroundColor(Color::srgb(0.015, 0.015, 0.015)),
                BorderColor::all(
                    item.map_or(Color::srgb(0.18, 0.18, 0.18), |item| item.rarity.color()),
                ),
                Node {
                    width: Val::Px(32.0),
                    height: Val::Px(32.0),
                    flex_shrink: 0.0,
                    border: UiRect::all(Val::Px(1.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
            ));
            if let Some(item) = item {
                cell.insert((Action::Select(item.id), ItemTip(item.id)))
                    .with_children(|ui| icon(ui, item, 28.0, inventory.equippable(item)));
            } else {
                cell.with_children(|ui| {
                    skin::art(
                        ui,
                        "Interface\\PaperDoll\\UI-Backpack-EmptySlot",
                        skin::rect(0.0, 0.0, 30.0, 30.0),
                        [0.0, 1.0, 0.0, 1.0],
                    )
                });
            }
        }
    });
    ui.spawn(Node {
        width: Val::Percent(100.0),
        height: Val::Px(26.0),
        align_items: AlignItems::Center,
        column_gap: Val::Px(6.0),
        ..default()
    })
    .with_children(|footer| {
        let count = cells.iter().flatten().count();
        footer.spawn(text(font, format!("{count} items"), 11.0, GREY));
        if let Some(item) = view.selected_item.and_then(|id| inventory.item(id)) {
            // Two clicks: the rolled item cannot be recovered.
            let label = if view.delete_armed == Some(item.id) {
                "Confirmer"
            } else {
                "Vendre"
            };
            action(footer, font, label, Action::Delete(item.id));
            for &(slot, kind) in &SLOTS {
                if !inventory.can_equip(item, kind) {
                    continue;
                }
                if inventory
                    .equipped(slot)
                    .is_some_and(|worn| worn.id == item.id)
                {
                    action(
                        footer,
                        font,
                        &format!("Unequip {slot}"),
                        Action::Unequip(slot),
                    );
                } else {
                    action(
                        footer,
                        font,
                        &format!("Equip {slot}"),
                        Action::Equip(item.id, slot),
                    );
                }
            }
        }
        footer.spawn(Node {
            flex_grow: 1.0,
            ..default()
        });
        if pages > 1 {
            action(footer, font, "<", Action::Page(-1));
            footer.spawn(text(font, format!("{}/{}", page + 1, pages), 11.0, GOLD));
            action(footer, font, ">", Action::Page(1));
        }
    });
    let (gold, silver, copper) = super::super::super::currency::split(inventory.money());
    ui.spawn(Node {
        width: Val::Percent(100.0),
        height: Val::Px(26.0),
        flex_shrink: 0.0,
        justify_content: JustifyContent::FlexEnd,
        align_items: AlignItems::Center,
        column_gap: Val::Px(3.0),
        ..default()
    })
    .with_children(|money| {
        super::auto_salvage::button(money, font);
        money.spawn(Node {
            flex_grow: 1.0,
            ..default()
        });
        // MoneyFrame.xml: gold, silver and copper occupy successive quarters of this atlas.
        for (amount, coords) in [
            (gold, [0.0, 0.25, 0.0, 1.0]),
            (silver, [0.25, 0.5, 0.0, 1.0]),
            (copper, [0.5, 0.75, 0.0, 1.0]),
        ] {
            money.spawn(text(font, amount.to_string(), 12.0, Color::WHITE));
            money.spawn((
                ImageNode::default(),
                hud::UiArt::crop("Interface\\MoneyFrame\\UI-MoneyIcons", coords),
                Node {
                    width: Val::Px(16.0),
                    height: Val::Px(16.0),
                    ..default()
                },
            ));
        }
    });
    if !inventory.notice.is_empty() {
        ui.spawn(text(
            font,
            &inventory.notice,
            11.0,
            Color::srgb(1.0, 0.3, 0.2),
        ));
    }
}
