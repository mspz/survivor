//! Every achievement and its tiers. Each tier adds a small bonus to the specialty-point gain,
//! larger the harder it is; the spell and expedition families are generated from the data.
use super::super::data;
use std::sync::LazyLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::survivors) enum Category {
    General,
    Expeditions,
    Spells,
    Equipment,
    Challenges,
    Feats,
}

impl Category {
    pub const ALL: [Self; 6] = [
        Self::General,
        Self::Expeditions,
        Self::Spells,
        Self::Equipment,
        Self::Challenges,
        Self::Feats,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Expeditions => "Expeditions",
            Self::Spells => "Spells",
            Self::Equipment => "Equipment",
            Self::Challenges => "Challenges",
            Self::Feats => "Feats",
        }
    }
}

/// How a value is read off the progress and compared with a tier's target.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::survivors) enum Metric {
    /// A counter or a best value; reached when it is at least the target.
    Counter(String),
    /// A best time in seconds; reached when it is at most the target.
    Time(String),
    /// How many counters start with this prefix: distinct maps, classes or types.
    Distinct(String),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::survivors) struct Tier {
    pub target: u64,
    /// Added to the specialty-point gain multiplier, as a fraction (0.004 = +0.4%).
    pub bonus: f32,
}

#[derive(Clone, Debug)]
pub(in crate::survivors) struct Achievement {
    pub id: String,
    pub name: String,
    /// Describes one tier: `{}` is replaced by its target.
    pub text: String,
    pub category: Category,
    /// The sub-category the list groups it under.
    pub group: String,
    pub metric: Metric,
    pub tiers: Vec<Tier>,
}

impl Achievement {
    pub fn describe(&self, tier: usize) -> String {
        let target = self.tiers[tier.min(self.tiers.len() - 1)].target;
        self.text.replace("{}", &format_target(&self.metric, target))
    }
}

pub(in crate::survivors) fn format_target(metric: &Metric, value: u64) -> String {
    match metric {
        Metric::Time(_) => format!("{value} s"),
        Metric::Counter(key) if key == "gold" => format!("{} gold", group(value / 10_000)),
        _ => group(value),
    }
}

