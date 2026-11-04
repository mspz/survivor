//! Reuse the live hero's renderable hierarchy in an isolated camera layer; no duplicate rig.
use super::{
    super::{hero::Hero, Phase, Selection},
    View,
};
use bevy::{
    asset::RenderAssetUsages,
    camera::{
        visibility::{RenderLayers, VisibilitySystems},
        RenderTarget,
    },
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
};
const LAYER: usize = 63; // Outside the client's world, portrait and UI-model camera ranges.
#[derive(Resource, Default)]
pub(super) struct Target {
    pub image: Option<Handle<Image>>,
}
#[derive(Component)]
struct PreviewCamera;
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Target>()
        .add_systems(PostUpdate, sync.before(VisibilitySystems::CheckVisibility));
}
#[allow(clippy::too_many_arguments)]
fn sync(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    selection: Res<Selection>,
    view: Res<View>,
    mut target: ResMut<Target>,
    images: Option<ResMut<Assets<Image>>>,
    heroes: Query<(Entity, &Transform), (With<Hero>, Without<PreviewCamera>)>,
    children: Query<&Children>,
    layers: Query<&RenderLayers>,
    mut cameras: Query<(&mut Camera, &mut Transform), With<PreviewCamera>>,
) {
    let show = selection.sheet_open && super::in_world(*phase.get());
    for (mut camera, _) in &mut cameras {
        camera.is_active = show;
    }
    if !show {
        return;
    }
    let Ok((hero, tf)) = heroes.single() else {
        return;
    };
    if target.image.is_none() {
        let Some(mut images) = images else {
            return;
        };
        let mut image = Image::new_fill(
            Extent3d {
                width: 288,
                height: 372,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[0, 0, 0, 255],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );
        image.texture_descriptor.usage |= TextureUsages::RENDER_ATTACHMENT;
        let image = images.add(image);
        target.image = Some(image.clone());
        commands.spawn((
            PreviewCamera,
            Camera3d::default(),
            bevy::render::view::Hdr,
            bevy::render::view::Msaa::Off,
            Camera {
                order: -1,
                clear_color: ClearColorConfig::Custom(Color::srgb(0.025, 0.022, 0.018)),
                ..default()
            },
            RenderTarget::Image(image.into()),
            RenderLayers::layer(LAYER),
            Projection::Perspective(PerspectiveProjection {
                fov: 36.0_f32.to_radians(),
                near: 0.05,
                far: 30.0,
                ..default()
            }),
            Transform::default(),
        ));
    }
    for entity in std::iter::once(hero).chain(children.iter_descendants(hero)) {
        let layer = layers.get(entity).cloned().unwrap_or_default();
        if layer.intersects(&RenderLayers::default())
            && !layer.intersects(&RenderLayers::layer(LAYER))
        {
            commands.entity(entity).insert(layer.with(LAYER));
        }
    }
    let facing = tf.rotation * Vec3::new(view.yaw.sin() * 5.5, 1.45, -view.yaw.cos() * 5.5);
    let pose = Transform::from_translation(tf.translation + facing)
        .looking_at(tf.translation + Vec3::Y * 1.2, Vec3::Y);
    for (_, mut camera_tf) in &mut cameras {
        *camera_tf = pose;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_targets_only_the_hero_and_stops_rendering_when_closed() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<Phase>()
            .insert_resource(Selection {
                sheet_open: true,
                ..default()
            })
            .init_resource::<View>()
            .init_resource::<Assets<Image>>();
        plugin(&mut app);
        let original = Transform::from_xyz(10.0, 2.0, 3.0);
        let hero = app
            .world_mut()
            .spawn((Hero { vy: 0.0, air: 0.0 }, original))
            .id();
        let other = app.world_mut().spawn(Transform::default()).id();
        app.world_mut()
            .resource_mut::<NextState<Phase>>()
            .set(Phase::Hub);
        app.update();
        app.update();
        assert!(app.world().resource::<Target>().image.is_some());
        assert!(app
            .world()
            .get::<RenderLayers>(hero)
            .unwrap()
            .intersects(&RenderLayers::layer(LAYER)));
        assert!(app
            .world()
            .get::<RenderLayers>(hero)
            .unwrap()
            .intersects(&RenderLayers::default()));
        assert!(app.world().get::<RenderLayers>(other).is_none());
        app.world_mut().resource_mut::<Selection>().sheet_open = false;
        app.update();
        let world = app.world_mut();
        assert!(
            !world
                .query_filtered::<&Camera, With<PreviewCamera>>()
                .single(world)
                .unwrap()
                .is_active
        );
        assert_eq!(*world.get::<Transform>(hero).unwrap(), original);
    }
}
