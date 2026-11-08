//! Replicated party status keeps resurrection rings identical for hosts and guests.
use super::{
    enemies, revival,
    unit_frames::{PartyPlayer, Status},
    Phase, Run, Selection,
};
use benilla_world::world_point::WorldPoint;
use bevy::{
    asset::RenderAssetUsages, camera::visibility::NoFrustumCulling, mesh::PrimitiveTopology,
    prelude::*,
};

#[derive(Resource, Default)]
struct Surface {
    entity: Option<Entity>,
    mesh: Handle<Mesh>,
}

fn positions(center: Vec3, ground: impl Fn(Vec3) -> Vec3) -> Vec<[f32; 3]> {
    let mut vertices = Vec::with_capacity(96 * 6);
    let point = |angle: f32, radius: f32| {
        ground(center + Vec3::new(angle.cos() * radius, 0.0, angle.sin() * radius)).to_array()
    };
    for segment in 0..96 {
        let a = std::f32::consts::TAU * segment as f32 / 96.0;
        let b = std::f32::consts::TAU * (segment + 1) as f32 / 96.0;
        let inner = revival::RADIUS - 0.15;
        let outer = revival::RADIUS;
        let [a, b, c, d] = [
            point(a, inner),
            point(a, outer),
            point(b, outer),
            point(b, inner),
        ];
        vertices.extend([a, b, c, a, c, d]);
    }
    vertices
}

fn fallen(status: &Status) -> bool {
    status.friendly && status.health <= 0.0
}

#[allow(clippy::too_many_arguments)]
fn draw(
    mut commands: Commands,
    run: Res<Run>,
    selection: Res<Selection>,
    phase: Res<State<Phase>>,
    players: Query<(&Transform, &Status), With<PartyPlayer>>,
    world: WorldPoint,
    mut surface: ResMut<Surface>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut visible: Query<&mut Visibility>,
    time: Res<Time<Real>>,
    mut next_mesh_at: Local<f32>,
) {
    let enabled = run.revival.enabled
        && !selection.in_hub
        && !matches!(*phase.get(), Phase::Menu | Phase::Hub | Phase::GameOver);
    let corpses: Vec<_> = players
        .iter()
        .filter(|(_, status)| enabled && fallen(status))
        .map(|(tf, _)| tf.translation)
        .collect();
    if let Some(entity) = surface.entity {
        if let Ok(mut visibility) = visible.get_mut(entity) {
            *visibility = if corpses.is_empty() {
                Visibility::Hidden
            } else {
                Visibility::Visible
            };
        }
    }
    if corpses.is_empty() {
        return;
    }
    // Corpses are stationary; refresh terrain projection at 10 Hz instead of rebuilding each frame.
    let now = time.elapsed_secs();
    if now < *next_mesh_at {
        return;
    }
    *next_mesh_at = now + 0.1;
    let mut vertices = Vec::new();
    for center in corpses {
        vertices.extend(positions(center, |p| {
            enemies::ground(&world, p, center.y) + Vec3::Y * 0.12
        }));
    }
    let count = vertices.len();
    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vertices)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; count]);
    if let Some(existing) = meshes.get_mut(&surface.mesh) {
        *existing = mesh;
    } else {
        surface.mesh = meshes.add(mesh);
        let material = materials.add(StandardMaterial {
            base_color: Color::srgba(0.1, 1.0, 0.25, 0.9),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            ..default()
        });
        surface.entity = Some(
            commands
                .spawn((
                    Mesh3d(surface.mesh.clone()),
                    MeshMaterial3d(material),
                    Transform::default(),
                    NoFrustumCulling,
                ))
                .id(),
        );
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Surface>().add_systems(PostUpdate, draw);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ring_marks_the_real_revival_radius_and_follows_slopes() {
        let center = Vec3::new(12.0, 3.0, -7.0);
        let vertices = positions(center, |p| Vec3::new(p.x, 3.0 + p.x * 0.1, p.z));
        assert_eq!(vertices.len(), 96 * 6);
        for vertex in vertices {
            let p = Vec3::from_array(vertex);
            let distance = Vec2::new(p.x - center.x, p.z - center.z).length();
            assert!((revival::RADIUS - 0.151..=revival::RADIUS + 0.001).contains(&distance));
            assert!((p.y - (3.0 + p.x * 0.1)).abs() < 0.0001);
        }
    }
    #[test]
    fn revived_players_and_dead_enemies_do_not_get_resurrection_rings() {
        let mut status = Status {
            name: "Ally".into(),
            health: 0.0,
            maximum: 100.0,
            level: 1,
            friendly: true,
            debuffs: vec![],
            debuff_stacks: vec![],
        };
        assert!(fallen(&status));
        status.health = 50.0;
        assert!(!fallen(&status));
        status.health = 0.0;
        status.friendly = false;
        assert!(!fallen(&status));
    }
}
