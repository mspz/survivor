//! Player ownership adapter for the shared monster-mod catalogue and runtime.
use super::*;
use crate::survivors::{self as game, abilities::Hit, items::legendary::events::Origin};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(in crate::survivors) struct Snapshot {
    active: Vec<(Monster, f32)>,
    shield: [f32; 2],
    death: [f32; 2],
    bastion: f32,
    riposte: f32,
    aura_speed: f32,
    protection: f32,
    control: [f32; 4],
    #[serde(default)]
    silence: f32,
}
pub(in crate::survivors) fn snapshot(run: &Run) -> Snapshot {
    let s = &run.monster_mods;
    Snapshot {
        active: s.active.clone(),
        shield: [s.runtime.shield, s.runtime.shield_until],
        death: [s.runtime.death_stacks as f32, s.runtime.death_until],
        bastion: s.runtime.bastion,
        riposte: s.runtime.riposte_until,
        aura_speed: s.aura_speed,
        protection: s.protection,
        control: [
            s.control_until,
            s.control_slow,
            s.stunned_until,
            s.runtime.charge_until,
        ],
        silence: run.siege.silenced_until,
    }
}
pub(in crate::survivors) fn restore(run: &mut Run, snapshot: Snapshot) {
    let mut next = State::default();
    for (m, until) in snapshot.active.into_iter().take(50) {
        next.grant(
            m,
            run.elapsed,
            if until == f32::MAX {
                None
            } else {
                Some(until - run.elapsed)
            },
            Vec3::ZERO,
        );
    }
    let clean = |value: f32, max: f32| {
        if value.is_finite() {
            value.clamp(0.0, max)
        } else {
            0.0
        }
    };
    next.runtime.shield = clean(snapshot.shield[0], 1.0e12);
    next.runtime.shield_until = clean(snapshot.shield[1], f32::MAX);
    next.runtime.death_stacks = clean(snapshot.death[0], config().death_stack_cap as f32) as u32;
    next.runtime.death_until = clean(snapshot.death[1], f32::MAX);
    next.runtime.bastion = clean(snapshot.bastion, config().bastion_cap);
    next.runtime.riposte_until = clean(snapshot.riposte, f32::MAX);
    next.aura_speed = clean(snapshot.aura_speed, 10.0).max(1.0);
    next.protection = clean(snapshot.protection, 1.0);
    next.control_until = clean(snapshot.control[0], f32::MAX);
    next.control_slow = clean(snapshot.control[1], 1.0);
    next.stunned_until = clean(snapshot.control[2], f32::MAX);
    run.siege.silenced_until = clean(snapshot.silence, f32::MAX);
    next.runtime.charge_until = clean(snapshot.control[3], f32::MAX);
    run.monster_mods = next;
}

