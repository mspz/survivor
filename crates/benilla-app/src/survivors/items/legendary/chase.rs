//! Dungeon-exclusive chase uniques. State belongs to the owning run, including guests.
use super::*;
use crate::survivors::{
    data,
    enemies::Rank,
    spell_engine::{Change, Modifier},
    units::flat,
};
#[cfg(test)]
#[path = "chase_tests.rs"]
mod tests;

pub const HEAD_HUNTER: u16 = 401;
pub const GOSPEL: u16 = 402;
pub const PACT: u16 = 403;
pub const HOURGLASS: u16 = 404;
pub const SKIN: u16 = 405;
pub const LEGACY: u16 = 406;
pub const BLOOD: u16 = 407;
pub const HANDS: u16 = 408;
pub const OATH: u16 = 409;
pub const VISAGE: u16 = 410;
pub const CROWN: u16 = 411;
pub const BULWARK: u16 = 412;
pub const HUNGER: u16 = 413;
pub const TESTAMENT: u16 = 414;
pub const EXECUTIONER: u16 = 415;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub loot_weight: f64,
    pub steal_chance: f32,
    pub steal_duration: f32,
    pub pact_cost: f32,
    pub pact_damage: f32,
    pub stored_seconds: f32,
    pub stored_damage: f32,
    pub burst_interval: f32,
    pub blocked_storage: f32,
    pub reserve_spend: f32,
    pub life_conversion: f32,
    pub healing_damage: f32,
    pub healing_radius: f32,
    pub deferred_seconds: f32,
    pub spread_interval: f32,
    pub spread_radius: f32,
    pub spread_fraction: f32,
    pub isolated_dot_cap: f32,
    pub maximum_block: f32,
    pub hunger_linear_drain: f32,
    pub hunger_quadratic_drain: f32,
    pub hunger_damage: f32,
    pub power_capacity: f32,
    pub power_regen: f32,
    pub power_cost_per_second: f32,
    pub minimum_cast_interval: f32,
    pub cooldown_quadratic: f32,
}
const FILE: &str = include_str!("../../../../../../config/survivors-chase-uniques.toml");
pub fn config() -> &'static Config {
    static CONFIG: std::sync::LazyLock<Config> = std::sync::LazyLock::new(|| {
        let builtin = || toml::from_str(FILE).expect("valid embedded chase config");
        if cfg!(test) {
            return builtin();
        }
        crate::local_state::read_to_string(std::path::Path::new(
            "config/survivors-chase-uniques.toml",
        ))
        .ok()
        .and_then(|s| parse_config(&s))
        .unwrap_or_else(builtin)
    });
    &CONFIG
}
fn parse_config(text: &str) -> Option<Config> {
    let c: Config = toml::from_str(text).ok()?;
    let values = [
        c.steal_chance,
        c.steal_duration,
        c.pact_cost,
        c.pact_damage,
        c.stored_seconds,
        c.stored_damage,
        c.burst_interval,
        c.blocked_storage,
        c.reserve_spend,
        c.life_conversion,
        c.healing_damage,
        c.healing_radius,
        c.deferred_seconds,
        c.spread_interval,
        c.spread_radius,
        c.spread_fraction,
        c.isolated_dot_cap,
        c.maximum_block,
        c.hunger_linear_drain,
        c.hunger_quadratic_drain,
        c.hunger_damage,
        c.power_capacity,
        c.power_regen,
        c.power_cost_per_second,
        c.minimum_cast_interval,
        c.cooldown_quadratic,
    ];
    (values
        .iter()
        .all(|v| v.is_finite() && *v >= 0.0 && *v <= 10000.0)
        && c.loot_weight.is_finite()
        && (0.0..=1.0).contains(&c.loot_weight)
        && c.steal_chance <= 1.0
        && c.pact_cost <= 1.0
        && c.reserve_spend <= 1.0
        && c.maximum_block <= 1.0
        && c.deferred_seconds > 0.0
        && c.spread_interval > 0.0
        && c.minimum_cast_interval > 0.0
        && c.burst_interval > 0.0
        && c.power_capacity > 0.0
        && c.stored_seconds > 0.0)
        .then_some(c)
}
pub fn dungeon(id: u16) -> Option<usize> {
    Some(match id {
        SKIN | BULWARK => 17,
        HEAD_HUNTER | BLOOD => 18,
        HUNGER | CROWN => 19,
        HOURGLASS | LEGACY | VISAGE => 20,
        PACT | TESTAMENT | EXECUTIONER => 21,
        GOSPEL | HANDS | OATH => 22,
        _ => return None,
    })
}
pub fn weight(id: u16, map: Option<usize>) -> f64 {
    match dungeon(id) {
        Some(required) if map == Some(required) => config().loot_weight,
        Some(_) => 0.0,
        None => 1.0,
    }
}
#[derive(Default)]
pub struct State {
    pub block_serial: u64,
    pub reserve: f32,
    pub hunger: f32,
    pub power: Option<f32>,
    pub deferred: Vec<Debt>,
    pub healing: Vec<(Option<Entity>, f32)>,
    pub stored: BTreeMap<usize, (f32, f32, u32)>, // cooldown progress, next burst, uses
    pub next_spread: f32,
}
pub struct Debt {
    left: f32,
    dps: f32,
    source: Option<crate::survivors::death_log::Source>,
    tags: Tags,
    ability: &'static str,
}
pub fn offensive(def: &AbilityDef) -> bool {
    crate::survivors::spell_engine::tags::for_spell(def).contains(Tags::DAMAGE)
}
pub fn power_skill(def: &AbilityDef) -> bool {
    let tags = crate::survivors::spell_engine::tags::for_spell(def);
    offensive(def)
        && !tags.intersects(Tags::DOT | Tags::BUFF | Tags::MINION)
        && def.dot <= 0.0
        && def.kind != Kind::Ground
}
pub fn power_skill_for(run: &Run, def: &AbilityDef) -> bool {
    power_skill(def)
        && !(run.legendary.has(70) && def.damage > 0.0)
        && !super::profile(run, def)
            .1
            .intersects(Tags::DOT | Tags::BUFF)
}
pub fn increased(change: Change) -> Option<f32> {
    match change {
        Change::IncreasedDamage(v) | Change::IncreasedDotDamage(v) => Some(v),
        _ => None,
    }
}
pub fn stats(run: &Run, stats: &mut crate::survivors::Stats) {
    if !run.legendary.has(BLOOD) {
        return;
    }
    let sum = (stats.might - 1.0)
        + stats.level_damage
        + stats.attack_power
        + stats.spell_power
        + run
            .spell_modifiers
            .entries()
            .iter()
            .filter_map(|m| increased(m.change))
            .sum::<f32>();
    stats.max_hp *= (1.0 + config().life_conversion * sum).max(0.05);
    stats.might = 1.0;
    stats.level_damage = 0.0;
    stats.attack_power = 0.0;
    stats.spell_power = 0.0;
}
pub fn modifiers<'a>(
    run: &'a Run,
    def: &AbilityDef,
    tags: Tags,
) -> std::borrow::Cow<'a, [Modifier]> {
    if !run.legendary.has(BLOOD)
        && (!offensive(def)
            || ![PACT, SKIN, HUNGER, GOSPEL]
                .iter()
                .any(|id| run.legendary.has(*id)))
    {
        return std::borrow::Cow::Borrowed(run.spell_modifiers.entries());
    }
    let mut out: Vec<_> = run
        .spell_modifiers
        .entries()
        .iter()
        .copied()
        .filter(|m| !run.legendary.has(BLOOD) || increased(m.change).is_none())
        .collect();
    let mut add = |change| {
        out.push(Modifier {
            source: 0x7c00_0000,
            filter: Filter::default(),
            change,
        })
    };
    if offensive(def) {
        if run.legendary.has(PACT) {
            add(Change::FlatDamage(
                run.stats().max_hp * config().pact_damage,
            ));
        }
        if run.legendary.has(SKIN) {
            add(Change::FlatDamage(
                run.legendary.chase.reserve * config().reserve_spend,
            ));
        }
        if run.legendary.has(HUNGER) {
            add(Change::MoreDamage(
                run.legendary.chase.hunger / run.stats().max_hp.max(1.0) * config().hunger_damage,
            ));
        }
        if run.legendary.has(GOSPEL) {
            add(Change::MoreDamage(2.0));
            if tags.contains(Tags::AREA) {
                add(Change::MoreArea(0.5));
            } else if tags.contains(Tags::DOT) { /* periodic adapter */
            } else if tags.contains(Tags::PROJECTILE) {
                add(Change::Fork(3));
            } else {
                add(Change::AdditionalMeleeTargets(2));
            }
        }
    }
    std::borrow::Cow::Owned(out)
}
pub fn prepare_profile(run: &Run, def: &mut AbilityDef) {
    if !offensive(def) || def.dot <= 0.0 {
        return;
    }
    let mut added = 0.0;
    if run.legendary.has(PACT) {
        added += run.stats().max_hp * config().pact_damage;
    }
    if run.legendary.has(SKIN) {
        added += run.legendary.chase.reserve * config().reserve_spend;
    }
    def.dot += added / def.duration.max(1.0);
}
pub fn resolved(run: &Run, def: &AbilityDef, p: &mut crate::survivors::spell_engine::Resolved) {
    if run.legendary.has(GOSPEL)
        && offensive(def)
        && !p.tags.intersects(Tags::AREA | Tags::DOT | Tags::PROJECTILE)
    {
        p.count = p.count.max(3);
    }
    if run.legendary.has(EXECUTIONER) && offensive(def) {
        let cd = if run.legendary.has(TESTAMENT) && power_skill_for(run, def) {
            0.0
        } else {
            p.cooldown
        };
        let mult = 1.0 + cd + config().cooldown_quadratic * cd * cd;
        p.dmg *= mult;
        p.dot *= mult;
        if let Some(dot) = &mut p.direct_dot {
            dot.dps *= mult;
        }
    }
}
pub fn can_pay(run: &Run, def: &AbilityDef, cooldown: f32) -> bool {
    (!run.legendary.has(PACT) || run.hp > run.stats().max_hp * config().pact_cost)
        && (!run.legendary.has(TESTAMENT)
            || !power_skill_for(run, def)
            || run.legendary.chase.power.unwrap_or(config().power_capacity) >= power_cost(cooldown))
}
fn power_cost(cooldown: f32) -> f32 {
    (cooldown * config().power_cost_per_second)
        .max(1.0)
        .min(config().power_capacity)
}
pub fn cast(run: &mut Run, def: &AbilityDef, cooldown: f32, life_cost: f32) {
    if run.legendary.has(PACT) {
        run.hp -= life_cost;
    }
    if run.legendary.has(SKIN) {
        run.legendary.chase.reserve *= 1.0 - config().reserve_spend;
    }
    if run.legendary.has(TESTAMENT) && power_skill_for(run, def) {
        let power = run
            .legendary
            .chase
            .power
            .get_or_insert(config().power_capacity);
        *power = (*power - power_cost(cooldown)).max(0.0);
    }
}
pub fn blocked(run: &mut Run, raw: f32) {
    run.legendary.chase.block_serial = run.legendary.chase.block_serial.wrapping_add(1);
    if run.legendary.has(SKIN) {
        run.legendary.chase.reserve += raw.max(0.0) * config().blocked_storage;
    }
    if run.legendary.has(BULWARK) {
        if let Some(skill) = run
            .abilities
            .iter_mut()
            .filter(|o| o.cd > 0.0 && offensive(o.def))
            .max_by(|a, b| a.cd.total_cmp(&b.cd))
        {
            skill.cd = 0.0;
        }
    }
}
pub fn equip(run: &mut Run) {
    if !run.legendary.has(SKIN) {
        run.legendary.chase.reserve = 0.0;
    }
    if !run.legendary.has(HUNGER) {
        run.legendary.chase.hunger = 0.0;
    }
    if !run.legendary.has(TESTAMENT) {
        run.legendary.chase.power = None;
    }
    if !run.legendary.has(HOURGLASS) {
        run.legendary.chase.stored.clear();
    }
    // Deferred damage is deliberately retained until paid, even without Oath.
}
pub fn steal(run: &mut Run, rank: Rank, rng: &mut crate::survivors::Rng, pos: Vec3) {
    if !run.legendary.has(HEAD_HUNTER) || rank.count() == 0 || rng.f32() >= config().steal_chance {
        return;
    }
    let pool: Vec<_> = Rank::MODS.into_iter().filter(|m| rank.has(*m) && m.stealable()).collect();
    if pool.is_empty() { return; }
    let modifier = pool[rng.index(pool.len())];
    let increased: f32 = run.spell_modifiers.entries().iter()
        .filter(|m| m.filter.matches("head_hunter", Tags::BUFF | Tags::DURATION))
        .filter_map(|m| match m.change { Change::IncreasedDuration(v) if v.is_finite() => Some(v), _ => None })
        .sum();
    let duration = (config().steal_duration * (1.0 + increased).max(0.0)).min(300.0);
    run.monster_mods
        .grant(modifier, run.elapsed, Some(duration), pos);
}
pub fn defer(run: &mut Run, damage: f32, tags: Tags) -> bool {
    if !run.legendary.has(OATH) || damage <= 0.0 {
        return false;
    }
    run.legendary.chase.deferred.push(Debt {
        left: config().deferred_seconds,
        dps: damage / config().deferred_seconds,
        source: run.death_log.context.clone(),
        tags,
        ability: run.taken_source,
    });
    true
}
pub fn advance(run: &mut Run, dt: f32) {
    if run.dead_t.is_some() || run.hp <= 0.0 || dt <= 0.0 {
        return;
    }
    if run.legendary.has(TESTAMENT) {
        let mut recovery = run.stats().haste;
        recovery += run
            .spell_modifiers
            .entries()
            .iter()
            .filter_map(|m| match m.change {
                Change::IncreasedCooldownRecovery(v) => Some(v),
                _ => None,
            })
            .sum::<f32>();
        let power = run
            .legendary
            .chase
            .power
            .get_or_insert(config().power_capacity);
        *power = (*power + dt * config().power_regen * (1.0 + recovery).max(0.05))
            .min(config().power_capacity);
    } else {
        run.legendary.chase.power = None;
    }
    if !run.legendary.has(SKIN) {
        run.legendary.chase.reserve = 0.0;
    }
    if run.legendary.has(HUNGER) {
        let max = run.stats().max_hp.max(1.0);
        let h = run.legendary.chase.hunger / max;
        // Exact positive solution for dh/dt = -a*h - b*h², independent of frame size.
        let e = (-config().hunger_linear_drain * dt).exp();
        let a = config().hunger_linear_drain;
        let value = if a > 0.0 {
            h * e / (1.0 + config().hunger_quadratic_drain * h / a * (1.0 - e))
        } else {
            h / (1.0 + config().hunger_quadratic_drain * h * dt)
        };
        run.legendary.chase.hunger = value * max;
    } else {
        run.legendary.chase.hunger = 0.0;
    }
    // Debt survives unequipping. Direct payment bypasses mitigation and hit triggers.
    let mut debts = std::mem::take(&mut run.legendary.chase.deferred);
    for debt in &mut debts {
        let damage = dt.min(debt.left) * debt.dps;
        debt.left -= dt;
        let previous = std::mem::replace(&mut run.death_log.context, debt.source.clone());
        run.death_log.record(
            run.elapsed,
            debt.source
                .as_ref()
                .and_then(|s| s.id)
                .map(Entity::from_bits),
            debt.ability,
            debt.tags | Tags::DOT,
            damage,
            damage,
            0.0,
            run.hp,
            crate::survivors::death_log::Outcome::Hit,
        );
        run.death_log.context = previous;
        run.taken_meter.record_hit(
            "Oath of the Final Second",
            run.elapsed,
            damage.min(run.hp.max(0.0)),
            false,
            true,
        );
        run.hp -= damage;
    }
    debts.retain(|d| d.left > 0.0);
    run.legendary.chase.deferred = debts;
}
pub fn queue_heal(run: &mut Run, recipient: Option<Entity>, amount: f32) {
    if run.legendary.has(HANDS) && amount > 0.0 && run.legendary.chase.healing.len() < 1024 {
        run.legendary
            .chase
            .healing
            .push((recipient, amount * config().healing_damage));
    }
}
pub fn visible_buffs(run: &Run) -> Vec<crate::survivors::player_auras::Aura> {
    let mut out = Vec::new();
    let mut add = |id, text: String| {
        let d = definition(id).unwrap();
        out.push(crate::survivors::player_auras::Aura::buff(
            12880, d.name, text, None,
        ));
    };
    if run.legendary.has(SKIN) {
        add(
            SKIN,
            format!("Stored blocked damage: {:.0}", run.legendary.chase.reserve),
        );
    }
    if run.legendary.has(HUNGER) {
        add(
            HUNGER,
            format!(
                "Stored excess Leech: {:.0}\n{:.1}% more Damage",
                run.legendary.chase.hunger,
                run.legendary.chase.hunger / run.stats().max_hp.max(1.0)
                    * config().hunger_damage
                    * 100.0
            ),
        );
    }
    if run.legendary.has(TESTAMENT) {
        add(
            TESTAMENT,
            format!(
                "Power: {:.0} / {:.0}",
                run.legendary.chase.power.unwrap_or(config().power_capacity),
                config().power_capacity
            ),
        );
    }
    if !run.legendary.chase.deferred.is_empty() {
        add(
            OATH,
            format!(
                "Remaining deferred Damage: {:.0}",
                run.legendary
                    .chase
                    .deferred
                    .iter()
                    .map(|d| d.left * d.dps)
                    .sum::<f32>()
            ),
        );
    }
    if run.legendary.has(HOURGLASS) {
        add(
            HOURGLASS,
            format!(
                "Stored uses: {}",
                run.legendary
                    .chase
                    .stored
                    .values()
                    .map(|s| s.2)
                    .sum::<u32>()
            ),
        );
    }
    out
}

