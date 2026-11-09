//! Persistent account experience and original Classic-style class talent trees.
use super::{
    data::{self, Stat},
    Owned, Run, Stats,
};
use bevy::prelude::*;

pub(super) const NODES: usize = 19;

pub(super) fn column(node: usize) -> usize {
    [0, 1, 3, 0, 2, 3, 0, 3, 1, 0, 2, 3, 1, 2, 3, 0, 2, 3, 1][node]
}
pub(super) fn prerequisite(node: usize) -> Option<usize> {
    match node {
        12 => Some(8),
        18 => Some(12),
        _ => None,
    }
}
const CLASSES: usize = 9;
type Build = [[u8; NODES]; 3];

pub(super) struct Spec {
    pub name: &'static str,
    pub spells: &'static [&'static str],
    pub cap: &'static str,
}
macro_rules! spec {
    ($name:literal, $cap:literal, $($spell:literal),+) => {
        Spec { name: $name, cap: $cap, spells: &[$($spell),+] }
    };
}
pub(super) static TREES: [[Spec; 3]; CLASSES] = [
    [
        spec!(
            "Arms",
            "mortal_strike",
            "cleave",
            "heroic_strike",
            "mortal_strike"
        ),
        spec!(
            "Fury",
            "whirlwind",
            "whirlwind",
            "execute",
            "sweeping_strikes"
        ),
        spec!(
            "Protection",
            "thunder_clap",
            "thunder_clap",
            "demoralizing_shout"
        ),
    ],
    [
        spec!("Holy", "holy_shock", "holy_shock", "holy_wrath", "exorcism"),
        spec!(
            "Protection",
            "divine_shield",
            "consecration",
            "divine_shield",
            "hammer_of_justice"
        ),
        spec!("Retribution", "judgement", "judgement", "hammer_of_wrath"),
    ],
    [
        spec!(
            "Beast Mastery",
            "kill_command",
            "tame_beast",
            "kill_command",
            "bestial_wrath"
        ),
        spec!(
            "Marksmanship",
            "aimed_shot",
            "aimed_shot",
            "multi_shot",
            "volley"
        ),
        spec!(
            "Survival",
            "explosive_trap",
            "serpent_sting",
            "explosive_trap"
        ),
    ],
    [
        spec!("Assassination", "eviscerate", "eviscerate", "rupture"),
        spec!("Combat", "blade_flurry", "sinister_strike", "blade_flurry"),
        spec!("Subtlety", "kidney_shot", "gouge", "kidney_shot"),
    ],
    [
        spec!(
            "Discipline",
            "power_word_shield",
            "smite",
            "power_word_shield"
        ),
        spec!("Holy", "holy_nova", "holy_fire", "holy_nova"),
        spec!(
            "Shadow",
            "mind_blast",
            "shadow_word_pain",
            "mind_blast",
            "psychic_scream"
        ),
    ],
    [
        spec!(
            "Elemental",
            "chain_lightning",
            "lightning_bolt",
            "chain_lightning",
            "flame_shock"
        ),
        spec!(
            "Enhancement",
            "lightning_shield",
            "earth_shock",
            "frost_shock",
            "lightning_shield"
        ),
        spec!("Restoration", "magma_totem", "searing_totem", "magma_totem"),
    ],
    [
        spec!(
            "Arcane",
            "arcane_missiles",
            "arcane_missiles",
            "arcane_explosion"
        ),
        spec!(
            "Fire",
            "pyroblast",
            "fireball",
            "fire_blast",
            "flamestrike",
            "blast_wave",
            "pyroblast"
        ),
        spec!(
            "Frost",
            "blizzard",
            "frostbolt",
            "frost_nova",
            "blizzard",
            "cone_of_cold"
        ),
    ],
    [
        spec!(
            "Affliction",
            "death_coil",
            "corruption",
            "curse_of_agony",
            "death_coil"
        ),
        spec!("Demonology", "hellfire", "shadow_bolt", "hellfire"),
        spec!(
            "Destruction",
            "shadowburn",
            "immolate",
            "rain_of_fire",
            "searing_pain",
            "shadowburn"
        ),
    ],
    [
        spec!(
            "Balance",
            "starfire",
            "wrath",
            "moonfire",
            "starfire",
            "hurricane"
        ),
        spec!("Feral Combat", "thorns", "thorns", "entangling_roots"),
        spec!(
            "Restoration",
            "rejuvenation",
            "rejuvenation",
            "insect_swarm"
        ),
    ],
];

