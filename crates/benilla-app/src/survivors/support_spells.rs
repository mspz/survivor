//! Party buffs refresh by identity; fields and reactions retain their original caster.
use super::{
    data,
    spell_engine::{Resolved, Tags},
    Run,
};
use bevy::prelude::*;

pub(super) const SPELLS: &[(u8, &str, &str, &str)] = &[
    (11,"mangle","Mangle","A melee strike that increases the victim's bleed damage taken for 8 sec."),
    (11,"rip","Rip","Apply a powerful, long Physical bleed in melee range."),
    (11,"ferocious_bite","Ferocious Bite","A heavy melee strike, dealing 20% more damage per distinct personal bleed on the victim, up to 60%."),
    (11,"savage_roar","Savage Roar","After three melee attack activations, temporarily increase your attack damage."),
    (11,"barkskin","Barkskin","Temporarily reduce incoming damage without interrupting your attacks."),
    (11,"mark_of_the_wild","Mark of the Wild","Temporarily increase nearby allies' armour and all elemental resistances."),
    (11,"stampeding_roar","Stampeding Roar","Temporarily increase nearby allies' movement speed."),
    (1,"battle_shout","Battle Shout","Temporarily increase nearby allies' attack damage."),
    (1,"commanding_shout","Commanding Shout","Give nearby allies a temporary protective reserve."),
    (2,"blessing_of_might","Blessing of Might","Temporarily increase nearby allies' attack damage."),
    (2,"devotion_aura","Devotion Aura","Continuously grant armour to allies within your aura. Leaving its range removes the benefit shortly afterwards."),
    (3,"hunters_mark","Hunter's Mark","Mark the toughest enemy in range, increasing attack damage it takes from the group. Half strength against bosses."),
    (3,"aspect_of_the_pack","Aspect of the Pack","Periodically increase nearby allies' movement speed."),
    (4,"slice_and_dice","Slice and Dice","After three successful attack activations, temporarily increase your attack speed."),
    (4,"smoke_bomb","Smoke Screen","Place a fixed smoke field. Allies inside take less damage from enemy ranged attacks."),
    (5,"prayer_of_mending","Prayer of Mending","Bless the most injured nearby ally. Taking health damage consumes the blessing to heal, then passes it to another living ally in range. Up to five heals; no bounce in solo play."),
    (5,"wand_attack","Wand Attack","An Arcane ranged weapon attack; benefits from Attack and Projectile modifiers."),
    (7,"strength_of_earth_totem","Strength of Earth Totem","Place a stationary totem that increases attack damage for nearby allies."),
    (7,"healing_stream_totem","Healing Stream Totem","Place a stationary totem that heals nearby living allies every two seconds."),
    (8,"arcane_intellect","Arcane Intellect","Temporarily increase nearby allies' damaging spells. Does not increase healing or buff strength."),
    (8,"frost_guardian","Frost Guardian","Give nearby allies temporary absorption. Breaking this protection slows the attacker."),
    (9,"curse_of_elements","Curse of the Elements","Curse the toughest enemy in range, increasing magical damage it takes from the group. Half strength against bosses."),
    (9,"demon_armor","Demon Armor","Temporarily increase your armour and healing received."),
];
pub(super) fn name(key: &str) -> Option<&'static str> {
    SPELLS.iter().find(|s| s.1 == key).map(|s| s.2)
}
pub(super) fn description(key: &str) -> Option<&'static str> {
    SPELLS.iter().find(|s| s.1 == key).map(|s| s.3)
}
pub(super) fn school(key: &str) -> Option<Tags> {
    Some(match key {
        "mangle" | "rip" | "ferocious_bite" | "savage_roar" | "battle_shout"
        | "commanding_shout" | "hunters_mark" | "aspect_of_the_pack" | "slice_and_dice"
        | "smoke_bomb" | "stampeding_roar" => Tags::PHYSICAL,
        "barkskin" | "mark_of_the_wild" | "strength_of_earth_totem" | "healing_stream_totem" => {
            Tags::NATURE
        }
        "blessing_of_might" | "devotion_aura" | "prayer_of_mending" => Tags::HOLY,
        "wand_attack" | "arcane_intellect" => Tags::ARCANE,
        "frost_guardian" => Tags::COLD,
        "curse_of_elements" | "demon_armor" => Tags::SHADOW,
        _ => return None,
    })
}
pub(super) fn magnitude(key: &str, level: u8) -> f32 {
    let rank = super::specialization::steps(level);
    match key {
        "battle_shout" | "hunters_mark" => 0.12 + 0.02 * rank,
        "blessing_of_might" | "strength_of_earth_totem" => 0.10 + 0.02 * rank,
        "savage_roar" | "slice_and_dice" | "mangle" => 0.20 + 0.02 * rank,
        "barkskin" => (0.20 + 0.01 * rank).min(0.5),
        "aspect_of_the_pack" => 0.15 + 0.01 * rank,
        "stampeding_roar" => 0.30 + 0.02 * rank,
        "smoke_bomb" => (0.25 + 0.02 * rank).min(0.6),
        "arcane_intellect" | "curse_of_elements" => 0.15 + 0.02 * rank,
        "devotion_aura" => 80.0 + 15.0 * rank,
        "mark_of_the_wild" => 40.0 + 8.0 * rank,
        "demon_armor" => 60.0 + 10.0 * rank,
        "commanding_shout" => 25.0 + 5.0 * rank,
        "frost_guardian" | "prayer_of_mending" => 20.0 + 3.0 * rank,
        "healing_stream_totem" => 6.0 + 1.5 * rank,
        _ => 0.0,
    }
}
pub(super) fn effective_magnitude(key: &str, level: u8, effect: f32) -> f32 {
    let power = magnitude(key, level)
        * (1.0 + effect.max(0.0))
        * if level >= 6 && data::ability(key).kind == data::Kind::Buff {
            super::specialization::SIGNATURE_POWER
        } else {
            1.0
        };
    if matches!(key, "barkskin" | "smoke_bomb") {
        power.min(0.75)
    } else {
        power
    }
}
#[derive(Clone)]
pub(super) struct Buff {
    pub key: &'static str,
    pub power: f32,
    pub until: f32,
    pub reserve: f32,
    pub caster: Option<u64>,
    pub jumps: u8,
}
#[derive(Default)]
pub(super) struct Runtime {
    pub buffs: Vec<Buff>,
    pub zones: Vec<Zone>,
    pub attacks: u8,
    pub melee: u8,
    pub transfers: Vec<Buff>,
    pub credits: Vec<(Option<u64>, f32)>,
    pub slow_attacker: Option<Entity>,
}
pub(super) struct Zone {
    pub entity: Entity,
    pub key: &'static str,
    pub pos: Vec3,
    pub radius: f32,
    pub until: f32,
    pub next: f32,
    pub power: f32,
}
impl Runtime {
    pub fn power(&self, now: f32, key: &str) -> f32 {
        self.buffs
            .iter()
            .find(|b| b.key == key && b.until > now)
            .map_or(0.0, |b| b.power)
    }
    pub fn grant(&mut self, buff: Buff) {
        if let Some(old) = self.buffs.iter_mut().find(|b| b.key == buff.key) {
            if buff.until <= old.until && buff.power < old.power {
                return;
            }
            if old.power > buff.power && old.until > buff.until {
                return;
            }
            *old = buff;
        } else {
            self.buffs.push(buff);
        }
    }
}
pub(super) fn grant(
    run: &mut Run,
    key: &'static str,
    power: f32,
    duration: f32,
    caster: Option<u64>,
) {
    run.support.buffs.retain(|b| b.until > run.elapsed);
    let old = run.support.buffs.iter().find(|b| b.key == key);
    // A weaker overlapping copy can refresh its duration without replacing its strength.
    let power = old.map_or(power, |b| b.power.max(power));
    let until = old.map_or(run.elapsed + duration, |b| {
        b.until.max(run.elapsed + duration)
    });
    run.support.grant(Buff {
        key,
        power,
        until,
        reserve: if matches!(key, "commanding_shout" | "frost_guardian") {
            power
        } else {
            0.0
        },
        caster,
        jumps: 5,
    });
}
pub(super) fn cast_ready(run: &Run, key: &str) -> bool {
    match key {
        "tame_beast" => !run.talent_runtime.minions.tame_active,
        "savage_roar" => run.support.melee >= 3,
        "slice_and_dice" => run.support.attacks >= 3,
        _ => true,
    }
}
pub(super) fn activated(run: &mut Run, key: &str, tags: Tags) {
    if tags.contains(Tags::ATTACK | Tags::DAMAGE) {
        run.support.attacks = run.support.attacks.saturating_add(1).min(3);
        if tags.contains(Tags::MELEE) {
            run.support.melee = run.support.melee.saturating_add(1).min(3);
        }
    }
    if key == "savage_roar" {
        run.support.melee = 0;
    }
    if key == "slice_and_dice" {
        run.support.attacks = 0;
    }
}
pub(super) fn absorb(run: &mut Run, mut damage: f32) -> f32 {
    for buff in run
        .support
        .buffs
        .iter_mut()
        .filter(|b| b.until > run.elapsed && b.reserve > 0.0)
    {
        let used = buff.reserve.min(damage.max(0.0));
        buff.reserve -= used;
        damage -= used;
        if used > 0.0 && buff.reserve <= 0.0 && buff.key == "frost_guardian" {
            run.support.slow_attacker = run.legendary.events.attacker;
        }
    }
    run.support
        .buffs
        .retain(|b| !matches!(b.key, "commanding_shout" | "frost_guardian") || b.reserve > 0.0);
    damage
}
pub(super) fn hurt(run: &mut Run, damage: f32) {
    if damage <= 0.0 || run.hp <= 0.0 {
        return;
    }
    let Some(index) = run
        .support
        .buffs
        .iter()
        .position(|b| b.key == "prayer_of_mending" && b.until > run.elapsed)
    else {
        return;
    };
    let mut buff = run.support.buffs.remove(index);
    let amount = run.recover(buff.power);
    run.support.credits.push((buff.caster, amount));
    buff.jumps = buff.jumps.saturating_sub(1);
    if buff.jumps > 0 {
        run.support.transfers.push(buff);
    }
}
#[derive(Resource, Default)]
pub(super) struct Debuffs {
    entries: std::collections::BTreeMap<Entity, Vec<Buff>>,
}
impl Debuffs {
    pub fn prune(&mut self, now: f32) {
        self.entries.retain(|_, list| {
            list.retain(|b| b.until > now);
            !list.is_empty()
        });
    }
    #[cfg(test)]
    pub fn apply(
        &mut self,
        target: Entity,
        key: &'static str,
        power: f32,
        now: f32,
        duration: f32,
    ) {
        self.apply_from(target, key, power, now, duration, None);
    }
    pub fn active(&self, now:f32) -> impl Iterator<Item=(Entity,&Buff)> {
        self.entries.iter().flat_map(move |(entity,buffs)| buffs.iter().filter(move |b| b.until>now).map(move |buff|(*entity,buff)))
    }
    pub fn apply_from(&mut self, target:Entity, key:&'static str, power:f32, now:f32, duration:f32, caster:Option<u64>) {
        let list = self.entries.entry(target).or_default();
        list.retain(|b| b.until > now);
        if let Some(buff) = list.iter_mut().find(|b| b.key == key) {
            if power >= buff.power || now + duration > buff.until { buff.caster = caster; }
            buff.power = buff.power.max(power);
            buff.until = buff.until.max(now + duration);
        } else {
            list.push(Buff {
                key,
                power,
                until: now + duration,
                reserve: 0.0,
                caster,
                jumps: 0,
            });
        }
    }
    pub fn multiplier(&self, target: Entity, now: f32, tags: Tags, bleed: bool) -> f32 {
        self.entries.get(&target).map_or(1.0, |list| {
            list.iter().filter(|b| b.until > now).fold(1.0, |m, b| {
                m * if (b.key == "hunters_mark" && tags.contains(Tags::ATTACK))
                    || (b.key == "curse_of_elements" && !tags.contains(Tags::PHYSICAL))
                    || (b.key == "mangle" && bleed)
                {
                    1.0 + b.power
                } else {
                    1.0
                }
            })
        })
    }
}
pub(super) fn buff_tags(def: &data::AbilityDef, mut tags: Tags) -> Tags {
    if def.kind != data::Kind::Buff {
        return tags;
    }
    tags = tags | Tags::BUFF | Tags::UTILITY | Tags::DURATION;
    if def.radius > 0.0 {
        tags = tags | Tags::AREA;
    }
    if def.totem.0 != 0 {
        tags = tags | Tags::TOTEM;
    }
    if matches!(def.key, "devotion_aura") {
        tags = tags | Tags::AURA;
    }
    if matches!(def.key, "prayer_of_mending" | "healing_stream_totem") {
        tags = tags | Tags::HEAL;
    }
    if matches!(
        def.key,
        "barkskin"
            | "commanding_shout"
            | "devotion_aura"
            | "frost_guardian"
            | "demon_armor"
            | "smoke_bomb"
            | "mark_of_the_wild"
    ) {
        tags = tags | Tags::GUARD;
    }
    tags
}
pub(super) fn tooltip(key: &str, level: u8, p: &Resolved) -> Option<String> {
    let description = description(key)?;
    let power = effective_magnitude(key, level, p.buff_effect);
    let value = power_label(key, level, p.buff_effect);
    Some(if power > 0.0 {
        format!(
            "{description} Bonus: {value}; duration {:.1} sec; radius {:.1} yd.",
            p.duration, p.radius
        )
    } else {
        description.into()
    })
}
pub(super) fn power_label(key: &str, level: u8, effect: f32) -> String {
    let power = effective_magnitude(key, level, effect);
    if magnitude(key, 1) < 1.0 {
        format!("{:.0}%", power * 100.0)
    } else {
        format!("{power:.0}")
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_support_buff_or_vulnerability_with_power_gets_stronger_at_each_rank() {
        for &(_, key, _, _) in super::SPELLS {
            if super::magnitude(key, 1) <= 0.0 {
                continue;
            }
            let mut previous = super::effective_magnitude(key, 1, 0.0);
            for rank in 2..=6 {
                let next = super::effective_magnitude(key, rank, 0.0);
                assert!(next > previous, "{key} rank {rank}");
                previous = next;
            }
        }
    }
    #[test]
    fn signature_buffs_gain_twenty_five_percent_more_power() {
        let silver = super::effective_magnitude("battle_shout", 5, 0.0);
        let gold = super::effective_magnitude("battle_shout", 6, 0.0);
        assert!((silver - 0.24).abs() < 0.001);
        assert!((gold - 0.40).abs() < 0.001);
        assert!((super::effective_magnitude("battle_shout", 6, 0.2) - 0.48).abs() < 0.001);
    }
    use super::*;
    use crate::survivors::{hero, spell_engine};

    #[test]
    fn all_twenty_three_skills_are_learnable_weighted_and_have_distinct_visual_ids() {
        let rewards: toml::Value =
            toml::from_str(crate::survivors::reward_config::DEFAULT_FILE).unwrap();
        assert_eq!(SPELLS.len(), 23);
        for &(class, key, _, _) in SPELLS {
            let class = data::CLASSES.iter().find(|c| c.id == class).unwrap();
            assert_eq!(class.pool.iter().filter(|k| **k == key).count(), 1, "{key}");
            let def = data::ability(key);
            assert_eq!(
                data::ABILITIES
                    .iter()
                    .filter(|d| d.spell == def.spell)
                    .count(),
                1,
                "{key}"
            );
            assert_eq!(rewards["spells"][key].as_integer(), Some(1));
            let tags = spell_engine::tags::for_spell(def);
            if def.kind == data::Kind::Buff {
                assert!(tags.contains(Tags::BUFF | Tags::UTILITY));
                assert!(!tags.contains(Tags::DAMAGE));
                let p = spell_engine::resolve(def, 8, &Run::default().stats(), &[]);
                assert_eq!(p.dmg, 0.0);
                assert_eq!(p.dot, 0.0);
                assert_eq!(p.recast, 0.0);
            }
        }
        for key in ["mangle", "rip", "ferocious_bite"] {
            assert!(spell_engine::tags::for_spell(data::ability(key))
                .contains(Tags::ATTACK | Tags::MELEE | Tags::PHYSICAL));
        }
        assert!(spell_engine::tags::for_spell(data::ability("wand_attack"))
            .contains(Tags::ATTACK | Tags::PROJECTILE | Tags::ARCANE));
    }

    #[test]
    fn identical_buffs_refresh_without_stacking_and_expire() {
        let mut run = Run::default();
        grant(&mut run, "battle_shout", 0.12, 8.0, None);
        grant(&mut run, "battle_shout", 0.12, 8.0, Some(42));
        assert_eq!(run.support.power(0.0, "battle_shout"), 0.12);
        run.elapsed = 4.0;
        grant(&mut run, "battle_shout", 0.10, 8.0, Some(43));
        assert_eq!(run.support.power(9.0, "battle_shout"), 0.12);
        assert_eq!(run.support.power(12.0, "battle_shout"), 0.0);
    }

    #[test]
    fn attack_and_spell_buffs_affect_their_own_damage_and_not_healing() {
        let mut run = Run::default();
        let resolve =
            |run: &Run, key| spell_engine::resolve(data::ability(key), 1, &run.stats(), &[]);
        let attack = resolve(&run, "claw");
        let spell = resolve(&run, "fireball");
        let heal = resolve(&run, "regrowth");
        grant(&mut run, "battle_shout", 0.12, 8.0, None);
        assert!((resolve(&run, "claw").dmg - attack.dmg * 1.12).abs() < 0.001);
        assert_eq!(resolve(&run, "fireball").dmg, spell.dmg);
        grant(&mut run, "arcane_intellect", 0.15, 8.0, None);
        assert!((resolve(&run, "fireball").dmg - spell.dmg * 1.15).abs() < 0.001);
        assert_eq!(resolve(&run, "regrowth").dmg, heal.dmg);
    }

    #[test]
    fn protection_absorbs_damage_then_breaks_and_prayer_consumes_one_charge() {
        let mut run = Run {
            hp: 100.0,
            ..default()
        };
        grant(&mut run, "frost_guardian", 20.0, 6.0, None);
        run.legendary.events.attacker = Some(Entity::from_raw_u32(5).unwrap());
        assert_eq!(hero::hurt_hero(&mut run, 10.0), 0.0);
        assert_eq!(hero::hurt_hero(&mut run, 15.0), 5.0);
        assert!(run.support.slow_attacker.is_some());
        assert_eq!(run.hp, 95.0);
        grant(&mut run, "prayer_of_mending", 20.0, 12.0, Some(42));
        hero::hurt_hero(&mut run, 10.0);
        assert_eq!(run.hp, 105.0);
        assert_eq!(run.support.transfers.len(), 1);
        assert_eq!(run.support.transfers[0].jumps, 4);
        assert_eq!(run.support.transfers[0].caster, Some(42));
        assert_eq!(run.support.power(0.0, "prayer_of_mending"), 0.0);
    }

    #[test]
    fn shared_vulnerabilities_filter_damage_types_and_never_stack_identical_sources() {
        let mut state = Debuffs::default();
        let target = Entity::from_raw_u32(5).unwrap();
        state.apply(target, "hunters_mark", 0.12, 0.0, 8.0);
        state.apply(target, "hunters_mark", 0.12, 0.0, 8.0);
        assert_eq!(
            state.multiplier(target, 0.0, Tags::ATTACK | Tags::PHYSICAL, false),
            1.12
        );
        assert_eq!(
            state.multiplier(target, 0.0, Tags::SPELL | Tags::FIRE, false),
            1.0
        );
        state.apply(target, "mangle", 0.2, 0.0, 8.0);
        assert_eq!(
            state.multiplier(target, 0.0, Tags::PHYSICAL | Tags::DOT, true),
            1.2
        );
        assert_eq!(
            state.multiplier(target, 8.0, Tags::PHYSICAL | Tags::DOT, true),
            1.0
        );
    }
}
