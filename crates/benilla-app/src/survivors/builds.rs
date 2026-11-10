//! Saved, class-specific card priorities. They never pick without an explicit player action.
use super::{
    Phase, Run, data,
    hud::{Choice, Offer},
};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
mod presets;
pub(super) mod ui;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Build {
    id: u64,
    class: u8,
    name: String,
    spells: Vec<String>,
    passives: Vec<String>,
}

#[derive(Resource, Default, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct Store {
    builds: Vec<Build>,
    active: BTreeMap<u8, u64>,
    seeded_presets: std::collections::BTreeSet<String>,
    #[serde(skip)]
    revision: u64,
    #[serde(skip)]
    notice: String,
}

fn spell_keys(class: &data::ClassDef) -> Vec<String> {
    let mut keys = vec![class.start.to_owned()];
    for key in class.pool {
        if !keys.iter().any(|k| k == key) {
            keys.push((*key).into());
        }
    }
    keys
}
fn passive_keys() -> Vec<String> {
    let mut keys = Vec::new();
    for p in data::PASSIVES {
        if !keys.contains(&p.key.to_owned()) {
            keys.push(p.key.to_owned());
        }
    }
    keys
}
fn reconcile(order: &mut Vec<String>, allowed: &[String]) {
    let mut seen = std::collections::HashSet::new();
    order.retain(|key| allowed.contains(key) && seen.insert(key.clone()));
    for key in allowed {
        if seen.insert(key.clone()) {
            order.push(key.clone());
        }
    }
}

impl Store {
    #[cfg(test)]
    pub(super) fn fixture(class: &data::ClassDef) -> Self {
        let mut store = Self::default();
        let id = store.create(class).unwrap();
        store.active.insert(class.id, id);
        store
    }
    fn decode(text: &str) -> Result<Self, serde_json::Error> {
        let mut store: Self = serde_json::from_str(text)?;
        let mut ids = std::collections::HashSet::new();
        store
            .builds
            .retain(|b| data::CLASSES.iter().any(|c| c.id == b.class) && ids.insert(b.id));
        for build in &mut store.builds {
            let class = data::CLASSES.iter().find(|c| c.id == build.class).unwrap();
            reconcile(&mut build.spells, &spell_keys(class));
            reconcile(&mut build.passives, &passive_keys());
            build.name = build
                .name
                .chars()
                .filter(|c| !c.is_control())
                .take(40)
                .collect();
            if build.name.trim().is_empty() {
                build.name = format!("Build {}", build.id);
            }
        }
        store.active.retain(|class, id| {
            store
                .builds
                .iter()
                .any(|b| b.id == *id && b.class == *class)
        });
        Ok(store)
    }
    fn load() -> Self {
        let text = crate::local_state::home().and_then(|home| {
            crate::local_state::read_to_string(&home.join("survivors-card-builds-v1.json")).ok()
        });
        let mut store = match text {
            Some(text) => Self::decode(&text).unwrap_or_else(|e| Self {
                notice: format!("Could not read saved builds: {e}"),
                ..default()
            }),
            None => Self::default(),
        };
        if store.notice.is_empty() && presets::seed(&mut store) {
            store.save();
        }
        store
    }
    fn save(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        let Some(home) = crate::local_state::home() else {
            return;
        };
        match serde_json::to_string_pretty(self)
            .map_err(|e| e.to_string())
            .and_then(|text| {
                crate::local_state::write_queued(&home.join("survivors-card-builds-v1.json"), &text)
                    .map_err(|e| e.to_string())
            }) {
            Ok(()) => self.notice.clear(),
            Err(e) => self.notice = format!("Could not save builds: {e}"),
        }
    }
    fn active_build(&self, class: u8) -> Option<&Build> {
        let id = self.active.get(&class)?;
        self.builds.iter().find(|b| b.id == *id && b.class == class)
    }
    pub(super) fn has_active_build(&self, class: u8) -> bool {
        self.active_build(class).is_some()
    }
    fn create(&mut self, class: &data::ClassDef) -> Option<u64> {
        let id = self
            .builds
            .iter()
            .map(|b| b.id)
            .max()
            .unwrap_or(0)
            .checked_add(1)?;
        self.builds.push(Build {
            id,
            class: class.id,
            name: format!("Build {id}"),
            spells: spell_keys(class),
            passives: passive_keys(),
        });
        self.save();
        Some(id)
    }
    fn delete(&mut self, id: u64) {
        self.builds.retain(|b| b.id != id);
        self.active.retain(|_, active| *active != id);
        self.save();
    }
    fn reorder(&mut self, id: u64, tab: ui::Tab, from: usize, to: usize) {
        let Some(build) = self.builds.iter_mut().find(|b| b.id == id) else {
            return;
        };
        let order = match tab {
            ui::Tab::Spells => &mut build.spells,
            ui::Tab::Passives => &mut build.passives,
        };
        if from >= order.len() || to >= order.len() || from == to {
            return;
        }
        let key = order.remove(from);
        order.insert(to, key);
        self.save();
    }
    pub(super) fn choose(&self, run: &Run, choices: &[Choice]) -> Option<usize> {
        let build = self.active_build(run.class.id)?;
        choices
            .iter()
            .enumerate()
            .filter_map(|(i, choice)| {
                let (spell, key) = choice.card_key(run);
                let order = if spell {
                    &build.spells
                } else {
                    &build.passives
                };
                order
                    .iter()
                    .position(|k| k == key)
                    .map(|rank| ((rank, i), i))
            })
            .min_by_key(|(rank, _)| *rank)
            .map(|(_, i)| i)
            // Recovery is not a build preference; when it is the only offer, take it normally.
            .or_else(|| (choices.len() == 1 && matches!(choices[0], Choice::Heal)).then_some(0))
    }
    pub(super) fn choose_full_auto(&self, run: &Run, choices: &[Choice]) -> Option<usize> {
        if !self.has_active_build(run.class.id) { return None; }
        self.choose(run, choices).or_else(|| (!choices.is_empty()).then_some(0))
    }
}

