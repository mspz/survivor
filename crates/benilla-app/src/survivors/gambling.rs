//! Character-owned equipment gambling at the hub; payment and item delivery share one save.
use super::{
    combat_ui::skin,
    hero, hud,
    items::{Inventory, Item, SlotKind},
    multiplayer::Role,
    talents, units, Phase, Rng, Run, Selection,
};
use bevy::prelude::*;
const NPC_ENTRY: u32 = 9_000_001;
pub(super) const SLOTS: &[(SlotKind, &str)] = &[
    (SlotKind::Head, "Tête"),
    (SlotKind::Neck, "Cou"),
    (SlotKind::Shoulders, "Épaules"),
    (SlotKind::Back, "Cape"),
    (SlotKind::Chest, "Torse"),
    (SlotKind::Wrists, "Poignets"),
    (SlotKind::Hands, "Mains"),
    (SlotKind::Waist, "Taille"),
    (SlotKind::Legs, "Jambes"),
    (SlotKind::Feet, "Pieds"),
    (SlotKind::Ring, "Anneau"),
    (SlotKind::Trinket, "Bijou"),
    (SlotKind::MainHand, "Arme principale"),
    (SlotKind::OffHand, "Main gauche"),
    (SlotKind::Ranged, "Arme à distance"),
];
/// Diablo II-inspired slot hierarchy, adapted to copper and gently compounded at every level.
pub(super) fn price(slot: SlotKind, level: u32) -> u64 {
    let base = match slot {
        SlotKind::Wrists | SlotKind::Hands | SlotKind::Waist | SlotKind::Feet => 500,
        SlotKind::Back => 600,
        SlotKind::Head | SlotKind::Shoulders => 1000,
        SlotKind::Legs | SlotKind::OffHand => 1200,
        SlotKind::Chest => 1500,
        SlotKind::Ranged => 1800,
        SlotKind::MainHand => 2000,
        SlotKind::Ring => 2500,
        SlotKind::Neck => 3150,
        SlotKind::Trinket => 3500,
        SlotKind::Shirt | SlotKind::Tabard => 500,
    };
    (f64::from(base) * 1.04f64.powi(level.clamp(1, 60) as i32 - 1)).round() as u64
}
pub(super) fn position() -> Vec3 {
    // East of goldshireblacksmith.wmo (-9460.033, 94.203), facing the crossroads.
    units::wow_to_bevy([-9460.0, 80.0, 56.960_007])
}
fn near(pos: Vec3) -> bool {
    units::flat(pos - position()).length() <= 4.5
}
#[derive(Resource)]
struct View {
    page: usize,
    open: bool,
    slot: SlotKind,
    result: Option<Item>,
    notice: String,
    revision: u64,
}
impl Default for View {
    fn default() -> Self {
        Self {
            page: 0,
            open: false,
            slot: SlotKind::Head,
            result: None,
            notice: String::new(),
            revision: 0,
        }
    }
}
#[derive(Component)]
struct Gambler;
#[derive(Component)]
struct Root;
#[derive(Component)]
struct Tooltip;
#[derive(Component, Clone, Copy)]
enum Click {
    Open,
    Close,
    Slot(SlotKind),
    Page(i32),
}
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<View>()
        .add_systems(Update, register_name)
        .add_systems(Update, spawn.after(super::characters::arrive))
        .add_systems(
            Update,
            input
                .after(super::combat_ui::input)
                .before(super::client::input)
                .before(hud::keys),
        )
        .add_systems(Update, render.after(input))
        .add_systems(Update, tooltip.after(render));
}
fn register_name(mut names: Option<ResMut<crate::names::NameCache>>) {
    let Some(names) = names.as_deref_mut() else {
        return;
    };
    if names.creature_record(NPC_ENTRY).is_none() {
        names.insert_creature(
            NPC_ENTRY,
            Some(crate::names::CreatureRecord {
                name: "Gheed".into(),
                subname: Some("Paris d'équipement".into()),
                creature_type: 7,
                pet_family: 0,
                rank: 0,
                type_flags: 0,
                civilian: true,
                racial_leader: false,
                display_id: super::data::CLASSES[0].looks[0],
            }),
        );
    }
}
fn spawn(
    mut commands: Commands,
    selection: Res<Selection>,
    role: Res<Role>,
    mut run: ResMut<Run>,
    npc: Query<Entity, With<Gambler>>,
    heroes: Query<&hero::Hero>,
) {
    if !selection.in_hub {
        for entity in &npc {
            commands.entity(entity).despawn();
        }
        return;
    }
    if *role == Role::Guest || heroes.is_empty() || !npc.is_empty() {
        return;
    }
    let pos = position();
    let entity = units::spawn_unit(
        &mut commands,
        &units::UnitSpec {
            serial: run.guid(),
            display: super::data::CLASSES[0].looks[0],
            scale: 1.0,
            level: 60,
            faction: units::FACTION_FRIENDLY,
            bytes0: 0x0101,
            weapons: &[],
            sheath: 0,
        },
        pos,
        units::yaw_toward(hero::arena(&super::data::HUB) - pos),
    );
    commands.entity(entity).insert(Gambler);
    commands.queue(move |world: &mut World| {
        if let Some(mut store) = world.get_mut::<crate::net::ObjectStore>(entity) {
            store
                .0
                .merge(benilla_protocol::messages::ObjectFields::from_pairs(&[(
                    3, NPC_ENTRY,
                )]));
        }
    });
}
fn input(
    phase: Res<State<Phase>>,
    mut selection: ResMut<Selection>,
    heroes: Query<&Transform, With<hero::Hero>>,
    mut view: ResMut<View>,
    mut inventory: ResMut<Inventory>,
    account: Res<talents::Account>,
    mut rng: ResMut<Rng>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    clicks: Query<(&Interaction, &Click), Changed<Interaction>>,
) {
    let accessible = selection.in_hub
        && *phase.get() == Phase::Hub
        && heroes.single().is_ok_and(|t| near(t.translation));
    if !accessible {
        view.open = false;
        return;
    }
    if view.open && keys.just_pressed(KeyCode::Escape) {
        view.open = false;
        keys.clear_just_pressed(KeyCode::Escape);
    }
    if keys.just_pressed(KeyCode::KeyG) {
        view.open = !view.open;
        keys.clear_just_pressed(KeyCode::KeyG);
    }
    for (interaction, click) in &clicks {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *click {
            Click::Open => view.open = true,
            Click::Close => view.open = false,
            Click::Slot(slot) if view.open => {
                view.slot = slot;
                view.notice.clear();
                if account.character != inventory.money_owner() || account.character.is_none() {
                    view.notice = "Sélectionnez un personnage avant de parier.".into();
                } else {
                    match inventory.gamble(view.slot, talents::level(account.xp), &mut rng) {
                        Ok(item) => view.result = Some(item),
                        Err(error) => view.notice = error,
                    }
                }
                view.revision = view.revision.wrapping_add(1);
            }
            Click::Page(delta) if view.open => {
                let pages = SLOTS
                    .iter()
                    .filter(|(s, _)| inventory.gamble_available(*s))
                    .count()
                    .div_ceil(10);
                view.page = view
                    .page
                    .saturating_add_signed(delta as isize)
                    .min(pages.saturating_sub(1));
                view.revision = view.revision.wrapping_add(1);
            }
            _ => {}
        }
    }
    if view.open {
        selection.ui_captured = true;
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
    )
}
fn render(
    mut commands: Commands,
    view: Res<View>,
    inventory: Res<Inventory>,
    account: Res<talents::Account>,
    selection: Res<Selection>,
    phase: Res<State<Phase>>,
    heroes: Query<&Transform, With<hero::Hero>>,
    font: Option<Res<hud::HudFont>>,
    roots: Query<Entity, With<Root>>,
    mut cache: Local<String>,
) {
    let Some(font) = font else { return };
    let accessible = selection.in_hub
        && *phase.get() == Phase::Hub
        && heroes.single().is_ok_and(|t| near(t.translation));
    let level = talents::level(account.xp);
    let stamp = format!(
        "{accessible} {} {:?} {} {} {} {}",
        view.open,
        view.slot,
        view.revision,
        inventory.money(),
        inventory.revision,
        level
    );
    if *cache == stamp {
        return;
    }
    *cache = stamp;
    for entity in &roots {
        commands.entity(entity).despawn();
    }
    if !accessible {
        return;
    }
    if !view.open {
        commands
            .spawn((
                Root,
                GlobalZIndex(456),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(50.0),
                    bottom: Val::Px(58.0),
                    margin: UiRect::left(Val::Px(-130.0)),
                    ..default()
                },
            ))
            .with_children(|ui| {
                skin::button(ui, &font.0, "Gheed · Parier [G]", Click::Open, 260.0)
            });
        return;
    }
    let slots: Vec<_> = SLOTS
        .iter()
        .copied()
        .filter(|(slot, _)| inventory.gamble_available(*slot))
        .collect();
    let pages = slots.len().div_ceil(10).max(1);
    let page = view.page.min(pages - 1);
    commands
        .spawn((
            Root,
            GlobalZIndex(615),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(16.0),
                top: Val::Px(80.0),
                width: Val::Px(384.0),
                height: Val::Px(512.0),
                ..default()
            },
        ))
        .with_children(|ui| {
            // MerchantFrame.xml: four native 256/128 x 256 quadrants.
            for (path, x, y, w) in [
                (
                    "Interface\\MerchantFrame\\UI-Merchant-TopLeft",
                    0.0,
                    0.0,
                    256.0,
                ),
                (
                    "Interface\\MerchantFrame\\UI-Merchant-TopRight",
                    256.0,
                    0.0,
                    128.0,
                ),
                (
                    "Interface\\MerchantFrame\\UI-Merchant-BotLeft",
                    0.0,
                    256.0,
                    256.0,
                ),
                (
                    "Interface\\MerchantFrame\\UI-Merchant-BotRight",
                    256.0,
                    256.0,
                    128.0,
                ),
            ] {
                skin::art(ui, path, skin::rect(x, y, w, 256.0), [0.0, 1.0, 0.0, 1.0]);
            }
            skin::art(
                ui,
                "Interface\\Icons\\INV_Misc_Coin_01",
                skin::rect(9.0, 7.0, 52.0, 52.0),
                [0.0, 1.0, 0.0, 1.0],
            );
            ui.spawn((
                text(&font.0, "Gheed", 16.0, hud::GOLD_TONE),
                skin::rect(145.0, 16.0, 180.0, 24.0),
            ));
            ui.spawn(skin::rect(330.0, 10.0, 28.0, 28.0))
                .with_children(|b| skin::button(b, &font.0, "X", Click::Close, 28.0));
            ui.spawn((
                text(
                    &font.0,
                    format!("Votre classe · Niveau {level}"),
                    12.0,
                    hud::GOLD_TONE,
                ),
                skin::rect(100.0, 55.0, 225.0, 20.0),
            ));
            for (index, &(slot, label)) in slots.iter().skip(page * 10).take(10).enumerate() {
                let x = 22.0 + (index % 2) as f32 * 164.0;
                let y = 85.0 + (index / 2) as f32 * 60.0;
                ui.spawn((
                    Button,
                    Click::Slot(slot),
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
                    Node {
                        padding: UiRect::all(Val::Px(3.0)),
                        column_gap: Val::Px(5.0),
                        align_items: AlignItems::Center,
                        ..skin::rect(x, y, 156.0, 55.0)
                    },
                ))
                .with_children(|row| {
                    row.spawn((
                        BorderColor::all(hud::GOLD_TONE),
                        Node {
                            width: Val::Px(40.0),
                            height: Val::Px(40.0),
                            flex_shrink: 0.0,
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                    ))
                    .with_children(|icon| {
                        icon.spawn((
                            ImageNode::default(),
                            hud::UiArt::item(&format!("Interface\\Icons\\{}", slot_icon(slot))),
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                        ));
                    });
                    row.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        width: Val::Px(103.0),
                        row_gap: Val::Px(3.0),
                        ..default()
                    })
                    .with_children(|body| {
                        body.spawn(text(&font.0, label, 12.0, Color::srgb(0.2, 0.65, 1.0)));
                        coins(body, &font.0, price(slot, level));
                    });
                });
            }
            ui.spawn(Node {
                column_gap: Val::Px(10.0),
                align_items: AlignItems::Center,
                ..skin::rect(28.0, 392.0, 320.0, 30.0)
            })
            .with_children(|footer| {
                skin::button(footer, &font.0, "<", Click::Page(-1), 30.0);
                footer.spawn(text(
                    &font.0,
                    format!("Page {} / {pages}", page + 1),
                    13.0,
                    hud::GOLD_TONE,
                ));
                skin::button(footer, &font.0, ">", Click::Page(1), 30.0);
            });
            ui.spawn(Node {
                justify_content: JustifyContent::FlexEnd,
                ..skin::rect(24.0, 430.0, 320.0, 24.0)
            })
            .with_children(|wallet| coins(wallet, &font.0, inventory.money()));
            let message = if !view.notice.is_empty() {
                view.notice.clone()
            } else if let Some(item) = &view.result {
                format!("Reçu : {}", item.name)
            } else {
                "Cliquez sur un article pour parier.".into()
            };
            ui.spawn((
                text(
                    &font.0,
                    message,
                    12.0,
                    if view.notice.is_empty() {
                        hud::GOLD_TONE
                    } else {
                        Color::srgb(1.0, 0.3, 0.2)
                    },
                ),
                skin::rect(24.0, 461.0, 320.0, 38.0),
            ));
        });
}
fn slot_icon(slot: SlotKind) -> &'static str {
    match slot {
        SlotKind::Head => "INV_Helmet_03",
        SlotKind::Neck => "INV_Jewelry_Necklace_01",
        SlotKind::Shoulders => "INV_Shoulder_05",
        SlotKind::Back => "INV_Misc_Cape_01",
        SlotKind::Chest => "INV_Chest_Chain",
        SlotKind::Wrists => "INV_Bracer_07",
        SlotKind::Hands => "INV_Gauntlets_04",
        SlotKind::Waist => "INV_Belt_03",
        SlotKind::Legs => "INV_Pants_03",
        SlotKind::Feet => "INV_Boots_05",
        SlotKind::Ring => "INV_Jewelry_Ring_01",
        SlotKind::Trinket => "INV_Jewelry_Talisman_01",
        SlotKind::MainHand => "INV_Sword_04",
        SlotKind::OffHand => "INV_Shield_04",
        _ => "INV_Weapon_Bow_01",
    }
}
fn coins(ui: &mut ChildSpawnerCommands, font: &Handle<Font>, value: u64) {
    let (g, s, c) = super::currency::split(value);
    ui.spawn(Node {
        column_gap: Val::Px(2.0),
        align_items: AlignItems::Center,
        ..default()
    })
    .with_children(|row| {
        for (n, coords) in [
            (g, [0.0, 0.25, 0.0, 1.0]),
            (s, [0.25, 0.5, 0.0, 1.0]),
            (c, [0.5, 0.75, 0.0, 1.0]),
        ] {
            if n == 0 {
                continue;
            }
            row.spawn(text(font, n.to_string(), 11.0, Color::WHITE));
            row.spawn((
                ImageNode::default(),
                hud::UiArt::crop("Interface\\MoneyFrame\\UI-MoneyIcons", coords),
                Node {
                    width: Val::Px(12.0),
                    height: Val::Px(12.0),
                    ..default()
                },
            ));
        }
        if value == 0 {
            row.spawn(text(font, "0", 11.0, Color::WHITE));
        }
    });
}
fn tooltip(
    mut commands: Commands,
    view: Res<View>,
    account: Res<talents::Account>,
    inventory: Res<Inventory>,
    font: Option<Res<hud::HudFont>>,
    slots: Query<(&Interaction, &Click)>,
    roots: Query<Entity, With<Tooltip>>,
) {
    for e in &roots {
        commands.entity(e).despawn();
    }
    if !view.open {
        return;
    }
    let Some(font) = font else {
        return;
    };
    let Some(slot) = slots.iter().find_map(|(i, c)| match c {
        Click::Slot(s) if *i != Interaction::None => Some(*s),
        _ => None,
    }) else {
        return;
    };
    let level = talents::level(account.xp);
    commands
        .spawn((
            Tooltip,
            GlobalZIndex(660),
            BackgroundColor(Color::srgb(0.02, 0.025, 0.04)),
            BorderColor::all(hud::GOLD_TONE),
            Node {
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(7.0),
                ..skin::rect(405.0, 165.0, 280.0, 160.0)
            },
        ))
        .with_children(|tip| {
            tip.spawn(text(
                &font.0,
                format!("Gamble for {slot:?}"),
                16.0,
                hud::GOLD_TONE,
            ));
            tip.spawn(text(
                &font.0,
                format!("Objet de votre classe · Niveau {level}"),
                13.0,
                Color::WHITE,
            ));
            tip.spawn(text(
                &font.0,
                "Qualité et caractéristiques révélées à l'achat.",
                12.0,
                Color::srgb(0.7, 0.7, 0.7),
            ));
            coins(tip, &font.0, price(slot, level));
            tip.spawn(text(
                &font.0,
                if inventory.money() < price(slot, level) {
                    "Monnaie insuffisante"
                } else {
                    "Cliquer pour acheter"
                },
                12.0,
                Color::srgb(0.9, 0.8, 0.4),
            ));
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn slot_prices_compound_gently_and_jewellery_costs_more() {
        assert_eq!(price(SlotKind::Hands, 1), 500);
        assert!(price(SlotKind::Ring, 30) > price(SlotKind::Hands, 30));
        assert!(price(SlotKind::Neck, 30) > price(SlotKind::Ring, 30));
        for &(slot, _) in SLOTS {
            for level in 1..60 {
                let ratio = price(slot, level + 1) as f64 / price(slot, level) as f64;
                assert!((1.038..1.042).contains(&ratio));
            }
        }
    }
    #[test]
    fn generated_items_always_match_the_requested_slot_class_and_character_level() {
        let config = super::super::items::Config::builtin();
        let mut rng = Rng(123);
        for class in super::super::data::CLASSES {
            for &(slot, _) in SLOTS {
                if !config.gamble_available(slot, class.id) {
                    continue;
                }
                for level in [1, 20, 60] {
                    let item = config
                        .generate_gamble(level, class.id, slot, &mut rng)
                        .unwrap();
                    assert_eq!(item.slot, slot);
                    assert_eq!(item.level, level);
                    assert_eq!(item.loot_class, Some(class.id));
                    assert!(item.usable(class.id, slot), "{} {slot:?}", class.name);
                }
            }
        }
    }
    #[test]
    fn npc_window_buys_at_character_level_and_closes_outside_hub() {
        let mut app = App::new();
        let mut inventory = Inventory::fixture(vec![]);
        inventory.add_money(100000).unwrap();
        let mut account = talents::Account::fresh();
        account.character = Some(1);
        account.xp = talents::threshold(12);
        app.add_plugins(MinimalPlugins)
            .insert_resource(State::new(Phase::Hub))
            .insert_resource(Selection {
                in_hub: true,
                ..default()
            })
            .init_resource::<View>()
            .insert_resource(inventory)
            .insert_resource(account)
            .init_resource::<Rng>()
            .init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(hud::HudFont(Handle::default()))
            .add_systems(Update, (input, render, tooltip).chain());
        app.world_mut().spawn((
            hero::Hero { vy: 0.0, air: 0.0 },
            Transform::from_translation(position()),
        ));
        app.update();
        let press = |app: &mut App, buy: bool| {
            let world = app.world_mut();
            let button = world
                .query::<(Entity, &Click)>()
                .iter(world)
                .find(|(_, c)| {
                    if buy {
                        matches!(c, Click::Slot(SlotKind::Head))
                    } else {
                        matches!(c, Click::Open)
                    }
                })
                .unwrap()
                .0;
            world.entity_mut(button).insert(Interaction::Pressed);
            app.update();
        };
        press(&mut app, false);
        assert!(app.world().resource::<View>().open);
        let world = app.world_mut();
        let head = world
            .query::<(Entity, &Click)>()
            .iter(world)
            .find(|(_, c)| matches!(c, Click::Slot(SlotKind::Head)))
            .unwrap()
            .0;
        world.entity_mut(head).insert(Interaction::Hovered);
        app.update();
        let world = app.world_mut();
        assert_eq!(
            world
                .query_filtered::<Entity, With<Tooltip>>()
                .iter(world)
                .count(),
            1
        );
        assert!(world
            .query::<&Text>()
            .iter(world)
            .any(|t| t.0 == "Gamble for Head"));
        assert_eq!(world.resource::<Inventory>().money(), 100000);
        press(&mut app, true);
        let view = app.world().resource::<View>();
        assert_eq!(view.result.as_ref().unwrap().level, 12);
        assert_eq!(
            app.world().resource::<Inventory>().money(),
            100000 - price(SlotKind::Head, 12)
        );
        app.world_mut().resource_mut::<Selection>().in_hub = false;
        app.update();
        assert!(!app.world().resource::<View>().open);
    }
    #[test]
    fn hub_and_gambler_positions_match_installed_goldshire_terrain() {
        let path = benilla_formats::wow_data_or_skip!();
        let mut chain = benilla_formats::open_chain(&path).unwrap();
        let hub = &super::super::data::HUB;
        let tiles = benilla_formats::load_tiles_around(
            &mut chain,
            "Azeroth",
            hub.center[0],
            hub.center[1],
            0,
        )
        .unwrap();
        for p in [hub.center, [-position().z, -position().x, position().y]] {
            let h = tiles
                .iter()
                .find_map(|(_, tile)| benilla_formats::terrain_height_at(&tile.chunks, p))
                .unwrap();
            assert!((h - p[2]).abs() < 0.01);
        }
    }
}
