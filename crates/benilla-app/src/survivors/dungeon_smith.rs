//! A completed dungeon grants access to a local, character-owned item upgrade service.
use super::{Phase, Rng, Run, data, hud, items::Inventory};
use bevy::prelude::*;
pub(super) mod config;
mod ui;

pub(super) const NPC_ENTRY: u32 = 9_000_004;
const NAME: &str = "Aldren the Temperer";

#[derive(Component)]
struct SmithNpc;

#[derive(Resource, Default)]
pub(super) struct View {
    selected: Option<u64>,
    character: Option<u64>,
    notice: String,
    open: bool,
    interacted: bool,
    confirm_leave: bool,
}

impl View {
    pub(super) fn prepare(&mut self, inventory: &Inventory) {
        *self = Self {
            character: inventory.money_owner(),
            ..default()
        };
    }
    pub(super) fn selected_item(&self) -> Option<u64> { self.selected }
    pub(super) fn is_open(&self) -> bool {
        self.open && !self.confirm_leave
    }
    pub(super) fn select_item(&mut self, id: u64, inventory: &Inventory) {
        if self.is_open()
            && self.character.is_some()
            && self.character == inventory.money_owner()
            && inventory.upgrade_candidates().any(|item| item.id == id)
        {
            self.selected = Some(id);
            self.notice.clear();
        }
    }
    pub(super) fn speak(&mut self) {
        self.open = true;
        self.interacted = true;
        self.confirm_leave = false;
    }
    pub(super) fn blocks_input(&self) -> bool {
        self.open || self.confirm_leave
    }
    pub(super) fn request_leave(&mut self) -> bool {
        if self.interacted {
            self.confirm_leave = false;
            true
        } else {
            self.confirm_leave = true;
            false
        }
    }
}

pub(super) fn available(run: &Run) -> bool {
    run.won && run.endless.is_none() && data::dungeon(run.expedition_map)
}

fn dungeon_level(run: &Run) -> u32 {
    super::items::reward_item_level(run.expedition_map, 60, true)
        .saturating_add(run.dungeon_modifiers().bonus())
        .min(super::dungeon_config::MAX_REWARD_LEVEL)
}

#[derive(Component, Clone, Copy)]
enum Action {
    ClearSlot,
    Upgrade,
    Leave,
    Close,
    ConfirmLeave,
    CancelLeave,
}

fn arrive(
    mut commands: Commands,
    mut view: ResMut<View>,
    mut run: ResMut<Run>,
    inventory: Res<Inventory>,
    heroes: Query<&Transform, With<super::hero::Hero>>,
    surface: super::geometry::Surface,
    role: Option<Res<super::multiplayer::Role>>,
    mut names: Option<ResMut<crate::names::NameCache>>,
) {
    *view = View::default();
    if !available(&run) {
        return;
    }
    view.prepare(&inventory);
    if let Some(names) = names.as_deref_mut() {
        names.insert_creature(
            NPC_ENTRY,
            Some(crate::names::CreatureRecord {
                name: NAME.into(),
                subname: Some("Dungeon equipment upgrades".into()),
                creature_type: 7,
                pet_family: 0,
                rank: 0,
                type_flags: 0,
                civilian: true,
                racial_leader: false,
                display_id: data::CLASSES[0].looks[0],
            }),
        );
    }
    if role
        .as_deref()
        .is_some_and(|r| *r == super::multiplayer::Role::Guest)
    {
        return;
    }
    let Ok(hero) = heroes.single() else {
        return;
    };
    let pos = hero.translation + hero.rotation * Vec3::new(2.5, 0.0, -1.5);
    let pos = super::enemies::ground(&surface, pos, hero.translation.y);
    let serial = run.guid();
    let entity = super::units::spawn_unit(
        &mut commands,
        &super::units::UnitSpec {
            serial,
            display: data::CLASSES[0].looks[0],
            scale: 1.0,
            level: 60,
            faction: super::units::FACTION_FRIENDLY,
            bytes0: 0x0101,
            weapons: &[],
            sheath: 0,
        },
        pos,
        super::units::yaw_toward(hero.translation - pos),
    );
    commands.entity(entity).insert(SmithNpc);
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

fn clicks(
    phase: Res<State<Phase>>,
    run: Res<Run>,
    account: Res<super::talents::Account>,
    mut view: ResMut<View>,
    mut inventory: ResMut<Inventory>,
    mut rng: ResMut<Rng>,
    mut next: ResMut<NextState<Phase>>,
    buttons: Query<(&Interaction, &Action), Changed<Interaction>>,
) {
    if *phase.get() != Phase::GameOver || !available(&run) {
        return;
    }
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *action {
            Action::Leave => {
                if view.request_leave() {
                    next.set(Phase::Hub);
                }
            }
            Action::Close => view.open = false,
            Action::ConfirmLeave => {
                if view.confirm_leave {
                    next.set(Phase::Hub);
                }
            }
            Action::CancelLeave => view.confirm_leave = false,
            Action::ClearSlot if view.open && !view.confirm_leave => {
                view.selected = None;
                view.notice.clear();
            }
            Action::Upgrade if view.open && !view.confirm_leave => {
                if view.character.is_none()
                    || view.character != inventory.money_owner()
                    || account.character != view.character
                {
                    view.notice =
                        "The selected character changed. Leave this dungeon before upgrading."
                            .into();
                    continue;
                }
                let Some(id) = view.selected else {
                    continue;
                };
                // Validate affordability before consuming a random draw.
                let quote = inventory
                    .item(id)
                    .ok_or_else(|| "Select an owned item.".to_owned())
                    .and_then(|i| config::get().quote(i.level, dungeon_level(&run)));
                let result = quote.and_then(|q| {
                    if inventory.money() < q.cost {
                        return Err("Not enough gold.".into());
                    }
                    inventory.dungeon_upgrade(id, dungeon_level(&run), config::get(), rng.f32())
                });
                view.notice = match result {
                    Ok((0, cost)) => format!(
                        "No ilvl gained. {} paid. The item is unchanged.",
                        ui::money(cost)
                    ),
                    Ok((gain, cost)) => {
                        format!("Upgrade succeeded: +{gain} ilvl. {} paid.", ui::money(cost))
                    }
                    Err(error) => error,
                };
            }
            _ => {}
        }
    }
}

fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    phase: Res<State<Phase>>,
    run: Res<Run>,
    mut view: ResMut<View>,
    mut next: ResMut<NextState<Phase>>,
) {
    if *phase.get() != Phase::GameOver || !available(&run) {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        if view.confirm_leave {
            view.confirm_leave = false;
        } else if view.open {
            view.open = false;
        } else if view.request_leave() {
            next.set(Phase::Hub);
        }
    } else if keys.just_pressed(KeyCode::Enter) && !view.confirm_leave && view.request_leave() {
        next.set(Phase::Hub);
    }
}

fn cleanup(mut commands: Commands, phase: Res<State<Phase>>, npcs: Query<Entity, With<SmithNpc>>) {
    if *phase.get() != Phase::GameOver {
        for entity in &npcs {
            commands.entity(entity).despawn();
        }
    }
}

fn leave(mut view: ResMut<View>) {
    *view = View::default();
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<View>()
        .add_systems(Startup, ui::spawn)
        .add_systems(OnEnter(Phase::GameOver), arrive)
        .add_systems(OnExit(Phase::GameOver), leave)
        .add_systems(
            Update,
            (
                clicks,
                keys.after(super::combat_ui::input).before(hud::keys),
                ui::render.after(clicks).after(keys),
                ui::render_exit.after(clicks).after(keys),
                ui::scroll,
                cleanup,
            ),
        );
    // Opt-in renderer probe, with its own BENILLA_HOME and copied balance files.
    if crate::webenv::var("WOW_PERF_DIAG").as_deref() == Some("1")
        && crate::webenv::var("WOW_SURVIVORS_DIAGNOSTIC_DUNGEON").is_some()
    {
        app.add_systems(Update, diagnostic_launch);
    }
}

