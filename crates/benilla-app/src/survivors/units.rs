//! Local units and effects, built the way the net layer builds streamed ones: a [`Guid`], a
//! [`NetEntity`] and an [`ObjectStore`] are all the attach, animation and spell-visual systems need.

use benilla_protocol::messages::{ObjectFields, ObjectType};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use super::data::Weapon;
use super::RunEntity;
use crate::combat_text::CombatTextSpawn;
use crate::creature_anim::{CastEvent, CastEventKind, PlaySeq, SpellGoTargets, SwingMessage};
use crate::net::{FieldChanged, Guid, NetEntity, ObjectStore};

const FIELD_HEALTH: u16 = 22;
const FIELD_MAX_HEALTH: u16 = 28;
const FIELD_LEVEL: u16 = 34;
const FIELD_FACTION: u16 = 35;
const FIELD_BYTES_0: u16 = 36;
const FIELD_VIRTUAL_ITEM_DISPLAY: u16 = 37;
const FIELD_VIRTUAL_ITEM_INFO: u16 = 40;
const FIELD_AURA: u16 = 47;
const FIELD_AURAFLAGS: u16 = 95;
const FIELD_BYTES_2: u16 = 164;

/// `FactionTemplate.dbc`: friendly to players, and the hostile monster template.
pub(super) const FACTION_FRIENDLY: u32 = 35;
pub(super) const FACTION_MONSTER: u32 = 14;

const HIGH_CREATURE: u64 = 0xF130 << 48;
const HIGH_GAMEOBJECT: u64 = 0xF110 << 48;
const HIGH_DYNAMIC: u64 = 0xF100 << 48;

const FIELD_GO_DISPLAY: u16 = 8;
const FIELD_GO_STATE: u16 = 14;
const FIELD_GO_TYPE: u16 = 21;
/// `GAMEOBJECT_TYPE_GENERIC`: a model and nothing to use.
const GO_TYPE_GENERIC: u32 = 5;
/// `GO_STATE_READY`.
const GO_STATE_READY: u32 = 1;

/// Draw the hunter's ranged weapon for projectiles and melee weapons for other skills.
pub(super) fn hunter_weapon(
    commands: &mut Commands,
    entity: Entity,
    tags: super::spell_engine::Tags,
) {
    let sheath = if tags.contains(super::spell_engine::Tags::PROJECTILE) {
        2
    } else {
        1
    };
    commands.queue(move |world: &mut World| {
        let Some(guid) = world.get::<Guid>(entity).map(|g| g.0) else {
            return;
        };
        let Some(mut store) = world.get_mut::<ObjectStore>(entity) else {
            return;
        };
        let old = store
            .0
            .raw_fields()
            .find(|(index, _)| *index == FIELD_BYTES_2)
            .map_or(0, |(_, value)| value);
        let new = (old & !0xff) | sheath;
        if old == new {
            return;
        }
        store
            .0
            .merge(ObjectFields::from_pairs(&[(FIELD_BYTES_2, new)]));
        world.write_message(FieldChanged {
            entity,
            guid,
            kind: ObjectType::Unit,
            index: FIELD_BYTES_2,
            old,
            new,
        });
    });
}

pub(super) fn wow_to_bevy(p: [f32; 3]) -> Vec3 {
    Vec3::new(-p[1], p[2], -p[0])
}

fn bevy_to_wow(p: Vec3) -> [f32; 3] {
    [-p.z, -p.x, p.y]
}

/// The Bevy yaw that faces a model along `dir` (its XZ part).
pub(super) fn yaw_toward(dir: Vec3) -> f32 {
    (-dir.x).atan2(-dir.z)
}

pub(super) fn flat(v: Vec3) -> Vec3 {
    Vec3::new(v.x, 0.0, v.z)
}

pub(super) struct UnitSpec<'a> {
    pub serial: u64,
    pub display: u32,
    pub scale: f32,
    pub level: u32,
    pub faction: u32,
    /// `UNIT_FIELD_BYTES_0`: race, class, gender, power type.
    pub bytes0: u32,
    pub weapons: &'a [Weapon],
    /// `UNIT_FIELD_BYTES_2` byte 0: 0 stowed, 1 melee drawn, 2 ranged drawn.
    pub sheath: u8,
}

