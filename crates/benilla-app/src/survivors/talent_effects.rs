//! Text and modifiers come from the same specialization design.
use super::{data::Stat, spell_engine::Change, talent_design as design, talents::Spec};
pub(super) fn character_stat(_spec: &Spec, node: usize) -> bool {
    design::milestone(node).is_none()
}
pub(super) fn effects(spec: &Spec, node: usize, rank: u8) -> Vec<Change> {
    if rank == 0 {
        return vec![];
    }
    if let Some(i) = design::milestone(node) {
        return design::design(spec).milestones[i].changes.to_vec();
    }
    vec![]
}
pub(super) fn name(spec: &Spec, node: usize) -> String {
    if let Some(i) = design::milestone(node) {
        return design::design(spec).milestones[i].name.into();
    }
    match design::passive(spec, node).0 {
        Stat::Might => "Power",
        Stat::Haste => "Haste",
        Stat::Crit => "Precision",
        Stat::Area => "Reach",
        Stat::Armor => "Resilience",
        Stat::Regen => "Recovery",
        Stat::Speed => "Mobility",
        Stat::MaxHp => "Vitality",
        _ => "Training",
    }
    .into()
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
        return description;
    }
    // Unlearned talents preview rank one; learned talents show their current effect.
    let (stat, value) = design::passive(spec, node);
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
