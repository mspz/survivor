use super::*;
use crate::survivors::{Selection, hud};
use bevy::{input::keyboard::KeyboardInput, ui::RelativeCursorPosition, window::PrimaryWindow};

const GOLD: Color = Color::srgb(1.0, 0.82, 0.2);
const MUTED: Color = Color::srgb(0.64, 0.62, 0.56);
const EDGE: Color = Color::srgb(0.23, 0.19, 0.12);

fn button(ui: &mut ChildSpawnerCommands, font: &bevy::asset::Handle<Font>, title: impl Into<String>,
    action: Click, width: f32, selected: bool) {
    ui.spawn((Button, action,
        BackgroundColor(if selected { Color::srgb(0.25, 0.17, 0.06) } else { Color::srgb(0.075, 0.065, 0.045) }),
        BorderColor::all(if selected { GOLD } else { Color::srgb(0.32, 0.26, 0.15) }),
        Node { width: Val::Px(width), min_height: Val::Px(32.0), flex_shrink: 0.0,
            padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)), border: UiRect::all(Val::Px(1.0)),
            align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() },
    )).with_children(|b| {
        b.spawn((hud::text_bundle(font, title, 13.0, if selected { GOLD } else { Color::srgb(0.88, 0.85, 0.75) }),
            bevy::ui::FocusPolicy::Pass));
    });
}


#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::survivors) enum Tab {
    #[default]
    Spells,
    Passives,
}
#[derive(Resource, Default)]
pub(in crate::survivors) struct Panel {
    pub open: bool,
    selected: Option<u64>,
    tab: Tab,
    renaming: bool,
    name: String,
    dragging: Option<(usize, Vec2, bool)>,
    delete: bool,
}
#[derive(Component, Clone, Copy)]
pub(in crate::survivors) enum Click {
    Open,
    Close,
    Select(u64),
    Create,
    Activate,
    Deactivate,
    Delete,
    ConfirmDelete,
    CancelDelete,
    Rename,
    SaveName,
    Tab(Tab),
    Move(usize, i32),
}
#[derive(Component)]
pub(super) struct Root;
#[derive(Component)]
pub(super) struct List(usize);
#[derive(Component)]
pub(super) struct Row(usize);
#[derive(Component)]
pub(super) struct Handle(usize);
#[derive(Component, Clone)]
pub(super) struct BuildTip {
    name: String,
    lines: Vec<String>,
}
#[derive(Component)]
pub(super) struct Tooltip;

fn spell_tip(def: &'static data::AbilityDef, reference: &Run,
    spells: Option<&crate::ui_action::Spells>) -> BuildTip {
    let resolved = reference.resolve_spell(def, 1);
    let card = hud::describe_card(hud::Choice::NewAbility(def), reference, spells);
    BuildTip {
        name: card.name,
        lines: vec![reference.class.name.into(), resolved.tags.labels(), card.desc,
            format!("Cooldown: {:.1} s · Rank 1", resolved.cooldown)],
    }
}

