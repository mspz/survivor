//! Predict only the local body. Combat and world positions remain authoritative on the host.
use super::*;
use benilla_world::world_point::WorldPoint;
use std::collections::VecDeque;

pub(super) struct Authority {
    pub transform: Transform,
    pub vertical_speed: f32,
    pub acknowledged: u64,
    pub speed: f32,
    pub map: usize,
    pub in_hub: bool,
    pub dead: bool,
}

struct Frame {
    sequence: u64,
    dt: f32,
    command: session::Command,
}

#[derive(Component)]
pub(super) struct Prediction {
    body: Transform,
    physics: hero::Hero,
    pending: VecDeque<Frame>,
    authority: Option<Authority>,
    offset: Vec3,
    map: usize,
    in_hub: bool,
    speed: f32,
    dead: bool,
}

pub(super) fn receive(world: &mut World, entity: Entity, authority: Authority) {
    // The local body never enters the remote interpolation path.
    world.entity_mut(entity).remove::<Interpolation>();
    if let Some(mut prediction) = world.get_mut::<Prediction>(entity) {
        prediction.authority = Some(authority);
    } else {
        world.entity_mut(entity).insert((
            hero::Hero { vy: 0.0, air: 0.0 },
            Prediction {
                body: authority.transform,
                physics: hero::Hero { vy: 0.0, air: 0.0 },
                pending: default(),
                offset: Vec3::ZERO,
                map: authority.map,
                in_hub: authority.in_hub,
                speed: authority.speed,
                dead: authority.dead,
                authority: Some(authority),
            },
        ));
    }
}

impl Prediction {
    fn reconcile(&mut self, movement: &mut MovementState, ground: impl Fn(Vec3) -> Option<f32>) {
        let Some(authority) = self.authority.take() else {
            return;
        };
        let previous = self.body.translation + self.offset;
        let teleport = authority.map != self.map
            || authority.in_hub != self.in_hub
            || authority.dead != self.dead
            || self
                .body
                .translation
                .distance(authority.transform.translation)
                > 15.0;
        self.map = authority.map;
        self.in_hub = authority.in_hub;
        self.speed = authority.speed;
        self.dead = authority.dead;
        if teleport || self.dead {
            self.pending.clear();
        } else {
            self.pending.retain(|f| f.sequence > authority.acknowledged);
        }
        self.body = authority.transform;
        let height = ground(self.body.translation).unwrap_or(self.body.translation.y);
        self.physics = hero::Hero {
            vy: authority.vertical_speed,
            air: (self.body.translation.y - height).max(0.0),
        };
        let map = if self.in_hub {
            &data::HUB
        } else {
            &data::MAPS[self.map.min(data::MAPS.len() - 1)]
        };
        for frame in &self.pending {
            hero::step_motion(
                &mut self.body,
                movement,
                &mut self.physics,
                frame.command,
                frame.dt,
                self.speed,
                map,
                &ground,
            );
        }
        self.offset = if teleport || self.dead {
            Vec3::ZERO
        } else {
            previous - self.body.translation
        };
    }
}

pub(super) fn predict(
    time: Res<Time<Real>>,
    network: Res<Network>,
    phase: Res<State<Phase>>,
    input: Res<session::ServerInput>,
    session: Res<session::LocalSession>,
    run: Res<Run>,
    selection: Res<Selection>,
    loading: Option<Res<crate::loading_screen::LoadingScreen>>,
    ground: WorldPoint,
    mut player: ResMut<crate::player::Player>,
    mut heroes: Query<(
        &mut Transform,
        &mut MovementState,
        &mut hero::Hero,
        &mut Prediction,
    )>,
) {
    let Some(guest) = network.guest.as_ref() else {
        return;
    };
    let Ok((mut tf, mut movement, mut physics, mut prediction)) = heroes.single_mut() else {
        return;
    };
    prediction.reconcile(&mut movement, |p| ground.terrain_height_under(p));
    let dt = time.delta_secs().min(0.1);
    let command = if run.dead_t.is_none()
        && matches!(*phase.get(), Phase::Hub | Phase::Playing)
        && !loading.as_deref().is_some_and(|l| l.covering())
    {
        input.for_client(session.id())
    } else {
        session::Command::default()
    };
    let speed = prediction.speed;
    let Prediction {
        body,
        physics: body_physics,
        ..
    } = &mut *prediction;
    hero::step_motion(
        body,
        &mut movement,
        body_physics,
        command,
        dt,
        speed,
        selection.stage_def(),
        |p| ground.terrain_height_under(p),
    );
    prediction.pending.push_back(Frame {
        sequence: guest.sequence + 1,
        command,
        dt,
    });
    // A lost connection must not grow an unbounded replay history.
    while prediction.pending.len() > 240 {
        prediction.pending.pop_front();
    }
    prediction.offset *= (-12.0 * dt).exp();
    *tf = prediction.body;
    tf.translation += prediction.offset;
    *physics = prediction.physics;
    player.active = true;
    player.detached = false;
    player.settling = false;
    player.world_stale = false;
    player.pos = tf.translation;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unacknowledged_motion_is_replayed_and_teleports_clear_history() {
        let map = &data::MAPS[0];
        let origin = hero::arena(map);
        let mut prediction = Prediction {
            body: Transform::from_translation(origin),
            physics: hero::Hero { vy: 0.0, air: 0.0 },
            pending: default(),
            authority: None,
            offset: Vec3::ZERO,
            map: 0,
            in_hub: false,
            speed: 7.0,
            dead: false,
        };
        let mut movement = MovementState::default();
        let command = session::Command {
            axes: Vec2::Y,
            active: true,
            ..default()
        };
        prediction.pending.push_back(Frame {
            sequence: 2,
            dt: 0.1,
            command,
        });
        prediction.authority = Some(Authority {
            transform: Transform::from_translation(origin),
            vertical_speed: 0.0,
            acknowledged: 1,
            speed: 7.0,
            map: 0,
            in_hub: false,
            dead: false,
        });
        prediction.reconcile(&mut movement, |_| Some(origin.y));
        assert!((prediction.body.translation.z - (origin.z - 0.7)).abs() < 0.001);
        assert_eq!(prediction.pending.len(), 1);
        let destination = origin + Vec3::X * 20.0;
        prediction.authority = Some(Authority {
            transform: Transform::from_translation(destination),
            vertical_speed: 0.0,
            acknowledged: 2,
            speed: 7.0,
            map: 0,
            in_hub: false,
            dead: false,
        });
        prediction.reconcile(&mut movement, |_| Some(origin.y));
        assert!(prediction.pending.is_empty());
        assert_eq!(prediction.body.translation, destination);
        assert_eq!(prediction.offset, Vec3::ZERO);
    }
}
