//! Stateful item combat effects. The run clock owns buffs/ICDs, so inspection pauses them.
use super::super::{
    abilities::{Hit, PendingHits},
    data::{self, AbilityDef, Kind},
    spell_engine::{
        self as engine,
        targeting::{self, Target},
    },
    units::{self, Fx},
};
use super::*;
const PROC_SOURCE: u64 = 0x4000_0000;
#[derive(Clone)]
pub(super) struct EquippedProc {
    pub item: u64,
    pub index: usize,
    pub chance: f32,
    pub filter: Filter,
    pub roll: ProcRoll,
}
#[derive(Clone)]
struct Event {
    skill: &'static str,
    tags: Tags,
    equipped: Vec<(u64, usize)>,
}
struct Buff {
    item: u64,
    index: usize,
    until: f32,
    change: Change,
}
#[derive(Default)]
pub(in crate::survivors) struct Runtime {
    procs: Vec<EquippedProc>,
    cooldowns: BTreeMap<(u64, usize), f32>,
    buffs: Vec<Buff>,
    events: Vec<Event>,
    leech_window: f32,
    leech_used: f32,
}
pub(super) fn equip(run: &mut Run, procs: Vec<EquippedProc>) {
    run.item_runtime.buffs.retain(|buff| {
        procs
            .iter()
            .any(|p| p.item == buff.item && p.index == buff.index)
    });
    run.item_runtime.procs = procs;
    sync_buffs(run);
}
fn sync_buffs(run: &mut Run) {
    let modifiers: Vec<_> = run
        .item_runtime
        .buffs
        .iter()
        .map(|buff| Modifier {
            source: PROC_SOURCE,
            filter: Filter::default(),
            change: buff.change,
        })
        .collect();
    run.replace_spell_modifiers(PROC_SOURCE, modifiers);
}
#[cfg(test)]
pub(in crate::survivors) fn record_hit(run: &mut Run, skill: &'static str, tags: Tags) {
    record_hit_from(run, skill, tags, 0);
}
pub(in crate::survivors) fn record_hit_from(
    run: &mut Run,
    skill: &'static str,
    tags: Tags,
    cast: u64,
) {
    if run.item_runtime.events.len() < 256 && !run.item_runtime.procs.is_empty() {
        let equipped = run
            .item_runtime
            .procs
            .iter()
            .filter(|p| {
                !run.legendary.has(279)
                    || !tags.contains(Tags::MELEE | Tags::ATTACK)
                    || run.legendary.weapon_items.get(&p.item).is_none_or(|hand| {
                        run.legendary
                            .hands
                            .get(&cast)
                            .is_none_or(|used| used == hand)
                    })
            })
            .map(|p| (p.item, p.index))
            .collect();
        run.item_runtime.events.push(Event {
            skill,
            tags,
            equipped,
        });
    }
}
pub(in crate::survivors) fn leech(run: &mut Run, raw: f32) -> f32 {
    if raw <= 0.0 {
        return 0.0;
    }
    let window = run.elapsed.floor();
    if run.item_runtime.leech_window != window {
        run.item_runtime.leech_window = window;
        run.item_runtime.leech_used = 0.0;
    }
    let available = (run.stats().max_hp * 0.20 - run.item_runtime.leech_used).max(0.0);
    let recovery = (raw * (1.0 + run.equipment.healing))
        .max(0.0)
        .min(available)
        .min((run.stats().max_hp - run.hp).max(0.0));
    if run.hp <= 0.0 || run.dead_t.is_some() {
        return 0.0;
    }
    run.hp += recovery;
    run.item_runtime.leech_used += recovery;
    recovery
}
/// Normalize first to keep large finite configuration weights and luck from overflowing.
pub(in crate::survivors) fn lucky_weights(weights: [f64; 4], luck: f32) -> [f64; 4] {
    if !luck.is_finite() || luck <= 0.0 {
        return weights;
    }
    let largest = weights.into_iter().fold(0.0, f64::max);
    if largest <= 0.0 {
        return weights;
    }
    let factor = 1.0 + f64::from(luck.min(1000.0));
    std::array::from_fn(|i| weights[i] / largest * factor.powi(i as i32))
}
pub(super) fn proc_spell(key: &str) -> Option<&'static AbilityDef> {
    data::ABILITIES.iter().find(|d| {
        d.key == key
            && d.damage > 0.0
            && matches!(
                d.kind,
                Kind::Bolt | Kind::Strike | Kind::Melee | Kind::Nova | Kind::Cone | Kind::Chain
            )
    })
}
pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        tick.after(super::super::enemies::move_enemies)
            .before(super::super::abilities::cast_abilities)
            .run_if(super::super::multiplayer::simulating)
            .run_if(super::super::multiplayer::authoritative),
    );
}
pub(in crate::survivors) fn tick(
    mut run: ResMut<Run>,
    time: Res<Time>,
    mut rng: ResMut<Rng>,
    hero: Query<(Entity, &Transform), With<Hero>>,
    enemies: Query<(Entity, &Transform, &super::super::enemies::Enemy)>,
    spells: Option<Res<crate::ui_action::Spells>>,
    mut fx: Fx,
    mut pending: ResMut<PendingHits>,
    mut hits: MessageWriter<Hit>,
) {
    if time.delta_secs() <= 0.0 || run.dead_t.is_some() || run.pending_levels > 0 {
        return;
    }
    let now = run.elapsed;
    let old = run.item_runtime.buffs.len();
    run.item_runtime.buffs.retain(|buff| buff.until > now);
    let mut changed = old != run.item_runtime.buffs.len();
    let events = std::mem::take(&mut run.item_runtime.events);
    let procs = run.item_runtime.procs.clone();
    let mut casts = Vec::new();
    for event in events {
        for proc in &procs {
            let key = (proc.item, proc.index);
            if !event.equipped.contains(&key)
                || !proc.filter.matches_hit(event.skill, event.tags)
                || run
                    .item_runtime
                    .cooldowns
                    .get(&key)
                    .is_some_and(|&at| at > now)
                || rng.f32() >= proc.chance.clamp(0.0, 1.0)
            {
                continue;
            }
            run.item_runtime
                .cooldowns
                .insert(key, now + proc.roll.cooldown);
            let change = match proc.roll.kind {
                ProcKind::Damage => Some(Change::IncreasedDamage(proc.roll.power)),
                ProcKind::CritChance => Some(Change::CriticalChance(proc.roll.power)),
                ProcKind::AttackSpeed => Some(Change::IncreasedAttackSpeed(proc.roll.power)),
                ProcKind::ClassSpell => None,
            };
            if let Some(change) = change {
                run.item_runtime
                    .buffs
                    .retain(|b| b.item != proc.item || b.index != proc.index);
                run.item_runtime.buffs.push(Buff {
                    item: proc.item,
                    index: proc.index,
                    until: now + proc.roll.duration,
                    change,
                });
                changed = true;
                run.banner = Some((
                    format!(
                        "Trinket proc: {} ({:.0}s)",
                        change.description(),
                        proc.roll.duration
                    ),
                    2.0,
                ));
            } else if let Some(def) = proc.roll.spell.as_deref().and_then(proc_spell) {
                casts.push((def, proc.roll.power));
            }
        }
    }
    if changed {
        sync_buffs(&mut run);
    }
    // Stat-only procs and ordinary frames do not need another enemy sort/target graph.
    if casts.is_empty() {
        return;
    }
    let Some((caster, hero)) = hero.iter().next() else {
        return;
    };
    let mut foes: Vec<_> = enemies
        .iter()
        .filter(|(_, _, e)| !e.dead && e.hp > 0.0)
        .map(|(e, tf, en)| Target {
            e,
            pos: tf.translation,
            hp: en.hp,
            dist: super::super::hero::reach(hero.translation, tf.translation),
            radius: en.radius,
        })
        .collect();
    foes.sort_by(|a, b| a.dist.total_cmp(&b.dist));
    for (def, power) in casts {
        let level = run
            .abilities
            .iter()
            .find(|o| o.def.key == def.key)
            .map_or(1, |o| o.level);
        let mut params = run.resolve_spell_for_cast(def, level, &mut rng);
        params.dmg *= power;
        params.dot *= power;
        let speed = spells
            .as_ref()
            .and_then(|s| s.catalog.get(def.spell))
            .map_or(0.0, |s| s.speed);
        let area = matches!(def.kind, Kind::Nova | Kind::Cone);
        let targets = if area {
            let mut targets = targeting::around(&foes, hero.translation, params.radius);
            if def.kind == Kind::Cone {
                let forward = foes.first().map_or(Vec3::NEG_Z, |f| {
                    units::flat(f.pos - hero.translation).normalize_or_zero()
                });
                targets.retain(|f| {
                    units::flat(f.pos - hero.translation)
                        .normalize_or_zero()
                        .dot(forward)
                        >= 0.5
                });
            }
            targets
        } else {
            targeting::nearest(
                &foes,
                params.range,
                if def.kind == Kind::Chain {
                    1
                } else {
                    params.count
                },
            )
        };
        if targets.is_empty() {
            continue;
        }
        fx.cast(
            caster,
            def.spell,
            &targets.iter().map(|f| f.e).collect::<Vec<_>>(),
            None,
        );
        for link in targeting::links(
            &targets,
            &foes,
            &params,
            if area { 0.0 } else { speed },
            def.kind == Kind::Chain,
        ) {
            let (damage, crit) = engine::critical_with_multiplier(
                params.dmg * link.scale,
                params.crit,
                params.crit_multiplier,
                rng.f32(),
            );
            let mut hit =
                super::super::abilities::hit_for(def, &params, link.target.e, damage, crit);
            hit.triggered = true;
            hit.origin = super::legendary::events::Origin::Item;
            hit.dot *= link.scale;
            if area {
                hit.splash = 0.0;
            }
            if let Some(from) = link.from {
                pending
                    .1
                    .push((now + link.launch, from, link.target.e, def.spell));
            }
            if link.arrival > 0.0 {
                pending.0.push((now + link.arrival, hit));
            } else {
                hits.write(hit);
            }
        }
        run.banner = Some((format!("Trinket proc: {}", def.key.replace('_', " ")), 2.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::creature_anim::{CastEvent, PlaySeq, SpellGoTargets, SwingMessage};
    use bevy::ecs::system::RunSystemOnce;
    fn world() -> World {
        let mut world = World::new();
        world.init_resource::<Run>();
        world.insert_resource(Rng(42));
        world.init_resource::<Time>();
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(0.1));
        world.init_resource::<PendingHits>();
        world.init_resource::<PlaySeq>();
        world.init_resource::<Messages<Hit>>();
        world.init_resource::<Messages<CastEvent>>();
        world.init_resource::<Messages<SpellGoTargets>>();
        world.init_resource::<Messages<SwingMessage>>();
        world.init_resource::<Messages<crate::combat_text::CombatTextSpawn>>();
        world.init_resource::<Messages<crate::net::FieldChanged>>();
        world.init_resource::<Messages<super::super::super::enemies::Loot>>();
        world.spawn((Hero { vy: 0.0, air: 0.0 }, Transform::default()));
        world
    }
    fn proc(item: u64, kind: ProcKind) -> EquippedProc {
        EquippedProc {
            item,
            index: 0,
            chance: 1.0,
            filter: Filter::default(),
            roll: ProcRoll {
                kind,
                cooldown: 10.0,
                duration: 3.0,
                power: 0.5,
                spell: None,
                class: None,
            },
        }
    }
    #[test]
    fn weapon_proc_filters_accept_physical_spell_impacts_without_auto_attacking() {
        let mut world = world();
        let mut effect = proc(1, ProcKind::Damage);
        effect.filter = Filter::requiring(Tags::ATTACK);
        {
            let mut run = world.resource_mut::<Run>();
            equip(&mut run, vec![effect]);
            record_hit(
                &mut run,
                "fireball",
                Tags::SPELL | Tags::FIRE | Tags::DAMAGE,
            );
        }
        world.run_system_once(tick).unwrap();
        assert!(world.resource::<Run>().item_runtime.buffs.is_empty());
        record_hit(
            &mut world.resource_mut::<Run>(),
            "proc_bloodburst",
            Tags::SPELL | Tags::PHYSICAL | Tags::DAMAGE,
        );
        world.run_system_once(tick).unwrap();
        assert_eq!(world.resource::<Run>().item_runtime.buffs.len(), 1);
    }
    #[test]
    fn luck_favours_higher_rarities_without_enabling_disabled_categories() {
        let base = [50.0, 30.0, 15.0, 5.0];
        assert_eq!(lucky_weights(base, 0.0), base);
        let lucky = lucky_weights(base, 1.0);
        let total: f64 = lucky.iter().sum();
        assert!(lucky[3] / total > 0.05);
        assert!(lucky[0] / total < 0.50);
        assert!(lucky[3] / base[3] > lucky[2] / base[2]);
        assert_eq!(lucky_weights([50.0, 30.0, 15.0, 0.0], 10.0)[3], 0.0);
        assert!(lucky_weights([f64::MAX; 4], 1.0e6)
            .iter()
            .all(|v| v.is_finite()));
    }
    #[test]
    fn leech_is_capped_after_healing_effectiveness_and_only_recovers_missing_life() {
        let mut run = Run {
            hp: 10.0,
            ..default()
        };
        run.equipment.healing = 1.0;
        let cap = run.stats().max_hp * 0.20;
        assert_eq!(leech(&mut run, 1.0e6), cap);
        assert_eq!(leech(&mut run, 1.0e6), 0.0);
        run.elapsed = 1.0;
        run.hp = run.stats().max_hp - 3.0;
        assert_eq!(leech(&mut run, 100.0), 3.0);
        assert_eq!(leech(&mut run, 100.0), 0.0);
        run.hp = 0.0;
        assert_eq!(leech(&mut run, 100.0), 0.0);
    }
    #[test]
    fn stat_procs_respect_internal_cooldowns_expiry_and_unequipping() {
        let mut world = world();
        let fire = data::ability("fireball");
        let base = world.resource::<Run>().resolve_spell(fire, 1).dmg;
        {
            let mut run = world.resource_mut::<Run>();
            equip(&mut run, vec![proc(1, ProcKind::Damage)]);
            record_hit(
                &mut run,
                "fireball",
                Tags::SPELL | Tags::FIRE | Tags::DAMAGE,
            );
        }
        world.run_system_once(tick).unwrap();
        assert_eq!(
            world.resource::<Run>().resolve_spell(fire, 1).dmg,
            base * 1.5
        );
        world.resource_mut::<Run>().elapsed = 4.0;
        record_hit(&mut world.resource_mut::<Run>(), "fireball", Tags::FIRE);
        world.run_system_once(tick).unwrap();
        assert_eq!(world.resource::<Run>().resolve_spell(fire, 1).dmg, base);
        {
            let mut run = world.resource_mut::<Run>();
            equip(&mut run, vec![]);
            equip(&mut run, vec![proc(1, ProcKind::Damage)]);
            record_hit(&mut run, "fireball", Tags::FIRE);
        }
        world.run_system_once(tick).unwrap();
        assert!(world.resource::<Run>().item_runtime.buffs.is_empty());
        world.resource_mut::<Run>().elapsed = 10.0;
        record_hit(&mut world.resource_mut::<Run>(), "fireball", Tags::FIRE);
        world.run_system_once(tick).unwrap();
        assert_eq!(world.resource::<Run>().item_runtime.buffs.len(), 1);
        equip(&mut world.resource_mut::<Run>(), vec![]);
        assert_eq!(world.resource::<Run>().resolve_spell(fire, 1).dmg, base);
    }
    #[test]
    fn class_proc_uses_the_spell_engine_without_learning_spells_or_spawning_recursive_hits() {
        let mut world = world();
        let enemy = super::super::super::enemies::Enemy {
            def: &data::roster(0).enemies[0],
            hp: 1000.0,
            max_hp: 1000.0,
            speed: 0.0,
            damage: 0.0,
            radius: 1.0,
            xp: 1,
            boss: false,
            attack_cd: 0.0,
            slow: 1.0,
            slow_t: 0.0,
            root_t: 0.0,
            stun_t: 0.0,
            fear_t: 0.0,
            weaken: 1.0,
            weaken_t: 0.0,
            dots: Vec::new(),
            dead: false,
        };
        world.spawn((
            enemy,
            Transform::from_translation(Vec3::X * 3.0),
            crate::net::Guid(1),
            crate::net::ObjectStore(benilla_protocol::messages::ObjectFields::from_pairs(&[])),
            super::super::super::units::Marks::debuffs(),
        ));
        let mut p = proc(1, ProcKind::ClassSpell);
        p.roll.spell = Some("fireball".into());
        p.roll.class = Some(8);
        {
            let mut run = world.resource_mut::<Run>();
            equip(&mut run, vec![p]);
            run.replace_spell_modifiers(
                1,
                [Modifier {
                    source: 1,
                    filter: Filter::default(),
                    change: Change::AdditionalProjectiles(1),
                }],
            );
            record_hit(&mut run, "frostbolt", Tags::COLD);
        }
        world.run_system_once(tick).unwrap();
        let messages = world.resource::<Messages<Hit>>();
        let mut cursor = messages.get_cursor();
        let hits: Vec<_> = cursor.read(messages).collect();
        assert!(!hits.is_empty());
        assert!(hits
            .iter()
            .all(|h| h.triggered && h.skill == "fireball" && h.damage > 0.0));
        assert!(world.resource::<Run>().abilities.is_empty());
        assert!(world.resource::<Run>().item_runtime.events.is_empty());
        world
            .run_system_once(super::super::super::abilities::apply_hits)
            .unwrap();
        assert!(
            world.resource::<Run>().item_runtime.events.is_empty(),
            "triggered hits must not enqueue further procs"
        );
    }
    #[test]
    fn events_cannot_proc_items_that_were_not_equipped_at_impact() {
        let mut world = world();
        {
            let mut run = world.resource_mut::<Run>();
            equip(&mut run, vec![proc(1, ProcKind::Damage)]);
            record_hit(&mut run, "fireball", Tags::FIRE);
            equip(&mut run, vec![proc(2, ProcKind::Damage)]);
        }
        world.run_system_once(tick).unwrap();
        assert!(world.resource::<Run>().item_runtime.buffs.is_empty());
    }
}
