//! Existing-item affix editor, shared by the hub and training blacksmiths.
use super::*;
use crate::survivors::items::recraft as service;
#[cfg(test)]
#[path = "recraft_tests.rs"]
mod tests;

impl View {
    pub(in crate::survivors) fn recraft_active(&self) -> bool {
        self.open && self.recraft
    }

    pub(in crate::survivors) fn place_recraft_item(&mut self, id: u64, inventory: &Inventory) {
        if !self.recraft_active() {
            return;
        }
        let Some(item) = inventory.item(id) else {
            return;
        };
        match service::slots(item) {
            Ok(slots) => {
                self.source = Some(id);
                self.source_owner = inventory.money_owner();
                self.affixes = slots.map(|a| a.map(|a| a.id.clone())).to_vec();
                self.recraft_dirty.fill(false);
                self.slot = item.slot;
                self.equipment = item.equipment_type;
                self.rarity = item.rarity;
                self.ilvl = Some(item.level);
                self.notice.clear();
                self.result = None;
                self.dropdown = None;
            }
            Err(error) => self.notice = error,
        }
        self.revision = self.revision.wrapping_add(1);
    }
}

pub(super) fn tabs(ui: &mut ChildSpawnerCommands, font: &Handle<Font>, recraft: bool) {
    ui.spawn(Node {
        column_gap: Val::Px(4.0),
        ..skin::rect(0.0, -34.0, 330.0, 32.0)
    })
    .with_children(|row| {
        smith_button(
            row,
            font,
            if recraft { "Craft" } else { "[Craft]" },
            Click::Tab(false),
            160.0,
        );
        smith_button(
            row,
            font,
            if recraft { "[Recraft]" } else { "Recraft" },
            Click::Tab(true),
            160.0,
        );
    });
}

fn edits(view: &View) -> Vec<(usize, Option<String>)> {
    view.recraft_dirty
        .iter()
        .enumerate()
        .filter(|(_, dirty)| **dirty)
        .map(|(slot, _)| (slot, view.affixes[slot].clone()))
        .collect()
}

fn cost(view: &View, inventory: &Inventory) -> Result<u64, String> {
    let item = view
        .source
        .and_then(|id| inventory.item(id))
        .ok_or("Place an item in the slot.")?;
    let count = edits(view).len();
    if count == 0 {
        return Err("Select an affix to edit or add.".into());
    }
    service::price(item.level)?
        .checked_mul(count as u64)
        .filter(|price| *price <= crate::survivors::currency::MAX_BALANCE)
        .ok_or_else(|| "Recraft price too high.".into())
}

pub(super) fn click(
    view: &mut View,
    click: Click,
    inventory: &mut Inventory,
    account: &talents::Account,
    rng: &mut Rng,
) {
    match click {
        Click::ClearSource => {
            view.source = None;
            view.affixes.fill(None);
            view.recraft_dirty.fill(false);
            view.dropdown = None;
            view.notice.clear();
        }
        Click::RestoreAffix(slot) if view.slot_kind(slot).is_some() => {
            if let Some(item) = view.source.and_then(|id| inventory.item(id)) {
                if let Ok(original) = service::slots(item) {
                    view.affixes[slot] = original[slot].map(|a| a.id.clone());
                    view.recraft_dirty[slot] = false;
                    view.dropdown = None;
                }
            }
        }
        Click::Dropdown(slot) if view.ready() && view.slot_kind(slot).is_some() => {
            view.dropdown = if view.dropdown == Some(slot) {
                None
            } else {
                Some(slot)
            };
        }
        Click::SelectAffix(slot, choice) if view.ready() && view.slot_kind(slot).is_some() => {
            let choices = inventory.recraft_affixes(view.source.unwrap());
            match choice {
                None => {
                    view.affixes[slot] = None;
                    view.recraft_dirty[slot] = true;
                }
                Some(index) if view.dropdown_choices(&choices, slot).contains(&index) => {
                    view.affixes[slot] = Some(choices[index].id.clone());
                    view.recraft_dirty[slot] = true;
                }
                _ => view.notice = "Affix incompatible or already selected.".into(),
            }
            view.dropdown = None;
        }
        Click::DismissDropdown => view.dropdown = None,
        Click::Buy => {
            if account.character.is_none()
                || account.character != inventory.money_owner()
                || account.character != view.source_owner
            {
                view.notice = "The selected character changed. Place the item again.".into();
            } else if let Some(id) = view.source {
                let pending = edits(view);
                let quoted = cost(view, inventory);
                match quoted.and_then(|price| {
                    inventory
                        .recraft(id, &pending, rng)
                        .map(|item| (item, price))
                }) {
                    Ok((item, price)) => {
                        view.place_recraft_item(id, inventory);
                        view.result = Some(item);
                        let (gold, silver, copper) = crate::survivors::currency::split(price);
                        view.notice = format!(
                            "{} affix(es) recrafted. Paid: {gold}g {silver}s {copper}c.",
                            pending.len()
                        );
                    }
                    Err(error) => view.notice = error,
                }
            }
        }
        _ => {}
    }
    view.revision = view.revision.wrapping_add(1);
}