#[derive(Resource)]
pub(super) struct Account {
    pub character: Option<u64>,
    pub xp: u64,
    builds: [Build; CLASSES],
    pub last_reward: u64,
    /// Extra points from the specialty trees (Insight), on top of the level budget.
    pub bonus_points: u32,
}

pub(super) fn level(xp: u64) -> u32 {
    (1..60).find(|&l| xp < threshold(l + 1)).unwrap_or(60)
}
pub(super) fn threshold(level: u32) -> u64 {
    let n = u64::from(level.saturating_sub(1));
    50 * n * (n + 1)
}
pub(super) fn budget(xp: u64) -> u32 {
    level(xp).saturating_sub(1)
}
pub(super) fn max_rank(node: usize) -> u8 {
    if matches!(node, 8 | 12 | 18) {
        1
    } else {
        5
    }
}
pub(super) fn tier(node: usize) -> usize {
    node / 3
}
fn spent(build: &Build) -> u32 {
    build.iter().flatten().map(|&r| u32::from(r)).sum()
}

impl Default for Account {
    fn default() -> Self {
        let text = crate::local_state::home()
            .and_then(|home| {
                crate::local_state::read_to_string(&home.join("survivors-talents-v4.txt"))
                    .ok()
                    .or_else(|| {
                        crate::local_state::read_to_string(&home.join("survivors-talents-v3.txt"))
                            .ok()
                    })
                    .or_else(|| {
                        crate::local_state::read_to_string(&home.join("survivors-talents-v2.txt"))
                            .ok()
                    })
                    .or_else(|| {
                        crate::local_state::read_to_string(&home.join("survivors-talents-v1.txt"))
                            .ok()
                    })
            })
            .unwrap_or_default();
        let account = Self::from_save(&text);
        if text.split_whitespace().next() == Some("v1") {
            account.save();
        }
        account
    }
}
impl Account {
    #[cfg(not(target_arch = "wasm32"))]
    pub(in crate::survivors) fn shared_build(&self, class: usize) -> [[u8; NODES]; 3] {
        self.builds[class]
    }

    /// No server-side talent budget verification for trusted friend sessions.
    #[cfg(not(target_arch = "wasm32"))]
    pub(in crate::survivors) fn from_shared(
        class: usize,
        xp: u64,
        build: [[u8; NODES]; 3],
    ) -> Self {
        let mut account = Self::fresh();
        account.xp = xp;
        account.builds[class] = build;
        account
    }
    pub fn fresh() -> Self {
        Self::from_save("")
    }
    pub fn load_character(id: u64) -> Self {
        let text = crate::local_state::home()
            .and_then(|home| {
                crate::local_state::read_to_string(
                    &home.join(format!("characters/{id}-talents.txt")),
                )
                .ok()
            })
            .unwrap_or_default();
        let mut account = Self::from_save(&text);
        account.character = Some(id);
        account
    }
    fn from_save(text: &str) -> Self {
        let mut out = Self {
            character: None,
            xp: 0,
            builds: [[[0; NODES]; 3]; CLASSES],
            last_reward: 0,
            bonus_points: 0,
        };
        let mut values = text.split_whitespace();
        let version = values.next();
        if !matches!(version, Some("v1" | "v2" | "v3" | "v4")) {
            return out;
        }
        out.xp = values
            .next()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        // Version 4 refunds earlier layouts while preserving character experience.
        if version != Some("v4") {
            return out;
        }
        for class in 0..CLASSES {
            for tree in 0..3 {
                for node in 0..NODES {
                    let rank = values
                        .next()
                        .and_then(|s| s.parse::<u8>().ok())
                        .unwrap_or(0)
                        .min(max_rank(node));
                    for _ in 0..rank {
                        out.buy(class, tree, node);
                    }
                }
            }
        }
        out
    }