/// `1234567` as `1,234,567`.
pub(in crate::survivors) fn group(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// The bonus of a tier by its difficulty.
const EASY: f32 = 0.002;
const MEDIUM: f32 = 0.004;
const HARD: f32 = 0.008;
const ELITE: f32 = 0.015;
const INSANE: f32 = 0.03;

fn tiers(list: &[(u64, f32)]) -> Vec<Tier> {
    list.iter()
        .map(|&(target, bonus)| Tier { target, bonus })
        .collect()
}

fn add(
    out: &mut Vec<Achievement>,
    category: Category,
    group: impl Into<String>,
    id: impl Into<String>,
    name: impl Into<String>,
    text: impl Into<String>,
    metric: Metric,
    list: &[(u64, f32)],
) {
    out.push(Achievement {
        id: id.into(),
        name: name.into(),
        text: text.into(),
        category,
        group: group.into(),
        metric,
        tiers: tiers(list),
    });
}

fn counter(key: &str) -> Metric {
    Metric::Counter(key.into())
}

/// `fire_blast` as `Fire Blast`.
pub(in crate::survivors) fn title(key: &str) -> String {
    key.split('_')
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(chars).collect()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The damaging spells with the class that first has them, in class order.
pub(in crate::survivors) fn damaging_spells_by_class() -> Vec<(&'static str, &'static str)> {
    let mut out: Vec<(&'static str, &'static str)> = Vec::new();
    for class in data::CLASSES {
        for &key in std::iter::once(&class.start).chain(class.pool) {
            let def = data::ability(key);
            if def.key == key && !super::support(def) && !out.iter().any(|&(_, k)| k == key) {
                out.push((class.name, key));
            }
        }
    }
    out
}

/// The challenges a victory can meet, as `(key, name, condition)`.
pub(in crate::survivors) const CHALLENGES: [(&str, &str, &str, [f32; 3]); 11] = [
    ("no_spells", "Purist", "without learning a new spell", [HARD, ELITE, INSANE]),
    ("one_spell", "Specialist", "learning at most one new spell", [MEDIUM, HARD, ELITE]),
    ("no_support", "No Backup", "without a support spell", [MEDIUM, HARD, ELITE]),
    ("no_draft", "Take It or Leave It", "without rerolling, banishing or skipping", [MEDIUM, HARD, ELITE]),
    ("no_cards", "Ascetic", "without taking a single card", [ELITE, INSANE, INSANE]),
    ("common_only", "Humble", "without taking a card above Common", [HARD, ELITE, INSANE]),
    ("no_passive", "Raw Power", "without taking a passive", [MEDIUM, HARD, ELITE]),
    ("no_death", "Flawless", "without falling", [HARD, ELITE, INSANE]),
    ("solo", "Lone Wolf", "alone", [EASY, MEDIUM, HARD]),
    ("naked", "Naked and Afraid", "with nothing equipped", [ELITE, INSANE, INSANE]),
    ("pacifist", "Pacifist", "without defeating a single enemy yourself", [ELITE, INSANE, INSANE]),
];

fn build() -> Vec<Achievement> {
    use Category::*;
    let mut out = Vec::new();

    // --- General: lifetime grinds. ---
    add(&mut out, General, "Combat", "kills", "Exterminator", "Defeat {} enemies", counter("kills"),
        &[(1_000, EASY), (10_000, EASY), (50_000, MEDIUM), (250_000, MEDIUM), (1_000_000, HARD), (5_000_000, ELITE), (25_000_000, INSANE)]);
    add(&mut out, General, "Combat", "bosses", "Boss Hunter", "Defeat {} bosses", counter("bosses"),
        &[(1, EASY), (10, EASY), (50, MEDIUM), (200, HARD), (1_000, ELITE), (5_000, INSANE)]);
    add(&mut out, General, "Journey", "wins", "Conqueror", "Complete {} expeditions", counter("wins"),
        &[(1, EASY), (10, MEDIUM), (50, MEDIUM), (150, HARD), (500, ELITE), (2_000, INSANE)]);
    add(&mut out, General, "Journey", "runs", "Adventurer", "Start {} expeditions", counter("runs"),
        &[(5, EASY), (25, EASY), (100, MEDIUM), (500, HARD), (2_000, ELITE)]);
    add(&mut out, General, "Combat", "deaths", "Not My Day", "Fall {} times", counter("deaths"),
        &[(1, EASY), (10, EASY), (50, MEDIUM), (200, MEDIUM), (1_000, HARD)]);
    add(&mut out, General, "Journey", "level", "Coming of Age", "Reach character level {}", counter("level"),
        &[(10, EASY), (20, EASY), (30, MEDIUM), (40, MEDIUM), (50, HARD), (60, HARD)]);
    add(&mut out, General, "Journey", "paragon", "Paragon", "Reach Paragon level {}", counter("paragon"),
        &[(1, MEDIUM), (10, MEDIUM), (25, HARD), (50, HARD), (100, ELITE), (250, INSANE)]);
    add(&mut out, General, "Wealth", "gold", "Treasure Hunter", "Earn {}", counter("gold"),
        &[(100_000, EASY), (1_000_000, MEDIUM), (10_000_000, HARD), (100_000_000, ELITE), (1_000_000_000, INSANE)]);
    add(&mut out, General, "Journey", "classes", "Jack of All Trades", "Complete an expedition with {} different classes", Metric::Distinct("class_win:".into()),
        &[(1, EASY), (3, MEDIUM), (6, HARD), (9, ELITE)]);
    add(&mut out, General, "Journey", "coop", "Brothers in Arms", "Complete {} expeditions in a group", counter("coop_wins"),
        &[(1, EASY), (10, MEDIUM), (50, HARD), (200, ELITE)]);
    add(&mut out, General, "Wealth", "sp_spent", "Investor", "Spend {} specialty points", counter("sp_spent"),
        &[(1_000, EASY), (10_000, MEDIUM), (100_000, HARD), (1_000_000, ELITE), (10_000_000, INSANE)]);

    // --- Expeditions: one family per campaign map. ---
    for &map in data::CAMPAIGN {
        let name = data::MAPS[map].name;
        add(&mut out, Expeditions, name, format!("map_wins:{map}"), format!("{name}: Conqueror"),
            format!("Complete {name} {{}} times"), counter(&format!("map_wins:{map}")),
            &[(1, EASY), (5, MEDIUM), (25, HARD), (100, ELITE)]);
        add(&mut out, Expeditions, name, format!("map_kills:{map}"), format!("{name}: Purge"),
            format!("Defeat {{}} enemies in {name}"), counter(&format!("map_kills:{map}")),
            &[(1_000, EASY), (10_000, MEDIUM), (100_000, HARD), (1_000_000, ELITE)]);
        add(&mut out, Expeditions, name, format!("map_kills_run:{map}"), format!("{name}: Massacre"),
            format!("Defeat {{}} enemies in a single {name} expedition"), counter(&format!("map_kills_run:{map}")),
            &[(500, EASY), (1_500, MEDIUM), (3_000, HARD), (6_000, ELITE)]);
        add(&mut out, Expeditions, name, format!("boss_time:{map}"), format!("{name}: Boss Rush"),
            format!("Defeat a {name} boss within {{}} of its arrival"), Metric::Time(format!("boss_time:{map}")),
            &[(120, EASY), (60, MEDIUM), (30, HARD), (15, ELITE), (8, INSANE)]);
        for (key, title, condition, bonus) in CHALLENGES {
            add(&mut out, Expeditions, name, format!("challenge:{key}:{map}"), format!("{name}: {title}"),
                format!("Complete {name} {{}} times {condition}"), counter(&format!("challenge_wins:{key}:{map}")),
                &[(1, bonus[0] / 4.0), (5, bonus[1] / 4.0), (20, bonus[2] / 4.0)]);
        }
    }
    add(&mut out, Expeditions, "Campaign", "kills_run", "Massacre", "Defeat {} enemies in a single expedition", counter("kills_run"),
        &[(500, EASY), (1_000, MEDIUM), (2_000, MEDIUM), (3_500, HARD), (5_000, ELITE), (8_000, INSANE)]);
    add(&mut out, Expeditions, "Campaign", "campaign", "World Traveller", "Complete {} different expeditions", Metric::Distinct("map_wins:".into()),
        &[(1, EASY), (3, MEDIUM), (6, HARD)]);

    // --- Spells: kills with each damaging spell. ---
    for (class, key) in damaging_spells_by_class() {
        let name = title(key);
        add(&mut out, Spells, class, format!("spell:{key}"), format!("{name} Specialist"),
            format!("Defeat {{}} enemies with {name}"), counter(&format!("spell:{key}")),
            &[(100, 0.0003), (1_000, 0.0006), (10_000, 0.0012), (100_000, 0.0025), (1_000_000, 0.005)]);
    }
    for class in data::CLASSES {
        for &key in std::iter::once(&class.start).chain(class.pool) {
            let def = data::ability(key);
            if def.key != key || out.iter().any(|a: &Achievement| a.id == format!("spell_max:{key}")) {
                continue;
            }
            let name = title(key);
            add(&mut out, Spells, class.name, format!("spell_max:{key}"), format!("{name} Mastered"),
                format!("Raise {name} to its top rank in {{}} expeditions"), counter(&format!("spell_max:{key}")),
                &[(1, 0.0005), (5, 0.001), (25, 0.002)]);
        }
    }
    add(&mut out, Spells, "Arsenal", "spells_owned", "Arsenal", "Own {} spells at once", counter("spells_owned"),
        &[(4, EASY), (6, MEDIUM), (8, HARD), (10, ELITE)]);
    add(&mut out, Spells, "Arsenal", "max_spells", "Mastery", "Raise {} spells to their top rank in one expedition", counter("max_spells"),
        &[(1, EASY), (3, MEDIUM), (5, HARD), (8, ELITE)]);
    add(&mut out, Spells, "Arsenal", "spell_families", "Polymath", "Defeat 100 enemies with {} different spells", Metric::Distinct("spell100:".into()),
        &[(5, EASY), (15, MEDIUM), (30, HARD), (60, ELITE)]);

    // --- Equipment. ---
    for (key, name, list) in [
        ("magic", "Well Equipped", [(5, EASY), (10, EASY), (17, MEDIUM)]),
        ("epic", "Epic Wardrobe", [(5, MEDIUM), (10, HARD), (17, ELITE)]),
        ("legendary", "Living Legend", [(3, HARD), (8, ELITE), (17, INSANE)]),
    ] {
        add(&mut out, Equipment, "Quality", format!("slots:{key}"), name,
            format!("Wear {{}} pieces of {key} quality or better at once"), counter(&format!("slots:{key}")), &list);
    }
    add(&mut out, Equipment, "Item level", "ilvl", "Geared Up", "Reach an average equipped item level of {}", counter("ilvl"),
        &[(10, EASY), (20, EASY), (30, MEDIUM), (40, MEDIUM), (50, HARD), (55, ELITE), (60, INSANE)]);
    add(&mut out, Equipment, "Collection", "types", "Collector", "Wear {} different kinds of equipment", Metric::Distinct("type:".into()),
        &[(4, EASY), (8, MEDIUM), (12, HARD), (14, ELITE)]);
    add(&mut out, Equipment, "Collection", "full_set", "Fully Dressed", "Fill all {} equipment slots", counter("slots:any"),
        &[(19, MEDIUM)]);

    // --- Challenges: victories under a constraint, on distinct expeditions. ---
    for (key, name, condition, bonus) in CHALLENGES {
        let group = if matches!(key, "no_death" | "solo" | "naked" | "no_support" | "pacifist") { "Survival" } else { "Cards and spells" };
        add(&mut out, Challenges, group, format!("challenge:{key}"), name,
            format!("Complete {{}} different expeditions {condition}"), Metric::Distinct(format!("challenge:{key}:")),
            &[(1, bonus[0]), (3, bonus[1]), (6, bonus[2])]);
    }

    // --- Feats: records and oddities. ---
    add(&mut out, Feats, "Power", "damage_run", "Devastation", "Deal {} damage in a single expedition", counter("damage_run"),
        &[(100_000, EASY), (1_000_000, MEDIUM), (10_000_000, MEDIUM), (100_000_000, HARD), (1_000_000_000, ELITE)]);
    add(&mut out, Feats, "Power", "hit", "One Punch", "Land a single hit of {}", counter("hit"),
        &[(1_000, EASY), (10_000, MEDIUM), (100_000, HARD), (1_000_000, ELITE), (10_000_000, INSANE)]);
    add(&mut out, Feats, "Cards", "legendary_run", "Jackpot", "Take {} Legendary cards in one expedition", counter("legendary_run"),
        &[(1, EASY), (3, MEDIUM), (6, HARD), (10, ELITE)]);
    add(&mut out, Feats, "Cards", "rerolls_run", "Never Satisfied", "Reroll {} times in one expedition", counter("rerolls_run"),
        &[(3, EASY), (5, MEDIUM), (10, HARD)]);
    add(&mut out, Feats, "Cards", "banishes_run", "Picky Eater", "Banish {} cards in one expedition", counter("banishes_run"),
        &[(3, EASY), (5, MEDIUM), (10, HARD)]);
    add(&mut out, Feats, "Cards", "stored", "Hoarder", "Keep {} cards in reserve at once", counter("stored"),
        &[(10, EASY), (25, MEDIUM), (50, HARD), (100, ELITE)]);
    for (percent, name, bonus) in [(10, "By a Thread", [MEDIUM, HARD, ELITE]), (5, "Hanging by a Hair", [HARD, ELITE, INSANE]), (1, "Death's Door", [ELITE, INSANE, INSANE])] {
        add(&mut out, Feats, "Daring", format!("close_call:{percent}"), name,
            format!("Complete {{}} expeditions without falling after dropping below {percent}% health"), counter(&format!("close_call:{percent}")),
            &[(1, bonus[0]), (5, bonus[1]), (25, bonus[2])]);
    }
    add(&mut out, Feats, "Daring", "boss_any", "Speed Demon", "Defeat any boss within {} of its arrival", Metric::Time("boss_time:any".into()),
        &[(60, EASY), (20, MEDIUM), (10, HARD), (5, ELITE), (3, INSANE)]);
    add(&mut out, Feats, "Daring", "challenges", "Masochist", "Meet {} different challenges", Metric::Distinct("challenge_any:".into()),
        &[(1, MEDIUM), (5, HARD), (10, ELITE)]);
    out
}

static ALL: LazyLock<Vec<Achievement>> = LazyLock::new(build);

pub(in crate::survivors) fn all() -> &'static [Achievement] {
    &ALL
}
