//! The Achievements and Specialty window, opened from the hub's launcher bar. Laid out like the
//! expedition picker: a sidebar of categories (or promotions) and a scrolling list of cards.
use super::super::achievements::{self, Achievement, Achievements, Category, Metric};
use super::super::combat_ui::skin;
use super::super::{data, hud, Phase, Selection};
use super::{cost, unlocked, Effect, Promotion, Specialty, Unlock, PROMOTIONS};
use bevy::prelude::*;

const MUTED: Color = Color::srgb(0.62, 0.61, 0.57);
const EDGE: Color = Color::srgb(0.29, 0.25, 0.17);
const DONE: Color = Color::srgb(0.45, 0.85, 0.42);
const LOCKED: Color = Color::srgb(0.78, 0.33, 0.27);
const CARD: Color = Color::srgb(0.065, 0.06, 0.045);
const CARD_DONE: Color = Color::srgb(0.05, 0.085, 0.045);
const PICKED: Color = Color::srgb(0.19, 0.14, 0.065);
const SP_TONE: Color = Color::srgb(0.55, 0.8, 1.0);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(in crate::survivors) enum Tab {
    #[default]
    Achievements,
    Specialty,
}

#[derive(Component, Clone)]
pub(in crate::survivors) enum Click {
    Open(Tab),
    Close,
    Tab(Tab),
    Category(Category),
    Group(Category, String),
    Promotion(usize),
    Buy(&'static str),
    Reset(usize),
}

#[derive(Resource)]
pub(in crate::survivors) struct View {
    pub open: bool,
    tab: Tab,
    category: Category,
    /// The sub-category shown; empty for the whole category.
    group: String,
    promotion: usize,
    scroll: f32,
    message: String,
    built: Option<(Tab, Category, String, usize, u64, u64, String)>,
}

impl Default for View {
    fn default() -> Self {
        Self {
            open: false,
            tab: Tab::Achievements,
            category: Category::General,
            group: String::new(),
            promotion: 0,
            scroll: 0.0,
            message: String::new(),
            built: None,
        }
    }
}

#[derive(Component)]
struct Root;
#[derive(Component)]
struct Scroll;

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

/// What an icon slot shows: a client icon file or a spell's own icon.
enum Icon {
    File(&'static str),
    Spell(u32),
}

fn category_icon(category: Category) -> &'static str {
    match category {
        Category::General => "Interface\\Icons\\INV_Misc_Book_09",
        Category::Expeditions => "Interface\\Icons\\INV_Misc_Map_01",
        Category::Spells => "Interface\\Icons\\Spell_Fire_Fireball02",
        Category::Equipment => "Interface\\Icons\\INV_Chest_Plate16",
        Category::Challenges => "Interface\\Icons\\Ability_Warrior_Challange",
        Category::Feats => "Interface\\Icons\\INV_Misc_Gem_Variety_01",
    }
}

fn challenge_icon(key: &str) -> &'static str {
    match key {
        "no_spells" => "Interface\\Icons\\Spell_Shadow_CurseOfTounges",
        "one_spell" => "Interface\\Icons\\Spell_Fire_Fireball02",
        "no_support" => "Interface\\Icons\\Spell_Shadow_SacrificialShield",
        "no_draft" => "Interface\\Icons\\INV_Misc_Ticket_Tarot_Stack_01",
        "no_cards" => "Interface\\Icons\\INV_Misc_Note_01",
        "common_only" => "Interface\\Icons\\INV_Misc_Rune_01",
        "no_passive" => "Interface\\Icons\\Spell_Shadow_UnholyStrength",
        "no_death" => "Interface\\Icons\\Spell_Holy_Resurrection",
        "solo" => "Interface\\Icons\\Ability_Hunter_SniperShot",
        "naked" => "Interface\\Icons\\Spell_Magic_PolymorphChicken",
        "pacifist" => "Interface\\Icons\\Spell_Holy_Restoration",
        _ => "Interface\\Icons\\INV_Misc_QuestionMark",
    }
}

fn achievement_icon(achievement: &Achievement) -> Icon {
    let id = achievement.id.as_str();
    if let Some(key) = id.strip_prefix("spell:").or_else(|| id.strip_prefix("spell_max:")) {
        return Icon::Spell(data::ability(key).spell);
    }
    if let Some(rest) = id.strip_prefix("challenge:") {
        return Icon::File(challenge_icon(rest.split(':').next().unwrap_or(rest)));
    }
    let prefix = id.split(':').next().unwrap_or(id);
    Icon::File(match prefix {
        "kills" => "Interface\\Icons\\Ability_Warrior_OffensiveStance",
        "bosses" => "Interface\\Icons\\INV_Misc_Head_Dragon_01",
        "wins" => "Interface\\Icons\\Spell_Holy_SealOfRighteousness",
        "runs" => "Interface\\Icons\\Ability_Hunter_Pathfinding",
        "deaths" => "Interface\\Icons\\INV_Misc_Bone_HumanSkull_01",
        "level" => "Interface\\Icons\\Spell_Holy_HolyBolt",
        "paragon" => "Interface\\Icons\\Spell_Holy_DivineSpirit",
        "gold" => "Interface\\Icons\\INV_Misc_Coin_01",
        "classes" => "Interface\\Icons\\Spell_Holy_MindVision",
        "coop" => "Interface\\Icons\\Ability_Warrior_BattleShout",
        "sp_spent" => "Interface\\Icons\\INV_Misc_Gem_Pearl_05",
        "map_wins" => "Interface\\Icons\\INV_Misc_Map_01",
        "map_kills" => "Interface\\Icons\\Ability_Whirlwind",
        "map_kills_run" | "kills_run" => "Interface\\Icons\\Spell_Fire_SelfDestruct",
        "boss_time" => "Interface\\Icons\\INV_Misc_PocketWatch_01",
        "campaign" => "Interface\\Icons\\Spell_Arcane_PortalIronForge",
        "spells_owned" => "Interface\\Icons\\INV_Misc_Book_11",
        "max_spells" => "Interface\\Icons\\Spell_Holy_InnerFire",
        "spell_families" => "Interface\\Icons\\Spell_Nature_Lightning",
        "slots" => match id {
            "slots:magic" => "Interface\\Icons\\INV_Jewelry_Ring_03",
            "slots:epic" => "Interface\\Icons\\INV_Chest_Plate16",
            _ => "Interface\\Icons\\INV_Crown_01",
        },
        "ilvl" => "Interface\\Icons\\INV_Helmet_01",
        "types" => "Interface\\Icons\\INV_Misc_Bag_10",
        "full_set" => "Interface\\Icons\\INV_Shield_06",
        "damage_run" => "Interface\\Icons\\Spell_Fire_Incinerate",
        "hit" => "Interface\\Icons\\INV_Misc_Bomb_04",
        "legendary_run" => "Interface\\Icons\\INV_Misc_Gift_01",
        "rerolls_run" => "Interface\\Icons\\INV_Misc_Ticket_Tarot_Stack_01",
        "banishes_run" => "Interface\\Icons\\Spell_Shadow_Teleport",
        "stored" => "Interface\\Icons\\INV_Misc_Bag_08",
        "close_call" => "Interface\\Icons\\Spell_Shadow_DeathScream",
        "boss_any" => "Interface\\Icons\\Ability_Rogue_Sprint",
        "challenges" => "Interface\\Icons\\Spell_Shadow_RitualOfSacrifice",
        _ => "Interface\\Icons\\INV_Misc_QuestionMark",
    })
}

