//! Deterministic spell calculations and target graphs. ECS, animation and visuals are adapters.
pub mod on_hit;
pub mod tags;
pub mod targeting;
use super::{
    data::{AbilityDef, Effect, Kind},
    Stats,
};
pub use tags::{Filter, Tags};

pub const MAX_TARGETS: usize = 64;
pub const RECAST_DELAY: f32 = 0.18;
/// Slower player spell pacing, applied after rank, recovery, attack speed and the cooldown floor.
pub const COOLDOWN_MULTIPLIER: f32 = 1.2;

#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(
    dead_code,
    reason = "Affix authoring API: item and talent sources opt into these mechanics."
)]
pub enum Change {
    FlatDamage(f32),
    FlatSpellDamage(f32),
    IncreasedDamage(f32),
    IncreasedEffect(f32),
    IncreasedBuffEffect(f32),
    MoreDamage(f32),
    IncreasedDotDamage(f32),
    MoreDotDamage(f32),
    AdditionalProjectiles(u32),
    AdditionalChains(u32),
    Fork(u32),
    AdditionalMeleeTargets(u32),
    ProjectileChance(f32),
    MeleeTargetChance(f32),
    ChainChance(f32),
    ForkChance(f32),
    IncreasedArea(f32),
    FlatRadius(f32),
    FlatRange(f32),
    IncreasedDuration(f32),
    IncreasedCooldownRecovery(f32),
    IncreasedAttackSpeed(f32),
    CriticalChance(f32),
    CriticalMultiplier(f32),
    RecastChance(f32),
    LifeLeech(f32),
    DamageAgainstBosses(f32),
    DamageAgainstFullLife(f32),
    DamageAgainstLowLife(f32),
    ArmourPenetration(f32),
    BleedChance(f32),
    IgniteChance(f32),
    ChillChance(f32),
    KnockbackChance(f32),
}
impl Change {
    pub fn scaled(self, rank: u64) -> Self {
        let r = rank as f32;
        let count = u32::try_from(rank).unwrap_or(u32::MAX);
        match self {
            Self::FlatDamage(v) => Self::FlatDamage(v * r),
            Self::ProjectileChance(v) => Self::ProjectileChance(v * r),
            Self::MeleeTargetChance(v) => Self::MeleeTargetChance(v * r),
            Self::ChainChance(v) => Self::ChainChance(v * r),
            Self::ForkChance(v) => Self::ForkChance(v * r),
            Self::FlatSpellDamage(v) => Self::FlatSpellDamage(v * r),
            Self::IncreasedDamage(v) => Self::IncreasedDamage(v * r),
            Self::IncreasedEffect(v) => Self::IncreasedEffect(v * r),
            Self::IncreasedBuffEffect(v) => Self::IncreasedBuffEffect(v * r),
            Self::MoreDamage(v) => Self::MoreDamage(v * r),
            Self::IncreasedDotDamage(v) => Self::IncreasedDotDamage(v * r),
            Self::MoreDotDamage(v) => Self::MoreDotDamage(v * r),
            Self::AdditionalProjectiles(v) => Self::AdditionalProjectiles(v.saturating_mul(count)),
            Self::AdditionalChains(v) => Self::AdditionalChains(v.saturating_mul(count)),
            Self::Fork(v) => Self::Fork(v.saturating_mul(count)),
            Self::AdditionalMeleeTargets(v) => {
                Self::AdditionalMeleeTargets(v.saturating_mul(count))
            }
            Self::IncreasedArea(v) => Self::IncreasedArea(v * r),
            Self::FlatRadius(v) => Self::FlatRadius(v * r),
            Self::FlatRange(v) => Self::FlatRange(v * r),
            Self::IncreasedDuration(v) => Self::IncreasedDuration(v * r),
            Self::IncreasedCooldownRecovery(v) => Self::IncreasedCooldownRecovery(v * r),
            Self::IncreasedAttackSpeed(v) => Self::IncreasedAttackSpeed(v * r),
            Self::CriticalChance(v) => Self::CriticalChance(v * r),
            Self::CriticalMultiplier(v) => Self::CriticalMultiplier(v * r),
            Self::RecastChance(v) => Self::RecastChance(v * r),
            Self::LifeLeech(v) => Self::LifeLeech(v * r),
            Self::DamageAgainstBosses(v) => Self::DamageAgainstBosses(v * r),
            Self::DamageAgainstFullLife(v) => Self::DamageAgainstFullLife(v * r),
            Self::DamageAgainstLowLife(v) => Self::DamageAgainstLowLife(v * r),
            Self::ArmourPenetration(v) => Self::ArmourPenetration(v * r),
            Self::BleedChance(v) => Self::BleedChance(v * r),
            Self::IgniteChance(v) => Self::IgniteChance(v * r),
            Self::ChillChance(v) => Self::ChillChance(v * r),
            Self::KnockbackChance(v) => Self::KnockbackChance(v * r),
        }
    }
    pub fn description(self) -> String {
        match self {
            Self::FlatDamage(v) => format!("+{v:.0} hit damage"),
            Self::ProjectileChance(v) => format!("+{:.0}% extra projectile chance", v * 100.0),
            Self::MeleeTargetChance(v) => format!("+{:.0}% extra melee target chance", v * 100.0),
            Self::ChainChance(v) => format!("+{:.0}% extra chain jump chance", v * 100.0),
            Self::ForkChance(v) => format!("+{:.0}% fork chance", v * 100.0),
            Self::FlatSpellDamage(v) => format!("+{v:.0} spell damage"),
            Self::IncreasedDamage(v) => format!("+{:.0}% increased damage", v * 100.0),
            Self::IncreasedEffect(v) => format!("+{:.0}% skill power", v * 100.0),
            Self::IncreasedBuffEffect(v) => format!("+{:.0}% buff effect", v * 100.0),
            Self::MoreDamage(v) => format!("{:+.0}% more damage", v * 100.0),
            Self::IncreasedDotDamage(v) => format!("+{:.0}% damage over time", v * 100.0),
            Self::MoreDotDamage(v) => format!("{:+.0}% more damage over time", v * 100.0),
            Self::AdditionalProjectiles(n) => format!("+{n} projectiles"),
            Self::AdditionalChains(n) => format!("+{n} chain jumps"),
            Self::Fork(n) => format!("+{n} fork generations"),
            Self::AdditionalMeleeTargets(n) => format!("+{n} melee targets"),
            Self::IncreasedArea(v) => format!("+{:.0}% area of effect", v * 100.0),
            Self::FlatRadius(v) => format!("+{v:.1} yd radius"),
            Self::FlatRange(v) => format!("+{v:.0} attack range (yd)"),
            Self::IncreasedDuration(v) => format!("+{:.0}% duration", v * 100.0),
            Self::IncreasedCooldownRecovery(v) => format!("+{:.0}% cooldown recovery", v * 100.0),
            Self::IncreasedAttackSpeed(v) => format!("+{:.0}% attack speed", v * 100.0),
            Self::CriticalChance(v) => format!("+{:.0}% critical strike chance", v * 100.0),
            Self::CriticalMultiplier(v) => format!("+{:.0}% critical strike multiplier", v * 100.0),
            Self::RecastChance(v) => format!("+{:.0}% repeat chance", v * 100.0),
            Self::LifeLeech(v) => format!("{:.1}% attack damage leeched as life", v * 100.0),
            Self::DamageAgainstBosses(v) => {
                format!("+{:.1}% more damage against bosses", v * 100.0)
            }
            Self::DamageAgainstFullLife(v) => {
                format!("+{:.1}% more damage against full-life enemies", v * 100.0)
            }
            Self::DamageAgainstLowLife(v) => {
                format!("+{:.1}% more damage against low-life enemies", v * 100.0)
            }
            Self::ArmourPenetration(v) => format!("{:.1}% physical armour penetration", v * 100.0),
            Self::BleedChance(v) => format!("{:.1}% chance to bleed", v * 100.0),
            Self::IgniteChance(v) => format!("{:.1}% chance to ignite", v * 100.0),
            Self::ChillChance(v) => format!("{:.1}% chance to chill", v * 100.0),
            Self::KnockbackChance(v) => format!("{:.1}% chance to knock back", v * 100.0),
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Modifier {
    /// Stable item/talent/passive identity: replace a source without accumulating stale rolls.
    pub source: u64,
    pub filter: Filter,
    pub change: Change,
}
#[derive(Default)]
pub struct Modifiers {
    entries: Vec<Modifier>,
}
impl Modifiers {
    pub fn entries(&self) -> &[Modifier] {
        &self.entries
    }
    pub fn replace_source(&mut self, source: u64, changes: impl IntoIterator<Item = Modifier>) {
        self.remove_source(source);
        self.entries.extend(changes.into_iter().map(|mut m| {
            m.source = source;
            m
        }));
    }
    pub fn remove_source(&mut self, source: u64) {
        self.entries.retain(|m| m.source != source);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PeriodicSnapshot {
    pub dps: f32,
    pub duration: f32,
    pub tags: Tags,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Resolved {
    pub effect: Effect,
    pub buff_effect: f32,
    /// Projectile, melee target, chain and fork chances; rolled once per cast.
    pub extra_chances: [f32; 4],
    pub triggered: bool,
    pub direct_dot: Option<PeriodicSnapshot>,
    pub dot_tags: Option<Tags>,
    /// Assigned by the authoritative cast adapter; zero denotes a triggered copy.
    pub cast_id: u64,
    pub primary_critical: bool,
    pub tags: Tags,
    pub dmg: f32,
    pub dot: f32,
    pub count: usize,
    pub radius: f32,
    pub duration: f32,
    pub cooldown: f32,
    pub range: f32,
    pub crit: f32,
    pub crit_multiplier: f32,
    pub chains: usize,
    pub forks: usize,
    pub link_range: f32,
    pub recast: f32,
}
fn finite(v: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        0.0
    }
}
fn bounded_damage(value: f32) -> f32 {
    if value.is_nan() {
        0.0
    } else {
        value.clamp(0.0, 1.0e12)
    }
}
pub fn resolve(def: &AbilityDef, level: u8, stats: &Stats, modifiers: &[Modifier]) -> Resolved {
    resolve_with_weapon_damage(def, level, stats, modifiers, 0.0)
}

pub fn resolve_with_weapon_damage(
    def: &AbilityDef,
    level: u8,
    stats: &Stats,
    modifiers: &[Modifier],
    weapon_damage: f32,
) -> Resolved {
    resolve_tagged(
        def,
        level,
        stats,
        modifiers,
        weapon_damage,
        tags::for_spell(def),
    )
}

pub(super) fn resolve_tagged(
    def: &AbilityDef,
    level: u8,
    stats: &Stats,
    modifiers: &[Modifier],
    weapon_damage: f32,
    tags: Tags,
) -> Resolved {
    let level = level.max(1);
    let rank = super::specialization::steps(level);
    let spell_shield = def.kind == Kind::Shield && tags.contains(Tags::SPELL);
    let mut increased = finite(stats.might) - 1.0;
    if tags.contains(Tags::DAMAGE) {
        increased += finite(stats.level_damage);
        if tags.contains(Tags::ATTACK) {
            increased += finite(stats.attack_power);
        }
        if tags.contains(Tags::SPELL) {
            increased += finite(stats.spell_power);
        }
    }
    if spell_shield && !tags.contains(Tags::DAMAGE) {
        increased += finite(stats.spell_power);
    }
    let (mut flat, mut more, mut dot_inc, mut dot_more) = (0.0, 1.0, 0.0, 1.0);
    if tags.contains(Tags::ATTACK) && def.damage > 0.0 {
        flat += finite(weapon_damage);
    }
    let (mut area, mut radius, mut duration, mut recovery) = (0.0, 0.0, 0.0, 0.0);
    let mut attack_range = 0.0;
    let mut attack_speed = if tags.contains(Tags::ATTACK) {
        finite(stats.attack_speed)
    } else {
        0.0
    };
    let (mut projectiles, mut melee, mut chains, mut forks) = (0u32, 0u32, 0u32, 0u32);
    let (mut crit, mut recast) = (finite(stats.crit), 0.0);
    let mut crit_multiplier = 2.0;
    let mut extra_chances = [0.0; 4];
    let mut buff_effect = 0.0;
    for modifier in modifiers.iter().filter(|m| {
        // Absorption uses spell/school power filters without becoming a damaging hit.
        let power_tags = if spell_shield
            && matches!(
                m.change,
                Change::IncreasedEffect(_)
                    | Change::FlatSpellDamage(_)
                    | Change::IncreasedDamage(_)
                    | Change::MoreDamage(_)
            ) {
            tags | Tags::DAMAGE
        } else {
            tags
        };
        m.filter.matches(def.key, power_tags)
    }) {
        use Change::*;
        match modifier.change {
            IncreasedBuffEffect(v) if tags.contains(Tags::BUFF) => buff_effect += finite(v),
            ProjectileChance(v) if tags.contains(Tags::PROJECTILE) => extra_chances[0] += finite(v),
            MeleeTargetChance(v)
                if tags.contains(Tags::MELEE)
                    && matches!(def.kind, Kind::Melee | Kind::Strike | Kind::Dot) =>
            {
                extra_chances[1] += finite(v)
            }
            ChainChance(v) if tags.intersects(Tags::PROJECTILE | Tags::CHAIN) => {
                extra_chances[2] += finite(v)
            }
            ForkChance(v) if tags.contains(Tags::PROJECTILE) => extra_chances[3] += finite(v),
            IncreasedEffect(v) => increased += finite(v),
            FlatDamage(v) if tags.contains(Tags::DAMAGE) && def.damage > 0.0 => flat += finite(v),
            FlatSpellDamage(v)
                if (tags.contains(Tags::SPELL | Tags::DAMAGE) || spell_shield)
                    && def.damage > 0.0 =>
            {
                flat += finite(v)
            }
            IncreasedDamage(v) if tags.contains(Tags::DAMAGE) || spell_shield => {
                increased += finite(v)
            }
            MoreDamage(v) if tags.contains(Tags::DAMAGE) || spell_shield => {
                more *= (1.0 + finite(v)).max(0.0)
            }
            IncreasedDotDamage(v) if tags.contains(Tags::DOT) => dot_inc += finite(v),
            MoreDotDamage(v) if tags.contains(Tags::DOT) => dot_more *= (1.0 + finite(v)).max(0.0),
            AdditionalProjectiles(n) if tags.contains(Tags::PROJECTILE) => {
                projectiles = projectiles.saturating_add(n)
            }
            AdditionalMeleeTargets(n)
                if tags.contains(Tags::MELEE)
                    && matches!(def.kind, Kind::Melee | Kind::Strike | Kind::Dot) =>
            {
                melee = melee.saturating_add(n)
            }
            AdditionalChains(n) if tags.intersects(Tags::PROJECTILE | Tags::CHAIN) => {
                chains = chains.saturating_add(n)
            }
            Fork(n) if tags.contains(Tags::PROJECTILE) => forks = forks.saturating_add(n),
            IncreasedArea(v) if tags.contains(Tags::AREA) => area += finite(v),
            FlatRadius(v) if tags.contains(Tags::AREA) => radius += finite(v),
            FlatRange(v)
                if tags.contains(Tags::ATTACK)
                    && def.range > 0.0
                    && matches!(
                        def.kind,
                        Kind::Bolt
                            | Kind::Strike
                            | Kind::Melee
                            | Kind::Chain
                            | Kind::Dot
                            | Kind::Ground
                            | Kind::Totem
                    ) =>
            {
                attack_range += finite(v)
            }
            IncreasedDuration(v) if tags.contains(Tags::DURATION) => duration += finite(v),
            IncreasedCooldownRecovery(v) => recovery += finite(v),
            IncreasedAttackSpeed(v) if tags.contains(Tags::ATTACK) => attack_speed += finite(v),
            CriticalMultiplier(v)
                if tags.contains(Tags::DAMAGE)
                    && !tags.contains(Tags::GUARD)
                    && def.kind != Kind::Dot =>
            {
                crit_multiplier += finite(v);
            }
            CriticalChance(v)
                if tags.contains(Tags::DAMAGE)
                    && !tags.contains(Tags::GUARD)
                    && def.kind != Kind::Dot =>
            {
                crit += finite(v)
            }
            RecastChance(v)
                if !tags.intersects(
                    Tags::GUARD | Tags::HEAL | Tags::TOTEM | Tags::AURA | Tags::BUFF,
                ) =>
            {
                recast += finite(v)
            }
            _ => {}
        }
    }
    let count = (def.count as usize
        + super::specialization::extra_target(def, level)
        + projectiles.min(32) as usize
        + melee.min(32) as usize)
        .clamp(1, MAX_TARGETS);
    // Run ranks grant flat power, recharge, AoE radius and DoT duration bonuses.
    let rank_damage = 5.0 * rank;
    let rank_radius = if tags.contains(Tags::AREA) {
        0.25 * rank
            + if level >= 6
                && def.kind != Kind::Buff
                && super::specialization::extra_target(def, level) == 0
            {
                super::specialization::SIGNATURE_RADIUS
            } else {
                0.0
            }
    } else {
        0.0
    };
    let rank_duration = if level >= 6
        && (def.kind == Kind::Immune || (def.kind == Kind::Thorns && def.damage == 0.0))
    {
        super::specialization::SIGNATURE_DURATION
    } else if tags.contains(Tags::DOT) && def.duration > 0.0 {
        0.5 * rank
    } else {
        0.0
    };
    // Area modifiers scale surface area, hence sqrt; legacy `stats.area` remains a radius bonus.
    let radius = (((def.radius + rank_radius) * stats.area + radius).max(0.0)
        * (1.0 + area).max(0.0).sqrt())
    .min(100.0);
    let signature_power = if level >= 6
        && !tags.contains(Tags::AREA)
        && matches!(
            def.kind,
            Kind::Shield | Kind::Heal | Kind::Thorns | Kind::Totem
        ) {
        super::specialization::SIGNATURE_POWER
    } else {
        1.0
    };
    Resolved {
        effect: ranked_effect(def.effect, rank, duration),
        buff_effect: buff_effect.max(0.0),
        extra_chances,
        triggered: false,
        direct_dot: None,
        dot_tags: None,
        cast_id: 0,
        primary_critical: false,
        tags,
        dmg: bounded_damage(
            (def.damage + if def.damage > 0.0 { rank_damage } else { 0.0 } + flat).max(0.0)
                * (1.0 + increased).max(0.0)
                * more
                * signature_power,
        ),
        dot: bounded_damage(
            (def.dot + if def.dot > 0.0 { rank_damage } else { 0.0 })
                * (1.0 + increased + dot_inc).max(0.0)
                * more
                * dot_more,
        ),
        count: if def.kind == Kind::Chain {
            (count + chains.min(16) as usize).min(32)
        } else {
            count
        },
        radius,
        duration: ((def.duration + rank_duration) * (1.0 + duration).max(0.0)).min(300.0),
        cooldown: (((def.cooldown - 0.1 * super::specialization::cooldown_steps(level)).max(0.0)
            * if def.key == "totemic_recall" && level >= 6 {
                0.75
            } else {
                1.0
            }
            * (1.0 - stats.haste).max(0.35))
            / ((1.0 + recovery).max(0.05) * (1.0 + attack_speed).max(0.05)))
        .max(0.2)
            * COOLDOWN_MULTIPLIER,
        range: (def.range + attack_range).clamp(0.0, def.range.max(100.0)),
        crit: crit.clamp(0.0, 1.0),
        crit_multiplier: crit_multiplier.clamp(1.0, 1.0e6),
        chains: ((if def.kind == Kind::Chain { count } else { 0 }) + chains.min(16) as usize)
            .min(32),
        forks: forks.min(3) as usize,
        link_range: if def.kind == Kind::Chain {
            radius
        } else {
            12.0
        },
        recast: recast.max(0.0),
    }
}

#[cfg(test)]
pub fn critical(damage: f32, chance: f32, roll: f32) -> (f32, bool) {
    critical_with_multiplier(damage, chance, 2.0, roll)
}
pub fn critical_with_multiplier(
    damage: f32,
    chance: f32,
    multiplier: f32,
    roll: f32,
) -> (f32, bool) {
    let crit = roll < chance.clamp(0.0, 1.0);
    (
        bounded_damage(damage.max(0.0) * if crit { multiplier.max(1.0) } else { 1.0 }),
        crit,
    )
}
/// Number of extra casts: whole hundreds are guaranteed, the remainder rolls once.
pub fn recast_count(chance: f32, roll: f32, repeated: bool) -> u64 {
    if repeated {
        return 0;
    }
    let chance = chance.max(0.0);
    (chance as u64).saturating_add(u64::from(roll < chance.fract()))
}

impl Resolved {
    pub fn roll_extras(&mut self, rolls: [f32; 4]) {
        let [projectiles, melee, chains, forks] = std::array::from_fn(|i| {
            recast_count(self.extra_chances[i], rolls[i], false).min(MAX_TARGETS as u64) as usize
        });
        self.count = (self.count + projectiles + melee).min(MAX_TARGETS);
        if self.tags.contains(Tags::CHAIN) {
            self.count = (self.count + chains).min(32);
        }
        self.chains = (self.chains + chains).min(32);
        self.forks = (self.forks + forks).min(3);
        self.extra_chances = [0.0; 4];
    }
}

/// One compact job per ability, even at very high repeat chance. Copies never enqueue copies.
#[derive(Default)]
pub struct Recasts(Vec<Repeat>);
struct Repeat {
    ability: usize,
    due: f32,
    remaining: u64,
    batches: std::collections::VecDeque<RepeatBatch>,
}
struct RepeatBatch {
    remaining: u64,
    next_ordinal: u64,
}
impl Recasts {
    pub fn schedule(&mut self, ability: usize, now: f32, count: u64) {
        if count == 0 {
            return;
        }
        if let Some(job) = self.0.iter_mut().find(|job| job.ability == ability) {
            let accepted = count.min(u64::MAX - job.remaining);
            if accepted > 0 {
                job.batches.push_back(RepeatBatch {
                    remaining: accepted,
                    next_ordinal: 1,
                });
                job.remaining += accepted;
            }
        } else {
            self.0.push(Repeat {
                ability,
                due: now + RECAST_DELAY,
                remaining: count,
                batches: std::collections::VecDeque::from([RepeatBatch {
                    remaining: count,
                    next_ordinal: 1,
                }]),
            });
        }
    }
    pub fn take_due_ordinal(&mut self, ability: usize, now: f32) -> Option<u64> {
        let Some(index) = self
            .0
            .iter()
            .position(|job| job.ability == ability && job.due <= now)
        else {
            return None;
        };
        let job = &mut self.0[index];
        let batch = job.batches.front_mut().expect("repeat job has a batch");
        let ordinal = batch.next_ordinal;
        batch.next_ordinal = batch.next_ordinal.saturating_add(1);
        batch.remaining -= 1;
        if batch.remaining == 0 {
            job.batches.pop_front();
        }
        job.remaining -= 1;
        if job.remaining == 0 {
            self.0.swap_remove(index);
        } else {
            job.due = now + RECAST_DELAY;
        }
        Some(ordinal)
    }
    #[cfg(test)]
    pub fn take_due(&mut self, ability: usize, now: f32) -> bool {
        self.take_due_ordinal(ability, now).is_some()
    }
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.0.len()
    }
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedMobility {
    pub tags: Tags,
    pub cooldown: f32,
    pub duration: f32,
    pub radius: f32,
}
pub fn resolve_mobility(
    def: &super::mobility::Mobility,
    modifiers: &[Modifier],
) -> ResolvedMobility {
    let (key, tags) = tags::mobility(def.spell).unwrap_or(("unknown", Tags::UTILITY));
    let (mut recovery, mut duration, mut area, mut radius) = (0.0, 0.0, 0.0, 0.0);
    for modifier in modifiers.iter().filter(|m| m.filter.matches(key, tags)) {
        match modifier.change {
            Change::IncreasedCooldownRecovery(v) => recovery += finite(v),
            Change::IncreasedDuration(v) if tags.contains(Tags::DURATION) => duration += finite(v),
            Change::IncreasedArea(v) if tags.contains(Tags::AREA) => area += finite(v),
            Change::FlatRadius(v) if tags.contains(Tags::AREA) => radius += finite(v),
            _ => {}
        }
    }
    let base_radius = if let super::mobility::Move::Scatter(r) = def.kind {
        r
    } else {
        0.0
    };
    ResolvedMobility {
        tags,
        cooldown: (def.cooldown / (1.0 + recovery).max(0.05)).max(0.2) * COOLDOWN_MULTIPLIER,
        duration: (def.secs * (1.0 + duration).max(0.0)).min(300.0),
        radius: ((base_radius + radius).max(0.0) * (1.0 + area).max(0.0).sqrt()).min(100.0),
    }
}
fn ranked_effect(effect: Effect, rank: f32, duration: f32) -> Effect {
    let time = |base: f32| ((base + 0.25 * rank) * (1.0 + duration).max(0.0)).clamp(0.0, 30.0);
    let reduction =
        |multiplier: f32| 1.0 - ((1.0 - multiplier) * (1.0 + 0.05 * rank)).clamp(0.0, 0.75);
    match effect {
        Effect::Slow(m, t) => Effect::Slow(reduction(m), time(t)),
        Effect::Weaken(m, t) => Effect::Weaken(reduction(m), time(t)),
        Effect::Root(t) => Effect::Root(time(t)),
        Effect::Stun(t) => Effect::Stun(time(t)),
        Effect::Fear(t) => Effect::Fear(time(t)),
        other => other,
    }
}
pub fn mark_duration(effect: Effect, duration: f32) -> f32 {
    match effect {
        Effect::Slow(_, t)
        | Effect::Weaken(_, t)
        | Effect::Root(t)
        | Effect::Stun(t)
        | Effect::Fear(t) => t,
        _ if duration > 0.0 => duration,
        _ => 3.0,
    }
}
pub fn damage_on_target(raw: f32, hp: f32, max_hp: f32, boss: bool, effect: Effect) -> f32 {
    match effect {
        Effect::Execute(t) if !boss && max_hp > 0.0 && hp / max_hp <= t => raw.max(hp),
        _ => raw.max(0.0),
    }
}
#[cfg(test)]
mod tests;
