//! In-process transport. Commands contain intent; only the server moves the hero.
use bevy::prelude::*;
use std::collections::BTreeMap;

/// Shared scheduling contract; neither side names the other side's systems.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Stage {
    Input,
    Simulate,
    Present,
}

#[derive(Clone, Copy, Default)]
pub(super) struct Command {
    pub direction: Vec3,
    pub facing: Option<Vec3>,
    pub axes: Vec2,
    pub turn: f32,
    pub automatic: bool,
    pub active: bool,
    pub jump: bool,
    pub mobility: bool,
    pub aim: Option<Vec3>,
}

/// Allocated by the server, never reused during its lifetime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct ClientId(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SessionError {
    Disconnected,
}

/// One mailbox per connection. A client cannot replace another client's pending input.
#[derive(Resource, Default)]
pub(super) struct Sessions {
    next_id: u64,
    clients: BTreeMap<ClientId, Option<Command>>,
}

impl Sessions {
    pub fn connect(&mut self) -> LocalSession {
        self.next_id = self
            .next_id
            .checked_add(1)
            .expect("client ID space exhausted");
        let id = ClientId(self.next_id);
        self.clients.insert(id, None);
        LocalSession { id }
    }

    fn submit(&mut self, id: ClientId, command: Command) -> Result<(), SessionError> {
        let pending = self
            .clients
            .get_mut(&id)
            .ok_or(SessionError::Disconnected)?;
        *pending = Some(command);
        Ok(())
    }

    /// Invalidates outstanding handles and removes this client's current input immediately.
    #[allow(dead_code)] // Lifecycle API for additional local clients and the future network adapter.
    pub fn disconnect(&mut self, id: ClientId, input: &mut ServerInput) -> bool {
        input.0.remove(&id);
        self.clients.remove(&id).is_some()
    }

    pub(super) fn receive(&mut self, input: &mut ServerInput) {
        input.0.clear();
        for (&id, pending) in &mut self.clients {
            input.0.insert(id, pending.take().unwrap_or_default());
        }
    }
}

/// Handle for one connection; the solo frontend creates exactly one of these.
#[derive(Resource)]
pub(super) struct LocalSession {
    id: ClientId,
}

impl LocalSession {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn wire_id(&self) -> u64 {
        self.id.0
    }
    pub fn id(&self) -> ClientId {
        self.id
    }

    pub fn submit(&self, server: &mut Sessions, command: Command) -> Result<(), SessionError> {
        server.submit(self.id, command)
    }
}

impl FromWorld for LocalSession {
    fn from_world(world: &mut World) -> Self {
        world.resource_mut::<Sessions>().connect()
    }
}

/// Inputs for every connected client, consumed independently in the same simulation frame.
#[derive(Resource, Default)]
pub(super) struct ServerInput(BTreeMap<ClientId, Command>);

impl ServerInput {
    pub fn for_client(&self, id: ClientId) -> Command {
        self.0.get(&id).copied().unwrap_or_default()
    }
}

pub(super) fn receive(mut sessions: ResMut<Sessions>, mut input: ResMut<ServerInput>) {
    sessions.receive(&mut input);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clients_have_independent_commands() {
        let mut server = Sessions::default();
        let first = server.connect();
        let second = server.connect();
        assert_ne!(first.id(), second.id());
        first
            .submit(
                &mut server,
                Command {
                    jump: true,
                    ..default()
                },
            )
            .unwrap();
        second
            .submit(
                &mut server,
                Command {
                    mobility: true,
                    ..default()
                },
            )
            .unwrap();
        let mut input = ServerInput::default();
        server.receive(&mut input);
        assert_eq!(input.0.len(), 2);
        assert!(input.for_client(first.id()).jump);
        assert!(!input.for_client(first.id()).mobility);
        assert!(!input.for_client(second.id()).jump);
        assert!(input.for_client(second.id()).mobility);
    }

    #[test]
    fn latest_command_only_replaces_its_senders_input() {
        let mut server = Sessions::default();
        let first = server.connect();
        let second = server.connect();
        first
            .submit(
                &mut server,
                Command {
                    jump: true,
                    ..default()
                },
            )
            .unwrap();
        second
            .submit(
                &mut server,
                Command {
                    mobility: true,
                    ..default()
                },
            )
            .unwrap();
        first.submit(&mut server, Command::default()).unwrap();
        let mut input = ServerInput::default();
        server.receive(&mut input);
        assert!(!input.for_client(first.id()).jump);
        assert!(input.for_client(second.id()).mobility);
    }

    #[test]
    fn a_silent_client_does_not_replay_actions_or_block_other_clients() {
        let mut app = App::new();
        app.init_resource::<Sessions>()
            .init_resource::<ServerInput>()
            .add_systems(Update, receive);
        let first = app.world_mut().resource_mut::<Sessions>().connect();
        let second = app.world_mut().resource_mut::<Sessions>().connect();
        first
            .submit(
                &mut app.world_mut().resource_mut::<Sessions>(),
                Command {
                    jump: true,
                    ..default()
                },
            )
            .unwrap();
        app.update();
        assert!(
            app.world()
                .resource::<ServerInput>()
                .for_client(first.id())
                .jump
        );
        second
            .submit(
                &mut app.world_mut().resource_mut::<Sessions>(),
                Command {
                    mobility: true,
                    ..default()
                },
            )
            .unwrap();
        app.update();
        let input = app.world().resource::<ServerInput>();
        assert!(!input.for_client(first.id()).jump);
        assert!(input.for_client(second.id()).mobility);
    }

    #[test]
    fn disconnect_discards_pending_and_current_input_without_affecting_peers() {
        let mut server = Sessions::default();
        let first = server.connect();
        let second = server.connect();
        let mut input = ServerInput::default();
        server.receive(&mut input);
        first
            .submit(
                &mut server,
                Command {
                    jump: true,
                    ..default()
                },
            )
            .unwrap();
        assert!(server.disconnect(first.id(), &mut input));
        assert!(!input.0.contains_key(&first.id()));
        assert_eq!(
            first.submit(&mut server, Command::default()),
            Err(SessionError::Disconnected)
        );
        second
            .submit(
                &mut server,
                Command {
                    mobility: true,
                    ..default()
                },
            )
            .unwrap();
        server.receive(&mut input);
        assert_eq!(input.0.len(), 1);
        assert!(input.for_client(second.id()).mobility);
        assert!(!server.disconnect(first.id(), &mut input));
        let third = server.connect();
        assert_ne!(third.id(), first.id());
        assert_ne!(third.id(), second.id());
    }

    #[test]
    fn solo_frontend_connects_one_client_to_an_initially_empty_server() {
        let mut app = App::new();
        app.init_resource::<Sessions>();
        assert!(app.world().resource::<Sessions>().clients.is_empty());
        app.init_resource::<LocalSession>();
        app.init_resource::<LocalSession>();
        assert_eq!(app.world().resource::<Sessions>().clients.len(), 1);
    }
}