fn effect_icon(effect: Effect) -> &'static str {
    use Effect::*;
    match effect {
        Damage => "Interface\\Icons\\INV_Sword_04",
        MaxHealth => "Interface\\Icons\\Spell_Holy_BlessingOfStrength",
        Movement => "Interface\\Icons\\Ability_Rogue_Sprint",
        Pickup => "Interface\\Icons\\INV_Misc_Bag_08",
        Experience => "Interface\\Icons\\Spell_Holy_HolyBolt",
        Armour => "Interface\\Icons\\Ability_Warrior_DefensiveStance",
        Regeneration => "Interface\\Icons\\Spell_Nature_Rejuvenation",
        CritChance => "Interface\\Icons\\Ability_CriticalStrike",
        CritDamage => "Interface\\Icons\\Ability_Rogue_Ambush",
        Cooldown => "Interface\\Icons\\Spell_Nature_TimeStop",
        Area => "Interface\\Icons\\Ability_Whirlwind",
        Duration => "Interface\\Icons\\INV_Misc_PocketWatch_01",
        AttackSpeed => "Interface\\Icons\\Ability_GhoulFrenzy",
        GoldFind => "Interface\\Icons\\INV_Misc_Coin_01",
        Rerolls => "Interface\\Icons\\INV_Misc_Ticket_Tarot_Stack_01",
        Resistance => "Interface\\Icons\\INV_Misc_Orb_05",
        BossDamage => "Interface\\Icons\\INV_Misc_Head_Dragon_01",
        LifeLeech => "Interface\\Icons\\Spell_Shadow_LifeDrain",
        Dodge => "Interface\\Icons\\Ability_Rogue_Feint",
        DamageReduction => "Interface\\Icons\\Spell_Holy_PowerWordShield",
        MagicFind => "Interface\\Icons\\INV_Misc_Gem_Pearl_05",
        Banishes => "Interface\\Icons\\Spell_Shadow_Teleport",
        Skips => "Interface\\Icons\\Spell_Arcane_Blink",
        ProjectileChance => "Interface\\Icons\\Ability_Marksmanship",
        CardLuck => "Interface\\Icons\\INV_Misc_Gift_01",
        ChainChance => "Interface\\Icons\\Spell_Nature_ChainLightning",
        RecastChance => "Interface\\Icons\\Spell_Arcane_StarFire",
        DotDamage => "Interface\\Icons\\Spell_Fire_FlameShock",
        ArmourPenetration => "Interface\\Icons\\Ability_Warrior_Sunder",
        MeleeTargetChance => "Interface\\Icons\\Ability_DualWield",
        MultiCritChance => "Interface\\Icons\\Ability_Hunter_SniperShot",
        FinalDamage => "Interface\\Icons\\Spell_Shadow_UnholyStrength",
        ForkChance => "Interface\\Icons\\Ability_UpgradeMoonGlaive",
        MultiCritChain => "Interface\\Icons\\Spell_Fire_SelfDestruct",
        MultiTarget => "Interface\\Icons\\Ability_DualWield",
        Haste => "Interface\\Icons\\Spell_Nature_TimeStop",
        LowLifeDamage => "Interface\\Icons\\Ability_Warrior_Revenge",
        FullLifeDamage => "Interface\\Icons\\Ability_Rogue_Ambush",
        IgniteChance => "Interface\\Icons\\Spell_Fire_Incinerate",
        BleedChance => "Interface\\Icons\\Ability_GhoulFrenzy",
        BuffEffect => "Interface\\Icons\\Spell_Holy_DivineSpirit",
        RuneUpgrade => "Interface\\Icons\\INV_Misc_Rune_01",
        RuneTier => "Interface\\Icons\\INV_Misc_Gem_Variety_01",
        CraftLuck => "Interface\\Icons\\INV_Shield_06",
        TalentPoints => "Interface\\Icons\\INV_Misc_Book_11",
        StartingCards => "Interface\\Icons\\INV_Misc_Ticket_Tarot_Stack_01",
        MultiCritUnlock => "Interface\\Icons\\Ability_Hunter_SniperShot",
        MultiCritPower => "Interface\\Icons\\Spell_Fire_SelfDestruct",
    }
}

