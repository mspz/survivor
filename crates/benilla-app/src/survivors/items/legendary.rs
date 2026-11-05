//! Saved unique identities and owner-local combat state.
use super::*;
use crate::survivors::data::{AbilityDef, Kind};
pub(in crate::survivors) mod casting;
mod catalogue;
pub(in crate::survivors) mod effects;
pub(in crate::survivors) mod events;
pub(in crate::survivors) mod milestones;
pub(in crate::survivors) mod movement;
pub(in crate::survivors) mod periodic;
pub(super) use catalogue::DEFINITIONS;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::survivors) struct Roll {
    pub unique: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub milestone: Option<u8>,
}
impl Roll {
    pub fn valid(&self, item: &Item) -> bool {
        definition(self.unique).is_some_and(|d| d.slot == item.slot)
            && if self.unique == 301 {
                self.milestone.is_some_and(|m| m < 81)
            } else {
                self.milestone.is_none()
            }
    }
}
pub(super) fn definition(id: u16) -> Option<&'static catalogue::Definition> {
    DEFINITIONS.iter().find(|d| d.id == id)
}

#[derive(Default)]
pub(in crate::survivors) struct Runtime {
    pub equipped: BTreeMap<u16, Roll>,
    pub levels: BTreeMap<u16, u32>,
    pub events: events::State,
    pub casting: casting::State,
    pub grants: Vec<milestones::Grant>,
    pub grant_spells: BTreeSet<&'static str>,
    pub periodic: periodic::State,
    pub effects: effects::State,
    pub moves: Vec<movement::Move>,
    pub weapon_items: BTreeMap<u64, usize>,
    pub hands: BTreeMap<u64, usize>,
}

const SCHOOLS: u64 = 0xff << 16;
pub(in crate::survivors) fn profile(run: &Run, original: &AbilityDef) -> (AbilityDef, Tags) {
    let mut def = original.clone();
    let runtime = &run.legendary;
    let mut tags = super::super::spell_engine::tags::for_spell(original);
    casting::profile(run, original, &mut def, &mut tags);
    if runtime.has(22) && original.key == "arcane_explosion" {
        def.kind = Kind::Cone;
        tags = Tags((tags.0 & !SCHOOLS) | Tags::FIRE.0);
    }
    if runtime.has(29) && tags.contains(Tags::MELEE | Tags::ATTACK) {
        def.kind = Kind::Bolt;
        def.range = 24.0;
        def.radius = 0.0;
        def.count = 1;
        tags = Tags(
            (tags.0 & !(Tags::MELEE.0 | Tags::ATTACK.0 | Tags::AREA.0))
                | Tags::SPELL.0
                | Tags::PROJECTILE.0,
        );
    }
    if runtime.has(266) && tags.contains(Tags::MELEE) {
        def.kind = Kind::Bolt;
        def.range = 24.0;
        def.radius = 0.0;
        tags = Tags((tags.0 & !(Tags::MELEE.0 | Tags::AREA.0)) | Tags::PROJECTILE.0);
    }
    if tags.contains(Tags::DAMAGE) {
        let school = if runtime.has(96) {
            Some(Tags::PHYSICAL)
        } else if runtime.has(154) {
            Some(if tags.contains(Tags::PHYSICAL) {
                Tags::ARCANE
            } else {
                Tags::PHYSICAL
            })
        } else {
            None
        };
        if let Some(school) = school {
            tags = Tags((tags.0 & !SCHOOLS) | school.0);
        }
    }
    (def, tags)
}
impl Runtime {
    pub fn has(&self, id: u16) -> bool {
        self.equipped.contains_key(&id)
    }
}

pub(super) fn validate_item(item: &Item) -> bool {
    match &item.legendary {
        Some(roll) => item.rarity == Rarity::Legendary && roll.valid(item),
        None => item.rarity != Rarity::Legendary,
    }
}

/// Registration alone cannot make an item drop without an execution adapter.
pub(super) fn supported(id: u16) -> bool {
    definition(id).is_some()
}
pub(super) fn base_allowed(id: u16, class: u8, kind: EquipmentType) -> bool {
    if matches!(id, 47 | 99 | 213 | 277) {
        kind == EquipmentType::Shield
    } else if definition(id).is_some_and(|d| d.slot == SlotKind::OffHand) && kind.one_handed() {
        super::equipment::dual_wield(class)
    } else {
        true
    }
}

pub(super) fn eligible(id: u16, class: u8) -> bool {
    supported(id)
        && catalogue::class_binding(id).is_none_or(|bound| bound == class)
        && match id {
            1 | 22 => class == 8,
            11 => class == 7,
            46 => EquipmentType::TwoHand.can_equip(class),
            _ => true,
        }
}

pub(super) fn titan_grip(save: &Save) -> bool {
    save.equipped.values().any(|id| {
        save.items.iter().any(|item| {
            item.id == *id
                && item.legendary.as_ref().is_some_and(|roll| {
                    matches!(roll.unique, 46 | 279)
                        || roll.unique == 301
                            && roll.milestone.is_some_and(|option| {
                                milestones::Grant { option }.milestone().mechanic
                                    == crate::survivors::talent_design::Mechanic::Titans
                            })
                })
        })
    })
}

