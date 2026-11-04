//! Native Classic UI textures, read through the existing MPQ UI-art loader.
use super::*;
const BORDER: &str = "Interface\\DialogFrame\\UI-DialogBox-Border";

pub(in crate::survivors) fn rect(x: f32, y: f32, width: f32, height: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(x),
        top: Val::Px(y),
        width: Val::Px(width),
        height: Val::Px(height),
        ..default()
    }
}
pub(in crate::survivors) fn art(
    ui: &mut ChildSpawnerCommands,
    path: &'static str,
    bounds: Node,
    coords: [f32; 4],
) {
    ui.spawn((
        ImageNode::default(),
        hud::UiArt::crop(path, coords),
        bevy::ui::FocusPolicy::Pass,
        bounds,
    ));
}
pub(in crate::survivors) fn frame(ui: &mut ChildSpawnerCommands, width: f32, height: f32) {
    art(
        ui,
        "Interface\\DialogFrame\\UI-DialogBox-Background",
        rect(4.0, 4.0, width - 8.0, height - 8.0),
        [0.0, 1.0, 0.0, 1.0],
    );
    let edge = 12.0;
    for (index, bounds) in [
        (0, rect(0.0, edge, edge, height - 2.0 * edge)),
        (1, rect(width - edge, edge, edge, height - 2.0 * edge)),
        (2, rect(edge, 0.0, width - 2.0 * edge, edge)),
        (3, rect(edge, height - edge, width - 2.0 * edge, edge)),
        (4, rect(0.0, 0.0, edge, edge)),
        (5, rect(width - edge, 0.0, edge, edge)),
        (6, rect(0.0, height - edge, edge, edge)),
        (7, rect(width - edge, height - edge, edge, edge)),
    ] {
        draw_edge(ui, index, bounds);
    }
}

/// A frame that follows its parent's layout, including auto-sized modal dialogs.
pub(in crate::survivors) fn backdrop(ui: &mut ChildSpawnerCommands) {
    art(
        ui,
        "Interface\\DialogFrame\\UI-DialogBox-Background",
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(4.0),
            right: Val::Px(4.0),
            top: Val::Px(4.0),
            bottom: Val::Px(4.0),
            ..default()
        },
        [0.0, 1.0, 0.0, 1.0],
    );
    let p = Val::Px;
    for (index, node) in [
        (
            0,
            Node {
                left: p(0.0),
                top: p(12.0),
                bottom: p(12.0),
                width: p(12.0),
                ..default()
            },
        ),
        (
            1,
            Node {
                right: p(0.0),
                top: p(12.0),
                bottom: p(12.0),
                width: p(12.0),
                ..default()
            },
        ),
        (
            2,
            Node {
                left: p(12.0),
                right: p(12.0),
                top: p(0.0),
                height: p(12.0),
                ..default()
            },
        ),
        (
            3,
            Node {
                left: p(12.0),
                right: p(12.0),
                bottom: p(0.0),
                height: p(12.0),
                ..default()
            },
        ),
        (
            4,
            Node {
                left: p(0.0),
                top: p(0.0),
                width: p(12.0),
                height: p(12.0),
                ..default()
            },
        ),
        (
            5,
            Node {
                right: p(0.0),
                top: p(0.0),
                width: p(12.0),
                height: p(12.0),
                ..default()
            },
        ),
        (
            6,
            Node {
                left: p(0.0),
                bottom: p(0.0),
                width: p(12.0),
                height: p(12.0),
                ..default()
            },
        ),
        (
            7,
            Node {
                right: p(0.0),
                bottom: p(0.0),
                width: p(12.0),
                height: p(12.0),
                ..default()
            },
        ),
    ] {
        draw_edge(
            ui,
            index,
            Node {
                position_type: PositionType::Absolute,
                ..node
            },
        );
    }
}

