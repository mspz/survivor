//! Cosmetic, client-owned paladin ground art with one shader signature per rare mod.
//! Installed client data supplies the textures; no WoW assets are bundled or modified.
use super::{
    Phase, Run,
    enemies::{Enemy, Rank},
    hero::Hero,
    multiplayer,
};
use benilla_assets::M2Model;
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    pbr::{Material, MaterialPlugin},
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderType},
    shader::ShaderRef,
};
use std::collections::{HashMap, HashSet};
#[cfg(test)]
#[path = "rare_mod_auras_tests.rs"]
mod tests;

const PALADIN: [u32; 6] = [465, 7294, 19876, 19888, 19891, 20218];
const GRID: usize = 4;
const REPROJECT_DISTANCE: f32 = 0.2;
const MAX_DISTANCE: f32 = 90.0;

#[derive(Clone, Copy)]
struct Style {
    modifier: Rank,
    source: usize,
    color: [f32; 3],
    accent: [f32; 3],
    rate: f32,
}
const fn style(
    modifier: Rank,
    source: usize,
    color: [f32; 3],
    accent: [f32; 3],
    rate: f32,
) -> Style {
    Style {
        modifier,
        source,
        color,
        accent,
        rate,
    }
}
// Indices are shader motif IDs: never reuse a motif for a second modifier.
const STYLES: [Style; 34] = [
    style(
        Rank::Explosive,
        4,
        [1.0, 0.28, 0.04],
        [1.0, 0.82, 0.18],
        1.25,
    ),
    style(Rank::Healer, 0, [0.22, 0.95, 0.42], [0.87, 1.0, 0.65], 0.65),
    style(
        Rank::Protector,
        0,
        [0.15, 0.42, 1.0],
        [0.55, 0.86, 1.0],
        0.5,
    ),
    style(Rank::Banner, 5, [1.0, 0.72, 0.12], [1.0, 0.97, 0.62], 0.55),
    style(Rank::Rally, 5, [1.0, 0.52, 0.08], [1.0, 0.91, 0.46], 1.05),
    style(
        Rank::DeathAvenger,
        2,
        [0.65, 0.1, 0.28],
        [0.8, 0.72, 0.9],
        0.72,
    ),
    style(
        Rank::Guardian,
        0,
        [0.18, 0.78, 0.92],
        [0.82, 1.0, 1.0],
        0.42,
    ),
    style(
        Rank::BloodLink,
        1,
        [0.9, 0.035, 0.16],
        [1.0, 0.36, 0.48],
        0.9,
    ),
    style(
        Rank::Electrostatic,
        3,
        [0.15, 0.62, 1.0],
        [0.8, 0.98, 1.0],
        1.6,
    ),
    style(
        Rank::Magnetic,
        2,
        [0.58, 0.15, 0.98],
        [0.25, 0.84, 1.0],
        0.82,
    ),
    style(Rank::Charger, 1, [1.0, 0.4, 0.12], [1.0, 0.92, 0.78], 1.35),
    style(
        Rank::BurningWake,
        4,
        [1.0, 0.18, 0.015],
        [1.0, 0.67, 0.04],
        0.92,
    ),
    style(Rank::Elusive, 3, [0.38, 0.88, 0.9], [0.88, 1.0, 1.0], 0.75),
    style(
        Rank::SecondSkin,
        3,
        [0.1, 0.85, 0.68],
        [0.72, 1.0, 0.91],
        0.38,
    ),
    style(Rank::Avenger, 1, [0.94, 0.08, 0.19], [1.0, 0.5, 0.17], 1.08),
    style(
        Rank::Riposte,
        1,
        [0.68, 0.77, 0.96],
        [1.0, 0.59, 0.22],
        0.85,
    ),
    style(Rank::Adaptive, 5, [0.3, 0.8, 1.0], [0.93, 0.3, 0.95], 0.4),
    style(Rank::Aegis, 0, [0.62, 0.65, 1.0], [1.0, 0.95, 0.7], 0.32),
    style(
        Rank::Bastion,
        0,
        [0.69, 0.53, 0.23],
        [0.91, 0.85, 0.62],
        0.22,
    ),
    style(
        Rank::Predator,
        1,
        [1.0, 0.31, 0.12],
        [1.0, 0.78, 0.41],
        0.95,
    ),
    style(
        Rank::Plagued,
        2,
        [0.39, 0.89, 0.07],
        [0.85, 1.0, 0.18],
        0.62,
    ),
    style(
        Rank::Hemorrhagic,
        1,
        [0.75, 0.015, 0.07],
        [1.0, 0.23, 0.32],
        0.7,
    ),
    style(
        Rank::Incendiary,
        4,
        [1.0, 0.46, 0.02],
        [1.0, 0.93, 0.25],
        1.18,
    ),
    style(
        Rank::GreaterArtillery,
        5,
        [0.85, 0.35, 0.98],
        [1.0, 0.75, 0.31],
        0.6,
    ),
    style(Rank::Echo, 2, [0.25, 0.65, 1.0], [0.84, 0.48, 1.0], 0.58),
    style(Rank::Bombardier, 4, [1.0, 0.55, 0.15], [0.95, 0.9, 0.72], 1.12),
    style(Rank::Aftermath, 4, [1.0, 0.12, 0.22], [0.95, 0.9, 0.72], 0.84),
    style(Rank::Flusher, 2, [0.8, 0.32, 1.0], [0.95, 0.9, 0.72], 0.93),
    style(Rank::MageHunter, 3, [0.22, 0.92, 1.0], [0.95, 0.9, 0.72], 1.43),
    style(Rank::ConcentrationBreaker, 2, [0.62, 0.2, 1.0], [0.95, 0.9, 0.72], 1.07),
    style(Rank::Beater, 1, [1.0, 0.72, 0.3], [0.95, 0.9, 0.72], 1.21),
    style(Rank::Besieger, 2, [0.65, 0.38, 0.86], [0.95, 0.9, 0.72], 0.47),
    style(Rank::SiegeVolley, 4, [1.0, 0.48, 0.32], [0.95, 0.9, 0.72], 1.32),
    style(Rank::Desecrator, 2, [0.51, 0.18, 0.72], [0.95, 0.9, 0.72], 0.36),

];

