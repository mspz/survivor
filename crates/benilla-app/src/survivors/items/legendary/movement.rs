//! Legendary steps use the same terrain and battleground leash as class mobility.
use super::*;
use crate::survivors::{
    hero::{self, Hero},
    multiplayer::{Companions, RemoteOwner},
    units::flat,
    Selection,
};
use crate::survivors::geometry::Surface as WorldPoint;
pub(in crate::survivors) struct Move {
    pub cast: u64,
    pub point: Vec3,
}
pub(in crate::survivors) fn tick(
    mut run: ResMut<Run>,
    mut players: Option<ResMut<Companions>>,
    world: WorldPoint,
    selection: Res<Selection>,
    mut actors: Query<(&mut Transform, Option<&RemoteOwner>, Has<Hero>)>,
) {
    let host = run.owner;
    let mut moves = Vec::new();
    moves.extend(
        std::mem::take(&mut run.legendary.moves)
            .into_iter()
            .map(|m| (host, m)),
    );
    if let Some(players) = players.as_deref_mut() {
        for p in players.0.values_mut() {
            moves.extend(
                std::mem::take(&mut p.run.legendary.moves)
                    .into_iter()
                    .map(|m| (p.run.owner, m)),
            );
        }
    }
    let map = selection.map_def();
    let center = hero::arena(map);
    for (owner, step) in moves {
        if !map.full_instance() && flat(step.point - center).length() > map.leash {
            continue;
        }
        let Some(height) = world.terrain_height_under(step.point) else {
            continue;
        };
        let Some((mut tf, _, _)) = actors.iter_mut().find(|(_, remote, local)| {
            remote.is_some_and(|r| Some(r.0) == owner) || *local && owner == host
        }) else {
            continue;
        };
        let pos = Vec3::new(step.point.x, height, step.point.z);
        let pos = world.constrain(tf.translation, pos);
        tf.translation = pos;
        if let Some(player) =
            crate::survivors::multiplayer::player_run(&mut run, players.as_deref_mut(), owner)
        {
            player.legendary.events.position = pos;
            for plan in &mut player.legendary.casting.plans {
                if plan.unique == 175 && plan.params.cast_id == step.cast {
                    plan.center = pos;
                }
            }
        }
    }
}
