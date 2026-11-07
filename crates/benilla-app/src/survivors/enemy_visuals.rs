//! Shared, unlit role symbols and the existing GPU instance tint keep variants readable on any roster.
use super::{
    enemies::{Enemy, Rank},
    unit_frames::Status,
    RunEntity,
};
use crate::net::NetEntity;
use benilla_world::{
    instance_tint::{self, InstanceTints},
    model_fade::ParentModel,
    rig_palette::RigSkin,
};
use bevy::{asset::RenderAssetUsages, mesh::PrimitiveTopology, prelude::*};

fn style(rank: Rank) -> Option<(usize, [u8; 3])> {
    Some(match rank {
        Rank::Elite => (0, [255, 200, 55]),
        Rank::Sprinter => (1, [55, 235, 255]),
        Rank::Explosive => (2, [255, 65, 40]),
        Rank::Healer => (3, [70, 255, 100]),
        Rank::Protector => (4, [75, 120, 255]),
        Rank::Boss => (5, [210, 80, 255]),
        Rank::Normal => return None,
    })
}

fn outline(rank: Rank) -> Vec<Vec2> {
    let star = |points: usize, inner: f32| {
        (0..points * 2)
            .map(|i| {
                let angle = i as f32 * std::f32::consts::PI / points as f32;
                Vec2::new(angle.sin(), angle.cos()) * if i % 2 == 0 { 1.2 } else { inner }
            })
            .collect()
    };
    match rank {
        Rank::Elite => star(5, 0.65),
        Rank::Explosive => star(8, 0.85),
        Rank::Boss => star(6, 0.5),
        Rank::Healer => vec![
            Vec2::new(-0.3, -1.1),
            Vec2::new(0.3, -1.1),
            Vec2::new(0.3, -0.3),
            Vec2::new(1.1, -0.3),
            Vec2::new(1.1, 0.3),
            Vec2::new(0.3, 0.3),
            Vec2::new(0.3, 1.1),
            Vec2::new(-0.3, 1.1),
            Vec2::new(-0.3, 0.3),
            Vec2::new(-1.1, 0.3),
            Vec2::new(-1.1, -0.3),
            Vec2::new(-0.3, -0.3),
        ],
        Rank::Protector => vec![
            Vec2::new(-0.9, -0.8),
            Vec2::new(0.9, -0.8),
            Vec2::new(0.9, 0.15),
            Vec2::new(0.0, 1.2),
            Vec2::new(-0.9, 0.15),
        ],
        _ => vec![],
    }
}
fn symbol_mesh(rank: Rank) -> Mesh {
    let mut positions = Vec::new();
    let mut line = |a: Vec2, b: Vec2| {
        let delta = (b - a).normalize_or_zero();
        let side = Vec2::new(-delta.y, delta.x) * 0.095;
        for p in [a - side, a + side, b + side, a - side, b + side, b - side] {
            positions.push([p.x, 0.0, p.y]);
        }
    };
    if rank == Rank::Sprinter {
        for offset in [-0.35, 0.45] {
            let tip = Vec2::new(0.0, offset - 0.65);
            line(Vec2::new(-0.85, offset + 0.15), tip);
            line(tip, Vec2::new(0.85, offset + 0.15));
        }
    } else {
        let points = outline(rank);
        for i in 0..points.len() {
            line(points[i], points[(i + 1) % points.len()]);
        }
    }
    let count = positions.len();
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; count])
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; count])
}

#[derive(Resource)]
struct Symbols(Vec<(Handle<Mesh>, Handle<StandardMaterial>)>);
#[derive(Component)]
struct MarkerLink {
    entity: Entity,
    rank: Rank,
}
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut symbols = Vec::new();
    for rank in [
        Rank::Elite,
        Rank::Sprinter,
        Rank::Explosive,
        Rank::Healer,
        Rank::Protector,
        Rank::Boss,
    ] {
        let (_, rgb) = style(rank).unwrap();
        let color = Color::srgb_u8(rgb[0], rgb[1], rgb[2]);
        symbols.push((
            meshes.add(symbol_mesh(rank)),
            materials.add(StandardMaterial {
                base_color: color,
                unlit: true,
                cull_mode: None,
                ..default()
            }),
        ));
    }
    commands.insert_resource(Symbols(symbols));
}

