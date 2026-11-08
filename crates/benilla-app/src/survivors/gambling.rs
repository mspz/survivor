//! Hub blacksmith: independent item profiles and atomic gold-to-equipment crafting.
use super::{
    combat_ui::skin,
    hero, hud,
    items::{CraftProfile, CraftRequest, EquipmentType, Inventory, Item, Rarity, SlotKind},
    multiplayer::Role,
    talents, units, Phase, Rng, Run, Selection,
};
use bevy::{prelude::*, ui::RelativeCursorPosition};
pub(super) const NPC_ENTRY: u32 = 9_000_001;
pub(super) const SLOTS: &[(SlotKind, &str)] = &[
    (SlotKind::Head, "Tête"),
    (SlotKind::Neck, "Cou"),
    (SlotKind::Shoulders, "Épaules"),
    (SlotKind::Back, "Cape"),
    (SlotKind::Chest, "Torse"),
    (SlotKind::Tabard, "Tabard"),
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
pub(super) fn near(pos: Vec3) -> bool {
    units::flat(pos - position()).length() <= 4.5
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Selector {
    Slot,
    Equipment,
    Profile,
    Rarity,
}
impl Selector {
    fn index(self) -> usize {
        match self {
            Self::Slot => 0,
            Self::Equipment => 1,
            Self::Profile => 2,
            Self::Rarity => 3,
        }
    }
}
fn equipment_label(kind: EquipmentType) -> &'static str {
    match kind {
        EquipmentType::Plate => "Plaque",
        EquipmentType::Mail => "Maille",
        EquipmentType::Leather => "Cuir",
        EquipmentType::Cloth => "Tissu",
        EquipmentType::MeleeOneHand => "Une main physique",
        EquipmentType::HybridOneHand => "Une main hybride",
        EquipmentType::OneHand => "Une main",
        EquipmentType::TwoHand => "Deux mains",
        EquipmentType::Staff => "Baton",
        EquipmentType::Shield => "Bouclier",
        EquipmentType::HeldOffHand => "Objet en main gauche",
        EquipmentType::Bow => "Arc",
        EquipmentType::Wand => "Baguette",
        EquipmentType::Universal => "Accessoire",
        EquipmentType::Legacy => "Autre",
    }
}
#[derive(Resource)]
pub(super) struct View {
    dropdown: Option<usize>,
    selector: Option<Selector>,
    selected: [bool; 4],
    ilvl: Option<u32>,
    pub open: bool,
    slot: SlotKind,
    equipment: EquipmentType,
    profile: CraftProfile,
    rarity: Rarity,
    custom: bool,
    affixes: Vec<Option<String>>,
    result: Option<Item>,
    notice: String,
    revision: u64,
}
impl Default for View {
    fn default() -> Self {
        Self {
            dropdown: None,
            selector: None,
            selected: [false; 4],
            ilvl: None,
            open: false,
            slot: SlotKind::Head,
            equipment: EquipmentType::Plate,
            profile: CraftProfile::Strength,
            rarity: Rarity::Normal,
            custom: false,
            affixes: vec![None; 6],
            result: None,
            notice: String::new(),
            revision: 0,
        }
    }
}
impl View {
    fn ready(&self) -> bool {
        self.selected.iter().all(|selected| *selected)
    }
    fn item_level(&self, xp: u64) -> u32 {
        self.ilvl
            .unwrap_or_else(|| talents::level(xp))
            .clamp(1, super::items::MAX_ITEM_LEVEL)
    }
    fn affix_cap(&self) -> usize {
        match self.rarity {
            Rarity::Normal => 1,
            Rarity::Magic => 2,
            _ => 3,
        }
    }
    fn slot_kind(&self, slot: usize) -> Option<super::items::AffixKind> {
        (slot < 6 && slot % 3 < self.affix_cap()).then_some(if slot < 3 {
            super::items::AffixKind::Prefix
        } else {
            super::items::AffixKind::Suffix
        })
    }
    fn dropdown_choices(&self, choices: &[super::items::CraftAffix], slot: usize) -> Vec<usize> {
        let Some(kind) = self.slot_kind(slot) else {
            return vec![];
        };
        let mut available: Vec<_> = choices
            .iter()
            .enumerate()
            .filter(|(_, choice)| {
                choice.kind == kind
                    && !self
                        .affixes
                        .iter()
                        .enumerate()
                        .filter(|(index, _)| *index != slot)
                        .filter_map(|(_, id)| id.as_ref())
                        .any(|id| {
                            choices
                                .iter()
                                .any(|c| &c.id == id && c.group == choice.group)
                        })
            })
            .map(|(index, _)| index)
            .collect();
        available.sort_by_key(|index| choices[*index].label.to_lowercase());
        available
    }
    fn request(&self) -> CraftRequest {
        CraftRequest {
            slot: self.slot,
            equipment: self.equipment,
            profile: self.profile,
            rarity: self.rarity,
            affixes: self
                .custom
                .then(|| self.affixes.iter().flatten().cloned().collect()),
        }
    }
}
#[derive(Component)]
struct Gambler;
#[derive(Component)]
struct Root;
#[derive(Component)]
struct Tooltip;
#[derive(Component)]
struct ScrollArea;
#[derive(Component)]
struct ItemPreview;
#[derive(Component, Clone, Copy)]
pub(super) enum Click {
    Open,
    Close,
    Selector(Selector),
    Slot(SlotKind),
    Dropdown(usize),
    SelectAffix(usize, Option<usize>),
    DismissDropdown,
    Equipment(EquipmentType),
    Level(i32),
    Profile(CraftProfile),
    Rarity(Rarity),
    Custom(bool),
    Buy,
    Result,
}
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<View>()
        .add_systems(Update, register_name)
        .add_systems(Update, spawn.after(super::characters::arrive))
        .add_systems(
            Update,
            input
                .after(super::combat_ui::input)
                .after(super::hub_interact::click)
                .before(super::client::input)
                .before(hud::keys),
        )
        .add_systems(Update, render.after(input))
        .add_systems(Update, tooltip.after(render))
        .add_systems(Update, scroll_dropdown.after(render));
}
fn register_name(mut names: Option<ResMut<crate::names::NameCache>>) {
    let Some(names) = names.as_deref_mut() else {
        return;
    };
    if names.creature_record(NPC_ENTRY).is_none() {
        names.insert_creature(
            NPC_ENTRY,
            Some(crate::names::CreatureRecord {
                name: "Gheed le forgeron".into(),
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
pub(super) fn input(
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
    let was_open = view.open;
    let accessible = selection.in_hub
        && matches!(*phase.get(), Phase::Hub | Phase::CharacterSheet)
        && heroes.single().is_ok_and(|t| near(t.translation));
    if !accessible {
        view.open = false;
        return;
    }
    if view.open && keys.just_pressed(KeyCode::Escape) {
        let popup = view.dropdown.take().is_some() | view.selector.take().is_some();
        if !popup {
            view.open = false;
        }
        view.revision = view.revision.wrapping_add(1);
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
            Click::Close => {
                view.open = false;
                view.dropdown = None;
            }
            Click::Selector(selector) if view.open => {
                if selector == Selector::Equipment && !view.selected[0] {
                    view.notice = "Selectionnez d'abord un emplacement.".into();
                } else {
                    view.selector = if view.selector == Some(selector) {
                        None
                    } else {
                        Some(selector)
                    };
                    view.dropdown = None;
                    view.notice.clear();
                }
                view.revision = view.revision.wrapping_add(1);
            }
            Click::Slot(slot) if view.open => {
                view.selected[0] = true;
                view.selected[1] = false;
                view.selector = None;
                view.slot = slot;
                view.notice.clear();
                let types = inventory.craft_types(slot);
                if !types.contains(&view.equipment) {
                    if let Some(kind) = types.first() {
                        view.equipment = *kind;
                    }
                }
                view.affixes.fill(None);
                view.dropdown = None;
                view.revision = view.revision.wrapping_add(1);
            }
            Click::Level(delta) if view.open => {
                let current = view.item_level(account.xp);
                view.ilvl = Some(
                    current
                        .saturating_add_signed(delta)
                        .clamp(1, super::items::MAX_ITEM_LEVEL),
                );
                view.notice.clear();
                view.revision = view.revision.wrapping_add(1);
            }
            Click::Equipment(kind)
                if view.open && inventory.craft_types(view.slot).contains(&kind) =>
            {
                view.notice.clear();
                view.equipment = kind;
                view.selected[1] = true;
                view.selector = None;
                view.affixes.fill(None);
                view.dropdown = None;
                view.revision = view.revision.wrapping_add(1);
            }
            Click::Profile(profile) if view.open => {
                view.profile = profile;
                view.selected[2] = true;
                view.selector = None;
                view.notice.clear();
                view.affixes.fill(None);
                view.dropdown = None;
                view.revision = view.revision.wrapping_add(1);
            }
            Click::Rarity(rarity) if view.open => {
                view.rarity = rarity;
                view.selected[3] = true;
                view.selector = None;
                view.notice.clear();
                view.affixes.fill(None);
                view.dropdown = None;
                view.revision = view.revision.wrapping_add(1);
            }
            Click::Custom(custom) if view.open => {
                view.custom = custom;
                if !custom {
                    view.affixes.fill(None);
                }
                view.dropdown = None;
                view.notice.clear();
                view.revision = view.revision.wrapping_add(1);
            }
            Click::Dropdown(slot)
                if view.open && view.ready() && view.slot_kind(slot).is_some() =>
            {
                view.custom = true;
                view.selector = None;
                view.dropdown = if view.dropdown == Some(slot) {
                    None
                } else {
                    Some(slot)
                };
                view.notice.clear();
                view.revision = view.revision.wrapping_add(1);
            }
            Click::SelectAffix(slot, choice)
                if view.open && view.custom && view.slot_kind(slot).is_some() =>
            {
                let choices = inventory.craft_affixes(&view.request(), view.item_level(account.xp));
                match choice {
                    None => view.affixes[slot] = None,
                    Some(index) if view.dropdown_choices(&choices, slot).contains(&index) => {
                        view.affixes[slot] = Some(choices[index].id.clone())
                    }
                    _ => {
                        view.notice = "Affixe incompatible ou deja choisi.".into();
                    }
                }
                view.dropdown = None;
                view.revision = view.revision.wrapping_add(1);
            }
            Click::DismissDropdown if view.open => {
                view.dropdown = None;
                view.selector = None;
                view.revision = view.revision.wrapping_add(1);
            }
            Click::Buy if view.open => {
                view.notice.clear();
                if !view.ready() {
                    view.notice =
                        "Selectionnez un emplacement, un type, un profil et une rarete.".into();
                } else if account.character != inventory.money_owner()
                    || account.character.is_none()
                {
                    view.notice = "Selectionnez un personnage avant de fabriquer.".into();
                } else {
                    match inventory.craft(&view.request(), view.item_level(account.xp), &mut rng) {
                        Ok(item) => view.result = Some(item),
                        Err(error) => view.notice = error,
                    }
                }
                view.revision = view.revision.wrapping_add(1);
            }
            _ => {}
        }
    }
    if view.open {
        if !was_open {
            selection.bags_open = true;
        }
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
        && matches!(*phase.get(), Phase::Hub | Phase::CharacterSheet)
        && heroes.single().is_ok_and(|t| near(t.translation));
    let level = view.item_level(account.xp);
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
                smith_button(ui, &font.0, "Forgeron - Fabriquer [G]", Click::Open, 260.0)
            });
        return;
    }
    let request = view.request();
    let choices = if view.ready() {
        inventory.craft_affixes(&request, level)
    } else {
        vec![]
    };
    commands
        .spawn((
            Root,
            GlobalZIndex(615),
            BackgroundColor(Color::srgb(0.025, 0.023, 0.02)),
            Node {
                border: UiRect::all(Val::Px(2.0)),
                ..skin::rect(16.0, 70.0, 800.0, 624.0)
            },
            BorderColor::all(hud::GOLD_TONE),
        ))
        .with_children(|ui| {
            skin::backdrop(ui);
            ui.spawn((
                text(&font.0, "Gheed - Forgeron", 20.0, hud::GOLD_TONE),
                skin::rect(22.0, 14.0, 330.0, 28.0),
            ));
            ui.spawn(Node {
                column_gap: Val::Px(5.0),
                align_items: AlignItems::Center,
                ..skin::rect(370.0, 12.0, 365.0, 30.0)
            })
            .with_children(|row| {
                smith_button(row, &font.0, "-10", Click::Level(-10), 38.0);
                smith_button(row, &font.0, "-", Click::Level(-1), 28.0);
                row.spawn((
                    text(&font.0, format!("Ilvl : {level}"), 14.0, hud::GOLD_TONE),
                    Node {
                        width: Val::Px(110.0),
                        ..default()
                    },
                ));
                smith_button(row, &font.0, "+", Click::Level(1), 28.0);
                smith_button(row, &font.0, "+10", Click::Level(10), 38.0);
            });
            ui.spawn(skin::rect(750.0, 10.0, 30.0, 28.0))
                .with_children(|b| smith_button(b, &font.0, "X", Click::Close, 30.0));
            ui.spawn((
                text(&font.0, "Configuration de l'objet", 14.0, hud::GOLD_TONE),
                skin::rect(20.0, 58.0, 325.0, 22.0),
            ));
            for selector in [
                Selector::Slot,
                Selector::Equipment,
                Selector::Profile,
                Selector::Rarity,
            ] {
                let index = selector.index();
                let (title, placeholder, value) = match selector {
                    Selector::Slot => (
                        "Emplacement",
                        "Selectionner un emplacement",
                        SLOTS
                            .iter()
                            .find(|(slot, _)| *slot == view.slot)
                            .map_or("Objet", |(_, label)| *label),
                    ),
                    Selector::Equipment => (
                        "Type d'objet",
                        "Selectionner un type",
                        equipment_label(view.equipment),
                    ),
                    Selector::Profile => (
                        "Profil des affixes",
                        "Selectionner un profil",
                        view.profile.label(),
                    ),
                    Selector::Rarity => (
                        "Rarete",
                        "Selectionner une rarete",
                        match view.rarity {
                            Rarity::Normal => "Normal",
                            Rarity::Magic => "Magique",
                            Rarity::Epic => "Epique",
                            _ => "Indisponible",
                        },
                    ),
                };
                let top = 91.0 + index as f32 * 67.0;
                ui.spawn((
                    text(&font.0, title, 12.0, Color::WHITE),
                    skin::rect(20.0, top, 325.0, 18.0),
                ));
                let label = if view.selected[index] {
                    format!("[{value}]  v")
                } else {
                    format!("{placeholder}  v")
                };
                ui.spawn(skin::rect(20.0, top + 21.0, 325.0, 30.0))
                    .with_children(|row| {
                        smith_button(row, &font.0, label, Click::Selector(selector), 325.0)
                    });
            }
            for (index, (custom, label)) in [(false, "Affixes aleatoires"), (true, "Sur mesure")]
                .into_iter()
                .enumerate()
            {
                let label = if view.custom == custom {
                    format!("[{label}]")
                } else {
                    label.to_string()
                };
                ui.spawn(skin::rect(20.0 + index as f32 * 164.0, 380.0, 160.0, 30.0))
                    .with_children(|row| {
                        smith_button(row, &font.0, &label, Click::Custom(custom), 160.0)
                    });
            }
            ui.spawn((
                text(
                    &font.0,
                    if view.custom {
                        "Affixes choisis : valeurs aleatoires.
Chaque ajout double le prix."
                    } else {
                        "Affixes et valeurs tires au hasard
selon le slot, le type et le profil."
                    },
                    12.0,
                    Color::WHITE,
                ),
                skin::rect(20.0, 422.0, 325.0, 42.0),
            ));
            ui.spawn(Node {
                column_gap: Val::Px(8.0),
                ..skin::rect(20.0, 505.0, 330.0, 24.0)
            })
            .with_children(|row| {
                row.spawn(text(&font.0, "Prix :", 13.0, hud::GOLD_TONE));
                if !view.ready() {
                    row.spawn(text(
                        &font.0,
                        "Choisir la configuration",
                        12.0,
                        Color::WHITE,
                    ));
                } else {
                    match request.price(level) {
                        Ok(price) => coins(row, &font.0, price),
                        Err(_) => {
                            row.spawn(text(&font.0, "Prix trop eleve", 12.0, Color::WHITE));
                        }
                    }
                }
            });
            ui.spawn(skin::rect(20.0, 536.0, 325.0, 32.0))
                .with_children(|row| smith_button(row, &font.0, "Fabriquer", Click::Buy, 325.0));
            ui.spawn(Node {
                column_gap: Val::Px(8.0),
                ..skin::rect(20.0, 580.0, 325.0, 24.0)
            })
            .with_children(|row| {
                row.spawn(text(&font.0, "Votre or :", 13.0, hud::GOLD_TONE));
                coins(row, &font.0, inventory.money());
            });
            let cap = if view.ready() { view.affix_cap() } else { 0 };
            ui.spawn((
                text(
                    &font.0,
                    if view.ready() {
                        format!(
                            "Affixes sur mesure - {} / {} choisis",
                            view.affixes.iter().flatten().count(),
                            cap * 2
                        )
                    } else {
                        "Affixes".to_owned()
                    },
                    14.0,
                    hud::GOLD_TONE,
                ),
                skin::rect(370.0, 58.0, 405.0, 24.0),
            ));
            if !view.ready() {
                ui.spawn((
                    text(
                        &font.0,
                        "Completez la configuration a gauche\npour choisir vos affixes.",
                        13.0,
                        Color::WHITE,
                    ),
                    skin::rect(370.0, 105.0, 405.0, 44.0),
                ));
            }
            for (group, title) in [(0, "Prefixes"), (1, "Suffixes")]
                .into_iter()
                .filter(|_| view.ready())
            {
                let top = 95.0 + group as f32 * (54.0 + cap as f32 * 40.0);
                ui.spawn((
                    text(&font.0, title, 14.0, hud::GOLD_TONE),
                    skin::rect(370.0, top, 405.0, 22.0),
                ));
                for index in 0..cap {
                    let slot = group * 3 + index;
                    let selected = view.affixes[slot]
                        .as_ref()
                        .and_then(|id| choices.iter().find(|a| &a.id == id));
                    let label = selected
                        .map_or_else(|| "Choisir un affixe".to_owned(), |a| a.label.clone());
                    let label = format!("{} : {label}  v", index + 1);
                    ui.spawn(skin::rect(
                        370.0,
                        top + 28.0 + index as f32 * 40.0,
                        405.0,
                        30.0,
                    ))
                    .with_children(|row| {
                        smith_button(row, &font.0, label, Click::Dropdown(slot), 405.0)
                    });
                }
            }
            if let Some(slot) = view.dropdown.filter(|slot| view.slot_kind(*slot).is_some()) {
                ui.spawn((
                    Button,
                    Click::DismissDropdown,
                    GlobalZIndex(646),
                    BackgroundColor(Color::NONE),
                    skin::rect(0.0, 0.0, 800.0, 624.0),
                ));
                let row = if slot < 3 { slot } else { 3 + slot - 3 };
                let top = if row < 3 {
                    155.0 + row as f32 * 40.0
                } else {
                    180.0
                };
                ui.spawn((
                    GlobalZIndex(650),
                    BackgroundColor(Color::srgb(0.045, 0.035, 0.025)),
                    BorderColor::all(hud::GOLD_TONE),
                    Node {
                        border: UiRect::all(Val::Px(1.0)),
                        padding: UiRect::all(Val::Px(4.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(3.0),
                        overflow: Overflow::scroll_y(),
                        ..skin::rect(370.0, top, 405.0, 260.0)
                    },
                    ScrollArea,
                    ScrollPosition::default(),
                    RelativeCursorPosition::default(),
                ))
                .with_children(|list| {
                    smith_button(
                        list,
                        &font.0,
                        "Aucun / Retirer l'affixe",
                        Click::SelectAffix(slot, None),
                        393.0,
                    );
                    for index in view.dropdown_choices(&choices, slot) {
                        let label = &choices[index].label;
                        smith_button(
                            list,
                            &font.0,
                            label,
                            Click::SelectAffix(slot, Some(index)),
                            393.0,
                        );
                    }
                });
            }
            if let Some(selector) = view.selector {
                ui.spawn((
                    Button,
                    Click::DismissDropdown,
                    GlobalZIndex(646),
                    BackgroundColor(Color::NONE),
                    skin::rect(0.0, 0.0, 800.0, 624.0),
                ));
                let options: Vec<(String, Click)> = match selector {
                    Selector::Slot => SLOTS
                        .iter()
                        .map(|(slot, label)| (label.to_string(), Click::Slot(*slot)))
                        .collect(),
                    Selector::Equipment => inventory
                        .craft_types(view.slot)
                        .into_iter()
                        .map(|kind| (equipment_label(kind).to_owned(), Click::Equipment(kind)))
                        .collect(),
                    Selector::Profile => [
                        CraftProfile::Strength,
                        CraftProfile::Agility,
                        CraftProfile::Intelligence,
                    ]
                    .into_iter()
                    .map(|profile| (profile.label().to_string(), Click::Profile(profile)))
                    .collect(),
                    Selector::Rarity => [
                        (Rarity::Normal, "Normal"),
                        (Rarity::Magic, "Magique"),
                        (Rarity::Epic, "Epique"),
                    ]
                    .into_iter()
                    .map(|(rarity, label)| (label.to_owned(), Click::Rarity(rarity)))
                    .collect(),
                };
                let top = (144.0 + selector.index() as f32 * 67.0).min(355.0);
                let height = (options.len() as f32 * 31.0 + 8.0).min(235.0);
                ui.spawn((
                    ScrollArea,
                    ScrollPosition::default(),
                    RelativeCursorPosition::default(),
                    GlobalZIndex(650),
                    BackgroundColor(Color::srgb(0.045, 0.035, 0.025)),
                    BorderColor::all(hud::GOLD_TONE),
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(3.0),
                        padding: UiRect::all(Val::Px(4.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        overflow: Overflow::scroll_y(),
                        ..skin::rect(20.0, top, 325.0, height)
                    },
                ))
                .with_children(|menu| {
                    for (label, action) in options {
                        smith_button(menu, &font.0, label, action, 313.0);
                    }
                });
            }
            ui.spawn((
                BackgroundColor(Color::srgb(0.025, 0.025, 0.04)),
                BorderColor::all(view.rarity.color()),
                Node {
                    padding: UiRect::all(Val::Px(10.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(8.0),
                    ..skin::rect(370.0, 442.0, 405.0, 172.0)
                },
            ))
            .with_children(|panel| {
                panel.spawn(text(
                    &font.0,
                    "Objet a fabriquer - survoler l'icone",
                    12.0,
                    hud::GOLD_TONE,
                ));
                if let Some(item) = inventory.craft_preview(&request, level) {
                    let mut extra_lines = vec![(
                        "Valeurs indicatives, relancees a la fabrication.".to_string(),
                        Color::srgb(0.7, 0.7, 0.7),
                    )];
                    if view.custom {
                        for (slot, id) in view
                            .affixes
                            .iter()
                            .enumerate()
                            .filter_map(|(slot, id)| id.as_ref().map(|id| (slot, id)))
                        {
                            if let Some(affix) = choices.iter().find(|a| &a.id == id) {
                                let kind = if slot < 3 { "Prefixe" } else { "Suffixe" };
                                extra_lines.push((
                                    format!("{kind} : {}", affix.label),
                                    Color::srgb(0.3, 1.0, 0.3),
                                ));
                            }
                        }
                    } else {
                        let (min, max) = match view.rarity {
                            Rarity::Normal => (1, 2),
                            Rarity::Magic => (2, 4),
                            _ => (4, 6),
                        };
                        extra_lines.push((
                            format!("{min} a {max} affixes aleatoires a la fabrication."),
                            Color::srgb(0.3, 1.0, 0.3),
                        ));
                    }
                    panel
                        .spawn(Node {
                            column_gap: Val::Px(10.0),
                            align_items: AlignItems::Center,
                            ..default()
                        })
                        .with_children(|row| {
                            row.spawn((
                                ItemPreview,
                                Button,
                                super::combat_ui::bags::ReadOnlyItemTip {
                                    item: item.clone(),
                                    extra_lines,
                                },
                                BorderColor::all(item.rarity.color()),
                                BackgroundColor(Color::srgb(0.06, 0.06, 0.075)),
                                Node {
                                    width: Val::Px(40.0),
                                    height: Val::Px(40.0),
                                    border: UiRect::all(Val::Px(2.0)),
                                    flex_shrink: 0.0,
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                    ..default()
                                },
                            ))
                            .with_children(|cell| {
                                super::combat_ui::bags::icon(
                                    cell,
                                    &item,
                                    36.0,
                                    inventory.equippable(&item),
                                )
                            });
                            row.spawn((
                                text(&font.0, &item.name, 14.0, item.rarity.color()),
                                Node {
                                    width: Val::Px(325.0),
                                    ..default()
                                },
                            ));
                        });
                }
                if !view.notice.is_empty() {
                    panel.spawn(text(
                        &font.0,
                        &view.notice,
                        12.0,
                        Color::srgb(1.0, 0.3, 0.2),
                    ));
                } else if let Some(item) = &view.result {
                    smith_button(
                        panel,
                        &font.0,
                        format!("Dernier objet : {}", item.name),
                        Click::Result,
                        380.0,
                    );
                } else {
                    panel.spawn(text(
                        &font.0,
                        if view.ready() {
                            "Apercu non recuperable. Valeurs relancees a l'achat."
                        } else {
                            "Completez les quatre choix pour afficher l'objet."
                        },
                        11.0,
                        Color::srgb(0.7, 0.7, 0.7),
                    ));
                }
            });
        });
}

fn smith_button(
    ui: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    label: impl Into<String>,
    action: Click,
    width: f32,
) {
    let label = label.into();
    let selected = label.starts_with('[') && !label.starts_with("[ ]");
    ui.spawn((
        Button,
        action,
        BackgroundColor(if selected {
            Color::srgb(0.23, 0.13, 0.06)
        } else {
            Color::srgb(0.09, 0.07, 0.05)
        }),
        BorderColor::all(if selected {
            hud::GOLD_TONE
        } else {
            Color::srgb(0.35, 0.28, 0.16)
        }),
        Node {
            width: Val::Px(width),
            height: Val::Px(28.0),
            min_height: Val::Px(28.0),
            flex_shrink: 0.0,
            border: UiRect::all(Val::Px(1.0)),
            padding: UiRect::axes(Val::Px(4.0), Val::Px(3.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            overflow: Overflow::clip(),
            ..default()
        },
    ))
    .with_children(|b| {
        b.spawn((
            text(font, label, 12.0, hud::GOLD_TONE),
            TextLayout::new_with_linebreak(bevy::text::LineBreak::NoWrap),
        ));
    });
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
fn scroll_dropdown(
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    mut menus: Query<
        (&ComputedNode, &RelativeCursorPosition, &mut ScrollPosition),
        With<ScrollArea>,
    >,
) {
    for event in wheel.read() {
        for (node, cursor, mut position) in &mut menus {
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
fn tooltip(
    mut commands: Commands,
    view: Res<View>,
    inventory: Res<Inventory>,
    account: Res<talents::Account>,
    font: Option<Res<hud::HudFont>>,
    buttons: Query<(&Interaction, &Click)>,
    roots: Query<Entity, With<Tooltip>>,
) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
    if !view.open {
        return;
    }
    let Some(font) = font else {
        return;
    };
    let content = buttons.iter().filter(|(i, _)| **i == Interaction::Hovered).find_map(|(_, click)| match click {
        Click::Selector(Selector::Slot) if view.selected[0] => Some(format!("Emplacement : {:?}\nChoisir un autre emplacement.",view.slot)),
        Click::Dropdown(slot) => view.affixes.get(*slot).and_then(|id| id.as_ref()).and_then(|id| inventory.craft_affixes(&view.request(), view.item_level(account.xp)).into_iter().find(|a| &a.id == id)).map(|a| a.label),
        Click::SelectAffix(_, Some(index)) => inventory.craft_affixes(&view.request(), view.item_level(account.xp)).get(*index).map(|a| format!("{}\nDisponible une seule fois par objet.\nValeur tiree au hasard dans cette plage.\nSelectionner cet affixe double le prix.", a.label)),
        Click::Slot(slot) => Some(format!("Emplacement : {slot:?}\nChoisissez ensuite le type, le profil et la rarete.")),
        Click::Result => view.result.as_ref().map(|item| format!("{}\n{}", item.name, item.affixes.iter().map(|a| a.description()).collect::<Vec<_>>().join("\n"))),
        _ => None,
    });
    let Some(content) = content else {
        return;
    };
    commands
        .spawn((
            Tooltip,
            GlobalZIndex(660),
            BackgroundColor(Color::srgb(0.02, 0.025, 0.04)),
            BorderColor::all(hud::GOLD_TONE),
            Node {
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..skin::rect(825.0, 170.0, 330.0, 210.0)
            },
        ))
        .with_children(|tip| {
            tip.spawn(text(&font.0, content, 13.0, Color::WHITE));
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn press_action(app: &mut App, predicate: impl Fn(&Click) -> bool) {
        let entity = {
            let world = app.world_mut();
            world
                .query::<(Entity, &Click)>()
                .iter(world)
                .find(|(_, click)| predicate(click))
                .unwrap()
                .0
        };
        app.world_mut()
            .entity_mut(entity)
            .insert(Interaction::Pressed);
        app.update();
    }

    #[test]
    fn tabards_are_available_to_every_class_at_the_tabard_price() {
        assert!(SLOTS.iter().any(|(slot, _)| *slot == SlotKind::Tabard));
        let config = super::super::items::Config::builtin();
        for class in super::super::data::CLASSES {
            assert!(config.gamble_available(SlotKind::Tabard, class.id));
        }
        let mut inventory = Inventory::fixture(vec![]);
        inventory.add_money(10_000).unwrap();
        let before = inventory.money();
        let item = inventory
            .gamble(SlotKind::Tabard, 12, &mut Rng(456))
            .unwrap();
        assert_eq!(item.slot, SlotKind::Tabard);
        assert_eq!(item.level, 12);
        assert_eq!(inventory.money(), before - price(SlotKind::Tabard, 12));
        assert!(inventory.item(item.id).is_some());
    }
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
    fn npc_window_buys_at_selected_item_level_and_closes_outside_hub() {
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
                        matches!(c, Click::Buy)
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
        assert!(app.world().resource::<Selection>().bags_open);
        assert!(!app.world().resource::<View>().ready());
        {
            let world = app.world_mut();
            let texts: Vec<_> = world
                .query::<&Text>()
                .iter(world)
                .map(|t| t.0.clone())
                .collect();
            for placeholder in [
                "Selectionner un emplacement",
                "Selectionner un type",
                "Selectionner un profil",
                "Selectionner une rarete",
            ] {
                assert!(texts.iter().any(|text| text.starts_with(placeholder)));
            }
        }
        press(&mut app, true);
        assert_eq!(app.world().resource::<Inventory>().money(), 100000);
        assert!(app.world().resource::<View>().result.is_none());
        for (selector, action) in [
            (Selector::Slot, Click::Slot(SlotKind::Head)),
            (Selector::Equipment, Click::Equipment(EquipmentType::Plate)),
            (Selector::Profile, Click::Profile(CraftProfile::Strength)),
            (Selector::Rarity, Click::Rarity(Rarity::Normal)),
        ] {
            press_action(
                &mut app,
                |click| matches!(click, Click::Selector(s) if *s == selector),
            );
            press_action(&mut app, |click| match (*click, action) {
                (Click::Slot(a), Click::Slot(b)) => a == b,
                (Click::Equipment(a), Click::Equipment(b)) => a == b,
                (Click::Profile(a), Click::Profile(b)) => a == b,
                (Click::Rarity(a), Click::Rarity(b)) => a == b,
                _ => false,
            });
        }
        assert!(app.world().resource::<View>().ready());
        app.insert_resource(State::new(Phase::CharacterSheet));
        app.update();
        assert!(app.world().resource::<View>().open);
        let world = app.world_mut();
        let head = world
            .query::<(Entity, &Click)>()
            .iter(world)
            .find(|(_, c)| matches!(c, Click::Selector(Selector::Slot)))
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
            .any(|t| t.0.starts_with("Emplacement : Head")));
        assert_eq!(world.resource::<Inventory>().money(), 100000);
        assert_eq!(
            app.world()
                .resource::<View>()
                .item_level(talents::threshold(12)),
            12
        );
        let increase = {
            let world = app.world_mut();
            world
                .query::<(Entity, &Click)>()
                .iter(world)
                .find(|(_, c)| matches!(c, Click::Level(10)))
                .unwrap()
                .0
        };
        app.world_mut()
            .entity_mut(increase)
            .insert(Interaction::Pressed);
        app.update();
        assert_eq!(app.world().resource::<View>().ilvl, Some(22));
        {
            let world = app.world_mut();
            assert_eq!(
                world
                    .query_filtered::<Entity, With<ItemPreview>>()
                    .iter(world)
                    .count(),
                1
            );
            assert_eq!(
                world
                    .query::<&super::super::combat_ui::bags::ReadOnlyItemTip>()
                    .single(world)
                    .unwrap()
                    .item
                    .level,
                22
            );
        }
        assert_eq!(app.world().resource::<Inventory>().money(), 100000);
        press(&mut app, true);
        let view = app.world().resource::<View>();
        assert_eq!(view.result.as_ref().unwrap().level, 22);
        assert_eq!(
            app.world().resource::<Inventory>().money(),
            100000 - price(SlotKind::Head, 22) * 3
        );
        for (rarity, count) in [(Rarity::Normal, 2), (Rarity::Magic, 4), (Rarity::Epic, 6)] {
            press_action(&mut app, |click| {
                matches!(click, Click::Selector(Selector::Rarity))
            });
            press_action(
                &mut app,
                |click| matches!(click, Click::Rarity(r) if *r == rarity),
            );
            let world = app.world_mut();
            assert_eq!(
                world
                    .query::<&Click>()
                    .iter(world)
                    .filter(|c| matches!(c, Click::Dropdown(_)))
                    .count(),
                count
            );
        }
        let money = app.world().resource::<Inventory>().money();
        press_action(&mut app, |click| {
            matches!(click, Click::Selector(Selector::Equipment))
        });
        {
            let world = app.world_mut();
            let types: Vec<_> = world
                .query::<&Click>()
                .iter(world)
                .filter_map(|click| {
                    if let Click::Equipment(kind) = click {
                        Some(*kind)
                    } else {
                        None
                    }
                })
                .collect();
            for kind in [
                EquipmentType::Plate,
                EquipmentType::Mail,
                EquipmentType::Leather,
                EquipmentType::Cloth,
            ] {
                assert!(types.contains(&kind));
            }
            assert_eq!(types.len(), 4);
        }
        press_action(&mut app, |click| {
            matches!(click, Click::Equipment(EquipmentType::Cloth))
        });
        assert_eq!(
            app.world().resource::<View>().equipment,
            EquipmentType::Cloth
        );
        assert_eq!(app.world().resource::<Inventory>().money(), money);
        press_action(&mut app, |click| {
            matches!(click, Click::Selector(Selector::Equipment))
        });
        press_action(&mut app, |click| {
            matches!(click, Click::Equipment(EquipmentType::Plate))
        });
        assert_eq!(
            app.world().resource::<View>().equipment,
            EquipmentType::Plate
        );
        press_action(&mut app, |click| matches!(click, Click::Dropdown(0)));
        assert!(app.world().resource::<View>().custom);
        let first = {
            let world = app.world_mut();
            world
                .query::<&Click>()
                .iter(world)
                .find_map(|c| {
                    if let Click::SelectAffix(0, Some(index)) = c {
                        Some(*index)
                    } else {
                        None
                    }
                })
                .unwrap()
        };
        press_action(
            &mut app,
            |click| matches!(click, Click::SelectAffix(0, Some(index)) if *index == first),
        );
        let selected = app.world().resource::<View>().affixes[0].clone().unwrap();
        press_action(&mut app, |click| matches!(click, Click::Dropdown(1)));
        let choices = {
            let view = app.world().resource::<View>();
            app.world()
                .resource::<Inventory>()
                .craft_affixes(&view.request(), 22)
        };
        let group = &choices.iter().find(|a| a.id == selected).unwrap().group;
        {
            let affix = choices.iter().find(|a| a.id == selected).unwrap();
            let expected = format!("Prefixe : {}", affix.label);
            let world = app.world_mut();
            assert!(world
                .query::<&super::super::combat_ui::bags::ReadOnlyItemTip>()
                .iter(world)
                .any(|tip| tip.extra_lines.iter().any(|(line, _)| *line == expected)));
        }
        {
            let world = app.world_mut();
            for click in world.query::<&Click>().iter(world) {
                if let Click::SelectAffix(1, Some(index)) = click {
                    assert_eq!(choices[*index].kind, super::super::items::AffixKind::Prefix);
                    assert_ne!(&choices[*index].group, group);
                }
            }
        }
        press_action(&mut app, |click| {
            matches!(click, Click::SelectAffix(1, None))
        });
        press_action(&mut app, |click| matches!(click, Click::Dropdown(3)));
        {
            let world = app.world_mut();
            for click in world.query::<&Click>().iter(world) {
                if let Click::SelectAffix(3, Some(index)) = click {
                    assert_eq!(choices[*index].kind, super::super::items::AffixKind::Suffix);
                }
            }
        }
        press_action(&mut app, |click| {
            matches!(click, Click::SelectAffix(3, None))
        });
        press_action(&mut app, |click| matches!(click, Click::Dropdown(0)));
        press_action(&mut app, |click| {
            matches!(click, Click::SelectAffix(0, None))
        });
        assert!(app
            .world()
            .resource::<View>()
            .request()
            .affixes
            .as_ref()
            .unwrap()
            .is_empty());
        assert_eq!(app.world().resource::<Inventory>().money(), money);
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
