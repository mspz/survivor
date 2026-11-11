//! Host-owned special-mod state. Triggered copies and copied healing never trigger themselves.
use super::{
    Run, enemies::Enemy, monster_mods::Monster, monster_stats::Bonuses, spell_engine::Tags,
};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
#[path = "player_monster_mods.rs"]
pub(super) mod player;

#[derive(Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct Config {
    pub radius: f32,
    pub rally_threshold: f32,
    pub rally_cooldown: f32,
    pub rally_shield: f32,
    pub rally_duration: f32,
    pub push_distance: f32,
    pub death_duration: f32,
    pub death_damage: f32,
    pub death_action_speed: f32,
    pub death_reduction: f32,
    pub death_reduction_cap: f32,
    pub guardian_interval: f32,
    pub guardian_shield: f32,
    pub guardian_duration: f32,
    pub blood_interval: f32,
    pub blood_heal: f32,
    pub blood_copy: f32,
    pub electric_distance: f32,
    pub electric_radius: f32,
    pub electric_damage: f32,
    pub magnetic_interval: f32,
    pub magnetic_windup: f32,
    pub pull_distance: f32,
    pub charge_distance: f32,
    pub charge_length: f32,
    pub charge_duration: f32,
    pub charge_windup: f32,
    pub charge_damage: f32,
    pub wake_spacing: f32,
    pub wake_interval: f32,
    pub fire_duration: f32,
    pub fire_radius: f32,
    pub fire_damage: f32,
    pub elusive_seconds: f32,
    pub skin_threshold: f32,
    pub skin_shield: f32,
    pub skin_duration: f32,
    pub skin_cooldown: f32,
    pub reserve_cap: f32,
    pub reserve_threshold: f32,
    pub reserve_cooldown: f32,
    pub riposte_block: f32,
    pub block_cap: f32,
    pub riposte_damage: f32,
    pub riposte_duration: f32,
    pub adaptation_duration: f32,
    pub adaptation_taken: f32,
    pub aegis_delay: f32,
    pub aegis_shield: f32,
    pub aegis_duration: f32,
    pub bastion_per_second: f32,
    pub bastion_cap: f32,
    pub bastion_grace: f32,
    pub predator_radius: f32,
    pub predator_damage: f32,
    pub infection_duration: f32,
    pub infection_damage: f32,
    pub bleed_duration: f32,
    pub bleed_damage: f32,
    pub bleed_vulnerability: f32,
    pub ignite_chance: f32,
    pub ignite_duration: f32,
    pub ignite_damage: f32,
    pub boss_repeat_delay: f32,
    pub repeat_delay: f32,
    pub death_stack_cap: u32,
    pub extra_projectiles: u32,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            radius: 10.0,
            rally_threshold: 0.35,
            rally_cooldown: 20.0,
            rally_shield: 0.2,
            rally_duration: 5.0,
            push_distance: 4.0,
            death_duration: 8.0,
            death_damage: 0.1,
            death_action_speed: 0.08,
            death_reduction: 0.12,
            death_reduction_cap: 0.6,
            guardian_interval: 6.0,
            guardian_shield: 0.15,
            guardian_duration: 5.0,
            blood_interval: 6.0,
            blood_heal: 0.5,
            blood_copy: 0.3,
            electric_distance: 8.0,
            electric_radius: 8.0,
            electric_damage: 1.2,
            magnetic_interval: 8.0,
            magnetic_windup: 1.0,
            pull_distance: 3.0,
            charge_distance: 12.0,
            charge_length: 6.0,
            charge_duration: 0.3,
            charge_windup: 0.5,
            charge_damage: 1.5,
            wake_spacing: 3.0,
            wake_interval: 0.75,
            fire_duration: 3.0,
            fire_radius: 2.5,
            fire_damage: 0.25,
            elusive_seconds: 2.0,
            skin_threshold: 0.3,
            skin_shield: 1.0,
            skin_duration: 5.0,
            skin_cooldown: 30.0,
            reserve_cap: 0.5,
            reserve_threshold: 0.2,
            reserve_cooldown: 3.0,
            riposte_block: 0.5,
            block_cap: 0.75,
            riposte_damage: 2.0,
            riposte_duration: 5.0,
            adaptation_duration: 6.0,
            adaptation_taken: 0.5,
            aegis_delay: 5.0,
            aegis_shield: 0.25,
            aegis_duration: 8.0,
            bastion_per_second: 0.25,
            bastion_cap: 1.0,
            bastion_grace: 2.0,
            predator_radius: 6.0,
            predator_damage: 1.5,
            infection_duration: 5.0,
            infection_damage: 0.15,
            bleed_duration: 4.0,
            bleed_damage: 0.2,
            bleed_vulnerability: 1.25,
            ignite_chance: 0.25,
            ignite_duration: 4.0,
            ignite_damage: 0.2,
            boss_repeat_delay: 1.0,
            repeat_delay: 0.18,
            death_stack_cap: 5,
            extra_projectiles: 4,
        }
    }
}
impl Config {
    pub fn valid(&self) -> bool {
        [
            self.radius,
            self.rally_threshold,
            self.rally_cooldown,
            self.rally_shield,
            self.rally_duration,
            self.push_distance,
            self.death_duration,
            self.death_damage,
            self.death_action_speed,
            self.death_reduction,
            self.death_reduction_cap,
            self.guardian_interval,
            self.guardian_shield,
            self.guardian_duration,
            self.blood_interval,
            self.blood_heal,
            self.blood_copy,
            self.electric_distance,
            self.electric_radius,
            self.electric_damage,
            self.magnetic_interval,
            self.magnetic_windup,
            self.pull_distance,
            self.charge_distance,
            self.charge_length,
            self.charge_duration,
            self.charge_windup,
            self.charge_damage,
            self.wake_spacing,
            self.wake_interval,
            self.fire_duration,
            self.fire_radius,
            self.fire_damage,
            self.elusive_seconds,
            self.skin_threshold,
            self.skin_shield,
            self.skin_duration,
            self.skin_cooldown,
            self.reserve_cap,
            self.reserve_threshold,
            self.reserve_cooldown,
            self.riposte_block,
            self.block_cap,
            self.riposte_damage,
            self.riposte_duration,
            self.adaptation_duration,
            self.adaptation_taken,
            self.aegis_delay,
            self.aegis_shield,
            self.aegis_duration,
            self.bastion_per_second,
            self.bastion_cap,
            self.bastion_grace,
            self.predator_radius,
            self.predator_damage,
            self.infection_duration,
            self.infection_damage,
            self.bleed_duration,
            self.bleed_damage,
            self.bleed_vulnerability,
            self.ignite_chance,
            self.ignite_duration,
            self.ignite_damage,
            self.boss_repeat_delay,
            self.repeat_delay,
        ]
        .into_iter()
        .all(|v| v.is_finite() && v > 0.0 && v <= 1000.0)
            && self.death_stack_cap <= 20
            && self.death_stack_cap > 0
            && self.extra_projectiles <= 8
            && self.death_reduction_cap <= 0.6
            && self.block_cap <= 0.75
            && self.riposte_block <= self.block_cap
            && self.adaptation_taken <= 1.0
            && self.blood_heal <= 0.5
            && self.skin_shield <= 1.0
            && self.ignite_chance <= 1.0
    }
}
pub(super) fn config() -> &'static Config {
    &super::monster_mod_effects::get().special
}

