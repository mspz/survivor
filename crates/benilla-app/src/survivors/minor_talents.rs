//! Sixteen authored minor talents per specialization. All values are per rank.
use super::{
    data::Stat,
    spell_engine::{Change, Filter, Tags},
    talent_design,
    talents::Spec,
};
use Change as C;
use Tags as T;

#[derive(Clone, Copy)]
pub(super) struct Minor {
    pub name: &'static str,
    pub filter: Filter,
    pub stat: Option<(Stat, f32)>,
    pub change: Option<Change>,
}
macro_rules! m {
    ($name:literal,$tags:expr,$change:expr) => {
        Minor {
            name: $name,
            filter: Filter::requiring($tags),
            stat: None,
            change: Some($change),
        }
    };
}
macro_rules! s {
    ($name:literal,$stat:ident,$value:expr) => {
        Minor {
            name: $name,
            filter: Filter::requiring(Tags(0)),
            stat: Some((Stat::$stat, $value)),
            change: None,
        }
    };
}
macro_rules! k {
    ($name:literal,$skill:literal,$change:expr) => {
        Minor {
            name: $name,
            filter: Filter {
                skill: Some($skill),
                ..Filter::requiring(Tags(0))
            },
            stat: None,
            change: Some($change),
        }
    };
}

pub(super) fn minor(spec: &Spec, node: usize) -> &'static Minor {
    assert!(talent_design::milestone(node).is_none());
    let offset = node
        - [8, 12, 18]
            .into_iter()
            .filter(|&milestone| milestone < node)
            .count();
    &MINORS[talent_design::index(spec)][offset]
}

