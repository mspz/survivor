//! The promotions and their nodes. The first promotions have few ranks worth a lot; each one
//! after has more ranks (10, 25, 50, 100, 250), each worth less, from a far higher base price.
//! Every promotion repeats the core stats (damage, health) and adds rarer effects.

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::survivors) enum Effect {
    Damage,
    MaxHealth,
    Movement,
    Pickup,
    Experience,
    Armour,
    Regeneration,
    CritChance,
    CritDamage,
    Cooldown,
    Area,
    Duration,
    AttackSpeed,
    GoldFind,
    Rerolls,
    Resistance,
    BossDamage,
    LifeLeech,
    Dodge,
    DamageReduction,
    MagicFind,
    Banishes,
    Skips,
    ProjectileChance,
    CardLuck,
    ChainChance,
    RecastChance,
    DotDamage,
    ArmourPenetration,
    MeleeTargetChance,
    /// Raises the share of the crit chance each further multi-crit rolls with.
    MultiCritChance,
    FinalDamage,
    ForkChance,
    MultiCritChain,
    /// An extra projectile, melee target and chain, each by its own roll.
    MultiTarget,
    /// Faster everything: casts, attacks and cooldowns.
    Haste,
    LowLifeDamage,
    FullLifeDamage,
    IgniteChance,
    BleedChance,
    BuffEffect,
    /// Relative bonus on the rune upgrade chance.
    RuneUpgrade,
    /// Chance for a dropped rune to be one rarity higher.
    RuneTier,
    /// Chance for a crafted item to roll twice and keep the better result.
    CraftLuck,
    TalentPoints,
    /// Card draws in reserve at the start of each expedition.
    StartingCards,
    /// Opens multi-crits: a critical strike can crit again.
    MultiCritUnlock,
    /// Extra damage of each multi-crit, as a share of the hit.
    MultiCritPower,
}

impl Effect {
    /// One rank's value as the node list shows it.
    pub fn describe(self, value: f32) -> String {
        let pct = |label: &str| {
            let p = value * 100.0;
            if p < 1.0 && p > 0.0 {
                format!("+{p:.2}% {label}")
            } else {
                format!("+{p:.1}% {label}")
            }
        };
        let count = |one: &str, many: &str| {
            format!("+{value:.0} {}", if value == 1.0 { one } else { many })
        };
        match self {
            Self::Damage => pct("damage"),
            Self::MaxHealth => pct("maximum health"),
            Self::Movement => pct("movement speed"),
            Self::Pickup => pct("pickup radius"),
            Self::Experience => pct("expedition experience"),
            Self::Armour => pct("armour"),
            Self::Regeneration => pct("maximum health regenerated per second"),
            Self::CritChance => pct("critical strike chance"),
            Self::CritDamage => pct("critical strike damage"),
            Self::Cooldown => pct("cooldown recovery"),
            Self::Area => pct("area of effect"),
            Self::Duration => pct("effect duration"),
            Self::AttackSpeed => pct("attack speed"),
            Self::GoldFind => pct("gold found"),
            Self::Rerolls => count("reroll per expedition", "rerolls per expedition"),
            Self::Resistance => pct("to all elemental resistances"),
            Self::BossDamage => pct("damage against bosses"),
            Self::LifeLeech => pct("of damage leeched as life"),
            Self::Dodge => pct("dodge chance"),
            Self::DamageReduction => pct("damage reduction"),
            Self::MagicFind => pct("magic find"),
            Self::Banishes => count("banish per expedition", "banishes per expedition"),
            Self::Skips => count("skip per expedition", "skips per expedition"),
            Self::ProjectileChance => pct("chance of an extra projectile"),
            Self::CardLuck => pct("card luck: better card rarities"),
            Self::ChainChance => pct("chance of an extra chain"),
            Self::RecastChance => pct("chance to recast a spell"),
            Self::DotDamage => pct("damage over time"),
            Self::ArmourPenetration => pct("armour penetration"),
            Self::MeleeTargetChance => pct("chance to hit an extra melee target"),
            Self::MultiCritChance => pct("of the crit chance for each further multi-crit"),
            Self::FinalDamage => pct("final damage (multiplicative)"),
            Self::ForkChance => pct("chance for projectiles to fork"),
            Self::MultiCritChain => count("multi-crit in a chain", "multi-crits in a chain"),
            Self::MultiTarget => pct("chance of an extra target (projectile, melee and chain)"),
            Self::Haste => pct("haste: casts, attacks and cooldowns"),
            Self::LowLifeDamage => pct("damage against enemies on low life"),
            Self::FullLifeDamage => pct("damage against enemies on full life"),
            Self::IgniteChance => pct("chance to ignite"),
            Self::BleedChance => pct("chance to cause bleeding"),
            Self::BuffEffect => pct("effect of your buffs"),
            Self::RuneUpgrade => pct("rune upgrade chance (relative)"),
            Self::RuneTier => pct("chance for a found rune to be one rarity higher"),
            Self::CraftLuck => pct("chance to craft twice and keep the better item"),
            Self::TalentPoints => count("talent point", "talent points"),
            Self::StartingCards => count(
                "card draw in reserve at the start of an expedition",
                "card draws in reserve at the start of an expedition",
            ),
            Self::MultiCritUnlock => "A critical strike can crit again, with a third of the crit chance; each further crit a third again".into(),
            Self::MultiCritPower => pct("damage of each multi-crit"),
        }
    }
}