pub(super) fn validate_loadout(save: &Save) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    let mut conversions = 0;
    for item in save
        .items
        .iter()
        .filter(|item| save.equipped.values().any(|id| *id == item.id))
    {
        if let Some(roll) = &item.legendary {
            if !seen.insert(roll.unique) {
                return Err("Unique equipped: this legendary is already worn.".into());
            }
            if matches!(roll.unique, 94..=97 | 154) {
                conversions += 1;
            }
        }
    }
    if conversions > 1 {
        return Err("Only one absolute damage conversion can be equipped.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::survivors::{balance, data, spell_engine};

    fn rolled(id: u16) -> Item {
        let d = definition(id).unwrap();
        let mut item = super::super::tests::item(d.slot, vec![]);
        item.name = d.name.into();
        item.rarity = Rarity::Legendary;
        item.legendary = Some(Roll {
            unique: id,
            milestone: None,
        });
        item
    }

    #[test]
    fn identities_reject_unknown_variants_and_wrong_slots() {
        let mut item = rolled(46);
        assert!(validate_item(&item));
        item.slot = SlotKind::Head;
        assert!(!validate_item(&item));
        item = rolled(301);
        assert!(!validate_item(&item));
        for variant in 0..81 {
            item.legendary.as_mut().unwrap().milestone = Some(variant);
            assert!(validate_item(&item));
        }
        item.legendary.as_mut().unwrap().milestone = Some(81);
        assert!(!validate_item(&item));
    }

    #[test]
    fn unique_equipped_and_absolute_conversions_are_exclusive() {
        let mut inventory = Inventory::fixture(vec![rolled(46), rolled(46)]);
        inventory.equip(1, "Ring 1").unwrap();
        assert!(inventory.equip(2, "Ring 2").is_err());
        let mut inventory = Inventory::fixture(vec![rolled(96), rolled(154)]);
        inventory.equip(1, "Chest").unwrap();
        assert!(inventory.equip(2, "Trinket 1").is_err());
    }

    #[test]
    fn item_titans_grants_permission_to_shaman_and_removes_it_without_losing_weapons() {
        let mut weapon = super::super::tests::item(SlotKind::MainHand, vec![]);
        weapon.equipment_type = EquipmentType::TwoHand;
        let mut inventory = Inventory::fixture(vec![rolled(46), weapon.clone(), weapon]);
        inventory.class = 7;
        inventory.equip(1, "Ring 1").unwrap();
        inventory.equip(2, "Main hand").unwrap();
        inventory.equip(3, "Off hand").unwrap();
        let mut run = Run {
            class: &data::CLASSES[5],
            ..default()
        };
        inventory.apply(&mut run);
        let with_item = run.resolve_spell(data::ability("stormstrike"), 1).cooldown;
        inventory.unequip("Ring 1").unwrap();
        inventory.apply(&mut run);
        assert!(run.resolve_spell(data::ability("stormstrike"), 1).cooldown > with_item);
        assert!(!inventory.save.equipped.contains_key("Off hand"));
        assert_eq!(inventory.save.items.len(), 3);
    }

    #[test]
    fn conversion_changes_modifier_selection_before_resolving_damage() {
        let inventory = {
            let mut inventory = Inventory::fixture(vec![rolled(96)]);
            inventory.equip(1, "Chest").unwrap();
            inventory
        };
        let mut run = Run {
            balance: balance::Config::parse("version=1\n[classes]\n[tags]\n").unwrap(),
            ..default()
        };
        inventory.apply(&mut run);
        run.replace_spell_modifiers(
            10,
            [
                Modifier {
                    source: 10,
                    filter: Filter::requiring(Tags::PHYSICAL),
                    change: Change::MoreDamage(1.0),
                },
                Modifier {
                    source: 10,
                    filter: Filter::requiring(Tags::FIRE),
                    change: Change::MoreDamage(99.0),
                },
            ],
        );
        let resolved = run.resolve_spell(data::ability("fireball"), 1);
        assert!(resolved.tags.contains(Tags::PHYSICAL | Tags::SPELL));
        assert!(!resolved.tags.intersects(Tags::FIRE | Tags::ATTACK));
        let raw = spell_engine::resolve(data::ability("fireball"), 1, &run.stats(), &[]);
        assert_eq!(resolved.dmg, raw.dmg * 2.0);
    }

    #[test]
    fn shape_changes_keep_damage_school_and_select_the_new_card_tags() {
        for (id, key, kind, include, exclude) in [
            (
                22,
                "arcane_explosion",
                Kind::Cone,
                Tags::FIRE | Tags::AREA,
                Tags::ARCANE,
            ),
            (
                29,
                "sinister_strike",
                Kind::Bolt,
                Tags::SPELL | Tags::PROJECTILE,
                Tags::ATTACK | Tags::MELEE,
            ),
            (
                266,
                "sinister_strike",
                Kind::Bolt,
                Tags::ATTACK | Tags::PROJECTILE,
                Tags::MELEE | Tags::AREA,
            ),
        ] {
            let mut run = Run::default();
            run.legendary.equipped.insert(
                id,
                Roll {
                    unique: id,
                    milestone: None,
                },
            );
            let (def, tags) = profile(&run, data::ability(key));
            assert_eq!(def.kind, kind);
            assert!(tags.contains(include));
            assert!(!tags.intersects(exclude));
            assert_eq!(run.resolve_spell(data::ability(key), 1).tags, tags);
        }
    }
    #[test]
    fn catalogue_preserves_all_160_accepted_designs() {
        assert_eq!(DEFINITIONS.len(), 160);
        let mut ids = BTreeSet::new();
        let mut names = BTreeSet::new();
        for d in DEFINITIONS {
            assert!(ids.insert(d.id));
            assert!(names.insert(d.name));
            assert!(!d.description.is_empty());
        }
        let approved = include_str!("../../../../../docs/idees-100-legendaires.txt");
        for line in approved.lines().filter(|line| !line.trim().is_empty()) {
            let fields: Vec<_> = line.split('|').map(str::trim).collect();
            let d = definition(fields[0].parse().unwrap()).unwrap();
            assert_eq!(d.name, fields[1]);
            assert_eq!(d.description, fields[3]);
        }
    }
}