static MINORS: [[Minor; 16]; 27] = [
    [
        // Warrior / Arms
        m!(
            "Focused Power",
            Tags(T::PHYSICAL.0),
            C::IncreasedDamage(0.04)
        ),
        s!("Armour Training", Armor, 0.06),
        s!("Enduring Spirit", MaxHp, 0.03),
        m!(
            "Relentless Assault",
            Tags(T::PHYSICAL.0),
            C::IncreasedAttackSpeed(0.025)
        ),
        m!("Precision", Tags(T::PHYSICAL.0), C::CriticalChance(0.01)),
        m!(
            "Devastation",
            Tags(T::PHYSICAL.0),
            C::CriticalMultiplier(0.05)
        ),
        m!(
            "Exposed Armour",
            Tags(T::PHYSICAL.0),
            C::ArmourPenetration(0.025)
        ),
        m!("Sharpened Impact", Tags(T::PHYSICAL.0), C::FlatDamage(1.0)),
        m!("Wide Swings", Tags(T::MELEE.0), C::IncreasedArea(0.04)),
        m!(
            "Giant Slayer",
            Tags(T::PHYSICAL.0),
            C::DamageAgainstBosses(0.04)
        ),
        m!(
            "Finishing Touch",
            Tags(T::PHYSICAL.0),
            C::DamageAgainstLowLife(0.04)
        ),
        m!(
            "Opening Gambit",
            Tags(T::PHYSICAL.0),
            C::DamageAgainstFullLife(0.04)
        ),
        m!(
            "Sustaining Strikes",
            Tags(T::PHYSICAL.0),
            C::LifeLeech(0.006)
        ),
        m!(
            "Practiced Rhythm",
            Tags(T::PHYSICAL.0),
            C::IncreasedCooldownRecovery(0.025)
        ),
        s!("Light Footwork", Speed, 0.02),
        m!("Long Reach", Tags(T::MELEE.0), C::FlatRadius(0.15)),
    ],
    [
        // Warrior / Fury
        m!(
            "Relentless Assault",
            Tags(T::ATTACK.0),
            C::IncreasedAttackSpeed(0.025)
        ),
        m!("Focused Power", Tags(T::ATTACK.0), C::IncreasedDamage(0.04)),
        s!("Enduring Spirit", MaxHp, 0.03),
        m!(
            "Deepening Wounds",
            Tags(T::PHYSICAL.0),
            C::IncreasedDotDamage(0.05)
        ),
        m!(
            "Persistent Wounds",
            Tags(T::PHYSICAL.0 | T::DOT.0),
            C::IncreasedDuration(0.04)
        ),
        m!("Precision", Tags(T::ATTACK.0), C::CriticalChance(0.01)),
        m!(
            "Devastation",
            Tags(T::ATTACK.0),
            C::CriticalMultiplier(0.05)
        ),
        m!(
            "Sustaining Strikes",
            Tags(T::PHYSICAL.0),
            C::LifeLeech(0.006)
        ),
        m!(
            "Finishing Touch",
            Tags(T::ATTACK.0),
            C::DamageAgainstLowLife(0.04)
        ),
        m!(
            "Giant Slayer",
            Tags(T::ATTACK.0),
            C::DamageAgainstBosses(0.04)
        ),
        m!(
            "Practiced Rhythm",
            Tags(T::ATTACK.0),
            C::IncreasedCooldownRecovery(0.025)
        ),
        s!("Light Footwork", Speed, 0.02),
        m!("Sharpened Impact", Tags(T::ATTACK.0), C::FlatDamage(1.0)),
        m!("Wide Swings", Tags(T::MELEE.0), C::IncreasedArea(0.04)),
        s!("Armour Training", Armor, 0.06),
        m!(
            "Martial Training",
            Tags(T::PHYSICAL.0),
            C::IncreasedDamage(0.04)
        ),
    ],
    [
        // Warrior / Protection
        s!("Elemental Bulwark", AllResistance, 2.0),
        s!("Shield Discipline", Armor, 0.1),
        s!("Stalwart Heart", MaxHp, 0.04),
        m!(
            "Expanding Reach",
            Tags(T::PHYSICAL.0),
            C::IncreasedArea(0.04)
        ),
        m!(
            "Practiced Rhythm",
            Tags(T::PHYSICAL.0),
            C::IncreasedCooldownRecovery(0.025)
        ),
        k!(
            "Thunderous Rhythm",
            "thunder_clap",
            C::IncreasedCooldownRecovery(0.03)
        ),
        k!("Crushing Thunder", "thunder_clap", C::IncreasedDamage(0.05)),
        m!("Sharpened Impact", Tags(T::PHYSICAL.0), C::FlatDamage(1.0)),
        m!(
            "Giant Slayer",
            Tags(T::PHYSICAL.0),
            C::DamageAgainstBosses(0.04)
        ),
        m!(
            "Sustaining Strikes",
            Tags(T::PHYSICAL.0),
            C::LifeLeech(0.006)
        ),
        m!(
            "Finishing Touch",
            Tags(T::PHYSICAL.0),
            C::DamageAgainstLowLife(0.04)
        ),
        s!("Unyielding Guard", DamageReduction, 0.02),
        m!(
            "Lasting Imprint",
            Tags(T::PHYSICAL.0),
            C::IncreasedDuration(0.04)
        ),
        s!("Light Footwork", Speed, 0.02),
        s!("Second Breath", Regen, 0.4),
        m!("Long Reach", Tags(T::MELEE.0), C::FlatRadius(0.15)),
    ],
    [
        // Paladin / Holy
        m!("Gentle Touch", Tags(T::HEAL.0), C::IncreasedEffect(0.04)),
        m!(
            "Swift Mending",
            Tags(T::HEAL.0),
            C::IncreasedCooldownRecovery(0.025)
        ),
        m!(
            "Lingering Grace",
            Tags(T::HEAL.0),
            C::IncreasedDuration(0.04)
        ),
        m!("Radiant Power", Tags(T::HOLY.0), C::IncreasedDamage(0.04)),
        m!("Guiding Light", Tags(T::HOLY.0), C::CriticalChance(0.01)),
        m!(
            "Brilliant Judgment",
            Tags(T::HOLY.0),
            C::CriticalMultiplier(0.05)
        ),
        m!(
            "Sacred Rhythm",
            Tags(T::HOLY.0),
            C::IncreasedCooldownRecovery(0.025)
        ),
        m!("Radiant Reach", Tags(T::HOLY.0), C::IncreasedArea(0.04)),
        k!("Merciful Shock", "holy_shock", C::IncreasedEffect(0.05)),
        k!(
            "Quickened Light",
            "holy_shock",
            C::IncreasedCooldownRecovery(0.03)
        ),
        k!("Purifying Light", "exorcism", C::IncreasedDamage(0.05)),
        k!("Light Beyond Measure", "holy_wrath", C::IncreasedArea(0.05)),
        s!("Enduring Spirit", MaxHp, 0.03),
        s!("Second Breath", Regen, 0.4),
        s!("Light Footwork", Speed, 0.02),
        m!("Sacred Imprint", Tags(T::HOLY.0), C::FlatSpellDamage(1.0)),
    ],
    [
        // Paladin / Protection
        s!("Sacred Warding", AllResistance, 2.0),
        s!("Shield Discipline", Armor, 0.1),
        s!("Stalwart Heart", MaxHp, 0.04),
        m!(
            "Sacred Rhythm",
            Tags(T::HOLY.0),
            C::IncreasedCooldownRecovery(0.025)
        ),
        m!("Radiant Reach", Tags(T::HOLY.0), C::IncreasedArea(0.04)),
        k!("Hallowed Ground", "consecration", C::IncreasedDamage(0.05)),
        k!(
            "Enduring Sanctuary",
            "consecration",
            C::IncreasedDuration(0.05)
        ),
        k!("Sacred Ground", "consecration", C::IncreasedArea(0.05)),
        k!(
            "Renewed Sanctuary",
            "consecration",
            C::IncreasedCooldownRecovery(0.03)
        ),
        s!("Second Breath", Regen, 0.4),
        s!("Light Footwork", Speed, 0.02),
        s!("Divine Bulwark", DamageReduction, 0.02),
        m!(
            "Brilliant Judgment",
            Tags(T::HOLY.0),
            C::CriticalMultiplier(0.05)
        ),
        m!(
            "Giant Slayer",
            Tags(T::HOLY.0),
            C::DamageAgainstBosses(0.04)
        ),
        m!("Gentle Touch", Tags(T::HEAL.0), C::IncreasedEffect(0.04)),
        m!(
            "Swift Mending",
            Tags(T::HEAL.0),
            C::IncreasedCooldownRecovery(0.025)
        ),
    ],
    [
        // Paladin / Retribution
        m!("Radiant Power", Tags(T::HOLY.0), C::IncreasedDamage(0.04)),
        m!("Guiding Light", Tags(T::HOLY.0), C::CriticalChance(0.01)),
        m!(
            "Brilliant Judgment",
            Tags(T::HOLY.0),
            C::CriticalMultiplier(0.05)
        ),
        m!(
            "Sacred Rhythm",
            Tags(T::HOLY.0),
            C::IncreasedCooldownRecovery(0.025)
        ),
        m!("Radiant Reach", Tags(T::HOLY.0), C::IncreasedArea(0.04)),
        m!("Sharpened Impact", Tags(T::HOLY.0), C::FlatDamage(1.0)),
        m!(
            "Giant Slayer",
            Tags(T::HOLY.0),
            C::DamageAgainstBosses(0.04)
        ),
        m!(
            "Finishing Touch",
            Tags(T::HOLY.0),
            C::DamageAgainstLowLife(0.04)
        ),
        m!(
            "Opening Gambit",
            Tags(T::HOLY.0),
            C::DamageAgainstFullLife(0.04)
        ),
        m!("Gentle Touch", Tags(T::HEAL.0), C::IncreasedEffect(0.04)),
        m!(
            "Swift Mending",
            Tags(T::HEAL.0),
            C::IncreasedCooldownRecovery(0.025)
        ),
        s!("Enduring Spirit", MaxHp, 0.03),
        s!("Armour Training", Armor, 0.06),
        s!("Second Breath", Regen, 0.4),
        s!("Light Footwork", Speed, 0.02),
        m!(
            "Lasting Imprint",
            Tags(T::HOLY.0),
            C::IncreasedDuration(0.04)
        ),
    ],
    [
        // Hunter / Beast Mastery
        m!("Focused Power", T::MINION, C::IncreasedDamage(0.04)),
        m!("Savage Companion", T::MINION, C::IncreasedDamage(0.05)),
        m!("Pack Rhythm", T::MINION, C::IncreasedAttackSpeed(0.03)),
        m!("Predatory Instinct", T::MINION, C::CriticalChance(0.01)),
        m!("Crushing Fangs", T::MINION, C::CriticalMultiplier(0.05)),
        m!("Rending Fangs", T::MINION, C::IncreasedDotDamage(0.05)),
        m!("Deep Bites", T::MINION, C::IncreasedDuration(0.04)),
        m!("Deepening Wounds", T::MINION, C::IncreasedDotDamage(0.05)),
        m!(
            "Persistent Wounds",
            Tags(T::MINION.0 | T::DOT.0),
            C::IncreasedDuration(0.04)
        ),
        m!(
            "Relentless Assault",
            T::MINION,
            C::IncreasedAttackSpeed(0.025)
        ),
        m!("Precision", T::MINION, C::CriticalChance(0.01)),
        m!("Giant Slayer", T::MINION, C::DamageAgainstBosses(0.04)),
        s!("Enduring Spirit", MaxHp, 0.03),
        s!("Second Breath", Regen, 0.4),
        m!("Light Footwork", T::MINION, C::IncreasedMovementSpeed(0.02)),
        m!("Exposed Armour", T::MINION, C::ArmourPenetration(0.025)),
    ],
    [
        // Hunter / Marksmanship
        m!(
            "Focused Power",
            Tags(T::PROJECTILE.0),
            C::IncreasedDamage(0.04)
        ),
        m!("Precision", Tags(T::PROJECTILE.0), C::CriticalChance(0.01)),
        m!(
            "Devastation",
            Tags(T::PROJECTILE.0),
            C::CriticalMultiplier(0.05)
        ),
        m!(
            "Practiced Rhythm",
            Tags(T::PROJECTILE.0),
            C::IncreasedCooldownRecovery(0.025)
        ),
        m!(
            "Sharpened Impact",
            Tags(T::PROJECTILE.0),
            C::FlatDamage(1.0)
        ),
        m!(
            "Expanding Reach",
            Tags(T::PROJECTILE.0),
            C::IncreasedArea(0.04)
        ),
        m!(
            "Giant Slayer",
            Tags(T::PROJECTILE.0),
            C::DamageAgainstBosses(0.04)
        ),
        m!(
            "Opening Gambit",
            Tags(T::PROJECTILE.0),
            C::DamageAgainstFullLife(0.04)
        ),
        m!(
            "Finishing Touch",
            Tags(T::PROJECTILE.0),
            C::DamageAgainstLowLife(0.04)
        ),
        m!(
            "Forceful Shots",
            Tags(T::PROJECTILE.0),
            C::KnockbackChance(0.03)
        ),
        m!(
            "Exposed Armour",
            Tags(T::PHYSICAL.0),
            C::ArmourPenetration(0.025)
        ),
        m!("Follow Through", Tags(T::ATTACK.0), C::RecastChance(0.025)),
        m!(
            "Martial Training",
            Tags(T::PHYSICAL.0),
            C::IncreasedDamage(0.04)
        ),
        s!("Enduring Spirit", MaxHp, 0.03),
        s!("Light Footwork", Speed, 0.02),
        s!("Armour Training", Armor, 0.06),
    ],
    [
        // Hunter / Survival
        s!("Survivor's Vitality", MaxHp, 0.04),
        s!("Thick Hide", Armor, 0.08),
        m!("Trap Mastery", Tags(T::TRAP.0), C::IncreasedDamage(0.06)),
        m!("Widened Traps", Tags(T::TRAP.0), C::IncreasedArea(0.06)),
        m!(
            "Patient Trapper",
            Tags(T::TRAP.0),
            C::IncreasedDuration(0.06)
        ),
        m!(
            "Prepared Traps",
            Tags(T::TRAP.0),
            C::IncreasedCooldownRecovery(0.04)
        ),
        s!("Elemental Endurance", AllResistance, 1.5),
        s!("Survival Instincts", DamageReduction, 0.01),
        m!(
            "Burning Traps",
            Tags(T::TRAP.0),
            C::IncreasedDotDamage(0.08)
        ),
        m!("Deadly Traps", Tags(T::TRAP.0), C::CriticalChance(0.02)),
        s!("Enduring Spirit", MaxHp, 0.03),
        s!("Second Breath", Regen, 0.6),
        s!("Armour Training", Armor, 0.06),
        s!("Light Footwork", Speed, 0.02),
        m!(
            "Monster Traps",
            Tags(T::TRAP.0),
            C::DamageAgainstBosses(0.05)
        ),
        m!("Lethal Traps", Tags(T::TRAP.0), C::MoreDamage(0.03)),
    ],
    [
        // Rogue / Assassination
        m!(
            "Venomous Growth",
            Tags(T::NATURE.0),
            C::IncreasedDotDamage(0.05)
        ),
        m!(
            "Enduring Venom",
            Tags(T::NATURE.0 | T::DOT.0),
            C::IncreasedDuration(0.04)
        ),
        m!("Focused Power", Tags(T::NATURE.0), C::IncreasedDamage(0.04)),
        m!(
            "Practiced Rhythm",
            Tags(T::NATURE.0),
            C::IncreasedCooldownRecovery(0.025)
        ),
        m!(
            "Martial Training",
            Tags(T::PHYSICAL.0),
            C::IncreasedDamage(0.04)
        ),
        m!("Precision", Tags(T::NATURE.0), C::CriticalChance(0.01)),
        m!(
            "Devastation",
            Tags(T::NATURE.0),
            C::CriticalMultiplier(0.05)
        ),
        m!(
            "Relentless Assault",
            Tags(T::NATURE.0),
            C::IncreasedAttackSpeed(0.025)
        ),
        m!(
            "Exposed Armour",
            Tags(T::PHYSICAL.0),
            C::ArmourPenetration(0.025)
        ),
        m!(
            "Sustaining Strikes",
            Tags(T::PHYSICAL.0),
            C::LifeLeech(0.006)
        ),
        m!(
            "Giant Slayer",
            Tags(T::NATURE.0),
            C::DamageAgainstBosses(0.04)
        ),
        m!(
            "Opening Gambit",
            Tags(T::NATURE.0),
            C::DamageAgainstFullLife(0.04)
        ),
        m!(
            "Finishing Touch",
            Tags(T::NATURE.0),
            C::DamageAgainstLowLife(0.04)
        ),
        s!("Enduring Spirit", MaxHp, 0.03),
        s!("Light Footwork", Speed, 0.02),
        m!("Evasion", Tags(0), C::DodgeChance(0.03)),
    ],
    [
        // Rogue / Combat
        m!(
            "Relentless Assault",
            Tags(T::ATTACK.0),
            C::IncreasedAttackSpeed(0.025)
        ),
        m!("Focused Power", Tags(T::ATTACK.0), C::IncreasedDamage(0.04)),
        m!("Expanding Reach", Tags(T::ATTACK.0), C::IncreasedArea(0.04)),
        m!("Wide Swings", Tags(T::MELEE.0), C::IncreasedArea(0.04)),
        m!("Follow Through", Tags(T::ATTACK.0), C::RecastChance(0.025)),
        m!(
            "Practiced Rhythm",
            Tags(T::ATTACK.0),
            C::IncreasedCooldownRecovery(0.025)
        ),
        m!("Precision", Tags(T::ATTACK.0), C::CriticalChance(0.01)),
        m!(
            "Devastation",
            Tags(T::ATTACK.0),
            C::CriticalMultiplier(0.05)
        ),
        m!("Sharpened Impact", Tags(T::ATTACK.0), C::FlatDamage(1.0)),
        m!(
            "Exposed Armour",
            Tags(T::PHYSICAL.0),
            C::ArmourPenetration(0.025)
        ),
        m!(
            "Sustaining Strikes",
            Tags(T::PHYSICAL.0),
            C::LifeLeech(0.006)
        ),
        m!(
            "Giant Slayer",
            Tags(T::ATTACK.0),
            C::DamageAgainstBosses(0.04)
        ),
        m!(
            "Finishing Touch",
            Tags(T::ATTACK.0),
            C::DamageAgainstLowLife(0.04)
        ),
        s!("Enduring Spirit", MaxHp, 0.03),
        m!("Evasion", Tags(0), C::DodgeChance(0.03)),
        s!("Light Footwork", Speed, 0.02),
    ],
    [
        // Rogue / Subtlety
        m!("Precision", Tags(T::ATTACK.0), C::CriticalChance(0.01)),
        m!(
            "Devastation",
            Tags(T::ATTACK.0),
            C::CriticalMultiplier(0.05)
        ),
        m!(
            "Opening Gambit",
            Tags(T::ATTACK.0),
            C::DamageAgainstFullLife(0.04)
        ),
        m!(
            "Finishing Touch",
            Tags(T::ATTACK.0),
            C::DamageAgainstLowLife(0.04)
        ),
        m!(
            "Giant Slayer",
            Tags(T::ATTACK.0),
            C::DamageAgainstBosses(0.04)
        ),
        m!(
            "Exposed Armour",
            Tags(T::PHYSICAL.0),
            C::ArmourPenetration(0.025)
        ),
        m!("Focused Power", Tags(T::ATTACK.0), C::IncreasedDamage(0.04)),
        m!(
            "Practiced Rhythm",
            Tags(T::ATTACK.0),
            C::IncreasedCooldownRecovery(0.025)
        ),
        m!("Sharpened Impact", Tags(T::ATTACK.0), C::FlatDamage(1.0)),
        m!(
            "Sustaining Strikes",
            Tags(T::PHYSICAL.0),
            C::LifeLeech(0.006)
        ),
        m!(
            "Relentless Assault",
            Tags(T::ATTACK.0),
            C::IncreasedAttackSpeed(0.025)
        ),
        s!("Enduring Spirit", MaxHp, 0.03),
        m!("Evasion", Tags(0), C::DodgeChance(0.03)),
        s!("Light Footwork", Speed, 0.02),
        s!("Second Breath", Regen, 0.4),
        m!(
            "Martial Training",
            Tags(T::PHYSICAL.0),
            C::IncreasedDamage(0.04)
        ),
    ],
    [
        // Priest / Discipline
        m!("Woven Wards", Tags(T::GUARD.0), C::IncreasedEffect(0.04)),
        m!(
            "Renewed Wards",
            Tags(T::GUARD.0),
            C::IncreasedCooldownRecovery(0.025)
        ),
        m!(
            "Lasting Protection",
            Tags(T::GUARD.0),
            C::IncreasedDuration(0.04)
        ),
        m!("Radiant Power", Tags(T::HOLY.0), C::IncreasedDamage(0.04)),
        m!("Radiant Reach", Tags(T::HOLY.0), C::IncreasedArea(0.04)),
        m!(
            "Sacred Rhythm",
            Tags(T::HOLY.0),
            C::IncreasedCooldownRecovery(0.025)
        ),
        m!("Guiding Light", Tags(T::HOLY.0), C::CriticalChance(0.01)),
        m!(
            "Brilliant Judgment",
            Tags(T::HOLY.0),
            C::CriticalMultiplier(0.05)
        ),
        m!("Sacred Imprint", Tags(T::HOLY.0), C::FlatSpellDamage(1.0)),
        m!("Gentle Touch", Tags(T::HEAL.0), C::IncreasedEffect(0.04)),
        m!(
            "Swift Mending",
            Tags(T::HEAL.0),
            C::IncreasedCooldownRecovery(0.025)
        ),
        s!("Enduring Spirit", MaxHp, 0.03),
        s!("Armour Training", Armor, 0.06),
        s!("Second Breath", Regen, 0.4),
        s!("Light Footwork", Speed, 0.02),
        m!(
            "Giant Slayer",
            Tags(T::HOLY.0),
            C::DamageAgainstBosses(0.04)
        ),
    ],
    [
        // Priest / Holy
        m!("Gentle Touch", Tags(T::HEAL.0), C::IncreasedEffect(0.04)),
        m!(
            "Swift Mending",
            Tags(T::HEAL.0),
            C::IncreasedCooldownRecovery(0.025)
        ),
        m!(
            "Lingering Grace",
            Tags(T::HEAL.0),
            C::IncreasedDuration(0.04)
        ),
        m!(
            "Empowered Channel",
            Tags(T::HOLY.0),
            C::IncreasedEffect(0.04)
        ),
        m!("Radiant Power", Tags(T::HOLY.0), C::IncreasedDamage(0.04)),
        m!("Guiding Light", Tags(T::HOLY.0), C::CriticalChance(0.01)),
        m!(
            "Brilliant Judgment",
            Tags(T::HOLY.0),
            C::CriticalMultiplier(0.05)
        ),
        m!("Radiant Reach", Tags(T::HOLY.0), C::IncreasedArea(0.04)),
        m!(
            "Sacred Rhythm",
            Tags(T::HOLY.0),
            C::IncreasedCooldownRecovery(0.025)
        ),
        m!("Sacred Imprint", Tags(T::HOLY.0), C::FlatSpellDamage(1.0)),
        m!(
            "Giant Slayer",
            Tags(T::HOLY.0),
            C::DamageAgainstBosses(0.04)
        ),
        m!(
            "Opening Gambit",
            Tags(T::HOLY.0),
            C::DamageAgainstFullLife(0.04)
        ),
        s!("Enduring Spirit", MaxHp, 0.03),
        s!("Second Breath", Regen, 0.4),
        s!("Light Footwork", Speed, 0.02),
        s!("Armour Training", Armor, 0.06),
    ],
    [
        // Priest / Shadow
        m!(
            "Lingering Pain",
            Tags(T::SHADOW.0),
            C::IncreasedDotDamage(0.05)
        ),
        m!(
            "Lasting Imprint",
            Tags(T::SHADOW.0),
            C::IncreasedDuration(0.04)
        ),
        m!("Focused Power", Tags(T::SHADOW.0), C::IncreasedDamage(0.04)),
        m!(
            "Practiced Rhythm",
            Tags(T::SHADOW.0),
            C::IncreasedCooldownRecovery(0.025)
        ),
        k!("Soul Siphon", "drain_life", C::IncreasedEffect(0.05)),
        m!("Precision", Tags(T::SHADOW.0), C::CriticalChance(0.01)),
        m!(
            "Devastation",
            Tags(T::SHADOW.0),
            C::CriticalMultiplier(0.05)
        ),
        m!("Runic Imprint", Tags(T::SHADOW.0), C::FlatSpellDamage(1.0)),
        m!("Expanding Reach", Tags(T::SHADOW.0), C::IncreasedArea(0.04)),
        m!(
            "Giant Slayer",
            Tags(T::SHADOW.0),
            C::DamageAgainstBosses(0.04)
        ),
        m!(
            "Finishing Touch",
            Tags(T::SHADOW.0),
            C::DamageAgainstLowLife(0.04)
        ),
        m!(
            "Opening Gambit",
            Tags(T::SHADOW.0),
            C::DamageAgainstFullLife(0.04)
        ),
        s!("Enduring Spirit", MaxHp, 0.03),
        s!("Second Breath", Regen, 0.4),
        s!("Light Footwork", Speed, 0.02),
        s!("Armour Training", Armor, 0.06),
    ],
    [
        // Shaman / Elemental
        m!(
            "Gathering Storm",
            Tags(T::LIGHTNING.0),
            C::IncreasedDamage(0.04)
        ),
        m!("Wild Power", Tags(T::NATURE.0), C::IncreasedDamage(0.04)),
        m!("Kindled Power", Tags(T::FIRE.0), C::IncreasedDamage(0.04)),
        m!("Spell Resonance", Tags(T::SPELL.0), C::RecastChance(0.025)),
        m!("Precision", Tags(T::SPELL.0), C::CriticalChance(0.01)),
        m!("Devastation", Tags(T::SPELL.0), C::CriticalMultiplier(0.05)),
        m!(
            "Practiced Rhythm",
            Tags(T::SPELL.0),
            C::IncreasedCooldownRecovery(0.025)
        ),
        m!("Expanding Reach", Tags(T::SPELL.0), C::IncreasedArea(0.04)),
        m!(
            "Lasting Imprint",
            Tags(T::SPELL.0),
            C::IncreasedDuration(0.04)
        ),
        m!("Runic Imprint", Tags(T::SPELL.0), C::FlatSpellDamage(1.0)),
        m!(
            "Giant Slayer",
            Tags(T::SPELL.0),
            C::DamageAgainstBosses(0.04)
        ),
        m!(
            "Opening Gambit",
            Tags(T::SPELL.0),
            C::DamageAgainstFullLife(0.04)
        ),
        m!(
            "Finishing Touch",
            Tags(T::SPELL.0),
            C::DamageAgainstLowLife(0.04)
        ),
        s!("Enduring Spirit", MaxHp, 0.03),
        s!("Second Breath", Regen, 0.4),
        s!("Light Footwork", Speed, 0.02),
    ],
    [
        // Shaman / Enhancement
        m!(
            "Relentless Assault",
            Tags(T::ATTACK.0),
            C::IncreasedAttackSpeed(0.025)
        ),
        m!("Focused Power", Tags(T::ATTACK.0), C::IncreasedDamage(0.04)),
        m!(
            "Gathering Storm",
            Tags(T::LIGHTNING.0),
            C::IncreasedDamage(0.04)
        ),
        m!("Kindled Power", Tags(T::FIRE.0), C::IncreasedDamage(0.04)),
        m!("Wild Power", Tags(T::NATURE.0), C::IncreasedDamage(0.04)),
        k!(
            "Stormforged Weapons",
            "stormstrike",
            C::IncreasedDamage(0.05)
        ),
        m!("Precision", Tags(T::ATTACK.0), C::CriticalChance(0.01)),
        m!(
            "Devastation",
            Tags(T::ATTACK.0),
            C::CriticalMultiplier(0.05)
        ),
        m!(
            "Practiced Rhythm",
            Tags(T::ATTACK.0),
            C::IncreasedCooldownRecovery(0.025)
        ),
        m!("Expanding Reach", Tags(T::ATTACK.0), C::IncreasedArea(0.04)),
        m!(
            "Giant Slayer",
            Tags(T::ATTACK.0),
            C::DamageAgainstBosses(0.04)
        ),
        m!(
            "Opening Gambit",
            Tags(T::ATTACK.0),
            C::DamageAgainstFullLife(0.04)
        ),
        m!(
            "Finishing Touch",
            Tags(T::ATTACK.0),
            C::DamageAgainstLowLife(0.04)
        ),
        s!("Enduring Spirit", MaxHp, 0.03),
        s!("Armour Training", Armor, 0.06),
        s!("Light Footwork", Speed, 0.02),
    ],
    [
        // Shaman / Restoration
        m!("Totemic Power", Tags(T::TOTEM.0), C::IncreasedEffect(0.04)),
        m!(
            "Totemic Rhythm",
            Tags(T::TOTEM.0),
            C::IncreasedCooldownRecovery(0.025)
        ),
        m!(
            "Ancestral Persistence",
            Tags(T::TOTEM.0),
            C::IncreasedDuration(0.04)
        ),
        k!(
            "Healing Streams",
            "proc_totem_heal",
            C::IncreasedEffect(0.05)
        ),
        m!(
            "Empowered Offerings",
            Tags(T::TOTEM.0),
            C::IncreasedBuffEffect(0.04)
        ),
        k!(
            "Swift Recall",
            "totemic_recall",
            C::IncreasedCooldownRecovery(0.06)
        ),
        m!(
            "Totemic Precision",
            Tags(T::TOTEM.0),
            C::CriticalChance(0.01)
        ),
        m!(
            "Totemic Fury",
            Tags(T::TOTEM.0),
            C::CriticalMultiplier(0.05)
        ),
        m!(
            "Totemic Cadence",
            Tags(T::TOTEM.0),
            C::IncreasedCooldownRecovery(0.03)
        ),
        k!("Searing Focus", "searing_totem", C::IncreasedDamage(0.06)),
        k!("Magma Reach", "magma_totem", C::IncreasedArea(0.06)),
        m!("Expanding Reach", Tags(T::TOTEM.0), C::IncreasedArea(0.04)),
        s!("Enduring Spirit", MaxHp, 0.03),
        s!("Second Breath", Regen, 0.4),
        s!("Light Footwork", Speed, 0.02),
        s!("Armour Training", Armor, 0.06),
    ],
    [
        // Mage / Arcane
        m!("Focused Power", Tags(T::SPELL.0), C::IncreasedDamage(0.04)),
        m!("Spell Resonance", Tags(T::SPELL.0), C::RecastChance(0.025)),
        m!(
            "Arcane Reverberation",
            Tags(T::ARCANE.0 | T::SPELL.0),
            C::RecastChance(0.02)
        ),
        m!(
            "Practiced Rhythm",
            Tags(T::SPELL.0),
            C::IncreasedCooldownRecovery(0.025)
        ),
        m!("Precision", Tags(T::SPELL.0), C::CriticalChance(0.01)),
        m!("Devastation", Tags(T::SPELL.0), C::CriticalMultiplier(0.05)),
        m!("Runic Imprint", Tags(T::SPELL.0), C::FlatSpellDamage(1.0)),
        m!("Runic Power", Tags(T::ARCANE.0), C::IncreasedDamage(0.04)),
        m!(
            "Runic Cadence",
            Tags(T::ARCANE.0),
            C::IncreasedCooldownRecovery(0.025)
        ),
        m!("Expanding Runes", Tags(T::ARCANE.0), C::IncreasedArea(0.04)),
        m!(
            "Astral Precision",
            Tags(T::ARCANE.0),
            C::CriticalChance(0.01)
        ),
        m!(
            "Opening Gambit",
            Tags(T::SPELL.0),
            C::DamageAgainstFullLife(0.04)
        ),
        m!(
            "Giant Slayer",
            Tags(T::SPELL.0),
            C::DamageAgainstBosses(0.04)
        ),
        s!("Enduring Spirit", MaxHp, 0.03),
        s!("Second Breath", Regen, 0.4),
        s!("Light Footwork", Speed, 0.02),
    ],
    [
        // Mage / Fire
        m!("Focused Power", Tags(T::FIRE.0), C::IncreasedDamage(0.04)),
        m!(
            "Searing Precision",
            Tags(T::FIRE.0),
            C::CriticalChance(0.01)
        ),
        m!(
            "Lingering Pain",
            Tags(T::FIRE.0),
            C::IncreasedDotDamage(0.05)
        ),
        k!(
            "Smouldering Embers",
            "proc_ignite",
            C::IncreasedDotDamage(0.05)
        ),
        m!(
            "Lasting Imprint",
            Tags(T::FIRE.0),
            C::IncreasedDuration(0.04)
        ),
        m!("Expanding Reach", Tags(T::FIRE.0), C::IncreasedArea(0.04)),
        m!(
            "Practiced Rhythm",
            Tags(T::FIRE.0),
            C::IncreasedCooldownRecovery(0.025)
        ),
        m!("Devastation", Tags(T::FIRE.0), C::CriticalMultiplier(0.05)),
        m!("Runic Imprint", Tags(T::FIRE.0), C::FlatSpellDamage(1.0)),
        m!(
            "Opening Gambit",
            Tags(T::FIRE.0),
            C::DamageAgainstFullLife(0.04)
        ),
        m!(
            "Finishing Touch",
            Tags(T::FIRE.0),
            C::DamageAgainstLowLife(0.04)
        ),
        m!(
            "Giant Slayer",
            Tags(T::FIRE.0),
            C::DamageAgainstBosses(0.04)
        ),
        k!("Pyromaniac", "pyroblast", C::IncreasedDamage(0.05)),
        s!("Enduring Spirit", MaxHp, 0.03),
        s!("Second Breath", Regen, 0.4),
        s!("Light Footwork", Speed, 0.02),
    ],
    [
        // Mage / Frost
        m!("Focused Power", Tags(T::COLD.0), C::IncreasedDamage(0.04)),
        m!("Biting Cold", Tags(T::COLD.0), C::ChillChance(0.04)),
        m!("Precision", Tags(T::COLD.0), C::CriticalChance(0.01)),
        m!("Devastation", Tags(T::COLD.0), C::CriticalMultiplier(0.05)),
        m!("Expanding Reach", Tags(T::COLD.0), C::IncreasedArea(0.04)),
        m!(
            "Lasting Imprint",
            Tags(T::COLD.0),
            C::IncreasedDuration(0.04)
        ),
        m!(
            "Practiced Rhythm",
            Tags(T::COLD.0),
            C::IncreasedCooldownRecovery(0.025)
        ),
        k!("Piercing Ice", "frostbolt", C::FlatSpellDamage(1.0)),
        k!("Glacial Reach", "frost_nova", C::IncreasedArea(0.05)),
        k!("Long Winter", "blizzard", C::IncreasedDuration(0.05)),
        k!("Glacial Ward", "ice_barrier", C::IncreasedEffect(0.05)),
        m!(
            "Giant Slayer",
            Tags(T::COLD.0),
            C::DamageAgainstBosses(0.04)
        ),
        s!("Enduring Spirit", MaxHp, 0.03),
        s!("Armour Training", Armor, 0.06),
        s!("Second Breath", Regen, 0.4),
        s!("Light Footwork", Speed, 0.02),
    ],
    [
        // Warlock / Affliction
        m!(
            "Lingering Pain",
            Tags(T::SHADOW.0),
            C::IncreasedDotDamage(0.05)
        ),
        m!(
            "Lasting Imprint",
            Tags(T::SHADOW.0),
            C::IncreasedDuration(0.04)
        ),
        m!(
            "Practiced Rhythm",
            Tags(T::SHADOW.0),
            C::IncreasedCooldownRecovery(0.025)
        ),
        k!(
            "Spreading Corruption",
            "corruption",
            C::IncreasedDotDamage(0.05)
        ),
        k!(
            "Lingering Agony",
            "curse_of_agony",
            C::IncreasedDuration(0.05)
        ),
        k!("Soul Siphon", "drain_life", C::IncreasedEffect(0.05)),
        m!("Focused Power", Tags(T::SHADOW.0), C::IncreasedDamage(0.04)),
        m!("Expanding Reach", Tags(T::SHADOW.0), C::IncreasedArea(0.04)),
        m!(
            "Giant Slayer",
            Tags(T::SHADOW.0),
            C::DamageAgainstBosses(0.04)
        ),
        m!(
            "Finishing Touch",
            Tags(T::SHADOW.0),
            C::DamageAgainstLowLife(0.04)
        ),
        m!(
            "Opening Gambit",
            Tags(T::SHADOW.0),
            C::DamageAgainstFullLife(0.04)
        ),
        m!("Runic Imprint", Tags(T::SHADOW.0), C::FlatSpellDamage(1.0)),
        s!("Enduring Spirit", MaxHp, 0.03),
        s!("Second Breath", Regen, 0.4),
        s!("Light Footwork", Speed, 0.02),
        s!("Armour Training", Armor, 0.06),
    ],
    [
        // Warlock / Demonology
        k!("Demonic Might", "proc_demon_bolt", C::IncreasedDamage(0.05)),
        k!(
            "Demonic Haste",
            "proc_demon_bolt",
            C::IncreasedCooldownRecovery(0.03)
        ),
        k!(
            "Malicious Precision",
            "proc_demon_bolt",
            C::CriticalChance(0.01)
        ),
        k!(
            "Infernal Ruin",
            "proc_demon_bolt",
            C::CriticalMultiplier(0.05)
        ),
        k!(
            "Burning Pact",
            "proc_demon_bolt",
            C::IncreasedDotDamage(0.05)
        ),
        k!("Infernal Reach", "proc_demon_bolt", C::IncreasedArea(0.04)),
        m!("Kindled Power", Tags(T::FIRE.0), C::IncreasedDamage(0.04)),
        m!("Precision", Tags(T::SPELL.0), C::CriticalChance(0.01)),
        m!(
            "Practiced Rhythm",
            Tags(T::SPELL.0),
            C::IncreasedCooldownRecovery(0.025)
        ),
        m!(
            "Giant Slayer",
            Tags(T::SPELL.0),
            C::DamageAgainstBosses(0.04)
        ),
        m!("Runic Imprint", Tags(T::SPELL.0), C::FlatSpellDamage(1.0)),
        m!("Expanding Reach", Tags(T::SPELL.0), C::IncreasedArea(0.04)),
        s!("Enduring Spirit", MaxHp, 0.03),
        s!("Armour Training", Armor, 0.06),
        s!("Second Breath", Regen, 0.4),
        s!("Light Footwork", Speed, 0.02),
    ],
    [
        // Warlock / Destruction
        m!("Overwhelming Force", Tags(T::SPELL.0), C::MoreDamage(0.02)),
        m!("Precision", Tags(T::SPELL.0), C::CriticalChance(0.01)),
        m!("Devastation", Tags(T::SPELL.0), C::CriticalMultiplier(0.05)),
        m!("Runic Imprint", Tags(T::SPELL.0), C::FlatSpellDamage(1.0)),
        m!(
            "Practiced Rhythm",
            Tags(T::SPELL.0),
            C::IncreasedCooldownRecovery(0.025)
        ),
        m!("Kindled Power", Tags(T::FIRE.0), C::IncreasedDamage(0.04)),
        m!("Dark Arts", Tags(T::SHADOW.0), C::IncreasedDamage(0.04)),
        m!(
            "Giant Slayer",
            Tags(T::SPELL.0),
            C::DamageAgainstBosses(0.04)
        ),
        m!(
            "Opening Gambit",
            Tags(T::SPELL.0),
            C::DamageAgainstFullLife(0.04)
        ),
        m!(
            "Finishing Touch",
            Tags(T::SPELL.0),
            C::DamageAgainstLowLife(0.04)
        ),
        m!("Expanding Reach", Tags(T::SPELL.0), C::IncreasedArea(0.04)),
        m!(
            "Lasting Imprint",
            Tags(T::SPELL.0),
            C::IncreasedDuration(0.04)
        ),
        s!("Enduring Spirit", MaxHp, 0.03),
        s!("Second Breath", Regen, 0.4),
        s!("Light Footwork", Speed, 0.02),
        s!("Armour Training", Armor, 0.06),
    ],
    [
        // Druid / Balance
        m!("Focused Power", Tags(T::ARCANE.0), C::IncreasedDamage(0.04)),
        m!(
            "Lingering Pain",
            Tags(T::ARCANE.0),
            C::IncreasedDotDamage(0.05)
        ),
        m!(
            "Lasting Imprint",
            Tags(T::ARCANE.0),
            C::IncreasedDuration(0.04)
        ),
        m!(
            "Practiced Rhythm",
            Tags(T::ARCANE.0),
            C::IncreasedCooldownRecovery(0.025)
        ),
        m!("Precision", Tags(T::ARCANE.0), C::CriticalChance(0.01)),
        m!(
            "Devastation",
            Tags(T::ARCANE.0),
            C::CriticalMultiplier(0.05)
        ),
        m!("Runic Imprint", Tags(T::ARCANE.0), C::FlatSpellDamage(1.0)),
        m!("Expanding Reach", Tags(T::ARCANE.0), C::IncreasedArea(0.04)),
        m!(
            "Giant Slayer",
            Tags(T::ARCANE.0),
            C::DamageAgainstBosses(0.04)
        ),
        k!("Celestial Might", "starfire", C::IncreasedDamage(0.05)),
        k!("Lunar Persistence", "moonfire", C::IncreasedDuration(0.05)),
        k!("Stellar Reach", "proc_starfall", C::IncreasedArea(0.05)),
        m!("Wild Power", Tags(T::NATURE.0), C::IncreasedDamage(0.04)),
        s!("Enduring Spirit", MaxHp, 0.03),
        s!("Second Breath", Regen, 0.4),
        s!("Light Footwork", Speed, 0.02),
    ],
    [
        // Druid / Feral
        m!(
            "Focused Power",
            Tags(T::PHYSICAL.0 | T::MELEE.0),
            C::IncreasedDamage(0.04)
        ),
        m!(
            "Deepening Wounds",
            Tags(T::PHYSICAL.0),
            C::IncreasedDotDamage(0.05)
        ),
        m!(
            "Persistent Wounds",
            Tags(T::PHYSICAL.0 | T::DOT.0),
            C::IncreasedDuration(0.04)
        ),
        m!(
            "Relentless Assault",
            Tags(T::PHYSICAL.0 | T::MELEE.0),
            C::IncreasedAttackSpeed(0.025)
        ),
        m!(
            "Precision",
            Tags(T::PHYSICAL.0 | T::MELEE.0),
            C::CriticalChance(0.01)
        ),
        m!(
            "Devastation",
            Tags(T::PHYSICAL.0 | T::MELEE.0),
            C::CriticalMultiplier(0.05)
        ),
        m!(
            "Sustaining Strikes",
            Tags(T::PHYSICAL.0),
            C::LifeLeech(0.006)
        ),
        m!(
            "Expanding Reach",
            Tags(T::PHYSICAL.0 | T::MELEE.0),
            C::IncreasedArea(0.04)
        ),
        m!(
            "Giant Slayer",
            Tags(T::PHYSICAL.0 | T::MELEE.0),
            C::DamageAgainstBosses(0.04)
        ),
        m!(
            "Finishing Touch",
            Tags(T::PHYSICAL.0 | T::MELEE.0),
            C::DamageAgainstLowLife(0.04)
        ),
        m!(
            "Exposed Armour",
            Tags(T::PHYSICAL.0),
            C::ArmourPenetration(0.025)
        ),
        m!(
            "Sharpened Impact",
            Tags(T::PHYSICAL.0 | T::MELEE.0),
            C::FlatDamage(1.0)
        ),
        m!(
            "Practiced Rhythm",
            Tags(T::PHYSICAL.0 | T::MELEE.0),
            C::IncreasedCooldownRecovery(0.025)
        ),
        m!("Feral Evasion", Tags(0), C::DodgeChance(0.03)),
        s!("Enduring Spirit", MaxHp, 0.03),
        s!("Light Footwork", Speed, 0.02),
    ],
    [
        // Druid / Restoration
        m!("Gentle Touch", Tags(T::HEAL.0), C::IncreasedEffect(0.04)),
        m!(
            "Swift Mending",
            Tags(T::HEAL.0),
            C::IncreasedCooldownRecovery(0.025)
        ),
        m!(
            "Lingering Grace",
            Tags(T::HEAL.0),
            C::IncreasedDuration(0.04)
        ),
        k!("Blooming Life", "proc_bloom", C::IncreasedEffect(0.05)),
        k!("Blossoming Reach", "proc_bloom", C::IncreasedArea(0.05)),
        k!("Living Grove", "proc_grove", C::IncreasedEffect(0.05)),
        k!(
            "Grove Renewal",
            "proc_grove",
            C::IncreasedCooldownRecovery(0.03)
        ),
        k!("Verdant Renewal", "rejuvenation", C::IncreasedEffect(0.05)),
        m!(
            "Venomous Growth",
            Tags(T::NATURE.0),
            C::IncreasedDotDamage(0.05)
        ),
        m!("Wild Power", Tags(T::NATURE.0), C::IncreasedDamage(0.04)),
        m!(
            "Practiced Rhythm",
            Tags(T::NATURE.0),
            C::IncreasedCooldownRecovery(0.025)
        ),
        m!("Expanding Reach", Tags(T::NATURE.0), C::IncreasedArea(0.04)),
        s!("Enduring Spirit", MaxHp, 0.03),
        s!("Second Breath", Regen, 0.4),
        s!("Light Footwork", Speed, 0.02),
        s!("Armour Training", Armor, 0.06),
    ],
];

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::super::{data, procs, talent_effects, talents, Run};
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn rogue_and_feral_trees_have_three_percent_dodge_per_passive_rank() {
        for (class, tree) in [(3, 0), (3, 1), (3, 2), (8, 1)] {
            let spec = &talents::TREES[class][tree];
            let nodes: Vec<_> = (0..talents::NODES)
                .filter(|&node| talent_design::milestone(node).is_none())
                .filter(|&node| matches!(minor(spec, node).change, Some(C::DodgeChance(_))))
                .collect();
            assert_eq!(nodes.len(), 1);
            for rank in 1..=5 {
                let run = apply_tree(class, tree, &[(nodes[0], rank)]);
                assert!(
                    (super::super::extra_spells::dodge_chance(&run) - 0.03 * f32::from(rank)).abs()
                        < 0.0001
                );
                let tooltip = talent_effects::description(spec, nodes[0], rank);
                assert!(tooltip.contains("dodge") && tooltip.contains("75%"));
            }
        }
    }

    #[test]
    fn beast_mastery_modifiers_use_minion_tags_and_buff_commands_demons_and_pets() {
        let spec = &talents::TREES[2][0];
        let nodes: Vec<_> = (0..talents::NODES)
            .filter(|&node| talent_design::milestone(node).is_none())
            .map(|node| (node, 1))
            .collect();
        for &(node, _) in &nodes {
            let talent = minor(spec, node);
            if talent.change.is_some() {
                assert!(talent.filter.all.contains(T::MINION), "{}", talent.name);
                assert!(talent.filter.skill.is_none(), "{}", talent.name);
                assert!(talent_effects::description(spec, node, 1).contains("Minion"));
            }
        }
        let trained = apply_tree(2, 0, &nodes);
        let base = Run {
            class: trained.class,
            ..Default::default()
        };
        for key in ["proc_pet_bite", "proc_demon_bolt"] {
            let a = procs::resolve(&base, key, 18.0, 4.0, 4.0, 0.0, 1);
            let b = procs::resolve(&trained, key, 18.0, 4.0, 4.0, 0.0, 1);
            assert!(b.dmg > a.dmg);
            assert!(b.dot > a.dot);
            assert!(b.duration > a.duration);
            assert!(b.cooldown < a.cooldown);
            assert!(b.crit > a.crit);
            assert!(b.crit_multiplier > a.crit_multiplier);
            assert!(b.movement_speed > a.movement_speed);
        }
        for key in ["stomp", "kill_command"] {
            let a = base.resolve_spell(data::ability(key), 1);
            let b = trained.resolve_spell(data::ability(key), 1);
            assert!(b.dmg > a.dmg);
            assert!(b.cooldown < a.cooldown);
            assert!(b.crit > a.crit);
        }
        assert!(
            trained
                .resolve_spell(data::ability("bestial_wrath"), 1)
                .duration
                > base
                    .resolve_spell(data::ability("bestial_wrath"), 1)
                    .duration
        );
        for key in ["arcane_shot", "multi_shot", "raptor_strike"] {
            assert_eq!(
                trained.resolve_spell(data::ability(key), 1),
                base.resolve_spell(data::ability(key), 1),
                "{key}"
            );
        }
        assert_eq!(trained.stats().speed, base.stats().speed);
    }

    #[test]
    fn survival_minors_add_defenses_and_only_improve_trap_offense() {
        let nodes: Vec<_> = (0..talents::NODES)
            .filter(|&node| talent_design::milestone(node).is_none())
            .map(|node| (node, 1))
            .collect();
        let mut trained = apply_tree(2, 2, &nodes);
        let mut baseline = Run {
            class: trained.class,
            ..Default::default()
        };
        trained.equipment_armour = 250.0;
        baseline.equipment_armour = 250.0;
        let before = baseline.stats();
        let after = trained.stats();
        assert!(after.max_hp > before.max_hp);
        assert!(after.armor > before.armor);
        assert!(after.regen > before.regen);
        assert!(after.resistances.fire > before.resistances.fire);
        assert!(after.damage_reduction > before.damage_reduction);
        for tags in [T::PHYSICAL, T::FIRE, T::SHADOW | T::DOT] {
            assert!(trained.damage_reduction_tagged(tags) > baseline.damage_reduction_tagged(tags));
        }
        for key in ["frost_trap", "explosive_trap"] {
            let def = data::ability(key);
            let a = baseline.resolve_spell(def, 1);
            let b = trained.resolve_spell(def, 1);
            assert!(b.tags.contains(T::TRAP));
            assert!(b.tags.labels().contains("Trap"));
            assert!(b.radius > a.radius);
            assert!(b.cooldown < a.cooldown);
            if key == "frost_trap" {
                assert!(b.duration > a.duration);
            } else {
                assert!(b.dmg > a.dmg);
                assert!(b.dot > a.dot);
                assert!(b.crit > a.crit);
            }
        }
        for key in [
            "arcane_shot",
            "raptor_strike",
            "serpent_sting",
            "kill_command",
        ] {
            assert_eq!(
                trained.resolve_spell(data::ability(key), 1),
                baseline.resolve_spell(data::ability(key), 1),
                "{key}"
            );
        }
        talents::Account::fresh().apply(2, &mut trained);
        assert_eq!(trained.stats().armor, before.armor);
        assert_eq!(trained.stats().max_hp, before.max_hp);
        assert_eq!(trained.stats().damage_reduction, before.damage_reduction);
        assert_eq!(
            trained.resolve_spell(data::ability("explosive_trap"), 1),
            baseline.resolve_spell(data::ability("explosive_trap"), 1)
        );
        assert!(data::PASSIVES.iter().all(|p| !matches!(p.effect, data::PassiveEffect::Modifier(filter, _) if filter.all.contains(T::TRAP))));
    }

    #[test]
    fn every_specialization_has_sixteen_named_scaled_and_distinct_minors() {
        let mut identities = BTreeSet::new();
        for trees in &talents::TREES {
            for spec in trees {
                let mut names = BTreeSet::new();
                let mut signature = String::new();
                let mut count = 0;
                for node in 0..talents::NODES {
                    if talent_design::milestone(node).is_some() {
                        continue;
                    }
                    let minor = minor(spec, node);
                    assert!(names.insert(minor.name));
                    assert_ne!(minor.stat.is_some(), minor.change.is_some());
                    assert!(!talent_effects::description(spec, node, 0).is_empty());
                    assert_eq!(
                        talent_effects::description(spec, node, 0),
                        talent_effects::description(spec, node, 1)
                    );
                    assert!(talent_effects::effects(spec, node, 0).is_empty());
                    if let Some(change) = minor.change {
                        assert_eq!(
                            talent_effects::effects(spec, node, 5),
                            vec![change.scaled(5)]
                        );
                    }
                    if let Some(key) = minor.filter.skill {
                        assert!(
                            data::ABILITIES.iter().any(|d| d.key == key)
                                || procs::find(key).is_some(),
                            "{key}"
                        );
                    }
                    signature.push_str(&format!(
                        "{:?}:{:?}:{:?};",
                        minor.filter, minor.stat, minor.change
                    ));
                    count += 1;
                }
                assert_eq!(count, 16);
                assert!(
                    identities.insert(signature),
                    "Duplicated specialization: {}",
                    spec.name
                );
            }
        }
        assert_eq!(identities.len(), 27);
    }

    fn apply_tree(class: usize, tree: usize, nodes: &[(usize, u8)]) -> Run {
        let mut build = [[0; talents::NODES]; 3];
        for &(node, rank) in nodes {
            build[tree][node] = rank;
        }
        let mut run = Run {
            class: &data::CLASSES[class],
            ..Default::default()
        };
        talents::Account::from_shared(class, 0, build).apply(class, &mut run);
        run
    }
    #[test]
    fn protection_minors_reduce_real_hits_and_dots_and_respec_removes_their_defenses() {
        use crate::survivors::{hero, spell_engine::Tags};
        for (class, tree) in [(0, 2), (1, 1)] {
            for rank in [1, 5] {
                let mut trained = apply_tree(class, tree, &[(0, rank), (13, rank)]);
                let mut baseline = Run {
                    class: trained.class,
                    ..Default::default()
                };
                trained.equipment_armour = 250.0;
                baseline.equipment_armour = 250.0;
                trained.equipment.reduction = 0.2;
                baseline.equipment.reduction = 0.2;
                let stats = trained.stats();
                assert!(stats
                    .resistances
                    .entries()
                    .iter()
                    .all(|(_, _, v)| *v == 2.0 * f32::from(rank)));
                assert!((stats.damage_reduction - 0.02 * f32::from(rank)).abs() < 0.0001);
                for school in [
                    Tags::PHYSICAL,
                    Tags::FIRE,
                    Tags::COLD,
                    Tags::LIGHTNING,
                    Tags::NATURE,
                    Tags::ARCANE,
                    Tags::HOLY,
                    Tags::SHADOW,
                ] {
                    for periodic in [false, true] {
                        let tags = if periodic { school | Tags::DOT } else { school };
                        trained.hp = 1000.0;
                        baseline.hp = 1000.0;
                        let before = hero::hurt_hero_tagged(&mut baseline, 100.0, tags);
                        let after = hero::hurt_hero_tagged(&mut trained, 100.0, tags);
                        let less = 1.0 - 0.02 * f32::from(rank);
                        let resistance = if school == Tags::PHYSICAL { 1.0 } else { less };
                        assert!((after - before * less * resistance).abs() < 0.001);
                    }
                }
                let empty = talents::Account::from_shared(class, 0, [[0; talents::NODES]; 3]);
                empty.apply(class, &mut trained);
                assert_eq!(trained.stats().damage_reduction, 0.0);
                assert!(trained
                    .stats()
                    .resistances
                    .entries()
                    .iter()
                    .all(|(_, _, v)| *v == 0.0));
                let spec = &talents::TREES[class][tree];
                assert!(talent_effects::description(spec, 0, rank)
                    .contains("resistance to all elements"));
                assert!(talent_effects::description(spec, 13, rank).contains("less damage taken"));
            }
        }
    }
    #[test]
    fn restoration_tree_empowers_damage_healing_and_support_totems_instead_of_direct_spells() {
        let run = apply_tree(
            5,
            2,
            &[
                (8, 1),
                (12, 1),
                (18, 1),
                (0, 5),
                (1, 5),
                (2, 5),
                (3, 5),
                (4, 5),
                (5, 5),
            ],
        );
        let base = Run {
            class: run.class,
            ..Default::default()
        };
        for key in [
            "healing_stream_totem",
            "totemic_recall",
            "strength_of_earth_totem",
        ] {
            assert!(run.abilities.iter().any(|o| o.def.key == key), "{key}");
        }
        for key in ["searing_totem", "magma_totem"] {
            let trained = run.resolve_spell(data::ability(key), 1);
            let untrained = base.resolve_spell(data::ability(key), 1);
            assert!(trained.dmg > untrained.dmg * 1.35);
            assert!((trained.duration - untrained.duration * 1.2).abs() < 0.001);
            assert_eq!(trained.max_totems, untrained.max_totems + 1);
            assert!(trained.cooldown < untrained.cooldown);
        }
        let trained = run.resolve_spell(data::ability("healing_stream_totem"), 1);
        let untrained = base.resolve_spell(data::ability("healing_stream_totem"), 1);
        assert!(trained.buff_effect > untrained.buff_effect + 0.4);
        for key in ["lightning_bolt", "lesser_healing_wave"] {
            assert_eq!(
                run.resolve_spell(data::ability(key), 1).dmg,
                base.resolve_spell(data::ability(key), 1).dmg
            );
        }
    }

    #[test]
    fn poison_and_companion_minors_only_boost_their_authored_sources() {
        let poison = apply_tree(3, 0, &[(0, 5)]);
        let baseline = Run {
            class: poison.class,
            ..Default::default()
        };
        let before = procs::resolve(&baseline, "proc_poison", 0.0, 10.0, 6.0, 0.0, 1);
        let after = procs::resolve(&poison, "proc_poison", 0.0, 10.0, 6.0, 0.0, 1);
        assert!((after.dot - before.dot * 1.25).abs() < 0.001);
        let pet = apply_tree(2, 0, &[(1, 5)]);
        let baseline = Run {
            class: pet.class,
            ..Default::default()
        };
        let before = procs::resolve(&baseline, "proc_pet_bite", 20.0, 0.0, 0.0, 0.0, 1);
        let after = procs::resolve(&pet, "proc_pet_bite", 20.0, 0.0, 0.0, 0.0, 1);
        assert!((after.dmg - before.dmg * 1.25).abs() < 0.001);
        assert_eq!(pet.stats().might, baseline.stats().might);
        let direct = pet.resolve_spell(data::ability("arcane_shot"), 1);
        let untrained = Run {
            class: pet.class,
            ..Default::default()
        }
        .resolve_spell(data::ability("arcane_shot"), 1);
        assert_eq!(direct.dmg, untrained.dmg);
    }

    #[test]
    fn arcane_milestones_grant_half_repeat_chance_without_recursive_copies() {
        for (nodes, expected) in [
            (vec![(8, 1)], 0.50),
            (vec![(8, 1), (12, 1)], 0.50),
            (vec![(8, 1), (12, 1), (18, 1)], 0.50),
        ] {
            let run = apply_tree(6, 0, &nodes);
            for key in [
                "arcane_missiles",
                "arcane_explosion",
                "fireball",
                "frostbolt",
            ] {
                let p = run.resolve_spell(data::ability(key), 1);
                assert!((p.recast - expected).abs() < 0.0001, "{key}: {}", p.recast);
            }
            for key in [
                "heroic_strike",
                "flash_heal",
                "power_word_shield",
                "searing_totem",
            ] {
                assert_eq!(
                    run.resolve_spell(data::ability(key), 1).recast,
                    0.0,
                    "{key}"
                );
            }
            assert_eq!(
                super::super::spell_engine::recast_count(expected, 0.1, true),
                0
            );
        }
        let run = apply_tree(6, 0, &[(8, 1), (12, 1), (18, 1)]);
        for roll in [0.0, 0.25, 0.75, 0.999] {
            assert_eq!(
                super::super::spell_engine::recast_count(
                    run.resolve_spell(data::ability("arcane_missiles"), 1)
                        .recast,
                    roll,
                    false
                ),
                u64::from(roll < 0.5)
            );
        }
    }
}
