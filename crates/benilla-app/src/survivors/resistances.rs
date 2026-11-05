//! Additive elemental percentage points, capped at 75% independently of armour.
use super::spell_engine::Tags;
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct Resistances {
    pub fire: f32,
    pub cold: f32,
    pub lightning: f32,
    pub nature: f32,
    pub arcane: f32,
    pub holy: f32,
    pub shadow: f32,
}

impl Resistances {
    pub fn add_all(&mut self, percent: f32) {
        self.fire += percent;
        self.cold += percent;
        self.lightning += percent;
        self.nature += percent;
        self.arcane += percent;
        self.holy += percent;
        self.shadow += percent;
    }

    pub fn entries(self) -> [(Tags, &'static str, f32); 7] {
        [
            (Tags::FIRE, "Fire resistance", self.fire),
            (Tags::COLD, "Cold resistance", self.cold),
            (Tags::LIGHTNING, "Lightning resistance", self.lightning),
            (Tags::NATURE, "Nature resistance", self.nature),
            (Tags::ARCANE, "Arcane resistance", self.arcane),
            (Tags::HOLY, "Holy resistance", self.holy),
            (Tags::SHADOW, "Shadow resistance", self.shadow),
        ]
    }

    pub fn scaled(self, level: u32) -> Self {
        let scale = level as f32;
        Self {
            fire: self.fire * scale,
            cold: self.cold * scale,
            lightning: self.lightning * scale,
            nature: self.nature * scale,
            arcane: self.arcane * scale,
            holy: self.holy * scale,
            shadow: self.shadow * scale,
        }
    }

    pub fn valid(self) -> bool {
        self.entries()
            .iter()
            .all(|(_, _, v)| v.is_finite() && (0.0..=1000.0).contains(v))
    }

    /// Hybrid elemental tags use the strongest matching resistance once, not a product.
    /// DOT is deliberately absent: periodic damage keeps its elemental school.
    pub fn reduction(self, tags: Tags) -> f32 {
        let percent = self
            .entries()
            .into_iter()
            .filter(|(tag, _, _)| tags.contains(*tag))
            .map(|(_, _, percent)| percent)
            .fold(0.0_f32, f32::max);
        percent.clamp(0.0, 75.0) / 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentages_are_linear_and_capped_for_every_element_and_dot() {
        for (percent, expected) in [
            (-10.0, 0.0),
            (0.0, 0.0),
            (25.0, 0.25),
            (75.0, 0.75),
            (150.0, 0.75),
        ] {
            let r = Resistances {
                fire: percent,
                cold: percent,
                lightning: percent,
                nature: percent,
                arcane: percent,
                holy: percent,
                shadow: percent,
            };
            for (tag, _, _) in r.entries() {
                assert_eq!(r.reduction(tag), expected);
                assert_eq!(r.reduction(tag | Tags::DOT), expected);
            }
        }
    }

    #[test]
    fn hero_armour_is_physical_and_resistances_apply_before_absorption() {
        use super::super::{hero, Run};
        for (tag, _, _) in Resistances::default().entries() {
            let mut run = Run {
                equipment_armour: 1000.0,
                ..Default::default()
            };
            assert!((hero::hurt_hero_tagged(&mut run, 100.0, tag) - 100.0).abs() < 0.001);
            run.equipment.resistances = Resistances {
                fire: 100.0,
                cold: 100.0,
                lightning: 100.0,
                nature: 100.0,
                arcane: 100.0,
                holy: 100.0,
                shadow: 100.0,
            };
            let expected = 100.0 * (1.0 - run.equipment.resistances.reduction(tag));
            run.shield = 10.0;
            run.shield_t = 5.0;
            let actual = hero::hurt_hero_tagged(&mut run, 100.0, tag | Tags::DOT);
            assert!((actual - (expected - 10.0)).abs() < 0.001);
            assert_eq!(run.shield, 0.0);
            let physical = hero::hurt_hero_tagged(&mut run, 100.0, Tags::PHYSICAL | Tags::DOT);
            assert!(
                (physical - 100.0 * (1.0 - super::super::items::armour::reduction(1000.0))).abs()
                    < 0.001
            );
        }
    }

    #[test]
    fn every_element_protects_its_own_hits_and_dots_only() {
        for index in 0..7 {
            let mut values = [0.0; 7];
            values[index] = 100.0;
            let r = Resistances {
                fire: values[0],
                cold: values[1],
                lightning: values[2],
                nature: values[3],
                arcane: values[4],
                holy: values[5],
                shadow: values[6],
            };
            for (other, (tag, _, _)) in r.entries().into_iter().enumerate() {
                let expected = if index == other { 0.75 } else { 0.0 };
                assert_eq!(r.reduction(tag), expected);
                assert_eq!(r.reduction(tag | Tags::DOT), expected);
            }
            assert_eq!(r.reduction(Tags::PHYSICAL | Tags::DOT), 0.0);
            assert_eq!(r.reduction(Tags::DOT), 0.0);
        }
        let r = Resistances {
            lightning: 100.0,
            nature: 50.0,
            ..Default::default()
        };
        assert_eq!(
            r.reduction(Tags::LIGHTNING | Tags::NATURE),
            r.reduction(Tags::LIGHTNING)
        );
    }
}
