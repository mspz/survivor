//! The host runs the existing game rules; guests only send intent and render replicated units.
use super::{
    protocol as wire, transport::Socket, Companion, Companions, GuestActions, RemoteOwner, Role,
};
use crate::creature_anim::{
    CastEvent, CastEventKind, MovementState, PlaySeq, SpellGoTargets, SwingMessage,
};
use crate::net::{FieldChanged, Guid, NetEntity, ObjectStore};
use crate::survivors::{
    self as game, abilities, data, hero, hud, mobility, session, units, Owned, Phase, Rng, Run,
    RunEntity, Selection,
};
use benilla_protocol::{
    messages::{ObjectFields, ObjectType},
    EntityKind,
};
use benilla_world::schedule::WorldStage;
use bevy::{
    ecs::{message::MessageCursor, system::SystemId},
    prelude::*,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    net::{TcpListener, TcpStream, ToSocketAddrs},
    time::Duration,
};
pub(super) mod address;
mod prediction;
mod rooms;
mod saves;

struct Peer {
    money_earned: u64,
    trade: game::items::TradeAccount,
    socket: Socket,
    session: Option<session::LocalSession>,
    profile: Option<wire::Profile>,
    sequence: u64,
    held: wire::Input,
    since_input: f32,
    loot: Vec<(u64, String)>,
    next_loot: u64,
}

struct Guest {
    socket: Socket,
    id: Option<u64>,
    joined: bool,
    sequence: u64,
    tick: u64,
    offer: u64,
    entities: BTreeMap<u64, Entity>,
    since_input: f32,
    pending_jump: bool,
    pending_mobility: bool,
    save: Option<wire::ClientSave>,
    save_key: Option<saves::Signature>,
    receipts: saves::Receipts,
}

#[derive(Resource)]
struct Network {
    coins: MessageCursor<game::enemies::Loot>,
    connecting: Option<(
        std::net::SocketAddr,
        crossbeam_channel::Receiver<Result<TcpStream, String>>,
    )>,
    listener: Option<TcpListener>,
    peers: BTreeMap<u64, Peer>,
    guest: Option<Guest>,
    next_peer: u64,
    tick: u64,
    since_snapshot: f32,
    effects: Vec<wire::Effect>,
    casts: MessageCursor<CastEvent>,
    gos: MessageCursor<SpellGoTargets>,
    swings: MessageCursor<SwingMessage>,
    loot: MessageCursor<game::items::ChestOpened>,
    numbers: MessageCursor<units::CombatNumber>,
}

#[derive(Resource)]
struct Rules {
    mobility: SystemId,
    movement: SystemId,
    casts: SystemId,
    vitals: SystemId,
    gems: SystemId,
    items: SystemId,
    talents: SystemId,
}

#[derive(Component)]
struct Replicated;

#[derive(Component)]
struct Interpolation {
    from: Transform,
    to: Transform,
    elapsed: f32,
}

pub(super) fn plugin(app: &mut App) {
    let host = crate::webenv::var("SURVIVORS_HOST");
    let join = crate::webenv::var("SURVIVORS_JOIN");
    assert!(
        host.is_none() || join.is_none(),
        "Use either SURVIVORS_HOST or SURVIVORS_JOIN"
    );
    let (role, listener, guest) = if let Some(address) = host {
        let listener = TcpListener::bind(&address)
            .unwrap_or_else(|e| panic!("Cannot host Survivors at {address}: {e}"));
        listener
            .set_nonblocking(true)
            .expect("nonblocking listener");
        info!(
            "Survivors host listening on {address} ({} players maximum)",
            wire::MAX_PLAYERS
        );
        (Role::Host, Some(listener), None)
    } else if let Some(address) = join {
        let addresses = address
            .to_socket_addrs()
            .unwrap_or_else(|e| panic!("Invalid Survivors address {address}: {e}"));
        let stream = addresses
            .filter_map(|a| TcpStream::connect_timeout(&a, Duration::from_secs(3)).ok())
            .next()
            .unwrap_or_else(|| panic!("Cannot connect to Survivors host {address}"));
        let socket = Socket::new(stream).expect("configure multiplayer socket");
        (
            Role::Guest,
            None,
            Some(Guest {
                socket,
                id: None,
                joined: false,
                sequence: 0,
                tick: 0,
                offer: 0,
                entities: BTreeMap::new(),
                since_input: 0.0,
                pending_jump: false,
                pending_mobility: false,
                save: None,
                save_key: None,
                receipts: default(),
            }),
        )
    } else {
        (Role::Solo, None, None)
    };
    if role != Role::Solo {
        app.world_mut()
            .resource_mut::<super::lobby::Lobby>()
            .command_line = true;
    }
    if listener.is_some() {
        let mut lobby = app.world_mut().resource_mut::<super::lobby::Lobby>();
        lobby.active = true;
        lobby.members = 1;
        lobby.revision += 1;
    }
    app.insert_resource(role).insert_resource(Network {
        connecting: None,
        listener,
        peers: BTreeMap::new(),
        guest,
        next_peer: 0,
        tick: 0,
        since_snapshot: 0.0,
        effects: Vec::new(),
        coins: default(),
        casts: default(),
        gos: default(),
        swings: default(),
        numbers: default(),
        loot: default(),
    });
    register_rules(app.world_mut());
    app.add_systems(
        Update,
        pump.after(game::client::input)
            .before(session::receive)
            .in_set(session::Stage::Input),
    )
    .add_systems(
        Update,
        simulate
            .after(WorldStage::Input)
            .before(game::enemies::spawn_enemies),
    )
    .add_systems(Update, interpolate.after(pump).before(hero::place_camera))
    .add_systems(
        Update,
        prediction::predict
            .after(pump)
            .in_set(session::Stage::Simulate),
    )
    .add_systems(PostUpdate, publish.after(game::forms::sync));
    app.add_systems(Update, rooms::update.after(super::lobby::input));
}

fn register_rules(world: &mut World) {
    let rules = Rules {
        mobility: world.register_system(mobility::cast_mobility),
        movement: world.register_system(hero::drive_hero),
        casts: world.register_system(abilities::cast_abilities),
        vitals: world.register_system(hero::hero_vitals),
        gems: world.register_system(game::enemies::collect_gems),
        items: world.register_system(game::items::combat::tick),
        talents: world.register_system(game::talent_runtime::tick),
    };
    world.insert_resource(rules);
}

fn profile(world: &mut World) -> Option<wire::Profile> {
    let selection = world.resource::<Selection>();
    let class = selection.class?;
    let &(display, race, sex) = selection.looks.get(selection.look?)?;
    let mut run = world.resource_mut::<Run>();
    if run.class.id != data::CLASSES[class].id {
        run.class = &data::CLASSES[class];
        run.equipment_version = None;
    }
    run.race = race;
    drop(run);
    let save = Some(saves::capture(world, class)?);
    if let Some(save) = &save {
        if world.resource::<Run>().talent_build != save.talents {
            game::talents::Account::from_shared(class, save.talent_xp, save.talents)
                .apply(class, &mut world.resource_mut::<Run>());
        }
    }
    Some(wire::Profile {
        name: world
            .get_resource::<game::characters::Roster>()
            .and_then(|roster| roster.current())
            .map_or_else(|| data::CLASSES[class].name.into(), |c| c.name.clone()),
        class,
        display,
        race,
        sex,
        save,
    })
}

fn valid_profile(profile: &wire::Profile) -> bool {
    data::CLASSES.get(profile.class).is_some()
        && profile.display != 0
        && (1..=8).contains(&profile.race)
        && profile.sex <= 1
}

fn to_command(input: &wire::Input) -> session::Command {
    if !input.active {
        return default();
    }
    session::Command {
        axes: Vec2::from_array(input.axes),
        turn: input.turn,
        facing: input
            .facing
            .map(Vec3::from_array)
            .map(|v| units::flat(v).normalize_or(Vec3::NEG_Z)),
        aim: input.aim.map(Vec3::from_array),
        jump: input.jump,
        mobility: input.mobility,
        active: input.active,
        ..default()
    }
}

fn pump(world: &mut World) {
    let Some(mut network) = world.remove_resource::<Network>() else {
        return;
    };
    if network.guest.is_some() {
        if let Err(error) = pump_guest(world, &mut network) {
            warn!("Survivors disconnected: {error}");
            rooms::close(world, &mut network);
            if let Some(mut lobby) = world.get_resource_mut::<super::lobby::Lobby>() {
                lobby.fail(format!("Connexion au salon interrompue : {error}"));
            }
            world.resource_mut::<Run>().banner = Some((format!("Disconnected: {error}"), 60.0));
            world.resource_mut::<NextState<Phase>>().set(Phase::Menu);
            set_title(world, "Warcraft Survivors");
        }
    } else if *world.resource::<Role>() == Role::Host {
        pump_host(world, &mut network);
    }
    world.insert_resource(network);
}

fn pump_host(world: &mut World, network: &mut Network) {
    if let Some(listener) = &network.listener {
        for _ in 0..wire::MAX_PLAYERS {
            let stream = match listener.accept() {
                Ok((stream, _)) => stream,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => {
                    warn!("Survivors accept: {e}");
                    break;
                }
            };
            let Ok(mut socket) = Socket::new(stream) else {
                continue;
            };
            if network.peers.len() + 1 >= wire::MAX_PLAYERS {
                let _ = socket.send(&wire::ToClient::Error("Session is full".into()));
                continue;
            }
            network.next_peer += 1;
            network.peers.insert(
                network.next_peer,
                Peer {
                    money_earned: 0,
                    trade: default(),
                    socket,
                    session: None,
                    profile: None,
                    sequence: 0,
                    held: default(),
                    since_input: 0.0,
                    loot: default(),
                    next_loot: 0,
                },
            );
        }
    }
    let mut disconnected = Vec::new();
    for (&key, peer) in &mut network.peers {
        peer.since_input += world.resource::<Time<Real>>().delta_secs();
        let messages = match peer.socket.receive::<wire::ToServer>() {
            Ok(messages) => messages,
            Err(_) => {
                disconnected.push(key);
                continue;
            }
        };
        let mut failed = false;
        for message in messages {
            match message {
                wire::ToServer::Join { version, profile } if peer.session.is_none() => {
                    if version != wire::VERSION || !valid_profile(&profile) {
                        let _ = peer.socket.send(&wire::ToClient::Error(
                            "Personnage ou version du jeu incompatible.".into(),
                        ));
                        failed = true;
                        break;
                    }
                    let session = world.resource_mut::<session::Sessions>().connect();
                    let id = session.wire_id();
                    peer.session = Some(session);
                    if peer
                        .socket
                        .send(&wire::ToClient::Welcome {
                            version: wire::VERSION,
                            id,
                        })
                        .is_err()
                    {
                        failed = true;
                        break;
                    }
                    peer.trade = profile
                        .save
                        .as_ref()
                        .map(|s| game::items::TradeAccount::decode(&s.inventory))
                        .unwrap_or_default();
                    peer.profile = Some(profile);
                    info!("Survivors client {id} joined");
                }
                wire::ToServer::Input(input)
                    if peer.session.is_some()
                        && input.sequence > peer.sequence
                        && input.valid() =>
                {
                    peer.sequence = input.sequence;
                    peer.since_input = 0.0;
                    // Edge actions survive multiple packets received in one server frame.
                    let jump = peer.held.jump || input.jump;
                    let mobility = peer.held.mobility || input.mobility;
                    peer.held = input;
                    peer.held.jump = jump;
                    peer.held.mobility = mobility;
                }
                wire::ToServer::Pick { offer, index } => {
                    if let Some(session) = &peer.session {
                        apply_pick(world, session.wire_id(), offer, index);
                    }
                }
                wire::ToServer::Save(save) => {
                    if let (Some(session), Some(profile)) = (&peer.session, &mut peer.profile) {
                        let changed = profile.save.as_ref().is_none_or(|old| {
                            old.inventory != save.inventory
                                || old.talents != save.talents
                                || old.look != save.look
                        });
                        if changed {
                            if let Some(mut companion) = world
                                .resource_mut::<Companions>()
                                .0
                                .remove(&session.wire_id())
                            {
                                if let Err(error) = saves::apply(
                                    world,
                                    companion.entity,
                                    &mut companion.run,
                                    profile.class,
                                    &save,
                                ) {
                                    warn!("Client save: {error}");
                                }
                                world
                                    .resource_mut::<Companions>()
                                    .0
                                    .insert(session.wire_id(), companion);
                            }
                        }
                        peer.trade = game::items::TradeAccount::decode(&save.inventory);
                        profile.save = Some(save);
                    }
                }
                wire::ToServer::ResetMeter => {
                    if peer.session.is_some() {
                        world.resource_scope(|world, mut players: Mut<Companions>| {
                            super::reset_meters(
                                &mut world.resource_mut::<Run>(),
                                Some(&mut players),
                            );
                        });
                    }
                }
                wire::ToServer::Chest(request) => {
                    if let Some(session) = &peer.session {
                        let result = (|| {
                            let accessible = world.resource::<Selection>().in_hub
                                && world
                                    .resource::<Companions>()
                                    .0
                                    .get(&session.wire_id())
                                    .is_some_and(|p| {
                                        world.get::<Transform>(p.entity).is_some_and(|t| {
                                            game::shared_chest::near(t.translation)
                                        })
                                    });
                            world
                                .get_resource_mut::<game::shared_chest::Store>()
                                .ok_or_else(|| "Coffre indisponible.".to_string())?
                                .request_for(
                                    &peer.trade.identity,
                                    &peer.trade.pending_deposits,
                                    request,
                                    accessible,
                                )
                        })();
                        if let Some(mut view) = world.get_resource_mut::<game::shared_chest::View>()
                        {
                            view.notices
                                .insert(session.wire_id(), result.err().unwrap_or_default());
                        }
                    }
                }
                wire::ToServer::LootAck(ack) => {
                    peer.loot.retain(|(id, _)| *id > ack);
                }
                wire::ToServer::Ping => {
                    if peer.socket.send(&wire::ToClient::Pong).is_err() {
                        failed = true;
                        break;
                    }
                }
                _ => {
                    failed = true;
                    break;
                }
            }
        }
        if failed {
            disconnected.push(key);
            continue;
        }
        if let Some(session) = &peer.session {
            let mut command = to_command(&peer.held);
            if peer.since_input > 0.25 {
                command = default();
            }
            if let Some(companion) = world.resource::<Companions>().0.get(&session.wire_id()) {
                if companion.run.pending_levels > 0 || companion.run.dead_t.is_some() {
                    command = default();
                }
                if let Some(aim) = command.aim {
                    let pos = world
                        .get::<Transform>(companion.entity)
                        .map_or(Vec3::ZERO, |tf| tf.translation);
                    if aim.distance(pos) > 200.0 {
                        command.aim = None;
                    }
                }
            }
            let _ = session.submit(&mut world.resource_mut::<session::Sessions>(), command);
            peer.held.jump = false;
            peer.held.mobility = false;
            if !world
                .resource::<Companions>()
                .0
                .contains_key(&session.wire_id())
                && peer.socket.idle()
            {
                if peer.socket.send(&wire::ToClient::Pong).is_err() {
                    disconnected.push(key);
                }
            }
        }
    }
    for key in disconnected {
        disconnect(world, network, key);
    }
    if let Some(mut lobby) = world.get_resource_mut::<super::lobby::Lobby>() {
        lobby.members(
            network
                .peers
                .values()
                .filter(|p| p.session.is_some())
                .count()
                + 1,
        );
    }
    set_title(
        world,
        &format!(
            "Warcraft Survivors — host · {}/{} players",
            network
                .peers
                .values()
                .filter(|p| p.session.is_some())
                .count()
                + 1,
            wire::MAX_PLAYERS
        ),
    );
}