/// Shared ECS adapter also runs for companions because cast_abilities swaps their Run.
pub fn flush_heals(
    run: &mut Run,
    pos: Vec3,
    positions: impl Fn(Entity) -> Option<Vec3>,
    foes: &[(Entity, Vec3)],
    hits: &mut MessageWriter<crate::survivors::abilities::Hit>,
) {
    for (recipient, raw) in std::mem::take(&mut run.legendary.chase.healing) {
        let center = recipient.and_then(&positions).unwrap_or(pos);
        for &(entity, at) in foes {
            if flat(at - center).length() <= config().healing_radius {
                let mut hit =
                    crate::survivors::abilities::Hit::direct("flash_heal", entity, raw, false);
                hit.owner = run.owner;
                hit.tags = Some(Tags::HOLY | Tags::SPELL | Tags::AREA | Tags::DAMAGE);
                hit.origin = events::Origin::Legendary;
                hit.triggered = true;
                hit.proc_resolved = true;
                hit.proc_admitted = true;
                hit.item_source = Some(HANDS);
                hit.proc_chain = u64::MAX;
                hit.meter_source = Some("Hands of Sacrilege");
                hits.write(hit);
            }
        }
    }
}

pub fn spread(
    mut run: ResMut<Run>,
    mut players: Option<ResMut<crate::survivors::multiplayer::Companions>>,
    mut enemies: Query<(Entity, &Transform, &mut crate::survivors::enemies::Enemy)>,
    areas: Query<(&Transform, &crate::survivors::abilities::GroundArea)>,
) {
    let now = run.elapsed;
    let mut owners = Vec::new();
    let mut visit = |r: &mut Run| {
        if r.pending_levels > 0 || r.dead_t.is_some() || r.legendary.chase.next_spread > now {
            return;
        }
        if r.legendary.has(CROWN) || r.legendary.has(GOSPEL) {
            r.legendary.chase.next_spread = now + config().spread_interval;
            owners.push((r.owner, r.legendary.has(CROWN), r.legendary.has(GOSPEL)));
        }
    };
    visit(&mut run);
    if let Some(players) = players.as_deref_mut() {
        for p in players.0.values_mut() {
            visit(&mut p.run);
        }
    }
    if owners.is_empty() {
        return;
    }
    let live: Vec<_> = enemies
        .iter()
        .filter(|(_, _, e)| !e.dead && e.hp > 0.0)
        .map(|(id, tf, _)| (id, tf.translation))
        .collect();
    let mut snapshots = Vec::new();
    for (id, tf, mut enemy) in &mut enemies {
        if enemy.dead {
            continue;
        }
        for dot in &mut enemy.dots {
            let Some(&(_, crown, gospel)) = owners.iter().find(|(owner, _, _)| *owner == dot.owner)
            else {
                continue;
            };
            let Some(def) = data::ABILITIES.iter().find(|d| d.key == dot.skill) else {
                continue;
            };
            let tags = crate::survivors::spell_engine::tags::for_spell(def);
            if dot.origin != events::Origin::Skill
                || !(def.kind == Kind::Dot || def.dot > 0.0 || def.kind == Kind::Ground)
                || (!crown && !(gospel && tags.contains(Tags::DOT) && !tags.contains(Tags::AREA)))
            {
                continue;
            }
            if dot.meta.chase_base <= 0.0 {
                dot.meta.chase_base = dot.dps;
            }
            snapshots.push((id, tf.translation, dot.clone(), crown));
        }
    }
    for (tf, area) in &areas {
        let Some((dot, radius)) = area.chase_seed() else {
            continue;
        };
        if !owners
            .iter()
            .any(|(owner, crown, _)| *owner == dot.owner && *crown)
        {
            continue;
        }
        for &(id, pos) in &live {
            if flat(pos - tf.translation).length() <= radius {
                snapshots.push((id, pos, dot.clone(), true));
            }
        }
    }
    // One strongest transfer per owner/skill/destination per pulse prevents dense packs
    // from creating quadratic numbers of instances. Copies remain eligible next pulse.
    let mut transfers: BTreeMap<
        (Entity, Option<u64>, &'static str),
        (crate::survivors::enemies::Dot, bool),
    > = BTreeMap::new();
    for (source, pos, mut dot, crown) in snapshots {
        let neighbour = live
            .iter()
            .filter(|(id, at)| *id != source && flat(*at - pos).length() <= config().spread_radius)
            .min_by(|a, b| {
                a.1.distance_squared(pos)
                    .total_cmp(&b.1.distance_squared(pos))
                    .then(a.0.cmp(&b.0))
            });
        let isolated = neighbour.is_none();
        if isolated && !crown {
            continue;
        }
        let target = neighbour.map_or(source, |n| n.0);
        dot.dps *= config().spread_fraction;
        let key = (target, dot.owner, dot.skill);
        if transfers.get(&key).is_none_or(|(old, _)| old.dps < dot.dps) {
            transfers.insert(key, (dot, isolated));
        }
    }
    for ((target, _, _), (dot, isolated)) in transfers {
        let Ok((_, _, mut enemy)) = enemies.get_mut(target) else {
            continue;
        };
        if let Some(old) = enemy.dots.iter_mut().find(|old| {
            old.owner == dot.owner && old.skill == dot.skill && old.origin == events::Origin::Skill
        }) {
            if isolated {
                old.dps = (old.dps + dot.dps)
                    .min(old.meta.chase_base.max(dot.meta.chase_base) * config().isolated_dot_cap);
            } else {
                old.dps = old.dps.max(dot.dps);
            }
        } else if dot.left > 0.0 {
            enemy.dots.push(dot);
        }
    }
}
