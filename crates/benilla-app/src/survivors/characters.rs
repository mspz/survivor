//! Offline character roster, character-scoped progression and the hub sanctuary.
use super::{data, hero, hud, progression, talents, Owned, Phase, Run, Selection};
use bevy::{input::keyboard::KeyboardInput, prelude::*};
use serde::{Deserialize, Serialize};
mod expeditions;
mod frontend;
use super::combat_ui::skin;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Character {
    pub id: u64,
    pub name: String,
    pub class: usize,
    pub display: u32,
    pub race: u8,
    pub sex: u8,
    pub xp: u64,
    /// Older characters keep their existing inventory; only newly created ones get the kit.
    #[serde(default)]
    pub starter_equipment: bool,
}
#[derive(Clone, Serialize, Deserialize)]
struct Save {
    version: u32,
    next_id: u64,
    characters: Vec<Character>,
    /// None identifies a legacy roster whose post-60 XP has not been pooled yet.
    #[serde(default)]
    paragon_xp: Option<u64>,
}
#[derive(Resource)]
pub(super) struct Roster {
    save: Save,
    pub selected: Option<u64>,
    pub active: Option<u64>,
    pub creating: bool,
    pub name: String,
    pub error: String,
    pub confirm: bool,
    pub expeditions: bool,
    blocked: bool,
    deleting: Option<u64>,
    delete_name: String,
}
impl Default for Roster {
    fn default() -> Self {
        let mut roster = Self {
            save: Save {
                version: 1,
                next_id: 1,
                characters: Vec::new(),
                paragon_xp: Some(0),
            },
            selected: None,
            active: None,
            creating: false,
            name: String::new(),
            error: String::new(),
            confirm: false,
            expeditions: false,
            blocked: false,
            deleting: None,
            delete_name: String::new(),
        };
        if let Some(home) = crate::local_state::home() {
            match crate::local_state::read_to_string(&home.join("survivors-characters.toml")) {
                Ok(text) => match Self::decode(&text) {
                    Ok(save) => {
                        roster.selected = save.characters.first().map(|c| c.id);
                        roster.save = save;
                        // Recover a talent-file write that completed before the roster write.
                        let ids: Vec<_> = roster.save.characters.iter().map(|c| c.id).collect();
                        for id in ids {
                            let xp = talents::Account::load_character(id).xp;
                            if let Err(error) = roster.record_xp(id, xp) {
                                roster.error = format!("Could not recover character XP: {error}");
                            }
                        }
                    }
                    Err(error) => {
                        roster.error = format!("Cannot load characters: {error}");
                        roster.blocked = true;
                    }
                },
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                    roster.error = error.to_string();
                    roster.blocked = true;
                }
                _ => {}
            }
        }
        roster
    }
}
impl Roster {
    #[cfg(test)]
    pub(in crate::survivors) fn paragon_fixture(character_xp: u64) -> Self {
        let mut roster = Self::default();
        roster.save = Save {
            version: 1, next_id: 2,
            characters: vec![Character {
                id: 1, name: "Arthas".into(), class: 0, display: 3167, race: 1, sex: 0,
                xp: character_xp, starter_equipment: false,
            }],
            paragon_xp: Some(character_xp.saturating_sub(talents::threshold(60))),
        };
        roster.blocked = false;
        roster
    }
    fn decode(text: &str) -> Result<Save, String> {
        let mut save: Save = toml::from_str(text).map_err(|e| e.to_string())?;
        if save.version != 1 || save.next_id == 0 || save.characters.len() > 10 {
            return Err("unsupported roster".into());
        }
        let mut ids = Vec::new();
        let mut names = Vec::new();
        for c in &mut save.characters {
            if c.id == 0
                || c.id >= save.next_id
                || c.class >= data::CLASSES.len()
                || !valid_name(&c.name)
                || !(1..=8).contains(&c.race)
                || c.sex > 1
                || c.display == 0
                || ids.contains(&c.id)
                || names.contains(&c.name.to_lowercase())
            {
                return Err("invalid character record".into());
            }
            ids.push(c.id);
            names.push(c.name.to_lowercase());
            if is_test_character(&c.name) {
                c.xp = c.xp.max(talents::threshold(60));
            }
        }
        if save.paragon_xp.is_none() {
            save.paragon_xp = Some(save.characters.iter().fold(0_u64, |total, c| {
                total.saturating_add(c.xp.saturating_sub(talents::threshold(60)))
            }));
        }
        Ok(save)
    }
    fn persist(save: &Save) -> Result<(), String> {
        let Some(home) = crate::local_state::home() else {
            return Ok(());
        };
        let text = toml::to_string_pretty(save).map_err(|e| e.to_string())?;
        crate::local_state::write_queued(&home.join("survivors-characters.toml"), &text)
            .map_err(|e| e.to_string())
    }
    pub fn current(&self) -> Option<&Character> {
        self.active
            .and_then(|id| self.save.characters.iter().find(|c| c.id == id))
    }
    pub(in crate::survivors) fn paragon_xp(&self) -> u64 {
        self.save.paragon_xp.unwrap_or(0)
    }
    pub(in crate::survivors) fn record_xp(&mut self, id: u64, xp: u64) -> Result<(), String> {
        if self.blocked { return Err("Character save is blocked.".into()); }
        let Some(index) = self.save.characters.iter().position(|c| c.id == id) else { return Ok(()); };
        let old = self.save.characters[index].xp;
        let xp = xp.max(old);
        if xp == old { return Ok(()); }
        let cap = talents::threshold(60);
        let gained = xp.saturating_sub(cap).saturating_sub(old.saturating_sub(cap));
        let mut save = self.save.clone();
        save.characters[index].xp = xp;
        save.paragon_xp = Some(self.paragon_xp().saturating_add(gained));
        Self::persist(&save)?;
        self.save = save;
        Ok(())
    }
    pub fn preview_level(&self) -> u32 {
        self.selected
            .and_then(|id| self.save.characters.iter().find(|c| c.id == id))
            .map_or(1, |c| talents::level(c.xp))
    }
    fn create(&mut self, class: usize, look: (u32, u8, u8), xp: u64) -> Result<Character, String> {
        if self.blocked {
            return Err("Fix the character save before creating a character.".into());
        }
        if self.save.characters.len() >= 10 {
            return Err("Maximum of 10 characters reached.".into());
        }
        let name = self.name.trim();
        if !valid_name(name) {
            return Err("Name: 2-16 letters, or TEST_ followed by a test name.".into());
        }
        if self
            .save
            .characters
            .iter()
            .any(|c| c.name.to_lowercase() == name.to_lowercase())
        {
            return Err("That name already exists.".into());
        }
        if class >= data::CLASSES.len() || look.0 == 0 || !(1..=8).contains(&look.1) || look.2 > 1 {
            return Err("Choose an available class and appearance.".into());
        }
        let c = Character {
            id: self.save.next_id,
            name: name.to_owned(),
            class,
            display: look.0,
            race: look.1,
            sex: look.2,
            xp: if is_test_character(name) {
                talents::threshold(60)
            } else {
                xp
            },
            starter_equipment: true,
        };
        let mut save = self.save.clone();
        // The first-ever character can import the pre-roster talent save's XP once.
        if save.next_id == 1 {
            save.paragon_xp = Some(self.paragon_xp().saturating_add(c.xp.saturating_sub(talents::threshold(60))));
        }
        save.next_id = save
            .next_id
            .checked_add(1)
            .ok_or("Character id limit reached")?;
        save.characters.push(c.clone());
        Self::persist(&save)?;
        self.save = save;
        self.selected = Some(c.id);
        Ok(c)
    }
    fn delete(&mut self, id: u64, typed_name: &str) -> Result<(), String> {
        if self.blocked || self.active.is_some() {
            return Err("Log out before deleting a character.".into());
        }
        let character = self
            .save
            .characters
            .iter()
            .find(|c| c.id == id)
            .ok_or("Character no longer exists.")?;
        if typed_name.trim() != character.name {
            return Err("Type the character's exact name to confirm deletion.".into());
        }
        let mut save = self.save.clone();
        save.characters.retain(|c| c.id != id);
        // Commit the roster first. Never recycle ids, even when the last character is removed.
        Self::persist(&save)?;
        self.save = save;
        self.selected = self.save.characters.first().map(|c| c.id);
        self.deleting = None;
        self.delete_name.clear();
        if let Some(home) = crate::local_state::home() {
            let path = home.join(format!("characters/{id}-items.toml"));
            if let Err(error) = crate::local_state::remove_file(&path) {
                warn!(
                    "Deleted character {id}; could not remove {}: {error}",
                    path.display()
                );
            }
            for suffix in ["talents", "zones"] {
                let path = home.join(format!("characters/{id}-{suffix}.txt"));
                if let Err(error) = crate::local_state::remove_file(&path) {
                    warn!(
                        "Deleted character {id}; could not remove {}: {error}",
                        path.display()
                    );
                }
            }
        }
        Ok(())
    }
}
pub(super) fn is_test_character(name: &str) -> bool {
    name.starts_with("TEST_")
}
fn valid_name(name: &str) -> bool {
    if let Some(suffix) = name.strip_prefix("TEST_") {
        return !suffix.is_empty()
            && name.chars().count() <= 16
            && suffix
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '_' | '\'' | '-'));
    }
    (2..=16).contains(&name.chars().count())
        && name
            .chars()
            .all(|c| c.is_alphabetic() || c == '\'' || c == '-')
        && name.chars().next().is_some_and(char::is_alphabetic)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paragon_pool_migrates_once_and_keeps_both_characters_contributions_after_deletion() {
        let cap = talents::threshold(60);
        let mut roster = roster();
        roster.save.characters[0].xp = cap + 125;
        roster.save.characters[1].xp = cap + 200;
        roster.save.paragon_xp = None;
        roster.save = Roster::decode(&toml::to_string(&roster.save).unwrap()).unwrap();
        assert_eq!(roster.paragon_xp(), 325);
        roster.save = Roster::decode(&toml::to_string(&roster.save).unwrap()).unwrap();
        assert_eq!(roster.paragon_xp(), 325);
        roster.record_xp(1, cap + 225).unwrap();
        roster.record_xp(1, cap + 225).unwrap();
        roster.record_xp(2, cap + 300).unwrap();
        assert_eq!(roster.paragon_xp(), 525);
        roster.delete(1, "Arthas").unwrap();
        assert_eq!(roster.paragon_xp(), 525);
        roster.name = "Thrall".into();
        let alt = roster.create(0, (3167, 1, 0), 0).unwrap();
        assert_eq!(talents::level(alt.xp), 1);
        assert_eq!(roster.paragon_xp(), 525);
        roster.record_xp(alt.id, cap + 100).unwrap();
        assert_eq!(roster.paragon_xp(), 625);
        let restored = Roster::decode(&toml::to_string(&roster.save).unwrap()).unwrap();
        assert_eq!(restored.paragon_xp, Some(625));
    }
    #[test]
    fn roster_preserves_paragon_xp_for_normal_and_test_characters_independently() {
        let mut roster = roster();
        let xp = talents::threshold(60) + 125;
        roster.save.characters[0].xp = xp;
        roster.save.characters[0].name = "TEST_Paragon".into();
        let loaded = Roster::decode(&toml::to_string(&roster.save).unwrap()).unwrap();
        assert_eq!(loaded.characters[0].xp, xp);
        assert_eq!(loaded.characters[1].xp, 0);
        roster.save.characters[0].name = "Arthas".into();
        let loaded = Roster::decode(&toml::to_string(&roster.save).unwrap()).unwrap();
        assert_eq!(loaded.characters[0].xp, xp);
    }
    #[test]
    fn test_prefix_is_typable_and_grants_level_sixty_and_the_private_expedition() {
        let mut roster = roster();
        assert!(valid_name("TEST_Warrior"));
        assert!(valid_name("TEST_01"));
        assert!(!valid_name("TEST_"));
        assert!(!valid_name("Normal_Name"));
        roster.name = "TEST_Fury".to_owned();
        let test = roster.create(0, (3167, 1, 0), 0).unwrap();
        assert_eq!(talents::level(test.xp), 60);
        let mut selection = Selection::default();
        let mut account = talents::Account::fresh();
        activate_selected(&mut roster, &mut selection, &mut account).unwrap();
        assert_eq!(talents::level(account.xp), 60);
        assert_eq!(account.available(0), 59);
        assert!(selection.allows_map(data::TRAINING_MAP));
        assert!(data::CAMPAIGN.iter().all(|&map| selection.allows_map(map)));
        assert!(!selection.allows_map(data::MAPS.len()));
        assert!(!selection.allows_map(0)); // Retired maps stay outside the campaign.
        assert!(!progression::zone_order().contains(&data::TRAINING_MAP));
        let mut legacy = roster.save.clone();
        legacy.characters.last_mut().unwrap().xp = 0;
        let decoded = Roster::decode(&toml::to_string(&legacy).unwrap()).unwrap();
        assert_eq!(talents::level(decoded.characters.last().unwrap().xp), 60);
        roster.selected = Some(roster.save.characters[0].id);
        activate_selected(&mut roster, &mut selection, &mut account).unwrap();
        assert!(!selection.allows_map(data::TRAINING_MAP));
        selection.progress = progression::Progress::fresh(999);
        assert!(selection.allows_map(data::CAMPAIGN[0]));
        assert!(!selection.allows_map(data::CAMPAIGN[1]));
        assert_ne!(talents::level(account.xp), 60);
    }
    fn roster() -> Roster {
        let mut roster = Roster::default();
        roster.name = "Arthas".into();
        roster.create(0, (3167, 1, 0), 100).unwrap();
        roster.name = "Jaina".into();
        roster.create(6, (3167, 1, 1), 0).unwrap();
        roster
    }
    #[test]
    fn characters_have_unique_identity_and_independent_metadata() {
        let mut roster = roster();
        assert_ne!(roster.save.characters[0].id, roster.save.characters[1].id);
        assert_eq!(roster.save.characters[0].xp, 100);
        assert_eq!(roster.save.characters[1].xp, 0);
        let text = toml::to_string_pretty(&roster.save).unwrap();
        let loaded = Roster::decode(&text).unwrap();
        assert!(loaded.characters.iter().all(|c| c.starter_equipment));
        let legacy = text.replace("starter_equipment = true\n", "");
        assert!(Roster::decode(&legacy)
            .unwrap()
            .characters
            .iter()
            .all(|c| !c.starter_equipment));
        assert_eq!(loaded.characters[1].name, "Jaina");
        assert_eq!(loaded.characters[1].class, 6);
        roster.name = "arthas".into();
        assert!(roster.create(0, (3167, 1, 0), 0).is_err());
        for name in ["A", "two words", "Bad123", ""] {
            roster.name = name.into();
            assert!(roster.create(0, (3167, 1, 0), 0).is_err());
        }
        let corrupt = text.replace("id = 2", "id = 1");
        assert!(Roster::decode(&corrupt).is_err());
    }
    #[test]
    fn deletion_requires_the_exact_name_and_never_reuses_character_ids() {
        let mut roster = roster();
        assert!(roster.delete(1, "arthas").is_err());
        assert_eq!(roster.save.characters.len(), 2);
        roster.active = Some(1);
        assert!(roster.delete(1, "Arthas").is_err());
        roster.active = None;
        roster.delete(1, "Arthas").unwrap();
        assert_eq!(roster.selected, Some(2));
        assert_eq!(roster.save.characters[0].name, "Jaina");
        roster.delete(2, "Jaina").unwrap();
        assert!(roster.save.characters.is_empty());
        assert_eq!(roster.selected, None);
        assert_eq!(roster.save.next_id, 3);
        let text = toml::to_string_pretty(&roster.save).unwrap();
        assert_eq!(Roster::decode(&text).unwrap().next_id, 3);
        roster.name = "Thrall".into();
        assert_eq!(roster.create(0, (3167, 2, 0), 0).unwrap().id, 3);
    }
    #[test]
    fn confirmation_blocks_world_entry_and_recreating_after_deletion_does_not_import_legacy_xp() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<Phase>()
            .insert_resource(roster())
            .init_resource::<Selection>()
            .init_resource::<Run>()
            .init_resource::<talents::Account>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<KeyboardInput>()
            .add_systems(Update, (input, update_record).chain());
        click(&mut app, Click::Select(1));
        click(&mut app, Click::Delete);
        click(&mut app, Click::Enter);
        assert_eq!(*app.world().resource::<State<Phase>>().get(), Phase::Menu);
        click(&mut app, Click::Select(2));
        assert_eq!(app.world().resource::<Roster>().selected, Some(1));
        click(&mut app, Click::ConfirmDelete);
        assert_eq!(app.world().resource::<Roster>().save.characters.len(), 2);
        click(&mut app, Click::Cancel);
        assert_eq!(app.world().resource::<Roster>().deleting, None);
        click(&mut app, Click::Delete);
        app.world_mut().resource_mut::<Roster>().delete_name = "Arthas".into();
        click(&mut app, Click::ConfirmDelete);
        assert_eq!(app.world().resource::<Roster>().selected, Some(2));
        click(&mut app, Click::Delete);
        app.world_mut().resource_mut::<Roster>().delete_name = "Jaina".into();
        click(&mut app, Click::ConfirmDelete);
        assert!(app.world().resource::<Roster>().save.characters.is_empty());
        assert!(app.world().resource::<Selection>().class.is_none());
        app.world_mut().resource_mut::<talents::Account>().xp = talents::threshold(60);
        {
            let mut roster = app.world_mut().resource_mut::<Roster>();
            roster.creating = true;
            roster.name = "Fresh".into();
        }
        {
            let mut selection = app.world_mut().resource_mut::<Selection>();
            selection.class = Some(0);
            selection.looks = vec![(3167, 1, 0)];
            selection.look = Some(0);
        }
        click(&mut app, Click::Enter);
        assert_eq!(app.world().resource::<Roster>().active, Some(3));
        assert_eq!(app.world().resource::<talents::Account>().xp, 0);
    }
    fn click(app: &mut App, action: Click) {
        let entity = app.world_mut().spawn((Interaction::Pressed, action)).id();
        app.update();
        app.world_mut().entity_mut(entity).despawn();
        app.update();
    }
    #[test]
    fn entering_world_starts_in_hub_then_expeditions_return_to_the_same_character() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<Phase>()
            .insert_resource(roster())
            .init_resource::<Selection>()
            .init_resource::<Run>()
            .init_resource::<talents::Account>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<KeyboardInput>()
            .init_resource::<super::super::abilities::PendingHits>()
            .init_resource::<super::super::enemies::Spawner>()
            .add_systems(Update, (input, update_record).chain())
            .add_systems(
                OnEnter(Phase::Hub),
                (super::super::reset_run, arrive).chain().run_if(
                    |selection: Res<Selection>, run: Res<Run>| {
                        !selection.in_hub || run.abilities.is_empty()
                    },
                ),
            )
            .add_systems(OnEnter(Phase::Menu), super::super::reset_run)
            .add_systems(OnEnter(Phase::Playing), talents::prepare)
            .add_systems(OnExit(Phase::Playing), talents::flush);
        click(&mut app, Click::Select(1));
        click(&mut app, Click::Enter);
        assert_eq!(*app.world().resource::<State<Phase>>().get(), Phase::Hub);
        assert_eq!(
            app.world().resource::<talents::Account>().character,
            Some(1)
        );
        assert_eq!(app.world().resource::<Selection>().stage_def().name, "Hub");
        assert_eq!(app.world().resource::<Run>().elapsed, 0.0);
        let offset = hero::arena(&data::HUB) + Vec3::X * 10.0;
        {
            let world = app.world_mut();
            let mut heroes = world.query_filtered::<&mut Transform, With<hero::Hero>>();
            heroes.single_mut(world).unwrap().translation = offset;
        }
        app.world_mut()
            .resource_mut::<NextState<Phase>>()
            .set(Phase::Paused);
        app.update();
        app.world_mut()
            .resource_mut::<NextState<Phase>>()
            .set(Phase::Hub);
        app.update();
        {
            let world = app.world_mut();
            assert_eq!(
                world
                    .query_filtered::<&Transform, With<hero::Hero>>()
                    .single(world)
                    .unwrap()
                    .translation,
                offset
            );
        }
        click(&mut app, Click::Launch);
        assert_eq!(
            *app.world().resource::<State<Phase>>().get(),
            Phase::Playing
        );
        assert!(!app.world().resource::<Selection>().in_hub);
        app.world_mut().resource_mut::<Run>().meta_kills = 25;
        app.world_mut()
            .resource_mut::<NextState<Phase>>()
            .set(Phase::Hub);
        app.update();
        assert_eq!(app.world().resource::<talents::Account>().xp, 125);
        assert_eq!(app.world().resource::<Run>().kills, 0);
        assert!(app.world().resource::<Selection>().in_hub);
        assert_eq!(app.world().resource::<Roster>().save.characters[0].xp, 125);
        click(&mut app, Click::Logout);
        click(&mut app, Click::Select(2));
        click(&mut app, Click::Enter);
        assert_eq!(
            app.world().resource::<talents::Account>().character,
            Some(2)
        );
        assert_eq!(app.world().resource::<talents::Account>().xp, 0);
        assert_eq!(app.world().resource::<Run>().class.id, data::CLASSES[6].id);
        assert_eq!(app.world().resource::<Roster>().save.characters[0].xp, 125);
    }
    #[test]
    fn dungeon_modifiers_use_a_side_panel_and_show_details_on_hover() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<Phase>()
            .insert_resource(roster())
            .insert_resource(Selection {
                test_character: true, map: data::DUNGEONS[0], ..default()
            })
            .init_resource::<talents::Account>()
            .insert_resource(expeditions::View { open: [false, true, false], ..default() })
            .insert_resource(hud::HudFont(Handle::default()))
            .add_systems(Update, (render, expeditions::modifier_tooltip).chain());
        app.world_mut().resource_mut::<NextState<Phase>>().set(Phase::Hub);
        app.world_mut().resource_mut::<Roster>().expeditions = true;
        app.update();
        assert_eq!(app.world_mut().query::<&expeditions::ModifierPanel>().iter(app.world()).count(), 1);
        assert_eq!(app.world_mut().query::<&expeditions::ModifierHint>().iter(app.world()).count(), 6);
        assert_eq!(app.world_mut().query::<&Click>().iter(app.world())
            .filter(|click| matches!(click, Click::DungeonMod(_, true))).count(), 6);
        assert!(!app.world_mut().query::<&Text>().iter(app.world())
            .any(|text| text.0.contains("per rank")));
        {
            let world = app.world_mut();
            *world.query::<(&expeditions::ModifierHint, &mut Interaction)>()
                .iter_mut(world).next().unwrap().1 = Interaction::Hovered;
        }
        app.update();
        assert_eq!(app.world_mut().query::<&expeditions::ModifierTooltip>().iter(app.world()).count(), 1);
        assert!(app.world_mut().query::<&Text>().iter(app.world())
            .any(|text| text.0.contains("per rank")));
        {
            let mut selection = app.world_mut().resource_mut::<Selection>();
            for index in 0..5 { selection.dungeon_mods.toggle(index); }
            selection.version += 1;
        }
        app.update();
        assert!(app.world_mut().query::<&Text>().iter(app.world())
            .any(|text| text.0.contains("Loot ilvl 77 (+12)")));
        app.world_mut().resource_mut::<Selection>().map = data::CAMPAIGN[0];
        app.world_mut().resource_mut::<Selection>().version += 1;
        app.update();
        assert_eq!(app.world_mut().query::<&expeditions::ModifierPanel>().iter(app.world()).count(), 0);
    }
    #[test]
    fn expedition_browser_preserves_scroll_and_keeps_training_separate_and_restricted() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<Phase>()
            .insert_resource(roster())
            .insert_resource(Selection {
                map: progression::zone_order()[0],
                ..default()
            })
            .init_resource::<talents::Account>()
            .insert_resource(expeditions::View {
                scroll: 171.0,
                ..default()
            })
            .insert_resource(hud::HudFont(Handle::default()))
            .add_systems(Update, (render, expeditions::hover).chain());
        app.world_mut()
            .resource_mut::<NextState<Phase>>()
            .set(Phase::Hub);
        app.world_mut().resource_mut::<Roster>().expeditions = true;
        app.update();
        assert!(!app
            .world_mut()
            .query::<&Click>()
            .iter(app.world())
            .any(|c| matches!(c,Click::Map(i) if *i==data::TRAINING_MAP)));
        assert_eq!(
            app.world_mut()
                .query::<&Click>()
                .iter(app.world())
                .filter(|c| matches!(c, Click::Launch))
                .count(),
            1
        );
        {
            let mut selection = app.world_mut().resource_mut::<Selection>();
            selection.test_character = true;
            selection.map = data::TRAINING_MAP;
            selection.version += 1;
        }
        app.update();
        let positions: Vec<_> = app
            .world_mut()
            .query_filtered::<&ScrollPosition, With<Scroll>>()
            .iter(app.world())
            .map(|p| p.0.y)
            .collect();
        assert_eq!(positions, vec![171.0]);
        assert!(app
            .world_mut()
            .query::<&Click>()
            .iter(app.world())
            .any(|c| matches!(c,Click::Map(i) if *i==data::TRAINING_MAP)));
        let texts: Vec<_> = app
            .world_mut()
            .query::<&Text>()
            .iter(app.world())
            .map(|t| t.0.clone())
            .collect();
        assert!(texts.iter().any(|t| t == "PRACTICE"));
        assert!(texts.iter().any(|t| t == "Immortal dummy"));
        assert!(texts.iter().any(|t| t == "SELECTED DESTINATION"));
        app.world_mut().spawn((
            bevy::window::PrimaryWindow,
            Window {
                resolution: bevy::window::WindowResolution::new(560, 680),
                ..default()
            },
        ));
        app.update();
        let texts: Vec<_> = app
            .world_mut()
            .query::<&Text>()
            .iter(app.world())
            .map(|t| t.0.clone())
            .collect();
        assert!(!texts.iter().any(|t| t == "SELECTED DESTINATION"));
        assert!(texts.iter().any(|t| t == "Training Grounds"));
        assert_eq!(
            app.world_mut()
                .query::<&Click>()
                .iter(app.world())
                .filter(|c| matches!(c, Click::Launch))
                .count(),
            1
        );
    }
    #[test]
    fn every_character_screen_builds_without_duplicate_components() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<Phase>()
            .insert_resource(roster())
            .init_resource::<Selection>()
            .init_resource::<talents::Account>()
            .insert_resource(hud::HudFont(Handle::default()))
            .add_systems(Update, render);
        app.update();
        #[cfg(not(target_arch = "wasm32"))]
        {
            let world = app.world_mut();
            let actions: Vec<_> = world.query::<&Click>().iter(world).copied().collect();
            assert_eq!(
                actions.iter().filter(|a| matches!(a, Click::Enter)).count(),
                1
            );
            assert_eq!(
                actions
                    .iter()
                    .filter(|a| matches!(a, Click::HostLobby))
                    .count(),
                1
            );
            assert_eq!(
                actions
                    .iter()
                    .filter(|a| matches!(a, Click::JoinLobby))
                    .count(),
                1
            );
        }
        assert!(!app
            .world_mut()
            .query::<&Root>()
            .iter(app.world())
            .collect::<Vec<_>>()
            .is_empty());
        app.world_mut().resource_mut::<Roster>().creating = true;
        app.update();
        app.world_mut().resource_mut::<Roster>().creating = false;
        app.world_mut()
            .resource_mut::<NextState<Phase>>()
            .set(Phase::Hub);
        app.update();
        app.world_mut().resource_mut::<Roster>().expeditions = true;
        app.update();
        let world = app.world_mut();
        assert_eq!(world.query::<&Scroll>().iter(world).count(), 1);
    }
    #[test]
    fn full_survivors_ui_schedule_has_no_dependency_cycles() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::state::app::StatesPlugin,
            bevy::asset::AssetPlugin::default(),
        ))
        .init_asset::<Image>()
        .init_asset::<bevy::shader::Shader>()
        .add_plugins(super::super::SurvivorsPlugin);
        app.finish();
        app.cleanup();
        app.world_mut().schedule_scope(Update, |world, schedule| {
            schedule
                .initialize(world)
                .expect("acyclic survivors update schedule");
        });
    }
}
fn select(c: &Character, selection: &mut Selection) {
    selection.test_character = is_test_character(&c.name);
    selection.class = Some(c.class);
    selection.looks = vec![(c.display, c.race, c.sex)];
    selection.look = Some(0);
    selection.version += 1;
}
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Roster>()
        .init_resource::<expeditions::View>()
        .add_systems(Startup, boot)
        .add_systems(Update, scroll.before(input))
        .add_systems(Update, expeditions::hover.after(render))
        .add_systems(Update, expeditions::modifier_tooltip.after(render))
        .add_systems(Update, expeditions::toggle.before(render).after(input))
        .add_systems(Update, frontend::spell_hover.after(render))
        .add_systems(
            Update,
            (input, update_record, render)
                .chain()
                .after(super::menu::menu_clicks)
                .before(hud::show_panels),
        );
}
fn boot(roster: Res<Roster>, mut selection: ResMut<Selection>) {
    selection.in_hub = true;
    if let Some(c) = roster
        .selected
        .and_then(|id| roster.save.characters.iter().find(|c| c.id == id))
    {
        select(c, &mut selection);
    }
}
pub(super) fn logout(mut roster: ResMut<Roster>, account: Res<talents::Account>) {
    account.save();
    if let Some(id) = account.character {
        if let Err(error) = roster.record_xp(id, account.xp) { roster.error = error; }
    }
    roster.active = None;
    roster.creating = false;
    roster.expeditions = false;
}
#[derive(Component)]
struct Scroll;
fn scroll(
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    mut frames: Query<(&ComputedNode, &mut ScrollPosition), With<Scroll>>,
    roster: Option<Res<Roster>>,
    mut expeditions: Option<ResMut<expeditions::View>>,
) {
    for event in wheel.read() {
        let dy = match event.unit {
            bevy::input::mouse::MouseScrollUnit::Line => event.y * 32.0,
            bevy::input::mouse::MouseScrollUnit::Pixel => event.y,
        };
        // Only the current character list or expedition dialog exists. Keep it scrollable
        // over its child buttons, which take hover focus away from their parent node.
        for (node, mut position) in &mut frames {
            position.0.y = (position.0.y - dy).clamp(
                0.0,
                ((node.content_size.y - node.size.y) * node.inverse_scale_factor).max(0.0),
            );
            if roster.as_ref().is_some_and(|r| r.expeditions) {
                if let Some(view) = expeditions.as_deref_mut() {
                    view.scroll = position.0.y;
                }
            }
        }
    }
}
#[derive(Component)]
pub(super) struct Root;
#[derive(Component, Clone, Copy)]
pub(super) enum Click {
    Quit,
    Delete,
    ConfirmDelete,
    Select(u64),
    Create,
    Cancel,
    Enter,
    HostLobby,
    JoinLobby,
    Expeditions,
    Map(usize),
    DungeonMod(usize, bool),
    Endless,
    Launch,
    Runes,
    Logout,
    Back,
    Options,
    Rotate(f32),
}

