//! Offline inventory -> the client's ordinary player equipment and body compositor.
use super::*;
use crate::survivors::multiplayer;
use crate::{
    entities::CharCreate,
    net::{FieldChanged, Guid, NetEntity, ObjectStore},
};
use benilla_assets::{LockRecover, WorldAssets};
use benilla_formats::{load_creature_catalog, CreatureCatalog};
use benilla_protocol::{
    messages::{ObjectFields, ObjectType},
    EntityKind, ItemInfo,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::survivors) struct Appearance {
    pub display_id: u32,
    pub inventory_type: u32,
    pub item_class: u32,
    pub subclass: u32,
    #[serde(default)]
    pub material: u32,
    #[serde(default)]
    pub sheath: u32,
}
impl Appearance {
    pub(super) fn valid_for(self, slot: SlotKind) -> bool {
        self.display_id > 0
            && self.material <= 8
            && self.sheath <= 7
            && match slot {
                SlotKind::Head => self.inventory_type == 1,
                SlotKind::Shoulders => self.inventory_type == 3,
                SlotKind::Shirt => self.inventory_type == 4,
                SlotKind::Chest => matches!(self.inventory_type, 5 | 20),
                SlotKind::Waist => self.inventory_type == 6,
                SlotKind::Legs => self.inventory_type == 7,
                SlotKind::Feet => self.inventory_type == 8,
                SlotKind::Wrists => self.inventory_type == 9,
                SlotKind::Hands => self.inventory_type == 10,
                SlotKind::Back => self.inventory_type == 16,
                SlotKind::Tabard => self.inventory_type == 19,
                SlotKind::MainHand => matches!(self.inventory_type, 13 | 17 | 21),
                SlotKind::OffHand => matches!(self.inventory_type, 13 | 14 | 22 | 23),
                SlotKind::Ranged => matches!(self.inventory_type, 15 | 25 | 26 | 28),
                _ => false,
            }
    }
}