#[allow(clippy::type_complexity)]
fn sync_markers(
    mut commands: Commands,
    time: Res<Time<Real>>,
    symbols: Res<Symbols>,
    units: Query<
        (
            Entity,
            Option<&Rank>,
            &NetEntity,
            Option<&Enemy>,
            Option<&Status>,
            Option<&MarkerLink>,
        ),
        With<RunEntity>,
    >,
) {
    for (owner, rank, net, enemy, status, link) in &units {
        let Some((rank, index)) =
            rank.and_then(|&rank| style(rank).map(|(index, _)| (rank, index)))
        else {
            if let Some(link) = link {
                commands.entity(link.entity).despawn();
                commands.entity(owner).remove::<MarkerLink>();
            }
            continue;
        };
        let dead = enemy.is_some_and(|e| e.dead) || status.is_some_and(|s| s.health <= 0.0);
        let visibility = if dead {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        let pulse = if rank == Rank::Explosive {
            1.0 + 0.09 * (time.elapsed_secs() * 6.0).sin()
        } else {
            1.0
        };
        let transform = Transform::from_xyz(0.0, 0.12, 0.0)
            .with_scale(Vec3::splat(net.scale.max(0.6) * 1.25 * pulse));
        if let Some(link) = link.filter(|l| l.rank == rank) {
            commands.entity(link.entity).insert((transform, visibility));
        } else {
            if let Some(link) = link {
                commands.entity(link.entity).despawn();
            }
            let (mesh, material) = &symbols.0[index];
            let marker = commands
                .spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material.clone()),
                    transform,
                    visibility,
                    ChildOf(owner),
                ))
                .id();
            commands.entity(owner).insert(MarkerLink {
                entity: marker,
                rank,
            });
        }
    }
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
            if let Some((_, rgb)) = rank.and_then(|&rank| style(rank)) {
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
    app.add_systems(Startup, setup)
        .add_systems(
            Update,
            sync_markers.after(benilla_world::schedule::WorldStage::Input),
        )
        .add_systems(Update, paint.after(crate::aura_visual::apply_aura_tint));
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
            let expected = instance_tint::pack(style(rank).unwrap().1);
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
    #[test]
    fn symbols_share_assets_remain_unique_and_follow_role_changes_and_death() {
        let mut world = World::new();
        world.init_resource::<Assets<Mesh>>();
        world.init_resource::<Assets<StandardMaterial>>();
        world.init_resource::<Time<Real>>();
        world.run_system_once(setup).unwrap();
        let mut colors = std::collections::HashSet::new();
        for rank in [
            Rank::Elite,
            Rank::Sprinter,
            Rank::Explosive,
            Rank::Healer,
            Rank::Protector,
            Rank::Boss,
        ] {
            assert!(colors.insert(style(rank).unwrap().1));
            let owner = world
                .spawn((
                    RunEntity,
                    rank,
                    NetEntity {
                        kind: benilla_protocol::EntityKind::Unit,
                        display_id: None,
                        scale: 1.0,
                    },
                ))
                .id();
            world.run_system_once(sync_markers).unwrap();
            let marker = world.get::<MarkerLink>(owner).unwrap().entity;
            assert_eq!(world.get::<ChildOf>(marker).unwrap().parent(), owner);
            let mesh = world.get::<Mesh3d>(marker).unwrap();
            let vertices = world
                .resource::<Assets<Mesh>>()
                .get(&mesh.0)
                .unwrap()
                .attribute(Mesh::ATTRIBUTE_POSITION)
                .unwrap()
                .as_float3()
                .unwrap();
            assert!(vertices.len() >= 24);
            assert!(vertices.iter().flatten().all(|v| v.is_finite()));
            world.run_system_once(sync_markers).unwrap();
            assert_eq!(world.get::<MarkerLink>(owner).unwrap().entity, marker);
            world.entity_mut(owner).insert(Status {
                name: "Test".into(),
                health: 0.0,
                maximum: 100.0,
                level: 1,
                friendly: false,
                debuffs: vec![],
            });
            world.run_system_once(sync_markers).unwrap();
            assert_eq!(
                *world.get::<Visibility>(marker).unwrap(),
                Visibility::Hidden
            );
            world.entity_mut(owner).insert(Rank::Normal);
            world.run_system_once(sync_markers).unwrap();
            assert!(world.get::<MarkerLink>(owner).is_none());
            assert!(world.get_entity(marker).is_err());
        }
        assert_eq!(world.resource::<Assets<Mesh>>().len(), 6);
        assert_eq!(world.resource::<Assets<StandardMaterial>>().len(), 6);
    }
}
