//! The hero: its body, third-person movement, camera, and health.

use benilla_world::view::WorldCamera;
use crate::survivors::geometry::Surface as WorldPoint;
use bevy::input::mouse::{AccumulatedMouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;

use super::data::{self, ClassDef};
use super::options::{Action, Settings};
use super::units::{self, flat, wow_to_bevy, yaw_toward, Fx, Marks, UnitSpec};
use super::{Phase, Run, Selection};
use crate::creature_anim::{move_flags, MovementState};
use crate::net::{FieldChanged, Guid, ObjectStore};

#[derive(Component, Clone, Copy)]
pub(super) struct Hero {
    /// Vertical velocity in yards/second while airborne; 0 on the ground.
    pub(super) vy: f32,
    /// Height above the ground while airborne. Kept here, not in the transform: the creature
    /// ground clamp (`net::motion::ground_clamp_creatures`) re-seats every unit on the floor at
    /// the top of the frame, so the arc is re-applied over the ground each frame.
    pub(super) air: f32,
}

/// The body radius enemies stop at.
pub(super) const HERO_RADIUS: f32 = 0.7;

/// The hop's initial upward speed and the gravity that brings it back down â€” the reference
/// `CMovement` jump seed and gravity (`player/state.rs`'s `JUMP_SPEED` / `GRAVITY`).
const JUMP_SPEED: f32 = 7.955_547;
const GRAVITY: f32 = 19.291_105;

/// Radians of orbit per raw mouse unit while right-dragging.
pub(super) const ORBIT_RATE: f32 = 0.008;
/// Elevation clamp (radians): ~10Â°..~83Â° above the horizon, so the camera never dives underground.
const PITCH_MIN: f32 = 0.18;
const PITCH_MAX: f32 = 1.45;
/// Wheel zoom range, yards from the hero's shoulders.
const ZOOM_MIN: f32 = 1.5;
const ZOOM_MAX: f32 = 70.0;

#[derive(Resource)]
pub(super) struct CamRig {
    pub dist: f32,
    pub orbit: f32,
    /// Orbit azimuth (radians) around the hero; right-drag swings it during play.
    pub yaw: f32,
    /// Elevation above the horizon (radians), clamped so the camera never dives underground.
    pub pitch: f32,
    pub sensitivity: f32,
}

impl Default for CamRig {
    fn default() -> Self {
        Self {
            dist: 12.0,
            orbit: 0.6,
            yaw: 0.0,
            pitch: 0.30,
            sensitivity: load_sensitivity(),
        }
    }
}

fn load_sensitivity() -> f32 {
    crate::local_state::home()
        .and_then(|h| crate::local_state::read_to_string(&h.join("survivors-mouse.txt")).ok())
        .and_then(|s| s.trim().parse::<f32>().ok())
        .filter(|v| v.is_finite())
        .unwrap_or(1.0)
        .clamp(0.1, 3.0)
}

impl CamRig {
    pub(super) fn adjust_sensitivity(&mut self, delta: f32) {
        self.sensitivity = ((self.sensitivity + delta) * 10.0).round().clamp(1.0, 30.0) / 10.0;
        if let Some(home) = crate::local_state::home() {
            let _ = crate::local_state::write_queued(
                &home.join("survivors-mouse.txt"),
                &self.sensitivity.to_string(),
            );
        }
    }
}

/// Where a battleground's runs start.
pub(super) fn arena(map: &data::MapDef) -> Vec3 {
    wow_to_bevy(map.center)
}

/// The world is the battleground being stood in ([`Selection::stage`]): entering one on another
/// continent worldports, which raises the loading screen until the new ground is in.
pub(super) fn follow_map(
    selection: Res<Selection>,
    current: Option<ResMut<benilla_world::world_map::CurrentMap>>,
    mut entered: Local<Option<(usize, bool)>>,
    mut actors: Query<(&mut Transform, Option<&mut Hero>), Or<(With<Hero>, With<super::multiplayer::RemoteOwner>)>>,
    mut player: ResMut<crate::player::Player>,
) {
    let key = (selection.stage, selection.in_hub);
    if *entered != Some(key) {
        let start = arena(selection.stage_def());
        for (mut tf, physics) in &mut actors {
            tf.translation = start;
            if let Some(mut physics) = physics { physics.air = 0.0; physics.vy = 0.0; }
        }
        player.pos = start;
        *entered = Some(key);
    }
    let Some(mut current) = current else {
        return;
    };
    let want = selection.stage_def().map;
    if current.0 != want {
        current.0 = want;
    }
}

/// The selected face is retained; the inventory supplies the worn appearance.
pub(super) fn spawn_hero(
    commands: &mut Commands,
    class: &'static ClassDef,
    display: u32,
    race: u8,
    sex: u8,
    level: u32,
    serial: u64,
    pos: Vec3,
) -> Entity {
    let bytes0 = u32::from(race)
        | u32::from(class.id) << 8
        | u32::from(sex) << 16
        | u32::from(class.power) << 24;
    let e = units::spawn_unit(
        commands,
        &UnitSpec {
            serial,
            display,
            scale: 1.0,
            level,
            faction: units::FACTION_FRIENDLY,
            bytes0,
            weapons: &[],
            sheath: class.sheath,
        },
        pos,
        0.0,
    );
    commands.entity(e).insert((
        Hero { vy: 0.0, air: 0.0 },
        Marks::buffs(),
        super::items::visuals::HeroLook {
            display,
            race,
            sex,
            class: class.id,
        },
        Visibility::Hidden,
    ));
    e
}

/// Midday over the battleground, whatever the clock says.
pub(super) const MINUTE_OF_DAY: u32 = 780;

pub(super) fn daylight(debug: Option<ResMut<benilla_world::dev_state::DebugState>>) {
    let Some(mut debug) = debug else { return };
    if debug.lighting.follow_server_time || debug.lighting.manual_minute != MINUTE_OF_DAY {
        debug.lighting.follow_server_time = false;
        debug.lighting.manual_minute = MINUTE_OF_DAY;
    }
}

/// The ground under the mouse cursor, `None` with the cursor off the window.
/// Aimed mobility spells (Blink, Charge) go toward it.
#[derive(Resource, Default)]
pub(super) struct AimPoint(pub(super) Option<Vec3>);

/// Cast the cursor onto the terrain: the camera ray meets the plane at the hero's height, then the
/// terrain height there, a few times over, which settles on hills within a frame.
pub(super) fn aim_cursor(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    cams: Query<(&Camera, &Transform), With<WorldCamera>>,
    hero: Query<&Transform, (With<Hero>, Without<WorldCamera>)>,
    world: WorldPoint,
    mut aim: ResMut<AimPoint>,
) {
    let point = (|| {
        let cursor = windows.single().ok()?.cursor_position()?;
        let (cam, tf) = cams.single().ok()?;
        let ray = cam
            .viewport_to_world(&GlobalTransform::from(*tf), cursor)
            .ok()?;
        let mut y = hero.single().ok()?.translation.y;
        let mut at = None;
        for _ in 0..4 {
            if ray.direction.y.abs() < 1e-4 {
                break;
            }
            let t = (y - ray.origin.y) / ray.direction.y;
            if t <= 0.0 {
                break;
            }
            let p = ray.origin + *ray.direction * t;
            at = Some(p);
            match world.terrain_height_under(p) {
                Some(h) if (h - y).abs() > 0.05 => y = h,
                _ => break,
            }
        }
        at
    })();
    if aim.0 != point {
        aim.0 = point;
    }
}

/// Identical movement rules for authoritative simulation and guest prediction.
#[allow(clippy::too_many_arguments)]
pub(super) fn step_motion(
    tf: &mut Transform,
    mv: &mut MovementState,
    hm: &mut Hero,
    command: super::session::Command,
    dt: f32,
    speed: f32,
    map: &data::MapDef,
    ground_at: impl Fn(Vec3) -> Option<f32>,
) {
    let interactive = command.active;
    if let Some(facing) = command.facing {
        tf.rotation = Quat::from_rotation_y(yaw_toward(facing));
    } else {
        tf.rotate_y(command.turn * 2.5 * dt);
    }
    let forward = tf.rotation * Vec3::NEG_Z;
    let dir =
        command.direction + forward * command.axes.y + forward.cross(Vec3::Y) * command.axes.x;
    let mut pos = tf.translation;
    let face = (!command.automatic).then_some(forward);
    if let Some(look) = face {
        let target = Quat::from_rotation_y(yaw_toward(look));
        tf.rotation = target;
    }
    if dir != Vec3::ZERO && dt > 0.0 {
        let dir = dir.normalize();
        pos += dir * speed * dt;
        mv.speed = speed;
        mv.flags = match face {
            Some(look) => gait(look, dir),
            None => {
                let target = Quat::from_rotation_y(yaw_toward(dir));
                tf.rotation = tf.rotation.slerp(target, (dt * 20.0).min(1.0));
                move_flags::FORWARD
            }
        };
    } else if mv.speed != 0.0 || mv.flags != 0 {
        mv.speed = 0.0;
        mv.flags = 0;
    }

    let center = arena(map);
    let off = flat(pos - center);
    if !map.full_instance() && off.length() > map.leash * 2.0 {
        // Just entered another battleground: start at its centre.
        pos = center;
    } else if !map.full_instance() && off.length() > map.leash {
        let held = center + off.normalize() * map.leash;
        pos.x = held.x;
        pos.z = held.z;
    }
    let ground = ground_at(pos).unwrap_or(pos.y - hm.air);
    let playing = interactive;
    // Space hops: lift off the terrain, then gravity brings the hero back down.
    if playing && command.jump && hm.vy == 0.0 && hm.air == 0.0 {
        hm.vy = JUMP_SPEED;
    }
    if hm.vy != 0.0 || hm.air > 0.0 {
        hm.vy -= GRAVITY * dt;
        hm.air += hm.vy * dt;
        if hm.air <= 0.0 {
            hm.air = 0.0;
            hm.vy = 0.0;
        }
    } else {
        hm.air = 0.0;
        hm.vy = 0.0;
    }
    pos.y = ground + hm.air;
    // Feed the animation driver: `vertical_speed`'s sign and the `FALLING` flag are what
    // select the Jump (37/38) / Fall (40) animation (`creature_anim::select`).
    mv.vertical_speed = hm.vy;
    if hm.vy != 0.0 {
        mv.flags |= move_flags::FALLING;
    }
    tf.translation = pos;
}

pub(super) fn step_world_motion(
    tf: &mut Transform, mv: &mut MovementState, hm: &mut Hero,
    command: super::session::Command, dt: f32, speed: f32,
    map: &data::MapDef, surface: &WorldPoint,
) {
    let before = tf.translation;
    // The renderer may already have re-seated the feet on the floor. Subtracting the
    // saved jump height would then start a WMO floor ray underneath that floor.
    step_motion(tf, mv, hm, command, dt, speed, map,
        |p| surface.terrain_height_under(p));
    // A map-entry relocation is a teleport, rather than movement through the old map.
    if flat(tf.translation - before).length() <= map.leash {
        tf.translation = surface.constrain(before, tf.translation);
        if let Some(floor) = surface.terrain_height_under(tf.translation) {
            let air = (tf.translation.y - floor).max(0.0);
            if air + 0.01 < hm.air { hm.vy = hm.vy.min(0.0); }
            hm.air = air;
        }
    }
}

/// WASD over the terrain within the battleground's leash, and the streaming focus on the hero.
#[allow(clippy::too_many_arguments)]
pub(super) fn drive_hero(
    time: Res<Time>,
    input: Res<super::session::ServerInput>,
    session: Res<super::session::LocalSession>,
    mut run: ResMut<Run>,
    selection: Res<Selection>,
    world: WorldPoint,
    mut player: ResMut<crate::player::Player>,
    mut hero: Query<(&mut Transform, &mut MovementState, &mut Hero)>,
) {
    let command = input.for_client(session.id());
    let focus = match hero.single_mut() {
        Ok((mut tf, mut mv, mut hm)) => {
            let dt = time.delta_secs();
            let before = tf.translation;
            let speed = run.stats().speed * run.haste_mult();
            step_world_motion(
                &mut tf,
                &mut mv,
                &mut hm,
                command,
                dt,
                speed,
                selection.stage_def(),
                &world,
            );
            let pos = tf.translation;
            super::items::legendary::events::walk(&mut run, before, pos, dt);
            pos
        }
        Err(_) => {
            let mut p = arena(selection.stage_def());
            if !selection.in_hub {
                if let Some(h) = world.terrain_height_under(p) {
                    p.y = h;
                }
            }
            p
        }
    };
    // The terrain streams and the loading screen waits on the ground under this.
    player.active = true;
    player.detached = false;
    player.settling = false;
    player.world_stale = false;
    player.pos = focus;
}

/// Let go of the mouse buttons when the page loses focus or the pointer leaves it. A release that
/// lands elsewhere (alt-tab mid-drag, a browser or OS popup) never reaches the game, so the button
/// would read as held and the unlocked camera would swing with every move until both buttons were
/// clicked again. Survivors only: the engine's mouse-look keeps its buttons across a deactivate,
/// as 1.12.1 does.
pub(super) fn drop_stray_buttons(
    mut focus: MessageReader<bevy::window::WindowFocused>,
    mut left: MessageReader<bevy::window::CursorLeft>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
) {
    let lost = focus.read().any(|f| !f.focused);
    if lost | (left.read().count() > 0) {
        mouse.release_all();
    }
}

/// A stationary character-selection camera at the menu; third person in play.
pub(super) fn place_camera(
    time: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    selection: Res<Selection>,
    roster: Res<super::characters::Roster>,
    mut rig: ResMut<CamRig>,
    mut wheel: MessageReader<MouseWheel>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    keys: Res<ButtonInput<KeyCode>>,
    settings: Res<Settings>,
    player: Res<crate::player::Player>,
    world: WorldPoint,
    hero: Query<&Transform, (With<Hero>, Without<WorldCamera>)>,
    mut cams: Query<&mut Transform, With<WorldCamera>>,
) {
    let interactive = matches!(*phase.get(), Phase::Playing | Phase::Hub)
        && !selection.talents_open
        && !selection.sheet_open
        && !selection.bags_open
        && !selection.ui_captured
        && !selection.pointer_captured
        && !roster.expeditions;
    // With no hero yet, the focus `drive_hero` grounded.
    let focus = hero.single().map_or(player.pos, |t| t.translation);
    // Multiplicative, so a notch moves as far by eye close in as far out; all the way down to an
    // over-the-shoulder view.
    for w in wheel.read() {
        if !interactive {
            continue;
        }
        let notches = match w.unit {
            MouseScrollUnit::Line => w.y,
            MouseScrollUnit::Pixel => w.y * 0.02,
        };
        rig.dist = (rig.dist * (1.0 - 0.12 * notches * settings.zoom_speed).clamp(0.5, 1.5))
            .clamp(ZOOM_MIN, ZOOM_MAX);
    }
    let (eye, look) = if *phase.get() == Phase::Menu {
        let angle = std::f32::consts::PI + rig.orbit;
        let d = 4.8;
        (
            focus + Vec3::new(angle.sin() * d, 1.8, angle.cos() * d),
            focus + Vec3::Y * 1.1,
        )
    } else {
        // Unlocked, either mouse button drags the camera around the hero, as in WoW.
        if interactive && mouse.any_pressed([MouseButton::Left, MouseButton::Right]) {
            rig.yaw -= motion.delta.x
                * ORBIT_RATE
                * rig.sensitivity
                * if settings.invert_x { -1.0 } else { 1.0 };
            rig.pitch = (rig.pitch
                + motion.delta.y
                    * ORBIT_RATE
                    * rig.sensitivity
                    * if settings.invert_y { -1.0 } else { 1.0 })
            .clamp(PITCH_MIN, PITCH_MAX);
        } else if interactive
            && [
                Action::Forward,
                Action::Back,
                Action::TurnLeft,
                Action::TurnRight,
            ]
            .iter()
            .any(|&a| settings.pressed(a, &keys))
        {
            if let Ok(hero) = hero.single() {
                let facing = hero.rotation * Vec3::NEG_Z;
                let target = yaw_toward(facing);
                let delta = (target - rig.yaw + std::f32::consts::PI)
                    .rem_euclid(std::f32::consts::TAU)
                    - std::f32::consts::PI;
                rig.yaw += delta * (time.delta_secs() * 8.0).min(1.0);
            }
        }
        let (sy, cy) = rig.yaw.sin_cos();
        let (sp, cp) = rig.pitch.sin_cos();
        let look = focus + Vec3::Y * 1.4;
        let mut eye = look + Vec3::new(cp * sy, sp, cp * cy) * rig.dist;
        // Never under the ground: a low tilt on a slope would bury the lens.
        if let Some(h) = world.terrain_height_under(eye) {
            eye.y = eye.y.max(h + 0.6);
        }
        (world.camera_eye(look, eye), look)
    };
    let placed = Transform::from_translation(eye).looking_at(look, Vec3::Y);
    for mut cam in &mut cams {
        *cam = placed;
    }
}

/// Damage the hero takes from one blow, after immunity, armour and the shield; returns what got
/// through.
#[cfg(test)]
pub(super) fn hurt_hero(run: &mut Run, raw: f32) -> f32 {
    hurt_hero_tagged(run, raw, super::spell_engine::Tags::PHYSICAL)
}

/// Both hits and periodic ticks retain their damage school's tags.
pub(super) fn hurt_hero_tagged(run: &mut Run, raw: f32, tags: super::spell_engine::Tags) -> f32 {
    let chance = super::extra_spells::dodge_chance(run);
    let roll = if chance > 0.0 {
        run.extra_spells.dodge_rng.f32()
    } else {
        1.0
    };
    hurt_hero_with_dodge(run, raw, tags, roll)
}

fn hurt_hero_with_dodge(
    run: &mut Run,
    raw: f32,
    tags: super::spell_engine::Tags,
    roll: f32,
) -> f32 {
    if raw > 0.0
        && run.pending_levels == 0
        && run.immune_t <= 0.0
        && run.dead_t.is_none()
        && roll < super::extra_spells::dodge_chance(run)
    {
        super::extra_spells::reacted(run, true);
        return 0.0;
    }
    hurt_hero_after_avoidance(run, raw, tags)
}

fn hurt_hero_after_avoidance(run: &mut Run, raw: f32, tags: super::spell_engine::Tags) -> f32 {
    if run.pending_levels > 0 || run.immune_t > 0.0 || run.dead_t.is_some() {
        return 0.0;
    }
    let shield = if run.shield_t > 0.0 { run.shield } else { 0.0 };
    let raw =
        raw * if run.extra_spells.ice_until > run.elapsed {
            0.85
        } else {
            1.0
        } * (1.0 - run.damage_reduction_tagged(tags))
            * (1.0 - run.support.power(run.elapsed, "barkskin"))
            * if super::talent_runtime::active(run, super::talent_design::Mechanic::FeralGuard) {
                0.85
            } else {
                1.0
            };
    // Apply the minimum hit before protection, so a fully absorbed hit cannot leak damage.
    let raw = raw.max(1.0);
    let after_support = super::support_spells::absorb(run, raw);
    let support_absorbed = raw - after_support;
    let absorbed = after_support.min(shield.max(0.0));
    let dmg = after_support - absorbed;
    run.shield -= absorbed;
    if shield > 0.0 && run.shield <= 0.0 {
        super::items::legendary::effects::shield_expired(run, 0.0, true);
    }
    if shield > 0.0
        && run.shield <= 0.0
        && super::talent_runtime::active(run, super::talent_design::Mechanic::Shieldburst)
        && run.legendary.events.shield_origin == super::items::legendary::events::Origin::Skill
    {
        run.talent_runtime.shield_burst += run.talent_runtime.shield_capacity;
    }
    run.taken_meter.record_hit(
        run.taken_source,
        run.elapsed,
        dmg.min(run.hp.max(0.0)),
        false,
        tags.contains(super::spell_engine::Tags::DOT),
    );
    run.hp -= dmg;
    super::items::legendary::events::hurt(run, run.hp + dmg, absorbed + support_absorbed, dmg);
    super::support_spells::hurt(run, dmg);
    dmg
}

/// Shields block an incoming melee blow before armour or absorption is consumed.
pub(super) fn hurt_hero_with_block(run: &mut Run, raw: f32, roll: f32) -> f32 {
    // Reward selection protects each player's own run, including guests on the host.
    if run.pending_levels > 0 {
        return 0.0;
    }
    let block = (run.equipment.block
        + super::talent_runtime::block_chance(run)
        + super::extra_spells::block_bonus(run))
    .clamp(0.0, 0.75);
    if raw > 0.0 && roll >= block && roll < block + super::extra_spells::dodge_chance(run) {
        super::extra_spells::reacted(run, true);
        return 0.0;
    }
    if raw > 0.0 && roll < block {
        let recovery =
            run.equipment.life_on_block + run.stats().max_hp * run.equipment.life_on_block_percent;
        if recovery > 0.0 {
            run.heal(recovery);
        }
        super::extra_spells::reacted(run, false);
        run.talent_runtime.blocked = true;
        super::items::legendary::effects::blocked(run, raw);
        super::items::legendary::events::blocked(run);
        if super::talent_runtime::active(run, super::talent_design::Mechanic::Block) {
            if super::items::legendary::milestones::sources(
                run,
                super::talent_design::Mechanic::Block,
            )
            .contains(&2)
            {
                let max = run.stats().max_hp;
                let amount =
                    super::procs::resolve(run, "proc_holy_heal", max * 0.03, 0.0, 0.0, 0.0, 1).dmg;
                run.heal(amount);
            }
            let warrior_block = super::items::legendary::milestones::sources(
                run,
                super::talent_design::Mechanic::Block,
            )
            .contains(&1);
            for ability in &mut run.abilities {
                if ability.def.key == "thunder_clap" && warrior_block {
                    ability.cd = (ability.cd - 1.0).max(0.0);
                }
            }
        }
        return 0.0;
    }
    hurt_hero_after_avoidance(run, raw, super::spell_engine::Tags::PHYSICAL)
}

/// The clock, regeneration, the timed buffs and their visuals, death and victory.
pub(super) fn hero_vitals(
    time: Res<Time>,
    mut credits: Option<ResMut<Messages<super::healing::Credit>>>,
    mut run: ResMut<Run>,
    mut next: ResMut<NextState<Phase>>,
    mut hero: Query<(Entity, &Guid, &mut ObjectStore, &mut Marks), With<Hero>>,
    mut fields: MessageWriter<FieldChanged>,
    mut fx: Fx,
    players: Option<Res<super::multiplayer::Companions>>,
    clear: Option<Res<super::enemies::ExpeditionClear>>,
    selection: Option<Res<super::Selection>>,
    enemies: Query<&super::enemies::Enemy>,
) {
    let dt = time.delta_secs();
    let stats = run.stats();
    let practice = selection.as_deref().is_some_and(super::training::active);
    let cleared = !practice
        && !data::dungeon(run.expedition_map)
        && run.elapsed >= data::CLEAR_SECS
        && clear.as_deref().is_some_and(|clear| clear.done);
    let Ok((entity, guid, mut store, mut marks)) = hero.single_mut() else {
        return;
    };
    if let Some(t) = run.dead_t.as_mut() {
        *t += dt;
        let finished = *t > 3.0;
        run.elapsed += dt;
        let others_alive = players
            .as_ref()
            .is_some_and(|players| players.0.values().any(|p| p.run.dead_t.is_none()));
        if cleared || (!practice && data::dungeon(run.expedition_map)
            && run.elapsed >= run.expeditions.duration() && others_alive
            && data::dungeon_cleared(run.expedition_map, run.bosses,
                enemies.iter().filter(|e| e.boss && !e.dead).count())) {
            run.won = true;
            next.set(Phase::GameOver);
        } else if finished && (run.owner.is_some() || !others_alive) {
            next.set(Phase::GameOver);
        }
        return;
    }
    run.elapsed += dt;
    if run.shield_t > 0.0 && run.shield_t <= dt {
        let remaining = run.shield;
        super::items::legendary::events::shield_expired(&mut run, remaining);
    }
    let r = &mut *run;
    for t in [&mut r.shield_t, &mut r.immune_t, &mut r.thorns_t] {
        *t = (*t - dt).max(0.0);
    }
    let heal = (stats.regen + super::items::legendary::events::regeneration(&run)) * dt;
    if run.heal_t > 0.0 {
        let amount = run.heal_rate * dt.min(run.heal_t);
        if let Some((owner, skill)) = run.heal_credit {
            let amount = if run.heal_triggered {
                run.recover(amount)
            } else {
                super::items::legendary::events::receive_active_heal(&mut run, amount)
            };
            if let Some(credits) = credits.as_mut() {
                credits.write(super::healing::Credit {
                    owner,
                    skill,
                    amount,
                });
            }
        } else {
            if run.heal_triggered {
                run.heal(amount);
            } else {
                super::items::legendary::events::active_heal(&mut run, amount);
            }
        }
        run.heal_t = (run.heal_t - dt).max(0.0);
        if run.heal_t <= 0.0 {
            run.heal_credit = None;
            run.heal_triggered = false;
        }
    }
    run.heal(heal);
    store.0.merge_diff(
        benilla_protocol::messages::ObjectFields::from_pairs(&[
            (22, run.hp.max(0.0).ceil() as u32),
            (28, stats.max_hp.ceil() as u32),
        ]),
        |index, old, new| {
            fields.write(FieldChanged {
                entity,
                guid: guid.0,
                kind: benilla_protocol::messages::ObjectType::Unit,
                index,
                old,
                new,
            });
        },
    );

    // The buffs show as their own state kits: the shield bubble, the immunity glow, the orbs.
    let mut shown: Vec<u32> = Vec::new();
    if run.shield_t > 0.0 && run.shield > 0.0 {
        shown.push(17);
    }
    if run.immune_t > 0.0 || run.pending_levels > 0 {
        shown.push(642);
    }
    if run.thorns_t > 0.0 {
        shown.push(run.thorns_spell);
    }
    if run.heal_t > 0.0 {
        shown.push(774);
    }
    for owned in &run.abilities {
        if (owned.def.key == "holy_shield" && run.extra_spells.holy_until > run.elapsed)
            || (owned.def.key == "ice_armor" && run.extra_spells.ice_until > run.elapsed)
        {
            shown.push(owned.def.spell);
        }
        if owned.def.key == "lightning_shield" {
            shown.push(owned.def.spell);
        }
    }
    for buff in run.support.buffs.iter().filter(|b| b.until > run.elapsed) {
        shown.push(data::ability(buff.key).spell);
    }
    if run.passives.iter().any(|(p, _)| p.key == "renew") {
        shown.push(139);
    }
    marks.set(&shown);
    marks.flush(entity, guid.0, &mut store, &mut fields);

    if run.hp <= 0.0 {
        run.hp = 0.0;
        run.dead_t = Some(0.0);
        units::kill_unit(&mut store);
        marks.clear();
        marks.flush(entity, guid.0, &mut store, &mut fields);
    } else if cleared || (!practice && data::dungeon(run.expedition_map)
        && run.elapsed >= run.expeditions.duration()
        && data::dungeon_cleared(run.expedition_map, run.bosses,
            enemies.iter().filter(|e| e.boss && !e.dead).count())) {
        run.won = true;
        // A last flourish before the results.
        fx.cast(entity, 642, &[entity], None);
        next.set(Phase::GameOver);
    }
}

/// The ground-plane distance from the hero, the measure every range in the mode uses.
/// The movement flags for running `dir` while facing `look`: forward, backpedal or strafe, by
/// which quarter the run falls in, so the walk, backpedal and strafe animations play.
fn gait(look: Vec3, dir: Vec3) -> u32 {
    let ahead = look.dot(dir);
    // Positive when `dir` is to the hero's left: WoW faces âˆ’Z with +X on the right.
    let left = look.cross(dir).y;
    let quarter = std::f32::consts::FRAC_1_SQRT_2;
    if ahead >= quarter {
        move_flags::FORWARD
    } else if ahead <= -quarter {
        move_flags::BACKWARD
    } else if left > 0.0 {
        move_flags::STRAFE_LEFT
    } else {
        move_flags::STRAFE_RIGHT
    }
}

pub(super) fn reach(hero: Vec3, other: Vec3) -> f32 {
    flat(other - hero).length()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::window::{CursorLeft, WindowFocused};

    #[test]
    fn every_dungeon_allows_walking_far_beyond_the_old_arena() {
        for &index in data::DUNGEONS {
            let map = &data::MAPS[index];
            let start = arena(map) + Vec3::X * 1500.0;
            let mut tf = Transform::from_translation(start);
            step_motion(&mut tf, &mut MovementState::default(), &mut Hero { vy: 0.0, air: 0.0 },
                super::super::session::Command { direction: Vec3::X, active: true, ..default() },
                1.0, 7.0, map, |_| Some(start.y));
            assert!((tf.translation.x - start.x - 7.0).abs() < 0.001, "{}", map.name);
        }
        assert!(!data::HUB.full_instance());
        for &index in data::CAMPAIGN { assert!(!data::MAPS[index].full_instance()); }
    }

    #[test]
    fn dungeon_entry_relocates_once_and_never_resets_exploration() {
        let mut world = World::new();
        world.insert_resource(Selection { stage: data::DUNGEONS[0], in_hub: false, ..default() });
        world.init_resource::<crate::player::Player>();
        let entity = world.spawn((Transform::default(), Hero { vy: 3.0, air: 2.0 })).id();
        let system = world.register_system(follow_map);
        world.run_system(system).unwrap();
        let start = arena(&data::MAPS[data::DUNGEONS[0]]);
        assert_eq!(world.get::<Transform>(entity).unwrap().translation, start);
        let far = start + Vec3::X * 1500.0;
        world.get_mut::<Transform>(entity).unwrap().translation = far;
        world.run_system(system).unwrap();
        assert_eq!(world.get::<Transform>(entity).unwrap().translation, far);
        world.resource_mut::<Selection>().stage = data::DUNGEONS[1];
        world.run_system(system).unwrap();
        assert_eq!(world.get::<Transform>(entity).unwrap().translation, arena(&data::MAPS[data::DUNGEONS[1]]));
        assert_eq!(world.get::<Hero>(entity).unwrap().air, 0.0);
    }

    #[test]
    fn diagonals_and_combined_commands_never_exceed_straight_line_speed() {
        for yaw in [0.0, 0.7, 2.4] {
            for dt in [1.0 / 144.0, 1.0 / 60.0, 0.1] {
                for speed in [7.0, 14.0] {
                    for axes in [Vec2::Y, Vec2::X, Vec2::ONE, Vec2::new(-1.0, 1.0), -Vec2::ONE] {
                        for extra in [Vec3::ZERO, Vec3::X] {
                            let mut tf = Transform::from_translation(arena(&data::HUB));
                            tf.rotation = Quat::from_rotation_y(yaw);
                            let start = tf.translation;
                            let mut movement = MovementState::default();
                            let mut physics = Hero { vy: 0.0, air: 0.0 };
                            step_motion(&mut tf, &mut movement, &mut physics,
                                super::super::session::Command { axes, direction: extra, active: true, ..default() },
                                dt, speed, &data::HUB, |_| Some(start.y));
                            let travelled = flat(tf.translation - start).length();
                            // World coordinates near Goldshire have approximately 0.001 yd precision.
                            assert!((travelled - speed * dt).abs() < 0.001,
                                "axes={axes:?}, yaw={yaw}, dt={dt}: travelled {travelled}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn dodge_caps_at_seventy_five_and_prevents_physical_magical_and_area_damage() {
        use super::super::spell_engine::{Change, Filter, Modifier, Tags};
        for tags in [
            Tags::PHYSICAL,
            Tags::FIRE | Tags::SPELL,
            Tags::SHADOW | Tags::SPELL | Tags::AREA,
            Tags::PHYSICAL | Tags::AREA,
            Tags::FIRE | Tags::DOT,
        ] {
            let mut run = Run {
                hp: 100.0,
                shield: 20.0,
                shield_t: 10.0,
                ..default()
            };
            run.replace_spell_modifiers(
                904,
                [Modifier {
                    source: 904,
                    filter: Filter::default(),
                    change: Change::DodgeChance(1.5),
                }],
            );
            assert_eq!(super::super::extra_spells::dodge_chance(&run), 0.75);
            assert_eq!(hurt_hero_with_dodge(&mut run, 30.0, tags, 0.749), 0.0);
            assert_eq!(run.hp, 100.0);
            assert_eq!(run.shield, 20.0);
            assert!(run.extra_spells.dodge_until > run.elapsed);
            assert!(hurt_hero_with_dodge(&mut run, 30.0, tags, 0.75) > 0.0);
        }
    }

    #[test]
    fn melee_rolls_dodge_once_and_removes_it_when_the_talent_is_refunded() {
        use super::super::spell_engine::{Change, Filter, Modifier};
        let mut run = Run {
            hp: 100.0,
            ..default()
        };
        run.replace_spell_modifiers(
            904,
            [Modifier {
                source: 904,
                filter: Filter::default(),
                change: Change::DodgeChance(0.3),
            }],
        );
        assert_eq!(hurt_hero_with_block(&mut run, 10.0, 0.29), 0.0);
        assert!(hurt_hero_with_block(&mut run, 10.0, 0.9) > 0.0);
        assert_eq!(run.hp, 90.0);
        run.replace_spell_modifiers(904, []);
        assert_eq!(super::super::extra_spells::dodge_chance(&run), 0.0);
        assert!(hurt_hero_with_block(&mut run, 10.0, 0.0) > 0.0);
    }

    #[test]
    fn meters_count_effective_healing_and_damage_after_shields_without_overkill() {
        let mut run = Run {
            hp: 10.0,
            elapsed: 5.0,
            ..default()
        };
        let healed = run.heal(10000.0);
        assert_eq!(run.heal(10000.0), 0.0);
        assert_eq!(
            run.healing_meter
                .rows(5.0)
                .iter()
                .map(|r| r.total)
                .sum::<f64>(),
            f64::from(healed)
        );
        run.hp = 20.0;
        run.shield = 10.0;
        run.shield_t = 5.0;
        run.immune_t = 1.0;
        hurt_hero_tagged(&mut run, 1000.0, super::super::spell_engine::Tags::FIRE);
        assert!(run.taken_meter.rows(5.0).is_empty());
        run.immune_t = 0.0;
        hurt_hero_tagged(&mut run, 1000.0, super::super::spell_engine::Tags::FIRE);
        assert_eq!(run.taken_meter.rows(5.0)[0].total, 20.0);
    }

    #[test]
    fn reward_selection_protects_host_and_guests_until_the_last_choice() {
        use crate::survivors::{hud, spell_engine::Tags, Rng};
        for owner in [None, Some(42)] {
            let mut run = Run {
                owner,
                hp: 100.0,
                shield: 20.0,
                shield_t: 10.0,
                pending_levels: 2,
                ..default()
            };
            run.equipment.block = 1.0;
            let mut offer = hud::Offer::default();
            let mut rng = Rng(42);
            let mut next = NextState::<Phase>::Unchanged;
            for remaining in [2, 1] {
                assert_eq!(run.pending_levels, remaining);
                let hp = run.hp;
                assert_eq!(
                    hurt_hero_tagged(&mut run, 1000.0, Tags::FIRE | Tags::DOT),
                    0.0
                );
                assert_eq!(hurt_hero_with_block(&mut run, 1000.0, 0.0), 0.0);
                assert_eq!(run.hp, hp);
                assert_eq!(run.shield, 20.0);
                assert!(!run.talent_runtime.blocked);
                offer.choices = vec![hud::Choice::Heal];
                hud::pick(0, &mut run, &mut offer, &mut rng, &mut next);
            }
            assert_eq!(run.pending_levels, 0);
            assert!(run.immune_t > 0.0, "the last reward grants a short grace");
            run.immune_t = 0.0;
            assert!(hurt_hero_tagged(&mut run, 30.0, Tags::FIRE) > 0.0);
        }
    }

    fn held_left() -> World {
        let mut world = World::new();
        world.init_resource::<Messages<WindowFocused>>();
        world.init_resource::<Messages<CursorLeft>>();
        let mut mouse = ButtonInput::<MouseButton>::default();
        mouse.press(MouseButton::Left);
        world.insert_resource(mouse);
        world
    }

    fn left_held(world: &World) -> bool {
        world
            .resource::<ButtonInput<MouseButton>>()
            .pressed(MouseButton::Left)
    }

    #[test]
    fn a_held_button_is_dropped_when_the_page_loses_focus() {
        let mut world = held_left();
        world.run_system_once(drop_stray_buttons).unwrap();
        assert!(left_held(&world), "nothing happened: the drag goes on");
        world.write_message(WindowFocused {
            window: Entity::PLACEHOLDER,
            focused: true,
        });
        world.run_system_once(drop_stray_buttons).unwrap();
        assert!(left_held(&world), "gaining focus is no release");
        world.write_message(WindowFocused {
            window: Entity::PLACEHOLDER,
            focused: false,
        });
        world.run_system_once(drop_stray_buttons).unwrap();
        assert!(!left_held(&world));
    }

    #[test]
    fn a_held_button_is_dropped_when_the_pointer_leaves_the_page() {
        let mut world = held_left();
        world.write_message(CursorLeft {
            window: Entity::PLACEHOLDER,
        });
        world.run_system_once(drop_stray_buttons).unwrap();
        assert!(!left_held(&world));
    }
}