fn promotion_icon(index: usize) -> &'static str {
    [
        "Interface\\Icons\\INV_Sword_04",
        "Interface\\Icons\\INV_Shield_06",
        "Interface\\Icons\\Spell_Holy_SealOfWisdom",
        "Interface\\Icons\\Spell_Holy_SealOfMight",
        "Interface\\Icons\\INV_Misc_Head_Dragon_01",
        "Interface\\Icons\\INV_Crown_01",
    ]
    .get(index)
    .copied()
    .unwrap_or("Interface\\Icons\\INV_Misc_QuestionMark")
}

/// A framed icon square; dimmed when `faded`.
fn icon(ui: &mut ChildSpawnerCommands, source: Icon, size: f32, border: Color, faded: bool) {
    ui.spawn((
        BorderColor::all(border),
        BackgroundColor(Color::BLACK),
        Node {
            width: Val::Px(size + 4.0),
            height: Val::Px(size + 4.0),
            padding: UiRect::all(Val::Px(1.0)),
            border: UiRect::all(Val::Px(1.0)),
            flex_shrink: 0.0,
            ..default()
        },
        bevy::ui::FocusPolicy::Pass,
    ))
    .with_children(|frame| {
        let node = Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        };
        let mut image = frame.spawn((
            ImageNode {
                color: if faded {
                    Color::srgb(0.45, 0.45, 0.45)
                } else {
                    Color::WHITE
                },
                ..default()
            },
            bevy::ui::FocusPolicy::Pass,
            node,
        ));
        match source {
            Icon::File(path) if faded => image.insert(hud::UiArt::item_grey(path)),
            Icon::File(path) => image.insert(hud::UiArt::item(path)),
            Icon::Spell(spell) => image.insert(hud::SpellIcon(spell)),
        };
    });
}

fn input(
    phase: Res<State<Phase>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut view: ResMut<View>,
    mut selection: ResMut<Selection>,
    mut specialty: ResMut<Specialty>,
    mut achievements: ResMut<Achievements>,
    clicks: Query<(&Interaction, &Click), Changed<Interaction>>,
    menu: Query<(&Interaction, &hud::UiAction), Changed<Interaction>>,
) {
    // Browsable from the hub and from the Escape menu.
    let reachable = matches!(*phase.get(), Phase::Hub | Phase::Paused);
    if !reachable {
        view.open = false;
    }
    if *phase.get() == Phase::Paused
        && menu
            .iter()
            .any(|(i, a)| *i == Interaction::Pressed && matches!(a, hud::UiAction::Achievements))
    {
        view.open = true;
        view.tab = Tab::Achievements;
        view.scroll = 0.0;
        view.message.clear();
    }
    for (interaction, click) in &clicks {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match click.clone() {
            Click::Open(tab) => {
                if view.open && view.tab == tab {
                    view.open = false;
                } else if reachable {
                    view.open = true;
                    view.tab = tab;
                    view.scroll = 0.0;
                    view.message.clear();
                    selection.sheet_open = false;
                    selection.bags_open = false;
                }
            }
            Click::Close => view.open = false,
            Click::Tab(tab) => {
                view.tab = tab;
                view.scroll = 0.0;
                view.message.clear();
            }
            Click::Category(category) => {
                view.category = category;
                view.group.clear();
                view.scroll = 0.0;
            }
            Click::Group(category, group) => {
                view.category = category;
                view.group = group;
                view.scroll = 0.0;
            }
            Click::Promotion(index) => {
                view.promotion = index.min(PROMOTIONS.len() - 1);
                view.scroll = 0.0;
                view.message.clear();
            }
            Click::Buy(id) => {
                view.message = match specialty.buy(id, &mut achievements) {
                    Ok(()) => String::new(),
                    Err(error) => error,
                };
            }
            Click::Reset(index) => {
                specialty.reset(&PROMOTIONS[index]);
                view.message = format!("{} refunded.", PROMOTIONS[index].name);
            }
        }
    }
    if view.open {
        // The window owns the mouse: no camera orbit or zoom behind it.
        selection.pointer_captured = true;
        selection.ui_captured = true;
        if keys.just_pressed(KeyCode::Escape) {
            view.open = false;
            keys.clear_just_pressed(KeyCode::Escape);
        }
    }
}

fn scroll(
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    mut frames: Query<(&ComputedNode, &mut ScrollPosition), With<Scroll>>,
    mut view: ResMut<View>,
) {
    if !view.open {
        return;
    }
    for event in wheel.read() {
        let dy = match event.unit {
            bevy::input::mouse::MouseScrollUnit::Line => event.y * 40.0,
            bevy::input::mouse::MouseScrollUnit::Pixel => event.y,
        };
        for (node, mut position) in &mut frames {
            position.0.y = (position.0.y - dy).clamp(
                0.0,
                ((node.content_size.y - node.size.y) * node.inverse_scale_factor).max(0.0),
            );
            view.scroll = position.0.y;
        }
    }
}

fn bar(ui: &mut ChildSpawnerCommands, share: f32, color: Color) {
    ui.spawn((
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
        BorderColor::all(Color::srgb(0.18, 0.16, 0.11)),
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(8.0),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
    ))
    .with_children(|bar| {
        bar.spawn((
            BackgroundColor(color),
            Node {
                width: Val::Percent(share.clamp(0.0, 1.0) * 100.0),
                height: Val::Percent(100.0),
                ..default()
            },
        ));
    });
}

