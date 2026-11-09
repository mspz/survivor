//! Tag-scoped equipment mechanics evaluated at impact, when target health is known.
use super::*;
#[derive(Clone, Copy, Default)]
pub struct HitModifiers {
    pub tags: Tags,
    pub leech: f32,
    pub boss: f32,
    pub full: f32,
    pub low: f32,
    pub penetration: f32,
    pub bleed: f32,
    pub ignite: f32,
    pub chill: f32,
    pub poison: f32,
    pub knockback: f32,
}
pub fn modifiers(skill: &str, entries: &[Modifier]) -> HitModifiers {
    let tags = super::super::data::ABILITIES
        .iter()
        .find(|d| d.key == skill)
        .or_else(|| super::super::procs::find(skill).map(|s| &s.def))
        .map_or(Tags(0), |d| tags::for_spell(d));
    modifiers_tagged(skill, tags, entries)
}
pub fn modifiers_tagged(skill: &str, tags: Tags, entries: &[Modifier]) -> HitModifiers {
    let mut out = HitModifiers {
        tags,
        ..Default::default()
    };
    let minion_damage = tags.contains(Tags::MINION | Tags::DAMAGE);
    for m in entries.iter().filter(|m| {
        m.filter.matches_hit(skill, tags)
            || (minion_damage
                && matches!(m.change, Change::LifeLeech(_))
                && m.filter.matches_hit(skill, tags | Tags::ATTACK))
    }) {
        let attack = tags::weapon_hit(tags);
        match m.change {
            Change::LifeLeech(v) if attack || minion_damage => out.leech += finite(v),
            Change::DamageAgainstBosses(v) => out.boss += finite(v),
            Change::DamageAgainstFullLife(v) => out.full += finite(v),
            Change::DamageAgainstLowLife(v) => out.low += finite(v),
            Change::ArmourPenetration(v) if tags.contains(Tags::PHYSICAL) => {
                out.penetration += finite(v)
            }
            Change::AilmentChance(v)
                if tags.contains(Tags::DAMAGE)
                    && !tags.intersects(Tags::GUARD | Tags::HEAL | Tags::UTILITY) =>
            {
                let chance = finite(v);
                if tags.contains(Tags::PHYSICAL) {
                    out.bleed += chance;
                }
                if tags.contains(Tags::FIRE) {
                    out.ignite += chance;
                }
                if tags.contains(Tags::COLD) {
                    out.chill += chance;
                }
                if tags.contains(Tags::NATURE) {
                    out.poison += chance;
                }
            }
            Change::BleedChance(v)
                if tags.contains(Tags::PHYSICAL | Tags::DAMAGE)
                    && !tags.intersects(Tags::GUARD | Tags::HEAL | Tags::UTILITY) =>
            {
                out.bleed += finite(v)
            }
            Change::IgniteChance(v) if tags.contains(Tags::FIRE) => out.ignite += finite(v),
            Change::ChillChance(v) if tags.contains(Tags::COLD) => out.chill += finite(v),
            Change::KnockbackChance(v) if tags.contains(Tags::PROJECTILE) => {
                out.knockback += finite(v)
            }
            _ => {}
        }
    }
    out
}
pub fn damage(raw: f32, hp: f32, max_hp: f32, boss: bool, armour: f32, mods: HitModifiers) -> f32 {
    let bonus = if boss { mods.boss } else { 0.0 }
        + if max_hp > 0.0 && hp >= max_hp {
            mods.full
        } else {
            0.0
        }
        + if max_hp > 0.0 && hp / max_hp <= 0.35 {
            mods.low
        } else {
            0.0
        };
    let damage = raw.max(0.0) * (1.0 + bonus).max(0.0);
    if mods.tags.contains(Tags::PHYSICAL) {
        let effective_armour = armour.max(0.0) * (1.0 - mods.penetration.clamp(0.0, 1.0));
        damage * (1.0 - super::super::items::armour::reduction(effective_armour))
    } else {
        damage
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ailment_chance_follows_damage_school_and_excludes_non_damage() {
        let entries = [Modifier {
            source: 1,
            filter: Filter::default(),
            change: Change::AilmentChance(0.15),
        }];
        for (school, expected) in [
            (Tags::PHYSICAL, [0.15, 0.0, 0.0, 0.0]),
            (Tags::FIRE, [0.0, 0.15, 0.0, 0.0]),
            (Tags::COLD, [0.0, 0.0, 0.15, 0.0]),
            (Tags::NATURE, [0.0, 0.0, 0.0, 0.15]),
            (Tags::HOLY, [0.0; 4]),
        ] {
            let hit = modifiers_tagged("test", school | Tags::DAMAGE, &entries);
            assert_eq!([hit.bleed, hit.ignite, hit.chill, hit.poison], expected);
            for excluded in [Tags::HEAL, Tags::GUARD, Tags::UTILITY] {
                let hit = modifiers_tagged("test", school | Tags::DAMAGE | excluded, &entries);
                assert_eq!([hit.bleed, hit.ignite, hit.chill, hit.poison], [0.0; 4]);
            }
        }
    }
    #[test]
    fn physical_spell_hits_can_bleed_and_leech_through_attack_proc_filters() {
        let entries = [Change::BleedChance(1.0), Change::LifeLeech(0.1)].map(|change| Modifier {
            source: 1,
            filter: Filter::requiring(Tags::ATTACK),
            change,
        });
        let physical_spell = modifiers("proc_bloodburst", &entries);
        assert!(physical_spell.tags.contains(Tags::SPELL | Tags::PHYSICAL));
        assert_eq!(physical_spell.bleed, 1.0);
        assert_eq!(physical_spell.leech, 0.1);
        assert_eq!(modifiers("fireball", &entries).bleed, 0.0);
        assert_eq!(modifiers("fireball", &entries).leech, 0.0);
        assert!(tags::weapon_hit(
            Tags::SPELL | Tags::HOLY | Tags::MELEE | Tags::DAMAGE
        ));
        assert!(!tags::weapon_hit(
            Tags::SPELL | Tags::PHYSICAL | Tags::GUARD
        ));
        assert!(!tags::weapon_hit(Tags::SPELL | Tags::PHYSICAL | Tags::HEAL));
    }
    #[test]
    fn impact_modifiers_obey_tags_health_conditions_and_physical_armour() {
        let changes = [
            Change::LifeLeech(0.1),
            Change::BleedChance(0.2),
            Change::IgniteChance(0.3),
            Change::ChillChance(0.4),
            Change::KnockbackChance(0.5),
            Change::ArmourPenetration(0.5),
            Change::DamageAgainstBosses(0.2),
            Change::DamageAgainstFullLife(0.3),
            Change::DamageAgainstLowLife(0.4),
        ];
        let entries: Vec<_> = changes
            .into_iter()
            .map(|change| Modifier {
                source: 1,
                filter: Filter::default(),
                change,
            })
            .collect();
        let melee = modifiers("heroic_strike", &entries);
        assert_eq!(melee.leech, 0.1);
        assert_eq!(melee.bleed, 0.2);
        assert_eq!(melee.knockback, 0.0);
        assert_eq!(melee.ignite, 0.0);
        let fire = modifiers("fireball", &entries);
        assert_eq!(fire.leech, 0.0);
        assert_eq!(fire.bleed, 0.0);
        assert_eq!(fire.ignite, 0.3);
        assert_eq!(fire.knockback, 0.5);
        let cold = modifiers("frostbolt", &entries);
        assert_eq!(cold.chill, 0.4);
        let expected = 150.0 * (1.0 - super::super::super::items::armour::reduction(50.0));
        assert!((damage(100.0, 100.0, 100.0, true, 100.0, melee) - expected).abs() < 0.001);
        assert!((damage(100.0, 20.0, 100.0, false, 0.0, melee) - 140.0).abs() < 0.001);
        assert!((damage(100.0, 100.0, 100.0, false, 1000.0, fire) - 130.0).abs() < 0.001);
    }
}
