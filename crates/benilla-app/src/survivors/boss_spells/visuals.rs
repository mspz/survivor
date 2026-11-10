//! One reusable mesh follows the terrain; the faint footprint stays visible during its fill.
use super::*;
use bevy::{
    asset::RenderAssetUsages, camera::visibility::NoFrustumCulling, mesh::PrimitiveTopology,
};

#[derive(Resource, Default)]
struct Surface {
    entity: Option<Entity>,
    mesh: Handle<Mesh>,
    ground: std::collections::HashMap<[u32; 3], Vec3>,
    ground_age: f32,
}
#[derive(Default)]
struct Geometry {
    positions: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
}
#[derive(Default)]
struct GroundSamples(std::cell::RefCell<std::collections::HashMap<[u32; 3], Vec3>>);
impl GroundSamples {
    fn sample(&self, p: Vec3, ground: &impl Fn(Vec3) -> Vec3) -> Vec3 {
        *self.0.borrow_mut().entry(p.to_array().map(f32::to_bits)).or_insert_with(|| ground(p))
    }
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
        // Grounding only the corners leaves large triangles cutting through hills. Subdivide
        // in BOTH directions, including radial cone fills, wide lanes and their long outlines.
        const SPACING: f32 = 2.0;
        let steps = |a: Vec3, b: Vec3, c: Vec3, d: Vec3| {
            (flat(a - b).length().max(flat(c - d).length()) / SPACING).ceil().max(1.0) as usize
        };
        let columns = steps(points[0], points[1], points[3], points[2]);
        let rows = steps(points[0], points[3], points[1], points[2]);
        if columns == 1 && rows == 1 {
            let [a, b, c, d] = points.map(ground);
            self.triangle([a, b, c], color, &|p| p);
            self.triangle([a, c, d], color, &|p| p);
            return;
        }
        let point = |column: usize, row: usize| {
            let u = column as f32 / columns as f32;
            let v = row as f32 / rows as f32;
            ground(points[0].lerp(points[1], u).lerp(points[3].lerp(points[2], u), v))
        };
        // Reuse the shared row even when the caller has no terrain cache.
        let mut previous: Vec<_> = (0..=columns).map(|column| point(column, 0)).collect();
        let mut next = Vec::with_capacity(columns + 1);
        for row in 1..=rows {
            next.extend((0..=columns).map(|column| point(column, row)));
            for column in 0..columns {
                let [a, b, c, d] = [previous[column], previous[column + 1], next[column + 1], next[column]];
                self.triangle([a, b, c], color, &|p| p);
                self.triangle([a, c, d], color, &|p| p);
            }
            std::mem::swap(&mut previous, &mut next);
            next.clear();
        }
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
        let rows = ((outer - inner) / 2.0).ceil() as usize;
        let point = |a: f32, r: f32| center + Vec3::new(a.cos() * r, 0.0, a.sin() * r);
        for i in 0..count {
            let a = start + angle * i as f32 / count as f32;
            let b = start + angle * (i + 1) as f32 / count as f32;
            // Anchor radial subdivisions to the fixed inner radius. Rescaling every row to
            // the animated outer radius used to invalidate ALL terrain samples each frame.
            // Only the advancing front now introduces new points; completed rows stay cached.
            for row in 0..rows {
                let near = inner + row as f32 * 2.0;
                let far = (near + 2.0).min(outer);
                self.quad(
                    [point(a, near), point(a, far), point(b, far), point(b, near)],
                    color,
                    ground,
                );
            }
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
        if let Some([radius, _]) = w.circle {
            self.band(origin, 0.0, radius, 0.0, tau, base, ground);
            self.band(origin, 0.0, radius * p, 0.0, tau, fill, ground);
            self.band(
                origin,
                (radius - 0.1).max(0.0),
                radius + 0.05,
                0.0,
                tau,
                edge,
                ground,
            );
            return;
        }
        match w.spell {
            Spell::Dungeon(_) => {
                for zone in &w.zones {
                    match *zone {
                        dungeons::Zone::Ring { center, inner, outer } => {
                            let center = Vec3::from_array(center);
                            self.band(center, inner, outer, 0.0, tau, base, ground);
                            self.band(center, inner, inner + (outer - inner) * p, 0.0, tau, fill, ground);
                            for radius in [inner, outer] {
                                self.band(center, (radius - 0.07).max(0.0), radius + 0.07, 0.0, tau, edge, ground);
                            }
                        }
                        dungeons::Zone::Lane { from, to, width } => {
                            let from = Vec3::from_array(from);
                            let to = Vec3::from_array(to);
                            let side = flat(to - from).normalize_or_zero().cross(Vec3::Y) * width;
                            self.quad([from - side, from + side, to + side, to - side], base, ground);
                            if p > 0.0 {
                                self.quad([from - side * p, from + side * p, to + side * p, to - side * p], fill, ground);
                            }
                            self.line(from - side, to - side, edge, ground);
                            self.line(from + side, to + side, edge, ground);
                        }
                        dungeons::Zone::Sector { center, inner, outer, start, angle } => {
                            let center = Vec3::from_array(center);
                            self.band(center, inner, outer, start, angle, base, ground);
                            self.band(center, inner, inner + (outer - inner) * p, start, angle, fill, ground);
                            for radius in [inner, outer] {
                                self.band(center, radius - 0.07, radius + 0.07, start, angle, edge, ground);
                            }
                            for a in [start, start + angle] {
                                let direction = Vec3::new(a.cos(), 0.0, a.sin());
                                self.line(center + direction * inner, center + direction * outer, edge, ground);
                            }
                        }
                    }
                }
            }
            Spell::Cone => {
                let start = direction.z.atan2(direction.x) - std::f32::consts::FRAC_PI_4;
                let angle = std::f32::consts::FRAC_PI_2;
                self.band(origin, 0.0, CONE_RADIUS, start, angle, base, ground);
                self.band(origin, 0.0, CONE_RADIUS * p, start, angle, fill, ground);
                self.band(origin, CONE_RADIUS - 0.1, CONE_RADIUS + 0.05, start, angle, edge, ground);
                for a in [start, start + angle] {
                    self.line(
                        origin,
                        origin + Vec3::new(a.cos(), 0.0, a.sin()) * CONE_RADIUS,
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
                    self.band(center, 0.0, MORTAR_RADIUS, 0.0, tau, base, ground);
                    self.band(center, 0.0, MORTAR_RADIUS * p, 0.0, tau, fill, ground);
                    self.band(center, MORTAR_RADIUS - 0.1, MORTAR_RADIUS + 0.05, 0.0, tau, edge, ground);
                }
            }
            Spell::Rings => {
                for (inner, outer) in RING_BANDS {
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
    time: Res<Time>,
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
        surface.ground.clear();
        surface.ground_age = 0.0;
        return;
    }
    surface.ground_age += time.delta_secs();
    // Terrain is static between streaming updates. Reuse the denser footprint's samples for
    // one second, while the fill still animates every frame; bound memory as fill vertices move.
    if surface.ground_age >= 1.0 || surface.ground.len() > 32_768 {
        surface.ground.clear();
        surface.ground_age = 0.0;
    }
    let mut geometry = Geometry::default();
    let heights = GroundSamples(std::cell::RefCell::new(std::mem::take(&mut surface.ground)));
    for warning in &warnings.0 {
        geometry.warning(warning, &|p| {
            heights.sample(p, &|p| {
                Vec3::new(p.x, world.warning_height_under(p).unwrap_or(p.y) + 0.12, p.z)
            })
        });
    }
    surface.ground = heights.0.into_inner();
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
    app.init_resource::<Surface>().add_systems(
        Update,
        draw.after(super::tick)
            .after(crate::survivors::special_mobs::tick),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn advancing_a_circle_fill_only_samples_the_new_front() {
        let mut warning = Warning {
            spell: Spell::Cone, origin: [0.0; 3], direction: Vec3::X.to_array(),
            mortars: vec![], elapsed: Spell::Cone.windup() * 0.5, circle: Some([18.0, Spell::Cone.windup()]),
            windup_seconds: None, zones: vec![],
        };
        let samples = GroundSamples::default();
        let queries = std::cell::Cell::new(0usize);
        let ground = |p| samples.sample(p, &|p| { queries.set(queries.get() + 1); p });
        assert!((warning.progress() - 0.5).abs() < 0.001);
        Geometry::default().warning(&warning, &ground);
        let first = queries.get();
        warning.elapsed += Spell::Cone.windup() * 0.0002;
        Geometry::default().warning(&warning, &ground);
        let added = queries.get() - first;
        assert!(added > 0, "the moving front must actually sample new terrain");
        eprintln!("animated fill: {first} initial terrain samples, {added} on next frame");
        assert!(added * 4 < first, "completed rows should reuse the existing terrain samples");
    }
    #[test]
    fn large_cones_circles_and_lanes_follow_a_rounded_hill_between_their_corners() {
        let height = |p: Vec3| 6.0 + p.x * 0.3 - 0.025 * (p.x * p.x + p.z * p.z);
        let ground = |p: Vec3| Vec3::new(p.x, height(p) + 0.12, p.z);
        let mut mesh = Geometry::default();
        let mut warning = Warning {
            spell: Spell::Cone, origin: [0.0, 6.0, 0.0], direction: Vec3::X.to_array(),
            mortars: vec![], elapsed: Spell::Cone.windup(), circle: None,
            windup_seconds: None, zones: vec![],
        };
        mesh.warning(&warning, &ground);
        warning.circle = Some([18.0, 0.0]);
        mesh.warning(&warning, &ground);
        mesh.quad([
            Vec3::new(-12.0, 6.0, -5.0), Vec3::new(12.0, 6.0, -5.0),
            Vec3::new(12.0, 6.0, 5.0), Vec3::new(-12.0, 6.0, 5.0),
        ], Color::WHITE, &ground);
        for triangle in mesh.positions.chunks_exact(3) {
            let center = triangle.iter().map(|&p| Vec3::from_array(p)).sum::<Vec3>() / 3.0;
            assert!(center.y >= height(center), "triangle cuts through hill: {center:?}");
            for &vertex in triangle {
                let vertex = Vec3::from_array(vertex);
                assert!((vertex.y - height(vertex) - 0.12).abs() < 0.0001);
            }
        }
    }
    #[test]
    fn overlapping_ring_meshes_reuse_ground_queries_without_changing_vertices() {
        let warning = Warning {
            spell: Spell::Rings, origin: [0.0; 3], direction: Vec3::X.to_array(),
            mortars: vec![], elapsed: Spell::Rings.windup() * 0.5, circle: None,
            windup_seconds: None, zones: vec![],
        };
        let ground = |p: Vec3| Vec3::new(p.x, p.x * 0.2 + p.z * 0.1, p.z);
        let mut reference = Geometry::default();
        let mut cached = Geometry::default();
        let samples = GroundSamples::default();
        let queries = std::cell::Cell::new(0usize);
        for _ in 0..2 {
            reference.warning(&warning, &ground);
            cached.warning(&warning, &|p| samples.sample(p, &|p| {
                queries.set(queries.get() + 1);
                ground(p)
            }));
        }
        assert_eq!(reference.positions, cached.positions);
        assert_eq!(reference.colors, cached.colors);
        let original_queries = reference.positions.len();
        eprintln!("warning terrain queries: {original_queries} -> {}", queries.get());
        assert!(queries.get() * 3 < original_queries);
    }
    #[test]
    fn dungeon_mesh_fill_matches_the_authoritative_damage_shapes() {
        let targets = [Player { entity: Entity::PLACEHOLDER, owner: None, pos: Vec3::X * 20.0 }];
        for &map in super::super::super::data::DUNGEONS {
        for boss in &super::super::super::data::roster(map).bosses {
        let skills = dungeons::loadout(map, boss.name, 3);
        for slot in skills.slots {
            let Spell::Dungeon(ability) = slot.spell else { unreachable!() };
            for phase in 0..=2 {
                let mut warning = dungeons::warning(ability, Vec3::ZERO, phase, &targets);
                warning.elapsed = warning.spell.windup();
                let mut mesh = Geometry::default();
                mesh.warning(&warning, &|p| p);
                assert!(!mesh.positions.is_empty());
                for (triangle, colors) in mesh.positions.chunks_exact(3).zip(mesh.colors.chunks_exact(3)) {
                    if (colors[0][3] - 0.8).abs() > 0.001 { continue; }
                    let center = triangle.iter().map(|&p| Vec3::from_array(p)).sum::<Vec3>() / 3.0;
                    assert!(warning.contains(center, 0.01), "{center:?}");
                }
            }
        }
        }
        }
    }
    #[test]
    fn fills_grow_inside_the_same_footprint_and_ring_gaps_stay_empty() {
        let mut w = Warning {
            spell: Spell::Rings,
            origin: [0.0; 3],
            direction: Vec3::X.to_array(),
            mortars: vec![],
            elapsed: 0.0,
            circle: None,
            windup_seconds: None,
            zones: vec![],
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
            assert!(RING_BANDS.iter().any(|&(inner, outer)| distance >= inner - 0.1 && distance <= outer + 0.1));
        }
    }
}
