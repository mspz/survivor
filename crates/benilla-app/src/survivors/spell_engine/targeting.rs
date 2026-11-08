use super::{Resolved, MAX_TARGETS};
use bevy::prelude::*;

#[derive(Clone, Copy, Debug)]
pub struct Target {
    pub e: Entity,
    pub pos: Vec3,
    pub hp: f32,
    pub dist: f32,
    pub radius: f32,
}
fn flat(v: Vec3) -> Vec3 {
    Vec3::new(v.x, 0.0, v.z)
}
pub fn melee_contact_range(monster_radius: f32) -> f32 {
    super::super::hero::HERO_RADIUS + monster_radius.max(0.0) + 0.35
}
pub fn threatens_owner(distance: f32, monster_radius: f32) -> bool {
    distance <= melee_contact_range(monster_radius) * 1.2
}
pub fn nearest(foes: &[Target], range: f32, n: usize) -> Vec<Target> {
    let mut out: Vec<_> = foes
        .iter()
        .filter(|f| f.dist <= range + f.radius)
        .copied()
        .collect();
    out.sort_by_key(|f| !threatens_owner(f.dist, f.radius));
    out.truncate(n.min(MAX_TARGETS));
    out
}
/// Bosses in range come first; remaining ties prefer low health, then distance.
pub fn weakest(
    foes: &[Target],
    range: f32,
    n: usize,
    boss: impl Fn(Entity) -> bool,
) -> Vec<Target> {
    let mut out: Vec<_> = foes
        .iter()
        .filter(|f| f.hp > 0.0 && f.dist <= range + f.radius)
        .copied()
        .collect();
    out.sort_by(|a, b| {
        threatens_owner(b.dist, b.radius)
            .cmp(&threatens_owner(a.dist, a.radius))
            .then_with(|| {
                boss(b.e)
                    .cmp(&boss(a.e))
                    .then(a.hp.total_cmp(&b.hp))
                    .then(a.dist.total_cmp(&b.dist))
                    .then(a.e.to_bits().cmp(&b.e.to_bits()))
            })
    });
    out.truncate(n.min(MAX_TARGETS));
    out
}
pub fn around(foes: &[Target], center: Vec3, r: f32) -> Vec<Target> {
    foes.iter()
        .filter(|f| flat(f.pos - center).length() <= r + f.radius * 0.5)
        .copied()
        .collect()
}
pub fn densest(foes: &[Target], range: f32, r: f32) -> Option<Vec3> {
    let near: Vec<_> = foes.iter().filter(|f| f.dist <= range).take(40).collect();
    let danger = near.iter().any(|f| threatens_owner(f.dist, f.radius));
    near.iter()
        .filter(|f| !danger || threatens_owner(f.dist, f.radius))
        .max_by_key(|f| {
            near.iter()
                .filter(|o| flat(o.pos - f.pos).length() <= r)
                .count()
        })
        .map(|f| f.pos)
}
#[derive(Clone, Copy, Debug)]
pub struct PlannedHit {
    pub target: Target,
    pub from: Option<Entity>,
    pub scale: f32,
    pub launch: f32,
    pub arrival: f32,
}

