//! Presentation of learned proc spells without adding them to the player's cast/reward slots.
use super::*;
use crate::survivors::{data, talent_design, talent_runtime::active};

pub(in crate::survivors) struct ViewSpell {
    pub key: &'static str,
    pub name: String,
    pub spell: u32,
    pub description: Option<&'static str>,
    pub lines: Vec<String>,
}

pub(in crate::survivors) fn views(run: &Run) -> Vec<ViewSpell> {
    let Some(class) = data::CLASSES.iter().position(|c| c.id == run.class.id) else {
        return vec![];
    };
    super::available(run)
        .into_iter()
        .map(|def| {
            let source = (0..3)
                .flat_map(|tree| {
                    [8, 12, 18]
                        .into_iter()
                        .enumerate()
                        .filter_map(move |(i, node)| {
                            (run.talent_build[tree][node] > 0)
                                .then_some(&talent_design::DESIGNS[class * 3 + tree].milestones[i])
                        })
                })
                .filter(|m| {
                    super::key(run, m.mechanic) == def.key
                        || def.key == "proc_pet_bite" && m.mechanic == M::Pet
                        || def.key == "proc_totem_heal" && m.mechanic == M::SpiritTotem
                        || def.key == "proc_shadow_leech" && m.mechanic == M::ShadowDrain
                        || def.key == "proc_plagueburst"
                            && run.class.id == 5
                            && m.mechanic == M::PlagueSpread
                })
                .last();
            let name = if def.key == "proc_holy_heal" {
                "Talent Healing".to_owned()
            } else {
                source.map_or_else(
                    || def.key.trim_start_matches("proc_").replace('_', " "),
                    |m| m.name.to_owned(),
                )
            };
            let mut lines = vec![
                "Talent proc".to_owned(),
                super::find(def.key).unwrap().tags.labels(),
            ];
            if let Some(source) = source {
                lines.push(source.description.to_owned());
            }
            // Variable effects use a labelled reference value, never a fictitious fixed hit.
            let (damage, dot, duration, radius, count, basis): (f32, f32, f32, f32, usize, &str) =
                match def.key {
                    "proc_bleed" => (0., 20., 4., 0., 1, "Per 100 triggering hit damage"),
                    "proc_ignite" => (0., 25., 3., 0., 1, "Per 100 triggering hit damage"),
                    "proc_poison" | "proc_plague" | "proc_lunar_flare" => {
                        (0., 18., 6., 0., 1, "Per 100 triggering hit damage")
                    }
                    "proc_bloodburst" | "proc_poisonburst" | "proc_fireburst"
                    | "proc_plagueburst" | "proc_frostnova" => (
                        300.,
                        0.,
                        if def.key == "proc_frostnova" { 2. } else { 0. },
                        6.,
                        64,
                        "At 1,000 enemy maximum health (base cap)",
                    ),
                    "proc_splash" => (40., 0., 0., 4., 64, "Per 100 triggering hit damage"),
                    "proc_flurry" => (80., 0., 0., 6., 64, "Per 100 triggering hit damage"),
                    "proc_execution" => (200., 0., 0., 0., 1, "Per 100 triggering hit damage"),
                    "proc_windfury" => (
                        50.,
                        0.,
                        0.,
                        0.,
                        1,
                        "Per strike, per 100 triggering hit damage",
                    ),
                    "proc_seal" | "proc_flametongue" | "proc_seal_storm" => (
                        25.,
                        0.,
                        0.,
                        if def.key == "proc_seal_storm" { 6. } else { 0. },
                        1,
                        "Per 100 triggering hit damage",
                    ),
                    "proc_stormstrike" => (100., 0., 0., 8., 4, "Per 100 triggering hit damage"),
                    "proc_arcane_blast" | "proc_starfall" => {
                        (150., 0., 0., 7., 64, "Per 100 triggering hit damage")
                    }
                    "proc_cataclysm" => (200., 0., 0., 8., 64, "Per 100 triggering hit damage"),
                    "proc_thunder_fortress" | "proc_sanctuary" => (45., 0., 0., 6., 64, ""),
                    "proc_shieldburst" => (
                        if active(run, M::ShieldNova) {
                            120.
                        } else {
                            60.
                        },
                        0.,
                        0.,
                        if active(run, M::ShieldNova) { 10. } else { 6. },
                        64,
                        "Per 100 original shield absorption",
                    ),
                    "proc_holy_burst" => (40., 0., 0., 6., 64, ""),
                    "proc_holy_judgement" => {
                        (50., 0., 0., 6., 2, "Per 100 triggering Holy hit damage")
                    }
                    "proc_pet_bite" => (
                        18.,
                        if active(run, M::Pack) { 3.6 } else { 0. },
                        if active(run, M::Pack) { 4. } else { 0. },
                        0.,
                        1,
                        "",
                    ),
                    "proc_demon_bolt" => {
                        let base = if active(run, M::DemonLord) { 36. } else { 18. };
                        (
                            base,
                            if active(run, M::Demons) {
                                base * 0.2
                            } else {
                                0.
                            },
                            if active(run, M::Demons) { 4. } else { 0. },
                            if active(run, M::DemonLord) { 4. } else { 0. },
                            1,
                            "",
                        )
                    }
                    "proc_totem_bolt" | "proc_spirit_bolt" => (
                        24.,
                        0.,
                        0.,
                        0.,
                        if def.key == "proc_spirit_bolt" { 3 } else { 1 },
                        "",
                    ),
                    "proc_frostbite" => (0., 0., 3., 0., 1, ""),
                    "proc_taunt" => (0., 0., 3., 12., 1, ""),
                    "proc_poison_spread" | "proc_ignite_spread" | "proc_plague_spread" => {
                        (0., 0., 3., 6., 64, "Spreads existing damage over time")
                    }
                    "proc_shadow_leech" => (12., 0., 0., 0., 1, "Per 100 Shadow damage dealt"),
                    "proc_totem_heal" => (
                        if active(run, M::SpiritTotem) { 4. } else { 2. },
                        0.,
                        0.,
                        12.,
                        1,
                        "Healing per second, per 100 ally maximum health",
                    ),
                    "proc_bloom" | "proc_grove" => (
                        if active(run, M::Grove) { 3. } else { 1.5 },
                        0.,
                        0.,
                        12.,
                        1,
                        "Healing per second, per 100 ally maximum health",
                    ),
                    "proc_ally_buff" => (0., 0., 0., 12., 1, ""),
                    "proc_holy_heal" => (0., 0., 0., 12., 1, ""),
                    _ => (0., 0., 0., 0., 1, ""),
                };
            if !basis.is_empty() {
                lines.push(basis.to_owned());
            }
            let p = super::resolve(run, def.key, damage, dot, duration, radius, count);
            if let Some(buff) = crate::survivors::healer_buffs::aura_tooltip(run, def.key) {
                lines.push(buff);
            }
            if def.key == "proc_holy_heal" {
                for (enabled, amount, label) in [
                    (
                        active(run, M::HolyHeal) || active(run, M::HolyJudgement),
                        if active(run, M::HolyHeal) { 4. } else { 0. }
                            + if active(run, M::HolyJudgement) {
                                8.
                            } else {
                                0.
                            },
                        "Healing per 100 Holy hit damage",
                    ),
                    (
                        active(run, M::ShieldHeal),
                        10.,
                        "Healing per 100 maximum health, on shield explosion",
                    ),
                    (
                        active(run, M::SealStorm),
                        5.,
                        "Healing per 100 triggering hit damage",
                    ),
                    (
                        run.class.id == 2 && active(run, M::Block),
                        3.,
                        "Healing per 100 maximum health, on block",
                    ),
                    (
                        run.class.id == 2 && active(run, M::Bulwark),
                        2.,
                        "Healing per 100 maximum health, on Sanctuary",
                    ),
                    (
                        active(run, M::HolyBurst),
                        12.,
                        "Healing pulse per 100 ally maximum health",
                    ),
                ] {
                    if enabled {
                        lines.push(format!(
                            "{label}: {:.1}",
                            super::resolve(run, def.key, amount, 0., 0., 0., 1).dmg
                        ));
                    }
                }
            }
            if def.key == "proc_grove" {
                lines.push(format!(
                    "Healing pulse per 100 ally maximum health: {:.1}",
                    super::resolve(run, def.key, 12., 0., 0., 0., 1).dmg
                ));
            }
            if p.dmg > 0. {
                let label = if def.key == "proc_ally_buff" {
                    "Bonus"
                } else if p.tags.contains(Tags::HEAL) {
                    "Healing"
                } else {
                    "Damage"
                };
                lines.push(if def.key == "proc_ally_buff" {
                    format!("{label}: {:.1}%", p.dmg * 100.)
                } else {
                    format!("{label}: {:.1}", p.dmg)
                });
            }
            if p.dot > 0. {
                lines.push(format!("Damage over time: {:.1}/s", p.dot));
            }
            if p.duration > 0. {
                lines.push(format!(
                    "{}: {:.1} s",
                    if def.key.ends_with("_spread") {
                        "Minimum duration"
                    } else {
                        "Duration"
                    },
                    p.duration
                ));
            }
            if p.radius > 0. {
                lines.push(format!("Radius: {:.1} yd", p.radius));
            }
            if p.tags.contains(Tags::DAMAGE) && damage > 0. {
                lines.push(format!("Critical chance: {:.1}%", p.crit * 100.));
                lines.push(format!("Critical damage: {:.0}%", p.crit_multiplier * 100.));
            }
            if matches!(def.key, "proc_holy_burst" | "proc_taunt" | "proc_grove") {
                lines.push(format!(
                    "Interval: {:.2} s",
                    super::interval(run, def.key, 6.)
                ));
            } else if matches!(
                def.key,
                "proc_pet_bite" | "proc_demon_bolt" | "proc_totem_bolt" | "proc_spirit_bolt"
            ) {
                lines.push(format!("Interval: {:.2} s", p.cooldown));
            }
            if p.count > 1 {
                lines.push(format!("Targets: {}", p.count));
            }
            let chains = if def.kind == Kind::Chain {
                p.count.saturating_sub(1)
            } else {
                p.chains
            };
            if chains > 0 {
                lines.push(format!("Chains: {chains}"));
            }
            if p.forks > 0 {
                lines.push(format!("Forks: {}", p.forks));
            }
            if p.recast > 0. {
                lines.push(format!("Repeat chance: {:.0}%", p.recast * 100.));
            }
            ViewSpell {
                description: source.map(|s| s.description),
                key: def.key,
                name,
                spell: def.spell,
                lines,
            }
        })
        .collect()
}
