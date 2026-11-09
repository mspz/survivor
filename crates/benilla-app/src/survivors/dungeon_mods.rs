//! Optional dungeon risks, frozen at launch and paid by the item level of every chest.
use serde::{Deserialize, Serialize};

pub(super) const MAX_RANK: u8 = 3;
#[cfg(test)]
pub(super) const MAX_BONUS: u32 = 15 * MAX_RANK as u32;
pub(super) const BASE_XP_MULTIPLIER: u32 = 3;
pub(super) struct Definition {
    pub name: &'static str,
}
pub(super) const DEFINITIONS: &[Definition] = &[
    Definition {
        name: "Fleet monsters",
    },
    Definition {
        name: "Brutal monsters",
    },
    Definition {
        name: "Frenzy",
    },
    Definition {
        name: "Reinforcements",
    },
    Definition {
        name: "Boss arsenal",
    },
    Definition {
        name: "Hasty boss spells",
    },
];

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(try_from = "SavedMods", into = "[u8; 6]")]
pub struct Mods { ranks: [u8; 6], map: Option<usize> }
impl PartialEq for Mods { fn eq(&self, other: &Self) -> bool { self.ranks == other.ranks } }
impl Eq for Mods {}
impl From<Mods> for [u8; 6] { fn from(mods: Mods) -> Self { mods.ranks } }

