//! One immutable broad phase per hit batch; candidates retain ECS iteration order.
use bevy::prelude::*;
use std::collections::HashMap;

const CELL: f32 = 8.0;

#[derive(Default)]
pub(super) struct SplashIndex {
    points: Vec<(Entity, Vec3)>,
    cells: HashMap<(i32, i32), Vec<usize>>,
    candidates: Vec<usize>,
    indexed: bool,
}

impl SplashIndex {
    pub fn rebuild(&mut self, points: impl Iterator<Item = (Entity, Vec3)>, areas: usize) {
        self.points.clear();
        self.points.extend(points);
        // A handful of targets or one splash does not repay building a grid.
        self.indexed = areas >= 4 && self.points.len() >= 128;
        if !self.indexed {
            return;
        }
        // Keep cell allocations between batches, but discard obsolete map regions.
        if self.cells.len() > 4096 {
            self.cells.clear();
        }
        for cell in self.cells.values_mut() {
            cell.clear();
        }
        for (index, &(_, pos)) in self.points.iter().enumerate() {
            self.cells.entry(cell(pos)).or_default().push(index);
        }
    }

    pub fn within(&mut self, center: Vec3, radius: f32) -> impl Iterator<Item = Entity> + '_ {
        self.candidates.clear();
        let low = cell(center - Vec3::splat(radius));
        let high = cell(center + Vec3::splat(radius));
        let width = i64::from(high.0) - i64::from(low.0) + 1;
        let height = i64::from(high.1) - i64::from(low.1) + 1;
        // Huge or non-finite areas are cheaper and safer as a linear scan.
        if !self.indexed
            || !radius.is_finite()
            || !center.is_finite()
            || width <= 0
            || height <= 0
            || width.saturating_mul(height) > self.points.len() as i64
        {
            self.candidates.extend(0..self.points.len());
        } else {
            // Include neighbouring cells to absorb floating-point boundary rounding.
            for x in low.0.saturating_sub(1)..=high.0.saturating_add(1) {
                for z in low.1.saturating_sub(1)..=high.1.saturating_add(1) {
                    if let Some(indices) = self.cells.get(&(x, z)) {
                        self.candidates.extend(indices);
                    }
                }
            }
            self.candidates.sort_unstable();
        }
        let points = &self.points;
        self.candidates.iter().filter_map(move |&index| {
            let (entity, pos) = points[index];
            // Keep the original narrow-phase arithmetic, including inclusive edges.
            (super::units::flat(pos - center).length() <= radius).then_some(entity)
        })
    }
}

fn cell(pos: Vec3) -> (i32, i32) {
    ((pos.x / CELL).floor() as i32, (pos.z / CELL).floor() as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexed_splashes_match_linear_scan_including_order_and_edges() {
        let mut world = World::new();
        let points: Vec<_> = (-30..30)
            .flat_map(|x| (-30..30).map(move |z| (x, z)))
            .map(|(x, z)| {
                (
                    world.spawn_empty().id(),
                    Vec3::new(x as f32 * 2.0, 7.0, z as f32 * 2.0),
                )
            })
            .collect();
        let mut index = SplashIndex::default();
        index.rebuild(points.iter().copied(), 10);
        for center in [
            Vec3::ZERO,
            Vec3::new(-8.0, 123.0, 8.0),
            Vec3::new(7.999999, -9.0, -8.0),
        ] {
            for radius in [0.0, 0.001, 2.0, 8.0, 13.7, 1000.0, f32::INFINITY, f32::NAN] {
                let expected: Vec<_> = points
                    .iter()
                    .filter(|(_, p)| super::super::units::flat(*p - center).length() <= radius)
                    .map(|(e, _)| *e)
                    .collect();
                assert_eq!(index.within(center, radius).collect::<Vec<_>>(), expected);
            }
        }
        let _ = index.within(Vec3::ZERO, 8.0).count();
        assert!(
            index.candidates.len() < points.len() / 4,
            "dense local splashes must avoid scanning the map"
        );
        index.rebuild(points.iter().copied(), 1);
        assert!(
            !index.indexed,
            "one splash should avoid the grid setup cost"
        );
        assert_eq!(index.within(Vec3::ZERO, 1000.0).count(), points.len());
        index.rebuild(points.iter().take(3).copied(), 10);
        assert_eq!(index.within(Vec3::ZERO, 1000.0).count(), 3);
        index.rebuild(std::iter::empty(), 10);
        assert_eq!(index.within(Vec3::ZERO, 8.0).count(), 0);
    }
}
