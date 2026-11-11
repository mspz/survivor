//! Local target selection and mirrored Classic unit frames. Status is replicated by the host.
use super::{
    characters, data, enemies::Enemy, hero::Hero, hud, multiplayer, talents, units::Marks, Phase,
    Run, RunEntity, Selection,
};
use crate::net::ObjectStore;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
mod debuff_tooltip;
mod party;
mod picking;
mod portraits;
mod ui;

#[derive(Component)]
pub(in crate::survivors) struct PartyPlayer;

#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub(in crate::survivors) struct Status {
    pub name: String,
    pub health: f32,
    pub maximum: f32,
    pub level: u32,
    pub friendly: bool,
    pub debuffs: Vec<(u32, f32)>,
    #[serde(default)]
    pub debuff_stacks: Vec<(u32, u32)>,
}
#[derive(Component)]
pub(in crate::survivors) struct Label(pub String);
#[derive(Resource, Default)]
pub(in crate::survivors) struct Target {
    pub entity: Option<Entity>,
    press: Option<Vec2>,
}
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Target>()
        .add_plugins(portraits::plugin)
        .add_systems(Startup, ui::setup.after(hud::setup_hud))
        .add_systems(
            Update,
            party::refresh.after(sync_status).after(hud::show_panels),
        )
        .add_systems(
            Update,
            sync_status
                .after(super::hero::hero_vitals)
                .run_if(multiplayer::authoritative),
        )
        .add_systems(
            Update,
            picking::click.before(hud::keys).after(characters::input),
        )
        .add_systems(
            Update,
            ui::refresh
                .after(sync_status)
                .after(picking::click)
                .after(hud::show_panels),
        )
        .add_systems(Update, debuff_tooltip::show.after(ui::refresh));
}

fn add_debuff(debuffs: &mut Vec<(u32, f32)>, spell: u32, seconds: f32) {
    if seconds <= 0.0 || !seconds.is_finite() {
        return;
    }
    if let Some((_, left)) = debuffs.iter_mut().find(|(id, _)| *id == spell) {
        *left = left.max(seconds);
    } else {
        debuffs.push((spell, seconds));
    }
}

