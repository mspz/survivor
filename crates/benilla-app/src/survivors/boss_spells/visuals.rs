//! One reusable mesh follows the terrain; the faint footprint stays visible during its fill.
use super::*;
use bevy::{
    asset::RenderAssetUsages, camera::visibility::NoFrustumCulling, mesh::PrimitiveTopology,
};

#[derive(Resource, Default)]
struct Surface {
    entity: Option<Entity>,
    mesh: Handle<Mesh>,
}
#[derive(Default)]
struct Geometry {
    positions: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
}
impl Geometry {
    fn triangle(&mut self, points: [Vec3; 3], color: Color, ground: &impl Fn(Vec3) -> Vec3) {
        let color = color.to_linear().to_f32_array();
        for p in points {
            self.positions.push(ground(p).to_array());
            self.colors.push(color);
        }
    }
    fn quad(&mut self, points: [Vec3; 4], color: Color, ground: &impl Fn(Vec3) -> Vec3) {
        self.triangle([points[0], points[1], points[2]], color, ground);
        self.triangle([points[0], points[2], points[3]], color, ground);
    }
    #[allow(clippy::too_many_arguments)]
    fn band(
        &mut self,
        center: Vec3,
        inner: f32,
        outer: f32,
        start: f32,
        angle: f32,
        color: Color,
        ground: &impl Fn(Vec3) -> Vec3,
    ) {
        if outer <= inner {
            return;
        }
        let count = ((angle.abs() / std::f32::consts::TAU * 96.0).ceil() as usize).max(1);
        let point = |a: f32, r: f32| center + Vec3::new(a.cos() * r, 0.0, a.sin() * r);
        for i in 0..count {
            let a = start + angle * i as f32 / count as f32;
            let b = start + angle * (i + 1) as f32 / count as f32;
            self.quad(
                [
                    point(a, inner),
                    point(a, outer),
                    point(b, outer),
                    point(b, inner),
                ],
                color,
                ground,
            );
        }
    }
    fn line(&mut self, a: Vec3, b: Vec3, color: Color, ground: &impl Fn(Vec3) -> Vec3) {
        let side = flat(b - a).normalize_or_zero().cross(Vec3::Y) * 0.07;
        self.quad([a - side, a + side, b + side, b - side], color, ground);
    }
    fn warning(&mut self, w: &Warning, ground: &impl Fn(Vec3) -> Vec3) {
        let p = w.progress();
        let base = Color::srgba(0.65, 0.08, 0.02, 0.16);
        let fill = if p >= 1.0 {
            Color::srgba(1.0, 0.12, 0.015, 0.8)
        } else {
            Color::srgba(1.0, 0.38, 0.02, 0.55)
        };
        let edge = Color::srgba(1.0, 0.3, 0.02, 0.95);
        let origin = Vec3::from_array(w.origin);
        let direction = Vec3::from_array(w.direction);
        let tau = std::f32::consts::TAU;
        match w.spell {
            Spell::Cone => {
                let start = direction.z.atan2(direction.x) - std::f32::consts::FRAC_PI_4;
                let angle = std::f32::consts::FRAC_PI_2;
                self.band(origin, 0.0, 13.0, start, angle, base, ground);
                self.band(origin, 0.0, 13.0 * p, start, angle, fill, ground);
                self.band(origin, 12.9, 13.05, start, angle, edge, ground);
                for a in [start, start + angle] {
                    self.line(
                        origin,
                        origin + Vec3::new(a.cos(), 0.0, a.sin()) * 13.0,
                        edge,
                        ground,
                    );
                }
            }
            Spell::Charge => {
                let side = direction.cross(Vec3::Y) * 2.0;
                // Subdivide the corridor so it follows slopes along the charge.
                for i in 0..24 {
                    let a = origin + direction * i as f32;
                    let b = a + direction;
                    self.quad([a - side, a + side, b + side, b - side], base, ground);
                    let amount = (24.0 * p - i as f32).clamp(0.0, 1.0);
                    if amount > 0.0 {
                        let b = a + direction * amount;
                        self.quad([a - side, a + side, b + side, b - side], fill, ground);
                    }
                    self.line(a - side, b - side, edge, ground);
                    self.line(a + side, b + side, edge, ground);
                }
                self.line(origin - side, origin + side, edge, ground);
                let end = origin + direction * 24.0;
                self.line(end - side, end + side, edge, ground);
            }
            Spell::Mortars => {
                for point in &w.mortars {
                    let center = Vec3::from_array(*point);
                    self.band(center, 0.0, 3.5, 0.0, tau, base, ground);
                    self.band(center, 0.0, 3.5 * p, 0.0, tau, fill, ground);
                    self.band(center, 3.4, 3.55, 0.0, tau, edge, ground);
                }
            }
            Spell::Rings => {
                for (inner, outer) in [(4.0, 7.0), (10.0, 13.0), (16.0, 19.0)] {
                    self.band(origin, inner, outer, 0.0, tau, base, ground);
                    self.band(
                        origin,
                        inner,
                        inner + (outer - inner) * p,
                        0.0,
                        tau,
                        fill,
                        ground,
                    );
                    for radius in [inner, outer] {
                        self.band(origin, radius - 0.06, radius + 0.06, 0.0, tau, edge, ground);
                    }
                }
            }
        }
    }
    fn mesh(self) -> Mesh {
        let count = self.positions.len();
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; count])
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
    }
}

#[allow(clippy::too_many_arguments)]
fn draw(
    mut commands: Commands,
    warnings: Res<Warnings>,
    phase: Res<State<Phase>>,
    world: WorldPoint,
    mut surface: ResMut<Surface>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut visible: Query<&mut Visibility>,
) {
    let show = !warnings.0.is_empty()
        && !matches!(*phase.get(), Phase::Hub | Phase::Menu | Phase::GameOver);
    if let Some(entity) = surface.entity {
        if let Ok(mut visibility) = visible.get_mut(entity) {
            *visibility = if show {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }
    if !show {
        return;
    }
    let mut geometry = Geometry::default();
    for warning in &warnings.0 {
        let fallback = warning.origin[1];
        geometry.warning(warning, &|p| {
            enemies::ground(&world, p, fallback) + Vec3::Y * 0.12
        });
    }
    let mesh = geometry.mesh();
    if let Some(existing) = meshes.get_mut(&surface.mesh) {
        *existing = mesh;
    } else {
        surface.mesh = meshes.add(mesh);
        let material = materials.add(StandardMaterial {
            base_color: Color::WHITE,
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
    app.init_resource::<Surface>()
        .add_systems(Update, draw.after(super::tick));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fills_grow_inside_the_same_footprint_and_ring_gaps_stay_empty() {
        let mut w = Warning {
            spell: Spell::Rings,
            origin: [0.0; 3],
            direction: Vec3::X.to_array(),
            mortars: vec![],
            elapsed: 0.0,
        };
        let mut empty = Geometry::default();
        empty.warning(&w, &|p| p);
        w.elapsed = w.spell.windup();
        let mut filled = Geometry::default();
        filled.warning(&w, &|p| p);
        assert!(filled.positions.len() > empty.positions.len());
        for triangle in filled.positions.chunks_exact(3) {
            let center = triangle.iter().map(|p| Vec3::from_array(*p)).sum::<Vec3>() / 3.0;
            let distance = center.length();
            assert!(!(7.1..9.8).contains(&distance) && !(13.1..15.8).contains(&distance));
        }
    }
}