#[allow(clippy::too_many_arguments)]
pub(super) fn input(
    phase: Res<State<Phase>>,
    mut roster: ResMut<Roster>,
    mut selection: ResMut<Selection>,
    mut account: ResMut<talents::Account>,
    mut next: ResMut<NextState<Phase>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut typing: MessageReader<KeyboardInput>,
    buttons: Query<(&Interaction, &Click), Changed<Interaction>>,
    mut run: ResMut<Run>,
    mut rig: Option<ResMut<hero::CamRig>>,
    mut lobby: Option<ResMut<super::multiplayer::lobby::Lobby>>,
    mut exit: Option<ResMut<Messages<AppExit>>>,
    mut runes: Option<ResMut<super::runes::Runes>>,
    builds: Option<Res<super::builds::ui::Panel>>,
) {
    if builds.as_ref().is_some_and(|b| b.open) { typing.clear(); return; }
    if lobby.as_ref().is_some_and(|l| l.joining) {
        typing.clear();
        return;
    }
    if *phase.get() != Phase::Menu {
        typing.clear();
    }
    let mut actions: Vec<_> = buttons
        .iter()
        .filter(|(i, _)| **i == Interaction::Pressed)
        .map(|(_, c)| *c)
        .collect();
    if *phase.get() == Phase::Menu {
        if roster.creating || roster.deleting.is_some() {
            for event in typing.read().filter(|e| e.state.is_pressed()) {
                if let Some(text) = &event.text {
                    for c in text
                        .chars()
                        .filter(|c| c.is_alphanumeric() || matches!(*c, '_' | '\'' | '-'))
                    {
                        let name = if roster.deleting.is_some() {
                            &mut roster.delete_name
                        } else {
                            &mut roster.name
                        };
                        if name.chars().count() < 16 {
                            name.push(c);
                        }
                    }
                }
            }
            if keys.just_pressed(KeyCode::Backspace) {
                if roster.deleting.is_some() {
                    roster.delete_name.pop();
                } else {
                    roster.name.pop();
                }
            }
            if keys.just_pressed(KeyCode::Escape) {
                actions.push(Click::Cancel);
            }
        } else {
            typing.clear();
        }
        if keys.just_pressed(KeyCode::Enter) || roster.confirm {
            actions.push(if roster.deleting.is_some() {
                Click::ConfirmDelete
            } else {
                Click::Enter
            });
            roster.confirm = false;
            keys.clear_just_pressed(KeyCode::Enter);
        }
    }
    if *phase.get() == Phase::Hub && keys.just_pressed(KeyCode::Escape) && roster.expeditions {
        roster.expeditions = false;
        keys.clear_just_pressed(KeyCode::Escape);
    }
    if let Some(id) = roster.deleting {
        if let Some(action) = actions
            .into_iter()
            .find(|action| matches!(action, Click::Cancel | Click::ConfirmDelete))
        {
            match action {
                Click::Cancel => {
                    roster.deleting = None;
                    roster.delete_name.clear();
                    roster.error.clear();
                }
                Click::ConfirmDelete => {
                    let name = roster.delete_name.clone();
                    match roster.delete(id, &name) {
                        Ok(()) => {
                            *account = talents::Account::fresh();
                            selection.progress = progression::Progress::fresh(0);
                            roster.error.clear();
                            selection.class = None;
                            selection.look = None;
                            selection.looks.clear();
                            selection.version += 1;
                            if let Some(c) = roster
                                .selected
                                .and_then(|id| roster.save.characters.iter().find(|c| c.id == id))
                            {
                                select(c, &mut selection);
                            }
                        }
                        Err(error) => roster.error = error,
                    }
                }
                _ => {}
            }
        }
        return;
    }
    for action in actions {
        match action {
            Click::HostLobby if *phase.get() == Phase::Menu && !roster.creating => {
                match activate_selected(&mut roster, &mut selection, &mut account) {
                    Ok(()) => {
                        if let Some(lobby) = lobby.as_deref_mut() {
                            lobby.host();
                        }
                    }
                    Err(error) => roster.error = error,
                }
            }
            Click::JoinLobby if *phase.get() == Phase::Menu && !roster.creating => {
                if roster.selected.is_some() {
                    if let Some(lobby) = lobby.as_deref_mut() {
                        lobby.open_join();
                    }
                } else {
                    roster.error = "Select a character.".into();
                }
            }
            Click::Delete if *phase.get() == Phase::Menu && !roster.creating && !roster.blocked => {
                if let Some(id) = roster.selected {
                    roster.deleting = Some(id);
                    roster.delete_name.clear();
                    roster.error.clear();
                    break;
                }
            }
            Click::Select(id) if *phase.get() == Phase::Menu && !roster.creating => {
                if let Some(c) = roster.save.characters.iter().find(|c| c.id == id) {
                    select(c, &mut selection);
                    roster.selected = Some(id);
                    roster.error.clear();
                }
            }
            Click::Quit if *phase.get() == Phase::Menu => {
                if let Some(mut exit) = exit.take() {
                    exit.write(AppExit::Success);
                }
            }
            Click::Create if *phase.get() == Phase::Menu => {
                roster.creating = true;
                roster.name.clear();
                roster.error.clear();
                selection.class = Some(0);
                selection.looks.clear();
                selection.look = None;
                selection.version += 1;
            }
            Click::Cancel if *phase.get() == Phase::Menu => {
                roster.creating = false;
                roster.error.clear();
                if let Some(c) = roster
                    .selected
                    .and_then(|id| roster.save.characters.iter().find(|c| c.id == id))
                {
                    select(c, &mut selection);
                }
            }
            Click::Enter if *phase.get() == Phase::Menu => {
                if roster.creating {
                    let (Some(class), Some(&look)) = (
                        selection.class,
                        selection.look.and_then(|i| selection.looks.get(i)),
                    ) else {
                        roster.error =
                            "Waiting for character models. Choose a class and race.".into();
                        continue;
                    };
                    let first = roster.save.next_id == 1;
                    match roster.create(class, look, if first { account.xp } else { 0 }) {
                        Ok(c) => {
                            if first {
                                account.character = Some(c.id);
                                selection.progress.bind(c.id);
                            } else {
                                *account = talents::Account::load_character(c.id);
                                selection.progress = progression::Progress::fresh(c.id);
                            }
                            account.save();
                            roster.creating = false;
                            roster.error.clear();
                            select(&c, &mut selection);
                        }
                        Err(error) => {
                            roster.error = error;
                            continue;
                        }
                    }
                }
                if let Err(error) = activate_selected(&mut roster, &mut selection, &mut account) {
                    roster.error = error;
                    continue;
                }
                if let Some(lobby) = lobby.as_deref_mut() {
                    lobby.solo();
                }
                next.set(Phase::Hub);
            }
            Click::Expeditions
                if selection.in_hub
                    && matches!(*phase.get(), Phase::Hub | Phase::CharacterSheet) =>
            {
                selection.sheet_open = false;
                selection.bags_open = false;
                roster.expeditions = !roster.expeditions;
                selection.talents_open = false;
            }
            Click::Back => roster.expeditions = false,
            Click::Map(i) if *phase.get() == Phase::Hub && selection.allows_map(i) => {
                selection.map = i;
                selection.endless = false;
                selection.version += 1;
            }
            Click::DungeonMod(i, increase) if *phase.get() == Phase::Hub
                && data::dungeon(selection.map) && selection.allows_map(selection.map) => {
                selection.dungeon_mods.change(i, increase);
                selection.version += 1;
            }
            Click::Endless if *phase.get() == Phase::Hub && talents::level(account.xp) >= 60 => {
                selection.endless = true;
                selection.version += 1;
            }
            Click::Launch if *phase.get() == Phase::Hub => {
                if hud::start_run(&mut run, &mut selection) {
                    roster.expeditions = false;
                    next.set(Phase::Playing);
                }
            }
            Click::Runes
                if selection.in_hub
                    && matches!(*phase.get(), Phase::Hub | Phase::CharacterSheet) =>
            {
                selection.sheet_open = false;
                selection.bags_open = false;
                if let Some(runes) = runes.as_deref_mut() {
                    runes.open = !runes.open;
                    if !runes.open {
                        runes.message.clear();
                    }
                }
                selection.talents_open = false;
                roster.expeditions = false;
            }
            Click::Rotate(delta) if *phase.get() == Phase::Menu => {
                if let Some(rig) = rig.as_mut() {
                    rig.orbit += delta;
                }
            }
            Click::Logout
                if selection.in_hub
                    && matches!(*phase.get(), Phase::Hub | Phase::CharacterSheet) =>
            {
                account.save();
                roster.active = None;
                roster.creating = false;
                roster.expeditions = false;
                selection.talents_open = false;
                next.set(Phase::Menu);
            }
            Click::Options
                if matches!(
                    *phase.get(),
                    Phase::Hub | Phase::Playing | Phase::CharacterSheet
                ) =>
            {
                next.set(Phase::Paused);
            }
            _ => {}
        }
    }
}
pub(super) fn activate_selected(
    roster: &mut Roster,
    selection: &mut Selection,
    account: &mut talents::Account,
) -> Result<(), String> {
    if let Some(id) = account.character {
        roster.record_xp(id, account.xp)?;
        account.save();
    }
    let c = roster
        .selected
        .and_then(|id| roster.save.characters.iter().find(|c| c.id == id))
        .cloned()
        .ok_or("Select or create a character.")?;
    *account = talents::Account::load_character(c.id);
    account.xp = account.xp.max(if is_test_character(&c.name) {
        c.xp.max(talents::threshold(60))
    } else {
        c.xp
    });
    selection.progress = progression::Progress::load_character(c.id);
    select(&c, selection);
    selection.map = super::progression::zone_order()
        .into_iter()
        .find(|&i| selection.progress.allows(i))
        .unwrap_or(0);
    selection.in_hub = true;
    selection.talents_open = false;
    roster.active = Some(c.id);
    roster.expeditions = false;
    roster.error.clear();
    Ok(())
}