/// Spawn a unit the attach builds a body for; the caller adds its own components.
pub(super) fn spawn_unit(commands: &mut Commands, spec: &UnitSpec, pos: Vec3, yaw: f32) -> Entity {
    let guid = HIGH_CREATURE | spec.serial;
    let mut pairs: Vec<(u16, u32)> = vec![
        (0, guid as u32),
        (1, (guid >> 32) as u32),
        (2, 0x9),
        (4, spec.scale.to_bits()),
        (FIELD_HEALTH, 100),
        (FIELD_MAX_HEALTH, 100),
        (FIELD_LEVEL, spec.level),
        (FIELD_FACTION, spec.faction),
        (FIELD_BYTES_0, spec.bytes0),
        (FIELD_BYTES_2, u32::from(spec.sheath)),
    ];
    for &(slot, display, class, sub, material, inv, sheath) in spec.weapons {
        pairs.push((FIELD_VIRTUAL_ITEM_DISPLAY + slot, display));
        pairs.push((
            FIELD_VIRTUAL_ITEM_INFO + 2 * slot,
            u32::from(class)
                | u32::from(sub) << 8
                | u32::from(material) << 16
                | u32::from(inv) << 24,
        ));
        pairs.push((FIELD_VIRTUAL_ITEM_INFO + 2 * slot + 1, u32::from(sheath)));
    }
    commands
        .spawn((
            Guid(guid),
            NetEntity {
                kind: benilla_protocol::EntityKind::Unit,
                display_id: Some(spec.display),
                scale: spec.scale,
            },
            ObjectStore(ObjectFields::from_pairs(&pairs)),
            crate::creature_anim::MovementState::default(),
            crate::net::ScriptedFacing,
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
            RunEntity,
        ))
        .id()
}

/// Spawn a generic GameObject showing `GameObjectDisplayInfo` row `display`.
pub(super) fn spawn_object(
    commands: &mut Commands,
    serial: u64,
    display: u32,
    scale: f32,
    pos: Vec3,
) -> Entity {
    let guid = HIGH_GAMEOBJECT | serial;
    let pairs = [
        (0, guid as u32),
        (1, (guid >> 32) as u32),
        (2, 0x21),
        (4, scale.to_bits()),
        (FIELD_GO_DISPLAY, display),
        (FIELD_GO_STATE, GO_STATE_READY),
        (FIELD_GO_TYPE, GO_TYPE_GENERIC),
    ];
    commands
        .spawn((
            Guid(guid),
            NetEntity {
                kind: benilla_protocol::EntityKind::GameObject,
                display_id: Some(display),
                scale,
            },
            ObjectStore(ObjectFields::from_pairs(&pairs)),
            Transform::from_translation(pos),
            Visibility::default(),
            RunEntity,
        ))
        .id()
}

/// Explosive Trap's original GameObjectDisplayInfo model. Type 6 enables WoW's
/// closed/open state animation; a generic decorative object would never animate.
pub(super) fn spawn_explosive_trap(commands: &mut Commands, serial: u64, pos: Vec3) -> Entity {
    spawn_animated_trap(commands, serial, 3072, pos)
}

fn spawn_animated_trap(commands: &mut Commands, serial: u64, display: u32, pos: Vec3) -> Entity {
    let entity = spawn_object(commands, serial, display, 1.0, pos);
    commands
        .entity(entity)
        .insert(ObjectStore(ObjectFields::from_pairs(&[
            (0, (HIGH_GAMEOBJECT | serial) as u32),
            (1, ((HIGH_GAMEOBJECT | serial) >> 32) as u32),
            (2, 0x21),
            (4, 1.0f32.to_bits()),
            (FIELD_GO_DISPLAY, display),
            (FIELD_GO_STATE, GO_STATE_READY),
            (FIELD_GO_TYPE, 6),
        ])));
    entity
}

/// The physical Frost Trap (GameObjectDisplayInfo 3073) and its persistent frost layer.
pub(super) fn spawn_frost_trap(
    commands: &mut Commands,
    serial: u64,
    radius: f32,
    scale: f32,
    pos: Vec3,
) -> Entity {
    let trap = spawn_animated_trap(commands, serial, 3073, pos);
    let aura = spawn_area_object(commands, serial, 13809, radius, scale, pos);
    commands
        .entity(aura)
        .insert((ChildOf(trap), Transform::from_translation(Vec3::Y * 0.03)));
    trap
}

/// Zero health: the animation selector plays Death and holds the corpse pose.
pub(super) fn kill_unit(store: &mut ObjectStore) {
    store
        .0
        .merge(ObjectFields::from_pairs(&[(FIELD_HEALTH, 0)]));
}