#[derive(Component, Clone, Copy)]
pub(in crate::survivors) struct HeroLook {
    pub display: u32,
    pub race: u8,
    pub sex: u8,
    pub class: u8,
}
type LookSource = (bool, Option<u64>, u64, u32, u32, u8, bool);
#[derive(Component)]
struct AppliedLook {
    source: LookSource,
    fields: Vec<(u16, u32)>,
}
#[derive(Resource, Default)]
struct VisualCache {
    catalog: Option<CreatureCatalog>,
    attempted: bool,
    templates: std::collections::HashMap<Appearance, u32>,
    menu_key: Option<(Option<u64>, u64)>,
    menu_save: Save,
}
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<VisualCache>()
        .add_message::<FieldChanged>()
        .add_systems(PreUpdate, sync);
}
fn worn_slot(name: &str) -> Option<u8> {
    Some(match name {
        "Head" => 0,
        "Neck" => 1,
        "Shoulders" => 2,
        "Shirt" => 3,
        "Chest" => 4,
        "Waist" => 5,
        "Legs" => 6,
        "Feet" => 7,
        "Wrists" => 8,
        "Hands" => 9,
        "Ring 1" => 10,
        "Ring 2" => 11,
        "Trinket 1" => 12,
        "Trinket 2" => 13,
        "Back" => 14,
        "Main hand" => 15,
        "Off hand" => 16,
        "Ranged" => 17,
        "Tabard" => 18,
        _ => return None,
    })
}
fn worn_appearances(save: &Save, config: &Config, class: u8) -> [Option<Appearance>; 19] {
    let mut result = [None; 19];
    for (name, id) in &save.equipped {
        let Some(slot) = worn_slot(name) else {
            continue;
        };
        let Some(item) = save.items.iter().find(|i| i.id == *id) else {
            continue;
        };
        let Some((_, kind)) = SLOTS.iter().find(|(s, _)| *s == name) else {
            continue;
        };
        let titan = (class == 1 && save.titan_grip || super::legendary::titan_grip(save))
            && item.equipment_type == EquipmentType::TwoHand;
        if !item.usable(class, *kind) && !(titan && *kind == SlotKind::OffHand) {
            continue;
        }
        result[slot as usize] = item
            .appearance
            .or_else(|| config.appearance_for(item, class).map(|a| a.0))
            .map(|mut appearance| {
                // Keep the two-handed model, using the normal dual-wield hand attachments.
                if titan && save.equipped.contains_key("Off hand") {
                    appearance.inventory_type = 13;
                }
                appearance
            });
    }
    result
}
fn body_fields(
    body: u32,
    npc: Option<&benilla_formats::NpcAppearance>,
    entries: [u32; 19],
) -> Vec<(u16, u32)> {
    let customization = npc.map_or(0, |n| {
        u32::from(n.skin)
            | u32::from(n.face) << 8
            | u32::from(n.hair_style) << 16
            | u32::from(n.hair_color) << 24
    });
    let mut pairs = vec![
        (2, 0x19),
        (131, body),
        (132, body),
        (193, customization),
        (194, npc.map_or(0, |n| u32::from(n.facial_hair))),
    ];
    // Clear every slot, including removals and NPC virtual weapons. Entries reference local templates.
    for (slot, entry) in entries.into_iter().enumerate() {
        pairs.push((260 + 12 * slot as u16, entry));
    }
    for index in 37..46 {
        pairs.push((index, 0));
    }
    pairs.push((
        164,
        if entries[15] != 0 || entries[16] != 0 {
            1
        } else if entries[17] != 0 {
            2
        } else {
            0
        },
    ));
    pairs
}
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn sync(
    mut commands: Commands,
    assets: Option<Res<WorldAssets>>,
    create: Option<Res<CharCreate>>,
    inventory: Option<Res<Inventory>>,
    roster: Option<Res<Roster>>,
    phase: Option<Res<State<Phase>>>,
    run: Option<Res<Run>>,
    role: Option<Res<multiplayer::Role>>,
    templates: Option<ResMut<crate::items::Items>>,
    mut cache: ResMut<VisualCache>,
    mut heroes: Query<(
        Entity,
        &HeroLook,
        &Guid,
        &mut NetEntity,
        &mut ObjectStore,
        &mut Visibility,
        Option<&AppliedLook>,
    )>,
    mut changes: MessageWriter<FieldChanged>,
) {
    if heroes.is_empty() {
        return;
    }
    let (Some(create), Some(inventory), Some(mut templates)) = (create, inventory, templates)
    else {
        return;
    };
    if !cache.attempted {
        let Some(assets) = assets else { return };
        cache.attempted = true;
        match load_creature_catalog(&mut assets.chain.lock_recover()) {
            Ok(catalog) => cache.catalog = Some(catalog),
            Err(error) => warn!("Survivors faces: {error}; using default character customization"),
        };
    }
    let menu = phase.as_ref().is_some_and(|p| *p.get() == Phase::Menu);
    let character = if menu {
        roster
            .as_ref()
            .filter(|r| !r.creating)
            .and_then(|r| r.selected)
    } else {
        inventory.character
    };
    if menu && cache.menu_key != Some((character, inventory.revision)) {
        cache.menu_key = Some((character, inventory.revision));
        cache.menu_save = character
            .and_then(|id| {
                if inventory.character == Some(id) {
                    return Some(inventory.save.clone());
                }
                let path = crate::local_state::home()?.join(format!("characters/{id}-items.toml"));
                match crate::local_state::read_to_string(&path) {
                    Ok(text) => match Inventory::decode(&text) {
                        Ok(save) => Some(save),
                        Err(e) => {
                            warn!("Character preview inventory: {e}");
                            None
                        }
                    },
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                    Err(e) => {
                        warn!("Character preview inventory: {e}");
                        None
                    }
                }
            })
            .unwrap_or_default();
    }
    for (entity, look, guid, mut net, mut store, mut visibility, applied) in &mut heroes {
        let Some(body) = create.0.body_display(look.race, look.sex) else {
            continue;
        };
        let cat = !menu
            && if role
                .as_ref()
                .is_some_and(|r| **r == multiplayer::Role::Guest)
            {
                store.0.unit_shapeshift_form() == 1
            } else {
                run.as_ref().is_some_and(|r| super::super::forms::cat(r))
            };
        let source = (
            menu,
            character,
            inventory.revision,
            body,
            look.display,
            look.class,
            cat,
        );
        if applied.is_some_and(|a| a.source == source) {
            continue;
        }
        let save = if menu {
            &cache.menu_save
        } else {
            &inventory.save
        };
        let worn = worn_appearances(save, &inventory.config, look.class);
        let mut entries = [0; 19];
        for (slot, appearance) in worn.into_iter().enumerate() {
            let Some(appearance) = appearance else {
                continue;
            };
            let entry = if let Some(entry) = cache.templates.get(&appearance) {
                *entry
            } else {
                let entry = 0x6000_0000 + cache.templates.len() as u32;
                templates.insert_template(entry, Some(template(appearance)));
                cache.templates.insert(appearance, entry);
                entry
            };
            entries[slot] = entry;
        }
        let npc = cache
            .catalog
            .as_ref()
            .and_then(|c| c.model(look.display))
            .and_then(|m| m.npc_appearance);
        let mut fields = body_fields(body, npc.as_ref(), entries);
        let display = if cat {
            super::super::forms::CAT_DISPLAY
        } else {
            body
        };
        fields.iter_mut().find(|(i, _)| *i == 131).unwrap().1 = display;
        if applied.is_some_and(|a| a.fields == fields) {
            commands
                .entity(entity)
                .insert(AppliedLook { source, fields });
            continue;
        }
        net.kind = EntityKind::Player;
        net.display_id = Some(display);
        store
            .0
            .merge_diff(ObjectFields::from_pairs(&fields), |index, old, new| {
                changes.write(FieldChanged {
                    entity,
                    guid: guid.0,
                    kind: ObjectType::Player,
                    index,
                    old,
                    new,
                });
            });
        *visibility = Visibility::Inherited;
        commands
            .entity(entity)
            .insert(AppliedLook { source, fields });
    }
}