/// Tier pips: filled for each tier reached.
fn pips(ui: &mut ChildSpawnerCommands, reached: usize, total: usize) {
    ui.spawn(Node {
        column_gap: Val::Px(3.0),
        align_items: AlignItems::Center,
        flex_shrink: 0.0,
        ..default()
    })
    .with_children(|row| {
        for tier in 0..total {
            row.spawn((
                BackgroundColor(if tier < reached {
                    hud::GOLD_TONE
                } else {
                    Color::srgba(0.0, 0.0, 0.0, 0.6)
                }),
                BorderColor::all(EDGE),
                Node {
                    width: Val::Px(9.0),
                    height: Val::Px(9.0),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
            ));
        }
    });
}

fn card(done: bool) -> impl Bundle {
    (
        BackgroundColor(if done { CARD_DONE } else { CARD }),
        BorderColor::all(if done {
            Color::srgb(0.25, 0.42, 0.2)
        } else {
            EDGE
        }),
        Node {
            width: Val::Percent(100.0),
            flex_shrink: 0.0,
            padding: UiRect::all(Val::Px(10.0)),
            border: UiRect::all(Val::Px(1.0)),
            column_gap: Val::Px(12.0),
            align_items: AlignItems::Center,
            ..default()
        },
    )
}

fn column(grow: bool) -> Node {
    Node {
        flex_direction: FlexDirection::Column,
        flex_grow: if grow { 1.0 } else { 0.0 },
        flex_shrink: if grow { 1.0 } else { 0.0 },
        min_width: Val::Px(0.0),
        row_gap: Val::Px(4.0),
        ..default()
    }
}

/// A small caption over a value, as the side column of a card shows it.
fn stat(ui: &mut ChildSpawnerCommands, font: &Handle<Font>, caption: &str, value: String, color: Color) {
    ui.spawn(Node {
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::End,
        ..default()
    })
    .with_children(|stat| {
        stat.spawn(text(font, caption, 10.0, MUTED));
        stat.spawn(text(font, value, 15.0, color));
    });
}

fn achievement_card(list: &mut ChildSpawnerCommands, font: &Handle<Font>, progress: &Achievements, achievement: &Achievement) {
    let unlocked = progress.unlocked(achievement);
    let tiers = achievement.tiers.len();
    let complete = unlocked >= tiers;
    let next = unlocked.min(tiers - 1);
    let target = achievement.tiers[next].target;
    let (shown, share) = match (&achievement.metric, progress.value(achievement)) {
        (Metric::Time(_), Some(best)) => (
            format!("Best {best:.1} s / {target} s"),
            if complete { 1.0 } else { (target as f64 / best.max(0.01)) as f32 },
        ),
        (Metric::Time(_), None) => (format!("No kill yet / {target} s"), 0.0),
        (metric, value) => {
            let value = value.unwrap_or(0.0) as u64;
            (
                format!(
                    "{} / {}",
                    achievements::format_target(metric, value.min(target)),
                    achievements::format_target(metric, target)
                ),
                if complete { 1.0 } else { value as f32 / target.max(1) as f32 },
            )
        }
    };
    let earned: f32 = achievement.tiers[..unlocked].iter().map(|t| t.bonus).sum();
    // Inside an expedition's group its name is already the heading.
    let name = achievement
        .name
        .strip_prefix(&format!("{}: ", achievement.group))
        .unwrap_or(&achievement.name)
        .to_owned();
    list.spawn(card(complete)).with_children(|row| {
        icon(row, achievement_icon(achievement), 40.0, if complete { DONE } else { EDGE }, unlocked == 0);
        row.spawn(column(true)).with_children(|body| {
            body.spawn(Node {
                column_gap: Val::Px(10.0),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|head| {
                head.spawn(text(font, name, 16.0, if complete { DONE } else { hud::GOLD_TONE }));
                if tiers > 1 {
                    pips(head, unlocked, tiers);
                }
            });
            body.spawn(text(
                font,
                if complete {
                    format!("Completed: {}", achievement.describe(tiers - 1))
                } else if tiers > 1 {
                    format!("Tier {}: {}", achievements::roman(next + 1), achievement.describe(next))
                } else {
                    achievement.describe(next)
                },
                12.0,
                Color::srgb(0.88, 0.86, 0.8),
            ));
            body.spawn(Node {
                column_gap: Val::Px(10.0),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|line| {
                line.spawn(Node {
                    flex_grow: 1.0,
                    ..default()
                })
                .with_children(|holder| bar(holder, share, if complete { DONE } else { hud::GOLD_TONE }));
                line.spawn((
                    text(font, if complete { "Done".to_owned() } else { shown }, 11.0, MUTED),
                    Node {
                        min_width: Val::Px(130.0),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    TextLayout::new_with_justify(Justify::Right),
                ));
            });
        });
        row.spawn((
            Node {
                width: Val::Px(110.0),
                flex_shrink: 0.0,
                row_gap: Val::Px(6.0),
                ..column(false)
            },
        ))
        .with_children(|side| {
            side.spawn(Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::End,
                row_gap: Val::Px(6.0),
                width: Val::Percent(100.0),
                ..default()
            })
            .with_children(|side| {
                if !complete {
                    stat(side, font, "NEXT TIER", format!("+{:.2}%", achievement.tiers[next].bonus * 100.0), SP_TONE);
                }
                stat(
                    side,
                    font,
                    "SP GAIN",
                    format!("+{:.2}%", earned * 100.0),
                    if earned > 0.0 { DONE } else { MUTED },
                );
            });
        });
    });
}

fn node_card(
    list: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    specialty: &Specialty,
    promotion: &Promotion,
    node: &super::trees::Node,
    open: bool,
    editable: bool,
) {
    let rank = specialty.rank(node.id);
    let maxed = rank >= node.ranks;
    list.spawn(card(maxed)).with_children(|row| {
        icon(row, Icon::File(effect_icon(node.effect)), 40.0, if maxed { DONE } else { EDGE }, rank == 0);
        row.spawn(column(true)).with_children(|body| {
            body.spawn(Node {
                column_gap: Val::Px(10.0),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|head| {
                head.spawn(text(font, node.name, 16.0, if maxed { DONE } else { hud::GOLD_TONE }));
                head.spawn(text(font, format!("Rank {}/{}", rank, node.ranks), 12.0, MUTED));
            });
            body.spawn(text(
                font,
                if node.ranks > 1 {
                    format!("{} per rank", node.effect.describe(node.value))
                } else {
                    node.effect.describe(node.value)
                },
                12.0,
                Color::srgb(0.88, 0.86, 0.8),
            ));
            bar(body, f32::from(rank) / f32::from(node.ranks), if maxed { DONE } else { hud::GOLD_TONE });
        });
        row.spawn(Node {
            width: Val::Px(150.0),
            flex_shrink: 0.0,
            ..column(false)
        })
        .with_children(|side| {
            side.spawn(Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::End,
                row_gap: Val::Px(6.0),
                width: Val::Percent(100.0),
                ..default()
            })
            .with_children(|side| {
                stat(
                    side,
                    font,
                    "CURRENT",
                    if rank == 0 {
                        "-".into()
                    } else {
                        node.effect.describe(node.value * f32::from(rank))
                    },
                    if rank == 0 { MUTED } else { DONE },
                );
                if maxed {
                    side.spawn(text(font, "MAXED", 13.0, DONE));
                } else {
                    let price = cost(promotion, node, rank);
                    let label = format!("{} SP", achievements::group(price));
                    if open && editable {
                        skin::button_sized(side, font, label, Click::Buy(node.id), 120.0, 30.0, 13.0, specialty.available() < price);
                    } else {
                        side.spawn(text(font, label, 13.0, MUTED));
                    }
                }
            });
        });
    });
}

/// A sidebar entry, styled like an expedition row.
#[allow(clippy::too_many_arguments)]
fn side_row(
    ui: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    click: Click,
    picture: Option<Icon>,
    label: String,
    detail: String,
    detail_color: Color,
    selected: bool,
    indent: bool,
) {
    ui.spawn((
        Button,
        click,
        BorderColor::all(if selected { hud::GOLD_TONE } else { EDGE }),
        BackgroundColor(if selected { PICKED } else { CARD }),
        Node {
            width: Val::Percent(100.0),
            flex_shrink: 0.0,
            padding: UiRect {
                left: Val::Px(if indent { 22.0 } else { 8.0 }),
                right: Val::Px(8.0),
                top: Val::Px(if indent { 4.0 } else { 6.0 }),
                bottom: Val::Px(if indent { 4.0 } else { 6.0 }),
            },
            border: UiRect::all(Val::Px(1.0)),
            column_gap: Val::Px(8.0),
            align_items: AlignItems::Center,
            ..default()
        },
    ))
    .with_children(|row| {
        if let Some(picture) = picture {
            icon(row, picture, 28.0, if selected { hud::GOLD_TONE } else { EDGE }, false);
        }
        row.spawn((
            text(
                font,
                label,
                if indent { 12.0 } else { 14.0 },
                if selected { hud::GOLD_TONE } else { Color::WHITE },
            ),
            Node {
                flex_grow: 1.0,
                flex_shrink: 1.0,
                min_width: Val::Px(0.0),
                ..default()
            },
            bevy::ui::FocusPolicy::Pass,
        ));
        row.spawn((
            text(font, detail, 11.0, detail_color),
            Node {
                flex_shrink: 0.0,
                ..default()
            },
            bevy::ui::FocusPolicy::Pass,
        ));
    });
}

fn tally<'a>(progress: &Achievements, list: impl Iterator<Item = &'a Achievement>) -> (usize, usize) {
    list.fold((0, 0), |(d, t), a| (d + progress.unlocked(a), t + a.tiers.len()))
}

/// The category's sub-categories, in catalog order.
fn groups(category: Category) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for achievement in achievements::all().iter().filter(|a| a.category == category) {
        if !out.contains(&achievement.group.as_str()) {
            out.push(achievement.group.as_str());
        }
    }
    out
}

fn achievement_side(side: &mut ChildSpawnerCommands, font: &Handle<Font>, progress: &Achievements, view: &View) {
    for category in Category::ALL {
        let (done, total) = tally(progress, achievements::all().iter().filter(|a| a.category == category));
        let current = view.category == category;
        side_row(
            side,
            font,
            Click::Category(category),
            Some(Icon::File(category_icon(category))),
            category.name().to_owned(),
            format!("{done}/{total}"),
            if done == total { DONE } else { MUTED },
            current && view.group.is_empty(),
            false,
        );
        if !current {
            continue;
        }
        for group in groups(category) {
            let (done, total) = tally(
                progress,
                achievements::all().iter().filter(|a| a.category == category && a.group == group),
            );
            side_row(
                side,
                font,
                Click::Group(category, group.to_owned()),
                None,
                group.to_owned(),
                format!("{done}/{total}"),
                if done == total { DONE } else { MUTED },
                view.group == group,
                true,
            );
        }
    }
}

fn promotion_spent(specialty: &Specialty, promotion: &Promotion) -> u64 {
    promotion
        .nodes
        .iter()
        .map(|node| (0..specialty.rank(node.id)).map(|r| cost(promotion, node, r)).sum::<u64>())
        .sum()
}

fn specialty_side(
    side: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    specialty: &Specialty,
    progress: &Achievements,
    view: &View,
) {
    for (index, promotion) in PROMOTIONS.iter().enumerate() {
        let open = unlocked(promotion, progress);
        let ranks: u32 = promotion.nodes.iter().map(|n| u32::from(specialty.rank(n.id))).sum();
        let max: u32 = promotion.nodes.iter().map(|n| u32::from(n.ranks)).sum();
        side_row(
            side,
            font,
            Click::Promotion(index),
            Some(Icon::File(promotion_icon(index))),
            promotion.name.to_owned(),
            if open { format!("{ranks}/{max}") } else { "LOCKED".into() },
            if !open {
                LOCKED
            } else if ranks == max {
                DONE
            } else {
                MUTED
            },
            view.promotion == index,
            false,
        );
    }
}

/// The header's summary chips: a caption over a large value.
fn chip(ui: &mut ChildSpawnerCommands, font: &Handle<Font>, caption: &str, value: String, color: Color) {
    ui.spawn((
        BorderColor::all(EDGE),
        BackgroundColor(Color::srgb(0.06, 0.05, 0.035)),
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: UiRect::axes(Val::Px(14.0), Val::Px(6.0)),
            border: UiRect::all(Val::Px(1.0)),
            min_width: Val::Px(120.0),
            flex_shrink: 0.0,
            ..default()
        },
    ))
    .with_children(|chip| {
        chip.spawn(text(font, caption, 10.0, MUTED));
        chip.spawn(text(font, value, 18.0, color));
    });
}