/// A dynamic object, as a ground spell's server would place it: the dest-effect systems arm the
/// spell's area visuals on it. `scale` sizes those visuals: the radius over the spell's base.
pub(super) fn spawn_area_object(
    commands: &mut Commands,
    serial: u64,
    spell: u32,
    radius: f32,
    scale: f32,
    pos: Vec3,
) -> Entity {
    // Spell.dbc: 13809 only places the trap; 13810's area visual is FrostTrap_Aura.
    // Its ICESLICK_AREA ground quad spans 25 yd in the shipped M2 (vertices +/-12.5).
    let (spell, scale, pos) = if spell == 13809 {
        (13810, (radius / 12.5).max(0.01), pos + Vec3::Y * 0.03)
    } else {
        (spell, scale, pos)
    };
    let guid = HIGH_DYNAMIC | serial;
    let wow = bevy_to_wow(pos);
    commands
        .spawn((
            Guid(guid),
            NetEntity {
                kind: benilla_protocol::EntityKind::DynamicObject,
                display_id: None,
                scale: 1.0,
            },
            ObjectStore(ObjectFields::from_pairs(&[
                (0, guid as u32),
                (1, (guid >> 32) as u32),
                (2, 0x41),
                (3, spell),
                (4, scale.to_bits()),
                (8, 1),
                (9, spell),
                (10, radius.to_bits()),
                (11, wow[0].to_bits()),
                (12, wow[1].to_bits()),
                (13, wow[2].to_bits()),
                (14, 0),
            ])),
            Transform::from_translation(pos),
            Visibility::default(),
            RunEntity,
        ))
        .id()
}

/// Eight aura slots we drive on a unit, from `base` (a multiple of 8, so one flag word covers
/// them): the spell visuals' state kits arm and reap off these as off a server's.
#[derive(Component)]
pub(super) struct Marks {
    base: u8,
    slots: [(u32, f32); 8],
    dirty: bool,
}

impl Marks {
    pub(super) fn active_debuffs(&self) -> impl Iterator<Item = (u32, f32)> + '_ {
        self.slots.iter().copied().filter(|&(spell, seconds)| {
            self.base >= 32 && spell != 0 && seconds > 0.0 && seconds.is_finite()
        })
    }
    pub fn buffs() -> Self {
        Self {
            base: 0,
            slots: [(0, 0.0); 8],
            dirty: false,
        }
    }
    pub fn debuffs() -> Self {
        Self {
            base: 32,
            slots: [(0, 0.0); 8],
            dirty: false,
        }
    }

    /// Show `spell` for `secs`, refreshing it if already shown.
    pub fn add(&mut self, spell: u32, secs: f32) {
        if let Some(slot) = self.slots.iter_mut().find(|s| s.0 == spell) {
            slot.1 = slot.1.max(secs);
            return;
        }
        if let Some(slot) = self.slots.iter_mut().find(|s| s.0 == 0) {
            *slot = (spell, secs);
            self.dirty = true;
        }
    }

    /// Exactly these spells, held until changed.
    pub fn set(&mut self, spells: &[u32]) {
        let mut want = [(0u32, f32::INFINITY); 8];
        for (slot, &spell) in want.iter_mut().zip(spells) {
            slot.0 = spell;
        }
        if want.iter().map(|s| s.0).ne(self.slots.iter().map(|s| s.0)) {
            self.slots = want;
            self.dirty = true;
        }
    }

    pub fn clear(&mut self) {
        if self.slots.iter().any(|s| s.0 != 0) {
            self.slots = [(0, 0.0); 8];
            self.dirty = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        for slot in &mut self.slots {
            if slot.0 != 0 {
                slot.1 -= dt;
                if slot.1 <= 0.0 {
                    *slot = (0, 0.0);
                    self.dirty = true;
                }
            }
        }
    }

    /// Write the slots into the unit's aura arrays and ring the watch, if they changed.
    pub fn flush(
        &mut self,
        entity: Entity,
        guid: u64,
        store: &mut ObjectStore,
        fields: &mut MessageWriter<FieldChanged>,
    ) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        let mut pairs = Vec::with_capacity(9);
        let mut word = 0u32;
        for (i, &(spell, _)) in self.slots.iter().enumerate() {
            pairs.push((FIELD_AURA + u16::from(self.base) + i as u16, spell));
            if spell != 0 {
                // Effect 0 live, and a buff is cancelable.
                let flags = 0x2 | u32::from(self.base < 32);
                word |= flags << (i * 4);
            }
        }
        pairs.push((FIELD_AURAFLAGS + u16::from(self.base >> 3), word));
        store.0.merge(ObjectFields::from_pairs(&pairs));
        fields.write(FieldChanged {
            entity,
            guid,
            kind: ObjectType::Unit,
            index: FIELD_AURA + u16::from(self.base),
            old: 0,
            new: 1,
        });
    }
}

