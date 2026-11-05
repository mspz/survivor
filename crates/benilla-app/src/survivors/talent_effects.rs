//! Text and modifiers come from the same specialization design.
use super::{data::Stat, spell_engine::Change, talent_design as design, talents::Spec};
pub(super) fn character_stat(spec: &Spec, node: usize) -> bool {
    design::milestone(node).is_none() && super::minor_talents::minor(spec, node).stat.is_some()
}
pub(super) fn effects(spec: &Spec, node: usize, rank: u8) -> Vec<Change> {
    if rank == 0 {
        return vec![];
    }
    if let Some(i) = design::milestone(node) {
        return design::design(spec).milestones[i].changes.to_vec();
    }
    super::minor_talents::minor(spec, node)
        .change
        .map(|c| vec![c.scaled(u64::from(rank))])
        .unwrap_or_default()
}
pub(super) fn filter(spec: &Spec, node: usize, change: Change) -> super::spell_engine::Filter {
    use super::spell_engine::{Filter, Tags};
    if design::milestone(node).is_none() {
        return super::minor_talents::minor(spec, node).filter;
    }
    let tags = if matches!(change, Change::BleedChance(_)) {
        if design::design(spec).filter.contains(Tags::MELEE) {
            Tags::PHYSICAL | Tags::MELEE
        } else {
            Tags::PHYSICAL
        }
    } else {
        design::design(spec).filter
    };
    Filter::requiring(tags)
}
pub(super) fn name(spec: &Spec, node: usize) -> String {
    if let Some(i) = design::milestone(node) {
        return design::design(spec).milestones[i].name.into();
    }
    super::minor_talents::minor(spec, node).name.into()
}
pub(super) fn description(spec: &Spec, node: usize, rank: u8) -> String {
    if let Some(i) = design::milestone(node) {
        let mut description = design::design(spec).milestones[i].description.to_owned();
        if i == 0 {
            if let Some(replacement) = super::starter_spells::tooltip(spec) {
                description.push(' ');
                description.push_str(&replacement);
            }
        }
        if let Some(buff) = super::healer_buffs::talent_tooltip(spec, node) {
            description.push(' ');
            description.push_str(&buff);
        }
        return description;
    }
    // Unlearned talents preview rank one; learned talents show their current effect.
    let minor = super::minor_talents::minor(spec, node);
    if let Some(change) = minor.change {
        let scope = minor
            .filter
            .skill
            .map(|key| key.trim_start_matches("proc_").replace('_', " "))
            .unwrap_or_else(|| minor.filter.all.labels());
        let rank = u64::from(rank.max(1));
        // Every authored minor float modifier uses precise per-rank values in its tooltip.
        let value = match change {
            Change::IncreasedDamage(v)
            | Change::IncreasedEffect(v)
            | Change::MoreDamage(v)
            | Change::IncreasedDotDamage(v)
            | Change::IncreasedArea(v)
            | Change::IncreasedDuration(v)
            | Change::IncreasedCooldownRecovery(v)
            | Change::IncreasedAttackSpeed(v)
            | Change::CriticalChance(v)
            | Change::CriticalMultiplier(v)
            | Change::RecastChance(v)
            | Change::LifeLeech(v)
            | Change::DamageAgainstBosses(v)
            | Change::DamageAgainstFullLife(v)
            | Change::DamageAgainstLowLife(v)
            | Change::ArmourPenetration(v)
            | Change::ChillChance(v) => Some(v * rank as f32 * 100.0),
            _ => None,
        };
        let mut text = change.scaled(rank).description();
        if let Some(value) = value {
            text = text.replace(&format!("{value:.0}%"), &format!("{value:.1}%"));
        }
        return format!("{scope}: {text}.");
    }
    let (stat, value) = minor.stat.expect("minor talent has an effect");
    let v = value * f32::from(rank.max(1));
    match stat {
        Stat::Armor => format!("+{:.1}% armor.", v * 100.0),
        Stat::Regen => format!("Recover {v:.2} health per second."),
        Stat::MaxHp => format!("+{:.1}% maximum health.", v * 100.0),
        Stat::Speed => format!("+{:.1}% movement speed.", v * 100.0),
        Stat::Might => format!("+{:.1}% power.", v * 100.0),
        Stat::Haste => format!("+{:.1}% haste.", v * 100.0),
        Stat::Crit => format!("+{:.1}% critical strike chance.", v * 100.0),
        Stat::Area => format!("+{:.1}% area of effect.", v * 100.0),
        Stat::Growth => format!("+{:.1}% experience gained.", v * 100.0),
        Stat::Magnet => format!("+{:.1}% pickup range.", v * 100.0),
    }
}