fn update_record(mut roster: ResMut<Roster>, account: Res<talents::Account>) {
    let Some(id) = account.character else {
        return;
    };
    if let Err(error) = roster.record_xp(id, account.xp) {
        roster.error = format!("Could not save characters: {error}");
    }
}
pub(super) fn arrive(
    mut commands: Commands,
    mut selection: ResMut<Selection>,
    mut run: ResMut<Run>,
    account: Res<talents::Account>,
) {
    selection.in_hub = true;
    selection.talents_open = false;
    selection.version += 1;
    if let (Some(class), Some(&(display, race, sex))) = (
        selection.class,
        selection.look.and_then(|i| selection.looks.get(i)),
    ) {
        run.class = &data::CLASSES[class];
        run.race = race;
        let start = run.class.start;
        run.abilities.push(Owned {
            def: data::ability(start),
            level: 1,
            cd: 0.5,
        });
        account.apply(class, &mut run);
        run.hp = run.stats().max_hp;
        let serial = run.guid();
        hero::spawn_hero(
            &mut commands,
            &data::CLASSES[class],
            display,
            race,
            sex,
            talents::level(account.xp),
            serial,
            hero::arena(&data::HUB),
        );
    }
}
fn label(f: &Handle<Font>, text: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font: f.clone(),
            font_size: size,
            ..default()
        },
        TextColor(color),
    )
}
fn button(
    ui: &mut ChildSpawnerCommands,
    f: &Handle<Font>,
    action: Click,
    text: impl Into<String>,
    selected: bool,
) {
    let text = text.into();
    let width =
        (text.lines().map(str::len).max().unwrap_or(0) as f32 * 7.5 + 30.0).clamp(100.0, 264.0);
    skin::button_sized(ui, f, text, action, width, 34.0, 15.0, selected);
}
#[allow(clippy::too_many_arguments)]
pub(super) fn render(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    roster: Res<Roster>,
    selection: Res<Selection>,
    account: Res<talents::Account>,
    font: Option<Res<hud::HudFont>>,
    roots: Query<Entity, With<Root>>,
    mut cache: Local<String>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    scale: Option<Res<UiScale>>,
    art: Option<Res<crate::glue::art::GlueArt>>,
    spells: Option<Res<crate::ui_action::Spells>>,
    expeditions: Option<Res<expeditions::View>>,
) {
    let Some(font) = font else {
        return;
    };
    let viewport = windows.single().map_or(Vec2::new(1365.0, 768.0), |window| {
        Vec2::new(window.width(), window.height()) / scale.as_deref().map_or(1.0, |s| s.0).max(0.01)
    });
    let stamp = format!(
        "{:?} {:?} {:?} {} {} {} {} {} {} {} {} {} {:?} {}",
        phase.get(),
        roster.selected,
        roster.active,
        roster.creating,
        roster.name,
        roster.error,
        roster.expeditions,
        selection.version,
        selection.talents_open,
        account.xp,
        roster.save.characters.len(),
        roster.blocked,
        roster.deleting,
        roster.delete_name
    );
    let stamp = format!(
        "{stamp} {viewport:?} {} {}",
        spells.is_some(),
        art.as_ref().is_some_and(|a| a.select_highlight.is_some())
    );
    let stamp = format!("{stamp} {:?}", expeditions.as_ref().map(|view| view.open));
    if *cache == stamp {
        return;
    }
    *cache = stamp;
    for e in &roots {
        commands.entity(e).despawn();
    }
    let f = &font.0;
    if *phase.get() == Phase::Menu {
        frontend::menu(
            &mut commands,
            &roster,
            f,
            viewport,
            art.as_deref(),
            &selection,
            spells.as_deref(),
        );
    }
    if *phase.get() == Phase::Menu {
        if let Some(character) = roster
            .deleting
            .and_then(|id| roster.save.characters.iter().find(|c| c.id == id))
        {
            commands.spawn((Root,Button,GlobalZIndex(700),BackgroundColor(Color::srgba(0.0,0.0,0.0,0.85)),Node{position_type:PositionType::Absolute,width:Val::Percent(100.0),height:Val::Percent(100.0),align_items:AlignItems::Center,justify_content:JustifyContent::Center,..default()})).with_children(|overlay|{
                overlay.spawn((BackgroundColor(Color::srgb(0.06,0.025,0.02)),BorderColor::all(hud::GOLD_TONE),Node{width:Val::Px(480.0),padding:UiRect::all(Val::Px(24.0)),border:UiRect::all(Val::Px(2.0)),row_gap:Val::Px(16.0),flex_direction:FlexDirection::Column,..default()})).with_children(|ui|{
                    skin::backdrop(ui);
                    ui.spawn(label(f,"DELETE CHARACTER",26.0,hud::GOLD_TONE));
                    ui.spawn(label(f,format!("Delete {} permanently?\nExperience, talents and zone unlocks will be lost.\n\nType the exact character name to confirm:",character.name),16.0,Color::WHITE));
                    ui.spawn((label(f,format!("{}|",roster.delete_name),26.0,Color::WHITE),BackgroundColor(Color::BLACK),Node{padding:UiRect::all(Val::Px(10.0)),..default()}));
                    if !roster.error.is_empty(){ui.spawn(label(f,&roster.error,14.0,Color::srgb(1.0,0.3,0.2)));}
                    button(ui,f,Click::ConfirmDelete,"Delete permanently",false);button(ui,f,Click::Cancel,"Cancel [Esc]",false);
                });
            });
        }
    }
    if *phase.get() != Phase::Hub {
        return;
    }
    if !roster.expeditions {
        return;
    }
    expeditions::draw(
        &mut commands,
        f,
        &selection,
        viewport,
        expeditions.as_ref().map_or(0.0, |v| v.scroll),
        expeditions
            .as_ref()
            .map_or([true, false, false], |v| v.open),
        talents::level(account.xp) >= 60,
    );
}