/// The messages that make a local cast look like a server's.
#[derive(Message, Clone)]
pub(super) struct CombatNumber {
    pub owner: Option<u64>,
    pub anchor: Entity,
    pub amount: f32,
    pub crit: bool,
    pub color: u32,
}

#[derive(SystemParam)]
pub(super) struct Fx<'w> {
    settings: Option<Res<'w, super::options::Settings>>,
    seq: ResMut<'w, PlaySeq>,
    casts: MessageWriter<'w, CastEvent>,
    gos: MessageWriter<'w, SpellGoTargets>,
    swings: MessageWriter<'w, SwingMessage>,
    text: MessageWriter<'w, CombatTextSpawn>,
    area_scales: Option<ResMut<'w, crate::entities::dest_fx::AreaVisualScales>>,
    echoes: Option<ResMut<'w, crate::entities::dest_fx::EchoCasts>>,
    numbers: Option<ResMut<'w, Messages<CombatNumber>>>,
    colors: Option<Res<'w, super::damage_colors::DamageColors>>,
}

/// More impacts than this per cast only cost frames.
const MAX_VISUAL_HITS: usize = 12;

impl Fx<'_> {
    /// Size `caster`'s visuals of `spell` for the radius it will cast at.
    pub fn area_scale(&mut self, caster: Entity, spell: u32, scale: f32) {
        if let Some(scales) = self.area_scales.as_mut() {
            scales.0.insert((caster, spell), scale);
        }
    }

    /// The next effects of `spell` on `unit` are an echo: they overlap the running ones.
    pub fn echo(&mut self, unit: Entity, spell: u32) {
        if let Some(echoes) = self.echoes.as_mut() {
            echoes.0.insert((unit, spell));
        }
    }

    pub fn trigger_trap(
        &mut self,
        entity: Entity,
        guid: u64,
        store: &mut ObjectStore,
        pos: Vec3,
        fields: &mut MessageWriter<FieldChanged>,
    ) {
        if store.0.gameobject_state() != Some(GO_STATE_READY) {
            return;
        }
        store
            .0
            .merge(ObjectFields::from_pairs(&[(FIELD_GO_STATE, 0)]));
        fields.write(FieldChanged {
            entity,
            guid,
            kind: ObjectType::GameObject,
            index: FIELD_GO_STATE,
            old: GO_STATE_READY,
            new: 0,
        });
        // 13813 places the trap; 13812 is its actual explosion (visual 7659).
        if store.0.gameobject_displayid() == Some(3072) {
            self.cast(entity, 13812, &[], Some(pos));
        }
    }
    /// `SMSG_SPELL_GO`: the cast kit on the caster, and a missile or an impact at each hit.
    pub fn cast(&mut self, caster: Entity, spell: u32, hits: &[Entity], dest: Option<Vec3>) {
        let seq = self.seq.next();
        self.casts.write(CastEvent {
            entity: caster,
            spell_id: spell,
            kind: CastEventKind::Go,
            seq,
        });
        self.gos.write(SpellGoTargets {
            caster,
            spell_id: spell,
            hits: hits.iter().copied().take(MAX_VISUAL_HITS).collect(),
            misses: Vec::new(),
            dest,
            ammo_display_id: None,
            seq,
        });
    }

    /// Only the spell's impact kit, on `target`: no cast animation on anyone.
    pub fn impact(&mut self, target: Entity, spell: u32) {
        let seq = self.seq.next();
        self.casts.write(CastEvent {
            entity: target,
            spell_id: spell,
            kind: CastEventKind::Impact {
                weapon_visual: None,
            },
            seq,
        });
    }

    /// A melee swing: the attack animation, the victim's flinch, blood and the hit sound.
    pub fn swing(&mut self, attacker: Entity, victim: Entity, damage: f32) {
        let seq = self.seq.next();
        self.swings.write(SwingMessage {
            attacker,
            victim: Some(victim),
            hit_info: 0x2,
            victim_state: 1,
            damage: damage.max(0.0) as u32,
            displayed: false,
            seq,
        });
    }

    /// The number of a direct hit, coloured by its type.
    pub fn hit_number(
        &mut self,
        owner: Option<u64>,
        anchor: Entity,
        amount: f32,
        crit: bool,
        tags: super::spell_engine::Tags,
    ) {
        let color = self
            .colors
            .as_deref()
            .map_or(super::damage_colors::FALLBACK, |c| c.direct(tags, crit));
        self.number(owner, anchor, amount, crit, color);
    }

    /// The colour of a damage-over-time tick.
    pub fn periodic_color(&self, tags: super::spell_engine::Tags) -> u32 {
        self.colors
            .as_deref()
            .map_or(COLOR_SPELL, |c| c.periodic(tags))
    }

    /// A floating number over `anchor`, 0xAARRGGBB.
    pub fn number(
        &mut self,
        owner: Option<u64>,
        anchor: Entity,
        amount: f32,
        crit: bool,
        color: u32,
    ) {
        if let Some(numbers) = self.numbers.as_mut() {
            numbers.write(CombatNumber {
                owner,
                anchor,
                amount,
                crit,
                color,
            });
        }
        // The authoritative process renders the host; guest numbers travel to their owner.
        if owner.is_some() {
            return;
        }
        if self
            .settings
            .as_deref()
            .is_some_and(|s| !s.numbers(color == COLOR_HEAL, color == COLOR_HURT, crit))
        {
            return;
        }
        let shown = amount.round().max(1.0) as i64;
        self.text.write(CombatTextSpawn {
            anchor,
            text: shown.to_string(),
            category: if crit { 2 } else { 0 },
            color: Some(color),
        });
    }
}