#[derive(Component, Clone, Debug)]
pub(super) struct Runtime {
    pub shield: f32,
    pub shield_until: f32,
    pub avoided: bool,
    pub death_stacks: u32,
    pub death_until: f32,
    pub riposte_until: f32,
    pub dodge_ready: bool,
    pub adapted: Tags,
    pub adapted_until: f32,
    pub bastion: f32,
    pub charge_until: f32,
    pub healed: f32,
    last_hurt: f32,
    aegis_next: f32,
    skin_next: f32,
    rally_next: f32,
    guardian_next: f32,
    blood_next: f32,
    magnetic_next: f32,
    wake_next: f32,
    reserve_next: f32,
    reserve: f32,
    reserve_pending: bool,
    rally_pending: bool,
    last_pos: Vec3,
    last_sample: f32,
    moving: f32,
    stationary: f32,
    grace_until: f32,
    distance: f32,
    electric: f32,
    wake_distance: f32,
}
impl Runtime {
    pub fn block_chance(existing: f32) -> f32 {
        (existing + config().riposte_block).clamp(0.0, config().block_cap)
    }
    pub fn new(now: f32, pos: Vec3) -> Self {
        let c = config();
        Self {
            shield: 0.0,
            shield_until: 0.0,
            avoided: false,
            death_stacks: 0,
            death_until: 0.0,
            riposte_until: 0.0,
            dodge_ready: false,
            adapted: Tags::default(),
            adapted_until: 0.0,
            bastion: 0.0,
            charge_until: 0.0,
            healed: 0.0,
            last_hurt: now,
            aegis_next: now + c.aegis_delay,
            skin_next: now,
            rally_next: now,
            guardian_next: now + c.guardian_interval,
            blood_next: now + c.blood_interval,
            magnetic_next: now + c.magnetic_interval,
            wake_next: now,
            reserve_next: now,
            reserve: 0.0,
            reserve_pending: false,
            rally_pending: false,
            last_pos: pos,
            last_sample: now,
            moving: 0.0,
            stationary: 0.0,
            grace_until: 0.0,
            distance: 0.0,
            electric: 0.0,
            wake_distance: 0.0,
        }
    }
    pub fn protect(&mut self, amount: f32, until: f32) {
        // One shared absorption pool; stronger existing protection is never replaced by less.
        self.shield = self.shield.max(amount);
        self.shield_until = self.shield_until.max(until);
    }
    pub fn reduction(&self, rank: Monster, tags: Tags, now: f32) -> f32 {
        let c = config();
        let death = if rank.has(Monster::DeathAvenger) && now < self.death_until {
            (self.death_stacks as f32 * c.death_reduction).min(c.death_reduction_cap)
        } else {
            0.0
        };
        let adaptive = if rank.has(Monster::Adaptive)
            && now < self.adapted_until
            && self.adapted.intersects(tags)
        {
            c.adaptation_taken
        } else {
            1.0
        };
        (1.0 - death) * adaptive
    }
    pub fn armour(&self, rank: Monster) -> f32 {
        1.0 + if rank.has(Monster::Bastion) {
            self.bastion
        } else {
            0.0
        }
    }
    pub fn damage(&self, now: f32) -> f32 {
        1.0 + if now < self.death_until {
            self.death_stacks as f32 * config().death_damage
        } else {
            0.0
        }
    }
    pub fn action_speed(&self, now: f32) -> f32 {
        1.0 + if now < self.death_until {
            self.death_stacks as f32 * config().death_action_speed
        } else {
            0.0
        }
    }
    pub fn take_riposte(&mut self, now: f32) -> f32 {
        if self.riposte_until > now {
            self.riposte_until = 0.0;
            config().riposte_damage
        } else {
            1.0
        }
    }
    pub fn forecast_uncertain(&self, rank: Monster) -> bool {
        self.shield > 0.0
            || rank.has(Monster::Adaptive)
            || rank.has(Monster::DeathAvenger)
            || rank.has(Monster::SecondSkin)
            || rank.has(Monster::Aegis)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn defend(&mut self, rank: Monster, raw: f32, tags: Tags, now: f32, roll: f32) -> f32 {
        self.avoided = false;
        if raw <= 0.0 {
            return 0.0;
        }
        let direct = !tags.contains(Tags::DOT);
        if direct && self.dodge_ready {
            self.dodge_ready = false;
            self.moving = 0.0;
            self.avoided = true;
            return 0.0;
        }
        if direct
            && rank.has(Monster::Riposte)
            && tags.contains(Tags::ATTACK)
            && roll < Self::block_chance(0.0)
        {
            self.riposte_until = now + config().riposte_duration;
            self.avoided = true;
            return 0.0;
        }
        let raw = raw * self.reduction(rank, tags, now);
        if now >= self.shield_until {
            self.shield = 0.0;
        }
        let absorbed = raw.min(self.shield.max(0.0));
        self.shield -= absorbed;
        (raw - absorbed).max(0.0)
    }
    pub fn hurt(&mut self, rank: Monster, amount: f32, hp: f32, max_hp: f32, tags: Tags, now: f32) {
        if amount <= 0.0 {
            return;
        }
        let c = config();
        self.last_hurt = now;
        self.aegis_next = now + c.aegis_delay;
        if rank.has(Monster::Adaptive) {
            self.adapted = Tags(tags.0 & (0xff << 16));
            self.adapted_until = now + c.adaptation_duration;
        }
        if rank.has(Monster::Avenger) {
            self.reserve = (self.reserve + amount.min(hp)).min(max_hp * c.reserve_cap);
            if self.reserve >= max_hp * c.reserve_threshold && now >= self.reserve_next {
                self.reserve_pending = true;
            }
        }
        let after = hp - amount;
        if after > 0.0
            && rank.has(Monster::SecondSkin)
            && hp > max_hp * c.skin_threshold
            && after <= max_hp * c.skin_threshold
            && now >= self.skin_next
        {
            self.protect(max_hp * c.skin_shield, now + c.skin_duration);
            self.skin_next = now + c.skin_cooldown;
        }
        if after > 0.0
            && rank.has(Monster::Rally)
            && hp > max_hp * c.rally_threshold
            && after <= max_hp * c.rally_threshold
            && now >= self.rally_next
        {
            self.rally_pending = true;
            self.rally_next = now + c.rally_cooldown;
        }
    }
}
pub(super) fn heal(
    enemy: &mut Enemy,
    runtime: Option<&mut Runtime>,
    amount: f32,
    copy: bool,
) -> f32 {
    if enemy.dead || enemy.hp <= 0.0 {
        return 0.0;
    }
    let restored = amount.max(0.0).min((enemy.max_hp - enemy.hp).max(0.0));
    enemy.hp += restored;
    if copy {
        if let Some(runtime) = runtime {
            runtime.healed += restored;
        }
    }
    restored
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AilmentKind {
    Infection,
    Bleed,
    Ignite,
}
#[derive(Clone, Debug)]
struct Ailment {
    source: Entity,
    kind: AilmentKind,
    left: f32,
    tick: f32,
    dps: f32,
    bonuses: Bonuses,
}
#[derive(Clone)]
pub(super) struct PlayerState {
    ailments: Vec<Ailment>,
    pub isolated: bool,
    death_sent: bool,
}
impl Default for PlayerState {
    fn default() -> Self {
        Self {
            ailments: Vec::new(),
            isolated: true,
            death_sent: false,
        }
    }
}
impl PlayerState {
    pub fn active(&self) -> Vec<(u32, f32)> {
        self.ailments
            .iter()
            .map(|a| {
                (
                    match a.kind {
                        AilmentKind::Infection => 2818,
                        AilmentKind::Bleed => 1943,
                        AilmentKind::Ignite => 133,
                    },
                    a.left,
                )
            })
            .collect()
    }
    fn bleeding(&self) -> bool {
        self.ailments
            .iter()
            .any(|a| a.kind == AilmentKind::Bleed && a.left > 0.0)
    }
    fn apply(&mut self, ailment: Ailment) {
        if let Some(old) = self
            .ailments
            .iter_mut()
            .find(|a| a.source == ailment.source && a.kind == ailment.kind)
        {
            old.left = old.left.max(ailment.left);
            old.dps = old.dps.max(ailment.dps);
        } else {
            if self.ailments.len() >= 64 {
                self.ailments.remove(0);
            }
            self.ailments.push(ailment);
        }
    }
}
pub(super) fn outgoing(run: &Run, rank: Monster, tags: Tags) -> f32 {
    let c = config();
    (if rank.has(Monster::Predator) && run.monster_ailments.isolated {
        c.predator_damage
    } else {
        1.0
    }) * if tags.contains(Tags::MELEE) && run.monster_ailments.bleeding() {
        c.bleed_vulnerability
    } else {
        1.0
    }
}
pub(super) fn inflicted(run: &mut Run, raw: f32, dealt: f32, tags: Tags, bonuses: Bonuses) {
    if dealt <= 0.0 || tags.contains(Tags::DOT) {
        return;
    }
    let Some(source) = run.legendary.events.attacker else {
        return;
    };
    let c = config();
    let rank = bonuses.0;
    for (kind, enabled, duration, damage) in [
        (
            AilmentKind::Infection,
            rank.has(Monster::Plagued),
            c.infection_duration,
            c.infection_damage,
        ),
        (
            AilmentKind::Bleed,
            rank.has(Monster::Hemorrhagic) && tags.contains(Tags::PHYSICAL),
            c.bleed_duration,
            c.bleed_damage,
        ),
        (
            AilmentKind::Ignite,
            rank.has(Monster::Incendiary) && run.extra_spells.dodge_rng.f32() < c.ignite_chance,
            c.ignite_duration,
            c.ignite_damage,
        ),
    ] {
        if enabled {
            run.monster_ailments.apply(Ailment {
                source,
                kind,
                left: duration,
                tick: 1.0,
                dps: raw * damage,
                bonuses,
            });
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Kind {
    Electric,
    PhysicalWave,
    Pull,
    Push,
    Fire,
    Charge,
    Repeat,
}
#[derive(Component)]
pub(super) struct Hazard {
    source: Entity,
    kind: Kind,
    warning: super::boss_spells::Warning,
    raw: f32,
    bonuses: Bonuses,
    duration: f32,
    next_tick: f32,
    target: Option<(Option<u64>, Entity)>,
    hit: bool,
    delay: f32,
    critical: bool,
}
fn hazard(
    source: Entity,
    kind: Kind,
    pos: Vec3,
    radius: f32,
    delay: f32,
    duration: f32,
    raw: f32,
    rank: Monster,
) -> Hazard {
    Hazard {
        source,
        kind,
        raw,
        bonuses: Bonuses(rank),
        duration,
        next_tick: delay,
        target: None,
        hit: false,
        delay,
        critical: false,
        warning: super::boss_spells::Warning {
            spell: super::boss_spells::Spell::Mortars,
            origin: pos.to_array(),
            direction: [0.0; 3],
            mortars: vec![],
            elapsed: 0.0,
            circle: Some([radius, delay.max(0.001)]),
            zones: vec![],
            windup_seconds: None,
            spatial_scale: Bonuses(rank).area_radius(),
        },
    }
}
#[allow(clippy::too_many_arguments)]
pub(super) fn repeat(
    commands: &mut Commands,
    source: Entity,
    target: Entity,
    owner: Option<u64>,
    pos: Vec3,
    rank: Monster,
    raw: f32,
    critical: bool,
) {
    let mut repeat = hazard(
        source,
        Kind::Repeat,
        pos,
        0.0,
        config().repeat_delay,
        0.0,
        raw,
        rank,
    );
    repeat.target = Some((owner, target));
    repeat.critical = critical;
    commands.spawn((super::RunEntity, repeat));
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn hazards(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut rng: ResMut<super::Rng>,
    world: super::geometry::Surface,
    mut companions: Option<ResMut<super::multiplayer::Companions>>,
    mut hazards: Query<(Entity, &mut Hazard)>,
    mut enemies: Query<(
        &mut Transform,
        &Enemy,
        Option<&Runtime>,
        Option<&mut crate::creature_anim::MovementState>,
    )>,
    mut players: Query<
        (
            Entity,
            &mut Transform,
            Option<&super::multiplayer::RemoteOwner>,
            Has<super::hero::Hero>,
        ),
        (Without<Enemy>, Or<(With<super::hero::Hero>, With<super::multiplayer::RemoteOwner>)>),
    >,
    mut warnings: ResMut<super::boss_spells::Warnings>,
    mut fx: super::units::Fx,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let c = config();
    for (entity, mut h) in &mut hazards {
        let source_alive = enemies
            .get(h.source)
            .is_ok_and(|(_, e, _, _)| !e.dead && e.hp > 0.0 && e.stun_t <= 0.0 && e.fear_t <= 0.0);
        if !source_alive && !matches!(h.kind, Kind::Fire) {
            commands.entity(entity).despawn();
            continue;
        }
        h.warning.elapsed += dt;
        if !matches!(h.kind, Kind::Repeat) {
            warnings.0.push(h.warning.clone());
        }
        if h.warning.elapsed < h.delay {
            continue;
        }
        let charge = matches!(h.kind, Kind::Charge);
        let mut source_pos = Vec3::from_array(h.warning.origin);
        if let Ok((mut tf, _, _, movement)) = enemies.get_mut(h.source) {
            if charge {
                let progress =
                    ((h.warning.elapsed - h.delay) / h.duration.max(0.001)).clamp(0.0, 1.0);
                let desired =
                    source_pos + Vec3::from_array(h.warning.direction) * c.charge_length * progress;
                let desired = world.constrain(tf.translation, desired);
                tf.translation = super::enemies::ground(&world, desired, tf.translation.y);
                tf.rotation = Quat::from_rotation_y(super::units::yaw_toward(Vec3::from_array(
                    h.warning.direction,
                )));
                if let Some(mut movement) = movement {
                    movement.speed = if progress < 1.0 {
                        c.charge_length / h.duration
                    } else {
                        0.0
                    };
                    movement.flags = if progress < 1.0 {
                        crate::creature_anim::move_flags::FORWARD
                    } else {
                        0
                    };
                }
            }
            source_pos = tf.translation;
        }
        if charge && h.warning.elapsed < h.delay + h.duration {
            continue;
        }
        let pulses = if matches!(h.kind, Kind::Fire) {
            let through = h.warning.elapsed.min(h.delay + h.duration - 0.0001);
            if through < h.next_tick {
                0
            } else {
                ((through - h.next_tick).floor() as u32 + 1).min(16)
            }
        } else {
            u32::from(!h.hit)
        };
        if pulses > 0 {
            h.next_tick += pulses as f32;
            h.hit = true;
            for (player, mut tf, owner, local) in &mut players {
                let owner = if local { None } else {
                    let Some(owner) = owner else { continue; };
                    Some(owner.0)
                };
                if let Some((expected_owner, expected)) = h.target {
                    if expected != player || expected_owner != owner {
                        continue;
                    }
                }
                let Some(target) =
                    super::multiplayer::player_run(&mut run, companions.as_deref_mut(), owner)
                else {
                    continue;
                };
                if target.hp <= 0.0 || target.dead_t.is_some() || target.pending_levels > 0 {
                    continue;
                }
                let melee = matches!(h.kind, Kind::Charge | Kind::Repeat);
                if melee {
                    let reach = enemies.get(h.source).map_or(0.0, |(_, e, _, _)| {
                        super::spell_engine::targeting::melee_contact_range(e.radius)
                            * h.bonuses.range()
                    });
                    if !near(source_pos, tf.translation, reach) {
                        continue;
                    }
                } else if !h.warning.contains(tf.translation, super::hero::HERO_RADIUS) {
                    continue;
                }
                if matches!(h.kind, Kind::Pull | Kind::Push) {
                    let center = Vec3::from_array(h.warning.origin);
                    let distance = super::units::flat(center - tf.translation).length();
                    let direction = super::units::flat(center - tf.translation).normalize_or_zero();
                    let offset = if matches!(h.kind, Kind::Pull) {
                        direction * c.pull_distance.min((distance - 1.0).max(0.0))
                    } else {
                        -direction * c.push_distance
                    };
                    let desired = world.constrain(tf.translation, tf.translation + offset);
                    tf.translation = super::enemies::ground(&world, desired, tf.translation.y);
                    continue;
                }
                let tags = match h.kind {
                    Kind::Fire => Tags::FIRE | Tags::DOT | Tags::AREA | Tags::SPELL,
                    Kind::PhysicalWave => Tags::PHYSICAL | Tags::AREA | Tags::SPELL,
                    Kind::Charge | Kind::Repeat => Tags::PHYSICAL | Tags::MELEE | Tags::ATTACK,
                    _ => Tags::LIGHTNING | Tags::NATURE | Tags::AREA | Tags::SPELL,
                };
                target.legendary.events.attacker = Some(h.source);
                target.taken_source = match h.kind {
                    Kind::Fire => "Monster: burning ground",
                    Kind::Electric => "Monster: electrostatic nova",
                    Kind::PhysicalWave => "Monster: vengeance wave",
                    Kind::Charge => "Monster: charge",
                    _ => "Monster: echo",
                };
                let dealt = if melee {
                    super::monster_stats::deal(
                        target,
                        h.raw,
                        tags,
                        h.bonuses,
                        h.critical,
                        Some(rng.f32()),
                    )
                } else {
                    super::monster_stats::deal_secondary(
                        target,
                        h.raw * pulses as f32,
                        tags,
                        h.bonuses,
                    )
                };
                target.legendary.events.attacker = None;
                if melee {
                    fx.swing(h.source, player, dealt);
                } else {
                    fx.impact(
                        player,
                        if matches!(h.kind, Kind::Fire) {
                            13812
                        } else {
                            8042
                        },
                    );
                }
                if dealt > 0.0 {
                    fx.number(
                        target.owner,
                        player,
                        dealt,
                        h.critical,
                        super::units::COLOR_HURT,
                    );
                }
                if !tags.contains(Tags::DOT) {
                    super::monster_stats::leech(&mut commands, h.source, dealt, h.bonuses);
                }
            }
        }
        if (!matches!(h.kind, Kind::Fire) && h.hit) || h.warning.elapsed >= h.delay + h.duration {
            commands.entity(entity).despawn();
        }
    }
}

#[derive(Clone)]
struct Ally {
    id: Entity,
    pos: Vec3,
    hp: f32,
    max: f32,
    dead: bool,
    shield: f32,
}
fn near(a: Vec3, b: Vec3, radius: f32) -> bool {
    super::units::flat(a - b).length_squared() <= radius * radius
}
fn player_ticks(run: &mut Run, dt: f32) -> Vec<Ailment> {
    let mut state = std::mem::take(&mut run.monster_ailments);
    if run.hp > 0.0 && run.dead_t.is_none() {
        state.death_sent = false;
        for ailment in &mut state.ailments {
            let elapsed = dt.min(ailment.left.max(0.0));
            ailment.left -= dt;
            ailment.tick -= elapsed;
            while ailment.tick <= 0.0 && elapsed > 0.0 {
                ailment.tick += 1.0;
                let tags = match ailment.kind {
                    AilmentKind::Infection => Tags::NATURE,
                    AilmentKind::Bleed => Tags::PHYSICAL,
                    AilmentKind::Ignite => Tags::FIRE,
                } | Tags::DOT;
                run.legendary.events.attacker = Some(ailment.source);
                run.taken_source = match ailment.kind {
                    AilmentKind::Infection => "Monster: infection",
                    AilmentKind::Bleed => "Monster: bleeding",
                    AilmentKind::Ignite => "Monster: ignite",
                };
                let damage = ailment.bonuses.packets(ailment.dps, tags, false)[0].0;
                let previous_source = run.death_log.begin(Some(ailment.source), ailment.bonuses.0, false, run.taken_source);
                super::hero::hurt_hero_tagged(run, damage, tags);
                run.death_log.context = previous_source;
                run.legendary.events.attacker = None;
            }
        }
    }
    state.ailments.retain(|a| a.left > 0.0);
    let death = if run.hp <= 0.0 && !state.death_sent {
        state.death_sent = true;
        state.ailments.drain(..).collect()
    } else {
        Vec::new()
    };
    run.monster_ailments = state;
    death
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn prepare(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut companions: Option<ResMut<super::multiplayer::Companions>>,
    mut enemies: Query<(Entity, &Transform, &mut Enemy, &Monster, &mut Runtime)>,
    players: Query<
        (
            Entity,
            &Transform,
            Option<&super::multiplayer::RemoteOwner>,
            Has<super::hero::Hero>,
        ),
        (Without<Enemy>, Or<(With<super::hero::Hero>, With<super::multiplayer::RemoteOwner>)>),
    >,
    hazards: Query<(), With<Hazard>>,
    mut seen: Local<std::collections::HashSet<Entity>>,
    mut fx: super::units::Fx,
) {
    let c = config();
    let now = run.elapsed;
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let allies: Vec<_> = enemies
        .iter()
        .map(|(id, tf, e, _, state)| Ally {
            id,
            pos: tf.translation,
            hp: e.hp,
            max: e.max_hp,
            dead: e.dead || e.hp <= 0.0,
            shield: if now < state.shield_until {
                state.shield
            } else {
                0.0
            },
        })
        .collect();
    seen.retain(|id| enemies.contains(*id));
    let deaths: Vec<_> = allies
        .iter()
        .filter(|a| a.dead && seen.insert(a.id))
        .map(|a| (a.id, a.pos))
        .collect();
    let player_positions: Vec<_> = players
        .iter()
        .filter_map(|(id, tf, owner, local)| {
            let owner = if local { None } else { Some(owner?.0) };
            let alive = if local {
                run.hp > 0.0 && run.dead_t.is_none()
            } else {
                companions
                    .as_ref()
                    .and_then(|p| p.0.get(&owner?))
                    .is_some_and(|p| p.run.hp > 0.0 && p.run.dead_t.is_none())
            };
            alive.then_some((owner, id, tf.translation))
        })
        .collect();
    let mut budget = 512usize.saturating_sub(hazards.iter().count());
    let mut emit = |hazard: Hazard| {
        if budget > 0 {
            budget -= 1;
            commands.spawn((super::RunEntity, hazard));
            true
        } else {
            false
        }
    };
    let mut heals = Vec::new();
    let mut shields = Vec::new();
    for (id, tf, enemy, rank, mut state) in &mut enemies {
        if enemy.dead || enemy.hp <= 0.0 || super::training::actor(enemy.def) {
            continue;
        }
        if now >= state.shield_until {
            state.shield = 0.0;
        }
        if now >= state.death_until {
            state.death_stacks = 0;
        }
        if rank.has(Monster::DeathAvenger) {
            let count = deaths
                .iter()
                .filter(|(source, pos)| *source != id && near(*pos, tf.translation, c.radius))
                .count() as u32;
            if count > 0 {
                state.death_stacks = state
                    .death_stacks
                    .saturating_add(count)
                    .min(c.death_stack_cap);
                state.death_until = now + c.death_duration;
            }
        }
        let travel = super::units::flat(tf.translation - state.last_pos).length();
        let walking = travel > 0.01
            && travel <= (enemy.speed * dt * 3.0).max(8.0)
            && !(state.charge_until > 0.0 && state.last_sample <= state.charge_until);
        state.last_pos = tf.translation;
        state.last_sample = now;
        if walking {
            if state.stationary > 0.0 {
                state.grace_until = now + c.bastion_grace;
            }
            state.stationary = 0.0;
            state.moving += dt;
            state.distance = (state.distance + travel).min(c.charge_distance);
            state.electric = (state.electric + travel).min(c.electric_distance);
            state.wake_distance += travel;
            if rank.has(Monster::Elusive) && state.moving >= c.elusive_seconds {
                state.dodge_ready = true;
            }
            if now > state.grace_until {
                state.bastion = (state.bastion - dt * c.bastion_per_second).max(0.0);
            }
            if rank.has(Monster::BurningWake)
                && state.wake_distance >= c.wake_spacing
                && now >= state.wake_next
            {
                if emit(hazard(
                    id,
                    Kind::Fire,
                    tf.translation,
                    c.fire_radius,
                    0.25,
                    c.fire_duration,
                    enemy.damage * enemy.weaken * state.damage(now) * c.fire_damage,
                    *rank,
                )) {
                    state.wake_distance = 0.0;
                    state.wake_next = now + c.wake_interval;
                }
            }
        } else if travel <= 0.01 {
            state.moving = 0.0;
            state.stationary += dt;
            if rank.has(Monster::Bastion) {
                state.bastion = (state.bastion + dt * c.bastion_per_second).min(c.bastion_cap);
            }
            if rank.has(Monster::Electrostatic) && state.electric >= c.electric_distance {
                if emit(hazard(
                    id,
                    Kind::Electric,
                    tf.translation,
                    c.electric_radius,
                    0.6,
                    0.0,
                    enemy.damage * enemy.weaken * state.damage(now) * c.electric_damage,
                    *rank,
                )) {
                    state.electric = 0.0;
                }
            }
        } else {
            // Teleports and forced charges neither count as walking nor as standing still.
            if state.stationary > 0.0 {
                state.grace_until = now + c.bastion_grace;
            }
            state.stationary = 0.0;
            state.moving = 0.0;
            if now > state.grace_until {
                state.bastion = (state.bastion - dt * c.bastion_per_second).max(0.0);
            }
        }
        if rank.has(Monster::Aegis) && now >= state.aegis_next && state.shield <= 0.0 {
            state.protect(enemy.max_hp * c.aegis_shield, now + c.aegis_duration);
            state.aegis_next = now + c.aegis_delay;
            fx.impact(id, 17);
        }
        if state.rally_pending {
            state.rally_pending = false;
            for ally in allies
                .iter()
                .filter(|a| !a.dead && near(a.pos, tf.translation, c.radius))
            {
                shields.push((
                    ally.id,
                    enemy.max_hp * c.rally_shield,
                    now + c.rally_duration,
                ));
            }
            emit(hazard(
                id,
                Kind::Push,
                tf.translation,
                c.radius,
                0.3,
                0.0,
                0.0,
                *rank,
            ));
        }
        if state.reserve_pending {
            if emit(hazard(
                id,
                Kind::PhysicalWave,
                tf.translation,
                c.electric_radius,
                0.6,
                0.0,
                state.reserve,
                *rank,
            )) {
                state.reserve = 0.0;
                state.reserve_pending = false;
                state.reserve_next = now + c.reserve_cooldown;
            }
        }
        if rank.has(Monster::Guardian) && now >= state.guardian_next {
            if let Some(ally) = allies
                .iter()
                .filter(|a| {
                    a.id != id
                        && !a.dead
                        && a.shield <= 0.0
                        && near(a.pos, tf.translation, c.radius)
                })
                .min_by(|a, b| (a.hp / a.max).total_cmp(&(b.hp / b.max)))
            {
                shields.push((
                    ally.id,
                    enemy.max_hp * c.guardian_shield,
                    now + c.guardian_duration,
                ));
                state.guardian_next = now + c.guardian_interval;
            }
        }
        let received = std::mem::take(&mut state.healed);
        if rank.has(Monster::BloodLink) {
            if received > 0.0 {
                if let Some(ally) = allies
                    .iter()
                    .filter(|a| {
                        a.id != id
                            && !a.dead
                            && a.hp < a.max
                            && near(a.pos, tf.translation, c.radius)
                    })
                    .min_by(|a, b| (a.hp / a.max).total_cmp(&(b.hp / b.max)))
                {
                    heals.push((id, ally.id, received * c.blood_copy, false));
                }
            }
            if now >= state.blood_next {
                for ally in allies
                    .iter()
                    .filter(|a| !a.dead && near(a.pos, tf.translation, c.radius))
                {
                    heals.push((id, ally.id, enemy.max_hp * c.blood_heal, true));
                }
                state.blood_next = now + c.blood_interval;
            }
        }
        if rank.has(Monster::Magnetic) && now >= state.magnetic_next {
            if emit(hazard(
                id,
                Kind::Pull,
                tf.translation,
                c.radius,
                c.magnetic_windup,
                0.0,
                0.0,
                *rank,
            )) {
                state.magnetic_next = now + c.magnetic_interval;
            }
        }
        if rank.has(Monster::Charger)
            && state.distance >= c.charge_distance
            && now >= state.charge_until
        {
            if let Some((owner, target, pos)) = player_positions
                .iter()
                .filter(|(_, _, pos)| near(*pos, tf.translation, c.charge_length + 3.0))
                .min_by(|a, b| {
                    a.2.distance_squared(tf.translation)
                        .total_cmp(&b.2.distance_squared(tf.translation))
                })
            {
                let mut h = hazard(
                    id,
                    Kind::Charge,
                    tf.translation,
                    0.0,
                    c.charge_windup,
                    c.charge_duration,
                    enemy.damage * enemy.weaken * state.damage(now) * c.charge_damage,
                    *rank,
                );
                h.target = Some((*owner, *target));
                h.warning.spell = super::boss_spells::Spell::Charge;
                h.warning.circle = None;
                h.warning.windup_seconds = Some(c.charge_windup);
                h.warning.direction = super::units::flat(*pos - tf.translation)
                    .normalize_or_zero()
                    .to_array();
                h.warning.spatial_scale = c.charge_length / 24.0;
                if emit(h) {
                    state.distance = 0.0;
                    state.charge_until = now + c.charge_windup + c.charge_duration;
                }
            }
        }
    }
    for (source, target, amount, copy) in heals {
        if let Ok((_, _, mut enemy, _, mut state)) = enemies.get_mut(target) {
            if heal(&mut enemy, Some(&mut state), amount, copy) > 0.0 {
                fx.cast(source, 635, &[target], None);
            }
        }
    }
    for (target, amount, until) in shields {
        if let Ok((_, _, _, _, mut state)) = enemies.get_mut(target) {
            state.protect(amount, until);
            fx.impact(target, 17);
        }
    }
    let mut deaths = Vec::new();
    for (id, tf, owner, local) in &players {
        let owner = if local { None } else {
            let Some(owner) = owner else { continue; };
            Some(owner.0)
        };
        let Some(player) =
            super::multiplayer::player_run(&mut run, companions.as_deref_mut(), owner)
        else {
            continue;
        };
        for ailment in player_ticks(player, dt) {
            deaths.push((id, tf.translation, ailment));
        }
    }
    // All periodic damage resolves before choosing live neighbours or isolation.
    let living: Vec<_> = player_positions
        .iter()
        .copied()
        .filter(|(owner, _, _)| {
            super::multiplayer::player_run(&mut run, companions.as_deref_mut(), *owner)
                .is_some_and(|p| p.hp > 0.0 && p.dead_t.is_none())
        })
        .collect();
    for (owner, id, pos) in &living {
        if let Some(player) =
            super::multiplayer::player_run(&mut run, companions.as_deref_mut(), *owner)
        {
            player.monster_ailments.isolated = !living.iter().any(|(_, other, other_pos)| {
                *other != *id && near(*other_pos, *pos, c.predator_radius)
            });
        }
    }
    let mut transfers = Vec::new();
    for (id, pos, ailment) in deaths {
        if ailment.kind == AilmentKind::Infection {
            if let Some((owner, _, _)) = living
                .iter()
                .filter(|(_, other, other_pos)| *other != id && near(*other_pos, pos, c.radius))
                .min_by(|a, b| {
                    a.2.distance_squared(pos)
                        .total_cmp(&b.2.distance_squared(pos))
                })
            {
                transfers.push((*owner, ailment));
            }
        } else if ailment.kind == AilmentKind::Ignite {
            emit(hazard(
                ailment.source,
                Kind::Fire,
                pos,
                c.fire_radius,
                0.25,
                c.fire_duration,
                ailment.dps,
                ailment.bonuses.0,
            ));
        }
    }
    for (owner, ailment) in transfers {
        if let Some(player) =
            super::multiplayer::player_run(&mut run, companions.as_deref_mut(), owner)
        {
            player.monster_ailments.apply(ailment);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn infection_transfers_remaining_duration_to_a_live_neighbour_and_ignite_leaves_one_fire() {
        use super::super::multiplayer::{Companion, Companions, RemoteOwner};
        let mut world = arena();
        let source = spawn(&mut world, Monster::Plagued, Vec3::ZERO, 100.0);
        world.spawn((
            super::super::hero::Hero { vy: 0.0, air: 0.0 },
            Transform::default(),
        ));
        let mut companions = Companions::default();
        for (owner, x, hp) in [(1, 1.0, 0.0), (2, 2.0, 100.0)] {
            let entity = world
                .spawn((RemoteOwner(owner), Transform::from_xyz(x, 0.0, 0.0)))
                .id();
            companions.0.insert(
                owner,
                Companion {
                    entity,
                    run: Run {
                        owner: Some(owner),
                        hp,
                        ..default()
                    },
                    physics: super::super::hero::Hero { vy: 0.0, air: 0.0 },
                    offer: default(),
                    offer_id: 0,
                },
            );
        }
        world.insert_resource(companions);
        {
            let mut run = world.resource_mut::<Run>();
            run.hp = 0.0;
            for kind in [AilmentKind::Infection, AilmentKind::Ignite] {
                run.monster_ailments.apply(Ailment {
                    source,
                    kind,
                    left: 2.25,
                    tick: 0.7,
                    dps: 3.0,
                    bonuses: Bonuses(Monster::Plagued),
                });
            }
        }
        step(&mut world, 0.1);
        let players = world.resource::<Companions>();
        assert!(players.0[&1].run.monster_ailments.ailments.is_empty());
        assert_eq!(players.0[&2].run.monster_ailments.ailments.len(), 1);
        assert_eq!(players.0[&2].run.monster_ailments.ailments[0].left, 2.25);
        assert_eq!(players.0[&2].run.monster_ailments.ailments[0].tick, 0.7);
        assert_eq!(
            world
                .query::<&Hazard>()
                .iter(&world)
                .filter(|h| matches!(h.kind, Kind::Fire))
                .count(),
            1
        );
        step(&mut world, 0.1);
        assert_eq!(
            world
                .query::<&Hazard>()
                .iter(&world)
                .filter(|h| matches!(h.kind, Kind::Fire))
                .count(),
            1
        );
    }
    #[test]
    fn charger_warns_moves_then_hits_and_forced_travel_does_not_recharge() {
        let mut world = arena();
        world.resource_mut::<Run>().hp = 1000.0;
        world.spawn((
            super::super::hero::Hero { vy: 0.0, air: 0.0 },
            Transform::from_xyz(6.0, 0.0, 0.0),
        ));
        let source = spawn(&mut world, Monster::Charger, Vec3::ZERO, 100.0);
        world.get_mut::<Runtime>(source).unwrap().distance = config().charge_distance;
        step(&mut world, 0.1);
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(0.1));
        world.run_system_once(hazards).unwrap();
        assert_eq!(world.get::<Transform>(source).unwrap().translation.x, 0.0);
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(0.75));
        world.resource_mut::<Run>().elapsed += 0.85;
        world.run_system_once(hazards).unwrap();
        assert!((world.get::<Transform>(source).unwrap().translation.x - 6.0).abs() < 0.01);
        assert!(world.resource::<Run>().hp < 1000.0);
        step(&mut world, 0.1);
        assert_eq!(world.get::<Runtime>(source).unwrap().distance, 0.0);
        assert_eq!(world.query::<&Hazard>().iter(&world).count(), 0);
    }
    #[derive(Resource)]
    struct Prepared(bevy::ecs::system::SystemId);
    #[test]
    fn magnetic_warns_before_pulling_and_echo_melee_does_not_repeat_itself() {
        let mut world = arena();
        world.resource_mut::<Run>().hp = 1000.0;
        let player = world
            .spawn((
                super::super::hero::Hero { vy: 0.0, air: 0.0 },
                Transform::from_xyz(5.0, 0.0, 0.0),
            ))
            .id();
        let source = spawn(&mut world, Monster::Magnetic, Vec3::ZERO, 100.0);
        step(&mut world, 8.0);
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(0.1));
        world.run_system_once(hazards).unwrap();
        assert_eq!(world.get::<Transform>(player).unwrap().translation.x, 5.0);
        assert!(
            !world
                .resource::<super::super::boss_spells::Warnings>()
                .0
                .is_empty()
        );
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(1.0));
        world.run_system_once(hazards).unwrap();
        assert!((world.get::<Transform>(player).unwrap().translation.x - 2.0).abs() < 0.01);
        world.get_mut::<Transform>(player).unwrap().translation.x = 1.0;
        world
            .run_system_once(move |mut commands: Commands| {
                repeat(
                    &mut commands,
                    source,
                    player,
                    None,
                    Vec3::ZERO,
                    Monster::Echo,
                    20.0,
                    false,
                )
            })
            .unwrap();
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(0.2));
        world.run_system_once(hazards).unwrap();
        let hp = world.resource::<Run>().hp;
        assert!(hp < 1000.0);
        assert_eq!(world.query::<&Hazard>().iter(&world).count(), 0);
        world.run_system_once(hazards).unwrap();
        assert_eq!(world.resource::<Run>().hp, hp);
    }
    #[test]
    fn rally_protects_allies_and_actual_damage_releases_one_capped_wave() {
        let mut world = arena();
        let source = spawn(
            &mut world,
            Monster::Rally.with(Monster::Avenger),
            Vec3::ZERO,
            100.0,
        );
        let ally = spawn(&mut world, Monster::Normal, Vec3::X, 10000.0);
        world.get_mut::<Runtime>(source).unwrap().hurt(
            Monster::Rally.with(Monster::Avenger),
            30.0,
            60.0,
            100.0,
            Tags::FIRE,
            0.0,
        );
        world.get_mut::<Enemy>(source).unwrap().hp = 30.0;
        step(&mut world, 0.1);
        assert_eq!(world.get::<Runtime>(source).unwrap().shield, 20.0);
        assert_eq!(world.get::<Runtime>(ally).unwrap().shield, 20.0);
        let waves: Vec<_> = world
            .query::<&Hazard>()
            .iter(&world)
            .filter(|h| matches!(h.kind, Kind::PhysicalWave))
            .collect();
        assert_eq!(waves.len(), 1);
        assert_eq!(waves[0].raw, 30.0);
        step(&mut world, 0.1);
        assert_eq!(
            world
                .query::<&Hazard>()
                .iter(&world)
                .filter(|h| matches!(h.kind, Kind::PhysicalWave))
                .count(),
            1
        );
    }
    #[test]
    fn ailments_tick_in_their_school_and_bleeding_only_amplifies_melee() {
        let mut run = Run {
            hp: 1000.0,
            ..default()
        };
        let source = Entity::from_raw_u32(1).unwrap();
        run.legendary.events.attacker = Some(source);
        inflicted(
            &mut run,
            100.0,
            100.0,
            Tags::PHYSICAL | Tags::MELEE | Tags::ATTACK,
            Bonuses(Monster::Plagued.with(Monster::Hemorrhagic)),
        );
        assert_eq!(run.monster_ailments.ailments.len(), 2);
        assert!((outgoing(&run, Monster::Normal, Tags::MELEE) - 1.25).abs() < 0.001);
        assert_eq!(outgoing(&run, Monster::Normal, Tags::SPELL), 1.0);
        assert_eq!(outgoing(&run, Monster::Predator, Tags::SPELL), 1.5);
        run.monster_ailments.isolated = false;
        assert_eq!(outgoing(&run, Monster::Predator, Tags::SPELL), 1.0);
        player_ticks(&mut run, 1.0);
        assert!(run.hp < 1000.0);
        let after_tick = run.hp;
        player_ticks(&mut run, 0.1);
        assert_eq!(run.hp, after_tick);
        run.hp = 0.0;
        let death = player_ticks(&mut run, 0.1);
        assert_eq!(death.len(), 2);
        assert!((death[0].left - (config().infection_duration - 1.1)).abs() < 0.001);
        assert!(player_ticks(&mut run, 0.1).is_empty());
    }
    use crate::{
        combat_text::CombatTextSpawn,
        creature_anim::{CastEvent, PlaySeq, SpellGoTargets, SwingMessage},
    };
    use bevy::ecs::system::RunSystemOnce;
    fn arena() -> World {
        let mut world = World::new();
        super::super::geometry::init_test_world(&mut world);
        world.init_resource::<Time>();
        world.init_resource::<Run>();
        world.init_resource::<super::super::Rng>();
        world.init_resource::<super::super::boss_spells::Warnings>();
        world.init_resource::<PlaySeq>();
        world.init_resource::<Messages<CastEvent>>();
        world.init_resource::<Messages<SpellGoTargets>>();
        world.init_resource::<Messages<SwingMessage>>();
        world.init_resource::<Messages<CombatTextSpawn>>();
        let prepare_system = world.register_system(prepare);
        world.insert_resource(Prepared(prepare_system));
        world
    }
    fn spawn(world: &mut World, rank: Monster, pos: Vec3, maximum: f32) -> Entity {
        let id = world
            .run_system_once(move |mut commands: Commands, mut run: ResMut<Run>| {
                super::super::enemies::spawn_enemy(
                    &mut commands,
                    &mut run,
                    &super::super::data::roster(0).enemies[0],
                    pos,
                    Vec3::ZERO,
                    rank.is_boss(),
                )
            })
            .unwrap();
        world.entity_mut(id).insert(rank);
        let mut enemy = world.get_mut::<Enemy>(id).unwrap();
        enemy.max_hp = maximum;
        enemy.hp = maximum;
        enemy.damage = 20.0;
        id
    }
    fn step(world: &mut World, dt: f32) {
        world.resource_mut::<Run>().elapsed += dt;
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(dt));
        let system = world.resource::<Prepared>().0;
        world.run_system(system).unwrap();
    }
    #[test]
    fn burning_ground_targets_only_real_players_once_per_second_in_dense_scenes() {
        let mut world=arena();
        world.resource_mut::<Run>().hp=10000.0;
        world.insert_resource(super::super::options::Settings { damage:false,incoming:true,..default() });
        let hero=world.spawn((super::super::hero::Hero {vy:0.0,air:0.0},Transform::default())).id();
        for _ in 0..1000 { world.spawn(Transform::default()); }
        let source=spawn(&mut world,Monster::BurningWake,Vec3::X*10.0,100.0);
        world.spawn(hazard(source,Kind::Fire,Vec3::ZERO,2.5,0.25,3.0,20.0,Monster::BurningWake));
        // Negative control: the old query included every effect/decor/pickup as the host.
        assert!(world.query_filtered::<Entity,(With<Transform>,Without<Enemy>)>().iter(&world).count()>=1001);
        world.resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.25));
        let system=world.register_system(hazards);
        world.run_system(system).unwrap();
        assert_eq!(world.resource::<Run>().hp,9980.0);
        assert_eq!(world.resource::<Messages<CombatTextSpawn>>().len(),1);
        for _ in 0..19 {
            world.resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.05));
            world.run_system(system).unwrap();
        }
        assert_eq!(world.resource::<Run>().hp,9980.0,"not one hit per frame or per scene object");
        world.resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.1));
        world.run_system(system).unwrap();
        assert_eq!(world.resource::<Run>().hp,9960.0);
        let texts=world.resource::<Messages<CombatTextSpawn>>();
        let mut cursor=texts.get_cursor();
        let texts:Vec<_>=cursor.read(texts).collect();
        assert_eq!(texts.len(),2);
        assert!(texts.iter().all(|text|text.anchor==hero));
    }
    #[test]
    fn burning_ground_hits_a_guest_without_hitting_the_host_through_world_objects() {
        use super::super::multiplayer::{Companion,Companions,RemoteOwner};
        let mut world=arena();
        world.init_resource::<Messages<super::super::units::CombatNumber>>();
        world.resource_mut::<Run>().hp=1000.0;
        world.spawn((super::super::hero::Hero {vy:0.0,air:0.0},Transform::from_xyz(100.0,0.0,0.0)));
        for _ in 0..1000 { world.spawn(Transform::default()); }
        let guest=world.spawn((RemoteOwner(42),Transform::default())).id();
        let mut companions=Companions::default();
        companions.0.insert(42,Companion { entity:guest,run:Run {owner:Some(42),hp:1000.0,..default()},
            physics:super::super::hero::Hero {vy:0.0,air:0.0},offer:default(),offer_id:0 });
        world.insert_resource(companions);
        let source=spawn(&mut world,Monster::BurningWake,Vec3::X*10.0,100.0);
        world.spawn(hazard(source,Kind::Fire,Vec3::ZERO,2.5,0.25,3.0,20.0,Monster::BurningWake));
        world.resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.25));
        world.run_system_once(hazards).unwrap();
        assert_eq!(world.resource::<Run>().hp,1000.0);
        assert_eq!(world.resource::<Companions>().0[&42].run.hp,980.0);
        let numbers=world.resource::<Messages<super::super::units::CombatNumber>>();
        let mut cursor=numbers.get_cursor();
        let numbers:Vec<_>=cursor.read(numbers).collect();
        assert_eq!(numbers.len(),1);
        assert_eq!(numbers[0].owner,Some(42));
        assert_eq!(numbers[0].anchor,guest);
    }
    #[test]
    fn player_ailments_advance_once_regardless_of_the_number_of_world_objects() {
        let mut world=arena();
        world.resource_mut::<Run>().hp=1000.0;
        world.spawn((super::super::hero::Hero {vy:0.0,air:0.0},Transform::default()));
        for _ in 0..1000 { world.spawn(Transform::default()); }
        let source=spawn(&mut world,Monster::Plagued,Vec3::X*10.0,100.0);
        world.resource_mut::<Run>().monster_ailments.apply(Ailment {
            source,kind:AilmentKind::Infection,left:5.0,tick:0.0,dps:10.0,bonuses:Bonuses(Monster::Plagued),
        });
        step(&mut world,0.1);
        let run=world.resource::<Run>();
        assert_eq!(run.hp,990.0);
        assert_eq!(run.monster_ailments.ailments.len(),1);
        assert!((run.monster_ailments.ailments[0].left-4.9).abs()<0.001);
    }
    #[test]
    fn riposte_blocks_half_of_attacks_caps_at_seventy_five_and_is_consumed_once() {
        let mut state = Runtime::new(0.0, Vec3::ZERO);
        assert_eq!(Runtime::block_chance(0.0), 0.5);
        assert_eq!(Runtime::block_chance(0.4), 0.75);
        assert_eq!(
            state.defend(
                Monster::Riposte,
                100.0,
                Tags::ATTACK | Tags::PHYSICAL,
                0.0,
                0.49
            ),
            0.0
        );
        assert_eq!(state.take_riposte(0.1), 2.0);
        assert_eq!(state.take_riposte(0.2), 1.0);
        assert_eq!(
            state.defend(Monster::Riposte, 100.0, Tags::ATTACK, 0.0, 0.5),
            100.0
        );
        assert_eq!(
            state.defend(Monster::Riposte, 100.0, Tags::FIRE | Tags::SPELL, 0.0, 0.0),
            100.0
        );
    }
    #[test]
    fn adaptive_remembers_exactly_one_school_and_shields_do_not_fill_vengeance() {
        let rank = Monster::Adaptive.with(Monster::Avenger);
        let mut state = Runtime::new(0.0, Vec3::ZERO);
        assert_eq!(state.defend(rank, 100.0, Tags::FIRE, 0.0, 1.0), 100.0);
        state.hurt(rank, 100.0, 1000.0, 1000.0, Tags::FIRE, 0.0);
        assert_eq!(
            state.defend(rank, 100.0, Tags::FIRE | Tags::DOT, 1.0, 1.0),
            50.0
        );
        assert_eq!(state.defend(rank, 100.0, Tags::COLD, 1.0, 1.0), 100.0);
        state.hurt(rank, 100.0, 900.0, 1000.0, Tags::COLD, 1.0);
        assert_eq!(state.defend(rank, 100.0, Tags::FIRE, 2.0, 1.0), 100.0);
        assert_eq!(state.defend(rank, 100.0, Tags::COLD, 2.0, 1.0), 50.0);
        state.protect(1000.0, 10.0);
        let previous = state.reserve;
        let damage = state.defend(rank, 100.0, Tags::SHADOW, 2.0, 1.0);
        state.hurt(rank, damage, 800.0, 1000.0, Tags::SHADOW, 2.0);
        assert_eq!(state.reserve, previous);
        state.hurt(rank, 10000.0, 800.0, 1000.0, Tags::PHYSICAL, 3.0);
        assert_eq!(state.reserve, 500.0);
    }
    #[test]
    fn second_skin_is_one_hundred_percent_life_has_a_cooldown_and_never_revives() {
        let mut state = Runtime::new(0.0, Vec3::ZERO);
        state.hurt(Monster::SecondSkin, 20.0, 40.0, 100.0, Tags::FIRE, 0.0);
        assert_eq!(state.shield, 100.0);
        assert_eq!(
            state.defend(Monster::SecondSkin, 60.0, Tags::PHYSICAL, 1.0, 1.0),
            0.0
        );
        assert_eq!(state.shield, 40.0);
        state.shield = 0.0;
        state.hurt(Monster::SecondSkin, 20.0, 40.0, 100.0, Tags::FIRE, 2.0);
        assert_eq!(state.shield, 0.0);
        state.hurt(Monster::SecondSkin, 200.0, 100.0, 100.0, Tags::FIRE, 40.0);
        assert_eq!(state.shield, 0.0);
    }
    #[test]
    fn blood_link_uses_source_life_and_copied_healing_does_not_copy_again() {
        let mut world = arena();
        let source = spawn(&mut world, Monster::BloodLink, Vec3::ZERO, 100.0);
        let boss = spawn(
            &mut world,
            Monster::Boss.with(Monster::BloodLink),
            Vec3::X,
            10000.0,
        );
        world.get_mut::<Enemy>(source).unwrap().hp = 40.0;
        world.get_mut::<Enemy>(boss).unwrap().hp = 5000.0;
        world.get_mut::<Runtime>(boss).unwrap().blood_next = 999.0;
        step(&mut world, 6.0);
        assert_eq!(world.get::<Enemy>(source).unwrap().hp, 90.0);
        assert_eq!(world.get::<Enemy>(boss).unwrap().hp, 5050.0);
        step(&mut world, 0.1);
        assert_eq!(world.get::<Enemy>(source).unwrap().hp, 100.0);
        assert_eq!(world.get::<Enemy>(boss).unwrap().hp, 5065.0);
        step(&mut world, 0.1);
        assert_eq!(world.get::<Enemy>(boss).unwrap().hp, 5065.0);
        assert_eq!(world.get::<Runtime>(source).unwrap().healed, 0.0);
        assert_eq!(world.get::<Runtime>(boss).unwrap().healed, 0.0);
    }
    #[test]
    fn allied_deaths_are_counted_once_and_damage_reduction_is_capped_at_sixty_percent() {
        let mut world = arena();
        let source = spawn(&mut world, Monster::DeathAvenger, Vec3::ZERO, 100.0);
        for i in 0..10 {
            let ally = spawn(
                &mut world,
                Monster::Normal,
                Vec3::X * (i as f32 * 0.5),
                100.0,
            );
            world.get_mut::<Enemy>(ally).unwrap().dead = true;
        }
        step(&mut world, 0.1);
        let state = world.get::<Runtime>(source).unwrap();
        assert_eq!(state.death_stacks, 5);
        assert!((state.reduction(Monster::DeathAvenger, Tags::PHYSICAL, 0.1) - 0.4).abs() < 0.001);
        let until = state.death_until;
        step(&mut world, 0.1);
        assert_eq!(world.get::<Runtime>(source).unwrap().death_until, until);
        step(&mut world, 9.0);
        assert_eq!(world.get::<Runtime>(source).unwrap().death_stacks, 0);
    }
    #[test]
    fn guardian_and_aegis_only_create_protection_when_eligible() {
        let mut world = arena();
        let source = spawn(
            &mut world,
            Monster::Guardian.with(Monster::Aegis),
            Vec3::ZERO,
            100.0,
        );
        let ally = spawn(&mut world, Monster::Normal, Vec3::X, 200.0);
        step(&mut world, 6.0);
        assert_eq!(world.get::<Runtime>(source).unwrap().shield, 25.0);
        assert!((world.get::<Runtime>(ally).unwrap().shield - 15.0).abs() < 0.001);
        world.get_mut::<Runtime>(source).unwrap().aegis_next = 20.0;
        world.get_mut::<Runtime>(source).unwrap().shield = 0.0;
        step(&mut world, 0.1);
        assert_eq!(world.get::<Runtime>(source).unwrap().shield, 0.0);
    }
    #[test]
    fn walking_charges_and_trails_exclude_teleports_and_elusive_stores_one_dodge() {
        let mut world = arena();
        let source = spawn(
            &mut world,
            Monster::BurningWake
                .with(Monster::Elusive)
                .with(Monster::Electrostatic),
            Vec3::ZERO,
            100.0,
        );
        world.get_mut::<Transform>(source).unwrap().translation.x = 100.0;
        step(&mut world, 0.1);
        assert_eq!(world.query::<&Hazard>().iter(&world).count(), 0);
        for i in 1..=4 {
            world.get_mut::<Transform>(source).unwrap().translation.x = 100.0 + i as f32 * 2.0;
            step(&mut world, 0.5);
        }
        assert!(world.get::<Runtime>(source).unwrap().dodge_ready);
        assert!(
            world
                .query::<&Hazard>()
                .iter(&world)
                .any(|h| matches!(h.kind, Kind::Fire))
        );
        step(&mut world, 0.1);
        assert!(
            world
                .query::<&Hazard>()
                .iter(&world)
                .any(|h| matches!(h.kind, Kind::Electric))
        );
        let mut state = world.get_mut::<Runtime>(source).unwrap();
        assert_eq!(
            state.defend(Monster::Elusive, 100.0, Tags::FIRE, 3.0, 1.0),
            0.0
        );
        assert_eq!(
            state.defend(Monster::Elusive, 100.0, Tags::FIRE, 3.0, 1.0),
            100.0
        );
    }
    #[test]
    fn bastion_builds_while_stationary_and_decays_after_moving_grace() {
        let mut world = arena();
        let source = spawn(&mut world, Monster::Bastion, Vec3::ZERO, 100.0);
        step(&mut world, 4.0);
        assert_eq!(
            world
                .get::<Runtime>(source)
                .unwrap()
                .armour(Monster::Bastion),
            2.0
        );
        world.get_mut::<Transform>(source).unwrap().translation.x = 1.0;
        step(&mut world, 0.1);
        assert_eq!(
            world
                .get::<Runtime>(source)
                .unwrap()
                .armour(Monster::Bastion),
            2.0
        );
        for i in 2..=5 {
            world.get_mut::<Transform>(source).unwrap().translation.x = i as f32;
            step(&mut world, 1.0);
        }
        assert!(
            world
                .get::<Runtime>(source)
                .unwrap()
                .armour(Monster::Bastion)
                < 2.0
        );
    }
}