    /// Lifetime XP includes post-60 paragon progression; character level and talent budget stay capped.
    pub fn gain_xp(&mut self, gained: u64) {
        self.xp = self.xp.saturating_add(gained);
    }
    pub fn rank(&self, class: usize, tree: usize, node: usize) -> u8 {
        self.builds[class][tree][node]
    }
    pub fn available(&self, class: usize) -> u32 {
        (budget(self.xp) + self.bonus_points).saturating_sub(spent(&self.builds[class]))
    }
    pub fn tree_points(&self, class: usize, tree: usize) -> u32 {
        self.builds[class][tree].iter().map(|&r| u32::from(r)).sum()
    }
    pub fn can_buy(&self, class: usize, tree: usize, node: usize) -> bool {
        if class >= CLASSES || tree >= 3 || node >= NODES {
            return false;
        }
        let ranks = &self.builds[class][tree];
        let previous: u32 = ranks[..tier(node) * 3].iter().map(|&r| u32::from(r)).sum();
        self.available(class) > 0
            && ranks[node] < max_rank(node)
            && previous >= tier(node) as u32 * 5
            && prerequisite(node).is_none_or(|parent| ranks[parent] == max_rank(parent))
    }
    pub fn buy(&mut self, class: usize, tree: usize, node: usize) -> bool {
        if !self.can_buy(class, tree, node) {
            return false;
        }
        self.builds[class][tree][node] += 1;
        true
    }
    pub fn reset(&mut self, class: usize) {
        self.builds[class] = [[0; NODES]; 3];
    }
    pub fn can_refund(&self, class: usize, tree: usize, node: usize) -> bool {
        if class >= CLASSES || tree >= 3 || node >= NODES {
            return false;
        }
        let mut ranks = self.builds[class][tree];
        if ranks[node] == 0 {
            return false;
        }
        ranks[node] -= 1;
        ranks.iter().enumerate().all(|(index, &rank)| {
            rank == 0
                || (ranks[..tier(index) * 3]
                    .iter()
                    .map(|&r| u32::from(r))
                    .sum::<u32>()
                    >= tier(index) as u32 * 5
                    && prerequisite(index).is_none_or(|parent| ranks[parent] == max_rank(parent)))
        })
    }
    pub fn refund(&mut self, class: usize, tree: usize, node: usize) -> bool {
        if !self.can_refund(class, tree, node) {
            return false;
        }
        self.builds[class][tree][node] -= 1;
        true
    }
    pub fn save(&self) {
        if let Some(home) = crate::local_state::home() {
            let mut text = format!("v4 {}", self.xp);
            for rank in self.builds.iter().flatten().flatten() {
                text.push_str(&format!(" {rank}"));
            }
            if let Err(err) = crate::local_state::write_queued(
                &home.join(self.character.map_or_else(
                    || "survivors-talents-v4.txt".to_owned(),
                    |id| format!("characters/{id}-talents.txt"),
                )),
                &text,
            ) {
                warn!("Could not save account progression: {err}");
            }
        }
    }
    pub fn apply(&self, class: usize, run: &mut Run) {
        run.set_global_level(level(self.xp));
        run.talent_build = self.builds[class];
        super::talent_runtime::configure(run);
        for (tree, spec) in TREES[class].iter().enumerate() {
            let mut modifiers = Vec::new();
            for node in 0..NODES {
                let rank = self.rank(class, tree, node);
                if rank == 0 {
                    continue;
                }
                for change in super::talent_effects::effects(spec, node, rank) {
                    modifiers.push(super::spell_engine::Modifier {
                        source: 0,
                        filter: super::talent_effects::filter(spec, node, change),
                        change,
                    });
                }
            }
            run.replace_spell_modifiers(0x1000_0000 + tree as u64, modifiers);
            if self.rank(class, tree, NODES - 1) > 0
                && !run.abilities.iter().any(|a| a.def.key == spec.cap)
            {
                run.abilities.push(Owned {
                    def: data::ability(spec.cap),
                    level: 1,
                    cd: 0.5,
                });
            }
        }
    }
}

