//! Owner-independent stat effects; the same damage packets can be used by a future player owner.
use super::{Run, enemies::Enemy, monster_mods::Monster, spell_engine::Tags};
use bevy::prelude::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct Bonuses(pub Monster);
impl Default for Bonuses {
    fn default() -> Self {
        Self(Monster::Normal)
    }
}
impl Bonuses {
    pub fn maximum_life(self) -> f32 {
        if self.has(Monster::Colossus) {
            super::monster_mod_effects::get().elites.health_multiplier
        } else {
            1.0
        }
    }
    pub fn size(self) -> f32 {
        if self.has(Monster::Colossus) {
            super::monster_mod_effects::get().elites.size_multiplier
        } else {
            1.0
        }
    }
    pub fn movement(self) -> f32 {
        if self.has(Monster::Sprinter) {
            super::monster_mod_effects::get().sprinters.speed_multiplier
        } else {
            1.0
        }
    }
    fn has(self, modifier: Monster) -> bool {
        self.0.has(modifier)
    }
    pub fn action_speed(self) -> f32 {
        if self.has(Monster::Frenzied) {
            super::monster_mod_effects::get().stats.action_speed
        } else {
            1.0
        }
    }
    pub fn range(self) -> f32 {
        if self.has(Monster::LongRange) {
            super::monster_mod_effects::get().stats.range
        } else {
            1.0
        }
    }
    pub fn area_radius(self) -> f32 {
        if self.has(Monster::Gigantic) {
            super::monster_mod_effects::get().stats.area.sqrt()
        } else {
            1.0
        }
    }
    pub fn regeneration(self) -> f32 {
        if self.has(Monster::Regenerating) {
            super::monster_mod_effects::get().stats.regeneration
        } else {
            0.0
        }
    }
    pub fn leech(self) -> f32 {
        if self.has(Monster::Vampiric) {
            super::monster_mod_effects::get().stats.life_leech
        } else {
            0.0
        }
    }
    pub fn control_duration(self) -> f32 {
        if self.has(Monster::Unstoppable) {
            super::monster_mod_effects::get().stats.control_duration
        } else {
            1.0
        }
    }
    pub fn slow(self, slow: f32) -> f32 {
        if self.has(Monster::Unstoppable) {
            1.0 - (1.0 - slow) * super::monster_mod_effects::get().stats.slow_effect
        } else {
            slow
        }
    }
    pub fn movement_speed(self, speed: f32, base: f32, aura: f32, endless: bool, slow: f32) -> f32 {
        let speed = speed * aura;
        let speed = if endless && base > 0.0 {
            base * super::endless::config().cap_movement(speed / base)
        } else {
            speed
        };
        speed * self.slow(slow)
    }
    pub fn critical(self, roll: f32) -> bool {
        self.has(Monster::Precise) && roll < super::monster_mod_effects::get().stats.critical_chance
    }
    pub fn projectiles(self) -> u32 {
        let ordinary = if self.has(Monster::Artillery) {
            super::monster_mod_effects::get().stats.projectiles
        } else {
            0
        };
        ordinary
            + if self.has(Monster::GreaterArtillery) {
                super::monster_specials::config().extra_projectiles
            } else {
                0
            }
    }
    pub fn chains(self) -> u32 {
        if self.has(Monster::Conductor) {
            super::monster_mod_effects::get().stats.chains
        } else {
            0
        }
    }
    pub fn damage_taken(self, tags: Tags) -> f32 {
        let s = &super::monster_mod_effects::get().stats;
        let mut multiplier = 1.0;
        for (modifier, school, factor) in [
            (Monster::Armoured, Tags::PHYSICAL, s.physical_taken),
            (Monster::Fireproof, Tags::FIRE, s.elemental_taken),
            (Monster::Winter, Tags::COLD, s.elemental_taken),
            (
                Monster::Insulated,
                Tags::LIGHTNING | Tags::NATURE,
                s.elemental_taken,
            ),
            (
                Monster::Occult,
                Tags::SHADOW | Tags::ARCANE,
                s.elemental_taken,
            ),
        ] {
            if self.has(modifier) && tags.intersects(school) {
                multiplier *= factor;
            }
        }
        multiplier
    }
    pub fn packets(self, raw: f32, tags: Tags, critical: bool) -> Vec<(f32, Tags)> {
        let s = &super::monster_mod_effects::get().stats;
        let crit = if critical && !tags.contains(Tags::DOT) {
            1.5 + if self.has(Monster::Devastating) {
                s.critical_multiplier
            } else {
                0.0
            }
        } else {
            1.0
        };
        let dot = if self.has(Monster::Venomous) && tags.contains(Tags::DOT) {
            s.dot_damage
        } else {
            1.0
        };
        let physical = if self.has(Monster::Brutal) && tags.contains(Tags::PHYSICAL) {
            s.physical_damage
        } else {
            1.0
        };
        let mut result = vec![(raw * physical * crit * dot, tags)];
        // Additional damage retains targeting/source tags but has exactly one damage school.
        for (modifier, school) in [
            (Monster::Pyromancer, Tags::FIRE),
            (Monster::Glacial, Tags::COLD),
            (Monster::Storm, Tags::LIGHTNING | Tags::NATURE),
            (Monster::Profane, Tags::SHADOW),
        ] {
            if self.has(modifier) && !tags.contains(Tags::DOT) {
                result.push((
                    raw * s.added_elemental * crit,
                    Tags((tags.0 & !(0xff << 16)) | school.0),
                ));
            }
        }
        result
    }
}
pub(super) fn deal(
    run: &mut Run,
    raw: f32,
    tags: Tags,
    bonuses: Bonuses,
    critical: bool,
    melee_roll: Option<f32>,
) -> f32 {
    deal_impl(run, raw, tags, bonuses, critical, melee_roll, true)
}
pub(super) fn deal_secondary(run: &mut Run, raw: f32, tags: Tags, bonuses: Bonuses) -> f32 {
    deal_impl(run, raw, tags, bonuses, false, None, false)
}
fn deal_impl(
    run: &mut Run,
    raw: f32,
    tags: Tags,
    bonuses: Bonuses,
    critical: bool,
    melee_roll: Option<f32>,
    trigger: bool,
) -> f32 {
    let raw = raw * super::monster_specials::outgoing(run, bonuses.0, tags);
    let previous_source = run.death_log.begin(run.legendary.events.attacker, bonuses.0, critical && !tags.contains(Tags::DOT), run.taken_source);
    let before = run.hp.max(0.0);
    let mut dealt = 0.0;
    for (index, (damage, tags)) in bonuses.packets(raw, tags, critical).into_iter().enumerate() {
        if index == 0 {
            if let Some(roll) = melee_roll {
                let avoidance = super::hero::melee_avoidance(run);
                dealt += super::hero::hurt_hero_with_block(run, damage, roll);
                if roll < avoidance {
                    break;
                }
                continue;
            }
        }
        let block_serial = run.legendary.chase.block_serial;
        dealt += super::hero::hurt_hero_tagged(run, damage, tags);
        if run.legendary.chase.block_serial != block_serial { break; }
    }
    let dealt = dealt.min(before);
    if trigger {
        super::monster_specials::inflicted(run, raw, dealt, tags, bonuses);
    }
    run.death_log.context = previous_source;
    dealt
}
pub(super) fn leech(commands: &mut Commands, source: Entity, dealt: f32, bonuses: Bonuses) {
    let amount = dealt.max(0.0) * bonuses.leech();
    if amount <= 0.0 {
        return;
    }
    commands.queue(move |world: &mut World| {
        let mut healed = 0.0;
        if let Some(mut enemy) = world.get_mut::<Enemy>(source) {
            if !enemy.dead && enemy.hp > 0.0 {
                healed = amount.min((enemy.max_hp - enemy.hp).max(0.0));
                enemy.hp += healed;
            }
        }
        if let Some(mut state) = world.get_mut::<super::monster_specials::Runtime>(source) {
            state.healed += healed;
        }
    });
}
#[derive(Component, Clone, Copy)]
pub(super) struct AuraBonuses {
    pub action_speed: f32,
}
impl Default for AuraBonuses {
    fn default() -> Self {
        Self { action_speed: 1.0 }
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn tick(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut companions: Option<ResMut<super::multiplayer::Companions>>,
    mut enemies: Query<(
        Entity,
        &Transform,
        &mut Enemy,
        &Monster,
        Option<&mut AuraBonuses>,
        Option<&mut super::monster_specials::Runtime>,
    )>,
    players: Query<
        (
            &Transform,
            Option<&super::multiplayer::RemoteOwner>,
            Has<super::hero::Hero>,
        ),
        Without<Enemy>,
    >,
) {
    let s = &super::monster_mod_effects::get().stats;
    let key = |pos: Vec3| {
        (
            (pos.x / s.aura_radius).floor() as i32,
            (pos.z / s.aura_radius).floor() as i32,
        )
    };
    let mut grid = std::collections::HashMap::<(i32, i32), Vec<(Vec3, Monster)>>::new();
    for (_, tf, enemy, rank, _, _) in &enemies {
        if !enemy.dead
            && enemy.hp > 0.0
            && !super::training::actor(enemy.def)
            && (rank.has(Monster::Banner) || rank.has(Monster::Oppressor))
        {
            grid.entry(key(tf.translation))
                .or_default()
                .push((tf.translation, *rank));
        }
    }
    let auras = |pos: Vec3| {
        let mut flags = Monster::Normal;
        if grid.is_empty() {
            return flags;
        }
        let (x, z) = key(pos);
        for dx in -1..=1 {
            for dz in -1..=1 {
                if let Some(sources) = grid.get(&(x + dx, z + dz)) {
                    for (source, rank) in sources {
                        if super::units::flat(pos - *source).length_squared()
                            <= s.aura_radius * s.aura_radius
                        {
                            flags = flags.with(*rank);
                            if flags.has(Monster::Banner) && flags.has(Monster::Oppressor) {
                                return flags;
                            }
                        }
                    }
                }
            }
        }
        flags
    };
    for (entity, tf, mut enemy, rank, aura, state) in &mut enemies {
        if enemy.dead || enemy.hp <= 0.0 || super::training::actor(enemy.def) {
            continue;
        }
        let amount = enemy.max_hp * Bonuses(*rank).regeneration() * time.delta_secs();
        super::monster_specials::heal(&mut enemy, state.map(|s| s.into_inner()), amount, true);
        let rate = if auras(tf.translation).has(Monster::Banner) {
            s.aura_action_speed
        } else {
            1.0
        };
        if let Some(mut aura) = aura {
            aura.action_speed = rate;
        } else {
            commands
                .entity(entity)
                .insert(AuraBonuses { action_speed: rate });
        }
    }
    run.monster_pressure = 1.0;
    if let Some(party) = companions.as_deref_mut() {
        for player in party.0.values_mut() {
            player.run.monster_pressure = 1.0;
        }
    }
    for (tf, owner, local) in &players {
        let multiplier = if auras(tf.translation).has(Monster::Oppressor) {
            1.0 - s.oppression
        } else {
            1.0
        };
        if local {
            run.monster_pressure = multiplier;
        } else if let Some(player) =
            owner.and_then(|id| companions.as_deref_mut().and_then(|p| p.0.get_mut(&id.0)))
        {
            player.run.monster_pressure = multiplier;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    #[test]
    fn action_auras_cannot_bypass_the_endless_movement_cap() {
        let bonuses = Bonuses(Monster::Banner.with(Monster::Sprinter));
        let cap = super::super::endless::config().cap_movement(1000.0);
        assert_eq!(bonuses.movement_speed(60.0, 6.0, 1.2, true, 1.0), 6.0 * cap);
        assert_eq!(bonuses.movement_speed(60.0, 6.0, 1.2, false, 1.0), 72.0);
        assert_eq!(bonuses.movement_speed(60.0, 6.0, 1.2, true, 0.5), 3.0 * cap);
    }
    #[test]
    fn less_damage_is_multiplicative_and_keeps_schools_separate_for_hits_and_dots() {
        let rank = Monster::Armoured
            .with(Monster::Fireproof)
            .with(Monster::Winter)
            .with(Monster::Insulated)
            .with(Monster::Occult);
        for school in [
            Tags::PHYSICAL,
            Tags::FIRE,
            Tags::COLD,
            Tags::LIGHTNING | Tags::NATURE,
            Tags::SHADOW,
            Tags::ARCANE,
        ] {
            for kind in [Tags::ATTACK, Tags::DOT] {
                assert_eq!(Bonuses(rank).damage_taken(school | kind), 0.8);
                assert_eq!(Bonuses(Monster::Normal).damage_taken(school | kind), 1.0);
            }
        }
        assert_eq!(Bonuses(rank).damage_taken(Tags::HOLY), 1.0);
        assert_eq!(Bonuses(Monster::Armoured).damage_taken(Tags::FIRE), 1.0);
        assert!(
            (100.0 * 0.25 * Bonuses(Monster::Fireproof).damage_taken(Tags::FIRE) - 20.0).abs()
                < 0.001
        );
    }
    #[test]
    fn added_elements_keep_their_school_and_do_not_inherit_physical_bonus() {
        let bonuses = Bonuses(
            Monster::Brutal
                .with(Monster::Pyromancer)
                .with(Monster::Precise)
                .with(Monster::Devastating),
        );
        let packets = bonuses.packets(100.0, Tags::PHYSICAL | Tags::ATTACK, true);
        assert_eq!(packets.len(), 2);
        assert_eq!(packets[0].0, 315.0);
        assert_eq!(packets[1].0, 78.75);
        assert!(packets[1].1.contains(Tags::FIRE | Tags::ATTACK));
        assert!(!packets[1].1.contains(Tags::PHYSICAL));
        assert!(bonuses.critical(0.149));
        assert!(!bonuses.critical(0.15));
        assert_eq!(
            Bonuses(Monster::Venomous).packets(100.0, Tags::SHADOW | Tags::DOT, true)[0].0,
            175.0
        );
    }
    fn enemy(world: &mut World, rank: Monster, pos: Vec3) -> Entity {
        let id = world
            .run_system_once(move |mut commands: Commands, mut run: ResMut<Run>| {
                super::super::enemies::spawn_enemy(
                    &mut commands,
                    &mut run,
                    &super::super::data::roster(0).enemies[0],
                    pos,
                    Vec3::ZERO,
                    false,
                )
            })
            .unwrap();
        world.entity_mut(id).insert(rank);
        let mut e = world.get_mut::<Enemy>(id).unwrap();
        e.max_hp = 100.0;
        e.hp = 50.0;
        id
    }
    #[test]
    fn regeneration_is_five_percent_of_maximum_life_per_second_and_never_revives() {
        let mut world = World::new();
        world.init_resource::<Run>();
        world.init_resource::<Time>();
        let live = enemy(&mut world, Monster::Regenerating, Vec3::ZERO);
        let dead = enemy(&mut world, Monster::Regenerating, Vec3::X);
        world.get_mut::<Enemy>(dead).unwrap().dead = true;
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(2));
        world.run_system_once(tick).unwrap();
        assert_eq!(world.get::<Enemy>(live).unwrap().hp, 60.0);
        assert_eq!(world.get::<Enemy>(dead).unwrap().hp, 50.0);
        world.get_mut::<Enemy>(live).unwrap().hp = 99.0;
        world.run_system_once(tick).unwrap();
        assert_eq!(world.get::<Enemy>(live).unwrap().hp, 100.0);
    }
    #[test]
    fn auras_do_not_stack_and_expire_as_soon_as_their_sources_leave() {
        let mut world = World::new();
        world.init_resource::<Run>();
        world.init_resource::<Time>();
        let first = enemy(
            &mut world,
            Monster::Banner.with(Monster::Oppressor),
            Vec3::ZERO,
        );
        let second = enemy(
            &mut world,
            Monster::Banner.with(Monster::Oppressor),
            Vec3::X,
        );
        let target = enemy(&mut world, Monster::Normal, Vec3::X * 3.0);
        world.spawn((
            super::super::hero::Hero { vy: 0.0, air: 0.0 },
            Transform::default(),
        ));
        world.run_system_once(tick).unwrap();
        assert_eq!(world.get::<AuraBonuses>(target).unwrap().action_speed, 1.2);
        assert_eq!(world.get::<AuraBonuses>(first).unwrap().action_speed, 1.2);
        assert_eq!(world.get::<AuraBonuses>(second).unwrap().action_speed, 1.2);
        assert_eq!(world.resource::<Run>().monster_pressure, 0.85);
        world.get_mut::<Enemy>(first).unwrap().dead = true;
        world.get_mut::<Transform>(second).unwrap().translation = Vec3::X * 50.0;
        world.run_system_once(tick).unwrap();
        assert_eq!(world.get::<AuraBonuses>(target).unwrap().action_speed, 1.0);
        assert_eq!(world.resource::<Run>().monster_pressure, 1.0);
    }
    #[test]
    fn vampirism_uses_actual_health_damage_and_does_not_restore_dead_attackers() {
        let mut run = Run {
            hp: 10.0,
            ..Default::default()
        };
        let bonuses = Bonuses(Monster::Vampiric);
        let dealt = deal(&mut run, 1000.0, Tags::FIRE, bonuses, false, None);
        assert_eq!(dealt, 10.0);
        let mut world = World::new();
        world.init_resource::<Run>();
        let source = enemy(&mut world, Monster::Vampiric, Vec3::ZERO);
        world
            .run_system_once(move |mut commands: Commands| {
                leech(&mut commands, source, dealt, bonuses)
            })
            .unwrap();
        assert_eq!(world.get::<Enemy>(source).unwrap().hp, 50.5);
        world.get_mut::<Enemy>(source).unwrap().dead = true;
        world
            .run_system_once(move |mut commands: Commands| {
                leech(&mut commands, source, dealt, bonuses)
            })
            .unwrap();
        assert_eq!(world.get::<Enemy>(source).unwrap().hp, 50.5);
    }
    #[test]
    fn blocking_a_melee_hit_also_blocks_its_added_elements() {
        let mut run = Run {
            hp: 100.0,
            ..Default::default()
        };
        run.equipment.block = 0.75;
        assert_eq!(
            deal(
                &mut run,
                100.0,
                Tags::PHYSICAL | Tags::MELEE,
                Bonuses(Monster::Pyromancer.with(Monster::Glacial)),
                false,
                Some(0.0)
            ),
            0.0
        );
        assert_eq!(run.hp, 100.0);
    }
}
