//! Build descriptive strings once per effect/target, while retaining every stack identity.
use super::*;
use game::enemies::Dot;

fn observation(now: f32, entity: Entity, enemy: &Enemy, dot: &Dot) -> Observation {
    Observation {
        source: dot.owner,
        key: format!(
            "dot:{}:{}",
            dot.skill,
            dot.meter_source.unwrap_or(dot.skill)
        ),
        name: dot.meter_source.unwrap_or(dot.skill).replace('_', " "),
        spell: game::data::ability(dot.skill).spell,
        kind: "DoT on enemy".into(),
        tags: dot.tags.unwrap_or_default().labels(),
        description: format!(
            "Periodic effect from {}. Each stack retains its own remaining duration.",
            dot.skill
        ),
        origin: format!("{:?} · Caster {:?}", dot.origin, dot.owner),
        target: format!("enemy:{}", entity.to_bits()),
        target_name: format!("{} #{}", enemy.def.name, entity.to_bits()),
        until: Some(now + dot.left),
        power: dot.dps,
        token: token(dot),
    }
}

fn token(dot: &Dot) -> String {
    format!(
        "{}:{}:{}",
        dot.meta.parent,
        dot.meta.started.to_bits(),
        dot.dps.to_bits()
    )
}

pub(super) fn collect(
    now: f32,
    entity: Entity,
    enemy: &Enemy,
    out: &mut BTreeMap<Option<u64>, Snapshots>,
) {
    let mut groups = BTreeMap::<(Option<u64>, &str, &str), Snapshot>::new();
    for dot in enemy.dots.iter().filter(|dot| dot.left > 0.0) {
        let snapshot = groups
            .entry((dot.owner, dot.skill, dot.meter_source.unwrap_or(dot.skill)))
            .or_default();
        snapshot.stacks += 1;
        snapshot.power += dot.dps;
        *snapshot
            .instances
            .entry(InstanceKey::Dot(
                dot.meta.parent,
                dot.meta.started.to_bits(),
                dot.dps.to_bits(),
            ))
            .or_default() += 1;
        if let Some(first) = &mut snapshot.observation {
            first.until = Some(first.until.unwrap().max(now + dot.left));
        } else {
            snapshot.observation = Some(observation(now, entity, enemy, dot));
        }
    }
    for ((owner, _, _), snapshot) in groups {
        let first = snapshot.observation.as_ref().unwrap();
        out.entry(owner)
            .or_default()
            .insert((first.key.clone(), first.target.clone()), snapshot);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game::{enemies::Dot, items::legendary::events::Origin, spell_engine::Tags};

    #[test]
    fn grouped_stacks_preserve_the_complete_meter_history_for_every_owner() {
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        let mut enemy = Enemy {
            def: &game::data::roster(0).enemies[0],
            hp: 100.0,
            max_hp: 100.0,
            speed: 0.0,
            damage: 0.0,
            radius: 1.0,
            xp: 0,
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
        for i in 0..300 {
            enemy.dots.push(Dot {
                owner: if i % 3 == 0 { Some(42) } else { None },
                skill: if i % 2 == 0 { "rend" } else { "garrote" },
                meter_source: if i % 7 == 0 { Some("rend") } else { None },
                tags: Some(Tags::PHYSICAL | Tags::DOT),
                item_source: None,
                origin: Origin::Skill,
                meta: default(),
                dps: (i % 13) as f32 * 1.01,
                left: (i % 11) as f32,
                tick: 1.0,
            });
        }
        let mut reference = BTreeMap::<Option<u64>, EffectMeter>::new();
        let mut grouped = BTreeMap::<Option<u64>, EffectMeter>::new();
        for now in [0.0, 0.1, 1.0, 2.0, 4.0] {
            let mut snapshots = BTreeMap::new();
            collect(now, entity, &enemy, &mut snapshots);
            for owner in [None, Some(42)] {
                let observations = enemy
                    .dots
                    .iter()
                    .filter(|d| d.left > 0.0 && d.owner == owner)
                    .map(|d| observation(now, entity, &enemy, d))
                    .collect();
                reference
                    .entry(owner)
                    .or_default()
                    .sample(now, observations);
                grouped
                    .entry(owner)
                    .or_default()
                    .sample_snapshots(now, snapshots.remove(&owner).unwrap_or_default());
                assert_eq!(
                    serde_json::to_value(&grouped[&owner]).unwrap(),
                    serde_json::to_value(&reference[&owner]).unwrap()
                );
            }
            enemy.dots.retain(|d| d.left > 1.0);
            for dot in &mut enemy.dots {
                dot.left -= 0.5;
            }
            if let Some(dot) = enemy.dots.first_mut() {
                dot.left += 2.0;
                dot.dps += 0.25;
            }
        }
    }
}