/// Homing links snapshot targets when cast. Forks precede chains. Each cast hits an entity once;
/// children cannot revisit any primary target, including targets reserved for other projectiles.
pub fn links(
    roots: &[Target],
    foes: &[Target],
    p: &Resolved,
    speed: f32,
    native_chain: bool,
) -> Vec<PlannedHit> {
    let mut visited: Vec<_> = roots.iter().map(|f| f.e).collect();
    let mut out: Vec<_> = roots
        .iter()
        .take(MAX_TARGETS)
        .map(|f| PlannedHit {
            target: *f,
            from: None,
            scale: 1.0,
            launch: 0.0,
            arrival: if native_chain || speed <= 0.0 {
                0.0
            } else {
                f.dist / speed
            },
        })
        .collect();
    let mut queue: std::collections::VecDeque<_> = out
        .iter()
        .copied()
        .map(|h| (h, p.forks, p.chains))
        .collect();
    while let Some((parent, forks, chains)) = queue.pop_front() {
        if out.len() >= MAX_TARGETS {
            break;
        }
        let (count, next_forks, next_chains, scale) = if forks > 0 {
            (2, forks - 1, chains, 0.7)
        } else if chains > 0 {
            (1, 0, chains - 1, 0.85)
        } else {
            continue;
        };
        let mut candidates: Vec<_> = foes
            .iter()
            .filter(|f| !visited.contains(&f.e))
            .filter(|f| flat(f.pos - parent.target.pos).length() <= p.link_range)
            .copied()
            .collect();
        candidates.sort_by(|a, b| {
            flat(a.pos - parent.target.pos)
                .length_squared()
                .total_cmp(&flat(b.pos - parent.target.pos).length_squared())
                .then_with(|| a.e.to_bits().cmp(&b.e.to_bits()))
        });
        for f in candidates.into_iter().take(count) {
            if out.len() >= MAX_TARGETS {
                break;
            }
            visited.push(f.e);
            let travel = if speed > 0.0 && !native_chain {
                flat(f.pos - parent.target.pos).length() / speed
            } else {
                0.08
            };
            let hit = PlannedHit {
                target: f,
                from: Some(parent.target.e),
                scale: parent.scale * scale,
                launch: parent.arrival,
                arrival: parent.arrival + travel.max(0.08),
            };
            out.push(hit);
            queue.push_back((hit, next_forks, next_chains));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn foe(id: u32, distance: f32, radius: f32, hp: f32) -> Target {
        Target {
            e: Entity::from_raw_u32(id).unwrap(),
            pos: Vec3::X * distance,
            hp,
            dist: distance,
            radius,
        }
    }
    #[test]
    fn danger_threshold_is_twenty_percent_beyond_the_monsters_actual_melee_range() {
        for radius in [0.0, 1.0, 3.0] {
            let edge = melee_contact_range(radius) * 1.2;
            assert!(threatens_owner(edge, radius));
            assert!(!threatens_owner(edge + 0.001, radius));
        }
    }
    #[test]
    fn immediate_threats_beat_bosses_and_low_health_then_old_order_returns() {
        let mut foes = [
            foe(1, 1.0, 0.0, 1000.0),
            foe(2, 4.0, 0.0, 1.0),
            foe(3, 6.0, 0.0, 10_000.0),
        ];
        let boss = foes[2].e;
        assert_eq!(
            weakest(&foes, 20.0, 3, |e| e == boss)
                .iter()
                .map(|f| f.e)
                .collect::<Vec<_>>(),
            vec![foes[0].e, boss, foes[1].e]
        );
        assert!(weakest(&foes, 0.5, 1, |_| false).is_empty());
        foes[0].dist = 2.0;
        foes[0].pos.x = 2.0;
        assert_eq!(weakest(&foes, 20.0, 1, |e| e == boss)[0].e, boss);
        assert_eq!(weakest(&foes, 20.0, 1, |_| false)[0].e, foes[1].e);
    }
    #[test]
    fn melee_and_area_selection_prioritize_threats_but_keep_distant_behavior() {
        let foes = [
            foe(1, 1.3, 0.0, 100.0),
            foe(2, 3.0, 2.0, 100.0),
            foe(3, 4.0, 0.0, 100.0),
        ];
        assert_eq!(nearest(&foes, 10.0, 1)[0].e, foes[1].e);
        let mut clusters = [
            foe(1, 1.0, 0.0, 100.0),
            foe(2, 10.0, 0.0, 100.0),
            foe(3, 11.0, 0.0, 100.0),
        ];
        assert_eq!(densest(&clusters, 20.0, 2.0), Some(clusters[0].pos));
        clusters[0].pos.x = 4.0;
        clusters[0].dist = 4.0;
        assert!(densest(&clusters, 20.0, 2.0).unwrap().x >= 10.0);
    }
}