#[allow(clippy::too_many_arguments)]
fn render(
    mut commands: Commands,
    font: Option<Res<hud::HudFont>>,
    mut view: ResMut<View>,
    phase: Res<State<Phase>>,
    selection: Res<Selection>,
    specialty: Res<Specialty>,
    progress: Res<Achievements>,
    roots: Query<Entity, With<Root>>,
) {
    let key = (
        view.tab,
        view.category,
        view.group.clone(),
        view.promotion,
        progress.revision,
        specialty.revision,
        view.message.clone(),
    );
    if !view.open {
        if view.built.take().is_some() {
            for root in &roots {
                commands.entity(root).despawn();
            }
        }
        return;
    }
    if view.built.as_ref() == Some(&key) {
        return;
    }
    let Some(font) = font else {
        return;
    };
    view.built = Some(key);
    for root in &roots {
        commands.entity(root).despawn();
    }
    let f = &font.0;
    let view = &*view;
    // Ranks are bought between expeditions only, not from a run's pause menu.
    let editable = *phase.get() == Phase::Hub || (*phase.get() == Phase::Paused && selection.in_hub);
    let bonus = progress.bonus();
    let rate = super::SP_PER_XP * f64::from(1.0 + bonus);
    commands
        .spawn((
            Root,
            Button,
            GlobalZIndex(760),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.78)),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        width: Val::Px(1040.0),
                        max_width: Val::Percent(96.0),
                        height: Val::Px(700.0),
                        max_height: Val::Percent(90.0),
                        padding: UiRect::all(Val::Px(20.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(12.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.035, 0.03, 0.02)),
                ))
                .with_children(|panel| {
                    skin::backdrop(panel);
                    panel
                        .spawn(Node {
                            position_type: PositionType::Absolute,
                            right: Val::Px(14.0),
                            top: Val::Px(12.0),
                            width: Val::Px(28.0),
                            height: Val::Px(28.0),
                            ..default()
                        })
                        .with_children(|corner| skin::close(corner, Click::Close, 0.0, 0.0));
                    // Header: title, tabs and the totals.
                    panel
                        .spawn(Node {
                            column_gap: Val::Px(14.0),
                            align_items: AlignItems::Center,
                            flex_shrink: 0.0,
                            padding: UiRect::right(Val::Px(36.0)),
                            ..default()
                        })
                        .with_children(|head| {
                            head.spawn(Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(4.0),
                                ..default()
                            })
                            .with_children(|title| {
                                title.spawn(text(f, "Progression", 26.0, hud::GOLD_TONE));
                                title.spawn(text(
                                    f,
                                    match view.tab {
                                        Tab::Achievements => "Every tier reached raises your specialty point gain",
                                        Tab::Specialty => "Spend specialty points on bonuses for every expedition",
                                    },
                                    12.0,
                                    MUTED,
                                ));
                            });
                            head.spawn(Node {
                                flex_grow: 1.0,
                                ..default()
                            });
                            let (done, total) = progress.totals();
                            chip(head, f, "ACHIEVEMENTS", format!("{done} / {total}"), hud::GOLD_TONE);
                            chip(head, f, "SP GAIN", format!("x{:.3}", 1.0 + bonus), DONE);
                            chip(head, f, "SPECIALTY POINTS", achievements::group(specialty.available()), SP_TONE);
                        });
                    panel
                        .spawn(Node {
                            column_gap: Val::Px(6.0),
                            flex_shrink: 0.0,
                            ..default()
                        })
                        .with_children(|tabs| {
                            for (label, tab) in [("Achievements", Tab::Achievements), ("Specialty", Tab::Specialty)] {
                                skin::button_sized(tabs, f, label, Click::Tab(tab), 170.0, 32.0, 14.0, view.tab == tab);
                            }
                        });
                    // Body: the sidebar and the list.
                    panel
                        .spawn(Node {
                            flex_grow: 1.0,
                            min_height: Val::Px(0.0),
                            column_gap: Val::Px(14.0),
                            ..default()
                        })
                        .with_children(|body| {
                            body.spawn((
                                Interaction::default(),
                                Node {
                                    width: Val::Px(250.0),
                                    flex_shrink: 0.0,
                                    flex_direction: FlexDirection::Column,
                                    row_gap: Val::Px(4.0),
                                    overflow: Overflow::clip_y(),
                                    ..default()
                                },
                            ))
                            .with_children(|side| match view.tab {
                                Tab::Achievements => achievement_side(side, f, &progress, view),
                                Tab::Specialty => specialty_side(side, f, &specialty, &progress, view),
                            });
                            body.spawn(Node {
                                flex_grow: 1.0,
                                min_width: Val::Px(0.0),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(8.0),
                                ..default()
                            })
                            .with_children(|main| {
                                list_header(main, f, view, &specialty, &progress, editable);
                                main.spawn((
                                    Scroll,
                                    Interaction::default(),
                                    ScrollPosition(Vec2::new(0.0, view.scroll)),
                                    Node {
                                        flex_grow: 1.0,
                                        min_height: Val::Px(0.0),
                                        overflow: Overflow::scroll_y(),
                                        flex_direction: FlexDirection::Column,
                                        row_gap: Val::Px(6.0),
                                        padding: UiRect::right(Val::Px(6.0)),
                                        ..default()
                                    },
                                ))
                                .with_children(|list| match view.tab {
                                    Tab::Achievements => {
                                        for achievement in achievements::all().iter().filter(|a| {
                                            a.category == view.category
                                                && (view.group.is_empty() || a.group == view.group)
                                        }) {
                                            achievement_card(list, f, &progress, achievement);
                                        }
                                    }
                                    Tab::Specialty => {
                                        let promotion = &PROMOTIONS[view.promotion];
                                        let open = unlocked(promotion, &progress);
                                        for node in promotion.nodes {
                                            node_card(list, f, &specialty, promotion, node, open, editable);
                                        }
                                    }
                                });
                            });
                        });
                    panel
                        .spawn(Node {
                            column_gap: Val::Px(12.0),
                            align_items: AlignItems::Center,
                            flex_shrink: 0.0,
                            ..default()
                        })
                        .with_children(|foot| {
                            foot.spawn(text(
                                f,
                                if view.message.is_empty() {
                                    format!(
                                        "1 XP = {rate:.3} SP  |  {} earned, {} spent  |  Scroll to browse, Esc closes",
                                        achievements::group(specialty.earned()),
                                        achievements::group(specialty.spent()),
                                    )
                                } else {
                                    view.message.clone()
                                },
                                12.0,
                                if view.message.is_empty() { MUTED } else { LOCKED },
                            ));
                        });
                });
        });
}