pub(in crate::survivors) fn template(a: Appearance) -> ItemInfo {
    ItemInfo {
        class: a.item_class,
        subclass: a.subclass,
        name: "Survivors equipment appearance".into(),
        display_info_id: a.display_id,
        quality: 1,
        flags: 0,
        buy_price: 0,
        sell_price: 0,
        inventory_type: a.inventory_type,
        allowable_class: -1,
        allowable_race: -1,
        item_level: 1,
        required_level: 0,
        required_skill: 0,
        required_skill_rank: 0,
        required_spell: 0,
        required_honor_rank: 0,
        required_city_rank: 0,
        required_rep_faction: 0,
        required_rep_rank: 0,
        max_count: 0,
        stackable: 1,
        container_slots: 0,
        stats: Vec::new(),
        damages: Vec::new(),
        dmg_min: 0.0,
        dmg_max: 0.0,
        dmg_type: 0,
        armor: 0,
        resistances: [0; 6],
        delay_ms: 0,
        ammo_type: 0,
        ranged_mod_range: 0.0,
        spells: Vec::new(),
        spell_charges_0: 0,
        use_spell: None,
        bonding: 0,
        description: String::new(),
        page_text: 0,
        language_id: 0,
        page_material: 0,
        start_quest: 0,
        lock_id: 0,
        material: a.material,
        sheath: a.sheath,
        random_property: 0,
        block: 0,
        item_set: 0,
        max_durability: 0,
        area: 0,
        map: 0,
        bag_family: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn data(path: &std::path::Path) -> (benilla_formats::CharCreateCatalog, CreatureCatalog) {
        let mut chain = benilla_formats::open_chain(path).unwrap();
        (
            benilla_formats::CharCreateCatalog::load(&mut chain).unwrap(),
            load_creature_catalog(&mut chain).unwrap(),
        )
    }
    fn rolled(id: &str, class: u8) -> Item {
        let config = Config::builtin();
        let mut rng = Rng(8765);
        for _ in 0..10000 {
            let item = config.generate_for_class(10, class, &mut rng).unwrap();
            if item.base == id {
                return item;
            }
        }
        panic!("Base {id} never generated")
    }
    #[test]
    fn existing_appearance_references_are_real_models_and_human_starter_garments() {
        let path = benilla_formats::wow_data_or_skip!();
        let mut chain = benilla_formats::open_chain(&path).unwrap();
        let create = benilla_formats::CharCreateCatalog::load(&mut chain).unwrap();
        let displays = benilla_formats::load_item_display_catalog(&mut chain).unwrap();
        let config = Config::builtin();
        for base in &config.bases {
            if matches!(
                base.slot,
                SlotKind::Neck | SlotKind::Ring | SlotKind::Trinket
            ) {
                assert!(base.appearance.is_none());
                continue;
            }
            let a = base.appearance.expect("Visible base needs an appearance");
            assert!(a.valid_for(base.slot), "{}", base.id);
            let display = displays
                .get(a.display_id)
                .expect("Real ItemDisplayInfo reference");
            match base.slot {
                SlotKind::MainHand | SlotKind::OffHand | SlotKind::Ranged | SlotKind::Head => {
                    assert!(display.model[0].is_some(), "{}", base.id)
                }
                SlotKind::Shoulders => {
                    assert!(display.model.iter().all(Option::is_some), "{}", base.id)
                }
                SlotKind::Back => assert!(display.model_texture[0].is_some(), "Cape texture"),
                _ => assert!(
                    display.region_textures.iter().any(Option::is_some),
                    "{}",
                    base.id
                ),
            }
            let directory = match base.slot {
                SlotKind::Head => "Head",
                SlotKind::Shoulders => "Shoulder",
                SlotKind::OffHand if a.inventory_type == 14 => "Shield",
                SlotKind::MainHand | SlotKind::OffHand | SlotKind::Ranged => "Weapon",
                SlotKind::Back => "Cape",
                _ => "",
            };
            for model in display.model.iter().flatten() {
                if base.slot == SlotKind::Head {
                    for sex in ["M", "F"] {
                        let path = format!(
                            "Item\\ObjectComponents\\Head\\{}_Hu{sex}.m2",
                            model.trim_end_matches(".m2")
                        );
                        assert!(chain.read_file(&path).is_ok(), "{} missing {path}", base.id);
                    }
                } else {
                    let path = format!("Item\\ObjectComponents\\{directory}\\{model}");
                    assert!(chain.read_file(&path).is_ok(), "{} missing {path}", base.id);
                }
            }
            for texture in display.model_texture.iter().flatten() {
                let path = format!("Item\\ObjectComponents\\{directory}\\{texture}.blp");
                assert!(chain.read_file(&path).is_ok(), "{} missing {path}", base.id);
            }
        }
        for (id, class) in [
            ("sword", 1),
            ("shield", 1),
            ("shirt", 1),
            ("staff", 8),
            ("cloth_chest", 8),
            ("cloth_leggings", 8),
            ("cloth_boots", 8),
            ("dagger", 4),
            ("leather_leggings", 4),
            ("leather_boots", 4),
            ("mace_two_hand", 2),
        ] {
            let display = config
                .bases
                .iter()
                .find(|b| b.id == id)
                .unwrap()
                .appearance
                .unwrap()
                .display_id;
            assert!(
                create
                    .start_outfit(1, class, 0)
                    .iter()
                    .any(|a| a.display_id == display),
                "{id} is not the selected human starter appearance"
            );
        }
    }
    #[test]
    fn live_player_fields_follow_equips_removals_two_handers_and_selected_character() {
        let path = benilla_formats::wow_data_or_skip!();
        let (create, catalog) = data(&path);
        let original = catalog.model(1300).unwrap().npc_appearance.unwrap();
        let body = create.body_display(original.race, original.sex).unwrap();
        let mut inv = Inventory::fixture(vec![
            rolled("sword", 2),
            rolled("shield", 2),
            rolled("mail_chest", 2),
            rolled("staff", 2),
        ]);
        inv.class = 2;
        inv.equip(1, "Main hand").unwrap();
        inv.equip(2, "Off hand").unwrap();
        inv.equip(3, "Chest").unwrap();
        let mut roster = Roster::default();
        roster.active = Some(1);
        roster.selected = Some(1);
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(State::new(Phase::Hub))
            .insert_resource(CharCreate(create))
            .insert_resource(VisualCache {
                catalog: Some(catalog),
                attempted: true,
                ..default()
            })
            .insert_resource(inv)
            .insert_resource(roster)
            .init_resource::<crate::items::Items>();
        plugin(&mut app);
        let entity = app
            .world_mut()
            .spawn((
                HeroLook {
                    display: 1300,
                    race: original.race,
                    sex: original.sex,
                    class: 2,
                },
                Guid(123),
                NetEntity {
                    kind: EntityKind::Unit,
                    display_id: Some(1300),
                    scale: 1.0,
                },
                ObjectStore(ObjectFields::from_pairs(&[
                    (22, 100),
                    (37, 999),
                    (
                        36,
                        u32::from(original.race) | 2 << 8 | u32::from(original.sex) << 16,
                    ),
                ])),
                Visibility::Hidden,
            ))
            .id();
        let other = app
            .world_mut()
            .spawn((
                NetEntity {
                    kind: EntityKind::Unit,
                    display_id: Some(1300),
                    scale: 1.0,
                },
                ObjectStore(ObjectFields::from_pairs(&[(37, 777)])),
            ))
            .id();
        app.update();
        let fields = &app.world().get::<ObjectStore>(entity).unwrap().0;
        assert_eq!(
            app.world().get::<NetEntity>(entity).unwrap().kind,
            EntityKind::Player
        );
        assert_eq!(fields.unit_displayid(), Some(body as i32));
        assert_eq!(
            fields.player_bytes(),
            Some(
                u32::from(original.skin)
                    | u32::from(original.face) << 8
                    | u32::from(original.hair_style) << 16
                    | u32::from(original.hair_color) << 24
            )
        );
        assert_eq!(fields.unit_virtual_item_display(0), Some(0));
        let entry = fields.player_visible_item_entry(15).unwrap();
        assert_eq!(
            app.world()
                .resource::<crate::items::Items>()
                .template_cached(entry)
                .unwrap()
                .display_info_id,
            1542
        );
        assert_ne!(fields.player_visible_item_entry(16), None);
        assert_ne!(fields.player_visible_item_entry(4), None);
        assert_eq!(
            *app.world().get::<Visibility>(entity).unwrap(),
            Visibility::Inherited
        );
        assert_eq!(
            app.world()
                .get::<ObjectStore>(other)
                .unwrap()
                .0
                .unit_virtual_item_display(0),
            Some(777)
        );
        {
            let mut inv = app.world_mut().resource_mut::<Inventory>();
            inv.equip(4, "Main hand").unwrap();
            inv.unequip("Chest").unwrap();
        }
        app.update();
        let fields = &app.world().get::<ObjectStore>(entity).unwrap().0;
        let entry = fields.player_visible_item_entry(15).unwrap();
        assert_eq!(
            app.world()
                .resource::<crate::items::Items>()
                .template_cached(entry)
                .unwrap()
                .display_info_id,
            472
        );
        assert_eq!(fields.player_visible_item_entry(16), None);
        assert_eq!(fields.player_visible_item_entry(4), None);
        app.world_mut().insert_resource(State::new(Phase::Menu));
        app.world_mut().resource_mut::<Roster>().selected = Some(2);
        app.update();
        assert_eq!(
            app.world()
                .get::<ObjectStore>(entity)
                .unwrap()
                .0
                .player_visible_item_entry(15),
            None,
            "Another character must not inherit the active character's equipment"
        );
    }

    #[test]
    fn titan_grip_keeps_both_two_handed_models_in_the_dual_wield_attachments() {
        let mut weapon = super::super::tests::item(
            SlotKind::MainHand,
            vec![super::super::tests::affix(
                Effect::Stamina,
                20.0,
                AffixKind::Prefix,
                vec![],
            )],
        );
        weapon.equipment_type = EquipmentType::TwoHand;
        weapon.appearance = Some(Appearance {
            display_id: 12274,
            inventory_type: 17,
            item_class: 2,
            subclass: 8,
            material: 1,
            sheath: 1,
        });
        let mut inv = Inventory::fixture(vec![weapon.clone(), weapon]);
        inv.save.titan_grip = true;
        inv.native_titan_grip = true;
        inv.equip(1, "Main hand").unwrap();
        inv.equip(2, "Off hand").unwrap();
        let worn = worn_appearances(&inv.save, &inv.config, 1);
        assert_eq!(worn[15].unwrap().inventory_type, 13);
        assert_eq!(worn[16].unwrap().inventory_type, 13);
        assert_eq!(worn[16].unwrap().display_id, 12274);
        assert_eq!(inv.item(2).unwrap().appearance.unwrap().inventory_type, 17);
    }
    #[test]
    fn old_tables_and_saved_rolls_receive_appearance_metadata_without_changing_stats() {
        let config = Config::builtin();
        let mut item = rolled("sword", 2);
        let mut serialized = toml::Value::try_from(&item).unwrap();
        serialized.as_table_mut().unwrap().remove("appearance");
        item = toml::from_str(&toml::to_string(&serialized).unwrap()).unwrap();
        assert!(item.appearance.is_none());
        let recovered = config.appearance_for(&item, 2).unwrap().0;
        assert_eq!(recovered.display_id, 1542);
        let mut inv = Inventory::fixture(vec![item]);
        inv.class = 2;
        inv.equip(1, "Main hand").unwrap();
        assert_eq!(
            worn_appearances(&inv.save, &config, 2)[15].unwrap(),
            recovered
        );
        let mut base_table: toml::Value = toml::from_str(config::DEFAULT_BASES).unwrap();
        for base in base_table["bases"].as_array_mut().unwrap() {
            base.as_table_mut().unwrap().remove("appearance");
        }
        let mut table: toml::Value =
            toml::from_str(&toml::to_string(&base_table).unwrap()).unwrap();
        let affix_table: toml::Value = toml::from_str(include_str!(
            "../../../../../config/survivors-item-affixes.toml"
        ))
        .unwrap();
        table
            .as_table_mut()
            .unwrap()
            .insert("affixes".into(), affix_table["affixes"].clone());
        let migrated = Config::parse(&toml::to_string(&table).unwrap()).unwrap();
        assert_eq!(migrated.boss_drop_chance, config.boss_drop_chance);
        assert_eq!(
            migrated
                .bases
                .iter()
                .find(|b| b.id == "sword")
                .unwrap()
                .appearance
                .unwrap(),
            recovered
        );
        item_invalid_appearance_is_rejected(recovered);
    }
    fn item_invalid_appearance_is_rejected(mut appearance: Appearance) {
        let mut item = rolled("sword", 2);
        appearance.display_id = 0;
        item.appearance = Some(appearance);
        let inv = Inventory::fixture(vec![item]);
        assert!(Inventory::decode(&toml::to_string(&inv.save).unwrap()).is_err());
    }
}