pub(in crate::survivors) struct Node {
    pub id: &'static str,
    pub name: &'static str,
    pub effect: Effect,
    /// Per rank.
    pub value: f32,
    pub ranks: u8,
    /// The first rank's cost, before the promotion's multiplier.
    pub cost: u32,
    /// Few-rank special nodes: each rank costs three times the last. Otherwise see [`growth`].
    pub steep: bool,
}

/// How many times the first rank's price the last rank of a long node costs: as much as for
/// a ten-rank node (1.3 to the ninth).
const SPAN: f64 = 10.604;

/// What opens a promotion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::survivors) enum Unlock {
    Always,
    /// Every campaign expedition completed at least once, by any character.
    Campaign,
    /// Content still to come: the condition is shown, but it cannot be met yet.
    Future(&'static str),
}

pub(in crate::survivors) struct Promotion {
    pub name: &'static str,
    pub unlock: Unlock,
    /// How far the costs rise over the first promotion's.
    pub scale: u32,
    pub nodes: &'static [Node],
}

const fn node(
    id: &'static str,
    name: &'static str,
    effect: Effect,
    value: f32,
    ranks: u8,
    cost: u32,
) -> Node {
    Node {
        id,
        name,
        effect,
        value,
        ranks,
        cost,
        steep: false,
    }
}

/// Few, very expensive ranks.
const fn rare(
    id: &'static str,
    name: &'static str,
    effect: Effect,
    value: f32,
    ranks: u8,
    cost: u32,
) -> Node {
    Node {
        steep: true,
        ..node(id, name, effect, value, ranks, cost)
    }
}

use Effect::*;