#[derive(Clone, Copy, ShaderType)]
struct AuraUniform {
    color: Vec4,
    accent: Vec4,
    animation: Vec4,
}
#[derive(Asset, TypePath, AsBindGroup, Clone)]
struct AuraMaterial {
    #[uniform(0)]
    params: AuraUniform,
    #[texture(1)]
    #[sampler(2)]
    paladin: Option<Handle<Image>>,
}
impl Material for AuraMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://benilla_app/survivors/rare_mod_auras.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
    fn depth_bias(&self) -> f32 {
        1.0
    }
}
#[derive(Resource, Default)]
struct Art {
    initialized: bool,
    models: [Vec<Handle<M2Model>>; 6],
}
#[derive(Component)]
struct AuraBatch;
struct Batch {
    entity: Entity,
    mesh: Handle<Mesh>,
    material: Handle<AuraMaterial>,
}
struct Footprint {
    center: Vec3,
    radius: f32,
    sampled: f32,
    points: Vec<Vec3>,
}
#[derive(Resource, Default)]
struct Scene {
    batches: Vec<Batch>,
    feet: HashMap<(Entity, usize), Footprint>,
}

fn paladin_paths(
    spells: &benilla_formats::SpellCatalog,
    visuals: &benilla_formats::SpellVisualCatalog,
    id: u32,
) -> Vec<String> {
    let Some(stages) = spells.get(id).and_then(|s| visuals.stages(s.visual)) else {
        return vec![];
    };
    let mut paths = Vec::new();
    for kit in [stages.state, stages.cast, stages.impact] {
        if let Some(kit) = visuals.kit(kit) {
            // Prefer the base attachment (0x13), but inspect every part for a ground quad.
            let mut effects: Vec<_> = kit.effects().collect();
            effects.sort_by_key(|(tag, _)| *tag != 0x13);
            for (_, effect) in effects {
                if let Some(path) = visuals.effect_path(effect) {
                    if !paths.iter().any(|s| s == path) {
                        paths.push(path.to_owned());
                    }
                }
            }
        }
    }
    paths
}
fn load_art(
    mut art: ResMut<Art>,
    server: Res<AssetServer>,
    spells: Option<Res<crate::ui_action::Spells>>,
    visuals: Option<Res<crate::creature_anim::SpellVisuals>>,
) {
    if art.initialized {
        return;
    }
    let (Some(spells), Some(visuals)) = (spells, visuals) else {
        return;
    };
    for (index, id) in PALADIN.into_iter().enumerate() {
        art.models[index] = paladin_paths(&spells.catalog, &visuals.0, id)
            .iter()
            .map(|path| server.load(benilla_assets::m2_url(path)))
            .collect();
    }
    art.initialized = true;
}
fn texture(art: &Art, models: &Assets<M2Model>, source: usize) -> Option<Handle<Image>> {
    art.models[source]
        .iter()
        .filter_map(|h| models.get(h))
        .flat_map(|m| &m.submeshes)
        .find_map(|part| part.ground_quad.as_ref().and(part.texture.clone()))
}

