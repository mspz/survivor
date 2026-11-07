//! Additional run skills, with owner-local reactions and finite periodic explosions.
use super::{
    abilities::Hit,
    data,
    enemies::Enemy,
    spell_engine::{self, Tags},
    Run,
};
use bevy::prelude::*;

#[derive(Default)]
pub(super) struct Runtime {
    pub revenge_until: f32,
    pub dodge_until: f32,
    pub holy_until: f32,
    pub ice_until: f32,
    pub blooms: Vec<Bloom>,
    bombs: Vec<Bomb>,
    pending: Vec<Hit>,
}
pub(super) struct Bloom {
    pub entity: Entity,
    pub center: Vec3,
    pub radius: f32,
    pub amount: f32,
    pub next: f32,
    pub left: u32,
}
struct Bomb {
    target: Entity,
    skill: &'static str,
    pos: Vec3,
    expires: f32,
    radius: f32,
    damage: f32,
    threshold: f32,
    taken: f32,
    tags: Tags,
}

#[cfg(test)]
pub(super) const ADDITIONS: &[(u8, &str, &str)] = &[
    (1, "bloodthirst", "revenge"),
    (2, "holy_shield", "hammer_of_the_righteous"),
    (3, "frost_trap", "mongoose_bite"),
    (4, "fan_of_knives", "garrote"),
    (5, "devouring_plague", "penance"),
    (7, "earthbind_totem", "lava_lash"),
    (8, "living_bomb", "ice_armor"),
    (9, "seed_of_corruption", "soul_burn"),
    (11, "rake", "efflorescence"),
];
pub(super) fn name(key: &str) -> Option<&'static str> {
    if let Some(name) = super::support_spells::name(key) {
        return Some(name);
    }
    Some(match key {
        "bloodthirst" => "Bloodthirst",
        "revenge" => "Revenge",
        "holy_shield" => "Holy Shield",
        "hammer_of_the_righteous" => "Hammer of the Righteous",
        "frost_trap" => "Frost Trap",
        "mongoose_bite" => "Mongoose Bite",
        "fan_of_knives" => "Fan of Knives",
        "garrote" => "Garrote",
        "devouring_plague" => "Devouring Plague",
        "penance" => "Penance",
        "earthbind_totem" => "Earthbind Totem",
        "totemic_recall" => "Totemic Recall",
        "lava_lash" => "Lava Lash",
        "living_bomb" => "Living Bomb",
        "ice_armor" => "Ice Armor",
        "seed_of_corruption" => "Seed of Corruption",
        "soul_burn" => "Soul Burn",
        "rake" => "Rake",
        "efflorescence" => "Efflorescence",
        _ => return None,
    })
}
pub(super) fn description(key: &str) -> Option<&'static str> {
    if let Some(description) = super::support_spells::description(key) {
        return Some(description);
    }
    Some(match key {
        "bloodthirst" => "A powerful melee strike that heals you for 15% of damage dealt.",
        "revenge" => "A Physical cone, available for 5 sec after you block or dodge. Learning this skill grants 5% dodge chance.",
        "holy_shield" => "For 6 sec, gain 25 percentage points of block chance. Each block retaliates with Holy damage.",
        "hammer_of_the_righteous" => "A melee Holy strike with a small explosion around its victim.",
        "frost_trap" => "Place a frost field at your feet, slowing enemies inside by 50%.",
        "mongoose_bite" => "A melee counterattack. Deals 75% more damage within 4 sec of dodging. Learning this skill grants 5% dodge chance.",
        "fan_of_knives" => "Strike every enemy around you with Physical attack damage.",
        "garrote" => "A melee strike that applies a bleed. Deals 50% more direct and periodic damage to full-health targets.",
        "devouring_plague" => "A Shadow disease that heals you for 15% of its periodic damage dealt.",
        "penance" => "Three Holy strikes, 0.3 sec apart. Instead heals the most injured nearby ally when below 35% health.",
        "earthbind_totem" => "Summon a totem that slows nearby enemies by 50% without dealing damage.",
        "totemic_recall" => "Bring all your active totems around you, preserving their power and remaining duration. Casts automatically when a totem is more than 4 yd away.",
        "lava_lash" => "A Fire melee strike dealing 50% more damage to victims of your Flame Shock.",
        "living_bomb" => "A Fire DoT that explodes for four seconds of its DPS when it expires or its victim dies. The explosion cannot apply bombs.",
        "ice_armor" => "Temporarily take 15% less damage. Melee attackers are slowed by 40% for 2 sec.",
        "seed_of_corruption" => "A Shadow DoT that explodes for four seconds of its DPS when its victim takes five seconds of its DPS in your damage, or dies. Explosions cannot plant seeds or charge other seeds.",
        "soul_burn" => "A Shadow strike that consumes half the remaining duration of your skill DoTs, immediately releasing the consumed damage on the victim.",
        "rake" => "A melee Physical strike followed by a bleed.",
        "efflorescence" => "Place a healing circle at your feet for 6 sec. Each second heals every living ally inside; the field stays where it was placed.",
        _ => return None,
    })
}

