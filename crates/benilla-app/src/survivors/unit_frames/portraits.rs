//! Two isolated close-up cameras reuse the dressed live models, including remote units.
use super::*;
use bevy::{
    asset::RenderAssetUsages,
    camera::{
        visibility::{RenderLayers, VisibilitySystems},
        RenderTarget,
    },
    render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
};
const LAYERS: [usize; 2] = [61, 62];
#[derive(Component)]
pub(super) struct Portrait(pub bool);
#[derive(Component)]
struct PortraitCamera(usize);
#[derive(Resource, Default)]
struct Portraits {
    images: [Option<Handle<Image>>; 2],
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn changing_target_removes_the_previous_portrait_layer() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<Phase>()
            .init_resource::<Target>()
            .init_resource::<Assets<Image>>()
            .add_plugins(plugin);
        let net = || crate::net::NetEntity {
            kind: benilla_protocol::EntityKind::Unit,
            display_id: Some(367),
            scale: 1.0,
        };
        let a = app.world_mut().spawn((Transform::default(), net())).id();
        let b = app.world_mut().spawn((Transform::default(), net())).id();
        app.world_mut()
            .resource_mut::<NextState<Phase>>()
            .set(Phase::Playing);
        app.world_mut().resource_mut::<Target>().entity = Some(a);
        app.update();
        assert!(app
            .world()
            .get::<RenderLayers>(a)
            .unwrap()
            .intersects(&RenderLayers::layer(LAYERS[1])));
        app.world_mut().resource_mut::<Target>().entity = Some(b);
        app.update();
        assert!(!app
            .world()
            .get::<RenderLayers>(a)
            .unwrap()
            .intersects(&RenderLayers::layer(LAYERS[1])));
        assert!(app
            .world()
            .get::<RenderLayers>(b)
            .unwrap()
            .intersects(&RenderLayers::layer(LAYERS[1])));
    }
}
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Portraits>()
        .add_systems(PostUpdate, sync.before(VisibilitySystems::CheckVisibility));
}
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn sync(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    target: Res<Target>,
    mut portraits: ResMut<Portraits>,
    images: Option<ResMut<Assets<Image>>>,
    heroes: Query<Entity, With<Hero>>,
    units: Query<(&Transform, &crate::net::NetEntity), Without<PortraitCamera>>,
    children: Query<&Children>,
    layers: Query<(Entity, &RenderLayers)>,
    mut cameras: Query<(&PortraitCamera, &mut Camera, &mut Transform)>,
    mut widgets: Query<(&Portrait, &mut ImageNode)>,
    creatures: Option<Res<crate::entities::Creatures>>,
) {
    let show = !matches!(*phase.get(), Phase::Menu | Phase::GameOver);
    let subjects = [heroes.single().ok(), target.entity];
    let mut wanted = std::collections::HashMap::<Entity, [bool; 2]>::new();
    if show {
        for (index, subject) in subjects.iter().enumerate() {
            if let Some(entity) = subject {
                for part in std::iter::once(*entity).chain(children.iter_descendants(*entity)) {
                    wanted.entry(part).or_insert([false; 2])[index] = true;
                }
            }
        }
    }
    // Remove the old target's layer before drawing the new one, including worn items.
    for (entity, layer) in &layers {
        if cameras.get(entity).is_ok() {
            continue;
        }
        let mut next = layer.clone();
        for (index, &id) in LAYERS.iter().enumerate() {
            if wanted.get(&entity).is_some_and(|mask| mask[index]) {
                next = next.with(id);
            } else {
                next = next.without(id);
            }
        }
        if next != *layer {
            commands.entity(entity).insert(next);
        }
    }
    for (entity, mask) in &wanted {
        if layers.get(*entity).is_ok() {
            continue;
        }
        let mut layer = RenderLayers::default();
        for index in 0..2 {
            if mask[index] {
                layer = layer.with(LAYERS[index]);
            }
        }
        commands.entity(*entity).insert(layer);
    }
    let mut images = images;
    for index in 0..2 {
        let subject = subjects[index].and_then(|e| units.get(e).ok());
        if portraits.images[index].is_none() && subject.is_some() && show {
            if let Some(images) = images.as_deref_mut() {
                let mut image = Image::new_fill(
                    Extent3d {
                        width: 192,
                        height: 192,
                        depth_or_array_layers: 1,
                    },
                    TextureDimension::D2,
                    &[15, 12, 9, 255],
                    TextureFormat::Rgba8UnormSrgb,
                    RenderAssetUsages::default(),
                );
                image.texture_descriptor.usage |= TextureUsages::RENDER_ATTACHMENT;
                let image = images.add(image);
                portraits.images[index] = Some(image.clone());
                commands.spawn((
                    PortraitCamera(index),
                    Camera3d::default(),
                    bevy::render::view::Hdr,
                    bevy::render::view::Msaa::Off,
                    Camera {
                        order: -2 - index as isize,
                        clear_color: ClearColorConfig::Custom(Color::srgb(0.05, 0.04, 0.03)),
                        ..default()
                    },
                    RenderTarget::Image(image.into()),
                    RenderLayers::layer(LAYERS[index]),
                    Projection::Perspective(PerspectiveProjection {
                        fov: 32.0_f32.to_radians(),
                        near: 0.05,
                        far: 100.0,
                        ..default()
                    }),
                    Transform::default(),
                ));
            }
        }
        for (id, mut camera, mut tf) in &mut cameras {
            if id.0 != index {
                continue;
            }
            camera.is_active = show && subject.is_some();
            if let Some((body, net)) = subject {
                let size = net.scale.clamp(0.4, 8.0);
                let head = net
                    .display_id
                    .and_then(|id| creatures.as_ref()?.display_anchors(id))
                    .and_then(|a| a.head)
                    .unwrap_or(Vec3::Y * 1.55);
                let at = body.translation + body.rotation * head * size;
                let forward = body.rotation * Vec3::NEG_Z;
                *tf =
                    Transform::from_translation(at + forward * 2.6 * size + Vec3::Y * 0.15 * size)
                        .looking_at(at, Vec3::Y);
            }
        }
    }
    for (portrait, mut image) in &mut widgets {
        if let Some(handle) = &portraits.images[usize::from(portrait.0)] {
            if image.image != *handle {
                image.image = handle.clone();
            }
        }
    }
}
