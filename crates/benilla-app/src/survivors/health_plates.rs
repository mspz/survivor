//! Optional health bars over allies and enemies, read from the replicated [`Status`] so they work
//! the same for the host and its guests.
use super::{hero::Hero, options::Settings, unit_frames::Status, Phase, RunEntity, Selection};
use crate::entities::{mount::MountChild, overhead_anchor, BoneAttach, OverheadFallback};
use crate::ui_pass::{overlay_z, UiQuad, UiQuadAppend, UiQuads};
use benilla_world::{rig_anim::RigPose, view::WorldCamera};
use bevy::prelude::*;

const SIZE: Vec2 = Vec2::new(46.0, 5.0);
/// Enemies are what the bars are mostly read for in a crowd, so theirs are larger.
const ENEMY_SIZE: Vec2 = Vec2::new(64.0, 8.0);
const BORDER: f32 = 1.0;
const LIFT: f32 = 0.35;
const BACKDROP: [f32; 4] = [0.0, 0.0, 0.0, 0.75];
const ALLY: [f32; 4] = [0.1, 0.85, 0.1, 1.0];
const ENEMY: [f32; 4] = [0.85, 0.1, 0.1, 1.0];

/// The bar's outline and its fill, centred on `center`.
fn bar(center: Vec2, size: Vec2, ratio: f32) -> (Rect, Rect) {
    let inner = Rect::from_center_size(center, size);
    let fill = Rect::new(
        inner.min.x,
        inner.min.y,
        inner.min.x + size.x * ratio.clamp(0.0, 1.0),
        inner.max.y,
    );
    (inner.inflate(BORDER), fill)
}

fn shown(settings: &Settings, status: &Status) -> bool {
    let enabled = if status.friendly {
        settings.ally_plates
    } else {
        settings.enemy_plates
    };
    enabled && status.health > 0.0 && status.maximum > 0.0
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn draw(
    settings: Res<Settings>,
    selection: Res<Selection>,
    phase: Res<State<Phase>>,
    units: Query<(Entity, &Transform, &Status), (With<RunEntity>, Without<Hero>)>,
    camera: Query<(&Camera, &Transform), With<WorldCamera>>,
    anchors: Query<&BoneAttach>,
    poses: Query<&RigPose>,
    fallbacks: Query<&OverheadFallback>,
    globals: Query<&GlobalTransform>,
    mounts: Query<(), With<MountChild>>,
    mut quads: ResMut<UiQuads>,
) {
    let in_run = matches!(
        phase.get(),
        Phase::Playing | Phase::LevelUp | Phase::Paused | Phase::CharacterSheet
    );
    if !(settings.ally_plates || settings.enemy_plates) || selection.in_hub || !in_run {
        return;
    }
    let Ok((camera, pose)) = camera.single() else {
        return;
    };
    let view = GlobalTransform::from(*pose);
    for (entity, transform, status) in &units {
        if !shown(&settings, status) {
            continue;
        }
        let head = overhead_anchor(
            entity, transform, &anchors, &poses, &fallbacks, &globals, &mounts,
        );
        let Ok(center) = camera.world_to_viewport(&view, head + Vec3::Y * LIFT) else {
            continue;
        };
        let size = if status.friendly { SIZE } else { ENEMY_SIZE };
        let (outline, fill) = bar(center, size, status.health / status.maximum);
        for (rect, color) in [
            (outline, BACKDROP),
            (fill, if status.friendly { ALLY } else { ENEMY }),
        ] {
            quads.overlays.push(UiQuad {
                rect,
                z_key: overlay_z::WORLD_TEXT,
                color,
                ..default()
            });
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, draw.in_set(UiQuadAppend));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(friendly: bool, health: f32) -> Status {
        Status {
            name: String::new(),
            health,
            maximum: 100.0,
            level: 1,
            friendly,
            debuffs: vec![],
        }
    }

    #[test]
    fn allies_and_enemies_have_separate_options_and_the_dead_have_no_bar() {
        let mut settings = Settings::defaults();
        assert!(!shown(&settings, &status(true, 50.0)));
        assert!(!shown(&settings, &status(false, 50.0)));
        settings.ally_plates = true;
        assert!(shown(&settings, &status(true, 50.0)));
        assert!(!shown(&settings, &status(false, 50.0)));
        settings.enemy_plates = true;
        assert!(shown(&settings, &status(false, 50.0)));
        assert!(!shown(&settings, &status(false, 0.0)));
    }

    #[test]
    fn the_fill_follows_the_health_share_from_the_left() {
        let (outline, fill) = bar(Vec2::new(100.0, 50.0), SIZE, 0.25);
        assert_eq!(fill.width(), SIZE.x * 0.25);
        assert_eq!(fill.min.x, 100.0 - SIZE.x / 2.0);
        assert_eq!(outline.width(), SIZE.x + BORDER * 2.0);
        assert_eq!(bar(Vec2::ZERO, ENEMY_SIZE, 2.0).1.width(), ENEMY_SIZE.x);
    }
}