/// The line over the list: the shown group, or the promotion's unlock and refund.
fn list_header(
    ui: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    view: &View,
    specialty: &Specialty,
    progress: &Achievements,
    editable: bool,
) {
    ui.spawn((
        BorderColor::all(EDGE),
        BackgroundColor(Color::srgb(0.06, 0.05, 0.035)),
        Node {
            flex_shrink: 0.0,
            padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
            border: UiRect::all(Val::Px(1.0)),
            column_gap: Val::Px(12.0),
            align_items: AlignItems::Center,
            ..default()
        },
    ))
    .with_children(|head| match view.tab {
        Tab::Achievements => {
            let title = if view.group.is_empty() {
                view.category.name().to_owned()
            } else {
                format!("{}  >  {}", view.category.name(), view.group)
            };
            let (done, total) = tally(
                progress,
                achievements::all().iter().filter(|a| {
                    a.category == view.category && (view.group.is_empty() || a.group == view.group)
                }),
            );
            head.spawn((
                text(font, title, 17.0, hud::GOLD_TONE),
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
            ));
            head.spawn(text(font, format!("{done} / {total} tiers"), 12.0, MUTED));
        }
        Tab::Specialty => {
            let promotion = &PROMOTIONS[view.promotion];
            let open = unlocked(promotion, progress);
            head.spawn(Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                row_gap: Val::Px(2.0),
                ..default()
            })
            .with_children(|title| {
                title.spawn(text(font, promotion.name, 17.0, if open { hud::GOLD_TONE } else { LOCKED }));
                title.spawn(text(
                    font,
                    match promotion.unlock {
                        Unlock::Always => "Available from the start".to_owned(),
                        Unlock::Campaign if open => "Unlocked: every campaign expedition completed".to_owned(),
                        Unlock::Campaign => "Locked: complete every campaign expedition".to_owned(),
                        Unlock::Future(text) => format!("Locked: {text}"),
                    },
                    11.0,
                    if open { MUTED } else { LOCKED },
                ));
            });
            let spent = promotion_spent(specialty, promotion);
            head.spawn(text(font, format!("{} SP invested", achievements::group(spent)), 12.0, MUTED));
            if editable && spent > 0 {
                skin::button_sized(head, font, "Refund", Click::Reset(view.promotion), 90.0, 26.0, 12.0, false);
            }
        }
    });
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<View>().add_systems(
        Update,
        (
            input
                .after(super::super::combat_ui::input)
                .before(hud::keys)
                .before(benilla_world::schedule::WorldStage::Input),
            scroll,
            render,
        )
            .chain(),
    );
}