#[derive(Component)]
pub(super) struct AutoButton;

fn auto_label(
    store: Res<Store>,
    run: Res<Run>,
    offer: Res<Offer>,
    settings: Res<super::options::Settings>,
    mut buttons: Query<(&Children, &mut Node), With<AutoButton>>,
    mut texts: Query<&mut Text>,
) {
    let label = match store.active_build(run.class.id) {
        Some(b) if store.choose(&run, offer.choices()).is_some() => format!("Auto · {}", b.name),
        Some(_) => "Auto · no match".into(),
        None => "Auto · no active build".into(),
    };
    let label = settings
        .key_name(super::options::Action::AutoPick)
        .map_or(label.clone(), |key| format!("[{key}] {label}"));
    let display = if store.active_build(run.class.id).is_some() {
        Display::Flex
    } else {
        Display::None
    };
    for (children, mut node) in &mut buttons {
        if node.display != display {
            node.display = display;
        }
        for child in children.iter() {
            if let Ok(mut text) = texts.get_mut(child) {
                if text.0 != label {
                    text.0 = label.clone();
                }
            }
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.insert_resource(Store::load())
        .init_resource::<ui::Panel>()
        .add_message::<bevy::input::keyboard::KeyboardInput>()
        .add_message::<bevy::input::mouse::MouseWheel>()
        .add_systems(
            Update,
            ui::input
                .after(super::combat_ui::input)
                .before(super::talent_ui::input)
                .before(super::hud::keys)
                .before(benilla_world::schedule::WorldStage::Input),
        )
        .add_systems(
            Update,
            (ui::drag, ui::scroll, ui::render).chain().after(ui::input),
        )
        .add_systems(Update, ui::tooltip.after(ui::render))
        .add_systems(Update, auto_label);
}

#[cfg(test)]
mod tests {
    use super::*;
    fn run() -> Run {
        Run::default()
    }
    #[test]
    fn auto_button_is_absent_until_a_build_is_explicitly_activated() {
        let mut app = App::new();
        let run = run();
        let class = run.class;
        app.insert_resource(run)
            .init_resource::<Store>()
            .init_resource::<Offer>()
            .insert_resource(super::super::options::Settings::defaults())
            .add_systems(Update, auto_label);
        let button = app
            .world_mut()
            .spawn((AutoButton, Node::default()))
            .with_children(|b| {
                b.spawn(Text::new("Auto"));
            })
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Node>(button).unwrap().display,
            Display::None
        );
        let id = app
            .world_mut()
            .resource_mut::<Store>()
            .create(class)
            .unwrap();
        app.update();
        assert!(app.world().resource::<Store>().active.is_empty());
        assert_eq!(
            app.world().get::<Node>(button).unwrap().display,
            Display::None
        );
        app.world_mut()
            .resource_mut::<Store>()
            .active
            .insert(class.id, id);
        app.update();
        assert_eq!(
            app.world().get::<Node>(button).unwrap().display,
            Display::Flex
        );
        app.world_mut().resource_mut::<Store>().active.clear();
        app.update();
        assert_eq!(
            app.world().get::<Node>(button).unwrap().display,
            Display::None
        );
    }
    #[test]
    fn priorities_pick_the_first_present_entry_for_new_and_upgraded_cards() {
        let mut run = run();
        let mut store = Store::fixture(run.class);
        let id = store.active_build(run.class.id).unwrap().id;
        let keys = spell_keys(run.class);
        let preferred = data::ability(&keys[2]);
        let other = data::ability(&keys[1]);
        store.reorder(id, ui::Tab::Spells, 2, 0);
        assert_eq!(
            store.choose(
                &run,
                &[Choice::NewAbility(other), Choice::NewAbility(preferred)]
            ),
            Some(1)
        );
        run.abilities.clear();
        run.abilities.push(crate::survivors::Owned {
            def: preferred,
            level: 1,
            cd: 0.0,
        });
        assert_eq!(
            store.choose(&run, &[Choice::NewAbility(other), Choice::Upgrade(0)]),
            Some(1)
        );
        let p = &data::PASSIVES[1];
        store.reorder(id, ui::Tab::Passives, 1, 0);
        run.passives.push((p, 1));
        for choice in [
            Choice::NewPassive(p),
            Choice::NumericPassive(p, 4),
            Choice::UpPassive(0),
        ] {
            assert_eq!(
                store.choose(&run, &[Choice::NewPassive(&data::PASSIVES[0]), choice]),
                Some(1)
            );
        }
        assert_eq!(store.choose(&run, &[Choice::Heal]), Some(0));
        assert_eq!(store.choose(&run, &[]), None);
    }
    #[test]
    fn active_build_is_class_scoped_and_deleting_it_disables_auto() {
        let run = run();
        let mut store = Store::fixture(run.class);
        let id = store.active_build(run.class.id).unwrap().id;
        let other = data::CLASSES.iter().find(|c| c.id != run.class.id).unwrap();
        let other_id = store.create(other).unwrap();
        assert!(store.active_build(other.id).is_none());
        store.active.insert(other.id, other_id);
        assert_eq!(store.active_build(run.class.id).unwrap().id, id);
        assert_eq!(store.active_build(other.id).unwrap().id, other_id);
        store.delete(id);
        assert_eq!(store.choose(&run, &[Choice::Heal]), None);
        assert_eq!(store.active_build(other.id).unwrap().id, other_id);
    }
    #[test]
    fn persistence_preserves_orders_and_reconciles_catalogue_updates() {
        let run = run();
        let mut store = Store::fixture(run.class);
        let id = store.active_build(run.class.id).unwrap().id;
        store.reorder(id, ui::Tab::Spells, 2, 0);
        store.reorder(id, ui::Tab::Passives, 5, 0);
        let expected = store.builds[0].clone();
        let json = serde_json::to_string(&store).unwrap();
        let loaded = Store::decode(&json).unwrap();
        assert_eq!(loaded.builds[0].spells, expected.spells);
        assert_eq!(loaded.builds[0].passives, expected.passives);
        assert_eq!(loaded.active, store.active);
        store.builds[0].spells = vec![
            expected.spells[0].clone(),
            "removed_spell".into(),
            expected.spells[0].clone(),
        ];
        let loaded = Store::decode(&serde_json::to_string(&store).unwrap()).unwrap();
        assert_eq!(loaded.builds[0].spells[0], expected.spells[0]);
        assert_eq!(loaded.builds[0].spells.len(), spell_keys(run.class).len());
        assert!(!loaded.builds[0].spells.iter().any(|k| k == "removed_spell"));
        assert!(Store::decode("broken").is_err());
    }
}
