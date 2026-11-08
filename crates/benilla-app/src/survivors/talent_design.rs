//! Authored specialization identities. Tooltips and combat consume these same milestones.
use super::{
    spell_engine::{Change, Tags},
    talents::Spec,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Mechanic {
    None,
    Weapon,
    Bleed,
    Bloodburst,
    Titans,
    Taunt,
    Block,
    Bulwark,
    HolyHeal,
    HolyBurst,
    HolyJudgement,
    Seal,
    SealStorm,
    Pet,
    Pack,
    Poison,
    Proliferate,
    Poisonburst,
    Splash,
    Flurry,
    Focus,
    Execution,
    Shieldburst,
    ShieldHeal,
    ShieldNova,
    ShadowDrain,
    ShadowPlague,
    Windfury,
    Stormstrike,
    HealTotem,
    DualTotem,
    SpiritTotem,
    #[allow(
        dead_code,
        reason = "Retained proc-chain identity for legacy Arcane mechanics"
    )]
    ArcaneFocus,
    #[allow(
        dead_code,
        reason = "Retained proc-chain identity for legacy Arcane mechanics"
    )]
    ArcaneBlast,
    Ignite,
    IgniteSpread,
    Fireburst,
    Freeze,
    Shatter,
    FrostNova,
    Plague,
    PlagueSpread,
    Plagueburst,
    Demon,
    Demons,
    DemonLord,
    SlowCast,
    Cataclysm,
    Moonfire,
    Starfall,
    FeralGuard,
    Hot,
    AllyBuff,
    Grove,
    ArcaneCompanion,
    ArcaneCascade,
    TrapLaunch,
    TrapVolley,
    DeathTrap,
}
#[derive(Clone, Copy)]
pub(super) struct Milestone {
    pub name: &'static str,
    pub description: &'static str,
    pub mechanic: Mechanic,
    pub changes: &'static [Change],
}
pub(super) struct Design {
    pub title: &'static str,
    pub focus: &'static str,
    pub filter: Tags,
    pub milestones: [Milestone; 3],
}
macro_rules! m {
    ($name:literal,$text:literal,$mechanic:ident $(,$change:expr)*) => {
        Milestone {name:$name,description:$text,mechanic:Mechanic::$mechanic,changes:&[$($change),*]}
    };
}
use Change as C;
use Tags as T;
pub(super) static DESIGNS: [Design; 27] = [
    Design {title:"Arms",focus:"Equipped weapons and Physical damage",filter:T::PHYSICAL,milestones:[
        m!("Weapon Master","Equipped weapons gain 35% base damage, affix strength and proc chance.",Weapon),
        m!("Sweeping Strikes","Physical melee attacks hit 2 extra targets.",None,C::AdditionalMeleeTargets(2)),
        m!("Armor Breaker","Physical attacks ignore 60% armor and deal 35% more damage.",None,C::ArmourPenetration(0.6),C::MoreDamage(0.35))]},
    Design {title:"Fury",focus:"Bleeds and attack speed",filter:T::ATTACK,milestones:[
        m!("Deep Wounds","Physical skill hits bleed for 20% damage per second for 4 sec.",Bleed,C::BleedChance(1.0)),
        m!("Bloodbath","Bleeding enemies explode on death for 30% of the dying enemy's maximum health, capped at 1000 base damage before damage modifiers and critical strikes. +20% attack speed.",Bloodburst,C::IncreasedAttackSpeed(0.2)),
        m!("Titan's Grip","Equip two two-handed weapons. +25% attack speed.",Titans,C::IncreasedAttackSpeed(0.25))]},
    Design {title:"Protection",focus:"Taunting, blocking and Thunder Clap",filter:T::PHYSICAL,milestones:[
        m!("Taunt","Taunt enemies within 12 yards for 3 sec every 6 sec. Unlocks Thunder Clap.",Taunt),
        m!("Bulwark","+20% block chance. Blocking reduces Thunder Clap's cooldown by 1 sec.",Block),
        m!("Thunder Fortress","Blocking releases a 45-damage shockwave within 6 yards, once per second. +40% area.",Bulwark,C::IncreasedArea(0.4))]},
    Design {title:"Holy",focus:"Healing, Holy Shock and Exorcism",filter:T::HOLY,milestones:[
        m!("Benevolent Light","Holy hits heal you and nearby allies for 4% damage dealt.",HolyHeal),
        m!("Healing Shock","Every 6 sec, heal nearby allies for 12% max health and deal 40 Holy damage around you.",HolyBurst),
        m!("Judgment of Light","Holy hits heal for 8% extra damage and strike 2 nearby enemies for 50% damage.",HolyJudgement)]},
    Design {title:"Protection",focus:"Consecration, blocking and self-healing",filter:T::HOLY,milestones:[
        m!("Consecrated Defender","Taunt nearby enemies every 6 sec. Unlocks Consecration.",Taunt),
        m!("Blessed Shield","+20% block chance. Blocking restores 3% max health.",Block),
        m!("Sanctuary","Blocking deals 45 Holy damage around you and restores 2% max health, once per second. +40% area.",Bulwark,C::IncreasedArea(0.4))]},
    Design {title:"Retribution",focus:"Seals and area attacks",filter:T::ATTACK,milestones:[
        m!("Seal of Righteousness","Skill hits deal an extra Holy hit for 25% damage.",Seal),
        m!("Seal of Command","Skill hits splash 40% damage within 4 yards.",Splash),
        m!("Divine Storm","Seal hits splash to nearby enemies and restore health equal to 5% attack damage.",SealStorm)]},
    Design {title:"Beast Mastery",focus:"Pets, speed and bleeds",filter:T::MINION,milestones:[
        m!("Wild Companion","Summon a wolf whose attacks hit up to 3 nearby enemies. Replaces Arcane Shot with Kill Command. Minions gain 20% more action speed, multiplicative with their attack and movement speed.",Pet,C::MoreActionSpeed(0.20)),
        m!("Hungry Pack","Summon a second wolf. Damaging Minion attacks and skills cause Physical bleeding for 20% of the hit's damage per second for 4 sec. Scales with damage over time and duration bonuses.",Pack),
        m!("Pack Frenzy","Explosions count as Minion attacks and apply Hungry Pack bleeding. Bleeding enemies explode on death for 30% of the dying enemy's maximum health, capped at 1000 base damage before damage modifiers and critical strikes. +35% Minion attack speed.",Bloodburst,C::IncreasedAttackSpeed(0.35))]},
    Design {title:"Marksmanship",focus:"Projectiles and critical hits",filter:T::PROJECTILE,milestones:[
        m!("Barrage","Projectile skills fire 2 extra projectiles.",None,C::AdditionalProjectiles(2)),
        m!("Ricochet","Projectiles chain to 2 extra enemies.",None,C::AdditionalChains(2)),
        m!("Splinter Shot","Projectiles fork on impact. +40% critical damage.",None,C::Fork(1),C::CriticalMultiplier(0.4))]},
    Design {title:"Survival",focus:"Durability and trap mastery",filter:T::TRAP,milestones:[
        m!("Trap Launcher","Place traps under enemies using spell targeting. Replaces your starting spell with Explosive Trap. Traps have 50% less cooldown.",TrapLaunch,C::LessCooldown(0.5)),
        m!("Trapper","Traps target two enemies. 40% more Trap damage and 40% more Trap area (multiplicative).",TrapVolley,C::MoreDamage(0.4),C::MoreArea(0.4)),
        m!("Death Trap","Spreads Explosive Trap's damage over time on death, preserving its remaining duration and tick timing. Copied DoTs can trigger Death Trap again without refreshing their duration. Enemies affected by your damage over time explode on death for 30% of the dying enemy's maximum health, capped at 1000 base damage before damage modifiers and critical strikes as Fire damage.",DeathTrap)]},
    Design {title:"Assassination",focus:"Poison and proliferation",filter:T::ATTACK,milestones:[
        m!("Deadly Poison","Skill hits add a poison stack dealing 18% damage per second for 6 sec. Each stack has its own duration.",Poison),
        m!("Toxic Contagion","Your poison spreads to enemies within 6 yards when its target dies, preserving its remaining duration and tick timing.",Proliferate),
        m!("Poison Bomb","Poisoned enemies explode on death for 30% of the dying enemy's maximum health, capped at 1000 base damage before damage modifiers and critical strikes.",Poisonburst)]},
    Design {title:"Combat",focus:"Attack speed and area attacks",filter:T::ATTACK,milestones:[
        m!("Blade Flurry","Skill hits splash 40% damage to nearby enemies.",Splash),
        m!("Combat Potency","25% chance to repeat attacks. +20% attack speed.",None,C::RecastChance(0.25),C::IncreasedAttackSpeed(0.2)),
        m!("Blade Dance","Every third skill hit deals 80% damage to enemies within 6 yards.",Flurry)]},
    Design {title:"Subtlety",focus:"Critical hits and sustained single-target attacks",filter:T::ATTACK,milestones:[
        m!("Exposed Weakness","Repeated hits on one target gain 8% damage per hit, up to 80%. Switching targets resets the bonus.",Focus),
        m!("Premeditation","+15% critical chance and +50% critical damage.",None,C::CriticalChance(0.15),C::CriticalMultiplier(0.5)),
        m!("Perfect Execution","Every tenth consecutive hit on one target strikes again for 200% damage.",Execution)]},
    Design {title:"Discipline",focus:"Shields and detonations",filter:T::GUARD,milestones:[
        m!("Reactive Shield","Refreshing or breaking your shield deals 60% of its original absorption as nearby Holy damage.",Shieldburst),
        m!("Atonement","Shield explosions restore 10% max health. +25% shield strength.",ShieldHeal,C::IncreasedEffect(0.25)),
        m!("Discipline Nova","Shield explosions deal 120% of original absorption and reach 10 yards.",ShieldNova)]},
    Design {title:"Holy",focus:"Direct healing and Holy damage",filter:T::HOLY,milestones:[
        m!("Flash Heal","Every 6 sec, heal nearby allies for 12% max health and deal 40 Holy damage around you.",HolyBurst),
        m!("Wave of Light","Holy damage heals you and nearby allies for 4% damage dealt.",HolyHeal,C::AdditionalMeleeTargets(1)),
        m!("Apotheosis","+35% Holy damage. Holy hits heal you and nearby allies for 8% extra damage.",HolyJudgement,C::MoreDamage(0.35))]},
    Design {title:"Shadow",focus:"Shadow damage over time and life leech",filter:T::SHADOW,milestones:[
        m!("Vampiric Embrace","Recover health equal to 12% of your direct and periodic Shadow damage.",ShadowDrain),
        m!("Devouring Plague","Shadow hits apply a plague dealing 18% damage per second for 6 sec. +30% Shadow damage over time.",ShadowPlague,C::IncreasedDotDamage(0.3)),
        m!("Shadow Pandemic","Your damage over time spreads with its remaining duration and tick timing when its target dies, causing a Shadow explosion for 30% of its maximum health, capped at 1000 base damage before damage modifiers and critical strikes.",PlagueSpread)]},
    Design {title:"Elemental",focus:"Elemental damage and chains",filter:T::SPELL,milestones:[
        m!("Conduction","Elemental projectiles chain to an extra target.",None,C::AdditionalChains(1)),
        m!("Elemental Overload","25% chance to repeat elemental spells.",None,C::RecastChance(0.25)),
        m!("Maelstrom","Elemental projectiles gain 2 chain jumps and fork on impact.",None,C::AdditionalChains(2),C::Fork(1))]},
    Design {title:"Enhancement",focus:"Elemental weapon procs",filter:T::ATTACK,milestones:[
        m!("Windfury Weapon","Skill hits have a 20% chance to trigger two extra strikes for 50% damage each.",Windfury),
        m!("Flametongue Weapon","Skill hits deal an extra Fire hit for 25% damage.",Seal),
        m!("Stormstrike","Every fifth skill hit strikes up to 4 nearby enemies for 100% damage as Lightning.",Stormstrike)]},
    Design {title:"Restoration",focus:"Healing and damage totems",filter:T::TOTEM,milestones:[
        m!("Ancestral Totem","Summon a stationary healing totem.\nHealing: 2% max health per second. Radius: 12 yd.\nParty blessing: +25% cooldown recovery and +10% offensive spell repeat chance, scaled by buff effect.\nUnlocks Healing Stream Totem and Totemic Recall.\n+20% totem power and buff effect.",HealTotem,C::IncreasedEffect(0.2),C::IncreasedBuffEffect(0.2)),
        m!("Totemic Harmony","Your ancestral totem also fires a 24-damage Lightning bolt.\nUnlocks Strength of Earth Totem.\n+1 maximum totems for every totem spell.",DualTotem,C::AdditionalTotems(1)),
        m!("Spirit Council","Summon a second ancestral totem. Their healing rises to 4% max health per second without stacking, and each volley hits up to 3 enemies. All totems deal 35% more damage and gain +25% buff effect. Party blessings are retained.",SpiritTotem,C::MoreDamage(0.35),C::IncreasedBuffEffect(0.25))]},
    Design {title:"Arcane",focus:"Spell repeats and magical resonance",filter:T::SPELL,milestones:[
        m!("Arcane Echo","Offensive spells gain 50% repeat chance. Repeated casts cannot repeat again.",None,C::RecastChance(0.50)),
        m!("Resonant Casting","Repeated spells trigger one additional learned spell, or the source if it is your only spell. Fixed 0.75s internal cooldown, unaffected by haste or cooldown modifiers. This extra cast cannot repeat or trigger any other proc.",ArcaneCompanion),
        m!("Greater Spell Echo","Each successive repeat deals 20% more damage than the previous one: 120%, 144%, 172.8%, 207.36%. The counter resets for each original cast. Copies cannot create new repeats.",ArcaneCascade)]},
    Design {title:"Fire",focus:"Ignite and proliferation",filter:T::FIRE,milestones:[
        m!("Ignite","Fire hits burn enemies for 25% damage per second for 3 sec.",Ignite,C::IgniteChance(1.0)),
        m!("Spreading Flames","Ignite spreads to enemies within 6 yards when its target dies, preserving its remaining duration and tick timing.",IgniteSpread),
        m!("Combustion","Ignited enemies explode on death for 30% of the dying enemy's maximum health, capped at 1000 base damage before damage modifiers and critical strikes.",Fireburst)]},
    Design {title:"Frost",focus:"Slows and freezing",filter:T::COLD,milestones:[
        m!("Frostbite","Frost hits slow by 40% for 3 sec and have a 20% chance to freeze. Frost damage is 10% lower.",Freeze,C::MoreDamage(-0.1)),
        m!("Shatter","Deal 60% more Frost damage to frozen enemies.",Shatter),
        m!("Endless Winter","Frozen enemies explode on death for 30% of their maximum health, capped at 1000 base damage before damage modifiers and critical strikes, and freeze nearby enemies for 2 sec.",FrostNova)]},
    Design {title:"Affliction",focus:"Damage over time, contagion and explosions",filter:T::SHADOW,milestones:[
        m!("Unstable Affliction","Shadow hits add an affliction stack dealing 18% damage per second for 6 sec. Each stack has its own duration. +25% Shadow damage over time.",Plague,C::IncreasedDotDamage(0.25)),
        m!("Contagion","Your damage over time spreads to enemies within 6 yards when its target dies, preserving its remaining duration and tick timing.",PlagueSpread),
        m!("Doom Eruption","Enemies affected by your damage over time explode on death for 30% of the dying enemy's maximum health, capped at 1000 base damage before damage modifiers and critical strikes.",Plagueburst)]},
    Design {title:"Demonology",focus:"Summoning a demonic army",filter:T::MINION,milestones:[
        m!("Demonic Pact","Summon an imp that casts Firebolts.",Demon),
        m!("Legion","Summon two extra demons. Their attacks burn enemies.",Demons),
        m!("Demon Lord","Demons deal double damage and their hits explode within 4 yards.",DemonLord)]},
    Design {title:"Destruction",focus:"Slow, devastating spells",filter:T::SPELL,milestones:[
        m!("Devastating Cast","Spells deal 70% more damage but have 35% longer cooldowns.",SlowCast,C::MoreDamage(0.7),C::IncreasedCooldownRecovery(-0.26)),
        m!("Ruin","+100% spell critical damage.",None,C::CriticalMultiplier(1.0)),
        m!("Cataclysm","Every third skill hit explodes for 200% damage within 8 yards, once per second.",Cataclysm)]},
    Design {title:"Balance",focus:"Arcane damage and stars",filter:T::ARCANE,milestones:[
        m!("Starfire","Arcane spells deal 30% more damage and fire an extra projectile.",None,C::MoreDamage(0.3),C::AdditionalProjectiles(1)),
        m!("Lunar Flare","Arcane hits burn enemies for 18% damage per second for 6 sec.",Moonfire),
        m!("Starfall","Every fifth Arcane hit causes a starburst for 150% damage within 7 yards.",Starfall)]},
    Design {title:"Feral Combat",focus:"Melee bleeds and durability",filter:T::MELEE,milestones:[
        m!("Blood Claws","Permanently take Cat Form. Physical melee skill hits always cause bleeding.",Bleed,C::BleedChance(1.0)),
        m!("Bear Instinct","Take 15% less damage and recover health equal to 5% of melee damage.",FeralGuard,C::LifeLeech(0.05)),
        m!("Savage Carnage","Bleeding enemies explode on death for 30% of their maximum health, capped at 1000 base damage before damage modifiers and critical strikes. +25% attack speed.",Bloodburst,C::IncreasedAttackSpeed(0.25))]},
    Design {title:"Restoration",focus:"Healing over time and party support",filter:T::HEAL,milestones:[
        m!("Bloom","Nearby allies recover 1.5% max health per second.",Hot),
        m!("Gift of Nature","Nearby allies gain 8% damage, 20% movement speed and 5% all elemental resistances. The support aura scales with your Nature spell power.",AllyBuff),
        m!("Living Grove","Bloom heals for 3% max health per second and an extra 12% every 6 sec.",Grove)]},
];

