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
    Design {title:"Armes",focus:"Armes équipées et dégâts physiques",filter:T::PHYSICAL,milestones:[
        m!("Weapon Master","Equipped weapons gain 35% base damage, affix strength and proc chance.",Weapon),
        m!("Sweeping Strikes","Physical melee attacks hit 2 extra targets.",None,C::AdditionalMeleeTargets(2)),
        m!("Armor Breaker","Physical attacks ignore 60% armor and deal 35% more damage.",None,C::ArmourPenetration(0.6),C::MoreDamage(0.35))]},
    Design {title:"Fureur",focus:"Saignements et vitesse d’attaque",filter:T::ATTACK,milestones:[
        m!("Deep Wounds","Physical skill hits bleed for 20% damage per second for 4 sec.",Bleed,C::BleedChance(1.0)),
        m!("Bloodbath","Bleeding enemies explode on death for 30% max health damage, up to 300. +20% attack speed.",Bloodburst,C::IncreasedAttackSpeed(0.2)),
        m!("Titan's Grip","Equip two two-handed weapons. +25% attack speed.",Titans,C::IncreasedAttackSpeed(0.25))]},
    Design {title:"Protection",focus:"Provocation, blocage et Coup de tonnerre",filter:T::PHYSICAL,milestones:[
        m!("Taunt","Taunt enemies within 12 yards for 3 sec every 6 sec. Unlocks Thunder Clap.",Taunt),
        m!("Bulwark","+20% block chance. Blocking reduces Thunder Clap's cooldown by 1 sec.",Block),
        m!("Thunder Fortress","Blocking releases a 45-damage shockwave within 6 yards, once per second. +40% area.",Bulwark,C::IncreasedArea(0.4))]},
    Design {title:"Sacré",focus:"Soins, Horion sacré et Exorcisme",filter:T::HOLY,milestones:[
        m!("Benevolent Light","Holy hits heal you and nearby allies for 4% damage dealt.",HolyHeal),
        m!("Healing Shock","Every 6 sec, heal nearby allies for 12% max health and deal 40 Holy damage around you.",HolyBurst),
        m!("Judgment of Light","Holy hits heal for 8% extra damage and strike 2 nearby enemies for 50% damage.",HolyJudgement)]},
    Design {title:"Protection",focus:"Consécration, blocage et soins personnels",filter:T::HOLY,milestones:[
        m!("Consecrated Defender","Taunt nearby enemies every 6 sec. Unlocks Consecration.",Taunt),
        m!("Blessed Shield","+20% block chance. Blocking restores 3% max health.",Block),
        m!("Sanctuary","Blocking deals 45 Holy damage around you and restores 2% max health, once per second. +40% area.",Bulwark,C::IncreasedArea(0.4))]},
    Design {title:"Vindicte",focus:"Sceaux et attaques de zone",filter:T::ATTACK,milestones:[
        m!("Seal of Righteousness","Skill hits deal an extra Holy hit for 25% damage.",Seal),
        m!("Seal of Command","Skill hits splash 40% damage within 4 yards.",Splash),
        m!("Divine Storm","Seal hits splash to nearby enemies and restore health equal to 5% attack damage.",SealStorm)]},
    Design {title:"Maîtrise des bêtes",focus:"Familier et saignements",filter:T::ATTACK,milestones:[
        m!("Wild Companion","Summon a wolf that hunts enemies. Physical skill hits have a 50% chance to bleed.",Pet,C::BleedChance(0.5)),
        m!("Hungry Pack","Summon a second wolf. Their bites cause bleeding for 4 sec.",Pack),
        m!("Pack Frenzy","Bleeding enemies explode on death. +35% attack speed.",Bloodburst,C::IncreasedAttackSpeed(0.35))]},
    Design {title:"Précision",focus:"Projectiles et critiques",filter:T::PROJECTILE,milestones:[
        m!("Barrage","Projectile skills fire 2 extra projectiles.",None,C::AdditionalProjectiles(2)),
        m!("Ricochet","Projectiles chain to 2 extra enemies.",None,C::AdditionalChains(2)),
        m!("Splinter Shot","Projectiles fork on impact. +40% critical damage.",None,C::Fork(1),C::CriticalMultiplier(0.4))]},
    Design {title:"Survie",focus:"Mêlée, vie, pièges et dégâts sur la durée",filter:T::ATTACK,milestones:[
        m!("Close-Range Predator","Unlocks Raptor Strike. Physical skill hits always cause bleeding.",Bleed,C::BleedChance(1.0)),
        m!("Trapper","Unlocks Explosive Trap. +60% attack duration and +40% attack area.",None,C::IncreasedDuration(0.6),C::IncreasedArea(0.4)),
        m!("Death Trap","Enemies affected by your damage over time explode on death for 30% max health damage, up to 300.",Plagueburst)]},
    Design {title:"Assassinat",focus:"Poison et prolifération",filter:T::ATTACK,milestones:[
        m!("Deadly Poison","Skill hits poison enemies for 18% damage per second for 6 sec. Further hits refresh the poison.",Poison),
        m!("Toxic Contagion","Your poison spreads to enemies within 6 yards when its target dies.",Proliferate),
        m!("Poison Bomb","Poisoned enemies explode on death for 30% max health damage, up to 300.",Poisonburst)]},
    Design {title:"Combat",focus:"Vitesse d’attaque et attaques de zone",filter:T::ATTACK,milestones:[
        m!("Blade Flurry","Skill hits splash 40% damage to nearby enemies.",Splash),
        m!("Combat Potency","25% chance to repeat attacks. +20% attack speed.",None,C::RecastChance(0.25),C::IncreasedAttackSpeed(0.2)),
        m!("Blade Dance","Every third skill hit deals 80% damage to enemies within 6 yards.",Flurry)]},
    Design {title:"Finesse",focus:"Critiques et acharnement monocible",filter:T::ATTACK,milestones:[
        m!("Exposed Weakness","Repeated hits on one target gain 8% damage per hit, up to 80%. Switching targets resets the bonus.",Focus),
        m!("Premeditation","+15% critical chance and +50% critical damage.",None,C::CriticalChance(0.15),C::CriticalMultiplier(0.5)),
        m!("Perfect Execution","Every tenth consecutive hit on one target strikes again for 200% damage.",Execution)]},
    Design {title:"Discipline",focus:"Boucliers et détonations",filter:T::GUARD,milestones:[
        m!("Reactive Shield","Refreshing or breaking your shield deals 60% of its original absorption as nearby Holy damage.",Shieldburst),
        m!("Atonement","Shield explosions restore 10% max health. +25% shield strength.",ShieldHeal,C::IncreasedEffect(0.25)),
        m!("Discipline Nova","Shield explosions deal 120% of original absorption and reach 10 yards.",ShieldNova)]},
    Design {title:"Sacré",focus:"Soins directs et dégâts sacrés",filter:T::HOLY,milestones:[
        m!("Flash Heal","Every 6 sec, heal nearby allies for 12% max health and deal 40 Holy damage around you.",HolyBurst),
        m!("Wave of Light","Holy damage heals you and nearby allies for 4% damage dealt.",HolyHeal,C::AdditionalMeleeTargets(1)),
        m!("Apotheosis","+35% Holy damage. Holy hits heal you and nearby allies for 8% extra damage.",HolyJudgement,C::MoreDamage(0.35))]},
    Design {title:"Ombre",focus:"Dégâts d’ombre sur la durée et vol de vie",filter:T::SHADOW,milestones:[
        m!("Vampiric Embrace","Recover health equal to 12% of your direct and periodic Shadow damage.",ShadowDrain),
        m!("Devouring Plague","Shadow hits apply a plague dealing 18% damage per second for 6 sec. +30% Shadow damage over time.",ShadowPlague,C::IncreasedDotDamage(0.3)),
        m!("Shadow Pandemic","Your damage over time spreads when its target dies, causing a nearby explosion.",PlagueSpread)]},
    Design {title:"Élémentaire",focus:"Dégâts élémentaires et chaînes",filter:T::SPELL,milestones:[
        m!("Conduction","Elemental projectiles chain to an extra target.",None,C::AdditionalChains(1)),
        m!("Elemental Overload","25% chance to repeat elemental spells.",None,C::RecastChance(0.25)),
        m!("Maelstrom","Elemental projectiles gain 2 chain jumps and fork on impact.",None,C::AdditionalChains(2),C::Fork(1))]},
    Design {title:"Amélioration",focus:"Procs d’armes élémentaires",filter:T::ATTACK,milestones:[
        m!("Windfury Weapon","Skill hits have a 20% chance to trigger two extra strikes for 50% damage each.",Windfury),
        m!("Flametongue Weapon","Skill hits deal an extra Fire hit for 25% damage.",Seal),
        m!("Stormstrike","Every fifth skill hit strikes up to 4 nearby enemies for 100% damage as Lightning.",Stormstrike)]},
    Design {title:"Restauration",focus:"Totems de soin et de dégâts",filter:T::TOTEM,milestones:[
        m!("Healing Totem","Summon a totem that heals nearby allies for 2% max health per second.",HealTotem),
        m!("Totemic Harmony","Your healing totem also fires a 24-damage bolt every second. +50% totem duration.",DualTotem,C::IncreasedDuration(0.5)),
        m!("Spirit Totem","Your totem heals for 4% max health per second and its bolts hit 3 enemies.",SpiritTotem)]},
    Design {title:"Arcanes",focus:"Répétition des sorts et résonance magique",filter:T::SPELL,milestones:[
        m!("Arcane Echo","Offensive spells gain 50% repeat chance. Repeated casts cannot repeat again.",None,C::RecastChance(0.50)),
        m!("Resonant Casting","Each repeated spell also triggers another learned spell, or the source if it is your only spell. This extra cast cannot repeat or trigger another extra cast.",ArcaneCompanion),
        m!("Greater Spell Echo","Each successive repeat deals 20% more damage than the previous one: 120%, 144%, 172.8%, 207.36%. The counter resets for each original cast. Copies cannot create new repeats.",ArcaneCascade)]},
    Design {title:"Feu",focus:"Enflammer et prolifération",filter:T::FIRE,milestones:[
        m!("Ignite","Fire hits burn enemies for 25% damage per second for 3 sec.",Ignite,C::IgniteChance(1.0)),
        m!("Spreading Flames","Ignite spreads to enemies within 6 yards when its target dies.",IgniteSpread),
        m!("Combustion","Ignited enemies explode on death for 30% max health damage, up to 300.",Fireburst)]},
    Design {title:"Givre",focus:"Ralentissements et gel",filter:T::COLD,milestones:[
        m!("Frostbite","Frost hits slow by 40% for 3 sec and have a 20% chance to freeze. Frost damage is 10% lower.",Freeze,C::MoreDamage(-0.1)),
        m!("Shatter","Deal 60% more Frost damage to frozen enemies.",Shatter),
        m!("Endless Winter","Frozen enemies explode on death and freeze nearby enemies for 2 sec.",FrostNova)]},
    Design {title:"Affliction",focus:"Dégâts sur la durée, contagion et explosions",filter:T::SHADOW,milestones:[
        m!("Unstable Affliction","Shadow hits apply an affliction dealing 18% damage per second for 6 sec. +25% Shadow damage over time.",Plague,C::IncreasedDotDamage(0.25)),
        m!("Contagion","Your damage over time spreads to enemies within 6 yards when its target dies.",PlagueSpread),
        m!("Doom Eruption","Enemies affected by your damage over time explode on death for 30% max health damage, up to 300.",Plagueburst)]},
    Design {title:"Démonologie",focus:"Invocation d’une armée démoniaque",filter:T::SHADOW,milestones:[
        m!("Demonic Pact","Summon an imp that casts Firebolts.",Demon),
        m!("Legion","Summon two extra demons. Their attacks burn enemies.",Demons),
        m!("Demon Lord","Demons deal double damage and their hits explode within 4 yards.",DemonLord)]},
    Design {title:"Destruction",focus:"Sorts lents et dévastateurs",filter:T::SPELL,milestones:[
        m!("Devastating Cast","Spells deal 70% more damage but have 35% longer cooldowns.",SlowCast,C::MoreDamage(0.7),C::IncreasedCooldownRecovery(-0.26)),
        m!("Ruin","+100% spell critical damage.",None,C::CriticalMultiplier(1.0)),
        m!("Cataclysm","Every third skill hit explodes for 200% damage within 8 yards, once per second.",Cataclysm)]},
    Design {title:"Équilibre",focus:"Dégâts d’arcanes et étoiles",filter:T::ARCANE,milestones:[
        m!("Starfire","Arcane spells deal 30% more damage and fire an extra projectile.",None,C::MoreDamage(0.3),C::AdditionalProjectiles(1)),
        m!("Lunar Flare","Arcane hits burn enemies for 18% damage per second for 6 sec.",Moonfire),
        m!("Starfall","Every fifth Arcane hit causes a starburst for 150% damage within 7 yards.",Starfall)]},
    Design {title:"Combat farouche",focus:"Saignements de mêlée et résistance",filter:T::MELEE,milestones:[
        m!("Blood Claws","Permanently take Cat Form. Physical melee skill hits always cause bleeding.",Bleed,C::BleedChance(1.0)),
        m!("Bear Instinct","Take 15% less damage and recover health equal to 5% of melee damage.",FeralGuard,C::LifeLeech(0.05)),
        m!("Savage Carnage","Bleeding enemies explode on death. +25% attack speed.",Bloodburst,C::IncreasedAttackSpeed(0.25))]},
    Design {title:"Restauration",focus:"Soins sur la durée et soutien des alliés",filter:T::HEAL,milestones:[
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