pub(super) fn stat_effect(spec: &Spec, node: usize) -> Option<(Stat, f32)> {
    super::minor_talents::minor(spec, node).stat
}
pub(super) fn apply_stats(run: &Run, stats: &mut Stats) {
    let Some(class) = data::CLASSES.iter().position(|c| c.id == run.class.id) else {
        return;
    };
    for (tree, spec) in TREES[class].iter().enumerate() {
        for node in 0..NODES {
            let rank = run.talent_build[tree][node];
            if rank == 0 || !super::talent_effects::character_stat(spec, node) {
                continue;
            }
            let Some((stat, value)) = stat_effect(spec, node) else {
                continue;
            };
            let v = value * f32::from(rank);
            match stat {
                Stat::AllResistance => stats.resistances.add_all(v),
                Stat::DamageReduction => stats.damage_reduction += v,
                Stat::Might => stats.might += v,
                Stat::MaxHp => stats.max_hp *= 1.0 + v,
                Stat::Speed => stats.speed *= 1.0 + v,
                Stat::Regen => stats.regen += v,
                Stat::Haste => stats.haste += v,
                Stat::Area => stats.area += v,
                Stat::Magnet => stats.magnet *= 1.0 + v,
                Stat::Growth => stats.growth += v,
                Stat::Armor => stats.armor *= 1.0 + v,
                Stat::Crit => stats.crit += v,
            }
        }
    }
}

pub(in crate::survivors) fn earned(run: &Run) -> u64 {
    let base = (run.elapsed.max(0.0) as u64 / 5)
        + (u64::from(run.meta_kills) * 1000).saturating_sub(run.meta_kill_penalty) / 1000
        + u64::from(run.bosses) * 100
        + if run.won { 500 } else { 0 };
    run.dungeon_modifiers().reward_xp(base, run.expedition_map)
}