#[derive(Clone)]
pub(in crate::survivors) struct State {
    active: Vec<(Monster, f32)>,
    pub runtime: Runtime,
    aura_speed: f32,
    protection: f32,
    healer_next: f32,
    dead_sent: bool,
    pub transfers: Vec<(Vec3, game::enemies::Dot)>,
    pub burning_deaths: Vec<(Vec3, f32)>,
    banners: Vec<Vec3>,
    control_until: f32,
    control_slow: f32,
    pub stunned_until: f32,
}
impl Default for State {
    fn default() -> Self {
        Self {
            active: vec![],
            runtime: Runtime::new(0.0, Vec3::ZERO),
            aura_speed: 1.0,
            protection: 0.0,
            healer_next: 0.0,
            dead_sent: false,
            transfers: vec![],
            burning_deaths: vec![],
            banners: vec![],
            control_until: 0.0,
            control_slow: 1.0,
            stunned_until: 0.0,
        }
    }
}
impl State {
    /// Incoming control adapter; Unstoppable modifies duration and slowing strength once.
    #[allow(dead_code)]
    pub fn control(&mut self, now: f32, duration: f32, slow: f32, stun: bool) {
        if !duration.is_finite() || duration <= 0.0 {
            return;
        }
        let b = self.bonuses(now);
        let until = now + duration * b.control_duration();
        let slow = b.slow(slow.clamp(0.0, 1.0));
        self.control_slow = if self.control_until > now {
            self.control_slow.min(slow)
        } else {
            slow
        };
        self.control_until = self.control_until.max(until);
        if stun {
            self.stunned_until = self.stunned_until.max(until);
        }
    }
    /// Shared entry point for the future unique: refresh a duration, never duplicate a mod.
    #[allow(dead_code)]
    pub fn grant(&mut self, modifier: Monster, now: f32, duration: Option<f32>, position: Vec3) {
        let until = match duration {
            Some(t) if t.is_finite() && t > 0.0 => (now + t).min(f32::MAX),
            None => f32::MAX,
            _ => return,
        };
        self.active.retain(|(_, until)| *until > now);
        if self.active.is_empty() {
            self.runtime = Runtime::new(now, position);
        }
        for m in Monster::MODS.into_iter().filter(|m| modifier.has(*m) && m.stealable()) {
            if let Some((_, old)) = self.active.iter_mut().find(|(owned, _)| *owned == m) {
                *old = old.max(until);
            } else {
                self.active.push((m, until));
            }
        }
    }
    pub fn rank(&self, now: f32) -> Monster {
        self.active
            .iter()
            .filter(|(_, until)| *until > now)
            .fold(Monster::Normal, |rank, (m, _)| rank.with(*m))
    }
    pub fn bonuses(&self, now: f32) -> Bonuses {
        Bonuses(self.rank(now))
    }
}
pub(in crate::survivors) fn stats(run: &Run, s: &mut game::Stats) {
    let b = run.monster_mods.bonuses(run.elapsed);
    let state = &run.monster_mods.runtime;
    s.max_hp *= b.maximum_life();
    let action = b.action_speed()
        * if b.0.has(Monster::DeathAvenger) {
            state.action_speed(run.elapsed)
        } else {
            1.0
        }
        * run.monster_mods.aura_speed;
    s.speed *= b.movement() * action;
    if run.monster_mods.stunned_until > run.elapsed || state.charge_until > run.elapsed {
        s.speed = 0.0;
    } else if run.monster_mods.control_until > run.elapsed {
        s.speed *= run.monster_mods.control_slow;
    }
    s.armor *= state.armour(b.0);
    s.regen += s.max_hp * b.regeneration();
    if b.0.has(Monster::Precise) {
        s.crit += game::monster_mod_effects::get().stats.critical_chance;
    }
}
pub(in crate::survivors) fn spell(run: &Run, p: &mut game::spell_engine::Resolved) {
    let b = run.monster_mods.bonuses(run.elapsed);
    let c = &game::monster_mod_effects::get().stats;
    let action = b.action_speed()
        * if b.0.has(Monster::DeathAvenger) {
            run.monster_mods.runtime.action_speed(run.elapsed)
        } else {
            1.0
        }
        * run.monster_mods.aura_speed;
    p.cooldown /= action;
    p.radius *= b.area_radius();
    p.range *= b.range();
    p.link_range *= b.range();
    if p.tags.contains(Tags::PROJECTILE) {
        p.count = p.count.saturating_add(b.projectiles() as usize);
        p.chains = p.chains.saturating_add(b.chains() as usize);
    } else if p.tags.contains(Tags::CHAIN) {
        // Native chain skills store their total links in count, rather than chains.
        p.count = p.count.saturating_add(b.chains() as usize);
    }
    if b.0.has(Monster::Devastating) {
        p.crit_multiplier += c.critical_multiplier;
    }
    if b.0.has(Monster::Echo) && p.tags.contains(Tags::DAMAGE) {
        p.recast += 1.0;
    }
    let power = if b.0.has(Monster::DeathAvenger) {
        run.monster_mods.runtime.damage(run.elapsed)
    } else {
        1.0
    };
    let dot_factor = b.packets(1.0, p.dot_tags.unwrap_or(p.tags) | Tags::DOT, false)[0].0 * power;
    p.dot *= dot_factor;
    if let Some(d) = &mut p.direct_dot {
        d.dps *= b.packets(1.0, d.tags | Tags::DOT, false)[0].0 * power;
    }
}
pub(in crate::survivors) fn block(run: &Run, existing: f32) -> f32 {
    if run.legendary.has(game::items::legendary::chase::BULWARK) {
        let bonus = if run.monster_mods.rank(run.elapsed).has(Monster::Riposte) { game::monster_mod_effects::get().special.riposte_block } else { 0.0 };
        return (existing + bonus).clamp(0.0, game::items::legendary::chase::config().maximum_block);
    }
    if run.monster_mods.rank(run.elapsed).has(Monster::Riposte) {
        Runtime::block_chance(existing)
    } else {
        existing.clamp(0.0, 0.75)
    }
}
pub(in crate::survivors) fn blocked(run: &mut Run) {
    if run.monster_mods.rank(run.elapsed).has(Monster::Riposte) {
        run.monster_mods.runtime.riposte_until = run.elapsed + config().riposte_duration;
    }
}
pub(in crate::survivors) fn defend(run: &mut Run, raw: f32, tags: Tags) -> f32 {
    let rank = run.monster_mods.rank(run.elapsed);
    // Native block is resolved once, before this absorption stage.
    let no_block = if rank.has(Monster::Riposte) {
        Monster::MODS
            .into_iter()
            .filter(|m| *m != Monster::Riposte && rank.has(*m))
            .fold(Monster::Normal, |r, m| r.with(m))
    } else {
        rank
    };
    run.monster_mods.runtime.defend(
        no_block,
        raw * Bonuses(rank).damage_taken(tags) * (1.0 - run.monster_mods.protection),
        tags,
        run.elapsed,
        1.0,
    )
}
pub(in crate::survivors) fn hurt(run: &mut Run, amount: f32, before: f32, tags: Tags) {
    let rank = run.monster_mods.rank(run.elapsed);
    let max = run.stats().max_hp;
    run.monster_mods
        .runtime
        .hurt(rank, amount, before, max, tags, run.elapsed);
}
pub(in crate::survivors) fn buffs(run: &Run) -> Vec<game::player_auras::Aura> {
    let mut out: Vec<_> = run
        .monster_mods
        .active
        .iter()
        .filter(|(_, until)| *until > run.elapsed)
        .map(|(m, until)| mod_aura(*m, (*until < f32::MAX).then_some(*until - run.elapsed)))
        .collect();
    if run.siege.silenced_until > run.elapsed {
        out.push(game::player_auras::Aura {
            spell: 2139, name: "Concentration broken".into(),
            description: "Cannot cast spells.".into(),
            left: Some(run.siege.silenced_until - run.elapsed), debuff: true,
        });
    }
    let state = &run.monster_mods.runtime;
    if state.shield > 0.0 && state.shield_until > run.elapsed {
        out.push(game::player_auras::Aura::buff(
            17,
            "Monster modifiers: Absorption",
            format!("Absorbs {:.0} more damage.", state.shield),
            Some(state.shield_until - run.elapsed),
        ));
    }
    out
}
/// One shared icon/name/effect for native monster mods and stolen player mods.
pub(in crate::survivors) fn mod_aura(modifier: Monster, left: Option<f32>) -> game::player_auras::Aura {
    let index = Monster::MODS.iter().position(|m| *m == modifier).unwrap();
    let spell = [
        6673, 2983, 13812, 635, 17, 2645, 6673, 133, 116, 8042, 686, 2818, 19434, 19434,
        75, 421, 10, 75, 871, 543, 6143, 8177, 6229, 1044, 139, 689, 467, 6673, 1160, 6673,
        6673, 17, 635, 8042, 421, 100, 13812, 5277, 17, 5308, 7384, 543, 17, 871, 1725,
        2818, 1943, 133, 75, 5143, 133, 13812, 2120, 100, 2139, 75, 2120, 5740, 43265,
    ][index];
    game::player_auras::Aura::buff(spell, format!("Monster modifier: {}", modifier.labels()[0]), description(modifier), left)
}
fn description(m: Monster) -> String {
    // The tooltip names the actual mod rather than the reused WoW icon's spell.
    let text = [
        "2.5× maximum health; 1.25× size.",
        "1.5× movement speed.",
        "Explodes in Fire damage on death after a visible warning.",
        "Periodically heals nearby allies, including itself.",
        "Grants a damage reduction aura to nearby allies, including itself. Identical auras do not stack.",
        "1.4× action speed.",
        "40% more Physical damage.",
        "Gain 35% of direct damage as extra Fire damage.",
        "Gain 35% of direct damage as extra Cold damage.",
        "Gain 35% of direct damage as extra Lightning/Nature damage.",
        "Gain 35% of direct damage as extra Shadow damage.",
        "1.75× damage over time.",
        "+15 percentage points of critical strike chance.",
        "+75 percentage points of critical strike multiplier.",
        "2 additional projectiles.",
        "2 additional chains.",
        "1.75× area of effect.",
        "1.4× range.",
        "20% less Physical damage taken.",
        "20% less Fire damage taken.",
        "20% less Cold damage taken.",
        "20% less Nature/Lightning damage taken.",
        "20% less Shadow/Arcane damage taken.",
        "Crowd control duration and strength are halved.",
        "Regenerates 5% of maximum health per second.",
        "Recover 5% of actual direct damage dealt as health.",
        "Removed modifier.",
        "Grants a 1.2× action speed aura to nearby allies, including itself. Identical auras do not stack.",
        "Nearby enemies deal 15% less damage.",
        "At low health, protects nearby allies and knocks back ordinary enemies.",
        "Nearby allied deaths temporarily grant damage, action speed and damage reduction, capped at 60% reduction.",
        "Periodically shields a nearby unshielded ally, including itself.",
        "Periodically heals itself and nearby allies for 50% of its own maximum health. Copies 30% of effective healing to the most injured nearby ally. Copies cannot trigger further copies.",
        "Moving charges a Lightning nova that is released when movement stops.",
        "Periodically pulls nearby enemies after a visible warning.",
        "Moving prepares a charge followed by a melee impact.",
        "Moving leaves a trail of fire. Teleports do not count as distance travelled.",
        "Continuous movement stores one charge to dodge the next direct hit.",
        "At low health, gains a shield equal to 100% of maximum health. Long cooldown.",
        "Health lost fills a reserve that is released as a Physical shockwave.",
        "+50 percentage points of Block Chance, capped at 75%. Blocking empowers the next melee attack.",
        "Take 50% less damage from the last damage school received for 6 seconds. Only one adaptation can be active.",
        "Gains an absorption shield after 5 seconds without losing health.",
        "Standing still builds armour. The bonus briefly persists after moving.",
        "50% more damage against isolated enemies.",
        "Direct hits apply a Nature infection. On death, the infection spreads to a nearby enemy with its remaining duration.",
        "Physical attacks apply Bleeding. Bleeding enemies take more melee damage.",
        "Direct hits can Ignite. Enemies killed while burning leave a small patch of fire.",
        "4 additional projectiles. Stacks with Artillery.",
        "100% chance to repeat damaging skills once. Repeats cannot trigger further repeats.",
        "Replaces caster projectiles with warned ground impacts dealing 300% more damage. Cannot be stolen.",
        "Impacts leave burning ground for 3 seconds. Cannot be stolen.",
        "Marks players who remain within 3 yards for 2 seconds. Explodes after 1 second. Cannot be stolen.",
        "Leaps onto the last warned position of a distant player. Cannot be stolen.",
        "Two spells without moving trigger a warned interrupting impact. Cannot be stolen.",
        "Warns three lanes toward the farthest player, leaving gaps. Cannot be stolen.",
        "Places persistent ground under a distant player. At most two fields. Cannot be stolen.",
        "Three separately warned impacts follow a player and their movement direction. Cannot be stolen.",
        "Creates large ground with damage escalating while a player remains inside. Cannot be stolen.",
    ];
    text[Monster::MODS.iter().position(|v| *v == m).unwrap()].into()
}