// Read old bit masks as rank one, while new snapshots carry explicit ranks.
#[derive(Deserialize)]
#[serde(untagged)]
enum SavedMods { Ranks([u8; 6]), Legacy(u8) }
impl TryFrom<SavedMods> for Mods {
    type Error = &'static str;
    fn try_from(value: SavedMods) -> Result<Self, Self::Error> {
        let ranks = match value {
            SavedMods::Ranks(ranks) => ranks,
            SavedMods::Legacy(bits) => std::array::from_fn(|i| u8::from(bits & (1 << i) != 0)),
        };
        if ranks.iter().any(|&rank| rank > MAX_RANK) { return Err("dungeon rank exceeds maximum"); }
        Ok(Self { ranks, map: None })
    }
}
impl Mods {
    pub(super) fn rank_bonus(self, index: usize) -> u32 { self.balance().modifiers.item_level_per_rank[index] }
    pub(super) fn rank_xp(self) -> f64 { self.balance().modifiers.xp_per_rank }
    pub(super) fn description(self, index: usize) -> String {
        let m = &self.balance().modifiers;
        match index {
            0 => format!("+{:.0}% monster and boss movement speed", m.movement_per_rank*100.0),
            1 => format!("+{:.0}% monster and boss damage", m.damage_per_rank*100.0),
            2 => format!("+{:.0}% attack rate and boss spell cooldown recovery", m.attack_rate_per_rank*100.0),
            3 => format!("+{} bosses in every boss wave", m.bosses_per_rank),
            4 => format!("+{} distinct spells per boss", m.spells_per_rank),
            _ => format!("Boss spell warning time x{:.2}", m.warning_multiplier_per_rank),
        }
    }
    fn balance(self) -> &'static super::dungeon_config::Config { super::dungeon_config::get(self.map.unwrap_or(super::data::DUNGEONS[0])) }
    pub(super) fn rank(self, index: usize) -> u8 { self.ranks.get(index).copied().unwrap_or(0) }
    pub(super) fn change(&mut self, index: usize, increase: bool) {
        if let Some(rank) = self.ranks.get_mut(index) {
            *rank = if increase { rank.saturating_add(1).min(MAX_RANK) } else { rank.saturating_sub(1) };
        }
    }
    pub(super) fn xp_multiplier(self) -> f64 {
        self.balance().character_xp_multiplier * self.balance().modifiers.xp_per_rank.powi(self.ranks.iter().map(|&r| i32::from(r)).sum())
    }
    pub(super) fn reward_xp(self, base: u64, map: usize) -> u64 {
        if !super::data::dungeon(map) { return base; }
        let configured = self.active(map);
        if configured.balance().character_xp_multiplier != 3.0 || configured.balance().modifiers.xp_per_rank != 1.25 {
            return (base as f64 * configured.xp_multiplier()) as u64;
        }
        let count: u32 = self.ranks.iter().map(|&r| u32::from(r)).sum();
        let numerator = u128::from(base) * u128::from(BASE_XP_MULTIPLIER) * 5_u128.pow(count);
        (numerator / 4_u128.pow(count)).min(u128::from(u64::MAX)) as u64
    }
    pub(super) fn active(self, map: usize) -> Self {
        if super::data::dungeon(map) { Self { map: Some(map), ..self } } else { Self::default() }
    }
    #[cfg(test)]
    pub(super) fn toggle(&mut self, index: usize) {
        if index < self.ranks.len() { self.ranks[index] = u8::from(self.ranks[index] == 0); }
    }
    pub(super) fn bonus(self) -> u32 {
        self.balance().modifiers.item_level_per_rank.iter().enumerate().map(|(i, bonus)| u32::from(self.rank(i)) * bonus).sum()
    }
    pub(super) fn movement(self) -> f32 { 1.0 + self.balance().modifiers.movement_per_rank * f32::from(self.rank(0)) }
    pub(super) fn damage(self) -> f32 { 1.0 + self.balance().modifiers.damage_per_rank * f32::from(self.rank(1)) }
    pub(super) fn attack_rate(self) -> f32 { 1.0 + self.balance().modifiers.attack_rate_per_rank * f32::from(self.rank(2)) }
    pub(super) fn boss_warning_multiplier(self) -> f32 { self.balance().modifiers.warning_multiplier_per_rank.powi(i32::from(self.rank(5))) }
    pub(super) fn extra_bosses(self) -> usize { usize::from(self.rank(3)) * self.balance().modifiers.bosses_per_rank }
    pub(super) fn extra_spells(self) -> usize { usize::from(self.rank(4)) * self.balance().modifiers.spells_per_rank }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ranks_scale_rewards_and_effects_and_preserve_legacy_selections() {
        let old: Mods = serde_json::from_str("63").unwrap();
        assert_eq!(old.bonus(), 15);
        let mut mods = old;
        for index in 0..6 {
            for _ in 0..5 { mods.change(index, true); }
            assert_eq!(mods.rank(index), MAX_RANK);
        }
        assert_eq!(mods.bonus(), MAX_BONUS);
        assert_eq!(mods.extra_bosses(), 3);
        assert_eq!(mods.extra_spells(), 3);
        assert!((mods.movement() - 1.9).abs() < 0.001);
        assert_eq!(mods.damage(), 1.75);
        assert!((mods.boss_warning_multiplier() - 0.343).abs() < 0.001);
        assert_eq!(mods.reward_xp(u64::MAX, super::super::data::DUNGEONS[0]), u64::MAX);
        assert_eq!(mods, serde_json::from_str(&serde_json::to_string(&mods).unwrap()).unwrap());
        assert!(serde_json::from_str::<Mods>("[4,0,0,0,0,0]").is_err());
        assert!(mods.reward_xp(1000, super::super::data::DUNGEONS[0]) > old.reward_xp(1000, super::super::data::DUNGEONS[0]));
        for index in 0..6 { for _ in 0..5 { mods.change(index, false); } }
        assert_eq!(mods, Mods::default());
    }
    #[test]
    fn dungeon_xp_multipliers_stack_without_overflow_or_affecting_campaign_xp() {
        let map = super::super::data::DUNGEONS[0];
        let mut mods = Mods::default();
        assert_eq!(mods.reward_xp(1024, map), 3072);
        mods.toggle(0);
        assert_eq!(mods.reward_xp(1024, map), 3840);
        for index in 1..5 {
            mods.toggle(index);
        }
        assert_eq!(mods.reward_xp(1024, map), 9375);
        assert_eq!(mods.reward_xp(u64::MAX, map), u64::MAX);
        assert_eq!(mods.reward_xp(1024, super::super::data::CAMPAIGN[5]), 1024);
    }
    #[test]
    fn launch_freezes_modifiers_and_campaign_launch_clears_their_effects() {
        let mut selection = super::super::Selection {
            test_character: true,
            class: Some(0),
            look: Some(0),
            looks: vec![(0, 1, 0)],
            map: super::super::data::DUNGEONS[0],
            ..Default::default()
        };
        selection.dungeon_mods.toggle(0);
        let mut run = super::super::Run::default();
        assert!(super::super::hud::start_run(&mut run, &mut selection));
        selection.dungeon_mods.toggle(0);
        assert_eq!(run.dungeon_modifiers().movement(), 1.3);
        selection.dungeon_mods.toggle(0);
        selection.map = super::super::data::CAMPAIGN[0];
        assert!(super::super::hud::start_run(&mut run, &mut selection));
        assert_eq!(run.dungeon_modifiers(), Mods::default());
    }
    #[test]
    fn modifiers_stack_once_and_are_inactive_outside_dungeons() {
        let mut mods = Mods::default();
        for i in 0..DEFINITIONS.len() {
            mods.toggle(i);
        }
        assert_eq!(mods.bonus(), 15);
        assert_eq!(mods.movement(), 1.3);
        assert_eq!(mods.damage(), 1.25);
        assert_eq!(mods.attack_rate(), 1.3);
        assert_eq!(mods.extra_bosses(), 1);
        assert_eq!(mods.extra_spells(), 1);
        for &map in super::super::data::DUNGEONS {
            assert_eq!(mods.active(map).bonus(), 15);
        }
        for &map in super::super::data::CAMPAIGN {
            assert_eq!(mods.active(map), Mods::default());
        }
        mods.toggle(0);
        assert_eq!(mods.bonus(), 13);
        mods.toggle(0);
        assert_eq!(mods.bonus(), 15);
    }
}
