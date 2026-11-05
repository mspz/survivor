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
pub fn nearest(foes: &[Target], range: f32, n: usize) -> Vec<Target> {
    foes.iter()
        .filter(|f| f.dist <= range + f.radius)
        .take(n.min(MAX_TARGETS))
        .copied()
        .collect()
}
pub fn weakest(foes: &[Target], range: f32, n: usize) -> Vec<Target> {
    let mut out: Vec<_> = foes
        .iter()
        .filter(|f| f.hp > 0.0 && f.dist <= range + f.radius)
        .copied()
        .collect();
    out.sort_by(|a, b| {
        a.hp.total_cmp(&b.hp)
            .then(a.dist.total_cmp(&b.dist))
            .then(a.e.to_bits().cmp(&b.e.to_bits()))
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
    near.iter()
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
