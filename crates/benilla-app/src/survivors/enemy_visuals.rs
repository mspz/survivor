//! Per-instance model tints distinguish monster variants without ground markers.
use super::enemies::Rank;
use benilla_world::{
    instance_tint::{self, InstanceTints},
    model_fade::ParentModel,
    rig_palette::RigSkin,
};
use bevy::prelude::*;

fn style(rank: Rank) -> Option<[u8; 3]> {
    Some(match rank {
        Rank::Elite => [255, 200, 55],
        Rank::Sprinter => [55, 235, 255],
        Rank::Explosive => [255, 65, 40],
        Rank::Healer => [70, 255, 100],
        Rank::Protector => [75, 120, 255],
        Rank::Boss => [210, 80, 255],
        Rank::Normal => return None,
    })
}

/// Follow the same model ownership chain as aura tints, including separately rigged attachments.
#[allow(clippy::type_complexity)]
fn paint(
    rigs: Query<(Entity, &RigSkin)>,
    chain: Query<(Option<&Rank>, Option<&ParentModel>, Option<&ChildOf>)>,
    mut tints: Option<ResMut<InstanceTints>>,
) {
    let Some(tints) = tints.as_deref_mut() else {
        return;
    };
    for (entity, rig) in &rigs {
        let mut at = entity;
        for _ in 0..benilla_world::model_fade::MAX_MODEL_CHAIN {
            let Ok((rank, model, parent)) = chain.get(at) else {
                break;
            };
            if let Some(rgb) = rank.and_then(|&rank| style(rank)) {
                tints.set(rig.slot, instance_tint::pack(rgb));
                break;
            }
            let Some(next) = model.map(|p| p.0).or_else(|| parent.map(ChildOf::parent)) else {
                break;
            };
            at = next;
        }
    }
}
pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, paint.after(crate::aura_visual::apply_aura_tint));
}

#[cfg(test)]
mod tests {
    use super::*;
    use benilla_world::rig_palette::RigPalettes;
    use bevy::ecs::system::RunSystemOnce;

    fn rig(world: &mut World) -> RigSkin {
        RigSkin::allocate_bones(
            &mut world.resource_mut::<RigPalettes>(),
            1,
            Handle::default(),
        )
        .unwrap()
    }
    #[test]
    fn model_tints_follow_equipment_ownership_change_with_rank_and_reset_with_aura_channel() {
        let mut world = World::new();
        world.init_resource::<RigPalettes>();
        world.init_resource::<InstanceTints>();
        let body_rig = rig(&mut world);
        let body_slot = body_rig.slot;
        let root = world.spawn((Rank::Healer, body_rig)).id();
        let item_rig = rig(&mut world);
        let item_slot = item_rig.slot;
        world.spawn((item_rig, ParentModel(root)));
        let normal_rig = rig(&mut world);
        let normal_slot = normal_rig.slot;
        world.spawn((Rank::Normal, normal_rig));
        for rank in [Rank::Healer, Rank::Protector] {
            world.entity_mut(root).insert(rank);
            world
                .run_system_once(crate::aura_visual::apply_aura_tint)
                .unwrap();
            world.run_system_once(paint).unwrap();
            let expected = instance_tint::pack(style(rank).unwrap());
            assert_eq!(world.resource::<InstanceTints>().get(body_slot), expected);
            assert_eq!(world.resource::<InstanceTints>().get(item_slot), expected);
            assert_eq!(
                world.resource::<InstanceTints>().get(normal_slot),
                instance_tint::IDENTITY
            );
        }
        world.entity_mut(root).remove::<Rank>();
        world
            .run_system_once(crate::aura_visual::apply_aura_tint)
            .unwrap();
        world.run_system_once(paint).unwrap();
        assert_eq!(
            world.resource::<InstanceTints>().get(body_slot),
            instance_tint::IDENTITY
        );
        assert_eq!(
            world.resource::<InstanceTints>().get(item_slot),
            instance_tint::IDENTITY
        );
    }
}
