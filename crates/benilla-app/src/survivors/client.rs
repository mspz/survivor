//! Presentation and local input adapter.
use super::*;

pub(super) fn input(
    keys: Res<ButtonInput<KeyCode>>,
    settings: Res<options::Settings>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<bevy::input::mouse::AccumulatedMouseMotion>,
    phase: Res<State<Phase>>,
    roster: Res<characters::Roster>,
    run: Res<Run>,
    selection: Res<Selection>,
    rig: Res<hero::CamRig>,
    aim: Res<hero::AimPoint>,
    auto: Option<Res<auto::Auto>>,
    session: Res<session::LocalSession>,
    mut server: ResMut<session::Sessions>,
) {
    use options::Action;
    let interactive = matches!(*phase.get(), Phase::Playing | Phase::Hub)
        && !selection.talents_open
        && !selection.sheet_open
        && !selection.bags_open
        && !selection.ui_captured
        && !roster.expeditions;
    let mut direction = Vec3::ZERO;
    let mut axes = Vec2::ZERO;
    let mut turn = 0.0;
    let mut facing = None;
    if let Some(auto) = auto
        .as_deref()
        .filter(|_| *phase.get() == Phase::Playing && run.dead_t.is_none())
    {
        direction = super::auto::steer(auto);
    } else if interactive && run.dead_t.is_none() {
        {
            let right = mouse.pressed(MouseButton::Right) && !selection.pointer_captured;
            if right {
                // Camera input is applied later in this chain; include this frame's drag.
                let yaw = rig.yaw - motion.delta.x * hero::ORBIT_RATE * rig.sensitivity;
                facing = Some(Vec3::new(-yaw.sin(), 0.0, -yaw.cos()));
            } else {
                turn = (settings.pressed(Action::TurnLeft, &keys) as u8 as f32)
                    - (settings.pressed(Action::TurnRight, &keys) as u8 as f32);
            }
            let advance = settings.pressed(Action::Forward, &keys)
                || (mouse.pressed(MouseButton::Left) && right);
            let back = settings.pressed(Action::Back, &keys);
            let left = settings.pressed(Action::StrafeLeft, &keys)
                || (right && settings.pressed(Action::TurnLeft, &keys));
            let rightward = settings.pressed(Action::StrafeRight, &keys)
                || (right && settings.pressed(Action::TurnRight, &keys));
            axes.y = (advance as u8 as f32) - (back as u8 as f32);
            axes.x = (rightward as u8 as f32) - (left as u8 as f32);
        }
    }

    let _ = session.submit(
        &mut server,
        session::Command {
            direction,
            facing,
            axes,
            turn,
            automatic: auto.is_some(),
            active: interactive && run.dead_t.is_none(),
            jump: settings.just_pressed(Action::Jump, &keys),
            mobility: interactive && settings.just_pressed(Action::Mobility, &keys),
            aim: aim.0,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn meter_pointer_capture_keeps_keyboard_movement_but_blocks_mouse_movement() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<bevy::input::mouse::AccumulatedMouseMotion>()
            .init_resource::<options::Settings>()
            .init_resource::<characters::Roster>()
            .init_resource::<Run>()
            .init_resource::<hero::CamRig>()
            .init_resource::<hero::AimPoint>()
            .init_resource::<session::Sessions>()
            .init_resource::<session::LocalSession>()
            .init_resource::<session::ServerInput>()
            .insert_resource(State::new(Phase::Playing))
            .insert_resource(Selection {
                pointer_captured: true,
                ..default()
            })
            .add_systems(Update, (input, session::receive).chain());
        let id = app.world().resource::<session::LocalSession>().id();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);
        for button in [MouseButton::Left, MouseButton::Right] {
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(button);
        }
        app.update();
        let command = app
            .world()
            .resource::<session::ServerInput>()
            .for_client(id);
        assert!(command.active);
        assert_eq!(command.axes, Vec2::Y);
        assert!(command.facing.is_none());
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::KeyW);
        app.update();
        assert_eq!(
            app.world()
                .resource::<session::ServerInput>()
                .for_client(id)
                .axes,
            Vec2::ZERO
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);
        app.world_mut().resource_mut::<Selection>().ui_captured = true;
        app.update();
        assert!(
            !app.world()
                .resource::<session::ServerInput>()
                .for_client(id)
                .active
        );
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_plugins((boss_spells::plugin, super::enemy_visuals::plugin));
    app.add_message::<units::CombatNumber>()
        .init_resource::<session::LocalSession>()
        .add_plugins(grade::plugin)
        .add_plugins(characters::plugin)
        .add_plugins(hub_ground::plugin)
        .add_plugins(options::plugin)
        .add_plugins(codex::plugin)
        .add_plugins(player_auras::plugin)
        .add_plugins(combat_ui::plugin)
        .add_plugins(unit_frames::plugin)
        .add_plugins(items::plugin)
        .add_plugins(shared_chest::plugin)
        .add_plugins(super::gambling::plugin)
        .add_plugins(runes::plugin)
        .init_resource::<crate::entities::dest_fx::AreaVisualScales>()
        .init_resource::<crate::entities::dest_fx::EchoCasts>()
        .add_plugins(talent_ui::plugin)
        .init_resource::<hero::CamRig>()
        .init_resource::<hero::AimPoint>()
        .init_resource::<hud::Offer>()
        .init_resource::<cards::PendingPick>()
        .init_resource::<cards::BanishMode>()
        .add_plugins(bevy::ui_render::UiMaterialPlugin::<hud::SweepUiMaterial>::default())
        .add_plugins(bevy::ui_render::UiMaterialPlugin::<fill::FillMaterial>::default())
        .add_systems(Startup, hud::setup_hud)
        .add_systems(Update, hero::daylight)
        .add_systems(
            Update,
            hud::keys
                .after(characters::input)
                .after(talent_ui::input)
                .after(combat_ui::input)
                .before(hud::show_panels),
        )
        .add_systems(
            Update,
            (hero::drop_stray_buttons, hero::aim_cursor, input)
                .chain()
                .in_set(session::Stage::Input),
        )
        .add_systems(Update, hero::place_camera.in_set(session::Stage::Present))
        .add_systems(
            Update,
            (
                hero::follow_map,
                hud::menu_preview,
                hud::buttons,
                hud::refresh_menu,
                menu::build_menu,
                menu::menu_clicks,
                menu::refresh_menu_ui.before(crate::glue::GlueVisuals),
                cards::deal_cards,
                cards::draft_controls,
                cards::choose_card,
                cards::animate_cards,
                hud::refresh_hud,
                hud::refresh_results,
                hud::fill_icons,
                hud::update_cooldown_sweeps,
                hud::toggle_training_spells,
                hud::refresh_mobility,
                hud::show_panels,
            )
                .chain()
                .after(WorldStage::Input),
        )
        .add_systems(
            Update,
            (
                hud::apply_ui_scale,
                hud::fill_art,
                hud::xp_hover,
                hud::icon_tooltip,
                minimap::map_buttons,
                minimap::zone_text,
                minimap::compose,
            )
                .chain()
                .after(hud::show_panels),
        );
}