fn passive_tip(passive: &'static data::PassiveDef) -> BuildTip {
    let lines = if super::super::passives::legendary(passive) {
        vec![format!("Legendary: {}", hud::passive_effect_text(passive, 1))]
    } else {
        ["Common", "Uncommon", "Epic", "Legendary"].into_iter().enumerate()
            .map(|(rank, rarity)| format!("{rarity}: {}", hud::passive_effect_text(passive, rank as u64 + 1)))
            .collect()
    };
    BuildTip { name: passive.name.into(), lines }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn input(
    phase: Res<State<Phase>>,
    mut next: ResMut<NextState<Phase>>,
    run: Res<Run>,
    mut panel: ResMut<Panel>,
    mut store: ResMut<Store>,
    mut selection: ResMut<Selection>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut typing: MessageReader<KeyboardInput>,
    clicks: Query<(&Interaction, &Click), Changed<Interaction>>,
    mut roster: ResMut<crate::survivors::characters::Roster>,
    mut services: (
        Option<ResMut<crate::survivors::bank::View>>,
        Option<ResMut<crate::survivors::shared_chest::View>>,
        Option<ResMut<crate::survivors::gambling::View>>,
    ),
) {
    if *phase.get() != Phase::Hub && !(*phase.get() == Phase::CharacterSheet && selection.in_hub) {
        *panel = Panel::default();
        typing.clear();
        return;
    }
    for (interaction, click) in &clicks {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if matches!(click, Click::Open) {
            panel.open = true;
            if *phase.get() == Phase::CharacterSheet {
                next.set(Phase::Hub);
            }
            panel.selected = store
                .active_build(run.class.id)
                .or_else(|| store.builds.iter().find(|b| b.class == run.class.id))
                .map(|b| b.id);
            panel.renaming = false;
            panel.delete = false;
            if let Some(v) = services.0.as_mut() {
                v.open = false;
            }
            if let Some(v) = services.1.as_mut() {
                v.open = false;
            }
            if let Some(v) = services.2.as_mut() {
                v.open = false;
            }
            continue;
        }
        if !panel.open {
            continue;
        }
        match *click {
            Click::Close => panel.open = false,
            Click::Select(id)
                if store
                    .builds
                    .iter()
                    .any(|b| b.id == id && b.class == run.class.id) =>
            {
                panel.selected = Some(id);
                panel.renaming = false;
                panel.delete = false;
                panel.dragging = None;
            }
            Click::Create => {
                panel.selected = store.create(run.class);
                panel.renaming = false;
                panel.delete = false;
            }
            Click::Activate => {
                if let Some(id) = panel.selected.filter(|id| {
                    store
                        .builds
                        .iter()
                        .any(|b| b.id == *id && b.class == run.class.id)
                }) {
                    store.active.insert(run.class.id, id);
                    store.save();
                }
            }
            Click::Deactivate => {
                store.active.remove(&run.class.id);
                store.save();
            }
            Click::Delete => panel.delete = panel.selected.is_some(),
            Click::ConfirmDelete if panel.delete => {
                if let Some(id) = panel.selected.take() {
                    store.delete(id);
                }
                panel.selected = store
                    .builds
                    .iter()
                    .find(|b| b.class == run.class.id)
                    .map(|b| b.id);
                panel.delete = false;
                panel.renaming = false;
            }
            Click::CancelDelete => panel.delete = false,
            Click::Tab(tab) => {
                panel.tab = tab;
                panel.dragging = None;
            }
            Click::Move(from, delta) if !panel.delete => {
                if let Some(id) = panel.selected {
                    store.reorder(
                        id,
                        panel.tab,
                        from,
                        from.saturating_add_signed(delta as isize),
                    );
                }
            }
            Click::Rename => {
                if let Some(b) = panel
                    .selected
                    .and_then(|id| store.builds.iter().find(|b| b.id == id))
                {
                    panel.name = b.name.clone();
                    panel.renaming = true;
                }
            }
            Click::SaveName => save_name(&mut panel, &mut store),
            _ => {}
        }
    }
    if !panel.open {
        panel.dragging = None;
        typing.clear();
        return;
    }
    selection.sheet_open = false;
    selection.bags_open = false;
    selection.talents_open = false;
    selection.ui_captured = true;
    selection.pointer_captured = true;
    roster.expeditions = false;
    if keys.just_pressed(KeyCode::Escape) {
        if panel.renaming {
            panel.renaming = false;
        } else if panel.delete {
            panel.delete = false;
        } else {
            panel.open = false;
            panel.dragging = None;
        }
        keys.clear_just_pressed(KeyCode::Escape);
    }
    if panel.renaming {
        for event in typing.read().filter(|e| e.state.is_pressed()) {
            if let Some(text) = &event.text {
                for c in text.chars().filter(|c| !c.is_control()) {
                    if panel.name.chars().count() < 40 {
                        panel.name.push(c);
                    }
                }
            }
        }
        if keys.just_pressed(KeyCode::Backspace) {
            panel.name.pop();
        }
        if keys.just_pressed(KeyCode::Enter) {
            save_name(&mut panel, &mut store);
        }
    } else {
        typing.clear();
    }
}
fn save_name(panel: &mut Panel, store: &mut Store) {
    let name = panel.name.trim();
    if name.is_empty() {
        return;
    }
    if let Some(build) = panel
        .selected
        .and_then(|id| store.builds.iter_mut().find(|b| b.id == id))
    {
        build.name = name.to_owned();
        store.save();
    }
    panel.renaming = false;
}

pub(super) fn drag(
    mut panel: ResMut<Panel>,
    mut store: ResMut<Store>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    scale: Option<Res<UiScale>>,
    handles: Query<(&Handle, &Interaction, &RelativeCursorPosition)>,
    mut rows: Query<(&Row, &RelativeCursorPosition, &mut BackgroundColor)>,
) {
    if !panel.open || panel.delete || panel.renaming {
        panel.dragging = None;
        return;
    }
    let cursor = windows
        .single()
        .ok()
        .filter(|w| w.focused)
        .and_then(Window::cursor_position)
        .map(|p| p / scale.as_deref().map_or(1.0, |s| s.0).max(0.01));
    let Some(cursor) = cursor else {
        panel.dragging = None;
        return;
    };
    if mouse.just_pressed(MouseButton::Left) {
        panel.dragging = handles
            .iter()
            .find(|(_, i, p)| **i == Interaction::Pressed && p.cursor_over())
            .map(|(h, _, _)| (h.0, cursor, false));
    }
    if let Some((from, start, moving)) = panel.dragging {
        let moving = moving || start.distance(cursor) >= 5.0;
        panel.dragging = Some((from, start, moving));
        if mouse.just_released(MouseButton::Left) {
            if moving {
                if let (Some(id), Some(to)) = (
                    panel.selected,
                    rows.iter()
                        .find(|(_, p, _)| p.cursor_over())
                        .map(|(r, _, _)| r.0),
                ) {
                    store.reorder(id, panel.tab, from, to);
                }
            }
            panel.dragging = None;
        } else if !mouse.pressed(MouseButton::Left) {
            panel.dragging = None;
        }
    }
    for (row, cursor, mut color) in &mut rows {
        color.0 = if panel
            .dragging
            .is_some_and(|(from, _, moving)| moving && (row.0 == from || cursor.cursor_over()))
        {
            Color::srgb(0.23, 0.18, 0.08)
        } else if row.0 % 2 == 0 {
            Color::srgb(0.075, 0.065, 0.045)
        } else {
            Color::srgb(0.055, 0.047, 0.037)
        };
    }
}
pub(super) fn scroll(
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    panel: Res<Panel>,
    time: Res<Time<Real>>,
    mut lists: Query<(&ComputedNode, &RelativeCursorPosition, &mut ScrollPosition), With<List>>,
) {
    let events: Vec<_> = wheel.read().collect();
    if !panel.open {
        return;
    }
    for (node, cursor, mut pos) in &mut lists {
        if !cursor.cursor_over() {
            continue;
        }
        let mut delta = 0.0;
        for e in &events {
            delta -= e.y
                * if e.unit == bevy::input::mouse::MouseScrollUnit::Line {
                    36.0
                } else {
                    1.0
                };
        }
        if panel.dragging.is_some_and(|(_, _, moving)| moving) {
            if let Some(p) = cursor.normalized {
                if p.y.abs() > 0.38 {
                    delta += p.y.signum() * 360.0 * time.delta_secs();
                }
            }
        }
        let max = ((node.content_size.y - node.size.y) * node.inverse_scale_factor).max(0.0);
        pos.0.y = (pos.0.y + delta).clamp(0.0, max);
    }
}

pub(super) fn render(
    mut commands: Commands,
    panel: Res<Panel>,
    store: Res<Store>,
    run: Res<Run>,
    phase: Res<State<Phase>>,
    font: Option<Res<crate::survivors::hud::HudFont>>,
    spells: Option<Res<crate::ui_action::Spells>>,
    roots: Query<Entity, With<Root>>,
    mut cache: Local<String>,
    lists: Query<(&List, &ScrollPosition)>,
    mut last_list: Local<Option<(Option<u64>, Tab)>>,
) {
    let Some(font) = font else {
        return;
    };
    let show = panel.open && *phase.get() == Phase::Hub;
    let key = format!(
        "{show} {} {} {:?} {:?} {} {} {}",
        run.class.id,
        store.revision,
        panel.selected,
        panel.tab,
        panel.renaming,
        panel.name,
        panel.delete
    );
    if *cache == key && !spells.as_ref().is_some_and(|s| s.is_changed()) {
        return;
    }
    *cache = key;
    let list_key = (panel.selected, panel.tab);
    let mut offsets = [Vec2::ZERO; 2];
    if last_list.as_ref() == Some(&list_key) {
        for (list, pos) in &lists {
            offsets[list.0] = pos.0;
        }
    }
    *last_list = Some(list_key);
    for root in &roots {
        commands.entity(root).despawn();
    }
    if !show {
        return;
    }
    let f = &font.0;
    let mut reference = Run::default();
    reference.balance = run.balance.clone();
    reference.class = run.class;
    commands.spawn((Root, GlobalZIndex(720), BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.88)),
        Node { position_type: PositionType::Absolute, left: Val::Px(0.0), right: Val::Px(0.0), top: Val::Px(0.0), bottom: Val::Px(0.0),
            padding: UiRect::axes(Val::Percent(5.0), Val::Percent(5.0)), ..default() }))
        .with_children(|overlay| {
            overlay.spawn((Node { width: Val::Percent(100.0), height: Val::Percent(100.0), padding: UiRect::all(Val::Px(16.0)),
                flex_direction: FlexDirection::Column, row_gap: Val::Px(12.0), border: UiRect::all(Val::Px(1.0)), ..default() },
                BackgroundColor(Color::srgb(0.025, 0.022, 0.018)), BorderColor::all(Color::srgb(0.55, 0.43, 0.15))))
                .with_children(|ui| {
                    ui.spawn(Node { width: Val::Percent(100.0), flex_shrink: 0.0,
                        align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween,
                        column_gap: Val::Px(12.0), ..default() }).with_children(|header| {
                            header.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() }).with_children(|title| {
                                title.spawn(hud::text_bundle(f, format!("{} Builds", run.class.name), 25.0, GOLD));
                                title.spawn(hud::text_bundle(f, "Set card priority. Auto chooses a card only when you press its button or key.", 12.0, MUTED));
                            });
                            button(header, f, "Close", Click::Close, 90.0, false);
                        });
                    ui.spawn(Node { flex_grow: 1.0, min_height: Val::Px(0.0), column_gap: Val::Px(18.0), ..default() }).with_children(|body| {
                        body.spawn((BackgroundColor(Color::srgb(0.055, 0.047, 0.037)), BorderColor::all(EDGE),
                            Node { width: Val::Percent(26.0), min_width: Val::Px(210.0), min_height: Val::Px(0.0),
                                padding: UiRect::all(Val::Px(12.0)), border: UiRect::all(Val::Px(1.0)),
                                flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), ..default() })).with_children(|left| {
                            left.spawn(hud::text_bundle(f, "Saved builds", 15.0, GOLD));
                            button(left, f, "Create build", Click::Create, 160.0, false);
                            left.spawn((List(0), ScrollPosition(offsets[0]), RelativeCursorPosition::default(), Node { flex_grow: 1.0, min_height: Val::Px(0.0), overflow: Overflow::scroll_y(), flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), ..default() })).with_children(|list| {
                                for b in store.builds.iter().filter(|b| b.class == run.class.id) {
                                    let active = store.active.get(&b.class) == Some(&b.id);
                                    button(list, f, format!("{}{}", if active { "[Active] " } else { "" }, b.name), Click::Select(b.id), 180.0, panel.selected == Some(b.id));
                                }
                            });
                            if panel.selected.is_some() {
                                button(left, f, "Activate", Click::Activate, 160.0, false);
                                button(left, f, "Rename", Click::Rename, 160.0, false);
                                button(left, f, "Delete", Click::Delete, 160.0, false);
                            }
                            if store.active_build(run.class.id).is_some() { button(left, f, "Disable Auto", Click::Deactivate, 160.0, false); }
                        });
                        body.spawn((BackgroundColor(Color::srgb(0.055, 0.047, 0.037)), BorderColor::all(EDGE),
                            Node { flex_grow: 1.0, min_width: Val::Px(0.0), min_height: Val::Px(0.0),
                                padding: UiRect::all(Val::Px(12.0)), border: UiRect::all(Val::Px(1.0)),
                                flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), ..default() })).with_children(|right| {
                            let Some(build) = panel.selected.and_then(|id| store.builds.iter().find(|b| b.id == id && b.class == run.class.id)) else {
                                right.spawn(hud::text_bundle(f, "Create or select a build to edit its priorities.", 18.0, hud::GOLD_TONE)); return;
                            };
                            if panel.delete {
                                right.spawn(hud::text_bundle(f, format!("Delete '{}' permanently?", build.name), 19.0, hud::GOLD_TONE));
                                button(right, f, "Cancel", Click::CancelDelete, 180.0, false);
                                button(right, f, "Delete build", Click::ConfirmDelete, 180.0, false);
                                return;
                            }
                            if panel.renaming {
                                right.spawn(hud::text_bundle(f, format!("Name: {}_", panel.name), 20.0, hud::GOLD_TONE));
                                right.spawn(hud::text_bundle(f, "Type a name, Enter to save, Escape to cancel.", 13.0, Color::WHITE));
                                button(right, f, "Save name", Click::SaveName, 180.0, false);
                            } else { right.spawn(hud::text_bundle(f, build.name.clone(), 20.0, hud::GOLD_TONE)); }
                            right.spawn(Node { column_gap: Val::Px(10.0), ..default() }).with_children(|tabs| {
                                button(tabs, f, "Spells", Click::Tab(Tab::Spells), 110.0, panel.tab == Tab::Spells);
                                button(tabs, f, "Passives", Click::Tab(Tab::Passives), 110.0, panel.tab == Tab::Passives);
                            });
                            right.spawn(hud::text_bundle(f, "Drag an icon or use the arrows to change priority.", 12.0, MUTED));
                            let order = match panel.tab { Tab::Spells => &build.spells, Tab::Passives => &build.passives };
                            right.spawn((List(1), ScrollPosition(offsets[1]), RelativeCursorPosition::default(), Node { flex_grow: 1.0, min_height: Val::Px(0.0), overflow: Overflow::scroll_y(), flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), ..default() })).with_children(|list| {
                                for (i, key) in order.iter().enumerate() {
                                    let (name, spell, tip) = match panel.tab {
                                        Tab::Spells => {
                                            let def = data::ability(key);
                                            let tip = spell_tip(def, &reference, spells.as_deref());
                                            (tip.name.clone(), def.spell, tip)
                                        }
                                        Tab::Passives => data::PASSIVES.iter().find(|p| p.key == key)
                                            .map_or_else(|| (key.clone(), 0, BuildTip { name: key.clone(), lines: Vec::new() }),
                                                |p| (p.name.to_owned(), p.spell, passive_tip(p))),
                                    };
                                    list.spawn((Row(i), RelativeCursorPosition::default(), BackgroundColor(Color::srgb(0.06, 0.055, 0.045)), BorderColor::all(EDGE), Node { min_height: Val::Px(44.0), flex_shrink: 0.0, padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)), border: UiRect::all(Val::Px(1.0)), column_gap: Val::Px(8.0), align_items: AlignItems::Center, ..default() })).with_children(|row| {
                                        row.spawn((Button, Handle(i), tip.clone(), RelativeCursorPosition::default(), BackgroundColor(Color::srgb(0.075, 0.065, 0.045)), BorderColor::all(Color::srgb(0.32, 0.26, 0.15)), Node { width: Val::Px(36.0), height: Val::Px(32.0), border: UiRect::all(Val::Px(1.0)), align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() })).with_children(|h| {
                                            if spell != 0 {
                                                h.spawn((ImageNode::default(), hud::SpellIcon(spell), bevy::ui::FocusPolicy::Pass,
                                                    Node { width: Val::Px(30.0), height: Val::Px(30.0), ..default() }));
                                            }
                                        });
                                        row.spawn((hud::text_bundle(f, format!("{}. {name}", i + 1), 15.0, Color::WHITE),
                                            Interaction::default(), tip,
                                            Node { flex_grow: 1.0, min_width: Val::Px(0.0), ..default() }));
                                        if i > 0 { button(row, f, "^", Click::Move(i, -1), 36.0, false); }
                                        if i + 1 < order.len() { button(row, f, "v", Click::Move(i, 1), 36.0, false); }
                                    });
                                }
                            });
                        });
                    });
                    if !store.notice.is_empty() { ui.spawn(hud::text_bundle(f, store.notice.clone(), 14.0, Color::srgb(1.0, 0.45, 0.3))); }
                    ui.spawn(hud::text_bundle(f, "Build priorities save automatically · Esc to close", 11.0, MUTED));
                });
        });
}