fn disconnect(world: &mut World, network: &mut Network, key: u64) {
    let Some(peer) = network.peers.remove(&key) else {
        return;
    };
    if let Some(session) = peer.session {
        world.resource_scope(|world, mut sessions: Mut<session::Sessions>| {
            sessions.disconnect(
                session.id(),
                &mut world.resource_mut::<session::ServerInput>(),
            );
        });
        if let Some(companion) = world
            .resource_mut::<Companions>()
            .0
            .remove(&session.wire_id())
        {
            let _ = world.despawn(companion.entity);
        }
        let effects: Vec<Entity> = world
            .query::<(Entity, &super::OwnedEffect)>()
            .iter(world)
            .filter(|(_, owner)| owner.0 == session.wire_id())
            .map(|(entity, _)| entity)
            .collect();
        for entity in effects {
            let _ = world.despawn(entity);
        }
        world
            .resource_mut::<abilities::PendingHits>()
            .0
            .retain(|(_, hit)| hit.owner != Some(session.wire_id()));
        info!("Survivors client {} left", session.wire_id());
    }
}

fn apply_pick(world: &mut World, id: u64, offer_id: u64, index: usize) {
    let Some(mut companion) = world.resource_mut::<Companions>().0.remove(&id) else {
        return;
    };
    if companion.offer_id == offer_id
        && companion.run.pending_levels > 0
        && (index < companion.offer.len() || (3..=7).contains(&index))
    {
        hud::pick(
            index,
            &mut companion.run,
            &mut companion.offer,
            &mut world.resource_mut::<Rng>(),
            &mut NextState::Unchanged,
        );
        companion.offer_id += 1;
    }
    world.resource_mut::<Companions>().0.insert(id, companion);
}

fn spawn_companion(world: &mut World, id: u64, profile: &wire::Profile) -> Companion {
    let class = &data::CLASSES[profile.class];
    let mut run = Run {
        owner: Some(id),
        class,
        race: profile.race,
        elapsed: world.resource::<Run>().elapsed,
        ..default()
    };
    run.abilities.push(Owned {
        def: data::ability(class.start),
        level: 1,
        cd: 0.3,
    });
    run.hp = run.stats().max_hp;
    let serial = world.resource_mut::<Run>().guid();
    let pos =
        hero::arena(world.resource::<Selection>().stage_def()) + Vec3::X * ((id % 5) as f32 + 1.0);
    let entity = {
        let mut commands = world.commands();
        let entity = hero::spawn_hero(
            &mut commands,
            class,
            profile.display,
            profile.race,
            profile.sex,
            profile
                .save
                .as_ref()
                .map_or(1, |save| game::talents::level(save.talent_xp)),
            serial,
            pos,
        );
        commands
            .entity(entity)
            .remove::<hero::Hero>()
            .remove::<game::items::visuals::HeroLook>()
            .insert((
                RemoteOwner(id),
                Visibility::Inherited,
                game::unit_frames::Label(profile.name.clone()),
            ));
        entity
    };
    world.flush();
    if let Some(save) = &profile.save {
        if let Err(error) = saves::apply(world, entity, &mut run, profile.class, save) {
            warn!("Client save: {error}");
        }
        run.hp = run.stats().max_hp;
    }
    saves::sync_vitals(world, entity, &run);
    Companion {
        entity,
        run,
        physics: hero::Hero { vy: 0.0, air: 0.0 },
        offer: default(),
        offer_id: 1,
    }
}

fn simulate(world: &mut World) {
    if *world.resource::<Role>() != Role::Host {
        return;
    }
    let phase = *world.resource::<State<Phase>>().get();
    let combat = !world.resource::<Selection>().in_hub && super::runs_in(phase, Role::Host);
    if !(phase == Phase::Hub || super::runs_in(phase, Role::Host))
        || world
            .resource::<crate::loading_screen::LoadingScreen>()
            .covering()
    {
        return;
    }
    let mut network = world.remove_resource::<Network>().unwrap();
    let ids = {
        let rules = world.resource::<Rules>();
        [
            rules.mobility,
            rules.movement,
            rules.items,
            rules.talents,
            rules.casts,
            rules.gems,
            rules.vitals,
        ]
    };
    let local_entity = world
        .query_filtered::<Entity, With<hero::Hero>>()
        .single(world)
        .ok();
    let Some(local_entity) = local_entity else {
        world.insert_resource(network);
        return;
    };
    for peer in network.peers.values_mut() {
        let (Some(connection), Some(profile)) = (peer.session.as_ref(), peer.profile.as_ref())
        else {
            continue;
        };
        let id = connection.wire_id();
        let existing = world.resource_mut::<Companions>().0.remove(&id);
        let mut companion = match existing {
            Some(c) if world.get_entity(c.entity).is_ok() => c,
            _ => spawn_companion(world, id, profile),
        };
        let Some(local_hero) = world.entity_mut(local_entity).take::<hero::Hero>() else {
            continue;
        };
        world.entity_mut(companion.entity).insert(companion.physics);
        let host_run = world.remove_resource::<Run>().unwrap();
        companion.run.next_guid = host_run.next_guid;
        world.insert_resource(companion.run);
        world.insert_resource(super::HostPlayer {
            entity: local_entity,
            run: host_run,
        });
        let host_session = world.remove_resource::<session::LocalSession>().unwrap();
        world.insert_resource(peer.session.take().unwrap());
        let next = world.remove_resource::<NextState<Phase>>().unwrap();
        world.insert_resource(NextState::<Phase>::Unchanged);
        let host_hits = world.remove_resource::<Messages<abilities::Hit>>().unwrap();
        world.insert_resource(Messages::<abilities::Hit>::default());
        let pending_start = world.resource::<abilities::PendingHits>().0.len();
        let focus = world.resource::<crate::player::Player>().pos;
        // Registered systems keep their query/message cursors; this does not rebuild systems per frame.
        for (index, system) in ids.into_iter().enumerate() {
            if index >= 2 && !combat {
                continue;
            }
            if let Err(error) = world.run_system(system) {
                error!("Remote player rule failed: {error}");
            }
        }
        let mut remote_hits = world.remove_resource::<Messages<abilities::Hit>>().unwrap();
        world.insert_resource(host_hits);
        for mut hit in remote_hits.drain() {
            hit.owner = Some(id);
            world.write_message(hit);
        }
        for (_, hit) in &mut world.resource_mut::<abilities::PendingHits>().0[pending_start..] {
            hit.owner = Some(id);
        }
        companion.run = world.remove_resource::<Run>().unwrap();
        let mut host_run = world.remove_resource::<super::HostPlayer>().unwrap().run;
        host_run.next_guid = companion.run.next_guid;
        world.insert_resource(host_run);
        companion.physics = world
            .entity_mut(companion.entity)
            .take::<hero::Hero>()
            .unwrap_or(hero::Hero { vy: 0.0, air: 0.0 });
        world.entity_mut(local_entity).insert(local_hero);
        peer.session = world.remove_resource::<session::LocalSession>();
        world.insert_resource(host_session);
        world.insert_resource(next);
        world.resource_mut::<crate::player::Player>().pos = focus;
        if companion.run.pending_levels > 0 && companion.offer.choices.is_empty() {
            companion.offer.choices = hud::choices_for(
                &companion.run,
                &mut world.resource_mut::<Rng>(),
                &companion.offer.config,
            );
            companion.offer_id += 1;
        }
        world.resource_mut::<Companions>().0.insert(id, companion);
    }
    world.insert_resource(network);
}

fn pump_guest(world: &mut World, network: &mut Network) -> std::io::Result<()> {
    let guest = network.guest.as_mut().unwrap();
    if let Some(key) = guest.save_key.as_ref() {
        if world
            .get_resource::<game::characters::Roster>()
            .and_then(|r| r.active)
            != Some(key.0)
        {
            return Err(std::io::Error::other("Le personnage a quitté le salon."));
        }
    }
    let phase = *world.resource::<State<Phase>>().get();
    if !guest.joined
        && (phase != Phase::Menu
            || world
                .get_resource::<super::lobby::Lobby>()
                .is_some_and(|l| l.busy))
    {
        if let Some(profile) = profile(world) {
            guest.save = profile.save.clone();
            guest.save_key = saves::signature(world, profile.class);
            guest.socket.send(&wire::ToServer::Join {
                version: wire::VERSION,
                profile,
            })?;
            guest.joined = true;
        }
    }
    let local_id = world.resource::<session::LocalSession>().id();
    // Guest commands were queued by the same input adapter the solo frontend uses.
    let mut server_input = world.remove_resource::<session::ServerInput>().unwrap();
    world.resource_scope(|_, mut sessions: Mut<session::Sessions>| {
        sessions.receive(&mut server_input)
    });
    let command = server_input.for_client(local_id);
    world.insert_resource(server_input);
    guest.pending_jump |= command.jump;
    guest.pending_mobility |= command.mobility;
    guest.since_input += world.resource::<Time<Real>>().delta_secs();
    if guest.since_input >= 1.0 / 30.0 {
        guest.since_input = 0.0;
        if guest.joined {
            let key = world
                .resource::<Selection>()
                .class
                .and_then(|class| saves::signature(world, class));
            if key != guest.save_key {
                if let Some(profile) = profile(world) {
                    if guest.save != profile.save {
                        if let Some(save) = profile.save {
                            guest.socket.send(&wire::ToServer::Save(save.clone()))?;
                            guest.save = Some(save);
                        }
                    }
                    guest.save_key = key;
                }
            }
            guest
                .socket
                .send(&wire::ToServer::LootAck(guest.receipts.loot))?;
            let vault = world
                .get_resource::<game::shared_chest::View>()
                .map(|v| v.replica.identity.clone())
                .unwrap_or_default();
            let mut chest_requests = world
                .get_resource_mut::<game::shared_chest::View>()
                .map(|mut v| std::mem::take(&mut v.requests))
                .unwrap_or_default();
            if let Some(inventory) = world.get_resource::<game::items::Inventory>() {
                chest_requests.extend(
                    inventory
                        .pending_deposits()
                        .iter()
                        .filter(|t| t.vault == vault)
                        .map(|t| game::shared_chest::Request::Deposit(t.token.clone())),
                );
            }
            if world
                .get_resource_mut::<GuestActions>()
                .is_some_and(|mut a| std::mem::take(&mut a.reset_meter))
            {
                guest.socket.send(&wire::ToServer::ResetMeter)?;
            }
            for request in chest_requests {
                guest.socket.send(&wire::ToServer::Chest(request))?;
            }
            guest.sequence += 1;
            guest.socket.send(&wire::ToServer::Input(wire::Input {
                sequence: guest.sequence,
                axes: command.axes.to_array(),
                turn: command.turn,
                facing: command.facing.map(|v| v.to_array()),
                aim: command.aim.map(|v| v.to_array()),
                jump: std::mem::take(&mut guest.pending_jump),
                mobility: std::mem::take(&mut guest.pending_mobility),
                active: command.active,
            }))?;
            if let Some(index) = world.resource_mut::<GuestActions>().pick.take() {
                guest.socket.send(&wire::ToServer::Pick {
                    offer: guest.offer,
                    index,
                })?;
            }
        } else {
            guest.socket.send(&wire::ToServer::Ping)?;
        }
    }
    for message in guest.socket.receive::<wire::ToClient>()? {
        match message {
            wire::ToClient::Welcome { version, id } if version == wire::VERSION => {
                guest.id = Some(id);
                if let Some(mut lobby) = world.get_resource_mut::<super::lobby::Lobby>() {
                    if lobby.busy {
                        lobby.busy = false;
                        lobby.joining = false;
                        lobby.status.clear();
                        lobby.error.clear();
                        lobby.revision += 1;
                        world.resource_mut::<NextState<Phase>>().set(Phase::Hub);
                    }
                }
                set_title(
                    world,
                    &format!("Warcraft Survivors — connected as player {id}"),
                );
            }
            wire::ToClient::Snapshot(snapshot) if snapshot.tick > guest.tick => {
                guest.tick = snapshot.tick;
                apply_snapshot(world, guest, snapshot);
            }
            wire::ToClient::Error(error) => return Err(std::io::Error::other(error)),
            _ => {}
        }
    }
    Ok(())
}

