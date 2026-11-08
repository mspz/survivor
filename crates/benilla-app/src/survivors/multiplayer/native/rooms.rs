//! Runtime lobby lifecycle; a failed or cancelled join never requires restarting the client.
use super::*;
use crate::survivors::multiplayer::lobby::{Lobby, Request};

pub(super) fn close(world: &mut World, network: &mut Network) {
    if *world.resource::<Role>() == Role::Guest {
        let credited = game::talents::earned(world.resource::<Run>());
        world.resource_mut::<Run>().account_credited = credited;
    }
    network.connecting = None;
    network.listener = None;
    let keys: Vec<u64> = network.peers.keys().copied().collect();
    for key in keys {
        disconnect(world, network, key);
    }
    if let Some(guest) = network.guest.take() {
        for entity in guest.entities.values() {
            let _ = world.despawn(*entity);
        }
    }
    network.effects.clear();
    if let Some(mut chest) = world.get_resource_mut::<game::shared_chest::View>() {
        *chest = game::shared_chest::View::default();
    }
    if let Some(mut party) = world.get_resource_mut::<game::combat_ui::PartyDamage>() {
        party.0.clear();
    }
    if let Some(mut pick) = world.get_resource_mut::<game::cards::PendingPick>() {
        pick.clear();
    }
    if let Some(mut offer) = world.get_resource_mut::<hud::Offer>() {
        offer.choices.clear();
    }
    world.insert_resource(Role::Solo);
    if let Some(mut lobby) = world.get_resource_mut::<Lobby>() {
        lobby.active = false;
        lobby.status.clear();
        lobby.busy = false;
        lobby.members = 0;
        lobby.command_line = false;
        lobby.revision += 1;
    }
}

pub(super) fn update(world: &mut World) {
    let request = world
        .get_resource_mut::<Lobby>()
        .and_then(|mut lobby| lobby.request.take());
    let Some(mut network) = world.remove_resource::<Network>() else {
        return;
    };
    if let Some(request) = request {
        match request {
            Request::Solo => {
                close(world, &mut network);
            }
            Request::Leave => {
                close(world, &mut network);
                let mut lobby = world.resource_mut::<Lobby>();
                lobby.joining = false;
                lobby.error.clear();
                lobby.revision += 1;
                world.resource_mut::<NextState<Phase>>().set(Phase::Menu);
            }
            Request::Cancel => {
                close(world, &mut network);
                let mut lobby = world.resource_mut::<Lobby>();
                lobby.joining = false;
                lobby.error.clear();
                lobby.revision += 1;
            }
            Request::Host => {
                close(world, &mut network);
                match TcpListener::bind((std::net::Ipv4Addr::UNSPECIFIED, address::PORT)).and_then(
                    |listener| {
                        listener.set_nonblocking(true)?;
                        Ok(listener)
                    },
                ) {
                    Ok(listener) => {
                        network.listener = Some(listener);
                        world.insert_resource(Role::Host);
                        let mut lobby = world.resource_mut::<Lobby>();
                        lobby.active = true;
                        lobby.joining = false;
                        lobby.busy = false;
                        lobby.error.clear();
                        lobby.status = "Vos amis doivent saisir votre IPv4 Hamachi.".into();
                        lobby.members = 1;
                        lobby.revision += 1;
                        world.resource_mut::<NextState<Phase>>().set(Phase::Hub);
                    }
                    Err(error) => world
                        .resource_mut::<Lobby>()
                        .fail(format!("Impossible de créer le salon : {error}")),
                }
            }
            Request::Join(text) => {
                close(world, &mut network);
                match address::parse(&text) {
                    Ok(address) => {
                        let (send, receive) = crossbeam_channel::bounded(1);
                        std::thread::spawn(move || {
                            let result = TcpStream::connect_timeout(&address, Duration::from_secs(3)).map_err(|_| "Salon introuvable. Vérifiez que votre ami héberge toujours le salon et que vous êtes sur le même réseau ou VPN.".to_owned());
                            let _ = send.send(result);
                        });
                        network.connecting = Some((address, receive));
                        let mut lobby = world.resource_mut::<Lobby>();
                        lobby.joining = true;
                        lobby.busy = true;
                        lobby.error.clear();
                        lobby.status = "Connexion au salon…".into();
                        lobby.revision += 1;
                    }
                    Err(error) => world.resource_mut::<Lobby>().fail(error),
                }
            }
        }
    }
    let result =
        network
            .connecting
            .as_ref()
            .and_then(|(address, receive)| match receive.try_recv() {
                Ok(result) => Some((*address, result)),
                Err(crossbeam_channel::TryRecvError::Empty) => None,
                Err(_) => Some((*address, Err("La connexion a été interrompue.".into()))),
            });
    if let Some((_address, result)) = result {
        network.connecting = None;
        match result.and_then(|stream| Socket::new(stream).map_err(|e| e.to_string())) {
            Ok(socket) => {
                network.guest = Some(Guest {
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
                });
                world.insert_resource(Role::Guest);
                let mut lobby = world.resource_mut::<Lobby>();
                lobby.active = true;
                lobby.status = "Entrée dans le salon…".into();
                lobby.revision += 1;
            }
            Err(error) => {
                world.resource_mut::<Lobby>().fail(error);
            }
        }
    }
    world.insert_resource(network);
}