pub(in crate::survivors) static PROMOTIONS: &[Promotion] = &[
    // Few ranks, each worth a lot.
    Promotion {
        name: "Beginner",
        unlock: Unlock::Always,
        scale: 1,
        nodes: &[
            node("b_damage", "Sharpened Edge", Damage, 0.05, 10, 100),
            node("b_health", "Vitality", MaxHealth, 0.05, 10, 100),
            node("b_armour", "Thick Skin", Armour, 0.02, 5, 90),
            node("b_crit", "Keen Eye", CritChance, 0.005, 5, 150),
            node("b_speed", "Light Feet", Movement, 0.01, 5, 120),
            node("b_pickup", "Magnetism", Pickup, 0.03, 5, 80),
            node("b_xp", "Quick Study", Experience, 0.02, 10, 100),
            node("b_gold", "Pocket Change", GoldFind, 0.04, 5, 80),
            node("b_regen", "Second Wind", Regeneration, 0.001, 5, 150),
            node("b_resist", "Warding", Resistance, 0.01, 5, 90),
        ],
    },
    Promotion {
        name: "Rookie",
        unlock: Unlock::Campaign,
        scale: 4,
        nodes: &[
            node("r_damage", "Honed Blade", Damage, 0.02, 25, 100),
            node("r_health", "Constitution", MaxHealth, 0.02, 25, 100),
            node("r_critdmg", "Brutality", CritDamage, 0.012, 25, 100),
            node("r_attack", "Fury", AttackSpeed, 0.004, 25, 110),
            node("r_cooldown", "Focus", Cooldown, 0.004, 25, 120),
            node("r_area", "Reach", Area, 0.008, 25, 100),
            node("r_duration", "Persistence", Duration, 0.01, 10, 100),
            node("r_dodge", "Footwork", Dodge, 0.002, 25, 120),
            node("r_resist", "Elemental Warding", Resistance, 0.004, 25, 90),
            node("r_gold", "Fortune", GoldFind, 0.016, 25, 80),
            node("r_craft", "Steady Hands", CraftLuck, 0.004, 25, 120),
            rare("r_reroll", "Second Thoughts", Rerolls, 1.0, 3, 250),
            rare("r_stash", "Head Start", StartingCards, 1.0, 3, 300),
            rare("r_talent", "Insight", TalentPoints, 1.0, 5, 400),
        ],
    },
    Promotion {
        name: "Adept",
        unlock: Unlock::Future("Complete every raid (raids are coming in a later update)"),
        scale: 16,
        nodes: &[
            node("a_damage", "Lethal Training", Damage, 0.01, 50, 100),
            node("a_health", "Fortitude", MaxHealth, 0.01, 50, 100),
            node("a_boss", "Giant Slayer", BossDamage, 0.008, 25, 100),
            node("a_execute", "Executioner", LowLifeDamage, 0.006, 25, 110),
            node("a_multi", "Volley", MultiTarget, 0.004, 25, 130),
            node("a_haste", "Quickening", Haste, 0.002, 25, 250),
            node("a_leech", "Bloodthirst", LifeLeech, 0.0004, 25, 150),
            node("a_dodge", "Evasion", Dodge, 0.002, 25, 120),
            node("a_reduction", "Bulwark", DamageReduction, 0.002, 25, 120),
            node("a_magic", "Treasure Sense", MagicFind, 0.016, 25, 90),
            node("a_rune", "Runesmith", RuneUpgrade, 0.01, 25, 120),
            rare("a_banish", "Exile", Banishes, 1.0, 3, 250),
            rare("a_skip", "Patience", Skips, 1.0, 3, 250),
            rare("a_talent", "Deep Insight", TalentPoints, 1.0, 5, 400),
        ],
    },
    Promotion {
        name: "Expert",
        unlock: Unlock::Future("Complete every raid with a modifier (coming later)"),
        scale: 64,
        nodes: &[
            node("e_damage", "Mastery of Arms", Damage, 0.005, 100, 100),
            node("e_health", "Iron Body", MaxHealth, 0.005, 100, 100),
            node("e_opener", "Ambush", FullLifeDamage, 0.004, 50, 110),
            node("e_dot", "Affliction", DotDamage, 0.006, 50, 100),
            node("e_ignite", "Kindling", IgniteChance, 0.002, 50, 110),
            node("e_bleed", "Serrated Edge", BleedChance, 0.002, 50, 110),
            node("e_chain", "Arc", ChainChance, 0.002, 50, 110),
            node("e_cleave", "Cleave", MeleeTargetChance, 0.002, 50, 110),
            node("e_recast", "Echo", RecastChance, 0.001, 50, 140),
            node("e_pen", "Penetration", ArmourPenetration, 0.002, 50, 110),
            node("e_haste", "Alacrity", Haste, 0.001, 50, 250),
            node("e_luck", "Lucky Draw", CardLuck, 0.002, 50, 120),
            node("e_craft", "Master Craftsman", CraftLuck, 0.004, 50, 120),
            node("e_runetier", "Rune Lore", RuneTier, 0.002, 50, 140),
            rare("e_multicrit", "Cascade", MultiCritUnlock, 1.0, 1, 1_500),
            node("e_overkill", "Overkill", MultiCritPower, 0.005, 50, 130),
        ],
    },
    Promotion {
        name: "Master",
        unlock: Unlock::Future("Complete a Torment I expedition (Torment is coming later)"),
        scale: 256,
        nodes: &[
            node("m_damage", "Ruin", Damage, 0.003, 250, 100),
            node("m_health", "Colossus", MaxHealth, 0.003, 250, 100),
            node("m_final", "Devastation", FinalDamage, 0.001, 100, 140),
            node("m_multicrit", "Cascade Echo", MultiCritChance, 0.0005, 100, 150),
            node("m_overkill", "Overwhelm", MultiCritPower, 0.0025, 100, 140),
            rare("m_chain", "Avalanche", MultiCritChain, 1.0, 1, 2_000),
            node("m_fork", "Splinter", ForkChance, 0.001, 100, 120),
            node("m_volley", "Barrage", ProjectileChance, 0.001, 100, 120),
            node("m_buff", "Inspiration", BuffEffect, 0.0025, 100, 110),
            node("m_resist", "Prismatic Ward", Resistance, 0.001, 100, 100),
            node("m_luck", "Jackpot", CardLuck, 0.001, 100, 140),
            node("m_magic", "Relic Hunter", MagicFind, 0.005, 100, 110),
            node("m_rune", "Rune Mastery", RuneUpgrade, 0.0025, 100, 130),
        ],
    },
    // Many ranks, each worth little, at the highest prices.
    Promotion {
        name: "Grandmaster",
        unlock: Unlock::Future("Complete a Torment X expedition (Torment is coming later)"),
        scale: 1024,
        nodes: &[
            node("g_damage", "Annihilation", Damage, 0.002, 250, 100),
            node("g_health", "Titan", MaxHealth, 0.002, 250, 100),
            node("g_final", "Cataclysm", FinalDamage, 0.001, 100, 140),
            rare("g_chain", "Endless Cascade", MultiCritChain, 1.0, 2, 2_000),
            node("g_recast", "Resonance", RecastChance, 0.001, 100, 140),
            node("g_multi", "Legion", MultiTarget, 0.001, 100, 140),
            node("g_haste", "Time Warp", Haste, 0.0005, 100, 250),
            node("g_runetier", "Ancient Runes", RuneTier, 0.001, 100, 140),
            rare("g_talent", "Enlightenment", TalentPoints, 1.0, 5, 500),
        ],
    },
];

/// Each rank's price over the last: 1.3 up to ten ranks, as the Beginner nodes; a longer node
/// rises so its last rank costs as many times its first as a ten-rank node does.
fn growth(node: &Node) -> f64 {
    if node.steep {
        3.0
    } else if node.ranks <= 10 {
        1.3
    } else {
        SPAN.powf(1.0 / f64::from(node.ranks - 1))
    }
}

/// The cost of the next rank of a node, from `rank` (the ranks already bought).
pub(in crate::survivors) fn cost(promotion: &Promotion, node: &Node, rank: u8) -> u64 {
    let raw =
        f64::from(node.cost) * f64::from(promotion.scale) * growth(node).powi(i32::from(rank));
    // Round to tens past ten thousand so large prices read cleanly.
    if raw >= 10_000.0 {
        ((raw / 10.0).round() * 10.0) as u64
    } else {
        raw.round().max(1.0) as u64
    }
}

pub(in crate::survivors) fn find(id: &str) -> Option<(&'static Promotion, &'static Node)> {
    PROMOTIONS
        .iter()
        .find_map(|p| p.nodes.iter().find(|n| n.id == id).map(|n| (p, n)))
}