pub(super) fn tooltip(
    mut commands: Commands,
    panel: Res<Panel>,
    font: Option<Res<hud::HudFont>>,
    hover: Query<(&Interaction, &BuildTip)>,
    roots: Query<Entity, With<Tooltip>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    scale: Option<Res<UiScale>>,
) {
    for root in &roots {
        commands.entity(root).despawn();
    }
    if !panel.open || panel.dragging.is_some() {
        return;
    }
    let (Some(font), Some((_, tip))) = (font, hover.iter().find(|(i, _)| **i != Interaction::None)) else {
        return;
    };
    let inv = scale.as_deref().map_or(1.0, |scale| scale.0).max(0.01);
    let (viewport, cursor) = windows.single().map_or(
        (Vec2::new(1280.0, 768.0), Vec2::new(700.0, 120.0)),
        |window| (Vec2::new(window.width(), window.height()) / inv,
            window.cursor_position().unwrap_or_default() / inv),
    );
    let width = 350.0_f32.min((viewport.x - 16.0).max(1.0));
    let height = 60.0 + tip.lines.iter()
        .map(|line| (line.chars().count() / 42 + 1 + line.matches('\n').count()) as f32 * 18.0)
        .sum::<f32>();
    let x = if cursor.x + width + 16.0 > viewport.x {
        cursor.x - width - 16.0
    } else {
        cursor.x + 16.0
    };
    commands.spawn((Tooltip, GlobalZIndex(850), bevy::ui::FocusPolicy::Pass,
        BackgroundColor(Color::srgb(0.02, 0.025, 0.04)), BorderColor::all(GOLD),
        Node { position_type: PositionType::Absolute,
            left: Val::Px(x.clamp(8.0, (viewport.x - width - 8.0).max(8.0))),
            top: Val::Px((cursor.y + 16.0).clamp(8.0, (viewport.y - height - 8.0).max(8.0))),
            width: Val::Px(width), max_height: Val::Vh(90.0),
            padding: UiRect::all(Val::Px(12.0)), border: UiRect::all(Val::Px(1.0)),
            flex_direction: FlexDirection::Column, row_gap: Val::Px(7.0), ..default() },
    )).with_children(|body| {
        body.spawn(hud::text_bundle(&font.0, &tip.name, 18.0, GOLD));
        for line in &tip.lines {
            body.spawn(hud::text_bundle(&font.0, line, 14.0, Color::WHITE));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn build_spell_and_passive_icons_show_reference_tooltips_on_hover() {
        let mut app = app();
        app.insert_resource(hud::HudFont(bevy::asset::Handle::default()))
            .add_systems(Update, (render, tooltip).chain());
        app.update();
        let spell_icon = {
            let world = app.world_mut();
            world.query::<(Entity, &Handle, &BuildTip)>()
                .iter(world).find(|(_, handle, _)| handle.0 == 0).unwrap().0
        };
        let spell_tip = app.world().get::<BuildTip>(spell_icon).unwrap();
        assert!(spell_tip.lines.iter().any(|line| line.contains("Cooldown:")));
        app.world_mut().entity_mut(spell_icon).insert(Interaction::Hovered);
        app.update();
        assert_eq!(app.world_mut().query::<&Tooltip>().iter(app.world()).count(), 1);
        app.world_mut().resource_mut::<Panel>().tab = Tab::Passives;
        app.update();
        let passive_tip = {
            let world = app.world_mut();
            world.query::<(&Handle, &BuildTip)>().iter(world)
                .find(|(handle, _)| handle.0 == 0).unwrap().1.clone()
        };
        assert!(passive_tip.lines.iter().any(|line| line.contains("Common:") || line.contains("Legendary:")));
    }
    fn app() -> App {
        let mut app = App::new();
        let run = Run::default();
        let store = Store::fixture(run.class);
        let id = store.active_build(run.class.id).unwrap().id;
        app.insert_resource(run)
            .insert_resource(store)
            .insert_resource(State::new(Phase::Hub))
            .init_resource::<NextState<Phase>>()
            .insert_resource(Panel {
                open: true,
                selected: Some(id),
                ..default()
            })
            .init_resource::<Selection>()
            .init_resource::<crate::survivors::characters::Roster>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_message::<KeyboardInput>()
            .add_systems(Update, (input, drag).chain());
        let mut window = Window::default();
        window.focused = true;
        window.set_cursor_position(Some(Vec2::new(100.0, 100.0)));
        app.world_mut().spawn((PrimaryWindow, window));
        app
    }
    fn click(app: &mut App, click: Click) {
        let button = app.world_mut().spawn((Interaction::Pressed, click)).id();
        app.update();
        app.world_mut().despawn(button);
    }
    #[test]
    fn management_and_arrows_keep_active_build_separate_from_selection() {
        let mut app = app();
        let class = app.world().resource::<Run>().class.id;
        let original = app.world().resource::<Store>().active[&class];
        let old = app.world().resource::<Store>().builds[0].spells.clone();
        click(&mut app, Click::Move(1, -1));
        assert_eq!(app.world().resource::<Store>().builds[0].spells[0], old[1]);
        click(&mut app, Click::Create);
        let selected = app.world().resource::<Panel>().selected.unwrap();
        assert_ne!(selected, original);
        assert_eq!(app.world().resource::<Store>().active[&class], original);
        click(&mut app, Click::Activate);
        assert_eq!(app.world().resource::<Store>().active[&class], selected);
        click(&mut app, Click::Rename);
        app.world_mut().resource_mut::<Panel>().name = "My rogue build".into();
        click(&mut app, Click::SaveName);
        assert_eq!(
            app.world()
                .resource::<Store>()
                .active_build(class)
                .unwrap()
                .name,
            "My rogue build"
        );
        click(&mut app, Click::Delete);
        assert_eq!(app.world().resource::<Store>().builds.len(), 2);
        click(&mut app, Click::ConfirmDelete);
        assert_eq!(app.world().resource::<Store>().builds.len(), 1);
        assert!(
            app.world()
                .resource::<Store>()
                .active_build(class)
                .is_none()
        );
        assert!(app.world().resource::<Selection>().ui_captured);
    }
    #[test]
    fn dragging_reorders_on_release_and_never_on_an_unfinished_drag() {
        let mut app = app();
        let before = app.world().resource::<Store>().builds[0].spells.clone();
        app.world_mut().spawn((
            Handle(2),
            Interaction::Pressed,
            RelativeCursorPosition {
                cursor_over: true,
                normalized: Some(Vec2::ZERO),
            },
        ));
        app.world_mut().spawn((
            Row(0),
            BackgroundColor::default(),
            RelativeCursorPosition {
                cursor_over: true,
                normalized: Some(Vec2::ZERO),
            },
        ));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear_just_pressed(MouseButton::Left);
        let world = app.world_mut();
        let mut windows = world.query::<&mut Window>();
        windows
            .single_mut(world)
            .unwrap()
            .set_cursor_position(Some(Vec2::new(180.0, 100.0)));
        app.update();
        assert_eq!(app.world().resource::<Store>().builds[0].spells, before);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.update();
        let order = &app.world().resource::<Store>().builds[0].spells;
        assert_eq!(order[0], before[2]);
        assert_eq!(order.len(), before.len());
        assert!(app.world().resource::<Panel>().dragging.is_none());
    }
    #[test]
    fn menu_is_hub_only_and_escape_is_consumed_without_pausing() {
        let mut app = app();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        assert!(!app.world().resource::<Panel>().open);
        assert!(
            !app.world()
                .resource::<ButtonInput<KeyCode>>()
                .just_pressed(KeyCode::Escape)
        );
        app.insert_resource(State::new(Phase::Playing));
        click(&mut app, Click::Open);
        assert!(!app.world().resource::<Panel>().open);
        app.insert_resource(State::new(Phase::CharacterSheet));
        app.world_mut().resource_mut::<Selection>().in_hub = true;
        click(&mut app, Click::Open);
        assert!(app.world().resource::<Panel>().open);
        assert!(matches!(
            app.world().resource::<NextState<Phase>>(),
            NextState::Pending(Phase::Hub)
        ));
        assert!(!app.world().resource::<Selection>().sheet_open);
    }
}