/// Count a kill toward the character's XP, kept in full within nine levels of the monster;
/// fractional rewards carry across kills.
pub(super) fn credit_kill(run: &mut Run, monster_level: u32) {
    run.meta_kills += 1;
    let retained = match run.meta_level.abs_diff(monster_level) {
        0..=9 => 1000,
        10 => 100,
        11 => 50,
        12 => 25,
        _ => 10,
    };
    run.meta_kill_penalty += 1000 - retained;
}
pub(super) fn prepare(
    account: Res<Account>,
    selection: Res<super::Selection>,
    mut run: ResMut<Run>,
) {
    if run.elapsed == 0.0 && run.level == 1 {
        if let Some(class) = selection.class {
            account.apply(class, &mut run);
            run.hp = run.stats().max_hp;
            run.version += 1;
        }
    }
}
pub(super) fn credit(mut account: ResMut<Account>, mut run: ResMut<Run>) {
    settle(&mut account, &mut run, false);
}
pub(super) fn flush(mut account: ResMut<Account>, mut run: ResMut<Run>) {
    settle(&mut account, &mut run, true);
}
fn settle(account: &mut Account, run: &mut Run, force: bool) {
    let total = earned(run);
    let gained = total.saturating_sub(run.account_credited);
    if gained == 0 || (!force && gained < 25 && !run.won && run.dead_t.is_none()) {
        return;
    }
    run.account_credited = total;
    account.gain_xp(gained);
    run.set_global_level(level(account.xp));
    account.last_reward = total;
    account.save();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dungeon_character_xp_rewards_scale_all_sources_and_modifiers_multiply_them() {
        let mut run = Run {
            elapsed: 100.0, kills: 20, bosses: 2, won: true,
            expedition_map: super::super::data::CAMPAIGN[5], ..default()
        };
        assert_eq!(earned(&run), 740);
        run.expedition_map = super::super::data::DUNGEONS[0];
        assert_eq!(earned(&run), 2220);
        run.dungeon_mods.toggle(0);
        assert_eq!(earned(&run), 2775);
        for index in 1..5 { run.dungeon_mods.toggle(index); }
        assert_eq!(earned(&run), 6774);
    }
    #[test]
    fn single_point_refunds_preserve_tier_requirements_and_other_trees() {
        let mut a = account();
        for _ in 0..5 {
            assert!(a.buy(0, 0, 0));
        }
        assert!(a.buy(0, 0, 3));
        assert!(a.buy(0, 1, 0));
        assert!(!a.refund(0, 0, 0));
        assert_eq!(a.rank(0, 0, 0), 5);
        assert!(a.buy(0, 0, 1));
        let available = a.available(0);
        assert!(a.refund(0, 0, 0));
        assert_eq!(a.available(0), available + 1);
        assert_eq!(a.rank(0, 0, 0), 4);
        assert_eq!(a.rank(0, 0, 3), 1);
        assert_eq!(a.rank(0, 1, 0), 1);
        assert!(!a.refund(0, 0, 2));
        assert!(!a.refund(CLASSES, 0, 0));
        assert!(!a.refund(0, 3, 0));
        assert!(!a.refund(0, 0, NODES));
    }
    #[test]
    fn milestones_must_be_refunded_from_child_to_parent() {
        let mut a = account();
        for node in [0, 1, 3, 4, 6, 7] {
            for _ in 0..5 {
                assert!(a.buy(0, 0, node));
            }
        }
        for node in [8, 12, 18] {
            assert!(a.buy(0, 0, node));
        }
        assert!(!a.refund(0, 0, 8));
        assert!(!a.refund(0, 0, 12));
        assert!(a.refund(0, 0, 18));
        assert!(a.refund(0, 0, 12));
        assert!(a.refund(0, 0, 8));
        assert!(!a.refund(0, 0, 8));
    }
    #[test]
    fn post_cap_xp_survives_save_loading_without_extra_talent_points_or_character_levels() {
        let cap = threshold(60);
        let mut account = Account::from_save(&format!("v4 {}", cap - 25));
        account.gain_xp(150);
        assert_eq!(account.xp, cap + 125);
        assert_eq!(super::super::paragon::progress(account.xp).level, 1);
        assert_eq!(super::super::paragon::progress(account.xp).xp, 25);
        let restored = Account::from_save(&format!("v4 {}", account.xp));
        assert_eq!(restored.xp, account.xp);
        assert_eq!(restored.available(0), 59);
        let mut run = Run::default();
        restored.apply(0, &mut run);
        assert_eq!(run.meta_level, 60);
        account.gain_xp(u64::MAX);
        assert_eq!(account.xp, u64::MAX);
        assert_eq!(account.available(0), 59);
    }
    #[test]
    fn ignite_accepts_every_fire_tagged_damage_spell_including_attacks_and_hidden_spells() {
        let mut account = Account::fresh();
        account.builds[6][1][8] = 1;
        let mut run = Run {
            class: &data::CLASSES[6],
            ..Default::default()
        };
        account.apply(6, &mut run);
        let modifiers = run.spell_modifiers.entries();
        for def in data::ABILITIES
            .iter()
            .chain(super::super::procs::SPELLS.iter().map(|s| &s.def))
        {
            let tags = super::super::spell_engine::tags::for_spell(def);
            let actual = super::super::spell_engine::on_hit::modifiers(def.key, modifiers).ignite;
            assert_eq!(
                actual,
                if tags.contains(super::super::spell_engine::Tags::FIRE) {
                    1.0
                } else {
                    0.0
                },
                "{}",
                def.key
            );
        }
    }
    #[test]
    fn bleed_milestones_filter_physical_skill_hits_and_keep_ferals_melee_requirement() {
        for (class, tree, chance) in [(0, 1, 1.0), (2, 0, 0.0), (2, 2, 1.0), (8, 1, 1.0)] {
            let mut account = Account::fresh();
            account.builds[class][tree][8] = 1;
            let mut run = Run {
                class: &data::CLASSES[class],
                ..Default::default()
            };
            account.apply(class, &mut run);
            let modifiers = run.spell_modifiers.entries();
            let physical_spell =
                super::super::spell_engine::on_hit::modifiers("proc_bloodburst", modifiers);
            assert_eq!(physical_spell.bleed, if class == 8 { 0.0 } else { chance });
            assert_eq!(
                super::super::spell_engine::on_hit::modifiers("heroic_strike", modifiers).bleed,
                chance
            );
            assert_eq!(
                super::super::spell_engine::on_hit::modifiers("fireball", modifiers).bleed,
                0.0
            );
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn shared_talents_trust_the_clients_build_without_server_budget_checks() {
        let mut build = [[0; NODES]; 3];
        build[0][NODES - 1] = 1;
        let account = Account::from_shared(0, 0, build);
        let mut run = Run::default();
        account.apply(0, &mut run);
        assert_eq!(account.character, None);
        assert_eq!(account.shared_build(0), build);
        assert!(run.abilities.iter().any(|a| a.def.key == TREES[0][0].cap));
    }
    #[test]
    fn beast_mastery_capstone_does_not_grant_concussive_shot_or_arcane_offers() {
        let mut account = account();
        for node in [8, 12, 18] { account.builds[2][0][node] = 1; }
        let mut run = Run { class: &data::CLASSES[2], ..Default::default() };
        run.abilities.push(Owned { def: data::ability(run.class.start), level: 1, cd: 0.5 });
        account.apply(2, &mut run);
        assert_eq!(run.abilities.iter().filter(|a| a.def.key == "kill_command").count(), 1);
        assert!(!run.abilities.iter().any(|a| matches!(a.def.key, "arcane_shot" | "concussive_shot")));
        let arcane = data::PASSIVES.iter().find(|p| p.key == "arcane").unwrap();
        assert!(!super::super::passives::eligible(arcane, &run));
    }

    #[test]
    fn arcane_capstone_keeps_missiles_without_granting_explosion() {
        let mut account = account();
        for node in [8, 12, 18] {
            account.builds[6][0][node] = 1;
        }
        let mut run = Run {
            class: &data::CLASSES[6],
            ..Default::default()
        };
        run.abilities.push(Owned {
            def: data::ability(run.class.start),
            level: 4,
            cd: 0.5,
        });
        account.apply(6, &mut run);
        account.apply(6, &mut run);
        assert_eq!(run.abilities.len(), 1);
        assert_eq!(run.abilities[0].def.key, "arcane_missiles");
        assert_eq!(run.abilities[0].level, 4);
        assert!(super::super::talent_runtime::active(
            &run,
            super::super::talent_design::Mechanic::ArcaneCascade,
        ));
    }

    fn account() -> Account {
        Account {
            character: None,
            xp: threshold(60),
            builds: [[[0; NODES]; 3]; CLASSES],
            last_reward: 0,
            bonus_points: 0,
        }
    }
    #[test]
    fn talents_start_at_level_two_and_reach_fifty_nine_points() {
        assert_eq!(level(0), 1);
        assert_eq!(budget(0), 0);
        assert_eq!(budget(threshold(2)), 1);
        assert_eq!(budget(threshold(9)), 8);
        assert_eq!(budget(threshold(10)), 9);
        assert_eq!(budget(u64::MAX), 59);
        let mut a = Account::fresh();
        assert!(!a.buy(0, 0, 0));
        a.xp = threshold(2);
        assert!(a.buy(0, 0, 0));
        assert_eq!(a.available(0), 0);
        assert!(!a.buy(0, 0, 1));
    }
    #[test]
    fn meta_kill_xp_penalty_uses_monster_gap_in_both_directions() {
        for (gap, expected) in [(9, 1000), (10, 100), (11, 50), (12, 25), (13, 10), (25, 10)] {
            for monster_level in [30 - gap, 30 + gap] {
                let mut run = Run {
                    meta_level: 30,
                    level: 100,
                    ..Run::default()
                };
                for _ in 0..1000 {
                    credit_kill(&mut run, monster_level);
                }
                assert_eq!(earned(&run), expected, "monster level {monster_level}");
            }
        }
    }

    #[test]
    fn meta_kill_penalties_preserve_previous_rewards_when_the_level_changes() {
        let mut run = Run {
            meta_level: 20,
            ..Run::default()
        };
        for _ in 0..10 {
            credit_kill(&mut run, 10);
        }
        assert_eq!(earned(&run), 1);
        run.meta_level = 19;
        credit_kill(&mut run, 10);
        assert_eq!(earned(&run), 2);
    }
    #[test]
    fn ranks_prerequisites_and_shared_tree_budget_are_enforced() {
        let mut a = account();
        assert!(!a.buy(0, 0, 3));
        for _ in 0..5 {
            assert!(a.buy(0, 0, 0));
        }
        assert!(!a.buy(0, 0, 0));
        assert!(a.buy(0, 0, 3));
        assert_eq!(a.available(0), 53);
        assert_eq!(a.available(1), 59);
        a.reset(0);
        assert_eq!(a.available(0), 59);
    }
    #[test]
    fn protection_milestones_never_grant_retaliation_or_duplicate_thunder_clap() {
        let mut account = Account::fresh();
        account.xp = threshold(60);
        let mut run = Run::default();
        run.abilities.push(Owned { def: data::ability(run.class.start), level: 1, cd: 0.5 });
        for (node, ranks) in [(0,5),(1,5),(8,1),(3,5),(4,5),(12,1),(6,5),(9,3),(18,1)] {
            for _ in 0..ranks { assert!(account.buy(0, 2, node)); }
            if [8,12,18].contains(&node) {
                account.apply(0, &mut run);
                assert!(!run.abilities.iter().any(|a| a.def.key == "retaliation"));
                assert_eq!(run.abilities.iter().filter(|a| a.def.key == "thunder_clap").count(), 1);
            }
        }
        assert!(super::super::talent_runtime::active(&run, super::super::talent_design::Mechanic::Bulwark));
        assert_eq!(account.tree_points(0,2), 31);
    }

    #[test]
    fn every_capstone_is_an_existing_class_ability() {
        for (class, trees) in data::CLASSES.iter().zip(TREES.iter()) {
            for spec in trees {
                assert!(class.pool.contains(&spec.cap));
                assert_eq!(data::ability(spec.cap).key, spec.cap);
            }
        }
    }

    #[test]
    fn rewards_are_not_counted_twice_when_leaving_a_run() {
        let mut a = account();
        a.xp = 0;
        let mut run = Run {
            meta_kills: 12,
            elapsed: 30.0,
            ..Run::default()
        };
        settle(&mut a, &mut run, false);
        assert_eq!(a.xp, 0); // Smaller rewards wait for the next batch or a transition.
        settle(&mut a, &mut run, true);
        assert_eq!(a.xp, 18);
        settle(&mut a, &mut run, true);
        assert_eq!(a.xp, 18);
        run.won = true;
        settle(&mut a, &mut run, true);
        assert_eq!(a.xp, 518);
    }

    #[test]
    fn passives_update_character_stats_and_milestones_remain_school_scoped() {
        let mut a = account();
        for _ in 0..5 {
            assert!(a.buy(6, 1, 0));
            assert!(a.buy(6, 1, 1));
        }
        let mut run = Run {
            class: &data::CLASSES[6],
            ..Run::default()
        };
        run.set_global_level(level(a.xp));
        let baseline = run.stats();
        let fire_before = run.resolve_spell(data::ability("fireball"), 1);
        let cold_before = run.resolve_spell(data::ability("frostbolt"), 1);
        a.apply(6, &mut run);
        assert_eq!(run.stats().might, baseline.might);
        assert_eq!(run.stats().crit, baseline.crit);
        let fire = run.resolve_spell(data::ability("fireball"), 1);
        let cold = run.resolve_spell(data::ability("frostbolt"), 1);
        let fire_increase = 0.2 / (1.0 + baseline.level_damage);
        assert!((fire.dmg - fire_before.dmg * (1.0 + fire_increase)).abs() < 0.001);
        assert!((fire.crit - fire_before.crit - 0.05).abs() < 0.001);
        assert_eq!(cold.dmg, cold_before.dmg);
        assert_eq!(cold.crit, cold_before.crit);
        assert!(a.buy(6, 1, 8));
        a.apply(6, &mut run);
        let entries = run.spell_modifiers.entries();
        assert_eq!(
            super::super::spell_engine::on_hit::modifiers("fireball", entries).ignite,
            1.0
        );
        assert_eq!(
            super::super::spell_engine::on_hit::modifiers("frostbolt", entries).ignite,
            0.0
        );
    }

    #[test]
    fn applying_persistent_character_levels_grants_stats_once_and_preserves_missing_health() {
        let mut account = Account::fresh();
        account.xp = threshold(10);
        let mut run = Run {
            hp: 50.0,
            ..Run::default()
        };
        let baseline = run.stats().max_hp;
        account.apply(0, &mut run);
        assert_eq!(run.meta_level, 10);
        assert_eq!(run.stats().max_hp, baseline + 90.0);
        assert_eq!(run.hp, 140.0);
        assert!((run.stats().level_damage - 0.45).abs() < 0.0001);
        account.apply(0, &mut run);
        assert_eq!(run.hp, 140.0);
        assert_eq!(run.stats().max_hp, baseline + 90.0);
    }
    #[test]
    fn health_and_armour_talents_scale_with_equipment_and_reset_without_flat_bonuses() {
        let mut run = Run {
            equipment_armour: 200.0,
            equipment_hp: 100.0,
            ..Run::default()
        };
        let baseline = run.stats();
        run.talent_build[2][1] = 5;
        run.talent_build[2][2] = 5;
        assert!((run.stats().armor - baseline.armor * 1.5).abs() < 0.001);
        assert!((run.stats().max_hp - baseline.max_hp * 1.2).abs() < 0.001);
        run.equipment_armour = 400.0;
        run.equipment_hp = 200.0;
        assert!((run.stats().armor - 600.0).abs() < 0.001);
        assert!((run.stats().max_hp - 432.0).abs() < 0.001);
        assert_eq!(
            super::super::talent_effects::description(&TREES[0][2], 1, 5),
            "+50.0% armor."
        );
        assert_eq!(
            super::super::talent_effects::description(&TREES[0][2], 2, 5),
            "+20.0% maximum health."
        );
        run.talent_build = [[0; NODES]; 3];
        assert!((run.stats().armor - 400.0).abs() < 0.001);
        assert!((run.stats().max_hp - 360.0).abs() < 0.001);
        run.talent_build[2][1] = 5;
        run.equipment_armour = 0.0;
        assert_eq!(run.stats().armor, 0.0);
    }
    #[test]
    fn thirty_one_points_unlock_an_additional_starting_spell() {
        let mut a = account();
        for node in [0, 3, 6] {
            for _ in 0..5 {
                assert!(a.buy(0, 0, node));
            }
        }
        assert!(a.buy(0, 0, 8));
        for _ in 0..5 {
            assert!(a.buy(0, 0, 9));
        }
        assert!(a.buy(0, 0, 12));
        for _ in 0..5 {
            assert!(a.buy(0, 0, 13));
        }
        for _ in 0..3 {
            assert!(a.buy(0, 0, 15));
        }
        assert!(a.buy(0, 0, NODES - 1));
        let mut run = Run::default();
        a.apply(0, &mut run);
        a.apply(0, &mut run);
        assert_eq!(
            run.abilities
                .iter()
                .filter(|a| a.def.key == "mortal_strike")
                .count(),
            1
        );
        assert_eq!(a.available(0), 28);
    }

    #[test]
    fn old_talents_are_refunded_without_losing_xp_and_v4_keeps_new_purchases() {
        let xp = threshold(60);
        let old = format!("v1 {xp} 5 5 5 5");
        let refunded = Account::from_save(&old);
        assert_eq!(refunded.xp, xp);
        for class in 0..CLASSES {
            assert_eq!(refunded.available(class), 59);
        }
        let old_v2 = Account::from_save(&format!("v2 {xp} 5 5"));
        assert_eq!(old_v2.xp, xp);
        assert_eq!(old_v2.available(0), 59);
        let current = Account::from_save(&format!("v4 {xp} 5 5"));
        assert_eq!(current.available(0), 49);
        assert_eq!(current.rank(0, 0, 0), 5);
        assert_eq!(current.rank(0, 0, 1), 5);
    }
}
