use serde::Deserialize;
use std::sync::OnceLock;

const FILE: &str = include_str!("../../../../../config/survivors-dungeon-upgrades.toml");

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::survivors) struct Config {
    version: u32,
    base_cost_gold: u64,
    cost_growth_per_ilvl: f64,
    cost_reference_ilvl: u32,
    ilvl_gap_per_gain: u32,
    maximum_gain: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::survivors) struct Quote {
    pub cost: u64,
    pub minimum_gain: u32,
    pub maximum_gain: u32,
    pub extra_chance: f32,
}
impl Quote {
    pub fn roll(self, roll: f32) -> Result<u32, String> {
        if !roll.is_finite() || !(0.0..1.0).contains(&roll) {
            return Err("Invalid upgrade roll.".into());
        }
        Ok(self.minimum_gain + u32::from(roll < self.extra_chance))
    }
    pub fn odds(self) -> String {
        if self.minimum_gain == self.maximum_gain {
            format!("Guaranteed +{} ilvl", self.minimum_gain)
        } else if self.minimum_gain == 0 {
            format!("{:.0}% chance of +1 ilvl", self.extra_chance * 100.0)
        } else {
            format!(
                "+{} guaranteed; {:.0}% chance of +{} ilvl",
                self.minimum_gain,
                self.extra_chance * 100.0,
                self.maximum_gain
            )
        }
    }
}
impl Config {
    pub fn parse(text: &str) -> Result<Self, String> {
        let value: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        if value.version != 1
            || value.base_cost_gold == 0
            || !value.cost_growth_per_ilvl.is_finite()
            || !(1.0..=2.0).contains(&value.cost_growth_per_ilvl)
            || !(1..=200).contains(&value.cost_reference_ilvl)
            || !(1..=100).contains(&value.ilvl_gap_per_gain)
            || !(1..=5).contains(&value.maximum_gain)
        {
            return Err("Invalid dungeon upgrade configuration.".into());
        }
        Ok(value)
    }
    pub fn quote(&self, current: u32, dungeon: u32) -> Result<Quote, String> {
        let dungeon = dungeon.min(super::super::dungeon_config::MAX_REWARD_LEVEL);
        let gap = dungeon.saturating_sub(current);
        if gap == 0 {
            return Err("This item has reached this dungeon's item level.".into());
        }
        let units = gap.min(self.maximum_gain * self.ilvl_gap_per_gain);
        let minimum_gain = units / self.ilvl_gap_per_gain;
        let remainder = units % self.ilvl_gap_per_gain;
        let exponent = current.saturating_sub(self.cost_reference_ilvl).min(200) as i32;
        let cost = (self.base_cost_gold as f64
            * super::super::currency::GOLD as f64
            * self.cost_growth_per_ilvl.powi(exponent))
        .round()
        .clamp(1.0, super::super::currency::MAX_BALANCE as f64) as u64;
        let cost = cost.min(super::super::currency::MAX_BALANCE);
        Ok(Quote {
            cost,
            minimum_gain,
            maximum_gain: minimum_gain + u32::from(remainder > 0),
            extra_chance: remainder as f32 / self.ilvl_gap_per_gain as f32,
        })
    }
}

pub(in crate::survivors) fn get() -> &'static Config {
    static CONFIG: OnceLock<Config> = OnceLock::new();
    CONFIG.get_or_init(|| {
        if !cfg!(test) {
            if let Ok(text) = crate::local_state::read_to_string(std::path::Path::new(
                "config/survivors-dungeon-upgrades.toml",
            )) {
                match Config::parse(&text) {
                    Ok(config) => return config,
                    Err(error) => bevy::log::warn!("{error} Using default dungeon upgrade prices."),
                }
            }
        }
        Config::parse(FILE).expect("valid dungeon upgrade defaults")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distance_controls_the_gain_without_overshooting_or_exceeding_five() {
        let config = get();
        for gap in 1..=200 {
            let quote = config.quote(60, 60 + gap).unwrap();
            for roll in [0.0, 0.249, 0.25, 0.5, 0.75, 0.999] {
                let gain = quote.roll(roll).unwrap();
                assert!(gain <= 5 && gain <= gap);
            }
        }
        let last = config.quote(109, 110).unwrap();
        assert_eq!(
            (last.minimum_gain, last.maximum_gain, last.extra_chance),
            (0, 1, 0.25)
        );
        assert_eq!(last.roll(0.249).unwrap(), 1);
        assert_eq!(last.roll(0.25).unwrap(), 0);
        assert_eq!(config.quote(60, 80).unwrap().roll(0.999).unwrap(), 5);
        assert!(config.quote(110, 110).is_err());
        assert!(config.quote(111, 110).is_err());
        assert!(last.roll(f32::NAN).is_err());
        assert!(last.roll(1.0).is_err());
    }
    #[test]
    fn prices_start_at_several_thousand_and_grow_exponentially_to_tens_of_thousands() {
        let config = get();
        assert_eq!(
            config.quote(60, 110).unwrap().cost,
            5000 * super::super::super::currency::GOLD
        );
        let late = config.quote(109, 110).unwrap().cost / super::super::super::currency::GOLD;
        assert!((50_000..60_000).contains(&late));
        for level in 60..109 {
            assert!(
                config.quote(level + 1, 111).unwrap().cost > config.quote(level, 111).unwrap().cost
            );
        }
    }
}