pub(super) fn render(
    commands: &mut Commands,
    view: &View,
    inventory: &Inventory,
    font: &Handle<Font>,
) {
    let item = view.source.and_then(|id| inventory.item(id));
    let choices = item.map_or_else(Vec::new, |item| inventory.recraft_affixes(item.id));
    commands.spawn((Root, GlobalZIndex(615), BackgroundColor(Color::srgb(0.025, 0.023, 0.02)),
        BorderColor::all(hud::GOLD_TONE), Node {border: UiRect::all(Val::Px(2.0)), ..skin::rect(16.0, 70.0, 800.0, 624.0)}))
        .with_children(|ui| {
            skin::backdrop(ui);
            tabs(ui, font, true);
            ui.spawn((text(font, "Blacksmith - Recraft", 20.0, hud::GOLD_TONE), skin::rect(20.0, 14.0, 600.0, 28.0)));
            ui.spawn(skin::rect(750.0, 10.0, 30.0, 28.0)).with_children(|row| smith_button(row, font, "X", Click::Close, 30.0));
            let mut slot = ui.spawn((Button, super::super::combat_ui::bags::DropTarget::Recraft,
                RelativeCursorPosition::default(), BorderColor::all(item.map_or(hud::GOLD_TONE, |i| i.rarity.color())),
                BackgroundColor(Color::srgb(0.06, 0.05, 0.035)), Node { border: UiRect::all(Val::Px(2.0)), padding: UiRect::all(Val::Px(8.0)),
                    column_gap: Val::Px(10.0), align_items: AlignItems::Center, ..skin::rect(20.0, 65.0, 325.0, 80.0)}));
            if let Some(item) = item {
                slot.insert(super::super::combat_ui::bags::ReadOnlyItemTip {item: item.clone(), extra_lines: vec![]});
            }
            slot.with_children(|row| {
                if let Some(item) = item {
                    super::super::combat_ui::bags::icon(row, item, 40.0, inventory.equippable(item));
                    row.spawn(text(font, format!("{}\nilvl {}", item.name, item.level), 14.0, item.rarity.color()));
                } else {
                    row.spawn(text(font, "Drop an item here\nor right-click it in your bags", 13.0, Color::WHITE));
                }
            });
            ui.spawn(skin::rect(20.0, 150.0, 325.0, 28.0)).with_children(|row| smith_button(row, font, "Remove item / cancel edits", Click::ClearSource, 325.0));
            ui.spawn((text(font, "Only marked affixes are rerolled.\nOther affixes keep their exact values.\nLegendary powers cannot be edited.", 12.0, Color::WHITE), skin::rect(20.0, 455.0, 325.0, 52.0)));
            ui.spawn(Node {column_gap: Val::Px(8.0), ..skin::rect(20.0, 515.0, 325.0, 24.0)}).with_children(|row| {
                match cost(view, inventory) {
                    Ok(price) => { row.spawn(text(font, format!("Price ({} affixes):", edits(view).len()), 12.0, hud::GOLD_TONE)); coins(row, font, price); }
                    Err(error) => { row.spawn(text(font, error, 12.0, hud::GOLD_TONE)); }
                }
            });
            ui.spawn(skin::rect(20.0, 546.0, 325.0, 30.0)).with_children(|row| smith_button(row, font, "Apply recraft", Click::Buy, 325.0));
            ui.spawn(Node {column_gap: Val::Px(8.0), ..skin::rect(20.0, 586.0, 325.0, 24.0)}).with_children(|row| {
                row.spawn(text(font, "Your gold:", 12.0, hud::GOLD_TONE)); coins(row, font, inventory.money());
            });
            ui.spawn((text(font, "Edit ordinary affixes", 16.0, hud::GOLD_TONE), skin::rect(370.0, 65.0, 405.0, 24.0)));
            if let Some(item) = item {
                for (group, title) in [(0, "Prefixes"), (1, "Suffixes")] {
                    let top = 100.0 + group as f32 * 175.0;
                    ui.spawn((text(font, title, 14.0, hud::GOLD_TONE), skin::rect(370.0, top, 405.0, 22.0)));
                    let original = service::slots(item).unwrap_or([None; 6]);
                    for index in 0..view.affix_cap() {
                        let slot = group * 3 + index;
                        let label = if view.recraft_dirty[slot] {
                            format!("* {}", view.affixes[slot].as_ref().and_then(|id| choices.iter().find(|a| &a.id == id))
                                .map_or("Remove affix", |a| a.label.as_str()))
                        } else {
                            original[slot].map_or_else(|| "Add an affix".into(), |a| a.description())
                        };
                        ui.spawn(skin::rect(370.0, top + 28.0 + index as f32 * 42.0, 405.0, 32.0))
                            .with_children(|row| smith_button(row, font, format!("{label}  v"), Click::Dropdown(slot), 405.0));
                    }
                }
                if item.legendary.is_some() {
                    ui.spawn((text(font, format!("Legendary power preserved: {}", item.name), 12.0, item.rarity.color()), skin::rect(370.0, 455.0, 405.0, 45.0)));
                }
                if let Ok(price) = service::price(item.level) {
                    let (gold, silver, copper) = super::super::currency::split(price);
                    ui.spawn((text(font, format!("Per edited/added affix: {gold}g {silver}s {copper}c\nPrice doubles every 10 ilvls.\n* = pending change; values roll when applied.",), 12.0, Color::WHITE), skin::rect(370.0, 510.0, 405.0, 54.0)));
                }
            } else {
                ui.spawn((text(font, "Select an existing item to edit its affixes.\nEquipped items and legendaries are supported.", 13.0, Color::WHITE), skin::rect(370.0, 110.0, 405.0, 70.0)));
            }
            if !view.notice.is_empty() {
                ui.spawn((text(font, &view.notice, 12.0, hud::GOLD_TONE), skin::rect(370.0, 575.0, 405.0, 40.0)));
            }
            if let Some(slot) = view.dropdown.filter(|slot| view.slot_kind(*slot).is_some()) {
                ui.spawn((Button, Click::DismissDropdown, GlobalZIndex(646), BackgroundColor(Color::NONE), skin::rect(0.0, 0.0, 800.0, 624.0)));
                ui.spawn((ScrollArea, ScrollPosition::default(), RelativeCursorPosition::default(), GlobalZIndex(650),
                    BackgroundColor(Color::srgb(0.045, 0.035, 0.025)), BorderColor::all(hud::GOLD_TONE),
                    Node {border: UiRect::all(Val::Px(1.0)), padding: UiRect::all(Val::Px(4.0)), flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(3.0), overflow: Overflow::scroll_y(), ..skin::rect(370.0, 150.0, 405.0, 310.0)}))
                    .with_children(|list| {
                        smith_button(list, font, "Keep original / cancel this edit", Click::RestoreAffix(slot), 393.0);
                        smith_button(list, font, "Remove affix", Click::SelectAffix(slot, None), 393.0);
                        for index in view.dropdown_choices(&choices, slot) {
                            smith_button(list, font, &choices[index].label, Click::SelectAffix(slot, Some(index)), 393.0);
                        }
                    });
            }
        });
}
