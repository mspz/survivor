//! Additive bonuses for gained expedition and persistent character levels (level one is baseline).
use super::Run;

pub(super) fn bonuses(expedition: u32, character: u32) -> (f32, f32) {
    let expedition = expedition.saturating_sub(1) as f32;
    let character = character.saturating_sub(1) as f32;
    (
        5.0 * expedition + 10.0 * character,
        0.02 * expedition + 0.05 * character,
    )
}

impl Run {
    pub(super) fn gain_expedition_level(&mut self) {
        self.set_levels(self.level.saturating_add(1), self.meta_level);
    }
    pub(super) fn set_global_level(&mut self, level: u32) {
        self.set_levels(self.level, level.max(1));
    }
    fn set_levels(&mut self, expedition: u32, character: u32) {
        if (self.level, self.meta_level) == (expedition, character) {
            return;
        }
        let before = self.stats().max_hp;
        self.level = expedition;
        self.meta_level = character;
        let maximum = self.stats().max_hp;
        // Grant the newly gained health without a full heal or resurrecting a fallen player.
        if self.hp > 0.0 && self.dead_t.is_none() {
            self.hp = (self.hp + (maximum - before).max(0.0)).min(maximum);
        }
        self.version = self.version.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::survivors::{
        data, healer_buffs,
        spell_engine::{Change, Filter, Modifier, Tags},
    };

    fn near(actual: f32, expected: f32) {
        assert!((actual - expected).abs() < 0.0001, "{actual} != {expected}");
    }
    #[test]
    fn gained_levels_add_the_requested_health_and_damage_and_stack_between_progressions() {
        assert_eq!(bonuses(1, 1), (0.0, 0.0));
        assert_eq!(bonuses(0, 0), (0.0, 0.0));
        let mut run = Run::default();
        let before = run.stats();
        run.gain_expedition_level();
        near(run.stats().max_hp, before.max_hp + 5.0);
        near(run.stats().level_damage, 0.02);
        run.set_global_level(2);
        near(run.stats().max_hp, before.max_hp + 15.0);
        near(run.stats().level_damage, 0.07);
        run.level = 5;
        run.set_global_level(10);
        near(run.stats().max_hp, before.max_hp + 110.0);
        near(run.stats().level_damage, 0.53);
    }
    #[test]
    fn damage_bonus_affects_attacks_spells_and_dots_but_not_healing_shields_or_blessings() {
        let mut run = Run::default();
        let keys = [
            "heroic_strike",
            "fireball",
            "corruption",
            "flash_heal",
            "power_word_shield",
        ];
        let baseline: Vec<_> = keys
            .iter()
            .map(|key| run.resolve_spell(data::ability(key), 1))
            .collect();
        let blessing = healer_buffs::tooltip(&run, data::ability("flash_heal"), 1);
        run.gain_expedition_level();
        run.set_global_level(2);
        for (i, key) in keys.iter().enumerate() {
            let now = run.resolve_spell(data::ability(key), 1);
            let factor = if now.tags.contains(Tags::DAMAGE) {
                1.07
            } else {
                1.0
            };
            near(now.dmg, baseline[i].dmg * factor);
            near(now.dot, baseline[i].dot * factor);
        }
        assert_eq!(
            healer_buffs::tooltip(&run, data::ability("flash_heal"), 1),
            blessing
        );
        run.replace_spell_modifiers(
            123,
            [Modifier {
                source: 0,
                filter: Filter::requiring(Tags::DAMAGE),
                change: Change::IncreasedDamage(0.5),
            }],
        );
        near(
            run.resolve_spell(data::ability("fireball"), 1).dmg,
            baseline[1].dmg * 1.57,
        );
    }
    #[test]
    fn level_ups_grant_only_the_gained_health_respect_health_scaling_and_do_not_revive() {
        let mut run = Run {
            hp: 50.0,
            ..Default::default()
        };
        let missing = run.stats().max_hp - run.hp;
        run.set_global_level(3);
        near(run.hp, 70.0);
        near(run.stats().max_hp - run.hp, missing);
        run.gain_expedition_level();
        near(run.hp, 75.0);
        let version = run.version;
        run.set_global_level(3);
        assert_eq!(run.version, version);
        run.talent_build[2][2] = 5; // +20% maximum health, applied to level health too.
        let before = run.stats().max_hp;
        run.gain_expedition_level();
        near(run.stats().max_hp - before, 6.0);
        near(run.hp, 81.0);
        run.hp = 0.0;
        run.set_global_level(4);
        assert_eq!(run.hp, 0.0);
        run.hp = 10.0;
        run.dead_t = Some(0.0);
        run.gain_expedition_level();
        assert_eq!(run.hp, 10.0);
    }
}