/// The achievement alert, as in WoW: a framed banner with the icon drops in from the top with a
/// fanfare and a sweep of light, holds, then fades.
#[derive(Component)]
struct Banner;
#[derive(Component)]
struct Shine;
/// A banner part's own colour, faded with the banner.
#[derive(Component, Clone, Copy)]
enum Fade {
    Back(Color),
    Border(Color),
    Frame(Color, Color),
    Text(Color),
    Image(Color),
}

const FANFARE: &str = r"Sound\Interface\LevelUp.wav";
const SLIDE: f32 = 0.35;
const FADE: f32 = 0.6;

fn spawn_banner(commands: &mut Commands, font: &Handle<Font>, earned: &achievements::Earned) {
    let picture = achievements::all()
        .iter()
        .find(|a| a.id == earned.id)
        .map_or(Icon::File(r"Interface\Icons\INV_Misc_Gem_Variety_01"), achievement_icon);
    let back = Color::srgba(0.05, 0.04, 0.02, 0.96);
    commands
        .spawn((
            Banner,
            GlobalZIndex(900),
            bevy::ui::FocusPolicy::Pass,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(-120.0),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_children(|row| {
            row.spawn((
                Fade::Frame(back, hud::GOLD_TONE),
                BackgroundColor(back),
                BorderColor::all(hud::GOLD_TONE),
                Node {
                    padding: UiRect::axes(Val::Px(16.0), Val::Px(10.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    column_gap: Val::Px(14.0),
                    align_items: AlignItems::Center,
                    min_width: Val::Px(420.0),
                    max_width: Val::Px(640.0),
                    overflow: Overflow::clip(),
                    ..default()
                },
            ))
            .with_children(|card| {
                // The sweep of light, under the content.
                card.spawn((
                    Shine,
                    Fade::Back(Color::srgba(1.0, 0.85, 0.4, 0.22)),
                    BackgroundColor(Color::srgba(1.0, 0.85, 0.4, 0.22)),
                    bevy::ui::FocusPolicy::Pass,
                    Node {
                        position_type: PositionType::Absolute,
                        top: Val::Px(0.0),
                        bottom: Val::Px(0.0),
                        left: Val::Percent(-30.0),
                        width: Val::Percent(22.0),
                        ..default()
                    },
                ));
                card.spawn((
                    Fade::Border(hud::GOLD_TONE),
                    BorderColor::all(hud::GOLD_TONE),
                    BackgroundColor(Color::BLACK),
                    Node {
                        width: Val::Px(54.0),
                        height: Val::Px(54.0),
                        padding: UiRect::all(Val::Px(1.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        flex_shrink: 0.0,
                        ..default()
                    },
                ))
                .with_children(|frame| {
                    let mut image = frame.spawn((
                        Fade::Image(Color::WHITE),
                        ImageNode::default(),
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                    ));
                    match picture {
                        Icon::File(path) => image.insert(hud::UiArt::item(path)),
                        Icon::Spell(spell) => image.insert(hud::SpellIcon(spell)),
                    };
                });
                card.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(3.0),
                    min_width: Val::Px(0.0),
                    ..default()
                })
                .with_children(|body| {
                    let muted = Color::srgb(0.75, 0.72, 0.62);
                    for (value, size, color) in [
                        ("ACHIEVEMENT EARNED".to_owned(), 11.0, muted),
                        (earned.title.clone(), 20.0, hud::GOLD_TONE),
                        (earned.line.clone(), 12.0, Color::srgb(0.9, 0.88, 0.82)),
                    ] {
                        body.spawn((Fade::Text(color), text(font, value, size, color), TextShadow::default()));
                    }
                });
            });
        });
}

fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

#[allow(clippy::type_complexity)]
fn banner(
    mut commands: Commands,
    time: Res<Time<Real>>,
    font: Option<Res<hud::HudFont>>,
    mut progress: ResMut<Achievements>,
    mut shown: Local<Option<achievements::Earned>>,
    mut sound: MessageWriter<crate::sound::FileSound>,
    mut roots: Query<(Entity, &mut Node), (With<Banner>, Without<Shine>)>,
    mut shines: Query<&mut Node, (With<Shine>, Without<Banner>)>,
    mut parts: Query<(
        &Fade,
        Option<&mut BackgroundColor>,
        Option<&mut BorderColor>,
        Option<&mut TextColor>,
        Option<&mut ImageNode>,
    )>,
) {
    let current = progress.toast(time.delta_secs()).map(|(e, age)| (e.clone(), age));
    let Some((earned, age)) = current else {
        if shown.take().is_some() {
            for (root, _) in &roots {
                commands.entity(root).despawn();
            }
        }
        return;
    };
    if shown.as_ref() != Some(&earned) {
        let Some(font) = font else {
            return;
        };
        for (root, _) in &roots {
            commands.entity(root).despawn();
        }
        spawn_banner(&mut commands, &font.0, &earned);
        sound.write(crate::sound::FileSound(FANFARE));
        *shown = Some(earned);
        return;
    }
    let drop = ease_out(age / SLIDE);
    let alpha = ((achievements::TOAST_SECS - age) / FADE).clamp(0.0, 1.0) * (age / 0.15).clamp(0.0, 1.0);
    for (_, mut node) in &mut roots {
        node.top = Val::Px(-120.0 + 140.0 * drop);
    }
    // The light crosses the banner once, just after it lands, and again a little later.
    let sweep = |start: f32| ((age - start) / 0.7).clamp(0.0, 1.0);
    let pass = if age < 1.4 { sweep(SLIDE) } else { sweep(2.2) };
    for mut node in &mut shines {
        node.left = Val::Percent(-30.0 + 140.0 * pass);
    }
    for (fade, back, border, color, image) in &mut parts {
        match *fade {
            Fade::Back(base) => {
                if let Some(mut back) = back {
                    back.0 = base.with_alpha(base.alpha() * alpha);
                }
            }
            Fade::Border(base) => {
                if let Some(mut border) = border {
                    *border = BorderColor::all(base.with_alpha(base.alpha() * alpha));
                }
            }
            Fade::Frame(fill, edge) => {
                if let Some(mut back) = back {
                    back.0 = fill.with_alpha(fill.alpha() * alpha);
                }
                if let Some(mut border) = border {
                    *border = BorderColor::all(edge.with_alpha(edge.alpha() * alpha));
                }
            }
            Fade::Text(base) => {
                if let Some(mut color) = color {
                    color.0 = base.with_alpha(alpha);
                }
            }
            Fade::Image(base) => {
                if let Some(mut image) = image {
                    image.color = base.with_alpha(alpha);
                }
            }
        }
    }
}

pub(super) fn banner_plugin(app: &mut App) {
    app.add_message::<crate::sound::FileSound>()
        .add_systems(Update, banner);
}