fn diagnostic_launch(
    mut step: Local<u8>,
    phase: Res<State<Phase>>,
    mut next: ResMut<NextState<Phase>>,
    mut roster: ResMut<super::characters::Roster>,
    mut selection: ResMut<super::Selection>,
    mut account: ResMut<super::talents::Account>,
    mut run: ResMut<Run>,
    loading: Res<crate::loading_screen::LoadingScreen>,
    heroes: Query<(), With<super::hero::Hero>>,
) {
    if *step >= 2 {
        return;
    }
    if *step == 0 && *phase.get() == Phase::Menu {
        match super::characters::activate_selected(&mut roster, &mut selection, &mut account) {
            Ok(()) => {
                *step = 1;
                next.set(Phase::Hub);
            }
            Err(error) => {
                warn!("Dungeon diagnostic: {error}");
                *step = 2;
            }
        }
    } else if *step == 1 && *phase.get() == Phase::Hub && !loading.covering() && !heroes.is_empty()
    {
        selection.endless = false;
        selection.map = crate::webenv::var("WOW_SURVIVORS_DIAGNOSTIC_DUNGEON")
            .and_then(|s| s.parse::<usize>().ok())
            .filter(|map| data::DUNGEONS.contains(map))
            .unwrap_or(data::DUNGEONS[0]);
        if hud::start_run(&mut run, &mut selection) {
            info!(
                "Dungeon smith diagnostic: {}",
                data::MAPS[selection.map].name
            );
            next.set(Phase::Playing);
        } else {
            warn!("Dungeon diagnostic requires an unlocked dungeon.");
        }
        *step = 2;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    #[test]
    fn selected_upgrade_slot_has_the_bag_background_and_a_renderable_item_icon() {
        let item = super::super::shared_chest::fixture_item();
        let inventory = Inventory::fixture(vec![item.clone()]);
        let item_id = inventory.all()[0].id;
        let mut view = View::default();
        view.prepare(&inventory);
        view.speak();
        view.select_item(item_id, &inventory);
        let mut world = World::new();
        world.insert_resource(State::new(Phase::GameOver));
        world.insert_resource(Run { won: true, expedition_map: data::DUNGEONS[0], ..default() });
        world.insert_resource(view);
        world.init_resource::<super::super::Selection>();
        world.insert_resource(inventory);
        world.insert_resource(hud::HudFont(Handle::default()));
        world.run_system_once(ui::spawn).unwrap();
        world.run_system_once(ui::render).unwrap();
        let slot = world.query::<(Entity, &super::super::combat_ui::bags::DropTarget)>()
            .iter(&world).find(|(_, target)| matches!(target,
                super::super::combat_ui::bags::DropTarget::DungeonUpgrade)).unwrap().0;
        assert!(world.get::<super::super::combat_ui::bags::ItemHoverTip>(slot).is_some());
        let children = world.get::<Children>(slot).unwrap();
        assert!(children.iter().any(|child| world.get::<hud::UiArt>(child).is_some()
            && world.get::<ImageNode>(child).is_some()
            && world.get::<Node>(child).is_some_and(|node| node.width == Val::Px(56.0))));
    }
    #[test]
    fn slot_starts_empty_and_only_references_owned_equipment() {
        let inventory = Inventory::fixture(vec![super::super::shared_chest::fixture_item()]);
        let before = inventory.item(1).unwrap().clone();
        let mut view = View {
            character: inventory.money_owner(),
            ..default()
        };
        assert_eq!(view.selected, None);
        view.select_item(1, &inventory);
        assert_eq!(view.selected, None);
        view.speak();
        view.select_item(1, &inventory);
        assert_eq!(view.selected, Some(1));
        view.select_item(u64::MAX, &inventory);
        assert_eq!(view.selected, Some(1));
        assert_eq!(serde_json::to_value(inventory.item(1).unwrap()).unwrap(), serde_json::to_value(&before).unwrap());
        view.character = Some(999);
        view.selected = None;
        view.select_item(1, &inventory);
        assert_eq!(view.selected, None);
    }
    #[test]
    fn upgrades_require_victory_a_conversation_and_the_correct_character() {
        let mut world = World::new();
        let mut item = super::super::shared_chest::fixture_item();
        item.level = 60;
        let mut inventory = Inventory::fixture(vec![item]);
        let starting_gold = 20_000 * super::super::currency::GOLD;
        inventory.add_money(starting_gold).unwrap();
        let mut run = Run::default();
        run.expedition_map = data::DUNGEONS[0];
        let mut account = super::super::talents::Account::default();
        account.character = Some(1);
        world.insert_resource(inventory);
        world.insert_resource(run);
        world.insert_resource(account);
        world.insert_resource(View {
            selected: Some(1),
            character: Some(1),
            ..default()
        });
        world.insert_resource(Rng(42));
        world.insert_resource(State::new(Phase::Playing));
        world.insert_resource(NextState::<Phase>::default());
        world.spawn((Interaction::Pressed, Action::Upgrade));
        world.run_system_once(clicks).unwrap();
        assert_eq!(world.resource::<Inventory>().money(), starting_gold);
        world.insert_resource(State::new(Phase::GameOver));
        world.run_system_once(clicks).unwrap();
        assert_eq!(world.resource::<Inventory>().money(), starting_gold);
        world.resource_mut::<Run>().won = true;
        world.run_system_once(clicks).unwrap();
        assert_eq!(world.resource::<Inventory>().money(), starting_gold);
        world.resource_mut::<View>().speak();
        world
            .resource_mut::<super::super::talents::Account>()
            .character = Some(2);
        world.run_system_once(clicks).unwrap();
        assert_eq!(world.resource::<Inventory>().money(), starting_gold);
        world
            .resource_mut::<super::super::talents::Account>()
            .character = Some(1);
        world.run_system_once(clicks).unwrap();
        assert_eq!(
            world.resource::<Inventory>().money(),
            starting_gold - 5000 * super::super::currency::GOLD
        );
        assert!(world.resource::<Inventory>().item(1).unwrap().level > 60);
        let before = world.resource::<Inventory>().money();
        world.run_system_once(clicks).unwrap();
        assert!(
            world.resource::<Inventory>().money() < before,
            "multiple paid attempts stay available"
        );
    }
    #[test]
    fn leaving_requires_confirmation_until_the_player_has_spoken_to_the_smith() {
        let mut world = World::new();
        world.insert_resource(State::new(Phase::GameOver));
        world.insert_resource(Run {
            won: true,
            expedition_map: data::DUNGEONS[0],
            ..default()
        });
        world.insert_resource(Inventory::fixture(vec![]));
        world.insert_resource(super::super::talents::Account::default());
        world.insert_resource(View::default());
        world.insert_resource(Rng(42));
        world.insert_resource(NextState::<Phase>::default());
        let button = world.spawn((Interaction::Pressed, Action::Leave)).id();
        world.run_system_once(clicks).unwrap();
        assert!(world.resource::<View>().confirm_leave);
        assert!(!matches!(
            world.resource::<NextState<Phase>>(),
            NextState::Pending(_)
        ));
        world.entity_mut(button).insert(Action::CancelLeave);
        world.run_system_once(clicks).unwrap();
        assert!(!world.resource::<View>().confirm_leave);
        assert!(!matches!(
            world.resource::<NextState<Phase>>(),
            NextState::Pending(_)
        ));
        // A forged confirmation cannot bypass the initial request.
        world.entity_mut(button).insert(Action::ConfirmLeave);
        world.run_system_once(clicks).unwrap();
        assert!(!matches!(
            world.resource::<NextState<Phase>>(),
            NextState::Pending(_)
        ));
        world.entity_mut(button).insert(Action::Leave);
        world.run_system_once(clicks).unwrap();
        world.entity_mut(button).insert(Action::ConfirmLeave);
        world.run_system_once(clicks).unwrap();
        assert!(matches!(
            world.resource::<NextState<Phase>>(),
            NextState::Pending(Phase::Hub)
        ));
        world.insert_resource(NextState::<Phase>::default());
        world.resource_mut::<View>().speak();
        world.entity_mut(button).insert(Action::Close);
        world.run_system_once(clicks).unwrap();
        assert!(!world.resource::<View>().open);
        assert!(!world.resource::<View>().blocks_input());
        world.entity_mut(button).insert(Action::Leave);
        world.run_system_once(clicks).unwrap();
        assert!(matches!(
            world.resource::<NextState<Phase>>(),
            NextState::Pending(Phase::Hub)
        ));
        assert_eq!(
            world.resource::<Inventory>().money(),
            0,
            "speaking does not require a purchase"
        );
        world.resource_mut::<View>().speak();
        world.run_system_once(leave).unwrap();
        assert!(
            !world.resource::<View>().blocks_input(),
            "leaving must not lock movement in the hub"
        );
    }
    #[test]
    fn keyboard_exit_stays_in_the_dungeon_until_confirmation_and_escape_cancels() {
        let mut world = World::new();
        world.insert_resource(State::new(Phase::GameOver));
        world.insert_resource(Run {
            won: true,
            expedition_map: data::DUNGEONS[0],
            ..default()
        });
        world.insert_resource(View::default());
        world.insert_resource(NextState::<Phase>::default());
        world.init_resource::<ButtonInput<KeyCode>>();
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        world.run_system_once(keys).unwrap();
        assert!(world.resource::<View>().confirm_leave);
        world.run_system_once(keys).unwrap();
        assert!(!matches!(
            world.resource::<NextState<Phase>>(),
            NextState::Pending(_)
        ));
        world.resource_mut::<ButtonInput<KeyCode>>().reset_all();
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        world.run_system_once(keys).unwrap();
        assert!(!world.resource::<View>().confirm_leave);
        assert!(!matches!(
            world.resource::<NextState<Phase>>(),
            NextState::Pending(_)
        ));
    }
    #[test]
    fn service_requires_a_successful_dungeon_and_uses_frozen_dungeon_rewards() {
        let mut run = Run::default();
        assert!(!available(&run));
        run.expedition_map = data::DUNGEONS[0];
        assert!(!available(&run));
        run.won = true;
        assert!(available(&run));
        assert_eq!(dungeon_level(&run), 65);
        for i in 0..6 {
            for _ in 0..3 {
                run.dungeon_mods.change(i, true);
            }
        }
        assert_eq!(dungeon_level(&run), 110);
        run.expedition_map = data::CAMPAIGN[0];
        assert!(!available(&run));
    }
}