pub(super) const COLOR_SPELL: u32 = 0xFFFF_DE00;
pub(super) const COLOR_MELEE: u32 = 0xFFFF_FFFF;
pub(super) const COLOR_HURT: u32 = 0xFFFF_3030;
pub(super) const COLOR_HEAL: u32 = 0xFF30_FF30;

#[cfg(test)]
mod trap_tests {
    use super::*;

    #[test]
    fn frost_trap_has_a_visible_armed_gameobject_and_a_child_frost_layer() {
        let mut world = World::new();
        let pos = Vec3::new(3.0, 5.0, 7.0);
        let trap = spawn_frost_trap(&mut world.commands(), 42, 6.0, 1.0, pos);
        world.flush();
        assert_eq!(world.get::<NetEntity>(trap).unwrap().display_id, Some(3073));
        let store = world.get::<ObjectStore>(trap).unwrap();
        assert_eq!(store.0.gameobject_state(), Some(GO_STATE_READY));
        assert_eq!(store.0.gameobject_type_id(), 6);
        let children: Vec<_> = world
            .query::<(&ChildOf, &ObjectStore, &Transform)>()
            .iter(&world)
            .filter(|(parent, _, _)| parent.parent() == trap)
            .collect();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].1 .0.dynamicobject_spell_id(), Some(13810));
        assert_eq!(children[0].2.translation, Vec3::Y * 0.03);
        world.despawn(trap);
        assert_eq!(world.query::<&NetEntity>().iter(&world).count(), 0);
    }

    #[test]
    fn physical_trap_models_exist_in_the_wow_install() {
        let data = benilla_formats::wow_data_or_skip!();
        let mut chain = benilla_formats::open_chain(&data).unwrap();
        let catalog = benilla_formats::load_gameobject_catalog(&mut chain).unwrap();
        for (display, model) in [
            (3072, "world\\goober\\g_explosivetrap"),
            (3073, "world\\goober\\g_frosttrap"),
        ] {
            assert_eq!(
                catalog.model_path(display).unwrap().to_ascii_lowercase(),
                format!("{model}.mdx")
            );
            assert!(chain.read_file(&format!("{model}.m2")).is_ok());
        }
    }

    #[test]
    fn frost_trap_arms_its_persistent_wow_frost_layer_at_the_gameplay_radius() {
        for radius in [6.0, 12.0] {
            let mut world = World::new();
            let pos = Vec3::new(3.0, 5.0, 7.0);
            let entity =
                spawn_area_object(&mut world.commands(), 42, 13809, radius, radius / 6.0, pos);
            world.flush();
            let store = world.get::<ObjectStore>(entity).unwrap();
            assert_eq!(store.0.dynamicobject_spell_id(), Some(13810));
            assert_eq!(store.0.dynamicobject_radius(), Some(radius));
            assert!((store.0.object_scale_x().unwrap() * 12.5 - radius).abs() < 0.001);
            assert_eq!(
                world.get::<Transform>(entity).unwrap().translation,
                pos + Vec3::Y * 0.03
            );
        }
    }

    #[test]
    fn frost_trap_aura_resolves_to_the_real_iceslick_model_in_the_install() {
        let data = benilla_formats::wow_data_or_skip!();
        let mut chain = benilla_formats::open_chain(&data).unwrap();
        let spells = benilla_formats::load_spell_catalog(&mut chain).unwrap();
        let visuals = benilla_formats::load_spell_visual_catalog(&mut chain).unwrap();
        let place = visuals.stages(spells.get(13809).unwrap().visual).unwrap();
        assert_eq!(place.area_effect, 0);
        let aura = visuals.stages(spells.get(13810).unwrap().visual).unwrap();
        assert_ne!(aura.area_gate, 0);
        let path = visuals.effect_path(aura.area_effect).unwrap();
        assert_eq!(path.to_ascii_lowercase(), "spells\\frosttrap_aura.mdx");
        assert!(chain.read_file("spells\\frosttrap_aura.m2").is_ok());
        assert!(chain.read_file("SPELLS\\ICESLICK_AREA.BLP").is_ok());
    }

    #[test]
    fn trap_uses_animated_wow_gameobject_and_explosion() {
        let mut app = App::new();
        app.init_resource::<PlaySeq>()
            .add_message::<CastEvent>()
            .add_message::<SpellGoTargets>()
            .add_message::<SwingMessage>()
            .add_message::<CombatTextSpawn>()
            .add_message::<FieldChanged>();
        let pos = Vec3::new(3.0, 5.0, 7.0);
        let entity = {
            let mut commands = app.world_mut().commands();
            spawn_explosive_trap(&mut commands, 42, pos)
        };
        app.world_mut().flush();
        let store = app.world().get::<ObjectStore>(entity).unwrap();
        assert_eq!(store.0.gameobject_state(), Some(1));
        assert_eq!(store.0.gameobject_type_id(), 6);
        assert_eq!(
            app.world().get::<NetEntity>(entity).unwrap().display_id,
            Some(3072)
        );
        app.add_systems(
            Update,
            |mut fx: Fx,
             mut fields: MessageWriter<FieldChanged>,
             mut traps: Query<(Entity, &Guid, &Transform, &mut ObjectStore)>| {
                for (e, guid, tf, mut store) in &mut traps {
                    fx.trigger_trap(e, guid.0, &mut store, tf.translation, &mut fields);
                }
            },
        );
        app.update();
        assert_eq!(
            app.world()
                .get::<ObjectStore>(entity)
                .unwrap()
                .0
                .gameobject_state(),
            Some(0)
        );
        let events = app.world().resource::<Messages<SpellGoTargets>>();
        let mut cursor = events.get_cursor();
        let go = cursor.read(events).next().unwrap();
        assert_eq!(go.spell_id, 13812);
        assert_eq!(go.dest, Some(pos));
        let events = app.world().resource::<Messages<FieldChanged>>();
        let mut cursor = events.get_cursor();
        assert!(cursor.read(events).any(|event| event.entity == entity
            && event.index == FIELD_GO_STATE
            && event.new == 0));
    }
}