#[derive(Component, Clone, Copy)]
enum ButtonSkin {
    Red(bool),
    Close,
}
#[derive(Component)]
struct ButtonFace([f32; 4]);
pub(in crate::survivors) fn button<C: Component>(
    ui: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    label: impl Into<String>,
    action: C,
    width: f32,
) {
    button_sized(ui, font, label, action, width, 26.0, 13.0, false);
}
#[allow(clippy::too_many_arguments)]
pub(in crate::survivors) fn button_sized<C: Component>(
    ui: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    label: impl Into<String>,
    action: C,
    width: f32,
    height: f32,
    font_size: f32,
    selected: bool,
) {
    ui.spawn((
        Button,
        action,
        ButtonSkin::Red(selected),
        Node {
            min_width: Val::Px(width),
            min_height: Val::Px(height),
            padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
            flex_shrink: 0.0,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
    ))
    .with_children(|b| {
        // Use the same artwork and authored crop as the character-creation buttons.
        let coords = crate::glue::art::BUTTON_TC;
        b.spawn((
            ButtonFace(coords),
            ImageNode::default(),
            hud::UiArt::crop("Interface\\Glues\\Common\\Glue-Panel-Button-Up", coords),
            bevy::ui::FocusPolicy::Pass,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
        ));
        b.spawn((
            text(font, label, font_size, GOLD),
            TextLayout::new_with_linebreak(bevy::text::LineBreak::NoWrap),
            Node {
                flex_shrink: 0.0,
                ..default()
            },
            bevy::ui::FocusPolicy::Pass,
        ));
    });
}
pub(in crate::survivors) fn close<C: Component>(
    ui: &mut ChildSpawnerCommands,
    action: C,
    x: f32,
    y: f32,
) {
    ui.spawn((
        Button,
        action,
        ButtonSkin::Close,
        ImageNode::default(),
        hud::UiArt::crop(
            "Interface\\Buttons\\UI-Panel-MinimizeButton-Up",
            [0.0, 1.0, 0.0, 1.0],
        ),
        rect(x, y, 28.0, 28.0),
    ));
}
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<BorderImages>()
        .add_systems(Update, (refresh, fill_edges));
}

