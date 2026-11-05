//! The host owns the vault. Inventory outboxes and vault delivery reservations survive retries.
use super::{
    hero, hud,
    items::{Inventory, Item},
    multiplayer::Role,
    units, Phase, Rng, Run, RunEntity, Selection,
};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
mod ui;

pub(super) fn nonce() -> String {
    static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let mut rng = Rng::default();
    format!(
        "{:016x}{:016x}{:016x}",
        rng.next_u64(),
        rng.next_u64(),
        SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Transfer {
    pub token: String,
    pub vault: String,
    pub item: Item,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Entry {
    pub id: u64,
    pub item: Item,
}
#[derive(Clone, Serialize, Deserialize)]
struct Delivery {
    identity: String,
    #[serde(default)]
    entry: u64,
    transfer: Transfer,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Save {
    version: u32,
    identity: String,
    next_id: u64,
    entries: Vec<Entry>,
    deposits: BTreeSet<String>,
    deliveries: Vec<Delivery>,
}
impl Default for Save {
    fn default() -> Self {
        Self {
            version: 1,
            identity: nonce(),
            next_id: 1,
            entries: vec![],
            deposits: default(),
            deliveries: vec![],
        }
    }
}
#[derive(Resource, Default)]
pub(super) struct Store {
    save: Save,
    pub error: String,
    blocked: bool,
    loaded: bool,
    revision: u64,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct Replica {
    pub identity: String,
    pub entries: Vec<Entry>,
    pub accepted: Vec<String>,
    pub deliveries: Vec<Transfer>,
    pub notice: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) enum Request {
    Deposit(String),
    Withdraw { vault: String, entry: u64 },
    Ack(String),
}
#[derive(Resource, Default)]
pub(super) struct View {
    pub open: bool,
    pub replica: Replica,
    pub requests: Vec<Request>,
    pub notices: BTreeMap<u64, String>,
    pub notice: String,
    pub revision: u64,
    stamp: Option<(u64, u64)>,
    scroll: [f32; 2],
}
#[derive(Component)]
struct Chest;

pub(super) fn position() -> Vec3 {
    hero::arena(&super::data::HUB) + Vec3::X * 4.0
}
pub(super) fn near(pos: Vec3) -> bool {
    units::flat(pos - position()).length() <= 5.0
}

impl Store {
    fn persist(save: &Save) -> Result<(), String> {
        let Some(home) = crate::local_state::home() else {
            return Ok(());
        };
        let text = toml::to_string_pretty(save).map_err(|e| e.to_string())?;
        crate::local_state::write_atomic(&home.join("shared-chest.toml"), &text)
            .map_err(|e| e.to_string())
    }
    fn commit(&mut self, save: Save) -> Result<(), String> {
        if self.blocked {
            return Err(self.error.clone());
        }
        Self::persist(&save)?;
        self.save = save;
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }
    fn load(&mut self) {
        self.loaded = true;
        self.revision = self.revision.wrapping_add(1);
        let result = crate::local_state::home().and_then(|h| {
            match crate::local_state::read_to_string(&h.join("shared-chest.toml")) {
                Ok(text) => Some(toml::from_str::<Save>(&text).map_err(|e| e.to_string())),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => Some(Err(e.to_string())),
            }
        });
        match result {
            Some(Ok(save)) if save.version == 1 && !save.identity.is_empty() => self.save = save,
            Some(_) => {
                self.blocked = true;
                self.error = "Sauvegarde du coffre illisible ; aucun transfert autorisé.".into();
            }
            None => {
                if let Err(e) = Self::persist(&self.save) {
                    self.blocked = true;
                    self.error = e;
                }
            }
        }
    }
    pub fn replica(&self, inventory: &Inventory) -> Replica {
        self.replica_for(inventory.trade_identity(), inventory.pending_deposits())
    }
    pub fn replica_for(&self, identity: &str, pending: &[Transfer]) -> Replica {
        Replica {
            identity: if self.blocked {
                String::new()
            } else {
                self.save.identity.clone()
            },
            entries: self.save.entries.clone(),
            accepted: pending
                .iter()
                .filter(|t| self.save.deposits.contains(&t.token))
                .map(|t| t.token.clone())
                .collect(),
            deliveries: self
                .save
                .deliveries
                .iter()
                .filter(|d| d.identity == identity)
                .map(|d| d.transfer.clone())
                .collect(),
            notice: self.error.clone(),
        }
    }
    pub fn request(
        &mut self,
        inventory: &Inventory,
        request: Request,
        accessible: bool,
    ) -> Result<(), String> {
        self.request_for(
            inventory.trade_identity(),
            inventory.pending_deposits(),
            request,
            accessible,
        )
    }
    pub fn request_for(
        &mut self,
        identity: &str,
        pending: &[Transfer],
        request: Request,
        accessible: bool,
    ) -> Result<(), String> {
        match request {
            Request::Deposit(token) => {
                if self.save.deposits.contains(&token) {
                    return Ok(());
                }
                if !accessible {
                    return Err("Approchez du coffre à Hurlevent pour déposer un objet.".into());
                }
                let transfer = pending
                    .iter()
                    .find(|t| t.token == token)
                    .ok_or("Transfert introuvable dans l'inventaire.")?;
                if transfer.vault != self.save.identity {
                    return Err("Ce dépôt appartient à un autre coffre.".into());
                }
                let mut save = self.save.clone();
                let id = save.next_id;
                save.next_id = id.checked_add(1).ok_or("Coffre plein.")?;
                save.entries.push(Entry {
                    id,
                    item: transfer.item.clone(),
                });
                save.deposits.insert(token);
                self.commit(save)
            }
            Request::Withdraw { vault, entry } => {
                if !accessible || vault != self.save.identity {
                    return Err("Approchez du coffre à Hurlevent pour récupérer un objet.".into());
                }
                if identity.is_empty() {
                    return Err("Identité de transfert manquante.".into());
                }
                if self
                    .save
                    .deliveries
                    .iter()
                    .any(|d| d.entry == entry && d.identity == identity)
                {
                    return Ok(());
                }
                let mut save = self.save.clone();
                let index = save
                    .entries
                    .iter()
                    .position(|e| e.id == entry)
                    .ok_or("Cet objet a déjà été récupéré.")?;
                let item = save.entries.remove(index).item;
                let token = format!("{}:{}", save.identity, save.next_id);
                save.next_id = save.next_id.checked_add(1).ok_or("Identifiants épuisés.")?;
                save.deliveries.push(Delivery {
                    identity: identity.into(),
                    entry,
                    transfer: Transfer {
                        token,
                        vault: save.identity.clone(),
                        item,
                    },
                });
                self.commit(save)
            }
            Request::Ack(token) => {
                if !self
                    .save
                    .deliveries
                    .iter()
                    .any(|d| d.transfer.token == token && d.identity == identity)
                {
                    return Ok(());
                }
                let mut save = self.save.clone();
                save.deliveries
                    .retain(|d| !(d.transfer.token == token && d.identity == identity));
                self.commit(save)
            }
        }
    }
}

pub(super) fn receive(replica: Replica, inventory: &mut Inventory, view: &mut View) {
    for token in &replica.accepted {
        if let Err(e) = inventory.finish_deposit(token) {
            view.notice = e;
        }
    }
    for delivery in &replica.deliveries {
        match inventory.receive_trade(delivery) {
            Ok(()) => view.requests.push(Request::Ack(delivery.token.clone())),
            Err(e) => view.notice = e,
        }
    }
    if !replica.notice.is_empty() {
        view.notice = replica.notice.clone();
    }
    view.replica = replica;
    view.revision = view.revision.wrapping_add(1);
}

fn local_sync(
    role: Res<Role>,
    selection: Res<Selection>,
    heroes: Query<&Transform, With<hero::Hero>>,
    mut store: ResMut<Store>,
    mut inventory: ResMut<Inventory>,
    mut view: ResMut<View>,
) {
    if *role == Role::Guest {
        return;
    }
    if !store.loaded {
        store.load();
    }
    let accessible = selection.in_hub && heroes.single().is_ok_and(|t| near(t.translation));
    let mut requests = std::mem::take(&mut view.requests);
    requests.extend(
        inventory
            .pending_deposits()
            .iter()
            .filter(|t| t.vault == store.save.identity)
            .map(|t| Request::Deposit(t.token.clone())),
    );
    for request in requests {
        if let Err(e) = store.request(&inventory, request, accessible) {
            view.notice = e;
        }
    }
    if view.stamp != Some((store.revision, inventory.revision)) {
        let replica = store.replica(&inventory);
        receive(replica, &mut inventory, &mut view);
        view.stamp = Some((store.revision, inventory.revision));
    }
}

fn spawn(
    mut commands: Commands,
    selection: Res<Selection>,
    mut run: ResMut<Run>,
    role: Res<Role>,
    chest: Query<Entity, With<Chest>>,
    heroes: Query<&Transform, With<hero::Hero>>,
) {
    if !selection.in_hub {
        for entity in &chest {
            commands.entity(entity).despawn();
        }
        return;
    }
    if *role == Role::Guest || heroes.is_empty() {
        return;
    }
    if chest.is_empty() {
        let entity = units::spawn_object(&mut commands, run.guid(), 10, 3.0, position());
        commands.entity(entity).insert((Chest, RunEntity));
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Store>()
        .init_resource::<View>()
        .add_systems(Startup, |mut store: ResMut<Store>, role: Res<Role>| {
            if *role != Role::Guest {
                store.load();
            }
        })
        .add_systems(Update, spawn.after(super::characters::arrive))
        .add_systems(
            Update,
            local_sync.after(super::items::sync).before(ui::render),
        )
        .add_systems(
            Update,
            ui::input
                .after(super::combat_ui::input)
                .before(super::client::input)
                .before(hud::keys),
        )
        .add_systems(Update, ui::render.after(ui::input));
}

#[cfg(test)]
pub(super) fn fixture_item() -> Item {
    Item {
        affix_rules: 0,
        legendary: None,
        base_damage: None,
        base_armour: None,
        appearance: None,
        equipment_type: super::items::EquipmentType::Legacy,
        base_block: 0.0,
        loot_class: None,
        id: 0,
        base: "trade-test".into(),
        name: "Anneau échangé".into(),
        slot: super::items::SlotKind::Ring,
        icon: "Interface\\Icons\\INV_Misc_QuestionMark".into(),
        rarity: super::items::Rarity::Normal,
        level: 1,
        affixes: vec![],
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    #[test]
    fn transfers_survive_retries_reloads_and_competing_withdrawals_without_duplication() {
        let mut store = Store::default();
        let mut sender = Inventory::fixture(vec![fixture_item(), fixture_item()]);
        sender.equip(1, "Ring 1").unwrap();
        let vault = store.replica(&sender).identity;
        assert!(sender.begin_deposit(1, &vault).is_err());
        sender.begin_deposit(2, &vault).unwrap();
        assert!(sender.item(2).is_none());
        let token = sender.pending_deposits()[0].token.clone();
        assert!(store
            .request(&sender, Request::Deposit(token.clone()), false)
            .is_err());
        store
            .request(&sender, Request::Deposit(token.clone()), true)
            .unwrap();
        store
            .request(&sender, Request::Deposit(token.clone()), true)
            .unwrap();
        assert_eq!(store.save.entries.len(), 1);
        store.save = toml::from_str(&toml::to_string(&store.save).unwrap()).unwrap();
        let pending_sender = Inventory::from_shared(1, &sender.shared_save().unwrap()).unwrap();
        assert_eq!(store.replica(&pending_sender).accepted, vec![token.clone()]);
        sender.finish_deposit(&token).unwrap();
        let mut receiver = Inventory::fixture(vec![fixture_item(), fixture_item()]);
        receiver.ensure_trade_identity().unwrap();
        let mut competitor = Inventory::fixture(vec![]);
        competitor.ensure_trade_identity().unwrap();
        let entry = store.save.entries[0].id;
        store
            .request(
                &receiver,
                Request::Withdraw {
                    vault: vault.clone(),
                    entry,
                },
                true,
            )
            .unwrap();
        assert!(store
            .request(
                &competitor,
                Request::Withdraw {
                    vault: vault.clone(),
                    entry
                },
                true
            )
            .is_err());
        assert!(store.save.entries.is_empty());
        store
            .request(
                &receiver,
                Request::Withdraw {
                    vault: vault.clone(),
                    entry,
                },
                true,
            )
            .unwrap();
        assert_eq!(store.save.deliveries.len(), 1);
        store.save = toml::from_str(&toml::to_string(&store.save).unwrap()).unwrap();
        let replica = store.replica(&receiver);
        assert_eq!(replica.deliveries.len(), 1);
        assert!(store.replica(&competitor).deliveries.is_empty());
        let mut view = View::default();
        receive(replica.clone(), &mut receiver, &mut view);
        receive(replica.clone(), &mut receiver, &mut view);
        assert_eq!(receiver.all().len(), 3);
        let reloaded = Inventory::from_shared(1, &receiver.shared_save().unwrap()).unwrap();
        let mut reloaded = reloaded;
        reloaded.receive_trade(&replica.deliveries[0]).unwrap();
        assert_eq!(reloaded.all().len(), 3);
        for ack in view.requests {
            store.request(&receiver, ack, false).unwrap();
        }
        assert!(store.replica(&receiver).deliveries.is_empty());
    }
    #[test]
    fn transfers_cannot_be_replayed_into_a_different_hosts_vault() {
        let original = Store::default();
        let mut other = Store::default();
        let mut sender = Inventory::fixture(vec![fixture_item()]);
        sender
            .begin_deposit(1, &original.replica(&sender).identity)
            .unwrap();
        let token = sender.pending_deposits()[0].token.clone();
        assert!(other
            .request(&sender, Request::Deposit(token), true)
            .is_err());
        assert!(other.save.entries.is_empty());
        assert_eq!(sender.pending_deposits().len(), 1);
    }
    #[test]
    fn blocked_vault_does_not_allow_items_to_leave_the_bag() {
        let store = Store {
            blocked: true,
            ..default()
        };
        let mut sender = Inventory::fixture(vec![fixture_item()]);
        assert!(sender
            .begin_deposit(1, &store.replica(&sender).identity)
            .is_err());
        assert!(sender.item(1).is_some());
    }
}