fn set_title(world: &mut World, text: &str) {
    for mut window in world.query::<&mut Window>().iter_mut(world) {
        if window.title != text {
            window.title = text.into();
        }
    }
}

fn encode_choice(choice: &hud::Choice) -> wire::Choice {
    match *choice {
        hud::Choice::NumericPassive(p, n) => wire::Choice::Numeric(p.key.into(), n),
        hud::Choice::NewAbility(a) => wire::Choice::Spell(a.key.into()),
        hud::Choice::Upgrade(i) => wire::Choice::Upgrade(i),
        hud::Choice::NewPassive(p) => wire::Choice::Passive(p.key.into()),
        hud::Choice::UpPassive(i) => wire::Choice::PassiveUpgrade(i),
        hud::Choice::Heal => wire::Choice::Heal,
    }
}

fn decode_choice(choice: wire::Choice) -> Option<hud::Choice> {
    Some(match choice {
        wire::Choice::Numeric(k, n) => {
            hud::Choice::NumericPassive(data::PASSIVES.iter().find(|p| p.key == k)?, n)
        }
        wire::Choice::Spell(k) => {
            hud::Choice::NewAbility(data::ABILITIES.iter().find(|a| a.key == k)?)
        }
        wire::Choice::Upgrade(i) => hud::Choice::Upgrade(i),
        wire::Choice::Passive(k) => {
            hud::Choice::NewPassive(data::PASSIVES.iter().find(|p| p.key == k)?)
        }
        wire::Choice::PassiveUpgrade(i) => hud::Choice::UpPassive(i),
        wire::Choice::Heal => hud::Choice::Heal,
    })
}

fn stats(run: &Run, offer: &hud::Offer, offer_id: u64) -> wire::Stats {
    wire::Stats {
        auras: game::player_auras::collect(run),
        healing_meter: run.healing_meter.clone(),
        taken_meter: run.taken_meter.clone(),
        movement_speed: run.stats().speed * run.haste_mult(),
        meter: run.meter.clone(),
        boss_meter: run.boss_meter.clone(),
        earned: game::talents::earned(run),
        hp: run.hp,
        max_hp: run.stats().max_hp,
        elapsed: run.elapsed,
        level: run.level,
        character_level: run.meta_level,
        xp: run.xp,
        kills: run.kills,
        dead: run.dead_t.is_some(),
        revival: [run.revival.seconds, run.revival.assisting],
        coop_revive: run.revival.enabled,
        won: run.won,
        abilities: run
            .abilities
            .iter()
            .map(|o| (o.def.key.into(), o.level, o.cd))
            .collect(),
        passives: run
            .passives
            .iter()
            .map(|(p, n)| (p.key.into(), *n))
            .collect(),
        offer_id,
        draft_uses: run.draft_uses,
        choices: offer.choices().iter().map(encode_choice).collect(),
        mobility: [
            run.mobile.cd,
            run.mobile.haste,
            run.mobile.haste_t,
            run.mobile.vanish_t,
        ],
        shield: [run.shield, run.shield_t],
        immune: run.immune_t,
        support_bonuses: game::healer_buffs::snapshot(run),
        damage_done: run.damage_done,
    }
}

fn kind(kind: EntityKind) -> u8 {
    match kind {
        EntityKind::Player => 0,
        EntityKind::Unit => 1,
        EntityKind::GameObject => 2,
        EntityKind::DynamicObject => 3,
        EntityKind::Corpse => 4,
        EntityKind::Other => 5,
    }
}

fn object_kind(kind: u8) -> (EntityKind, ObjectType) {
    match kind {
        0 => (EntityKind::Player, ObjectType::Player),
        1 => (EntityKind::Unit, ObjectType::Unit),
        2 => (EntityKind::GameObject, ObjectType::GameObject),
        3 => (EntityKind::DynamicObject, ObjectType::DynamicObject),
        4 => (EntityKind::Corpse, ObjectType::Corpse),
        _ => (EntityKind::Other, ObjectType::Object),
    }
}