#[allow(clippy::type_complexity)]
pub(in crate::survivors) fn sync_status(
    mut commands: Commands,
    run: Res<Run>,
    roster: Option<Res<characters::Roster>>,
    account: Option<Res<talents::Account>>,
    players: Option<Res<multiplayer::Companions>>,
    units: Query<
        (
            Entity,
            &ObjectStore,
            Option<&Hero>,
            Option<&Enemy>,
            Option<&multiplayer::RemoteOwner>,
            Option<&Label>,
            Option<&Marks>,
            Option<&super::talent_runtime::Taunted>,
            Option<&super::monster_specials::Runtime>,
        ),
        With<RunEntity>,
    >,
) {
    for (entity, store, hero, enemy, owner, label, marks, taunt, special) in &units {
        let player = if hero.is_some() {
            Some(&*run)
        } else {
            owner.and_then(|id| {
                players
                    .as_ref()?
                    .0
                    .get(&id.0)
                    .filter(|p| p.entity == entity)
                    .map(|p| &p.run)
            })
        };
        let mut status = if let Some(enemy) = enemy {
            Status {
                name: enemy.def.name.into(),
                health: enemy.hp.max(0.0),
                maximum: enemy.max_hp,
                level: enemy.def.level,
                friendly: false,
                debuffs: vec![],
                debuff_stacks: vec![],
            }
        } else if let Some(player) = player {
            Status {
                name: label
                    .map(|l| l.0.clone())
                    .or_else(|| {
                        hero.and_then(|_| roster.as_ref()?.current().map(|c| c.name.clone()))
                    })
                    .unwrap_or_else(|| player.class.name.into()),
                health: player.hp.max(0.0),
                maximum: player.stats().max_hp,
                level: if hero.is_some() {
                    account
                        .as_ref()
                        .map_or(player.level, |a| talents::level(a.xp))
                } else {
                    store.0.unit_level().unwrap_or(player.level)
                },
                friendly: true,
                debuffs: vec![],
                debuff_stacks: vec![],
            }
        } else if store.0.unit_faction_template() == Some(super::units::FACTION_FRIENDLY) {
            Status {
                name: label.map_or("Companion".into(), |l| l.0.clone()),
                health: store.0.unit_health().unwrap_or(100) as f32,
                maximum: store.0.unit_max_health().unwrap_or(100) as f32,
                level: store.0.unit_level().unwrap_or(1),
                friendly: true,
                debuffs: vec![],
                debuff_stacks: vec![],
            }
        } else {
            continue;
        };
        if let Some(marks) = marks {
            for (spell, left) in marks.active_debuffs() {
                add_debuff(&mut status.debuffs, spell, left);
            }
        }
        if let Some(special) = special {
            if special.shield > 0.0 { add_debuff(&mut status.debuffs, 17, special.shield_until - run.elapsed); }
            if special.death_stacks > 0 && special.death_until > run.elapsed {
                add_debuff(&mut status.debuffs, 6673, special.death_until - run.elapsed);
                status.debuff_stacks.push((6673, special.death_stacks));
            }
        }
        if let Some(player) = player {
            for (spell, left) in player.monster_ailments.active() { add_debuff(&mut status.debuffs, spell, left); }
        }
        if let Some(enemy) = enemy {
            for dot in &enemy.dots {
                if matches!(
                    dot.skill,
                    "proc_bleed" | "proc_poison" | "deadly_poison" | "proc_ignite"
                ) && dot.left > 0.0
                    && dot.left.is_finite()
                {
                    let spell = data::ability(dot.skill).spell;
                    if let Some((_, count)) =
                        status.debuff_stacks.iter_mut().find(|(id, _)| *id == spell)
                    {
                        *count += 1;
                    } else {
                        status.debuff_stacks.push((spell, 1));
                    }
                }
                add_debuff(
                    &mut status.debuffs,
                    data::ability(dot.skill).spell,
                    dot.left,
                );
            }
            for (spell, left) in [
                (116, enemy.slow_t),
                (339, enemy.root_t),
                (408, enemy.stun_t),
                (5782, enemy.fear_t),
                (702, enemy.weaken_t),
            ] {
                add_debuff(&mut status.debuffs, spell, left);
            }
        }
        if let Some(taunt) = taunt {
            add_debuff(&mut status.debuffs, 355, taunt.until - run.elapsed);
        }
        commands.entity(entity).insert(status);
        if hero.is_some() || owner.is_some() {
            commands.entity(entity).insert(PartyPlayer);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    #[test]
    fn enemy_status_keeps_real_health_and_only_active_debuffs() {
        let mut world = World::new();
        world.insert_resource(Run::default());
        world
            .run_system_once(|mut commands: Commands, mut run: ResMut<Run>| {
                super::super::enemies::spawn_enemy(
                    &mut commands,
                    &mut run,
                    &data::roster(data::CAMPAIGN[0]).enemies[0],
                    Vec3::ZERO,
                    Vec3::ZERO,
                    false,
                );
            })
            .unwrap();
        let entity = world
            .query_filtered::<Entity, With<Enemy>>()
            .single(&world)
            .unwrap();
        world.get_mut::<Enemy>(entity).unwrap().hp = 17.0;
        let name=world.get::<Enemy>(entity).unwrap().def.name;
        world.entity_mut(entity).insert((Label(format!("{name} - Brutal / Echo")),super::super::enemies::Rank::Brutal.with(super::super::enemies::Rank::Echo)));
        world.get_mut::<Enemy>(entity).unwrap().slow_t = 2.0;
        world.get_mut::<Marks>(entity).unwrap().add(589, 4.0);
        world.run_system_once(sync_status).unwrap();
        let status = world.get::<Status>(entity).unwrap();
        assert_eq!(status.name,name,"monster mods belong in the buff row, not in the frame caption");
        assert_eq!(status.health, 17.0);
        assert_eq!(
            status.maximum,
            world
                .resource::<Run>()
                .expeditions
                .stats(data::CAMPAIGN[0], false)
                .0
        );
        assert!(!status.friendly);
        assert_eq!(status.debuffs, vec![(589, 4.0), (116, 2.0)]);
        for (skill, count) in [("proc_bleed", 3), ("proc_poison", 2), ("proc_ignite", 1)] {
            for left in std::iter::repeat_n(4.0, count).chain([0.0, -1.0]) {
                world
                    .get_mut::<Enemy>(entity)
                    .unwrap()
                    .dots
                    .push(super::super::enemies::Dot {
                        owner: None,
                        skill,
                        meter_source: None,
                        tags: None,
                        item_source: None,
                        origin: super::super::items::legendary::events::Origin::Skill,
                        meta: Default::default(),
                        dps: 5.0,
                        left,
                        tick: 1.0,
                    });
            }
        }
        world.run_system_once(sync_status).unwrap();
        assert_eq!(
            world.get::<Status>(entity).unwrap().debuff_stacks,
            vec![(1943, 3), (2818, 2), (133, 1)]
        );
        world.get_mut::<Enemy>(entity).unwrap().dots.clear();
        world.get_mut::<Enemy>(entity).unwrap().slow_t = 0.0;
        world.get_mut::<Marks>(entity).unwrap().tick(5.0);
        world.run_system_once(sync_status).unwrap();
        assert!(world.get::<Status>(entity).unwrap().debuffs.is_empty());
        assert!(world
            .get::<Status>(entity)
            .unwrap()
            .debuff_stacks
            .is_empty());
    }
}