#[derive(Resource, Default)]
struct BorderImages(Option<crate::glue::backdrop::BackdropEdges>);
#[derive(Component)]
struct BorderEdge(usize);
#[derive(Component)]
struct EdgeReady;
fn draw_edge(ui: &mut ChildSpawnerCommands, index: usize, bounds: Node) {
    ui.spawn((
        ImageNode::default(),
        BorderEdge(index),
        bevy::ui::FocusPolicy::Pass,
        bounds,
    ));
}
fn fill_edges(
    mut commands: Commands,
    mut nodes: Query<(Entity, &BorderEdge, &mut ImageNode), Without<EdgeReady>>,
    mut cache: ResMut<BorderImages>,
    assets: Option<ResMut<benilla_assets::WorldAssets>>,
    images: Option<ResMut<Assets<Image>>>,
) {
    if nodes.is_empty() {
        return;
    }
    let (Some(mut assets), Some(mut images)) = (assets, images) else {
        return;
    };
    if cache.0.is_none() {
        cache.0 = crate::glue::backdrop::backdrop_edges(&mut assets, BORDER, &mut images);
    }
    let Some(edges) = cache.0.as_ref() else {
        return;
    };
    for (entity, index, mut image) in &mut nodes {
        image.image = match index.0 {
            0 => &edges.left,
            1 => &edges.right,
            2 => &edges.top,
            3 => &edges.bottom,
            4 => &edges.tl,
            5 => &edges.tr,
            6 => &edges.bl,
            _ => &edges.br,
        }
        .clone();
        commands.entity(entity).try_insert(EdgeReady);
    }
}
fn refresh(
    mut commands: Commands,
    buttons: Query<(Entity, &Interaction, &ButtonSkin, Option<&Children>), Changed<Interaction>>,
    faces: Query<&ButtonFace>,
) {
    for (entity, interaction, kind, children) in &buttons {
        let path = match (kind, *interaction == Interaction::Pressed) {
            (ButtonSkin::Red(false), false) => "Interface\\Glues\\Common\\Glue-Panel-Button-Up",
            (ButtonSkin::Red(_), _) => "Interface\\Glues\\Common\\Glue-Panel-Button-Down",
            (ButtonSkin::Close, false) => "Interface\\Buttons\\UI-Panel-MinimizeButton-Up",
            (ButtonSkin::Close, true) => "Interface\\Buttons\\UI-Panel-MinimizeButton-Down",
        };
        if matches!(kind, ButtonSkin::Close) {
            commands
                .entity(entity)
                .try_insert(hud::UiArt::crop(path, [0.0, 1.0, 0.0, 1.0]))
                .try_remove::<hud::ArtDone>();
        } else if let Some(children) = children {
            for child in children.iter() {
                if let Ok(face) = faces.get(child) {
                    commands
                        .entity(child)
                        .try_insert(hud::UiArt::crop(path, face.0))
                        .try_remove::<hud::ArtDone>();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Component)]
    struct TestButton;

    #[test]
    fn button_skin_updates_tolerate_panels_despawned_before_commands_apply() {
        fn close_panel(mut commands: Commands, roots: Query<Entity, With<TestButton>>) {
            for entity in &roots {
                commands.entity(entity).despawn();
            }
        }
        let mut app = App::new();
        // Both systems still see the buttons, but the deletion is applied before the skin.
        app.add_systems(Update, (close_panel, refresh).chain_ignore_deferred());
        app.world_mut().spawn((
            TestButton,
            ButtonSkin::Close,
            Interaction::Pressed,
            hud::ArtDone,
        ));
        let red = app
            .world_mut()
            .spawn((TestButton, ButtonSkin::Red(false), Interaction::Pressed))
            .id();
        let face = app
            .world_mut()
            .spawn((ButtonFace([0.0, 1.0, 0.0, 1.0]), hud::ArtDone))
            .id();
        app.world_mut().entity_mut(red).add_child(face);
        app.update();
        assert_eq!(
            app.world_mut()
                .query::<&TestButton>()
                .iter(app.world())
                .count(),
            0
        );
        assert!(app.world().get_entity(face).is_err());
    }

    #[test]
    fn wow_captions_fit_center_and_button_faces_cover_the_measured_bounds() {
        use bevy::app::{HierarchyPropagatePlugin, PropagateSet};
        use bevy::ui::{ComputedUiRenderTargetInfo, ComputedUiTargetCamera};
        let path = benilla_formats::wow_data_or_skip!();
        let mut chain = benilla_formats::open_chain(&path).unwrap();
        // Layout bounds alone cannot detect transparent padding in the button atlas.
        // Check the real pixels sampled by each face, including the right-hand cap.
        for name in ["Up", "Down"] {
            let bytes = chain
                .read_file(&format!(
                    "Interface\\Glues\\Common\\Glue-Panel-Button-{name}.blp"
                ))
                .unwrap();
            let (width, height, pixels) = benilla_formats::blp_to_rgba(&bytes).unwrap();
            let right = (width as f32 * crate::glue::art::BUTTON_TC[1]) as u32;
            let bottom = (height as f32 * crate::glue::art::BUTTON_TC[3]) as u32;
            for (left, right) in [(0, right / 2), (right / 2, right)] {
                let covered = (0..bottom)
                    .flat_map(|y| (left..right).map(move |x| (x, y)))
                    .filter(|(x, y)| pixels[((y * width + x) * 4 + 3) as usize] > 20)
                    .count();
                assert!(
                    covered > ((right - left) * bottom / 2) as usize,
                    "{name} face {left}..{right} must sample the visible button"
                );
            }
        }
        let font_bytes = chain.read_file("Fonts\\FRIZQT__.TTF").unwrap();
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
            bevy::text::TextPlugin,
        ))
        .init_asset::<Image>()
        .init_resource::<UiScale>()
        .init_resource::<bevy::ui::ui_surface::UiSurface>()
        .init_resource::<bevy::transform::StaticTransformOptimizations>()
        .add_plugins(HierarchyPropagatePlugin::<ComputedUiTargetCamera>::new(
            PostUpdate,
        ))
        .add_plugins(HierarchyPropagatePlugin::<ComputedUiRenderTargetInfo>::new(
            PostUpdate,
        ))
        .add_systems(
            PostUpdate,
            (
                bevy::ui::update::propagate_ui_target_cameras,
                bevy::text::detect_text_needs_rerender::<Text>,
                bevy::ui::widget::measure_text_system,
                bevy::ui::ui_layout_system,
                bevy::transform::systems::mark_dirty_trees,
                bevy::transform::systems::sync_simple_transforms,
                bevy::transform::systems::propagate_parent_transforms,
            )
                .chain(),
        )
        .configure_sets(
            PostUpdate,
            PropagateSet::<ComputedUiTargetCamera>::default()
                .after(bevy::ui::update::propagate_ui_target_cameras)
                .before(bevy::ui::widget::measure_text_system),
        )
        .configure_sets(
            PostUpdate,
            PropagateSet::<ComputedUiRenderTargetInfo>::default()
                .after(bevy::ui::update::propagate_ui_target_cameras)
                .before(bevy::ui::widget::measure_text_system),
        );
        app.world_mut().spawn((
            Camera2d,
            Camera {
                computed: bevy::camera::ComputedCameraValues {
                    target_info: Some(bevy::camera::RenderTargetInfo {
                        physical_size: UVec2::new(1280, 720),
                        scale_factor: 1.0,
                    }),
                    ..default()
                },
                viewport: Some(bevy::camera::Viewport {
                    physical_size: UVec2::new(1280, 720),
                    ..default()
                }),
                ..default()
            },
        ));
        let font = app
            .world_mut()
            .resource_mut::<Assets<Font>>()
            .add(Font::try_from_bytes(font_bytes).unwrap());
        app.world_mut()
            .commands()
            .spawn(Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Start,
                row_gap: Val::Px(8.0),
                ..default()
            })
            .with_children(|ui| {
                for label in [
                    "Create New Character",
                    "Enter World",
                    "Game menu [Esc]",
                    "Back to Stormwind",
                    "Talents | Level 40",
                ] {
                    button_sized(ui, &font, label, TestButton, 40.0, 20.0, 16.0, false);
                }
            });
        app.world_mut().flush();
        app.finish();
        app.cleanup();
        for _ in 0..3 {
            app.update();
        }
        let world = app.world_mut();
        let mut query = world
            .query_filtered::<(&ComputedNode, &UiGlobalTransform, &Children), With<TestButton>>();
        let mut count = 0;
        for (bounds, transform, children) in query.iter(world) {
            count += 1;
            assert!(bounds.size.x > 40.0);
            for child in children.iter() {
                let size = world.get::<ComputedNode>(child).unwrap().size;
                if world.get::<Text>(child).is_some() {
                    assert!(size.x > 0.0 && size.y > 0.0);
                    assert!(bounds.size.x >= size.x + 23.0);
                    assert!(bounds.size.y >= size.y + 11.0);
                    let position = world.get::<UiGlobalTransform>(child).unwrap();
                    assert!(position.translation.distance(transform.translation) < 1.5);
                } else if world.get::<ButtonFace>(child).is_some() {
                    assert!((size.y - bounds.size.y).abs() < 1.5);
                    let coords = world.get::<ButtonFace>(child).unwrap().0;
                    assert_eq!(coords, crate::glue::art::BUTTON_TC);
                }
            }
        }
        assert_eq!(count, 5);
    }
}