const MOD_SOURCE: &str = "Monster mod";
fn hit(owner: Option<u64>, target: Entity, damage: f32, tags: Tags) -> Hit {
    let mut h = Hit::direct("heroic_strike", target, damage, false);
    h.owner = owner;
    h.tags = Some(tags | Tags::DAMAGE);
    h.meter_source = Some(MOD_SOURCE);
    h.triggered = true;
    h.proc_admitted = true;
    h.proc_resolved = true;
    h.origin = Origin::Talent;
    h
}
pub(in crate::survivors) fn before_hit(
    run: &mut Run,
    h: &mut Hit,
    enemy: &Enemy,
    isolated: bool,
    extra: &mut Vec<Hit>,
) {
    if h.meter_source == Some(MOD_SOURCE) {
        return;
    }
    let rank = run.monster_mods.rank(run.elapsed);
    if rank.count() == 0
        && !enemy
            .dots
            .iter()
            .any(|d| d.meter_source == Some(MOD_SOURCE) && d.skill == "proc_bleed" && d.left > 0.0)
    {
        return;
    }
    let b = Bonuses(rank);
    let tags = h
        .tags
        .unwrap_or_else(|| game::spell_engine::tags::for_spell(game::data::ability(h.skill)));
    let mut factor = if rank.has(Monster::DeathAvenger) {
        run.monster_mods.runtime.damage(run.elapsed)
    } else {
        1.0
    };
    if rank.has(Monster::Predator) && isolated {
        factor *= config().predator_damage;
    }
    if tags.contains(Tags::MELEE)
        && enemy
            .dots
            .iter()
            .any(|d| d.left > 0.0 && d.skill == "proc_bleed" && d.meter_source == Some(MOD_SOURCE))
    {
        factor *= config().bleed_vulnerability;
    }
    if rank.has(Monster::Riposte) && tags.contains(Tags::MELEE) && tags.contains(Tags::ATTACK) {
        factor *= run.monster_mods.runtime.take_riposte(run.elapsed);
    }
    let packets = b.packets(h.damage * factor, tags, false);
    h.damage = packets[0].0;
    if !tags.contains(Tags::DOT) {
        for &(damage, school) in packets.iter().skip(1) {
            extra.push(hit(h.owner, h.target, damage, school));
        }
    }
}
pub(in crate::survivors) fn after_hit(
    run: &mut Run,
    h: &Hit,
    enemy: &mut Enemy,
    effective: f32,
    tags: Tags,
) {
    if effective <= 0.0 || tags.contains(Tags::DOT) {
        return;
    }
    let rank = run.monster_mods.rank(run.elapsed);
    let b = Bonuses(rank);
    run.heal(effective * b.leech());
    if h.meter_source == Some(MOD_SOURCE) {
        return;
    }
    let c = config();
    for (enabled, skill, school, duration, factor) in [
        (
            rank.has(Monster::Plagued),
            "proc_poison",
            Tags::NATURE,
            c.infection_duration,
            c.infection_damage,
        ),
        (
            rank.has(Monster::Hemorrhagic) && tags.contains(Tags::PHYSICAL | Tags::ATTACK),
            "proc_bleed",
            Tags::PHYSICAL,
            c.bleed_duration,
            c.bleed_damage,
        ),
        (
            rank.has(Monster::Incendiary) && run.extra_spells.dodge_rng.f32() < c.ignite_chance,
            "proc_ignite",
            Tags::FIRE,
            c.ignite_duration,
            c.ignite_damage,
        ),
    ] {
        if !enabled {
            continue;
        }
        let dps = b.packets(h.damage * factor, school | Tags::DOT, false)[0].0;
        if let Some(dot) = enemy.dots.iter_mut().find(|d| {
            d.owner == run.owner && d.skill == skill && d.meter_source == Some(MOD_SOURCE)
        }) {
            dot.left = dot.left.max(duration);
            dot.dps = dot.dps.max(dps);
        } else {
            enemy.dots.push(game::enemies::Dot {
                owner: run.owner,
                skill,
                meter_source: Some(MOD_SOURCE),
                tags: Some(school | Tags::DOT | Tags::DAMAGE),
                origin: Origin::Talent,
                item_source: None,
                meta: default(),
                dps,
                left: duration,
                tick: 1.0,
            });
        }
    }
}
pub(in crate::survivors) fn death(
    commands: &mut Commands,
    run: &mut Run,
    enemy: &Enemy,
    pos: Vec3,
) {
    for dot in enemy
        .dots
        .iter()
        .filter(|d| d.left > 0.0 && d.meter_source == Some(MOD_SOURCE))
    {
        if dot.owner == run.owner {
            record_death(&mut run.monster_mods, pos, dot);
        } else {
            let dot = dot.clone();
            commands.queue(move |world: &mut World| {
                if let Some(owner) = dot.owner {
                    if let Some(mut players) =
                        world.get_resource_mut::<game::multiplayer::Companions>()
                    {
                        if let Some(player) = players.0.get_mut(&owner) {
                            record_death(&mut player.run.monster_mods, pos, &dot);
                        }
                    }
                } else if let Some(mut player) = world.get_resource_mut::<Run>() {
                    record_death(&mut player.monster_mods, pos, &dot);
                }
            });
        }
    }
}

fn record_death(state: &mut State, pos: Vec3, dot: &game::enemies::Dot) {
    match dot.skill {
        "proc_poison" => state.transfers.push((pos, dot.clone())),
        "proc_ignite" => state.burning_deaths.push((pos, dot.dps)),
        _ => {}
    }
}