#[derive(Default)]
struct Geometry {
    points: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}
impl Geometry {
    fn push(&mut self, foot: &Footprint, center: Vec3, phase: f32) {
        let offset = self.points.len() as u32;
        let delta = Vec3::new(center.x - foot.center.x, 0.0, center.z - foot.center.z);
        for (index, &p) in foot.points.iter().enumerate() {
            self.points.push((p + delta).to_array());
            self.uvs.push([
                (index % (GRID + 1)) as f32 / GRID as f32,
                (index / (GRID + 1)) as f32 / GRID as f32,
            ]);
            self.colors.push([phase, 1.0, 1.0, 1.0]);
        }
        for z in 0..GRID {
            for x in 0..GRID {
                let a = offset + (z * (GRID + 1) + x) as u32;
                let b = a + 1;
                let c = a + (GRID + 1) as u32;
                let d = c + 1;
                self.indices.extend([a, c, b, b, c, d]);
            }
        }
    }
    fn mesh(self) -> Mesh {
        let normals = vec![[0.0, 1.0, 0.0]; self.points.len()];
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.points)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
        .with_inserted_indices(Indices::U32(self.indices))
    }
}
fn footprint(
    center: Vec3,
    radius: f32,
    now: f32,
    ground: impl Fn(Vec3) -> Option<f32>,
) -> Footprint {
    let mut points = Vec::with_capacity((GRID + 1).pow(2));
    for z in 0..=GRID {
        for x in 0..=GRID {
            let mut p = center
                + Vec3::new(
                    (x as f32 / GRID as f32 * 2.0 - 1.0) * radius,
                    0.0,
                    (z as f32 / GRID as f32 * 2.0 - 1.0) * radius,
                );
            p.y = ground(p).unwrap_or(center.y) + 0.065;
            points.push(p);
        }
    }
    Footprint {
        center,
        radius,
        sampled: now,
        points,
    }
}
fn rings(rank: Rank, body_radius: f32) -> impl Iterator<Item = (usize, f32)> {
    let mut layer = 0;
    STYLES.iter().enumerate().filter_map(move |(index, style)| {
        if !rank.has(style.modifier) {
            return None;
        }
        let radius = (body_radius + 0.65).clamp(1.1, 3.2) + 0.18 * layer as f32;
        layer += 1;
        Some((index, radius))
    })
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn draw(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    run: Res<Run>,
    party: Option<Res<multiplayer::Companions>>,
    world: super::geometry::Surface,
    units: Query<
        (
            Entity,
            &Transform,
            Option<&Enemy>,
            Option<&Rank>,
            Option<&multiplayer::RemoteOwner>,
            Has<Hero>,
            Option<&InheritedVisibility>,
            Option<&super::unit_frames::Status>,
        ),
        Without<AuraBatch>,
    >,
    mut scene: ResMut<Scene>,
    art: Res<Art>,
    models: Res<Assets<M2Model>>,
    images: Option<Res<Assets<Image>>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<AuraMaterial>>,
    mut shown: Query<&mut Visibility, With<AuraBatch>>,
) {
    let active = !matches!(*phase.get(), Phase::Menu | Phase::Hub | Phase::GameOver);
    let focus = units.iter().find(|u| u.5).map(|u| u.1.translation);
    let mut geometry: Vec<Geometry> = (0..STYLES.len()).map(|_| Geometry::default()).collect();
    let mut keep = HashSet::new();
    let now = run.elapsed;
    if active {
        for (entity, tf, enemy, rank, owner, hero, visibility, status) in &units {
            if visibility.is_some_and(|v| !v.get())
                || focus.is_some_and(|p| {
                    super::units::flat(p - tf.translation).length_squared() > MAX_DISTANCE.powi(2)
                })
            {
                continue;
            }
            let (rank, radius) = if let Some(enemy) = enemy {
                if enemy.dead || enemy.hp <= 0.0 {
                    continue;
                }
                (rank.copied().unwrap_or(Rank::Normal), enemy.radius)
            } else if hero {
                if run.hp <= 0.0 || run.dead_t.is_some() {
                    continue;
                }
                (run.monster_mods.rank(now), super::hero::HERO_RADIUS)
            } else if let Some(player) = owner.and_then(|id| party.as_ref()?.0.get(&id.0)) {
                if player.run.hp <= 0.0 || player.run.dead_t.is_some() {
                    continue;
                }
                (player.run.monster_mods.rank(now), super::hero::HERO_RADIUS)
            } else if let (Some(rank), Some(status)) = (rank, status) {
                // Guests mirror Rank + Status, without authoritative Enemy components.
                if status.friendly || status.health <= 0.0 {
                    continue;
                }
                (*rank, if rank.is_boss() { 1.5 } else { 0.6 })
            } else {
                continue;
            };
            let bits = entity.to_bits();
            let phase = ((bits ^ (bits >> 17)).wrapping_mul(0x9e3779b9) & 0xffff) as f32 / 65535.0;
            for (index, radius) in rings(rank, radius) {
                let key = (entity, index);
                keep.insert(key);
                let refresh = scene.feet.get(&key).is_none_or(|f| {
                    f.center.distance_squared(tf.translation) > REPROJECT_DISTANCE.powi(2)
                        || f.radius != radius
                        || now - f.sampled > 0.5
                        || now < f.sampled
                });
                if refresh {
                    scene.feet.insert(
                        key,
                        footprint(tf.translation, radius, now, |p| {
                            world.warning_height_under(p)
                        }),
                    );
                }
                geometry[index].push(&scene.feet[&key], tf.translation, phase);
            }
        }
    }
    scene.feet.retain(|key, _| keep.contains(key));
    if scene.batches.is_empty() && keep.is_empty() {
        return;
    }
    if scene.batches.is_empty() {
        for (index, style) in STYLES.iter().enumerate() {
            let mesh = meshes.add(Geometry::default().mesh());
            let material = materials.add(AuraMaterial {
                params: AuraUniform {
                    color: Color::srgb_from_array(style.color).to_linear().to_vec4(),
                    accent: Color::srgb_from_array(style.accent).to_linear().to_vec4(),
                    animation: Vec4::new(now, index as f32, style.rate, 0.0),
                },
                paladin: None,
            });
            let entity = commands
                .spawn((
                    AuraBatch,
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::default(),
                    Visibility::Hidden,
                    bevy::light::NotShadowCaster,
                    bevy::light::NotShadowReceiver,
                    bevy::camera::visibility::NoFrustumCulling,
                ))
                .id();
            scene.batches.push(Batch {
                entity,
                mesh,
                material,
            });
        }
    }
    for (index, (batch, geometry)) in scene.batches.iter().zip(geometry).enumerate() {
        let visible = !geometry.indices.is_empty();
        if let Ok(mut vis) = shown.get_mut(batch.entity) {
            *vis = if visible {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
        // New batches are visible immediately; existing batches follow the same rule.
        commands.entity(batch.entity).insert(if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        });
        if let Some(material) = materials.get_mut(&batch.material) {
            material.params.animation.x = now % 4096.0;
            material.paladin = texture(&art, &models, STYLES[index].source).filter(|h| {
                images
                    .as_ref()
                    .is_some_and(|images| images.get(h).is_some())
            });
            material.params.animation.w = if material.paladin.is_some() { 1.0 } else { 0.0 };
        }
        if visible {
            if let Some(mesh) = meshes.get_mut(&batch.mesh) {
                *mesh = geometry.mesh();
            }
        }
    }
}
pub(super) fn plugin(app: &mut App) {
    bevy::asset::embedded_asset!(app, "rare_mod_auras.wgsl");
    app.add_plugins(MaterialPlugin::<AuraMaterial>::default())
        .init_resource::<Scene>()
        .init_resource::<Art>()
        .add_systems(
            Update,
            (load_art, draw)
                .chain()
                .in_set(super::session::Stage::Present),
        );
}