#[cfg(test)]
mod tests {
    use super::*;

    static PORT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    fn empty_network() -> Network {
        Network {
            connecting: None,
            listener: None,
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

    fn menu() -> World {
        let mut world = World::new();
        world.insert_resource(Role::Solo);
        world.init_resource::<Run>();
        world.init_resource::<Lobby>();
        world.init_resource::<NextState<Phase>>();
        world.insert_resource(empty_network());
        world
    }

    #[test]
    fn creating_a_room_listens_on_the_fixed_port_and_leaving_closes_it() {
        let _guard = PORT_LOCK.lock().unwrap();
        let mut world = menu();
        world.resource_mut::<Lobby>().request = Some(Request::Host);
        update(&mut world);
        let network = world.resource::<Network>();

        assert_eq!(
            address::PORT,
            network
                .listener
                .as_ref()
                .unwrap()
                .local_addr()
                .unwrap()
                .port()
        );
        assert!(*world.resource::<Role>() == Role::Host);
        assert!(matches!(
            world.resource::<NextState<Phase>>(),
            NextState::Pending(Phase::Hub)
        ));
        world.resource_mut::<Lobby>().request = Some(Request::Leave);
        update(&mut world);
        assert!(world.resource::<Network>().listener.is_none());
        assert!(!world.resource::<Lobby>().active);
        assert!(*world.resource::<Role>() == Role::Solo);
        assert!(matches!(
            world.resource::<NextState<Phase>>(),
            NextState::Pending(Phase::Menu)
        ));
    }

    #[test]
    fn joining_connects_without_entering_the_world_until_the_host_confirms_and_can_be_cancelled() {
        let _guard = PORT_LOCK.lock().unwrap();
        let _listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, address::PORT)).unwrap();

        let mut world = menu();
        world.resource_mut::<Lobby>().request = Some(Request::Join("127.0.0.1".into()));
        update(&mut world);
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while world.resource::<Network>().guest.is_none() {
            update(&mut world);
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(*world.resource::<Role>() == Role::Guest);
        assert!(world.resource::<Lobby>().busy);
        assert!(world.resource::<Lobby>().active);
        assert!(matches!(
            world.resource::<NextState<Phase>>(),
            NextState::Unchanged
        ));
        world.resource_mut::<Lobby>().request = Some(Request::Cancel);
        update(&mut world);
        assert!(world.resource::<Network>().guest.is_none());
        assert!(!world.resource::<Lobby>().busy);
        assert!(*world.resource::<Role>() == Role::Solo);
    }

    #[test]
    fn closing_a_guest_does_not_credit_its_saved_progress_a_second_time() {
        let mut world = World::new();
        world.insert_resource(Role::Guest);
        world.insert_resource(Run {
            meta_kills: 41,
            ..default()
        });
        world.init_resource::<Lobby>();
        let mut network = Network {
            connecting: None,
            listener: None,
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
        };
        world.insert_resource(game::combat_ui::PartyDamage(vec![
            game::combat_ui::PartyRow {
                id: 42,
                name: "Guest".into(),
                class: 1,
                dps: 20.0,
                total: 100.0,
            },
        ]));
        close(&mut world, &mut network);
        assert_eq!(world.resource::<Run>().account_credited, 41);
        assert!(*world.resource::<Role>() == Role::Solo);
        assert!(!world.resource::<Lobby>().active);
        assert!(world
            .resource::<game::combat_ui::PartyDamage>()
            .0
            .is_empty());
    }
}