#[cfg(test)]
mod hunter_weapon_tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    #[test]
    fn casts_switch_hands_preserve_other_bytes_and_skip_redundant_updates() {
        let mut world = World::new();
        world.init_resource::<Messages<FieldChanged>>();
        let entity = world
            .spawn((
                Guid(42),
                ObjectStore(ObjectFields::from_pairs(&[(FIELD_BYTES_2, 0x12340001)])),
            ))
            .id();
        for (skill, expected, events) in [
            ("arcane_shot", 2, 1),
            ("serpent_sting", 2, 1),
            ("raptor_strike", 1, 2),
            ("explosive_trap", 1, 2),
        ] {
            let tags =
                super::super::spell_engine::tags::for_spell(super::super::data::ability(skill));
            world
                .run_system_once(move |mut commands: Commands| {
                    hunter_weapon(&mut commands, entity, tags)
                })
                .unwrap();
            let store = &world.get::<ObjectStore>(entity).unwrap().0;
            assert_eq!(store.unit_sheath_state(), Some(expected), "{skill}");
            assert!(store
                .raw_fields()
                .any(|pair| pair == (FIELD_BYTES_2, 0x12340000 | u32::from(expected))));
            assert_eq!(world.resource::<Messages<FieldChanged>>().len(), events);
        }
    }
}