fn publish(world: &mut World) {
    if *world.resource::<Role>() != Role::Host {
        return;
    }
    let mut network = world.remove_resource::<Network>().unwrap();
    let opened = world
        .get_resource::<Messages<game::items::ChestOpened>>()
        .map(|messages| {
            network
                .loot
                .read(messages)
                .map(|loot| loot.level)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let money = world
        .get_resource::<Messages<game::enemies::Loot>>()
        .map(|messages| {
            network
                .coins
                .read(messages)
                .map(|drop| game::currency::reward(drop.item_level, drop.boss))
                .fold(0u64, u64::saturating_add)
        })
        .unwrap_or(0);
    for peer in network.peers.values_mut() {
        let Some(connection) = &peer.session else {
            continue;
        };
        let Some(player) = world.resource::<Companions>().0.get(&connection.wire_id()) else {
            continue;
        };
        let (class, magic_find) = (player.run.class.id, player.run.equipment.magic_find);
        peer.money_earned = peer
            .money_earned
            .saturating_add(money)
            .min(game::currency::MAX_BALANCE);
        for &level in &opened {
            let item = world.resource_scope(|world, mut rng: Mut<Rng>| {
                let inventory = world.get_resource::<game::items::Inventory>()?;
                inventory
                    .config
                    .generate_loot(level, class, magic_find, &mut rng)
                    .ok()
            });
            if let Some(item) = item {
                if let Ok(encoded) = toml::to_string(&item) {
                    peer.next_loot += 1;
                    peer.loot.push((peer.next_loot, encoded));
                }
            }
        }
    }
    let entity_guids: BTreeMap<Entity, u64> = world
        .query::<(Entity, &Guid)>()
        .iter(world)
        .map(|(e, g)| (e, g.0))
        .collect();
    let guid = |e: Entity| entity_guids.get(&e).copied();
    if let Some(numbers) = world.get_resource::<Messages<units::CombatNumber>>() {
        for number in network.numbers.read(numbers) {
            if let Some(target) = guid(number.anchor) {
                network.effects.push(wire::Effect::Number {
                    owner: number.owner,
                    target,
                    amount: number.amount,
                    crit: number.crit,
                    color: number.color,
                });
            }
        }
    }
    for go in network
        .gos
        .read(world.resource::<Messages<SpellGoTargets>>())
    {
        if let Some(caster) = guid(go.caster) {
            network.effects.push(wire::Effect::Cast {
                caster,
                spell: go.spell_id,
                targets: go.hits.iter().filter_map(|e| guid(*e)).collect(),
                dest: go.dest.map(|v| v.to_array()),
            });
        }
    }
    for cast in network.casts.read(world.resource::<Messages<CastEvent>>()) {
        if let CastEventKind::Impact { .. } = cast.kind {
            if let Some(target) = guid(cast.entity) {
                network.effects.push(wire::Effect::Impact {
                    target,
                    spell: cast.spell_id,
                });
            }
        }
    }
    for swing in network
        .swings
        .read(world.resource::<Messages<SwingMessage>>())
    {
        if let Some(attacker) = guid(swing.attacker) {
            network.effects.push(wire::Effect::Swing {
                attacker,
                victim: swing.victim.and_then(guid),
                damage: swing.damage,
            });
        }
    }
    network.since_snapshot += world.resource::<Time<Real>>().delta_secs();
    if network.since_snapshot < 0.1 {
        world.insert_resource(network);
        return;
    }
    network.since_snapshot = 0.0;
    network.tick += 1;
    let phase = if world.resource::<Selection>().in_hub {
        Phase::Hub
    } else {
        *world.resource::<State<Phase>>().get()
    };
    let units: Vec<wire::Unit> = world
        .query_filtered::<(
            &Guid,
            &NetEntity,
            &ObjectStore,
            &Transform,
            Option<&MovementState>,
            Option<&RemoteOwner>,
            Option<&game::unit_frames::Status>,
            Has<game::enemies::StillPickup>,
            Has<hero::Hero>,
            Option<&game::enemies::Rank>,
        ), With<RunEntity>>()
        .iter(world)
        .map(
            |(guid, net, store, tf, movement, owner, status, static_pickup, local, rank)| wire::Unit {
                rank: rank.copied(),
                party_player: local || owner.is_some(),
                static_pickup,
                status: status.cloned(),
                guid: guid.0,
                owner: owner.map(|o| o.0),
                kind: kind(net.kind),
                display: net.display_id,
                scale: net.scale,
                position: tf.translation.to_array(),
                rotation: tf.rotation.to_array(),
                speed: movement.map_or(0.0, |m| m.speed),
                vertical_speed: movement.map_or(0.0, |m| m.vertical_speed),
                flags: movement.map_or(0, |m| m.flags),
                fields: store.0.raw_fields().collect(),
            },
        )
        .collect();
    let mut entries = BTreeSet::new();
    for unit in &units {
        if unit.kind == 0 {
            for &(i, entry) in &unit.fields {
                if (260..=476).contains(&i) && (i - 260) % 12 == 0 && entry != 0 {
                    entries.insert(entry);
                }
            }
        }
    }
    let equipment = world
        .get_resource::<crate::items::Items>()
        .map(|items| {
            entries
                .into_iter()
                .filter_map(|entry| {
                    let item = items.template_cached(entry)?;
                    Some((
                        entry,
                        wire::Appearance {
                            display_id: item.display_info_id,
                            inventory_type: item.inventory_type,
                            item_class: item.class,
                            subclass: item.subclass,
                            material: item.material,
                            sheath: item.sheath,
                        },
                    ))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let selection = world.resource::<Selection>();
    let stage = selection.stage;
    let in_hub = selection.in_hub;
    let effects = std::mem::take(&mut network.effects);
    let mut party = Vec::new();
    let host_name = world
        .get_resource::<game::characters::Roster>()
        .and_then(|r| r.current())
        .map_or_else(|| "Hôte".into(), |c| c.name.clone());
    let summary = |id, name, run: &Run| {
        let rows = run.meter.rows(run.elapsed);
        game::combat_ui::PartyRow {
            id,
            name,
            class: run.class.id,
            dps: rows.iter().map(|r| r.dps).sum(),
            total: rows.iter().map(|r| r.total).sum(),
        }
    };
    party.push(summary(0, host_name, world.resource::<Run>()));
    for (&id, player) in &world.resource::<Companions>().0 {
        let name = world
            .get::<game::unit_frames::Status>(player.entity)
            .map_or_else(|| player.run.class.name.into(), |s| s.name.clone());
        party.push(summary(id, name, &player.run));
    }
    world.insert_resource(game::combat_ui::PartyDamage(party.clone()));
    let mut disconnected = Vec::new();
    for (&key, peer) in &mut network.peers {
        let Some(connection) = &peer.session else {
            continue;
        };
        let Some(companion) = world.resource::<Companions>().0.get(&connection.wire_id()) else {
            continue;
        };
        // Slow peers keep at most a bounded queue. The next complete state catches them up.
        if !peer.socket.idle() {
            continue;
        }
        let mut chest = world
            .get_resource::<game::shared_chest::Store>()
            .map(|s| s.replica_for(&peer.trade.identity, &peer.trade.pending_deposits))
            .unwrap_or_default();
        if !in_hub {
            chest.entries.clear();
        }
        if let Some(notice) = world
            .get_resource::<game::shared_chest::View>()
            .and_then(|v| v.notices.get(&connection.wire_id()))
            .filter(|s| !s.is_empty())
        {
            chest.notice = notice.clone();
        }
        let snapshot = wire::Snapshot {
            boss_warnings: world
                .get_resource::<game::boss_spells::Warnings>()
                .map_or_else(Vec::new, |w| w.0.clone()),
            money_earned: peer.money_earned,
            chest,
            party: party.clone(),
            run_id: world.get::<Guid>(companion.entity).map_or(0, |g| g.0),
            loot: peer.loot.clone(),
            tick: network.tick,
            acknowledged: peer.sequence,
            phase: encode_phase(phase, companion),
            map: stage,
            in_hub,
            stats: stats(&companion.run, &companion.offer, companion.offer_id),
            units: units.clone(),
            effects: effects
                .iter()
                .filter(|e| visible_to(e, Some(connection.wire_id())))
                .cloned()
                .collect(),
            equipment: equipment.clone(),
        };
        if peer
            .socket
            .send(&wire::ToClient::Snapshot(snapshot))
            .is_err()
        {
            disconnected.push(key);
        }
    }
    for key in disconnected {
        disconnect(world, &mut network, key);
    }
    world.insert_resource(network);
}

fn encode_phase(phase: Phase, companion: &Companion) -> u8 {
    if phase == Phase::GameOver {
        return 6;
    }
    if matches!(phase, Phase::Menu | Phase::Hub) {
        return 1;
    }
    if companion.run.dead_t.is_none() && companion.run.pending_levels > 0 {
        return 3;
    }
    match phase {
        Phase::Menu => 0,
        Phase::Hub => 1,
        Phase::Playing => 2,
        Phase::LevelUp | Phase::Paused | Phase::CharacterSheet => 2,
        Phase::GameOver => 6,
    }
}

fn received_phase(remote: u8, in_hub: bool, choosing_reward: bool) -> Phase {
    // A snapshot can announce world transitions and this guest's rewards, never host menus.
    if remote == 6 {
        Phase::GameOver
    } else if in_hub {
        Phase::Hub
    } else if choosing_reward {
        Phase::LevelUp
    } else {
        Phase::Playing
    }
}

fn local_phase(current: Phase, remote: Phase, offer_changed: bool) -> Phase {
    // Only expedition transitions belong to the host. Personal menus stay on this machine.
    if matches!(current, Phase::Paused | Phase::CharacterSheet)
        && matches!(remote, Phase::Hub | Phase::Playing | Phase::LevelUp)
    {
        current
    } else if current == Phase::LevelUp && remote == Phase::LevelUp && !offer_changed {
        current
    } else {
        remote
    }
}

fn apply_snapshot(world: &mut World, guest: &mut Guest, snapshot: wire::Snapshot) {
    world.insert_resource(game::boss_spells::Warnings(snapshot.boss_warnings.clone()));
    if world.contains_resource::<game::items::Inventory>()
        && world.contains_resource::<game::shared_chest::View>()
    {
        world.resource_scope(|world, mut inventory: Mut<game::items::Inventory>| {
            game::shared_chest::receive(
                snapshot.chest.clone(),
                &mut inventory,
                &mut world.resource_mut::<game::shared_chest::View>(),
            );
        });
    }
    world.insert_resource(game::combat_ui::PartyDamage(snapshot.party.clone()));
    let members = snapshot
        .units
        .iter()
        .filter_map(|u| u.owner)
        .collect::<BTreeSet<_>>()
        .len()
        + 1;
    if let Some(mut lobby) = world.get_resource_mut::<super::lobby::Lobby>() {
        lobby.members(members);
    }
    saves::persist(
        world,
        &mut guest.receipts,
        &snapshot,
        guest.save_key.as_ref().map(|k| k.0),
    );
    let offer_changed = guest.offer != snapshot.stats.offer_id;
    guest.offer = snapshot.stats.offer_id;
    let old_heroes: Vec<Entity> = world
        .query_filtered::<Entity, (With<hero::Hero>, Without<Replicated>)>()
        .iter(world)
        .collect();
    for entity in old_heroes {
        let _ = world.despawn(entity);
    }
    if let Some(mut items) = world.get_resource_mut::<crate::items::Items>() {
        for (entry, a) in snapshot.equipment {
            items.insert_template(
                entry,
                Some(game::items::visuals::template(
                    game::items::visuals::Appearance {
                        display_id: a.display_id,
                        inventory_type: a.inventory_type,
                        item_class: a.item_class,
                        subclass: a.subclass,
                        material: a.material,
                        sheath: a.sheath,
                    },
                )),
            );
        }
    }
    {
        let mut selection = world.resource_mut::<Selection>();
        selection.stage = snapshot.map.min(data::MAPS.len() - 1);
        selection.in_hub = snapshot.in_hub;
    }
    let present: BTreeSet<u64> = snapshot.units.iter().map(|u| u.guid).collect();
    guest.entities.retain(|guid, entity| {
        if present.contains(guid) && world.get_entity(*entity).is_ok() {
            true
        } else {
            let _ = world.despawn(*entity);
            false
        }
    });
    for unit in snapshot.units {
        let (net_kind, object_type) = object_kind(unit.kind);
        let fields = ObjectFields::from_pairs(&unit.fields).into_created(object_type);
        let to = Transform::from_translation(Vec3::from_array(unit.position))
            .with_rotation(Quat::from_array(unit.rotation).normalize());
        let movement = MovementState {
            speed: unit.speed,
            vertical_speed: unit.vertical_speed,
            flags: unit.flags,
            ..default()
        };
        let entity = if let Some(&entity) = guest.entities.get(&unit.guid) {
            let from = world.get::<Transform>(entity).copied().unwrap_or(to);
            let mut changes = Vec::new();
            if let Some(mut store) = world.get_mut::<ObjectStore>(entity) {
                store.0.merge_diff(fields, |index, old, new| {
                    changes.push(FieldChanged {
                        entity,
                        guid: unit.guid,
                        kind: object_type,
                        index,
                        old,
                        new,
                    })
                });
            }
            for change in changes {
                world.write_message(change);
            }
            if let Some(mut net) = world.get_mut::<NetEntity>(entity) {
                net.display_id = unit.display;
                net.kind = net_kind;
                net.scale = unit.scale;
            }
            world.entity_mut(entity).insert((
                movement,
                Interpolation {
                    from,
                    to,
                    elapsed: 0.0,
                },
            ));
            entity
        } else {
            let entity = world
                .spawn((
                    Guid(unit.guid),
                    NetEntity {
                        kind: net_kind,
                        display_id: unit.display,
                        scale: unit.scale,
                    },
                    ObjectStore(fields),
                    movement,
                    to,
                    Visibility::Inherited,
                    crate::net::ScriptedFacing,
                    RunEntity,
                    Replicated,
                    Interpolation {
                        from: to,
                        to,
                        elapsed: 0.0,
                    },
                ))
                .id();
            guest.entities.insert(unit.guid, entity);
            entity
        };
        if let Some(rank) = unit.rank {
            world.entity_mut(entity).insert(rank);
        } else {
            world.entity_mut(entity).remove::<game::enemies::Rank>();
        }
        if unit.party_player {
            world
                .entity_mut(entity)
                .insert(game::unit_frames::PartyPlayer);
        } else {
            world
                .entity_mut(entity)
                .remove::<game::unit_frames::PartyPlayer>();
        }
        if unit.static_pickup {
            world.entity_mut(entity).insert(game::enemies::StillPickup);
        } else {
            world
                .entity_mut(entity)
                .remove::<game::enemies::StillPickup>();
        }
        if let Some(status) = unit.status {
            world.entity_mut(entity).insert(status);
        } else {
            world
                .entity_mut(entity)
                .remove::<game::unit_frames::Status>();
        }
        if unit.owner == guest.id && guest.id.is_some() {
            prediction::receive(
                world,
                entity,
                prediction::Authority {
                    transform: to,
                    vertical_speed: unit.vertical_speed,
                    acknowledged: snapshot.acknowledged,
                    speed: snapshot.stats.movement_speed,
                    map: snapshot.map,
                    in_hub: snapshot.in_hub,
                    dead: snapshot.stats.dead,
                },
            );
            let selection = world.resource::<Selection>();
            if let (Some(class), Some(&(display, race, sex))) = (
                selection.class,
                selection.look.and_then(|i| selection.looks.get(i)),
            ) {
                let look = game::items::visuals::HeroLook {
                    display,
                    race,
                    sex,
                    class: data::CLASSES[class].id,
                };
                world.entity_mut(entity).insert(look);
            }
        }
    }
    let stats = snapshot.stats;
    world.insert_resource(game::player_auras::Remote(stats.auras.clone()));
    let class = world
        .resource::<Selection>()
        .class
        .and_then(|i| data::CLASSES.get(i));
    {
        let mut run = world.resource_mut::<Run>();
        let loadout_changed = run
            .abilities
            .iter()
            .map(|o| (o.def.key, o.level))
            .ne(stats.abilities.iter().map(|(k, l, _)| (k.as_str(), *l)))
            || run
                .passives
                .iter()
                .map(|(p, n)| (p.key, *n))
                .ne(stats.passives.iter().map(|(k, n)| (k.as_str(), *n)));
        if let Some(class) = class {
            run.class = class;
        }
        run.hp = stats.hp;
        run.elapsed = stats.elapsed;
        run.level = stats.level;
        run.meta_level = stats.character_level.max(1);
        run.draft_uses = stats.draft_uses;
        run.xp = stats.xp;
        run.kills = stats.kills;
        run.won = stats.won;
        run.mobile = mobility::MobilityState {
            cd: stats.mobility[0],
            haste: stats.mobility[1],
            haste_t: stats.mobility[2],
            vanish_t: stats.mobility[3],
        };
        run.shield = stats.shield[0];
        run.shield_t = stats.shield[1];
        run.immune_t = stats.immune;
        game::healer_buffs::restore(&mut run, stats.support_bonuses);
        run.damage_done = stats.damage_done;
        run.meter = stats.meter.clone();
        run.boss_meter = stats.boss_meter.clone();
        run.healing_meter = stats.healing_meter.clone();
        run.taken_meter = stats.taken_meter.clone();
        run.dead_t = stats.dead.then_some(0.0);
        run.revival.enabled = stats.coop_revive;
        run.revival.seconds = stats.revival[0];
        run.revival.assisting = stats.revival[1];
        run.abilities = stats
            .abilities
            .iter()
            .filter_map(|(key, level, cd)| {
                data::ABILITIES
                    .iter()
                    .find(|a| a.key == key)
                    .map(|def| Owned {
                        def,
                        level: *level,
                        cd: *cd,
                    })
            })
            .collect();
        run.passives = stats
            .passives
            .iter()
            .filter_map(|(key, n)| {
                data::PASSIVES
                    .iter()
                    .find(|p| p.key == key)
                    .map(|p| (p, *n))
            })
            .collect();
        if loadout_changed {
            run.version = run.version.wrapping_add(1);
        }
    }
    let choosing_reward = !stats.dead && !stats.choices.is_empty();
    if offer_changed {
        world.resource_mut::<game::cards::PendingPick>().clear();
        world.resource_mut::<hud::Offer>().choices = stats
            .choices
            .into_iter()
            .filter_map(decode_choice)
            .collect();
    }
    let phase = received_phase(snapshot.phase, snapshot.in_hub, choosing_reward);
    let current = *world.resource::<State<Phase>>().get();
    let phase = local_phase(current, phase, offer_changed);
    if current != phase {
        world.resource_mut::<NextState<Phase>>().set(phase);
    }
    for effect in snapshot.effects {
        if visible_to(&effect, guest.id) {
            play_effect(world, &guest.entities, effect);
        }
    }
}

fn visible_to(effect: &wire::Effect, recipient: Option<u64>) -> bool {
    match effect {
        wire::Effect::Number { owner, .. } => *owner == recipient,
        _ => true,
    }
}

fn play_effect(world: &mut World, entities: &BTreeMap<u64, Entity>, effect: wire::Effect) {
    let seq = world.resource_mut::<PlaySeq>().next();
    match effect {
        wire::Effect::Number {
            owner: _,
            target,
            amount,
            crit,
            color,
        } => {
            if world
                .get_resource::<game::options::Settings>()
                .is_some_and(|s| {
                    !s.numbers(color == units::COLOR_HEAL, color == units::COLOR_HURT, crit)
                })
            {
                return;
            }
            if let Some(&anchor) = entities.get(&target) {
                world.write_message(crate::combat_text::CombatTextSpawn {
                    anchor,
                    text: (amount.round().max(1.0) as i64).to_string(),
                    category: if crit { 2 } else { 0 },
                    color: Some(color),
                });
            }
        }
        wire::Effect::Cast {
            caster,
            spell,
            targets,
            dest,
        } => {
            let Some(&caster) = entities.get(&caster) else {
                return;
            };
            world.write_message(CastEvent {
                entity: caster,
                spell_id: spell,
                kind: CastEventKind::Go,
                seq,
            });
            world.write_message(SpellGoTargets {
                caster,
                spell_id: spell,
                hits: targets
                    .iter()
                    .filter_map(|g| entities.get(g).copied())
                    .collect(),
                misses: Vec::new(),
                dest: dest.map(Vec3::from_array),
                ammo_display_id: None,
                seq,
            });
        }
        wire::Effect::Impact { target, spell } => {
            if let Some(&entity) = entities.get(&target) {
                world.write_message(CastEvent {
                    entity,
                    spell_id: spell,
                    kind: CastEventKind::Impact {
                        weapon_visual: None,
                    },
                    seq,
                });
            }
        }
        wire::Effect::Swing {
            attacker,
            victim,
            damage,
        } => {
            if let Some(&attacker) = entities.get(&attacker) {
                world.write_message(SwingMessage {
                    attacker,
                    victim: victim.and_then(|v| entities.get(&v).copied()),
                    hit_info: 2,
                    victim_state: 1,
                    damage,
                    displayed: false,
                    seq,
                });
            }
        }
    }
}

fn interpolate(
    time: Res<Time<Real>>,
    mut units: Query<(&mut Transform, &mut Interpolation), Without<hero::Hero>>,
) {
    for (mut tf, mut interpolation) in &mut units {
        interpolation.elapsed += time.delta_secs();
        let alpha = (interpolation.elapsed / 0.1).min(1.0);
        // Large jumps are teleports, not motion to interpolate across the map.
        if interpolation
            .from
            .translation
            .distance(interpolation.to.translation)
            > 15.0
        {
            *tf = interpolation.to;
        } else {
            tf.translation = interpolation
                .from
                .translation
                .lerp(interpolation.to.translation, alpha);
            tf.rotation = interpolation
                .from
                .rotation
                .slerp(interpolation.to.rotation, alpha);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn player_buff_snapshots_include_authoritative_effects_and_remaining_durations() {
        let mut run = game::Run::default();
        game::support_spells::grant(&mut run, "battle_shout", 0.4, 5.0, None);
        let snapshot = super::stats(&run, &game::hud::Offer::default(), 0);
        let bytes = serde_json::to_vec(&snapshot).unwrap();
        let decoded: super::wire::Stats = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded.auras.len(), 1);
        assert_eq!(decoded.auras[0].left, Some(5.0));
        assert!(decoded.auras[0].description.contains("40.0%"));
    }
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use std::time::Instant;

    #[test]
    fn host_menus_never_open_a_guest_menu_or_reward_picker() {
        for remote in [0, 1, 2, 3, 4, 5, 255] {
            assert_eq!(received_phase(remote, false, false), Phase::Playing);
            assert_eq!(received_phase(remote, true, false), Phase::Hub);
            assert_eq!(received_phase(remote, false, true), Phase::LevelUp);
            assert_eq!(
                local_phase(Phase::Paused, received_phase(remote, false, false), false),
                Phase::Paused
            );
        }
        assert_eq!(received_phase(6, false, true), Phase::GameOver);
    }

    #[test]
    fn snapshots_preserve_personal_menus_but_end_them_when_the_run_ends() {
        for menu in [Phase::Paused, Phase::CharacterSheet] {
            for remote in [Phase::Hub, Phase::Playing, Phase::LevelUp] {
                assert_eq!(local_phase(menu, remote, true), menu);
            }
            assert_eq!(local_phase(menu, Phase::GameOver, false), Phase::GameOver);
        }
        assert_eq!(
            local_phase(Phase::Playing, Phase::LevelUp, true),
            Phase::LevelUp
        );
        assert_eq!(
            local_phase(Phase::LevelUp, Phase::Playing, true),
            Phase::Playing
        );
    }

    #[test]
    fn floating_numbers_are_private_to_their_owner_and_guests_do_not_render_on_the_host() {
        for owner in [None, Some(42), Some(77)] {
            let effect = wire::Effect::Number {
                owner,
                target: 1,
                amount: 10.0,
                crit: false,
                color: units::COLOR_SPELL,
            };
            for recipient in [None, Some(42), Some(77)] {
                assert_eq!(visible_to(&effect, recipient), owner == recipient);
            }
        }
        let mut world = world();
        world.init_resource::<Messages<units::CombatNumber>>();
        let anchor = world.spawn_empty().id();
        world
            .run_system_once(move |mut fx: units::Fx| {
                fx.number(Some(42), anchor, 25.0, false, units::COLOR_SPELL);
                fx.number(None, anchor, 10.0, false, units::COLOR_SPELL);
            })
            .unwrap();
        assert_eq!(world.resource::<Messages<units::CombatNumber>>().len(), 2);
        let numbers: Vec<_> = world
            .resource_mut::<Messages<crate::combat_text::CombatTextSpawn>>()
            .drain()
            .collect();
        assert_eq!(numbers.len(), 1);
        assert_eq!(numbers[0].text, "10");
    }
    #[test]
    fn combat_numbers_use_the_receivers_filters_even_if_the_host_hides_them() {
        let mut world = world();
        world.init_resource::<Messages<units::CombatNumber>>();
        let mut settings = game::options::Settings::default();
        settings.damage = false;
        world.insert_resource(settings);
        let anchor = world.spawn_empty().id();
        world
            .run_system_once(move |mut fx: units::Fx| {
                fx.number(None, anchor, 26.6, true, units::COLOR_SPELL);
            })
            .unwrap();
        assert!(world
            .resource::<Messages<crate::combat_text::CombatTextSpawn>>()
            .is_empty());
        let number = world
            .resource_mut::<Messages<units::CombatNumber>>()
            .drain()
            .next()
            .unwrap();
        let entities = BTreeMap::from([(42, anchor)]);
        let effect = wire::Effect::Number {
            owner: number.owner,
            target: 42,
            amount: number.amount,
            crit: number.crit,
            color: number.color,
        };
        play_effect(&mut world, &entities, effect.clone());
        assert!(world
            .resource::<Messages<crate::combat_text::CombatTextSpawn>>()
            .is_empty());
        world.resource_mut::<game::options::Settings>().damage = true;
        play_effect(&mut world, &entities, effect);
        let text = world
            .resource_mut::<Messages<crate::combat_text::CombatTextSpawn>>()
            .drain()
            .next()
            .unwrap();
        assert_eq!(text.anchor, anchor);
        assert_eq!(text.text, "27");
        assert_eq!(text.category, 2);
    }

    fn world() -> World {
        let mut world = World::new();
        benilla_world::world_point::init_world_point_resources(&mut world);
        world.init_resource::<Time>();
        world.init_resource::<Time<Real>>();
        world.init_resource::<Run>();
        world.init_resource::<Rng>();
        world.init_resource::<Selection>();
        world.init_resource::<session::Sessions>();
        world.init_resource::<session::LocalSession>();
        world.init_resource::<session::ServerInput>();
        world.init_resource::<Companions>();
        world.init_resource::<GuestActions>();
        world.init_resource::<hud::Offer>();
        world.init_resource::<game::cards::PendingPick>();
        world.init_resource::<abilities::PendingHits>();
        world.init_resource::<NextState<Phase>>();
        world.init_resource::<crate::player::Player>();
        world.init_resource::<crate::loading_screen::LoadingScreen>();
        world.init_resource::<PlaySeq>();
        world.init_resource::<Messages<abilities::Hit>>();
        world.init_resource::<Messages<game::enemies::Loot>>();
        world.init_resource::<Messages<FieldChanged>>();
        world.init_resource::<Messages<CastEvent>>();
        world.init_resource::<Messages<SpellGoTargets>>();
        world.init_resource::<Messages<SwingMessage>>();
        world.init_resource::<Messages<crate::combat_text::CombatTextSpawn>>();
        world.insert_resource(State::new(Phase::Playing));
        world.insert_resource(Role::Host);
        register_rules(&mut world);
        let pos = hero::arena(world.resource::<Selection>().stage_def());
        let serial = world.resource_mut::<Run>().guid();
        hero::spawn_hero(
            &mut world.commands(),
            &data::CLASSES[0],
            data::CLASSES[0].looks[0],
            1,
            0,
            1,
            serial,
            pos,
        );
        world.flush();
        world
    }

    fn tick(world: &mut World) {
        let delta = Duration::from_millis(50);
        world.resource_mut::<Time>().advance_by(delta);
        world.resource_mut::<Time<Real>>().advance_by(delta);
        pump(world);
        world.run_system_once(session::receive).unwrap();
        simulate(world);
        publish(world);
    }

    #[test]
    fn all_specialization_runtimes_spawn_replicable_summons_and_resolve_support() {
        for class in 0..9 {
            let mut world = world();
            world.resource_mut::<Run>().class = &data::CLASSES[class];
            let mut build = [[0; 19]; 3];
            for tree in &mut build {
                for node in [8, 12, 18] {
                    tree[node] = 1;
                }
            }
            game::talents::Account::from_shared(class, 0, build)
                .apply(class, &mut world.resource_mut::<Run>());
            world
                .resource_mut::<Time>()
                .advance_by(Duration::from_millis(100));
            world.run_system_once(game::talent_runtime::tick).unwrap();
            world
                .run_system_once(game::talent_runtime::support)
                .unwrap();
            let pets = world
                .query::<(&game::talent_runtime::Summon, &NetEntity, &Guid)>()
                .iter(&world)
                .count();
            assert_eq!(
                pets,
                match class {
                    2 => 2,
                    5 => 2,
                    7 => 3,
                    _ => 0,
                },
                "{}",
                data::CLASSES[class].name
            );
            world.resource_mut::<Run>().elapsed = 2.0;
            world.run_system_once(game::talent_runtime::tick).unwrap();
            assert_eq!(
                world
                    .query::<&game::talent_runtime::Summon>()
                    .iter(&world)
                    .count(),
                pets
            );
        }
    }

    fn network(listener: TcpListener) -> Network {
        listener.set_nonblocking(true).unwrap();
        Network {
            connecting: None,
            listener: Some(listener),
            peers: default(),
            guest: None,
            next_peer: 0,
            tick: 0,
            since_snapshot: 0.0,
            effects: default(),
            coins: default(),
            casts: default(),
            gos: default(),
            swings: default(),
            numbers: default(),
            loot: default(),
        }
    }

    fn join(socket: &mut Socket, class: usize) {
        socket
            .send(&wire::ToServer::Join {
                version: wire::VERSION,
                profile: wire::Profile {
                    name: String::from("Test Player"),
                    class,
                    display: data::CLASSES[class].looks[0],
                    race: 1,
                    sex: 0,
                    save: None,
                },
            })
            .unwrap();
    }

    #[test]
    fn kill_money_is_given_to_every_guest_once_even_when_a_guest_is_dead() {
        let mut host = world();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        host.insert_resource(network(listener));
        let mut clients = [
            Socket::new(TcpStream::connect(address).unwrap()).unwrap(),
            Socket::new(TcpStream::connect(address).unwrap()).unwrap(),
        ];
        for (i, client) in clients.iter_mut().enumerate() {
            join(client, i);
        }
        let deadline = Instant::now() + Duration::from_secs(3);
        while host.resource::<Companions>().0.len() < 2 {
            tick(&mut host);
            for client in &mut clients {
                client.receive::<wire::ToClient>().unwrap();
            }
            assert!(Instant::now() < deadline);
        }
        host.resource_mut::<Companions>()
            .0
            .values_mut()
            .next()
            .unwrap()
            .run
            .hp = 0.0;
        host.write_message(game::enemies::Loot {
            item_level: 1,
            pos: Vec3::ZERO,
            xp: 1,
            boss: false,
        });
        host.write_message(game::enemies::Loot {
            item_level: 60,
            pos: Vec3::ZERO,
            xp: 1,
            boss: true,
        });
        publish(&mut host);
        assert!(host
            .resource::<Network>()
            .peers
            .values()
            .all(|p| p.money_earned == 50001));
        publish(&mut host);
        assert!(host
            .resource::<Network>()
            .peers
            .values()
            .all(|p| p.money_earned == 50001));
        let mut seen = [false; 2];
        while !seen.iter().all(|s| *s) {
            for (i, client) in clients.iter_mut().enumerate() {
                for message in client.receive::<wire::ToClient>().unwrap() {
                    if let wire::ToClient::Snapshot(snapshot) = message {
                        if snapshot.money_earned == 50001 {
                            seen[i] = true;
                        }
                    }
                }
            }
            assert!(Instant::now() < deadline);
        }
    }

    #[test]
    fn guest_opens_boss_chest_and_each_party_member_gets_exactly_one_item() {
        let mut host = world();
        host.insert_resource(game::items::Inventory::fixture(vec![]));
        host.init_resource::<Messages<game::items::ChestOpened>>();
        host.init_resource::<Messages<crate::sound::CoinSound>>();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        host.insert_resource(network(listener));
        let mut clients = [
            Socket::new(TcpStream::connect(address).unwrap()).unwrap(),
            Socket::new(TcpStream::connect(address).unwrap()).unwrap(),
        ];
        join(&mut clients[0], 0);
        join(&mut clients[1], 1);
        let deadline = Instant::now() + Duration::from_secs(3);
        while host.resource::<Companions>().0.len() < 2 {
            tick(&mut host);
            for client in &mut clients {
                client.receive::<wire::ToClient>().unwrap();
            }
            assert!(Instant::now() < deadline);
        }
        let pos = Vec3::new(10000.0, 0.0, 10000.0);
        host.resource_mut::<Messages<game::enemies::Loot>>()
            .write(game::enemies::Loot {
                item_level: 9,
                pos,
                xp: 0,
                boss: true,
            });
        host.run_system_once(game::items::chest_drops).unwrap();
        assert_eq!(
            host.query::<&game::items::ItemDrop>().iter(&host).count(),
            1
        );
        host.run_system_once(game::items::collect_drops).unwrap();
        tick(&mut host);
        assert!(host.resource::<game::items::Inventory>().all().is_empty());
        assert!(host
            .resource::<Network>()
            .peers
            .values()
            .all(|p| p.loot.is_empty()));
        let entities: Vec<_> = host
            .resource::<Companions>()
            .0
            .values()
            .map(|p| p.entity)
            .collect();
        for e in entities {
            host.get_mut::<Transform>(e).unwrap().translation = pos;
        }
        host.run_system_once(game::items::collect_drops).unwrap();
        assert_eq!(host.resource::<game::items::Inventory>().all().len(), 1);
        assert_eq!(
            host.query::<&game::items::ItemDrop>().iter(&host).count(),
            0
        );
        host.run_system_once(game::items::collect_drops).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut seen = [false; 2];
        while !seen.iter().all(|s| *s) {
            tick(&mut host);
            for (i, client) in clients.iter_mut().enumerate() {
                for message in client.receive::<wire::ToClient>().unwrap() {
                    if let wire::ToClient::Snapshot(s) = message {
                        if !s.loot.is_empty() {
                            assert_eq!(s.loot.len(), 1);
                            let item: game::items::Item = toml::from_str(&s.loot[0].1).unwrap();
                            assert_eq!(item.level, 9);
                            seen[i] = true;
                        }
                    }
                }
            }
            assert!(Instant::now() < deadline);
        }
        for _ in 0..3 {
            tick(&mut host);
        }
        assert_eq!(host.resource::<game::items::Inventory>().all().len(), 1);
        assert!(host
            .resource::<Network>()
            .peers
            .values()
            .all(|p| p.loot.len() == 1));
    }

    #[test]
    fn tcp_guest_can_reset_group_meters_without_changing_expedition_damage() {
        let mut host = world();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        host.insert_resource(network(listener));
        let mut client = Socket::new(TcpStream::connect(address).unwrap()).unwrap();
        join(&mut client, 0);
        let deadline = Instant::now() + Duration::from_secs(3);
        while host.resource::<Companions>().0.is_empty() {
            tick(&mut host);
            client.receive::<wire::ToClient>().unwrap();
            assert!(Instant::now() < deadline);
        }
        fn seed(run: &mut Run) {
            run.meter
                .record_hit("fireball", run.elapsed, 100.0, false, false);
            run.healing_meter
                .record_hit("flash_heal", run.elapsed, 20.0, false, false);
            run.taken_meter
                .record_hit("enemy", run.elapsed, 30.0, false, false);
            run.damage_done = 100.0;
        }
        seed(&mut host.resource_mut::<Run>());
        for p in host.resource_mut::<Companions>().0.values_mut() {
            seed(&mut p.run);
        }
        client.send(&wire::ToServer::ResetMeter).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            tick(&mut host);
            if client.receive::<wire::ToClient>().unwrap().into_iter().any(|message| {
                matches!(message, wire::ToClient::Snapshot(s) if !s.party.is_empty() && s.party.iter().all(|p| p.total == 0.0 && p.dps == 0.0) && s.stats.meter.rows(s.stats.elapsed).is_empty())
            }) { break; }
            assert!(Instant::now() < deadline);
        }
        let check = |run: &Run| {
            assert!(run.meter.rows(run.elapsed).is_empty());
            assert!(run.healing_meter.rows(run.elapsed).is_empty());
            assert!(run.taken_meter.rows(run.elapsed).is_empty());
            assert_eq!(run.damage_done, 100.0);
        };
        check(host.resource::<Run>());
        for p in host.resource::<Companions>().0.values() {
            check(&p.run);
        }
    }

    #[test]
    fn two_tcp_guests_receive_only_their_own_floating_numbers() {
        let mut host = world();
        host.init_resource::<Messages<units::CombatNumber>>();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        host.insert_resource(network(listener));
        let mut clients = [
            Socket::new(TcpStream::connect(address).unwrap()).unwrap(),
            Socket::new(TcpStream::connect(address).unwrap()).unwrap(),
        ];
        join(&mut clients[0], 0);
        join(&mut clients[1], 1);
        let mut ids = [None, None];
        let deadline = Instant::now() + Duration::from_secs(3);
        while ids.iter().any(Option::is_none) {
            tick(&mut host);
            for i in 0..2 {
                for message in clients[i].receive::<wire::ToClient>().unwrap() {
                    if let wire::ToClient::Welcome { id, .. } = message {
                        ids[i] = Some(id);
                    }
                }
            }
            assert!(Instant::now() < deadline);
        }
        let anchor = host
            .query_filtered::<Entity, With<hero::Hero>>()
            .single(&host)
            .unwrap();
        for (owner, amount) in [(None, 111.0), (ids[0], 222.0), (ids[1], 333.0)] {
            host.resource_mut::<Messages<units::CombatNumber>>()
                .write(units::CombatNumber {
                    owner,
                    anchor,
                    amount,
                    crit: false,
                    color: units::COLOR_SPELL,
                });
        }
        let mut seen = [vec![], vec![]];
        let deadline = Instant::now() + Duration::from_secs(3);
        while seen.iter().any(Vec::is_empty) {
            tick(&mut host);
            for i in 0..2 {
                for message in clients[i].receive::<wire::ToClient>().unwrap() {
                    if let wire::ToClient::Snapshot(snapshot) = message {
                        for effect in snapshot.effects {
                            if let wire::Effect::Number { owner, amount, .. } = effect {
                                assert_eq!(owner, ids[i]);
                                seen[i].push(amount);
                            }
                        }
                    }
                }
            }
            assert!(Instant::now() < deadline);
        }
        assert_eq!(seen, [vec![222.0], vec![333.0]]);
    }
    #[test]
    fn two_tcp_guests_exchange_an_item_through_the_shared_chest_once() {
        fn save(inventory: &game::items::Inventory) -> wire::ClientSave {
            wire::ClientSave {
                talent_xp: 0,
                talents: [[0; 19]; 3],
                inventory: inventory.shared_save().unwrap(),
                look: None,
            }
        }
        fn receive(
            clients: &mut [Socket; 2],
            inventories: &mut [game::items::Inventory; 2],
            views: &mut [game::shared_chest::View; 2],
        ) {
            for i in 0..2 {
                for message in clients[i].receive::<wire::ToClient>().unwrap() {
                    if let wire::ToClient::Snapshot(snapshot) = message {
                        game::shared_chest::receive(
                            snapshot.chest,
                            &mut inventories[i],
                            &mut views[i],
                        );
                    }
                }
                for request in std::mem::take(&mut views[i].requests) {
                    clients[i].send(&wire::ToServer::Chest(request)).unwrap();
                }
            }
        }
        let mut host = world();
        host.resource_mut::<Selection>().in_hub = true;
        let local = host
            .query_filtered::<Entity, With<hero::Hero>>()
            .single(&host)
            .unwrap();
        host.get_mut::<Transform>(local).unwrap().translation = game::shared_chest::position();
        host.init_resource::<game::shared_chest::Store>();
        host.init_resource::<game::shared_chest::View>();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        host.insert_resource(network(listener));
        let mut clients = [
            Socket::new(TcpStream::connect(address).unwrap()).unwrap(),
            Socket::new(TcpStream::connect(address).unwrap()).unwrap(),
        ];
        let mut inventories = [
            game::items::Inventory::fixture(vec![game::shared_chest::fixture_item()]),
            game::items::Inventory::fixture(vec![]),
        ];
        let mut views = [
            game::shared_chest::View::default(),
            game::shared_chest::View::default(),
        ];
        for i in 0..2 {
            inventories[i].ensure_trade_identity().unwrap();
            clients[i]
                .send(&wire::ToServer::Join {
                    version: wire::VERSION,
                    profile: wire::Profile {
                        name: format!("Trader {i}"),
                        class: 0,
                        display: data::CLASSES[0].looks[0],
                        race: 1,
                        sex: 0,
                        save: Some(save(&inventories[i])),
                    },
                })
                .unwrap();
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        while views.iter().any(|v| v.replica.identity.is_empty()) {
            tick(&mut host);
            receive(&mut clients, &mut inventories, &mut views);
            assert!(
                Instant::now() < deadline,
                "both traders should receive the chest"
            );
        }
        let traders: Vec<_> = host
            .resource::<Companions>()
            .0
            .values()
            .map(|p| p.entity)
            .collect();
        for trader in traders {
            host.get_mut::<Transform>(trader).unwrap().translation = game::shared_chest::position();
        }
        let vault = views[0].replica.identity.clone();
        inventories[0].begin_deposit(1, &vault).unwrap();
        let token = inventories[0].pending_deposits()[0].token.clone();
        clients[0]
            .send(&wire::ToServer::Save(save(&inventories[0])))
            .unwrap();
        clients[0]
            .send(&wire::ToServer::Chest(
                game::shared_chest::Request::Deposit(token.clone()),
            ))
            .unwrap();
        clients[0]
            .send(&wire::ToServer::Chest(
                game::shared_chest::Request::Deposit(token),
            ))
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while views[1].replica.entries.is_empty() || !inventories[0].pending_deposits().is_empty() {
            tick(&mut host);
            receive(&mut clients, &mut inventories, &mut views);
            assert!(
                Instant::now() < deadline,
                "the deposited item should be shared once"
            );
        }
        assert_eq!(views[1].replica.entries.len(), 1);
        let entry = views[1].replica.entries[0].id;
        let request = wire::ToServer::Chest(game::shared_chest::Request::Withdraw { vault, entry });
        clients[1].send(&request).unwrap();
        clients[1].send(&request).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            tick(&mut host);
            receive(&mut clients, &mut inventories, &mut views);
            if inventories[1].all().len() == 1
                && host
                    .resource::<game::shared_chest::Store>()
                    .replica(&inventories[1])
                    .deliveries
                    .is_empty()
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "the withdrawal should be delivered and acknowledged"
            );
        }
        assert!(inventories[0].all().is_empty());
        assert_eq!(inventories[1].all().len(), 1);
        assert!(host
            .resource::<game::shared_chest::Store>()
            .replica(&inventories[1])
            .entries
            .is_empty());
    }
    #[test]
    fn two_tcp_guests_move_independently_and_receive_authoritative_avatars() {
        let mut host = world();
        host.insert_resource(game::boss_spells::Warnings(vec![
            game::boss_spells::Warning {
                spell: game::boss_spells::Spell::Rings,
                origin: [10.0, 2.0, 20.0],
                direction: [1.0, 0.0, 0.0],
                mortars: vec![],
                elapsed: 1.2,
                circle: None,
            },
        ]));
        host.run_system_once(|mut commands: Commands, mut run: ResMut<Run>| {
            let serial = run.guid();
            game::enemies::spawn_gem(
                &mut commands,
                serial,
                Vec3::X * 10_000.0,
                game::enemies::GemKind::Xp(1),
            );
        })
        .unwrap();
        let healer = host.run_system_once(|mut commands: Commands, mut run: ResMut<Run>| {
            let def = &data::roster(run.expedition_map).enemies[0];
            game::enemies::spawn_enemy(&mut commands, &mut run, def, Vec3::X * 50.0, Vec3::ZERO, false)
        }).unwrap();
        host.entity_mut(healer).insert(game::enemies::Rank::Healer);
        let healer_guid = host.get::<Guid>(healer).unwrap().0;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        host.insert_resource(network(listener));
        let mut clients = [
            Socket::new(TcpStream::connect(address).unwrap()).unwrap(),
            Socket::new(TcpStream::connect(address).unwrap()).unwrap(),
        ];
        join(&mut clients[0], 0);
        join(&mut clients[1], 1);
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut ids = [None, None];
        while ids.iter().any(Option::is_none) {
            tick(&mut host);
            for (index, client) in clients.iter_mut().enumerate() {
                for message in client.receive::<wire::ToClient>().unwrap() {
                    if let wire::ToClient::Welcome { id, .. } = message {
                        ids[index] = Some(id);
                    }
                }
            }
            assert!(Instant::now() < deadline, "both clients should join");
        }
        let ids = [ids[0].unwrap(), ids[1].unwrap()];
        assert_ne!(ids[0], ids[1]);
        let entities = [
            host.resource::<Companions>().0[&ids[0]].entity,
            host.resource::<Companions>().0[&ids[1]].entity,
        ];
        let before = entities.map(|e| host.get::<Transform>(e).unwrap().translation);
        let local = host
            .query_filtered::<Entity, With<hero::Hero>>()
            .single(&host)
            .unwrap();
        let local_before = host.get::<Transform>(local).unwrap().translation;
        for sequence in 1..=4 {
            host.insert_resource(State::new(match sequence {
                1 => Phase::Playing,
                2 => Phase::Paused,
                3 => Phase::CharacterSheet,
                _ => Phase::LevelUp,
            }));
            for (index, client) in clients.iter_mut().enumerate() {
                client
                    .send(&wire::ToServer::Input(wire::Input {
                        sequence,
                        axes: if index == 0 { [0.0, 1.0] } else { [1.0, 0.0] },
                        active: true,
                        ..default()
                    }))
                    .unwrap();
            }
            tick(&mut host);
        }
        let after = entities.map(|e| host.get::<Transform>(e).unwrap().translation);
        assert!(after[0].z < before[0].z, "first client moves forwards");
        assert!(after[1].x > before[1].x, "second client strafes");
        assert_eq!(
            host.get::<Transform>(local).unwrap().translation,
            local_before
        );
        assert_eq!(
            host.query_filtered::<Entity, With<hero::Hero>>()
                .iter(&host)
                .count(),
            1,
            "remote simulation restores local ownership"
        );
        assert_eq!(host.resource::<Run>().owner, None);
        let mut snapshot = None;
        let deadline = Instant::now() + Duration::from_secs(3);
        while snapshot.is_none() {
            tick(&mut host);
            for message in clients[0].receive::<wire::ToClient>().unwrap() {
                if let wire::ToClient::Snapshot(value) = message {
                    snapshot = Some(value);
                }
            }
            assert!(Instant::now() < deadline);
        }
        let snapshot = snapshot.unwrap();
        assert_eq!(snapshot.boss_warnings.len(), 1);
        assert_eq!(snapshot.boss_warnings[0].progress(), 0.5);
        assert_eq!(snapshot.phase, 2, "the host's menu never pauses guests");
        assert!(ids
            .iter()
            .all(|id| snapshot.units.iter().any(|u| u.owner == Some(*id))));
        let mut guest_world = world();
        let mut guest = Guest {
            socket: clients.into_iter().next().unwrap(),
            id: Some(ids[0]),
            joined: true,
            sequence: 0,
            tick: 0,
            offer: 0,
            entities: default(),
            since_input: 0.0,
            pending_jump: false,
            pending_mobility: false,
            save: None,
            save_key: None,
            receipts: default(),
        };
        apply_snapshot(&mut guest_world, &mut guest, snapshot);
        let guest_healer = guest.entities[&healer_guid];
        assert_eq!(guest_world.get::<game::enemies::Rank>(guest_healer), Some(&game::enemies::Rank::Healer));

        let warnings = guest_world.resource::<game::boss_spells::Warnings>();
        assert_eq!(warnings.0[0].origin, [10.0, 2.0, 20.0]);
        assert_eq!(warnings.0[0].progress(), 0.5);
        let local_guest = guest_world
            .query_filtered::<Entity, With<hero::Hero>>()
            .single(&guest_world)
            .unwrap();
        assert!(guest_world.get::<Interpolation>(local_guest).is_none());
        let guest_before = guest_world
            .get::<Transform>(local_guest)
            .unwrap()
            .translation;
        let mut guest_network = network(TcpListener::bind("127.0.0.1:0").unwrap());
        guest_network.listener = None;
        guest_network.guest = Some(guest);
        guest_world.insert_resource(guest_network);
        guest_world.insert_resource(Role::Guest);
        let command = session::Command {
            active: true,
            axes: Vec2::Y,
            jump: true,
            ..default()
        };
        guest_world.resource_scope(|world, session: Mut<session::LocalSession>| {
            session
                .submit(&mut world.resource_mut::<session::Sessions>(), command)
                .unwrap();
        });
        guest_world.run_system_once(session::receive).unwrap();
        guest_world
            .resource_mut::<Time<Real>>()
            .advance_by(Duration::from_millis(16));
        guest_world.run_system_once(prediction::predict).unwrap();
        let predicted = guest_world
            .get::<Transform>(local_guest)
            .unwrap()
            .translation;
        assert!(
            predicted.z < guest_before.z,
            "input moves immediately without another snapshot"
        );
        assert!(predicted.y > guest_before.y, "jump also starts locally");
        assert_eq!(
            guest_world.resource::<crate::player::Player>().pos,
            predicted
        );
        assert_eq!(
            guest_world
                .query_filtered::<Entity, With<hero::Hero>>()
                .iter(&guest_world)
                .count(),
            1
        );
        assert_eq!(
            guest_world
                .query_filtered::<Entity, With<Replicated>>()
                .iter(&guest_world)
                .count(),
            5
        );
        assert_eq!(
            guest_world
                .resource::<game::combat_ui::PartyDamage>()
                .0
                .len(),
            3
        );
        assert_eq!(
            guest_world
                .query_filtered::<Entity, With<game::unit_frames::PartyPlayer>>()
                .iter(&guest_world)
                .count(),
            3
        );
        assert_eq!(
            guest_world
                .query_filtered::<Entity, With<game::enemies::StillPickup>>()
                .iter(&guest_world)
                .count(),
            1,
            "replicated pickups retain their animation-free presentation"
        );
        // Disconnection reaps the controlled character and invalidates its input.
        let owned_effect = host.spawn(super::super::OwnedEffect(ids[0])).id();
        let key = *host.resource::<Network>().peers.keys().next().unwrap();
        let mut network = host.remove_resource::<Network>().unwrap();
        disconnect(&mut host, &mut network, key);
        assert_eq!(host.resource::<Companions>().0.len(), 1);
        assert!(host.get_entity(entities[0]).is_err());
        assert!(host.get_entity(owned_effect).is_err());
    }

    #[test]
    fn a_stale_card_offer_cannot_apply_a_second_reward() {
        let mut world = world();
        let profile = wire::Profile {
            name: String::from("Test Player"),
            class: 0,
            display: data::CLASSES[0].looks[0],
            race: 1,
            sex: 0,
            save: None,
        };
        let mut player = spawn_companion(&mut world, 42, &profile);
        player.run.pending_levels = 1;
        player.offer.choices = vec![hud::Choice::Upgrade(0)];
        let original = player.run.abilities[0].level;
        let offer = player.offer_id;
        world.resource_mut::<Companions>().0.insert(42, player);
        apply_pick(&mut world, 42, offer, 0);
        apply_pick(&mut world, 42, offer, 0);
        assert_eq!(
            world.resource::<Companions>().0[&42].run.abilities[0].level,
            original + 1
        );
        assert_eq!(world.resource::<Run>().abilities.len(), 0);
    }

    #[test]
    fn horde_spawns_around_host_and_guests_and_skips_dead_players() {
        for elapsed in [1.0, data::BOSS_EVERY_SECS / 2.0] {
            let mut world = world();
            world.init_resource::<game::enemies::Spawner>();
            world.resource_mut::<Run>().elapsed = elapsed;
            let profile = wire::Profile {
                name: "Guest".into(),
                class: 0,
                display: data::CLASSES[0].looks[0],
                race: 1,
                sex: 0,
                save: None,
            };
            let player = spawn_companion(&mut world, 42, &profile);
            let entity = player.entity;
            let host_entity = world
                .query_filtered::<Entity, With<hero::Hero>>()
                .single(&world)
                .unwrap();
            let host_pos = world.get::<Transform>(host_entity).unwrap().translation;
            let guest_pos = host_pos + Vec3::X * 1000.0;
            world.get_mut::<Transform>(entity).unwrap().translation = guest_pos;
            world.resource_mut::<Companions>().0.insert(42, player);
            world.run_system_once(game::enemies::spawn_enemies).unwrap();
            let positions: Vec<_> = world
                .query_filtered::<(Entity, &Transform), With<game::enemies::Enemy>>()
                .iter(&world)
                .map(|(e, tf)| (e, tf.translation))
                .collect();
            let near = |p: Vec3, center: Vec3| units::flat(p - center).length() < 50.0;
            assert!(positions.iter().any(|(_, p)| near(*p, host_pos)));
            assert!(positions.iter().any(|(_, p)| near(*p, guest_pos)));
            let maximum = world.resource::<Run>().expeditions.max_monsters;
            assert!(positions.len() <= maximum);
            for (e, _) in positions {
                world.despawn(e);
            }
            world.resource_mut::<Run>().hp = 0.0;
            world.resource_mut::<Run>().dead_t = Some(1.0);
            world
                .resource_mut::<Time>()
                .advance_by(Duration::from_secs(1));
            world.run_system_once(game::enemies::spawn_enemies).unwrap();
            let spawned: Vec<_> = world
                .query_filtered::<(Entity, &Transform), With<game::enemies::Enemy>>()
                .iter(&world)
                .map(|(e, tf)| (e, tf.translation))
                .collect();
            assert!(!spawned.is_empty());
            assert!(spawned.iter().all(|(_, p)| near(*p, guest_pos)));
            for (e, _) in spawned {
                world.despawn(e);
            }
            world
                .resource_mut::<Companions>()
                .0
                .get_mut(&42)
                .unwrap()
                .run
                .hp = 0.0;
            world.run_system_once(game::enemies::spawn_enemies).unwrap();
            assert_eq!(
                world.query::<&game::enemies::Enemy>().iter(&world).count(),
                0
            );
        }
    }

    #[test]
    fn enemies_damage_the_nearest_guest_and_drains_heal_the_correct_player() {
        let mut world = world();
        world.init_resource::<game::enemies::Spawner>();
        let profile = wire::Profile {
            name: String::from("Test Player"),
            class: 0,
            display: data::CLASSES[0].looks[0],
            race: 1,
            sex: 0,
            save: None,
        };
        let player = spawn_companion(&mut world, 42, &profile);
        let position = world.get::<Transform>(player.entity).unwrap().translation;
        let remote_hp = player.run.hp;
        let host_hp = world.resource::<Run>().hp;
        world.resource_mut::<Companions>().0.insert(42, player);
        world
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(50));
        world.run_system_once(game::enemies::spawn_enemies).unwrap();
        let enemy = world
            .query_filtered::<Entity, With<game::enemies::Enemy>>()
            .iter(&world)
            .next()
            .unwrap();
        world.get_mut::<Transform>(enemy).unwrap().translation = position + Vec3::X * 0.1;
        assert_eq!(world.resource::<Run>().expedition_players, 2);
        let config = &world.resource::<Run>().expeditions;
        let expected = config
            .stats(world.resource::<Run>().expedition_map, false)
            .0
            * config.factor(world.resource::<Run>().elapsed)
            * config.party_health(2);
        assert_eq!(
            world.get::<game::enemies::Enemy>(enemy).unwrap().max_hp,
            expected
        );
        world
            .get_mut::<game::enemies::Enemy>(enemy)
            .unwrap()
            .attack_cd = 0.0;
        world.run_system_once(game::enemies::move_enemies).unwrap();
        let wounded = world.resource::<Companions>().0[&42].run.hp;
        assert!(wounded < remote_hp);
        assert_eq!(world.resource::<Run>().hp, host_hp);
        world.write_message(abilities::Hit {
            owner: Some(42),
            effect: data::Effect::Drain(1.0),
            ..abilities::Hit::direct("fireball", enemy, 4.0, false)
        });
        world.run_system_once(abilities::apply_hits).unwrap();
        assert!(world.resource::<Companions>().0[&42].run.hp > wounded);
        assert_eq!(world.resource::<Run>().hp, host_hp);
        assert!(world.resource::<Companions>().0[&42].run.damage_done > 0.0);
        assert_eq!(world.resource::<Run>().damage_done, 0.0);
    }

    #[test]
    fn fallen_guests_wait_for_revival_instead_of_entering_personal_game_over() {
        let companion = Companion {
            entity: Entity::from_raw_u32(42).unwrap(),
            run: Run {
                hp: 0.0,
                dead_t: Some(10.0),
                pending_levels: 1,
                ..default()
            },
            physics: hero::Hero { vy: 0.0, air: 0.0 },
            offer: default(),
            offer_id: 0,
        };
        assert_eq!(encode_phase(Phase::Playing, &companion), 2);
        assert_eq!(encode_phase(Phase::Paused, &companion), 2);
        assert_eq!(encode_phase(Phase::GameOver, &companion), 6);
    }
    #[test]
    fn a_living_guest_keeps_the_run_alive_after_the_host_falls() {
        let mut world = world();
        let profile = wire::Profile {
            name: String::from("Test Player"),
            class: 0,
            display: data::CLASSES[0].looks[0],
            race: 1,
            sex: 0,
            save: None,
        };
        let player = spawn_companion(&mut world, 42, &profile);
        world.resource_mut::<Companions>().0.insert(42, player);
        world.resource_mut::<Run>().dead_t = Some(0.0);
        world
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs(4));
        world.run_system_once(hero::hero_vitals).unwrap();
        assert!(matches!(
            world.resource::<NextState<Phase>>(),
            NextState::Unchanged
        ));
        assert_eq!(world.resource::<Run>().elapsed, 4.0);
        world
            .resource_mut::<Companions>()
            .0
            .get_mut(&42)
            .unwrap()
            .run
            .dead_t = Some(0.0);
        world.run_system_once(hero::hero_vitals).unwrap();
        assert!(matches!(
            world.resource::<NextState<Phase>>(),
            NextState::Pending(Phase::GameOver)
        ));
    }

    #[test]
    fn replicated_combat_events_use_local_entity_ids_for_animation() {
        let mut world = world();
        let caster = world.spawn_empty().id();
        let target = world.spawn_empty().id();
        let entities = BTreeMap::from([(100, caster), (200, target)]);
        play_effect(
            &mut world,
            &entities,
            wire::Effect::Cast {
                caster: 100,
                spell: 133,
                targets: vec![200],
                dest: None,
            },
        );
        play_effect(
            &mut world,
            &entities,
            wire::Effect::Impact {
                target: 200,
                spell: 133,
            },
        );
        play_effect(
            &mut world,
            &entities,
            wire::Effect::Swing {
                attacker: 100,
                victim: Some(200),
                damage: 4,
            },
        );
        let go = world
            .resource::<Messages<SpellGoTargets>>()
            .iter_current_update_messages()
            .next()
            .unwrap();
        assert_eq!(go.caster, caster);
        assert_eq!(go.hits, [target]);
        assert_eq!(world.resource::<Messages<CastEvent>>().len(), 2);
        let swing = world
            .resource::<Messages<SwingMessage>>()
            .iter_current_update_messages()
            .next()
            .unwrap();
        assert_eq!(swing.attacker, caster);
        assert_eq!(swing.victim, Some(target));
    }

    #[test]
    fn an_incompatible_version_is_rejected_before_a_character_is_admitted() {
        let mut world = world();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let network = network(listener);

        world.insert_resource(network);
        let mut client = Socket::new(TcpStream::connect(address).unwrap()).unwrap();
        client
            .send(&wire::ToServer::Join {
                version: wire::VERSION + 1,
                profile: wire::Profile {
                    name: String::from("Test Player"),
                    class: 0,
                    display: data::CLASSES[0].looks[0],
                    race: 1,
                    sex: 0,
                    save: None,
                },
            })
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            pump(&mut world);
            let messages = client.receive::<wire::ToClient>().unwrap();
            if messages
                .iter()
                .any(|m| matches!(m, wire::ToClient::Error(_)))
            {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(world.resource::<Network>().peers.is_empty());
        assert!(world.resource::<Companions>().0.is_empty());
    }

    fn live_ring(
        name: &str,
        life: f32,
        armour: f32,
        damage: f32,
        recovery: f32,
    ) -> game::items::Item {
        use game::items::{Affix, AffixKind, Effect, EquipmentType, Item, Rarity, SlotKind};
        Item {
            affix_rules: 0,
            id: 0,
            base: "live-gear-test".into(),
            name: name.into(),
            slot: SlotKind::Ring,
            icon: "Interface\\Icons\\INV_Misc_QuestionMark".into(),
            rarity: Rarity::Epic,
            level: 1,
            legendary: None,
            base_damage: None,
            base_armour: None,
            base_block: 0.0,
            appearance: None,
            loot_class: None,
            equipment_type: EquipmentType::Legacy,
            affixes: [
                (Effect::Life, life),
                (Effect::Armour, armour),
                (Effect::Damage, damage),
                (Effect::CooldownRecovery, recovery),
            ]
            .into_iter()
            .enumerate()
            .map(|(i, (effect, value))| Affix {
                id: format!("live-{i}"),
                name: "Live equipment".into(),
                group: format!("live-{i}"),
                kind: if i % 2 == 0 {
                    AffixKind::Prefix
                } else {
                    AffixKind::Suffix
                },
                effect,
                value,
                tags: vec![],
                proc: None,
            })
            .collect(),
        }
    }

    fn playing_guest() -> World {
        let mut world = world();
        world.insert_resource(State::new(Phase::Menu));
        world.init_resource::<game::characters::Roster>();
        world.init_resource::<game::talents::Account>();
        world.init_resource::<ButtonInput<KeyCode>>();
        world.init_resource::<Messages<bevy::input::keyboard::KeyboardInput>>();
        let create = world
            .spawn((Interaction::Pressed, game::characters::Click::Create))
            .id();
        world.run_system_once(game::characters::input).unwrap();
        world.despawn(create);
        world.resource_mut::<game::characters::Roster>().name = "Guest".into();
        {
            let mut selection = world.resource_mut::<Selection>();
            selection.class = Some(0);
            selection.looks = vec![(data::CLASSES[0].looks[0], 1, 0)];
            selection.look = Some(0);
        }
        let enter = world
            .spawn((Interaction::Pressed, game::characters::Click::Enter))
            .id();
        world.run_system_once(game::characters::input).unwrap();
        world.despawn(enter);
        assert_eq!(world.resource::<game::characters::Roster>().active, Some(1));
        world.insert_resource(State::new(Phase::Playing));
        world.insert_resource(Role::Guest);
        world.insert_resource(game::items::Inventory::fixture(vec![
            live_ring("First ring", 80.0, 25.0, 0.5, 0.25),
            live_ring("Replacement ring", 20.0, 5.0, 0.1, 0.1),
        ]));
        world
    }

    fn deliver_guest_update(host: &mut World, client: &mut World, network: &mut Network) {
        // The real guest pump detects the inventory revision and sends Save before Input.
        client
            .resource_mut::<Time<Real>>()
            .advance_by(Duration::from_millis(50));
        pump_guest(client, network).unwrap();
        let guest = network.guest.as_ref().unwrap();
        let (id, sequence) = (guest.id.unwrap(), guest.sequence);
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            pump(host);
            if host.resource::<Network>().peers.values().any(|p| {
                p.session.as_ref().is_some_and(|s| s.wire_id() == id) && p.sequence >= sequence
            }) {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "server should receive the equipment update"
            );
            std::thread::yield_now();
        }
    }

    #[test]
    fn guest_equips_swaps_and_unequips_during_a_run_update_only_its_server_stats() {
        let mut host = world();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        host.insert_resource(network(listener));
        let socket = Socket::new(TcpStream::connect(address).unwrap()).unwrap();
        let mut spectator = Socket::new(TcpStream::connect(address).unwrap()).unwrap();
        join(&mut spectator, 1);
        let mut client = playing_guest();
        let mut connection = Network {
            connecting: None,
            listener: None,
            peers: default(),
            next_peer: 0,
            tick: 0,
            since_snapshot: 0.0,
            effects: default(),
            coins: default(),
            casts: default(),
            gos: default(),
            swings: default(),
            numbers: default(),
            loot: default(),
            guest: Some(Guest {
                socket,
                id: None,
                joined: false,
                sequence: 0,
                tick: 0,
                offer: 0,
                entities: default(),
                since_input: 0.0,
                pending_jump: false,
                pending_mobility: false,
                save: None,
                save_key: None,
                receipts: default(),
            }),
        };
        let deadline = Instant::now() + Duration::from_secs(3);
        while connection.guest.as_ref().unwrap().id.is_none()
            || host.resource::<Companions>().0.len() != 2
        {
            client
                .resource_mut::<Time<Real>>()
                .advance_by(Duration::from_millis(50));
            pump_guest(&mut client, &mut connection).unwrap();
            tick(&mut host);
            let _ = spectator.receive::<wire::ToClient>().unwrap();
            assert!(Instant::now() < deadline);
        }
        let id = connection.guest.as_ref().unwrap().id.unwrap();
        let other_id = *host
            .resource::<Companions>()
            .0
            .keys()
            .find(|&&other| other != id)
            .unwrap();
        let (entity, base_hp, base_damage, base_cooldown) = {
            let mut players = host.resource_mut::<Companions>();
            let player = players.0.get_mut(&id).unwrap();
            player.run.elapsed = 25.0;
            player.run.kills = 7;
            player.run.hp = 100.0;
            player.run.abilities[0].cd = 3.25;
            let spell = player.run.resolve_spell(player.run.abilities[0].def, 1);
            (
                player.entity,
                player.run.stats().max_hp,
                spell.dmg,
                spell.cooldown,
            )
        };
        let other_hp = host.resource::<Companions>().0[&other_id]
            .run
            .stats()
            .max_hp;
        let host_hp = host.resource::<Run>().stats().max_hp;
        client
            .resource_mut::<game::items::Inventory>()
            .equip(1, "Ring 1")
            .unwrap();
        deliver_guest_update(&mut host, &mut client, &mut connection);
        {
            let players = host.resource::<Companions>();
            let run = &players.0[&id].run;
            let spell = run.resolve_spell(run.abilities[0].def, 1);
            assert_eq!(run.stats().max_hp, base_hp + 80.0);
            assert_eq!(run.equipment_armour, 25.0);
            assert!((spell.dmg - base_damage * 1.5).abs() < 0.001);
            assert!((spell.cooldown - base_cooldown / 1.25).abs() < 0.001);
            assert_eq!(
                run.hp, 100.0,
                "changing gear must not heal a running character"
            );
        }
        host.resource_mut::<Companions>()
            .0
            .get_mut(&id)
            .unwrap()
            .run
            .hp = base_hp + 70.0;
        client
            .resource_mut::<game::items::Inventory>()
            .equip(2, "Ring 1")
            .unwrap();
        deliver_guest_update(&mut host, &mut client, &mut connection);
        let replacement_version = {
            let players = host.resource::<Companions>();
            let run = &players.0[&id].run;
            assert_eq!(run.stats().max_hp, base_hp + 20.0);
            assert_eq!(run.equipment_armour, 5.0);
            assert_eq!(run.hp, base_hp + 20.0);
            assert!(
                (run.resolve_spell(run.abilities[0].def, 1).dmg - base_damage * 1.1).abs() < 0.001
            );
            run.version
        };
        // A replay cannot stack the old bonuses, and a repeated equip is a no-op on the host.
        let duplicate = connection.guest.as_ref().unwrap().save.clone().unwrap();
        connection
            .guest
            .as_mut()
            .unwrap()
            .socket
            .send(&wire::ToServer::Save(duplicate))
            .unwrap();
        client
            .resource_mut::<game::items::Inventory>()
            .equip(2, "Ring 1")
            .unwrap();
        deliver_guest_update(&mut host, &mut client, &mut connection);
        assert_eq!(
            host.resource::<Companions>().0[&id].run.version,
            replacement_version
        );
        client
            .resource_mut::<game::items::Inventory>()
            .unequip("Ring 1")
            .unwrap();
        deliver_guest_update(&mut host, &mut client, &mut connection);
        let players = host.resource::<Companions>();
        let run = &players.0[&id].run;
        assert_eq!(run.stats().max_hp, base_hp);
        assert_eq!(run.equipment_armour, 0.0);
        assert_eq!(run.hp, base_hp);
        assert_eq!(run.elapsed, 25.0);
        assert_eq!(run.kills, 7);
        assert_eq!(run.abilities[0].cd, 3.25);
        let spell = run.resolve_spell(run.abilities[0].def, 1);
        assert!((spell.dmg - base_damage).abs() < 0.001);
        assert!((spell.cooldown - base_cooldown).abs() < 0.001);
        assert_eq!(players.0[&other_id].run.stats().max_hp, other_hp);
        assert_eq!(players.0[&other_id].run.equipment_armour, 0.0);
        assert_eq!(host.resource::<Run>().stats().max_hp, host_hp);
        // The replicated fields are current before another simulation tick or while paused.
        let fields: BTreeMap<_, _> = host
            .get::<ObjectStore>(entity)
            .unwrap()
            .0
            .raw_fields()
            .collect();
        assert_eq!(fields[&22], base_hp as u32);
        assert_eq!(fields[&28], base_hp as u32);
    }
}
