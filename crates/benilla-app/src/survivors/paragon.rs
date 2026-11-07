//! Post-60 XP is progression only: no stat or talent rewards.
use super::talents;

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Progress {
    pub level: u32,
    pub xp: u64,
    pub needed: u64,
}
/// Paragon 0..60 repeats character-level costs (100, 200, ... 6000 XP).
/// Beyond 60 each cost grows by 10%, rounded to the nearest XP; saturation keeps every save representable.
fn cost(level: u32) -> u64 {
    if level < 60 {
        talents::threshold(level + 2) - talents::threshold(level + 1)
    } else {
        (6000.0_f64 * 1.1_f64.powf(f64::from(level - 59))).round() as u64
    }
}
pub(super) fn progress(total_xp: u64) -> Progress {
    let mut xp = total_xp.saturating_sub(talents::threshold(60));
    let mut level = 0;
    loop {
        let needed = cost(level);
        if xp < needed {
            return Progress { level, xp, needed };
        }
        xp -= needed;
        level += 1;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn xp_starts_after_sixty_and_repeats_the_character_curve() {
        let cap = talents::threshold(60);
        assert_eq!(
            progress(cap - 1),
            Progress {
                level: 0,
                xp: 0,
                needed: 100
            }
        );
        assert_eq!(
            progress(cap + 99),
            Progress {
                level: 0,
                xp: 99,
                needed: 100
            }
        );
        for level in 0..=60 {
            let xp = talents::threshold(level + 1);
            assert_eq!(progress(cap + xp).level, level);
            assert_eq!(progress(cap + xp).xp, 0);
        }
        assert_eq!(progress(cap + 125).xp, 25);
    }
    #[test]
    fn costs_continue_exponentially_and_extreme_saves_remain_finite() {
        assert_eq!(cost(59), 6000);
        assert_eq!(cost(60), 6600);
        assert!(cost(61) > cost(60));
        let p = progress(u64::MAX);
        assert!(p.level > 60 && p.level < 1000);
        assert!(p.xp < p.needed);
        assert_eq!(talents::level(u64::MAX), 60);
        assert_eq!(talents::budget(u64::MAX), 59);
    }
}
