//! Paper-doll layout and tag-aware current spell statistics, using client art from the MPQs.
use super::super::{
    spell_engine::{Change, Tags},
    talents,
};
use super::*;
mod stats;
pub(super) const HEIGHT: f32 = 626.0;
const FRAME_HEIGHT: f32 = 532.0;
#[derive(Component)]
pub(super) struct Root;
#[derive(Component)]
struct StatsText(usize);
#[derive(Component)]
struct CharacterTitle;
#[derive(Component)]
struct EquipmentLevel;
#[derive(Component)]
struct ParagonLabel;
#[derive(Component)]
struct ParagonFill;
#[derive(Component)]
struct CharacterXpLabel;
#[derive(Component)]
struct CharacterXpFill;
#[derive(Component)]
struct SkillTitle;
#[derive(Component)]
struct SlotHint;
#[derive(Component)]
struct Slot(&'static str);
// The same 19 positions as Classic's paper doll.
const LEFT: [(&str, &str); 8] = [
    ("Head", "Head"),
    ("Neck", "Neck"),
    ("Shoulders", "Shoulder"),
    ("Back", "Chest"),
    ("Chest", "Chest"),
    ("Shirt", "Shirt"),
    ("Tabard", "Tabard"),
    ("Wrists", "Wrists"),
];
const RIGHT: [(&str, &str); 8] = [
    ("Hands", "Hands"),
    ("Waist", "Waist"),
    ("Legs", "Legs"),
    ("Feet", "Feet"),
    ("Ring 1", "Finger"),
    ("Ring 2", "Finger"),
    ("Trinket 1", "Trinket"),
    ("Trinket 2", "Trinket"),
];
fn slot_path(kind: &str) -> &'static str {
    match kind {
        "Head" => "Interface\\PaperDoll\\UI-PaperDoll-Slot-Head",
        "Neck" => "Interface\\PaperDoll\\UI-PaperDoll-Slot-Neck",
        "Shoulder" => "Interface\\PaperDoll\\UI-PaperDoll-Slot-Shoulder",
        "Chest" => "Interface\\PaperDoll\\UI-PaperDoll-Slot-Chest",
        "Shirt" => "Interface\\PaperDoll\\UI-PaperDoll-Slot-Shirt",
        "Tabard" => "Interface\\PaperDoll\\UI-PaperDoll-Slot-Tabard",
        "Wrists" => "Interface\\PaperDoll\\UI-PaperDoll-Slot-Wrists",
        "Hands" => "Interface\\PaperDoll\\UI-PaperDoll-Slot-Hands",
        "Waist" => "Interface\\PaperDoll\\UI-PaperDoll-Slot-Waist",
        "Legs" => "Interface\\PaperDoll\\UI-PaperDoll-Slot-Legs",
        "Feet" => "Interface\\PaperDoll\\UI-PaperDoll-Slot-Feet",
        "Finger" => "Interface\\PaperDoll\\UI-PaperDoll-Slot-Finger",
        "Trinket" => "Interface\\PaperDoll\\UI-PaperDoll-Slot-Trinket",
        "MainHand" => "Interface\\PaperDoll\\UI-PaperDoll-Slot-MainHand",
        "SecondaryHand" => "Interface\\PaperDoll\\UI-PaperDoll-Slot-SecondaryHand",
        _ => "Interface\\PaperDoll\\UI-PaperDoll-Slot-Ranged",
    }
}
fn rect(x: f32, y: f32, w: f32, h: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(x),
        top: Val::Px(y),
        width: Val::Px(w),
        height: Val::Px(h),
        ..default()
    }
}
fn art(ui: &mut ChildSpawnerCommands, path: &'static str, bounds: Node) {
    ui.spawn((
        ImageNode::default(),
        hud::UiArt::crop(path, [0.0, 1.0, 0.0, 1.0]),
        bounds,
    ));
}
fn slot(
    ui: &mut ChildSpawnerCommands,
    name: &'static str,
    kind: &str,
    x: f32,
    y: f32,
    item: Option<&super::super::items::Item>,
) {
    ui.spawn((
        Slot(name),
        Button,
        bags::DropTarget::Equipment(name),
        bevy::ui::RelativeCursorPosition::default(),
        Interaction::default(),
        BackgroundColor(Color::BLACK),
        BorderColor::all(Color::srgb(0.42, 0.40, 0.33)),
        Node {
            border: UiRect::all(Val::Px(1.0)),
            ..rect(x, y, 40.0, 40.0)
        },
    ))
    .with_children(|b| {
        art(b, slot_path(kind), rect(2.0, 2.0, 36.0, 36.0));
        art(
            b,
            "Interface\\Buttons\\UI-Quickslot2",
            rect(-7.0, -7.0, 54.0, 54.0),
        );
        if let Some(item) = item {
            bags::equipped(b, item, name);
        }
    });
}
pub(super) fn plugin(app: &mut App) {
    app.add_message::<bevy::input::mouse::MouseWheel>()
        .add_systems(Update, stats::scroll)
        .add_systems(Update, stats::tooltip.after(refresh))
        .add_systems(Update, refresh_paragon.after(refresh))
        .add_systems(Update, refresh_character_xp.after(refresh));
    app.add_systems(
        Update,
        (render, refresh)
            .chain()
            .after(super::input)
            .after(super::super::talent_ui::input)
            .after(super::super::characters::input),
    );
}
#[allow(clippy::too_many_arguments)]
pub(super) fn render(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    selection: Res<Selection>,
    view: Res<View>,
    run: Res<Run>,
    inventory: Option<Res<super::super::items::Inventory>>,
    font: Option<Res<hud::HudFont>>,
    target: Res<preview::Target>,
    roots: Query<Entity, With<Root>>,
    mut cache: Local<String>,
    runes: Option<Res<super::super::runes::Runes>>,
) {
    let Some(font) = font else {
        return;
    };
    let show = selection.sheet_open && in_world(*phase.get());
    let index = view
        .selected
        .min((run.abilities.len() + super::super::procs::available(&run).len()).saturating_sub(1));
    let key = format!(
        "{} {} {} {:?} {} {} {:?} {:?}",
        show,
        index,
        run.version,
        target.image,
        selection.bags_open,
        view.bag_page,
        view.selected_item,
        inventory.as_ref().map(|i| (i.revision, &i.notice))
    );
    let key = format!(
        "{key} {} {} {:?}",
        view.stats_collapsed,
        view.rune_page,
        runes.as_ref().map(|r| r.revision)
    );
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
    let f = &font.0;
    let panel_height = FRAME_HEIGHT - 74.0;
    commands
        .spawn((
            Root,
            GlobalZIndex(575),
            BackgroundColor(Color::srgb(0.018, 0.016, 0.014)),
            BorderColor::all(Color::srgb(0.36, 0.32, 0.24)),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(view.sheet.x),
                top: Val::Px(view.sheet.y),
                width: Val::Px(824.0),
                height: Val::Px(HEIGHT),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
        ))
        .with_children(|root| {
            skin::backdrop(root);
            root.spawn((
                BackgroundColor(Color::srgb(0.055, 0.045, 0.028)),
                BorderColor::all(Color::srgb(0.3, 0.26, 0.17)),
                Node {
                    border: UiRect::bottom(Val::Px(1.0)),
                    ..rect(10.0, 10.0, 804.0, 62.0)
                },
            ));
            skin::close(root, Click::CloseSheet, 784.0, 16.0);
            root.spawn((
                BackgroundColor(Color::srgb(0.025, 0.035, 0.075)),
                rect(12.0, 550.0, 800.0, 30.0),
            ))
            .with_children(|panel| {
                skin::backdrop(panel);
                panel.spawn((
                    ParagonLabel,
                    text(f, "Paragon", 13.0, Color::srgb(0.45, 0.75, 1.0)),
                    rect(12.0, 5.0, 310.0, 20.0),
                ));
                panel
                    .spawn((
                        BackgroundColor(Color::srgb(0.08, 0.1, 0.16)),
                        BorderColor::all(Color::srgb(0.38, 0.44, 0.57)),
                        Node {
                            border: UiRect::all(Val::Px(1.0)),
                            ..rect(332.0, 8.0, 454.0, 14.0)
                        },
                    ))
                    .with_children(|bar| {
                        bar.spawn((
                            ParagonFill,
                            BackgroundColor(Color::srgb(0.2, 0.5, 1.0)),
                            Node {
                                width: Val::Percent(0.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                        ));
                    });
            });
            root.spawn((
                BackgroundColor(Color::srgb(0.045, 0.025, 0.075)),
                rect(12.0, 584.0, 800.0, 30.0),
            ))
            .with_children(|panel| {
                skin::backdrop(panel);
                panel.spawn((
                    CharacterXpLabel,
                    text(f, "Experience", 13.0, Color::srgb(0.8, 0.55, 1.0)),
                    rect(12.0, 5.0, 310.0, 20.0),
                ));
                panel
                    .spawn((
                        BackgroundColor(Color::srgb(0.12, 0.08, 0.16)),
                        BorderColor::all(Color::srgb(0.48, 0.34, 0.57)),
                        Node {
                            border: UiRect::all(Val::Px(1.0)),
                            ..rect(332.0, 8.0, 454.0, 14.0)
                        },
                    ))
                    .with_children(|bar| {
                        bar.spawn((
                            CharacterXpFill,
                            BackgroundColor(Color::srgb(0.6, 0.25, 0.9)),
                            Node {
                                width: Val::Percent(0.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                        ));
                    });
            });
            root.spawn((
                BackgroundColor(Color::srgb(0.025, 0.023, 0.02)),
                rect(12.0, 74.0, 424.0, FRAME_HEIGHT - 74.0),
            ))
            .with_children(|frame| {
                frame
                    .spawn((
                        Button,
                        DragTitle(Pane::Sheet),
                        rect(64.0, -60.0, 680.0, 50.0),
                    ))
                    .with_children(|header| {
                        header.spawn((
                            CharacterTitle,
                            text(f, "Character", 17.0, GOLD),
                            Node {
                                padding: UiRect::all(Val::Px(6.0)),
                                ..default()
                            },
                        ));
                    });
                frame.spawn((
                    ImageNode::default(),
                    hud::SpellIcon(data::ability(run.class.start).spell),
                    rect(4.0, -60.0, 52.0, 52.0),
                ));
                frame
                    .spawn((
                        BorderColor::all(Color::srgb(0.28, 0.26, 0.22)),
                        BackgroundColor(Color::srgb(0.025, 0.022, 0.018)),
                        Node {
                            border: UiRect::all(Val::Px(1.0)),
                            ..rect(62.0, 8.0, 294.0, 354.0)
                        },
                    ))
                    .with_children(|body| {
                        if let Some(image) = &target.image {
                            body.spawn((
                                ImageNode::new(image.clone()),
                                Node {
                                    width: Val::Percent(100.0),
                                    height: Val::Percent(100.0),
                                    ..default()
                                },
                            ));
                        } else {
                            body.spawn(text(f, "Loading character...", 14.0, GREY));
                        }
                    });
                for (i, (name, kind)) in LEFT.iter().enumerate() {
                    slot(
                        frame,
                        name,
                        kind,
                        8.0,
                        10.0 + i as f32 * 44.0,
                        inventory.as_ref().and_then(|inv| inv.equipped(name)),
                    );
                }
                for (i, (name, kind)) in RIGHT.iter().enumerate() {
                    slot(
                        frame,
                        name,
                        kind,
                        372.0,
                        10.0 + i as f32 * 44.0,
                        inventory.as_ref().and_then(|inv| inv.equipped(name)),
                    );
                }
                for (i, (name, kind)) in [
                    ("Main hand", "MainHand"),
                    ("Off hand", "SecondaryHand"),
                    ("Ranged", "Ranged"),
                ]
                .iter()
                .enumerate()
                {
                    slot(
                        frame,
                        name,
                        kind,
                        140.0 + i as f32 * 48.0,
                        374.0,
                        inventory.as_ref().and_then(|inv| inv.equipped(name)),
                    );
                }
                frame
                    .spawn(Node {
                        column_gap: Val::Px(6.0),
                        ..rect(160.0, 332.0, 120.0, 24.0)
                    })
                    .with_children(|ui| {
                        button(ui, f, Click::Rotate(-0.4), "<", 35.0);
                        button(ui, f, Click::Rotate(0.4), ">", 35.0);
                    });
                frame.spawn((
                    SlotHint,
                    text(f, "Equipment", 12.0, GREY),
                    rect(24.0, 422.0, 376.0, 20.0),
                ));
                if view.rune_page {
                    if let Some(runes) = runes.as_deref() {
                        rune_page(frame, f, runes);
                    }
                }
                // Classic-style tabs under the frame: Character | Runes.
                for (i, (label, page)) in [("Character", false), ("Runes", true)]
                    .into_iter()
                    .enumerate()
                {
                    let active = view.rune_page == page;
                    frame
                        .spawn((
                            Button,
                            Click::SheetPage(page),
                            rect(18.0 + i as f32 * 128.0, 444.0, 126.0, 30.0),
                        ))
                        .with_children(|tab| {
                            skin::art(
                                tab,
                                if active {
                                    "Interface\\PaperDollInfoFrame\\UI-Character-ActiveTab"
                                } else {
                                    "Interface\\PaperDollInfoFrame\\UI-Character-InActiveTab"
                                },
                                rect(0.0, 0.0, 126.0, 30.0),
                                [0.0, 1.0, 0.0, if active { 0.75 } else { 1.0 }],
                            );
                            tab.spawn((
                                text(f, label, 14.0, if active { Color::WHITE } else { GREY }),
                                rect(16.0, if active { 6.0 } else { 4.0 }, 100.0, 22.0),
                            ));
                        });
                }
            });
            root.spawn((
                BackgroundColor(Color::srgb(0.025, 0.022, 0.018)),
                rect(440.0, 74.0, 372.0, panel_height),
            ))
            .with_children(|panel| {
                skin::frame(panel, 372.0, panel_height);
                panel.spawn((
                    EquipmentLevel,
                    text(f, "Average equipped item level", 15.0, GOLD),
                    rect(18.0, 12.0, 290.0, 26.0),
                ));
                stats::draw(panel, f, &run, &view, panel_height);
            });
        });
}
/// The Runes tab: the equipped rune in full over the paper doll, then the account's slots
/// with an Equip button each. Placing, swapping and upgrading happen at the results.
fn rune_page(
    frame: &mut ChildSpawnerCommands,
    f: &Handle<Font>,
    runes: &super::super::runes::Runes,
) {
    use super::super::runes::{ui as rune_ui, RuneAction};
    frame
        .spawn((
            Interaction::default(),
            bevy::ui::FocusPolicy::Block,
            BackgroundColor(Color::srgb(0.02, 0.015, 0.01)),
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(10.0)),
                overflow: Overflow::clip(),
                ..rect(8.0, 8.0, 408.0, 436.0)
            },
        ))
        .with_children(|page| {
            page.spawn(text(f, "Equipped rune", 15.0, GOLD));
            match runes.save.equipped_rune() {
                Some(rune) => rune_ui::rune_lines(page, f, runes, rune),
                None => {
                    page.spawn(text(f, "No rune equipped", 13.0, GREY));
                }
            }
            page.spawn(Node {
                height: Val::Px(8.0),
                ..default()
            });
            page.spawn(text(f, "Rune slots", 15.0, GOLD));
            for slot in 0..runes.save.slots.len() {
                let equipped = runes.save.equipped == Some(slot);
                page.spawn(Node {
                    width: Val::Percent(100.0),
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(8.0),
                    ..default()
                })
                .with_children(|row| match runes.save.rune(slot) {
                    Some(rune) => {
                        row.spawn(Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(6.0),
                            ..default()
                        })
                        .with_children(|name| {
                            rune_ui::rune_icon(name, runes, rune);
                            name.spawn(text(
                                f,
                                format!(
                                    "{}. {}{}",
                                    slot + 1,
                                    runes.name(rune),
                                    if equipped { " (equipped)" } else { "" }
                                ),
                                13.0,
                                Color::WHITE,
                            ));
                        });
                        skin::button(
                            row,
                            f,
                            if equipped { "Unequip" } else { "Equip" },
                            RuneAction::Equip(slot),
                            90.0,
                        );
                    }
                    None => {
                        row.spawn(text(f, format!("{}. Empty", slot + 1), 13.0, GREY));
                    }
                });
            }
            page.spawn(Node {
                flex_grow: 1.0,
                ..default()
            });
            page.spawn(text(
                f,
                "Win a campaign zone to earn runes. Hover an icon for its stat ranges.",
                11.0,
                GREY,
            ));
        });
}

/// Compact skill values; all final values come from the combat resolver.
pub(super) fn spell_stats(run: &Run, index: usize) -> String {
    let Some(owned) = run.abilities.get(index) else {
        return super::super::procs::views(run)
            .get(index.saturating_sub(run.abilities.len()))
            .map_or_else(
                || {
                    if run.abilities.is_empty() {
                        "-".into()
                    } else {
                        spell_stats(run, run.abilities.len() - 1)
                    }
                },
                |p| p.lines.join("\n"),
            );
    };
    let profile = crate::survivors::items::legendary::profile(run, owned.def);
    let def = &profile.0;
    let p = run.resolve_spell(owned.def, owned.level);
    let tags = p.tags;
    let mut increased = run.stats().might - 1.0;
    if tags.contains(Tags::DAMAGE) {
        increased += run.stats().level_damage;
    }
    let (mut more, mut recovery, mut speed, mut flat) = (1.0, 0.0, 0.0, 0.0);
    for modifier in run
        .spell_modifiers
        .entries()
        .iter()
        .filter(|m| m.filter.matches(def.key, tags))
    {
        match modifier.change {
            Change::IncreasedEffect(v) => increased += v,
            Change::IncreasedDamage(v) if tags.contains(Tags::DAMAGE) => increased += v,
            Change::MoreDamage(v) if tags.contains(Tags::DAMAGE) => more *= (1.0 + v).max(0.0),
            Change::IncreasedCooldownRecovery(v) => recovery += v,
            Change::IncreasedAttackSpeed(v) if tags.contains(Tags::ATTACK) => speed += v,
            Change::FlatDamage(v) if tags.contains(Tags::DAMAGE) && def.damage > 0.0 => flat += v,
            Change::FlatSpellDamage(v)
                if tags.contains(Tags::SPELL | Tags::DAMAGE) && def.damage > 0.0 =>
            {
                flat += v
            }
            _ => {}
        }
    }
    let power = match def.kind {
        data::Kind::Heal => "Healing",
        data::Kind::Shield => "Absorption",
        _ => "Damage",
    };
    let bonus_label = match def.kind {
        data::Kind::Heal => "Healing bonus",
        data::Kind::Shield => "Absorption bonus",
        _ => "Damage bonus",
    };
    let low = run
        .resolve_spell_with_weapon_roll(def, owned.level, 0.0)
        .dmg;
    let high = run
        .resolve_spell_with_weapon_roll(def, owned.level, 1.0)
        .dmg;
    let mut rows = vec![
        if high > low {
            format!("{power}: {low:.1}-{high:.1}")
        } else {
            format!("{power}: {:.1}", p.dmg)
        },
        format!("{bonus_label}: {:+.0}%", increased * 100.0),
    ];
    if flat > 0.0 {
        rows.push(format!("Flat damage: +{flat:.0}"));
    }
    if more != 1.0 {
        rows.push(format!("More damage: {:+.0}%", (more - 1.0) * 100.0));
    }
    let crit =
        tags.contains(Tags::DAMAGE) && !tags.contains(Tags::GUARD) && def.kind != data::Kind::Dot;
    if crit {
        rows.push(format!("Critical chance: {:.1}%", p.crit * 100.0));
        rows.push(format!(
            "Critical damage: {:.0}%",
            p.crit_multiplier * 100.0
        ));
    }
    rows.push(format!("Cooldown: {:.2} s", p.cooldown));
    rows.push(format!(
        "Cooldown reduction: {:.1}%",
        if def.cooldown > 0.0 {
            (1.0 - p.cooldown / def.cooldown) * 100.0
        } else {
            0.0
        }
    ));
    rows.push(format!("Cooldown recovery: {:+.1}%", recovery * 100.0));
    if tags.contains(Tags::ATTACK) {
        rows.push(format!("Attack speed: {:+.1}%", speed * 100.0));
    }
    if p.dot > 0.0 {
        rows.push(format!("Damage over time: {:.1}/s", p.dot));
    }
    rows.push(format!("Targets: {}", p.count));
    if p.range > 0.0 {
        rows.push(format!("Range: {:.1} yd", p.range));
    }
    if tags.contains(Tags::AREA) {
        rows.push(format!("Radius: {:.1} yd", p.radius));
    }
    if p.duration > 0.0 {
        rows.push(format!("Duration: {:.1} s", p.duration));
    }
    if p.chains > 0 {
        rows.push(format!("Chains: {}", p.chains));
    }
    if p.forks > 0 {
        rows.push(format!("Forks: {}", p.forks));
    }
    if p.recast > 0.0 {
        rows.push(format!("Repeat chance: {:.0}%", p.recast * 100.0));
    }
    let hit = super::super::spell_engine::on_hit::modifiers(def.key, run.spell_modifiers.entries());
    for (label, value) in [
        ("Life leech", hit.leech),
        ("Armour penetration", hit.penetration.clamp(0.0, 1.0)),
        ("Boss damage", hit.boss),
        ("Full-health damage", hit.full),
        ("Low-health damage", hit.low),
        ("Bleed chance", hit.bleed.clamp(0.0, 1.0)),
        ("Ignite chance", hit.ignite.clamp(0.0, 1.0)),
        ("Chill chance", hit.chill.clamp(0.0, 1.0)),
        ("Knockback chance", hit.knockback.clamp(0.0, 1.0)),
    ] {
        if value > 0.0 {
            rows.push(format!("{label}: {:.1}%", value * 100.0));
        }
    }
    if let Some(buff) = super::super::healer_buffs::tooltip(run, owned.def, owned.level) {
        rows.push(buff);
    }
    rows.join("\n")
}
fn refresh_character_xp(
    account: Res<talents::Account>,
    mut labels: Query<&mut Text, With<CharacterXpLabel>>,
    mut fills: Query<&mut Node, With<CharacterXpFill>>,
) {
    let level = talents::level(account.xp);
    let (label, percent) = if level >= 60 {
        ("Level 60 - Maximum XP".to_owned(), 100.0)
    } else {
        let xp = account.xp.saturating_sub(talents::threshold(level));
        let needed = talents::threshold(level + 1) - talents::threshold(level);
        (format!("Level {level} - {xp} / {needed} XP"), (xp as f64 / needed as f64 * 100.0) as f32)
    };
    for mut text in &mut labels {
        if text.0 != label { text.0 = label.clone(); }
    }
    for mut node in &mut fills { node.width = Val::Percent(percent); }
}

fn refresh_paragon(
    account: Res<talents::Account>,
    mut labels: Query<&mut Text, With<ParagonLabel>>,
    mut fills: Query<&mut Node, With<ParagonFill>>,
) {
    let active = talents::level(account.xp) == 60;
    let progress = super::super::paragon::progress(account.xp);
    let label = if active {
        format!(
            "Paragon {} - {} / {} XP",
            progress.level, progress.xp, progress.needed
        )
    } else {
        "Paragon - Available at level 60".to_owned()
    };
    for mut text in &mut labels {
        if text.0 != label {
            text.0 = label.clone();
        }
    }
    for mut node in &mut fills {
        node.width = Val::Percent(if active {
            (progress.xp as f64 / progress.needed as f64 * 100.0) as f32
        } else {
            0.0
        });
    }
}

#[allow(clippy::type_complexity)]
fn refresh(
    run: Res<Run>,
    view: Res<View>,
    roster: Res<super::super::characters::Roster>,
    account: Res<talents::Account>,
    spells: Option<Res<crate::ui_action::Spells>>,
    mut texts: ParamSet<(
        Query<(&StatsText, &mut Text)>,
        Query<(&stats::CategoryText, &mut Text)>,
        Query<&mut Text, With<CharacterTitle>>,
        Query<&mut Text, With<SkillTitle>>,
        Query<&mut Text, With<SlotHint>>,
        Query<&mut Text, With<EquipmentLevel>>,
    )>,
    slots: Query<(&Interaction, &Slot)>,
    inventory: Option<Res<super::super::items::Inventory>>,
) {
    if texts.p2().is_empty() {
        return;
    }
    let procs = super::super::procs::views(&run);
    let index = view
        .selected
        .min((run.abilities.len() + procs.len()).saturating_sub(1));
    let details = spell_stats(&run, index);
    let details: Vec<_> = details.lines().collect();
    for (line, mut text) in &mut texts.p0() {
        let value = details.get(line.0).copied().unwrap_or_default();
        if text.0 != value {
            text.0 = value.to_owned();
        }
    }
    let categories: Vec<Vec<String>> = (0..5)
        .map(|category| {
            stats::category_text(category, &run, inventory.as_deref(), index)
                .lines()
                .map(str::to_owned)
                .collect()
        })
        .collect();
    for (category, mut text) in &mut texts.p1() {
        let value = categories
            .get(usize::from(category.0))
            .and_then(|rows| rows.get(category.1))
            .map_or("", String::as_str);
        let value = if let Some((label, number)) = value.split_once(':') {
            if category.2 {
                number.trim().to_owned()
            } else {
                format!("{label}:")
            }
        } else if category.2 {
            String::new()
        } else {
            value.to_owned()
        };
        if text.0 != value {
            text.0 = value;
        }
    }
    let title = format!(
        "{}\nNiveau {} - {}",
        roster.current().map_or("Adventurer", |c| c.name.as_str()),
        talents::level(account.xp),
        run.class.name
    );
    for mut text in &mut texts.p2() {
        if text.0 != title {
            text.0 = title.clone();
        }
    }
    let title = run.abilities.get(index).map_or_else(
        || {
            procs
                .get(index.saturating_sub(run.abilities.len()))
                .map_or_else(|| "No spell".to_owned(), |p| p.name.clone())
        },
        |owned| hud::spell_name(spells.as_deref(), owned.def.spell, owned.def.key),
    );
    for mut text in &mut texts.p3() {
        if text.0 != title {
            text.0 = title.clone();
        }
    }
    let equipped: Vec<_> = inventory
        .as_deref()
        .map(|inventory| {
            super::super::items::SLOTS
                .iter()
                .filter_map(|(slot, _)| inventory.equipped(slot))
                .collect()
        })
        .unwrap_or_default();
    let average = if equipped.is_empty() {
        0.0
    } else {
        equipped
            .iter()
            .map(|item| f64::from(item.level))
            .sum::<f64>()
            / equipped.len() as f64
    };
    for mut text in &mut texts.p5() {
        text.0 = format!("Average equipped item level: {average:.1}");
    }
    let hint = slots
        .iter()
        .find(|(i, _)| **i != Interaction::None)
        .map_or_else(
            || {
                format!(
                    "Equipment: {} / 19",
                    inventory.as_ref().map_or(0, |i| super::super::items::SLOTS
                        .iter()
                        .filter(|(name, _)| i.equipped(name).is_some())
                        .count())
                )
            },
            |(_, slot)| {
                inventory
                    .as_ref()
                    .and_then(|i| i.equipped(slot.0))
                    .map_or_else(|| format!("{} - empty", slot.0), |i| i.name.clone())
            },
        );
    for mut text in &mut texts.p4() {
        if text.0 != hint {
            text.0 = hint.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::{
        spell_engine::{Filter, Modifier},
        Owned,
    };
    use super::*;
    fn run() -> Run {
        Run {
            class: &data::CLASSES[6],
            abilities: vec![
                Owned {
                    def: data::ability("fireball"),
                    level: 1,
                    cd: 0.0,
                },
                Owned {
                    def: data::ability("heroic_strike"),
                    level: 1,
                    cd: 0.0,
                },
            ],
            ..default()
        }
    }
    #[test]
    fn character_sheet_paragon_bar_unlocks_at_sixty_and_updates_live() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        world.insert_resource(talents::Account::fresh());
        let label = world.spawn((ParagonLabel, Text::new(""))).id();
        let fill = world.spawn((ParagonFill, Node::default())).id();
        let character_label = world.spawn((CharacterXpLabel, Text::new(""))).id();
        let character_fill = world.spawn((CharacterXpFill, Node::default())).id();
        world.resource_mut::<talents::Account>().xp = talents::threshold(10) + 250;
        world.run_system_once(refresh_character_xp).unwrap();
        assert_eq!(world.get::<Text>(character_label).unwrap().0, "Level 10 - 250 / 1000 XP");
        assert_eq!(world.get::<Node>(character_fill).unwrap().width, Val::Percent(25.0));
        world.resource_mut::<talents::Account>().xp = 0;
        world.run_system_once(refresh_character_xp).unwrap();
        assert_eq!(world.get::<Node>(character_fill).unwrap().width, Val::Percent(0.0));
        world.run_system_once(refresh_paragon).unwrap();
        assert!(world.get::<Text>(label).unwrap().0.contains("level 60"));
        world.resource_mut::<talents::Account>().xp = talents::threshold(60) + 125;
        world.run_system_once(refresh_paragon).unwrap();
        assert!(world.get::<Text>(label).unwrap().0.contains("Paragon 1"));
        assert!(world.get::<Text>(label).unwrap().0.contains("25 / 200 XP"));
        assert_eq!(world.get::<Node>(fill).unwrap().width, Val::Percent(12.5));
        world.run_system_once(refresh_character_xp).unwrap();
        assert_eq!(world.get::<Node>(character_fill).unwrap().width, Val::Percent(100.0));
        assert!(world.get::<Text>(character_label).unwrap().0.contains("Maximum"));
    }
    #[test]
    fn learned_proc_icons_are_selectable_in_spells_and_show_the_same_live_values_as_the_hud() {
        let mut app = App::new();
        let mut run = run();
        run.class = &data::CLASSES[0];
        run.talent_build[1][8] = 1;
        run.talent_build[1][12] = 1;
        super::super::super::talent_runtime::configure(&mut run);
        let first_proc = run.abilities.len();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<Phase>()
            .insert_resource(run)
            .insert_resource(Selection {
                sheet_open: true,
                ..default()
            })
            .init_resource::<View>()
            .init_resource::<preview::Target>()
            .init_resource::<super::super::super::characters::Roster>()
            .init_resource::<talents::Account>()
            .insert_resource(hud::HudFont(Handle::default()));
        app.world_mut().resource_mut::<View>().stats_collapsed &= !(1 << 5);
        plugin(&mut app);
        app.world_mut()
            .resource_mut::<NextState<Phase>>()
            .set(Phase::Hub);
        app.update();
        app.world_mut().resource_mut::<View>().selected = first_proc + 1;
        app.update();
        assert_eq!(
            app.world_mut()
                .query_filtered::<&Text, With<SkillTitle>>()
                .single(app.world())
                .unwrap()
                .0,
            "Bloodbath"
        );
        app.world_mut().resource_mut::<View>().selected = first_proc;
        app.update();
        assert_eq!(
            app.world_mut()
                .query_filtered::<&Text, With<SkillTitle>>()
                .single(app.world())
                .unwrap()
                .0,
            "Deep Wounds"
        );
        let expected = super::super::super::procs::views(app.world().resource::<Run>())
            .into_iter()
            .find(|p| p.key == "proc_bleed")
            .unwrap()
            .lines
            .join("\n");
        assert_eq!(
            spell_stats(app.world().resource::<Run>(), first_proc),
            expected
        );
        let count = app
            .world_mut()
            .query::<&Click>()
            .iter(app.world())
            .filter(|c| matches!(c, Click::Skill(_)))
            .count();
        assert_eq!(count, first_proc + 2);
        app.world_mut()
            .resource_mut::<Run>()
            .replace_spell_modifiers(
                999,
                [Modifier {
                    source: 999,
                    filter: Filter::default(),
                    change: Change::IncreasedDotDamage(1.0),
                }],
            );
        app.update();
        assert_ne!(
            spell_stats(app.world().resource::<Run>(), first_proc),
            expected
        );
        app.world_mut().resource_mut::<Run>().talent_build = [[0; 19]; 3];
        app.update();
        let count = app
            .world_mut()
            .query::<&Click>()
            .iter(app.world())
            .filter(|c| matches!(c, Click::Skill(_)))
            .count();
        assert_eq!(count, first_proc);
    }
    #[test]
    fn sheet_uses_tag_filtered_bonuses_and_the_resolved_critical_multiplier() {
        let mut run = run();
        run.replace_spell_modifiers(
            5,
            [
                Change::IncreasedDamage(0.20),
                Change::CriticalMultiplier(0.40),
                Change::IncreasedCooldownRecovery(0.50),
                Change::RecastChance(1.30),
            ]
            .map(|change| Modifier {
                source: 5,
                filter: Filter::requiring(Tags::FIRE),
                change,
            }),
        );
        let fire = spell_stats(&run, 0);
        assert!(fire.contains("Damage bonus: +20%"));
        assert!(fire.contains("Critical damage: 240%"));
        assert!(fire.contains("Cooldown recovery: +50.0%"));
        assert!(fire.contains("Repeat chance: 130%"));
        let melee = spell_stats(&run, 1);
        assert!(melee.contains("Damage bonus: +0%"));
        assert!(melee.contains("Critical damage: 200%"));
    }
    #[test]
    fn paper_doll_has_nineteen_empty_slots_and_refreshes_after_a_modifier_changes() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<Phase>()
            .insert_resource(run())
            .insert_resource(Selection {
                sheet_open: true,
                ..default()
            })
            .init_resource::<View>()
            .init_resource::<preview::Target>()
            .init_resource::<super::super::super::characters::Roster>()
            .init_resource::<talents::Account>()
            .insert_resource(hud::HudFont(Handle::default()));
        app.world_mut().resource_mut::<View>().stats_collapsed &= !(1 << 5);
        plugin(&mut app);
        app.world_mut()
            .resource_mut::<NextState<Phase>>()
            .set(Phase::Hub);
        app.update();
        {
            let world = app.world_mut();
            assert_eq!(world.query::<&Slot>().iter(world).count(), 19);
            assert_eq!(world.query::<&Root>().iter(world).count(), 1);
            assert!(world
                .query_filtered::<&Text, With<StatsText>>()
                .iter(world)
                .any(|text| text.0.contains("Critical damage: 200%")));
        }
        app.world_mut()
            .resource_mut::<Run>()
            .replace_spell_modifiers(
                5,
                [Modifier {
                    source: 5,
                    filter: Filter::default(),
                    change: Change::CriticalMultiplier(0.40),
                }],
            );
        app.update();
        {
            let world = app.world_mut();
            assert!(world
                .query_filtered::<&Text, With<StatsText>>()
                .iter(world)
                .any(|text| text.0.contains("Critical damage: 240%")));
            assert_eq!(world.query::<&Slot>().iter(world).count(), 19);
        }
        app.world_mut().resource_mut::<Selection>().sheet_open = false;
        app.update();
        let world = app.world_mut();
        assert_eq!(world.query::<&Root>().iter(world).count(), 0);
    }
}