pub(in crate::survivors) fn pet_action(run: &Run, pos: Vec3) -> f32 {
    if run
        .monster_mods
        .banners
        .iter()
        .any(|p| near(*p, pos, game::monster_mod_effects::get().stats.aura_radius))
    {
        game::monster_mod_effects::get().stats.aura_action_speed
    } else {
        1.0
    }
}
pub(in crate::survivors) fn pet_movement(run: &Run, pos: Vec3) -> f32 {
    // Pet movement already inherits owner speed; substitute the aura at the pet's
    // own position instead of applying the owner's aura a second time.
    pet_action(run, pos) / run.monster_mods.aura_speed
}
#[derive(Clone)]
struct Friend {
    owner: Option<u64>,
    id: Entity,
    pos: Vec3,
    hp: f32,
    max: f32,
    shield: f32,
    rank: Monster,
}
#[derive(Component)]
pub(in crate::survivors) struct Field {
    owner: Option<u64>,
    source: Entity,
    rank: Monster,
    kind: Kind,
    warning: game::boss_spells::Warning,
    damage: f32,
    delay: f32,
    duration: f32,
    age: f32,
    next: f32,
    target: Option<Entity>,
}
fn field(
    commands: &mut Commands,
    owner: Option<u64>,
    source: Entity,
    rank: Monster,
    kind: Kind,
    pos: Vec3,
    radius: f32,
    delay: f32,
    duration: f32,
    damage: f32,
) {
    let shape = hazard(source, kind, pos, radius, delay, duration, damage, rank);
    commands.spawn((
        game::RunEntity,
        Field {
            owner,
            source,
            rank,
            kind,
            warning: shape.warning,
            damage,
            delay,
            duration,
            age: 0.0,
            next: delay,
            target: None,
        },
    ));
}
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(in crate::survivors) fn tick(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut companions: Option<ResMut<game::multiplayer::Companions>>,
    mut players: Query<
        (
            Entity,
            &mut Transform,
            Option<&game::multiplayer::RemoteOwner>,
            Has<game::hero::Hero>,
        ),
        Without<Enemy>,
    >,
    mut enemies: Query<(Entity, &mut Transform, &mut Enemy)>,
    mut seen_dead: Local<std::collections::HashSet<Entity>>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let now = run.elapsed;
    let c = config();
    let mut friends = Vec::new();
    for (id, tf, owner, local) in &players {
        let owner = if local {
            None
        } else {
            let Some(owner) = owner else { continue };
            Some(owner.0)
        };
        if let Some(player) =
            game::multiplayer::player_run(&mut run, companions.as_deref_mut(), owner)
        {
            friends.push(Friend {
                owner,
                id,
                pos: tf.translation,
                hp: player.hp,
                max: player.stats().max_hp,
                shield: (if player.monster_mods.runtime.shield_until > now {
                    player.monster_mods.runtime.shield
                } else {
                    0.0
                }) + if player.shield_t > 0.0 {
                    player.shield.max(0.0)
                } else {
                    0.0
                } + player
                    .support
                    .buffs
                    .iter()
                    .filter(|b| b.until > now)
                    .map(|b| b.reserve.max(0.0))
                    .sum::<f32>(),
                rank: player.monster_mods.rank(now),
            });
        }
    }
    seen_dead.retain(|id| friends.iter().any(|p| p.id == *id && p.hp <= 0.0));
    let deaths: Vec<_> = friends
        .iter()
        .filter(|p| p.hp <= 0.0 && seen_dead.insert(p.id))
        .map(|p| (p.id, p.pos))
        .collect();
    let banners: Vec<_> = friends
        .iter()
        .filter(|p| p.hp > 0.0 && p.rank.has(Monster::Banner))
        .map(|p| p.pos)
        .collect();
    let protectors: Vec<_> = friends
        .iter()
        .filter(|p| p.hp > 0.0 && p.rank.has(Monster::Protector))
        .map(|p| p.pos)
        .collect();
    let mut healing = Vec::new();
    let mut shields = Vec::new();
    for friend in &friends {
        let Some(player) =
            game::multiplayer::player_run(&mut run, companions.as_deref_mut(), friend.owner)
        else {
            continue;
        };
        let rank = player.monster_mods.rank(now);
        let damage = if rank.has(Monster::Explosive)
            || rank.has(Monster::BurningWake)
            || rank.has(Monster::Electrostatic)
            || rank.has(Monster::Charger)
        {
            player
                .abilities
                .iter()
                .map(|o| player.resolve_spell(o.def, o.level).dmg)
                .fold(20.0_f32, f32::max)
        } else {
            20.0
        };
        let damage = damage
            * if rank.has(Monster::DeathAvenger) {
                player.monster_mods.runtime.damage(now)
            } else {
                1.0
            };
        let speed = player.stats().speed;
        let max = friend.max;
        let state = &mut player.monster_mods;
        state.active.retain(|(_, until)| *until > now);
        state.banners.clone_from(&banners);
        state.aura_speed = if banners.iter().any(|p| {
            near(
                *p,
                friend.pos,
                game::monster_mod_effects::get().stats.aura_radius,
            )
        }) {
            game::monster_mod_effects::get().stats.aura_action_speed
        } else {
            1.0
        };
        state.protection = if protectors
            .iter()
            .any(|p| near(*p, friend.pos, player.expeditions.protectors.radius))
        {
            player.expeditions.protectors.damage_reduction
        } else {
            0.0
        };
        if state.runtime.shield_until <= now {
            state.runtime.shield = 0.0;
        }
        if state.runtime.death_until <= now || !rank.has(Monster::DeathAvenger) {
            state.runtime.death_stacks = 0;
        }
        for (pos, dot) in std::mem::take(&mut state.transfers) {
            if let Some((_, _, mut enemy)) = enemies
                .iter_mut()
                .filter(|(_, _, e)| !e.dead && e.hp > 0.0)
                .filter(|(_, tf, _)| near(tf.translation, pos, c.radius))
                .min_by(|a, b| {
                    a.1.translation
                        .distance_squared(pos)
                        .total_cmp(&b.1.translation.distance_squared(pos))
                })
            {
                if let Some(old) = enemy.dots.iter_mut().find(|d| {
                    d.owner == dot.owner
                        && d.skill == dot.skill
                        && d.meter_source == dot.meter_source
                }) {
                    old.left = old.left.max(dot.left);
                    old.dps = old.dps.max(dot.dps);
                } else {
                    enemy.dots.push(dot);
                }
            }
        }
        for (pos, damage) in std::mem::take(&mut state.burning_deaths) {
            field(
                &mut commands,
                friend.owner,
                friend.id,
                rank,
                Kind::Fire,
                pos,
                c.fire_radius,
                0.25,
                c.fire_duration,
                damage,
            );
        }
        if friend.hp <= 0.0 {
            if !state.dead_sent && rank.has(Monster::Explosive) {
                field(
                    &mut commands,
                    friend.owner,
                    friend.id,
                    rank,
                    Kind::Repeat,
                    friend.pos,
                    player.expeditions.explosives.radius,
                    player.expeditions.explosives.delay,
                    0.0,
                    damage * player.expeditions.explosives.damage_multiplier,
                );
            }
            state.dead_sent = true;
            continue;
        }
        state.dead_sent = false;
        if rank.has(Monster::DeathAvenger) {
            let count = deaths
                .iter()
                .filter(|(id, pos)| *id != friend.id && near(*pos, friend.pos, c.radius))
                .count() as u32;
            if count > 0 {
                state.runtime.death_stacks =
                    (state.runtime.death_stacks + count).min(c.death_stack_cap);
                state.runtime.death_until = now + c.death_duration;
            }
        }
        let rt = &mut state.runtime;
        let travel = game::units::flat(friend.pos - rt.last_pos).length();
        let forced = rt.charge_until > 0.0 && rt.last_sample <= rt.charge_until;
        let walking = travel > 0.01 && travel <= (speed * dt * 3.0).max(8.0) && !forced;
        rt.last_pos = friend.pos;
        rt.last_sample = now;
        if walking {
            if rt.stationary > 0.0 {
                rt.grace_until = now + c.bastion_grace;
            }
            rt.stationary = 0.0;
            rt.moving += dt;
            rt.distance = (rt.distance + travel).min(c.charge_distance);
            rt.electric = (rt.electric + travel).min(c.electric_distance);
            rt.wake_distance += travel;
            if rank.has(Monster::Elusive) && rt.moving >= c.elusive_seconds {
                rt.dodge_ready = true;
            }
            if now > rt.grace_until {
                rt.bastion = (rt.bastion - dt * c.bastion_per_second).max(0.0);
            }
            if rank.has(Monster::BurningWake)
                && rt.wake_distance >= c.wake_spacing
                && now >= rt.wake_next
            {
                field(
                    &mut commands,
                    friend.owner,
                    friend.id,
                    rank,
                    Kind::Fire,
                    friend.pos,
                    c.fire_radius,
                    0.25,
                    c.fire_duration,
                    damage * c.fire_damage,
                );
                rt.wake_distance = 0.0;
                rt.wake_next = now + c.wake_interval;
            }
        } else if travel <= 0.01 && !forced {
            rt.moving = 0.0;
            rt.stationary += dt;
            if rank.has(Monster::Bastion) {
                rt.bastion = (rt.bastion + dt * c.bastion_per_second).min(c.bastion_cap);
            }
            if rank.has(Monster::Electrostatic) && rt.electric >= c.electric_distance {
                field(
                    &mut commands,
                    friend.owner,
                    friend.id,
                    rank,
                    Kind::Electric,
                    friend.pos,
                    c.electric_radius,
                    0.5,
                    0.0,
                    damage * c.electric_damage,
                );
                rt.electric = 0.0;
            }
        } else {
            rt.moving = 0.0;
            rt.stationary = 0.0;
        }
        if rank.has(Monster::Aegis) && now >= rt.aegis_next && rt.shield <= 0.0 {
            rt.protect(max * c.aegis_shield, now + c.aegis_duration);
            rt.aegis_next = now + c.aegis_delay;
        }
        if rt.rally_pending {
            rt.rally_pending = false;
            for ally in friends
                .iter()
                .filter(|p| p.hp > 0.0 && near(p.pos, friend.pos, c.radius))
            {
                shields.push((ally.owner, max * c.rally_shield, now + c.rally_duration));
            }
            field(
                &mut commands,
                friend.owner,
                friend.id,
                rank,
                Kind::Push,
                friend.pos,
                c.radius,
                0.25,
                0.0,
                0.0,
            );
        }
        if rt.reserve_pending {
            rt.reserve_pending = false;
            field(
                &mut commands,
                friend.owner,
                friend.id,
                rank,
                Kind::PhysicalWave,
                friend.pos,
                c.radius,
                0.5,
                0.0,
                rt.reserve,
            );
            rt.reserve = 0.0;
            rt.reserve_next = now + c.reserve_cooldown;
        }
        if rank.has(Monster::Guardian) && now >= rt.guardian_next {
            if let Some(ally) = friends
                .iter()
                .filter(|p| p.hp > 0.0 && p.shield <= 0.0 && near(p.pos, friend.pos, c.radius))
                .min_by(|a, b| (a.hp / a.max).total_cmp(&(b.hp / b.max)))
            {
                shields.push((
                    ally.owner,
                    max * c.guardian_shield,
                    now + c.guardian_duration,
                ));
                rt.guardian_next = now + c.guardian_interval;
            }
        }
        let received = std::mem::take(&mut rt.healed);
        if rank.has(Monster::BloodLink) {
            if received > 0.0 {
                if let Some(ally) = friends
                    .iter()
                    .filter(|p| {
                        p.id != friend.id
                            && p.hp > 0.0
                            && p.hp < p.max
                            && near(p.pos, friend.pos, c.radius)
                    })
                    .min_by(|a, b| (a.hp / a.max).total_cmp(&(b.hp / b.max)))
                {
                    healing.push((ally.owner, received * c.blood_copy, false));
                }
            }
            if now >= rt.blood_next {
                for ally in friends
                    .iter()
                    .filter(|p| p.hp > 0.0 && near(p.pos, friend.pos, c.radius))
                {
                    healing.push((ally.owner, max * c.blood_heal, true));
                }
                rt.blood_next = now + c.blood_interval;
            }
        }
        if rank.has(Monster::Healer) && now >= state.healer_next {
            for ally in friends.iter().filter(|p| {
                p.hp > 0.0 && near(p.pos, friend.pos, player.expeditions.healers.radius)
            }) {
                healing.push((
                    ally.owner,
                    ally.max * player.expeditions.healers.heal_fraction,
                    true,
                ));
            }
            state.healer_next = now + player.expeditions.healers.interval;
        }
        if rank.has(Monster::Magnetic) && now >= rt.magnetic_next {
            field(
                &mut commands,
                friend.owner,
                friend.id,
                rank,
                Kind::Pull,
                friend.pos,
                c.radius,
                c.magnetic_windup,
                0.0,
                0.0,
            );
            rt.magnetic_next = now + c.magnetic_interval;
        }
        if rank.has(Monster::Charger) && rt.distance >= c.charge_distance && now >= rt.charge_until
        {
            if let Some((id, tf, _)) = enemies
                .iter()
                .filter(|(_, _, e)| !e.dead && e.hp > 0.0)
                .filter(|(_, tf, _)| near(tf.translation, friend.pos, c.charge_length + 3.0))
                .min_by(|a, b| {
                    a.1.translation
                        .distance_squared(friend.pos)
                        .total_cmp(&b.1.translation.distance_squared(friend.pos))
                })
            {
                let mut shape = hazard(
                    friend.id,
                    Kind::Charge,
                    friend.pos,
                    0.0,
                    c.charge_windup,
                    c.charge_duration,
                    damage * c.charge_damage,
                    rank,
                )
                .warning;
                shape.spell = game::boss_spells::Spell::Charge;
                shape.circle = None;
                shape.direction = game::units::flat(tf.translation - friend.pos)
                    .normalize_or_zero()
                    .to_array();
                shape.spatial_scale = c.charge_length / 24.0;
                shape.windup_seconds = Some(c.charge_windup);
                commands.spawn((
                    game::RunEntity,
                    Field {
                        owner: friend.owner,
                        source: friend.id,
                        rank,
                        kind: Kind::Charge,
                        warning: shape,
                        damage: damage * c.charge_damage,
                        delay: c.charge_windup,
                        duration: c.charge_duration,
                        age: 0.0,
                        next: c.charge_windup + c.charge_duration,
                        target: Some(id),
                    },
                ));
                rt.distance = 0.0;
                rt.charge_until = now + c.charge_windup + c.charge_duration;
            }
        }
    }
    for (owner, amount, copy) in healing {
        if let Some(player) =
            game::multiplayer::player_run(&mut run, companions.as_deref_mut(), owner)
        {
            let received = player.recover(amount);
            if !copy {
                player.monster_mods.runtime.healed =
                    (player.monster_mods.runtime.healed - received).max(0.0);
            }
        }
    }
    for (owner, amount, until) in shields {
        if let Some(player) =
            game::multiplayer::player_run(&mut run, companions.as_deref_mut(), owner)
        {
            player.monster_mods.runtime.protect(amount, until);
        }
    }
    for (_, tf, mut enemy) in &mut enemies {
        if enemy.dead || enemy.hp <= 0.0 {
            continue;
        }
        if friends.iter().any(|p| {
            p.hp > 0.0
                && p.rank.has(Monster::Oppressor)
                && near(
                    p.pos,
                    tf.translation,
                    game::monster_mod_effects::get().stats.aura_radius,
                )
        }) {
            enemy.weaken = enemy
                .weaken
                .min(1.0 - game::monster_mod_effects::get().stats.oppression);
            enemy.weaken_t = enemy.weaken_t.max(dt * 2.0);
        }
    }
    // Visual size is absolute: repeated grants never multiply the transform again.
    for (_, mut tf, owner, local) in &mut players {
        let owner = if local {
            None
        } else {
            let Some(owner) = owner else { continue };
            Some(owner.0)
        };
        if let Some(player) =
            game::multiplayer::player_run(&mut run, companions.as_deref_mut(), owner)
        {
            tf.scale = Vec3::splat(player.monster_mods.bonuses(now).size());
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(in crate::survivors) fn fields(
    mut commands: Commands,
    time: Res<Time>,
    run: Res<Run>,
    companions: Option<Res<game::multiplayer::Companions>>,
    surface: game::geometry::Surface,
    mut fields: Query<(Entity, &mut Field)>,
    mut players: Query<&mut Transform, Without<Enemy>>,
    mut enemies: Query<(Entity, &mut Transform, &Enemy)>,
    mut warnings: ResMut<game::boss_spells::Warnings>,
    mut hits: MessageWriter<Hit>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let c = config();
    let positions: Vec<_> = enemies
        .iter()
        .filter(|(_, _, e)| !e.dead && e.hp > 0.0)
        .map(|(id, tf, _)| (id, tf.translation))
        .collect();
    for (id, mut field) in &mut fields {
        let alive = if let Some(owner) = field.owner {
            companions
                .as_ref()
                .and_then(|p| p.0.get(&owner))
                .is_some_and(|p| p.run.hp > 0.0 && p.run.dead_t.is_none())
        } else {
            run.hp > 0.0 && run.dead_t.is_none()
        };
        if !alive && !matches!(field.kind, Kind::Fire | Kind::Repeat) {
            commands.entity(id).despawn();
            continue;
        }
        field.age += dt;
        field.warning.elapsed = field.age;
        warnings.0.push(field.warning.clone());
        if field.age < field.delay {
            continue;
        }
        if matches!(field.kind, Kind::Charge) {
            if let Ok(mut tf) = players.get_mut(field.source) {
                let progress = ((field.age - field.delay) / field.duration).clamp(0.0, 1.0);
                let desired = Vec3::from_array(field.warning.origin)
                    + Vec3::from_array(field.warning.direction) * c.charge_length * progress;
                let desired = surface.constrain(tf.translation, desired);
                tf.translation = game::enemies::ground(&surface, desired, tf.translation.y);
            }
        }
        let through = if matches!(field.kind, Kind::Fire) {
            field.age.min(field.delay + field.duration - 0.0001)
        } else {
            field.age
        };
        let count = if through >= field.next {
            if matches!(field.kind, Kind::Fire) {
                ((through - field.next).floor() as u32 + 1).min(16)
            } else {
                1
            }
        } else {
            0
        };
        if count > 0 {
            for (target, mut tf, enemy) in &mut enemies {
                if enemy.dead || enemy.hp <= 0.0 || field.target.is_some_and(|t| t != target) {
                    continue;
                }
                let center = Vec3::from_array(field.warning.origin);
                let contact = if matches!(field.kind, Kind::Charge) {
                    players.get(field.source).is_ok_and(|p| {
                        near(
                            p.translation,
                            tf.translation,
                            game::spell_engine::targeting::melee_contact_range(enemy.radius),
                        )
                    })
                } else {
                    field.warning.contains(tf.translation, enemy.radius)
                };
                if !contact {
                    continue;
                }
                if matches!(field.kind, Kind::Push | Kind::Pull) {
                    if enemy.boss || game::training::actor(enemy.def) {
                        continue;
                    }
                    let delta = game::units::flat(center - tf.translation);
                    let shift = delta.normalize_or_zero()
                        * if matches!(field.kind, Kind::Pull) {
                            c.pull_distance.min((delta.length() - 1.0).max(0.0))
                        } else {
                            -c.push_distance
                        };
                    let desired = surface.constrain(tf.translation, tf.translation + shift);
                    tf.translation = game::enemies::ground(&surface, desired, tf.translation.y);
                } else {
                    let tags = match field.kind {
                        Kind::Fire => Tags::FIRE | Tags::DOT | Tags::AREA,
                        Kind::Electric => Tags::NATURE | Tags::LIGHTNING | Tags::SPELL | Tags::AREA,
                        Kind::Charge => Tags::PHYSICAL | Tags::MELEE | Tags::ATTACK,
                        Kind::Repeat => Tags::FIRE | Tags::SPELL | Tags::AREA,
                        _ => Tags::PHYSICAL | Tags::SPELL | Tags::AREA,
                    };
                    let factor = if field.rank.has(Monster::Predator)
                        && !positions.iter().any(|(id, pos)| {
                            *id != target && near(*pos, tf.translation, c.predator_radius)
                        }) {
                        c.predator_damage
                    } else {
                        1.0
                    };
                    for (raw, school) in Bonuses(field.rank).packets(
                        field.damage * count as f32 * factor,
                        tags,
                        false,
                    ) {
                        hits.write(hit(field.owner, target, raw, school));
                    }
                }
            }
            field.next = if matches!(field.kind, Kind::Fire) {
                field.next + count as f32
            } else {
                f32::MAX
            };
        }
        if (matches!(field.kind, Kind::Fire) && field.age >= field.delay + field.duration)
            || (!matches!(field.kind, Kind::Fire) && field.next == f32::MAX)
        {
            commands.entity(id).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn player_healer_guardian_and_protector_include_source_and_oppressor_affects_enemies() {
        let (mut world, _) = arena();
        let target = enemy(&mut world, Vec3::X);
        let maximum;
        {
            let mut run = world.resource_mut::<Run>();
            run.hp = 20.0;
            for m in [
                Monster::Healer,
                Monster::Guardian,
                Monster::Protector,
                Monster::Oppressor,
            ] {
                grant(&mut run, m);
            }
            maximum = run.stats().max_hp;
        }
        let enemy_hp = world.get::<Enemy>(target).unwrap().hp;
        step(&mut world, 6.0);
        let player = world.resource::<Run>();
        assert!(
            (player.hp - 20.0 - maximum * player.expeditions.healers.heal_fraction).abs() < 0.001
        );
        assert_eq!(player.monster_mods.protection, 0.25);
        assert!(
            (player.monster_mods.runtime.shield - maximum * config().guardian_shield).abs() < 0.001
        );
        assert_eq!(world.get::<Enemy>(target).unwrap().hp, enemy_hp);
        assert_eq!(world.get::<Enemy>(target).unwrap().weaken, 0.85);
    }
    #[test]
    fn action_speed_is_multiplicative_with_existing_recovery_and_auras() {
        let mut run = Run::default();
        run.replace_spell_modifiers(
            999,
            [game::spell_engine::Modifier {
                source: 999,
                filter: game::spell_engine::Filter::default(),
                change: game::spell_engine::Change::IncreasedCooldownRecovery(0.2),
            }],
        );
        let def = game::data::ability("fireball");
        let base = run.resolve_spell(def, 1).cooldown;
        grant(&mut run, Monster::Frenzied);
        run.monster_mods.aura_speed = 1.2;
        assert!((run.resolve_spell(def, 1).cooldown - base / (1.4 * 1.2)).abs() < 0.001);
    }
    #[test]
    fn less_damage_precedes_native_shields_and_elusive_does_not_spend_them() {
        let mut run = Run {
            hp: 1000.0,
            shield: 50.0,
            shield_t: 10.0,
            ..default()
        };
        grant(&mut run, Monster::Fireproof);
        assert_eq!(
            game::hero::hurt_hero_tagged(&mut run, 100.0, Tags::FIRE),
            30.0
        );
        run.shield = 50.0;
        grant(&mut run, Monster::Elusive);
        run.monster_mods.runtime.dodge_ready = true;
        assert_eq!(
            game::hero::hurt_hero_tagged(&mut run, 100.0, Tags::FIRE),
            0.0
        );
        assert_eq!(run.shield, 50.0);
    }
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    #[test]
    fn host_snapshot_preserves_player_mod_stats_durations_and_buff_names() {
        let mut host = Run::default();
        for m in [
            Monster::Colossus,
            Monster::Frenzied,
            Monster::Artillery,
            Monster::GreaterArtillery,
            Monster::Echo,
            Monster::Bastion,
        ] {
            host.monster_mods.grant(m, 0.0, Some(12.0), Vec3::ZERO);
        }
        host.elapsed = 3.0;
        host.monster_mods.runtime.bastion = 0.75;
        let encoded = serde_json::to_string(&snapshot(&host)).unwrap();
        let mut guest = Run {
            elapsed: host.elapsed,
            ..default()
        };
        restore(&mut guest, serde_json::from_str(&encoded).unwrap());
        assert_eq!(guest.stats().max_hp, host.stats().max_hp);
        assert_eq!(guest.stats().speed, host.stats().speed);
        assert_eq!(guest.monster_mods.rank(3.0), host.monster_mods.rank(3.0));
        let auras = buffs(&guest);
        assert_eq!(auras.len(), 6);
        assert!(auras.iter().all(|a| a.left == Some(9.0)));
        let baseline = game::data::ability("fireball");
        assert_eq!(
            guest.resolve_spell(baseline, 1),
            host.resolve_spell(baseline, 1)
        );
    }
    #[test]
    fn player_magnetic_warns_before_moving_enemies_and_fields_never_hurt_allies() {
        let (mut world, hero) = arena();
        let target = enemy(&mut world, Vec3::X * 5.0);
        {
            let mut run = world.resource_mut::<Run>();
            grant(&mut run, Monster::Magnetic);
        }
        step(&mut world, 8.0);
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(0.1));
        world.run_system_once(fields).unwrap();
        assert_eq!(world.get::<Transform>(target).unwrap().translation.x, 5.0);
        assert!(!world.resource::<game::boss_spells::Warnings>().0.is_empty());
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(1.0));
        world.run_system_once(fields).unwrap();
        assert!((world.get::<Transform>(target).unwrap().translation.x - 2.0).abs() < 0.01);
        let before = world.resource::<Run>().hp;
        world
            .run_system_once(move |mut commands: Commands| {
                field(
                    &mut commands,
                    None,
                    hero,
                    Monster::Electrostatic,
                    Kind::Electric,
                    Vec3::ZERO,
                    10.0,
                    0.1,
                    0.0,
                    20.0,
                )
            })
            .unwrap();
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(0.2));
        world.run_system_once(fields).unwrap();
        let hits = world.resource::<Messages<Hit>>();
        assert_eq!(hits.len(), 1);
        assert_eq!(world.resource::<Run>().hp, before);
    }
    #[test]
    fn player_charger_moves_after_warning_and_only_hits_its_enemy_once() {
        let (mut world, hero) = arena();
        let _target = enemy(&mut world, Vec3::X * 6.0);
        {
            let mut run = world.resource_mut::<Run>();
            grant(&mut run, Monster::Charger);
            run.monster_mods.runtime.distance = config().charge_distance;
        }
        step(&mut world, 0.1);
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(0.1));
        world.run_system_once(fields).unwrap();
        assert_eq!(world.get::<Transform>(hero).unwrap().translation.x, 0.0);
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(0.75));
        world.resource_mut::<Run>().elapsed += 0.85;
        world.run_system_once(fields).unwrap();
        assert!((world.get::<Transform>(hero).unwrap().translation.x - 6.0).abs() < 0.01);
        assert_eq!(world.resource::<Messages<Hit>>().len(), 1);
        world.run_system_once(fields).unwrap();
        assert_eq!(world.resource::<Messages<Hit>>().len(), 1);
        step(&mut world, 0.1);
        assert_eq!(world.resource::<Run>().monster_mods.runtime.distance, 0.0);
    }
    #[test]
    fn player_death_explosion_happens_once_and_retains_its_fire_school() {
        let (mut world, _) = arena();
        enemy(&mut world, Vec3::X);
        {
            let mut run = world.resource_mut::<Run>();
            grant(&mut run, Monster::Explosive);
            run.hp = 0.0;
        }
        step(&mut world, 0.1);
        step(&mut world, 0.1);
        assert_eq!(world.query::<&Field>().iter(&world).count(), 1);
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(2.0));
        world.run_system_once(fields).unwrap();
        let hits = world.resource::<Messages<Hit>>();
        assert_eq!(hits.len(), 1);
        let mut reader = hits.get_cursor();
        let h = reader.read(hits).next().unwrap();
        assert!(h.tags.unwrap().contains(Tags::FIRE));
        assert!(!h.tags.unwrap().contains(Tags::PHYSICAL));
    }
    #[test]
    fn player_defensive_triggers_and_vengeance_use_real_life_loss() {
        let (mut world, _) = arena();
        {
            let mut run = world.resource_mut::<Run>();
            for m in [
                Monster::Rally,
                Monster::SecondSkin,
                Monster::Avenger,
                Monster::Guardian,
                Monster::Aegis,
            ] {
                grant(&mut run, m);
            }
            let max = run.stats().max_hp;
            run.hp = max * 0.4;
            game::hero::hurt_hero_tagged(&mut run, max * 0.15, Tags::FIRE);
            assert_eq!(run.monster_mods.runtime.shield, max);
            let reserve = run.monster_mods.runtime.reserve;
            game::hero::hurt_hero_tagged(&mut run, 10.0, Tags::FIRE);
            assert_eq!(run.monster_mods.runtime.reserve, reserve);
        }
        step(&mut world, 0.1);
        assert!(
            world
                .query::<&Field>()
                .iter(&world)
                .any(|f| matches!(f.kind, Kind::Push))
        );
        step(&mut world, 6.0);
        assert!(world.resource::<Run>().monster_mods.runtime.shield > 0.0);
        assert!(
            buffs(world.resource::<Run>())
                .iter()
                .any(|a| a.name == "Monster modifiers: Absorption")
        );
    }
    #[test]
    fn player_death_avenger_counts_each_ally_once_and_expires() {
        use game::multiplayer::{Companion, Companions, RemoteOwner};
        let (mut world, _) = arena();
        {
            let mut run = world.resource_mut::<Run>();
            grant(&mut run, Monster::DeathAvenger);
        }
        let mut party = Companions::default();
        for owner in 1..=8 {
            let entity = world
                .spawn((
                    RemoteOwner(owner),
                    Transform::from_xyz(owner as f32, 0.0, 0.0),
                ))
                .id();
            party.0.insert(
                owner,
                Companion {
                    entity,
                    run: Run {
                        owner: Some(owner),
                        hp: 0.0,
                        ..default()
                    },
                    physics: game::hero::Hero { vy: 0.0, air: 0.0 },
                    offer: default(),
                    offer_id: 0,
                },
            );
        }
        world.insert_resource(party);
        step(&mut world, 0.1);
        let state = &world.resource::<Run>().monster_mods.runtime;
        assert_eq!(state.death_stacks, 5);
        let until = state.death_until;
        assert!((state.reduction(Monster::DeathAvenger, Tags::FIRE, 0.1) - 0.4).abs() < 0.001);
        step(&mut world, 0.1);
        assert_eq!(
            world.resource::<Run>().monster_mods.runtime.death_until,
            until
        );
        step(&mut world, 9.0);
        assert_eq!(world.resource::<Run>().monster_mods.runtime.death_stacks, 0);
    }
    #[test]
    fn player_ailments_and_leech_apply_on_hits_and_propagate_after_source_death() {
        let (mut world, _) = arena();
        let target = enemy(&mut world, Vec3::X);
        let next = enemy(&mut world, Vec3::X * 2.0);
        let mut run = world.remove_resource::<Run>().unwrap();
        run.hp = 20.0;
        for m in [
            Monster::Plagued,
            Monster::Hemorrhagic,
            Monster::Incendiary,
            Monster::Vampiric,
            Monster::Venomous,
        ] {
            grant(&mut run, m);
        }
        let mut h = Hit::direct("heroic_strike", target, 100.0, false);
        h.tags = Some(Tags::PHYSICAL | Tags::ATTACK | Tags::MELEE);
        {
            let mut en = world.get_mut::<Enemy>(target).unwrap();
            for _ in 0..100 {
                after_hit(&mut run, &h, &mut en, 10.0, h.tags.unwrap());
            }
            assert_eq!(en.dots.len(), 3);
            assert!(run.hp > 20.0);
            for dot in &mut en.dots {
                dot.left = 2.25;
                dot.tick = 0.6;
            }
        }
        let dead = world.get::<Enemy>(target).unwrap();
        for dot in &dead.dots {
            record_death(&mut run.monster_mods, Vec3::X, dot);
        }
        world.get_mut::<Enemy>(target).unwrap().dead = true;
        run.hp = 0.0;
        world.insert_resource(run);
        step(&mut world, 0.1);
        let received = &world.get::<Enemy>(next).unwrap().dots;
        assert_eq!(received.len(), 1);
        assert_eq!(received[0].left, 2.25);
        assert_eq!(received[0].tick, 0.6);
        assert!(
            world
                .query::<&Field>()
                .iter(&world)
                .any(|f| matches!(f.kind, Kind::Fire))
        );
    }
    #[derive(Resource)]
    struct Tick(bevy::ecs::system::SystemId);
    fn arena() -> (World, Entity) {
        let mut world = World::new();
        game::geometry::init_test_world(&mut world);
        world.init_resource::<Run>();
        world.init_resource::<Time>();
        world.init_resource::<game::Rng>();
        world.init_resource::<Messages<Hit>>();
        world.init_resource::<game::boss_spells::Warnings>();
        let hero = world
            .spawn((game::hero::Hero { vy: 0.0, air: 0.0 }, Transform::default()))
            .id();
        let system = world.register_system(tick);
        world.insert_resource(Tick(system));
        (world, hero)
    }
    fn step(world: &mut World, dt: f32) {
        world.resource_mut::<Run>().elapsed += dt;
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(dt));
        let system = world.resource::<Tick>().0;
        world.run_system(system).unwrap();
    }
    fn grant(run: &mut Run, modifier: Monster) {
        run.monster_mods
            .grant(modifier, run.elapsed, None, Vec3::ZERO);
    }
    fn enemy(world: &mut World, pos: Vec3) -> Entity {
        world
            .run_system_once(move |mut commands: Commands, mut run: ResMut<Run>| {
                game::enemies::spawn_enemy(
                    &mut commands,
                    &mut run,
                    &game::data::roster(0).enemies[0],
                    pos,
                    Vec3::ZERO,
                    false,
                )
            })
            .unwrap()
    }
    #[test]
    fn all_active_player_mods_are_unique_refreshable_visible_and_expire() {
        for modifier in Monster::MODS.into_iter().filter(|m| m.stealable()) {
            let mut run = Run::default();
            run.monster_mods.grant(modifier, 0.0, Some(5.0), Vec3::ZERO);
            let mut boss_mod = modifier;
            boss_mod.base = game::monster_mods::Base::Boss;
            run.monster_mods.grant(boss_mod, 0.0, Some(8.0), Vec3::ZERO);
            assert_eq!(run.monster_mods.rank(0.0).count(), 1);
            let visible = game::player_auras::collect(&run);
            assert_eq!(
                visible
                    .iter()
                    .filter(|a| a.name.starts_with("Monster modifier:"))
                    .count(),
                1,
                "{modifier:?}"
            );
            let buff = visible
                .iter()
                .find(|a| a.name.starts_with("Monster modifier:"))
                .unwrap();
            assert_eq!(buff.left, Some(8.0));
            assert!(!buff.description.is_empty());
            let copy: Vec<game::player_auras::Aura> =
                serde_json::from_str(&serde_json::to_string(&visible).unwrap()).unwrap();
            assert_eq!(copy.len(), visible.len());
            run.elapsed = 8.0;
            assert_eq!(run.monster_mods.rank(run.elapsed).count(), 0);
            assert!(
                game::player_auras::collect(&run)
                    .iter()
                    .all(|a| !a.name.starts_with("Monster modifier:"))
            );
        }
    }
    #[test]
    fn player_stats_and_native_spell_resolution_consume_mods_once() {
        let mut run = Run::default();
        let original = run.stats();
        let spell_def = game::data::ability("fireball");
        let base = run.resolve_spell(spell_def, 1);
        let chain_def = game::data::ability("chain_lightning");
        let base_chain = run.resolve_spell(chain_def, 1);
        for m in [
            Monster::Colossus,
            Monster::Sprinter,
            Monster::Frenzied,
            Monster::Regenerating,
            Monster::Precise,
            Monster::Devastating,
            Monster::Artillery,
            Monster::GreaterArtillery,
            Monster::Conductor,
            Monster::Gigantic,
            Monster::LongRange,
            Monster::Echo,
        ] {
            grant(&mut run, m);
            grant(&mut run, m);
        }
        let stats = run.stats();
        let spell = run.resolve_spell(spell_def, 1);
        assert!((stats.max_hp / original.max_hp - 2.5).abs() < 0.001);
        assert!((stats.speed / original.speed - 1.5 * 1.4).abs() < 0.001);
        assert!((stats.regen - stats.max_hp * 0.05).abs() < 0.001);
        assert!((stats.crit - original.crit - 0.15).abs() < 0.001);
        assert_eq!(spell.count, base.count + 6);
        assert_eq!(spell.chains, base.chains + 2);
        assert!((spell.radius / base.radius - 1.75_f32.sqrt()).abs() < 0.001 || base.radius == 0.0);
        assert!((spell.range / base.range - 1.4).abs() < 0.001);
        assert_eq!(spell.recast, base.recast + 1.0);
        assert!((spell.crit_multiplier - base.crit_multiplier - 0.75).abs() < 0.001);
        assert!(spell.cooldown < base.cooldown);
        assert_eq!(run.resolve_spell(chain_def, 1).count, base_chain.count + 2);
    }
    #[test]
    fn player_less_damage_and_adaptation_affect_real_incoming_hits_and_dots() {
        for (modifier, school) in [
            (Monster::Armoured, Tags::PHYSICAL),
            (Monster::Fireproof, Tags::FIRE),
            (Monster::Winter, Tags::COLD),
            (Monster::Insulated, Tags::NATURE),
            (Monster::Occult, Tags::SHADOW),
        ] {
            let mut baseline = Run {
                hp: 10000.0,
                ..default()
            };
            let ordinary = game::hero::hurt_hero_tagged(&mut baseline, 100.0, school | Tags::DOT);
            let mut run = Run {
                hp: 10000.0,
                ..default()
            };
            grant(&mut run, modifier);
            let taken = game::hero::hurt_hero_tagged(&mut run, 100.0, school | Tags::DOT);
            assert!((taken - ordinary * 0.8).abs() < 0.001, "{modifier:?}");
        }
        let mut run = Run {
            hp: 10000.0,
            ..default()
        };
        grant(&mut run, Monster::Adaptive);
        assert_eq!(
            game::hero::hurt_hero_tagged(&mut run, 100.0, Tags::FIRE),
            100.0
        );
        assert_eq!(
            game::hero::hurt_hero_tagged(&mut run, 100.0, Tags::FIRE | Tags::DOT),
            50.0
        );
        assert_eq!(
            game::hero::hurt_hero_tagged(&mut run, 100.0, Tags::COLD),
            100.0
        );
        assert_eq!(
            game::hero::hurt_hero_tagged(&mut run, 100.0, Tags::FIRE),
            100.0
        );
        grant(&mut run, Monster::Elusive);
        run.monster_mods.runtime.dodge_ready = true;
        assert_eq!(
            game::hero::hurt_hero_tagged(&mut run, 100.0, Tags::FIRE),
            0.0
        );
        assert!(!run.monster_mods.runtime.dodge_ready);
    }
    #[test]
    fn player_riposte_uses_native_block_and_consumes_one_melee_bonus() {
        let (mut world, _) = arena();
        let target = enemy(&mut world, Vec3::X);
        let mut run = Run {
            hp: 1000.0,
            ..default()
        };
        grant(&mut run, Monster::Riposte);
        assert_eq!(block(&run, 0.4), 0.75);
        assert_eq!(game::hero::hurt_hero_with_block(&mut run, 100.0, 0.49), 0.0);
        let mut extra = Vec::new();
        for expected in [40.0, 20.0] {
            let mut h = Hit::direct("heroic_strike", target, 20.0, false);
            h.tags = Some(Tags::PHYSICAL | Tags::MELEE | Tags::ATTACK);
            before_hit(
                &mut run,
                &mut h,
                world.get::<Enemy>(target).unwrap(),
                false,
                &mut extra,
            );
            assert_eq!(h.damage, expected);
        }
    }
    #[test]
    fn player_elemental_added_damage_keeps_school_and_never_recurses() {
        let (mut world, _) = arena();
        let target = enemy(&mut world, Vec3::X);
        let mut run = Run::default();
        for m in [
            Monster::Brutal,
            Monster::Pyromancer,
            Monster::Glacial,
            Monster::Storm,
            Monster::Profane,
        ] {
            grant(&mut run, m);
        }
        let mut h = Hit::direct("heroic_strike", target, 100.0, false);
        h.tags = Some(Tags::PHYSICAL | Tags::MELEE | Tags::ATTACK);
        let mut extra = vec![];
        before_hit(
            &mut run,
            &mut h,
            world.get::<Enemy>(target).unwrap(),
            false,
            &mut extra,
        );
        assert_eq!(h.damage, 140.0);
        assert_eq!(extra.len(), 4);
        for mut h in extra {
            assert_eq!(h.damage, 35.0);
            assert!(!h.tags.unwrap().contains(Tags::PHYSICAL));
            let mut copies = vec![];
            before_hit(
                &mut run,
                &mut h,
                world.get::<Enemy>(target).unwrap(),
                false,
                &mut copies,
            );
            assert!(copies.is_empty());
        }
    }
    #[test]
    fn player_auras_include_source_and_guests_without_stacking_and_blood_copy_uses_source_hp() {
        use game::multiplayer::{Companion, Companions, RemoteOwner};
        let (mut world, _) = arena();
        let guest = world
            .spawn((RemoteOwner(7), Transform::from_xyz(2.0, 0.0, 0.0)))
            .id();
        let mut party = Companions::default();
        let mut other = Run {
            owner: Some(7),
            hp: 20.0,
            equipment_hp: 10000.0,
            ..default()
        };
        grant(&mut other, Monster::Banner);
        party.0.insert(
            7,
            Companion {
                entity: guest,
                run: other,
                physics: game::hero::Hero { vy: 0.0, air: 0.0 },
                offer: default(),
                offer_id: 0,
            },
        );
        world.insert_resource(party);
        let maximum;
        {
            let mut run = world.resource_mut::<Run>();
            for m in [
                Monster::Banner,
                Monster::Protector,
                Monster::BloodLink,
                Monster::Healer,
            ] {
                grant(&mut run, m);
            }
            run.monster_mods.healer_next = 999.0;
            maximum = run.stats().max_hp;
            run.hp = 20.0;
        }
        step(&mut world, 6.0);
        let host = world.resource::<Run>();
        let guest = &world.resource::<Companions>().0[&7].run;
        assert_eq!(host.monster_mods.aura_speed, 1.2);
        assert_eq!(guest.monster_mods.aura_speed, 1.2);
        assert_eq!(host.monster_mods.protection, 0.25);
        assert_eq!(guest.monster_mods.protection, 0.25);
        assert!((host.hp - (20.0 + maximum * 0.5)).abs() < 0.001);
        assert!((guest.hp - (20.0 + maximum * 0.5)).abs() < 0.001);
        assert_eq!(pet_action(host, Vec3::X), 1.2);
        assert_eq!(pet_action(host, Vec3::X * 99.0), 1.0);
        let old_guest = guest.hp;
        step(&mut world, 0.1);
        assert!(
            (world.resource::<Companions>().0[&7].run.hp - old_guest - maximum * 0.5 * 0.3).abs()
                < 0.001
        );
        step(&mut world, 0.1);
        assert!(
            (world.resource::<Companions>().0[&7].run.hp - old_guest - maximum * 0.5 * 0.3).abs()
                < 0.001
        );
    }
    #[test]
    fn player_movement_mods_ignore_teleports_create_fire_and_discharge_when_stopping() {
        let (mut world, hero) = arena();
        {
            let mut run = world.resource_mut::<Run>();
            for m in [
                Monster::BurningWake,
                Monster::Electrostatic,
                Monster::Elusive,
                Monster::Bastion,
            ] {
                grant(&mut run, m);
            }
        }
        world.get_mut::<Transform>(hero).unwrap().translation.x = 100.0;
        step(&mut world, 0.1);
        assert_eq!(world.query::<&Field>().iter(&world).count(), 0);
        for i in 1..=4 {
            world.get_mut::<Transform>(hero).unwrap().translation.x = 100.0 + i as f32 * 2.0;
            step(&mut world, 0.5);
        }
        assert!(world.resource::<Run>().monster_mods.runtime.dodge_ready);
        assert!(
            world
                .query::<&Field>()
                .iter(&world)
                .any(|f| matches!(f.kind, Kind::Fire))
        );
        step(&mut world, 0.1);
        assert!(
            world
                .query::<&Field>()
                .iter(&world)
                .any(|f| matches!(f.kind, Kind::Electric))
        );
        step(&mut world, 4.0);
        assert_eq!(world.resource::<Run>().monster_mods.runtime.bastion, 1.0);
    }
    #[test]
    fn player_control_resistance_halves_duration_and_slow_strength() {
        let mut run = Run::default();
        let speed = run.stats().speed;
        grant(&mut run, Monster::Unstoppable);
        run.monster_mods.control(0.0, 4.0, 0.5, false);
        assert_eq!(run.monster_mods.control_until, 2.0);
        assert_eq!(run.stats().speed, speed * 0.75);
        run.elapsed = 2.0;
        assert_eq!(run.stats().speed, speed);
        run.monster_mods.control(2.0, 4.0, 1.0, true);
        assert_eq!(run.stats().speed, 0.0);
        assert_eq!(run.monster_mods.stunned_until, 4.0);
    }
}