fn owned(run: &Run, key: &str) -> Option<u8> {
    (!run.disabled_spells.contains(key))
        .then(|| {
            run.abilities
                .iter()
                .find(|o| o.def.key == key)
                .map(|o| o.level)
        })
        .flatten()
}
pub(super) fn block_bonus(run: &Run) -> f32 {
    if run.extra_spells.holy_until > run.elapsed {
        0.25
    } else {
        0.0
    }
}
pub(super) fn dodge_chance(run: &Run) -> f32 {
    if owned(run, "mongoose_bite").is_some() || owned(run, "revenge").is_some() {
        0.05
    } else {
        0.0
    }
}
pub(super) fn reacted(run: &mut Run, dodged: bool) {
    run.extra_spells.revenge_until = run.elapsed + 5.0;
    if dodged {
        run.extra_spells.dodge_until = run.elapsed + 4.0;
    }
    if !dodged && run.extra_spells.holy_until > run.elapsed {
        if let (Some(target), Some(level)) =
            (run.legendary.events.attacker, owned(run, "holy_shield"))
        {
            let p = run.resolve_spell(data::ability("holy_shield"), level);
            let mut hit = Hit::direct("holy_shield", target, p.dmg, false);
            hit.owner = run.owner;
            hit.tags = Some(p.tags);
            hit.triggered = true;
            hit.proc_admitted = true;
            run.extra_spells.pending.push(hit);
        }
    }
}
pub(super) fn before_hit(run: &mut Run, h: &mut Hit, en: &mut Enemy, pos: Vec3) {
    if !h.can_proc() {
        return;
    }
    match h.skill {
        "ferocious_bite" => {
            let bleeds: std::collections::BTreeSet<_> = en
                .dots
                .iter()
                .filter(|d| {
                    d.owner == h.owner
                        && d.left > 0.0
                        && super::items::legendary::periodic::family(d)
                            == super::items::legendary::periodic::Family::Bleed
                })
                .map(|d| d.skill)
                .collect();
            h.damage *= 1.0 + 0.2 * bleeds.len().min(3) as f32;
        }
        "garrote" if en.hp >= en.max_hp => {
            h.damage *= 1.5;
            h.dot *= 1.5;
        }
        "lava_lash"
            if en
                .dots
                .iter()
                .any(|d| d.owner == h.owner && d.skill == "flame_shock" && d.left > 0.0) =>
        {
            h.damage *= 1.5
        }
        "soul_burn" => {
            for dot in en.dots.iter_mut().filter(|d| {
                d.owner == h.owner && d.origin == super::items::legendary::events::Origin::Skill
            }) {
                let before = super::items::legendary::periodic::budget(dot);
                dot.left *= 0.5;
                h.damage += (before - super::items::legendary::periodic::budget(dot)).max(0.0);
                if let Some(bomb) = run
                    .extra_spells
                    .bombs
                    .iter_mut()
                    .find(|b| b.target == h.target && b.skill == dot.skill)
                {
                    bomb.expires = run.elapsed + dot.left;
                }
            }
        }
        _ => {}
    }
    if matches!(h.skill, "living_bomb" | "seed_of_corruption") && h.dot > 0.0 {
        run.extra_spells
            .bombs
            .retain(|b| b.target != h.target || b.skill != h.skill);
        let level = owned(run, h.skill).unwrap_or(1);
        let p = run.resolve_spell(data::ability(h.skill), level);
        run.extra_spells.bombs.push(Bomb {
            target: h.target,
            skill: h.skill,
            pos,
            expires: run.elapsed + h.dot_dur,
            radius: p.radius,
            damage: h.dot * 4.0,
            threshold: h.dot * 5.0,
            taken: 0.0,
            tags: p.tags,
        });
    }
}
pub(super) fn damage(run: &mut Run, target: Entity, pos: Vec3, amount: f32) {
    for bomb in run
        .extra_spells
        .bombs
        .iter_mut()
        .filter(|b| b.target == target)
    {
        bomb.pos = pos;
        bomb.taken += amount.max(0.0);
    }
}
pub(super) fn tick(
    run: &mut Run,
    foes: &[spell_engine::targeting::Target],
    hits: &mut MessageWriter<Hit>,
) {
    if run.dead_t.is_some() || run.pending_levels > 0 {
        return;
    }
    for h in run.extra_spells.pending.drain(..) {
        hits.write(h);
    }
    let mut keep = Vec::new();
    for mut bomb in std::mem::take(&mut run.extra_spells.bombs) {
        let target = foes.iter().find(|f| f.e == bomb.target);
        if let Some(target) = target {
            bomb.pos = target.pos;
        }
        let expired = run.elapsed >= bomb.expires;
        let explode = target.is_none()
            || if bomb.skill == "living_bomb" {
                expired
            } else {
                bomb.taken >= bomb.threshold
            };
        if explode {
            for target in spell_engine::targeting::around(foes, bomb.pos, bomb.radius) {
                let mut h = Hit::direct(bomb.skill, target.e, bomb.damage, false);
                h.owner = run.owner;
                h.tags = Some(Tags(
                    (bomb.tags.0 & !(Tags::DOT.0 | Tags::DURATION.0)) | Tags::AREA.0,
                ));
                h.triggered = true;
                h.proc_resolved = true;
                h.proc_admitted = true;
                hits.write(h);
            }
        } else if !expired {
            keep.push(bomb);
        }
    }
    run.extra_spells.bombs = keep;
}