pub(super) fn index(spec: &Spec) -> usize {
    super::talents::TREES
        .iter()
        .flatten()
        .position(|s| std::ptr::eq(s, spec))
        .expect("authored specialization")
}
pub(super) fn design(spec: &Spec) -> &'static Design {
    &DESIGNS[index(spec)]
}
pub(super) fn milestone(node: usize) -> Option<usize> {
    match node {
        8 => Some(0),
        12 => Some(1),
        18 => Some(2),
        _ => None,
    }
}
pub(super) fn icon(spec: &Spec, node: usize) -> u32 {
    let Some(index) = milestone(node) else {
        // Passive icons come from the native talent catalog, never a repeating skill list.
        return 0;
    };
    match design(spec).milestones[index].mechanic {
        Mechanic::Titans => 674,
        Mechanic::Taunt => 355,
        Mechanic::Block => 2565,
        Mechanic::Bulwark => 6343,
        Mechanic::Bleed | Mechanic::Bloodburst => 772,
        Mechanic::Poison => 2818,
        Mechanic::Proliferate | Mechanic::Poisonburst => 2823,
        Mechanic::Pet => 883,
        Mechanic::Pack => 19574,
        Mechanic::Demon => 688,
        Mechanic::Demons => 691,
        Mechanic::DemonLord => 1122,
        Mechanic::Shieldburst | Mechanic::ShieldHeal | Mechanic::ShieldNova => 17,
        Mechanic::HealTotem | Mechanic::DualTotem | Mechanic::SpiritTotem => 5394,
        Mechanic::Hot | Mechanic::Grove => 774,
        Mechanic::AllyBuff => 1126,
        Mechanic::Freeze => 116,
        Mechanic::Shatter => 122,
        Mechanic::FrostNova => 10,
        Mechanic::Ignite => 133,
        Mechanic::IgniteSpread => 2120,
        Mechanic::Fireburst => 11129,
        _ => super::data::ability(spec.cap).spell,
    }
}
