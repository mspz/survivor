//! Gameplay tags describe this offline mode, independently of DBC spell visuals.
use super::super::data::{AbilityDef, Kind};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tags(pub u64);
impl std::ops::BitOr for Tags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}
impl Tags {
    pub const ATTACK: Self = Self(1 << 0);
    pub const SPELL: Self = Self(1 << 1);
    pub const MELEE: Self = Self(1 << 2);
    pub const PROJECTILE: Self = Self(1 << 3);
    pub const AREA: Self = Self(1 << 4);
    pub const CHAIN: Self = Self(1 << 5);
    pub const DOT: Self = Self(1 << 6);
    pub const DURATION: Self = Self(1 << 7);
    pub const TOTEM: Self = Self(1 << 8);
    pub const TRAP: Self = Self(1 << 9);
    pub const AURA: Self = Self(1 << 10);
    pub const GUARD: Self = Self(1 << 11);
    pub const HEAL: Self = Self(1 << 12);
    pub const DAMAGE: Self = Self(1 << 13);
    pub const UTILITY: Self = Self(1 << 14);
    pub const MOVEMENT: Self = Self(1 << 15);
    pub const PHYSICAL: Self = Self(1 << 16);
    pub const FIRE: Self = Self(1 << 17);
    pub const COLD: Self = Self(1 << 18);
    pub const LIGHTNING: Self = Self(1 << 19);
    pub const NATURE: Self = Self(1 << 20);
    pub const ARCANE: Self = Self(1 << 21);
    pub const HOLY: Self = Self(1 << 22);
    pub const SHADOW: Self = Self(1 << 23);
    pub const BUFF: Self = Self(1 << 24);
    pub const MINION: Self = Self(1 << 25);
    pub fn contains(self, tags: Self) -> bool {
        self.0 & tags.0 == tags.0
    }
    pub fn intersects(self, tags: Self) -> bool {
        self.0 & tags.0 != 0
    }
    pub fn labels(self) -> String {
        [
            (Self::ATTACK, "Attack"),
            (Self::SPELL, "Spell"),
            (Self::MELEE, "Melee"),
            (Self::PROJECTILE, "Projectile"),
            (Self::AREA, "Area"),
            (Self::CHAIN, "Chain"),
            (Self::DOT, "Damage over time"),
            (Self::DURATION, "Duration"),
            (Self::TOTEM, "Totem"),
            (Self::TRAP, "Trap"),
            (Self::AURA, "Aura"),
            (Self::GUARD, "Guard"),
            (Self::HEAL, "Healing"),
            (Self::UTILITY, "Utility"),
            (Self::MOVEMENT, "Movement"),
            (Self::BUFF, "Buff"),
            (Self::MINION, "Minion"),
            (Self::PHYSICAL, "Physical"),
            (Self::FIRE, "Fire"),
            (Self::COLD, "Cold"),
            (Self::LIGHTNING, "Lightning"),
            (Self::NATURE, "Nature"),
            (Self::ARCANE, "Arcane"),
            (Self::HOLY, "Holy"),
            (Self::SHADOW, "Shadow"),
        ]
        .into_iter()
        .filter_map(|(tag, name)| self.contains(tag).then_some(name))
        .collect::<Vec<_>>()
        .join(", ")
    }
}

/// Weapon-style proc eligibility describes skill hits, never the client's auto-attack state.
/// Physical spells and magical melee skills qualify alongside authored attack skills.
pub fn weapon_hit(tags: Tags) -> bool {
    tags.contains(Tags::DAMAGE)
        && tags.intersects(Tags::ATTACK | Tags::PHYSICAL | Tags::MELEE)
        && !tags.intersects(Tags::GUARD | Tags::HEAL | Tags::UTILITY)
}

/// Every authored ability has an explicit damage school. Missing classifications fail tests.
pub fn school(key: &str) -> Option<Tags> {
    if matches!(
        key,
        "tame_beast" | "stomp" | "kill_command" | "bestial_wrath"
    ) {
        return Some(Tags::PHYSICAL);
    }
    if let Some(school) = super::super::support_spells::school(key) {
        return Some(school);
    }
    if let Some(spell) = super::super::procs::find(key) {
        return Some(Tags(spell.tags.0 & (0xff << 16)));
    }
    use Tags as T;
    Some(match key {
        "bloodthirst" | "revenge" | "mongoose_bite" | "fan_of_knives" | "garrote" | "rake" => {
            T::PHYSICAL
        }
        "holy_shield" | "hammer_of_the_righteous" | "penance" => T::HOLY,
        "frost_trap" | "ice_armor" => T::COLD,
        "earthbind_totem" | "efflorescence" | "totemic_recall" => T::NATURE,
        "lava_lash" | "living_bomb" => T::FIRE,
        "devouring_plague" | "seed_of_corruption" | "soul_burn" => T::SHADOW,
        "claw" => T::PHYSICAL,
        "flash_heal" => T::HOLY,
        "taunt" => T::PHYSICAL,
        "deadly_poison" => T::NATURE,
        "charge" | "sprint" | "dash" | "rend" | "overpower" | "raptor_strike" | "wing_clip"
        | "backstab" | "hemorrhage" | "swipe" => T::PHYSICAL,
        "holy_light" | "seal_of_righteousness" => T::HOLY,
        "mind_flay" | "drain_life" | "siphon_life" => T::SHADOW,
        "renew" => T::HOLY,
        "stormstrike" | "lesser_healing_wave" | "regrowth" => T::NATURE,
        "scorch" => T::FIRE,
        "ice_barrier" => T::COLD,
        "blessing_of_freedom" => T::HOLY,
        "aspect_of_the_cheetah" | "ghost_wolf" => T::NATURE,
        "fade" | "howl_of_terror" => T::SHADOW,
        "blink" => T::ARCANE,
        "cleave" | "thunder_clap" | "heroic_strike" | "whirlwind" | "sweeping_strikes"
        | "demoralizing_shout" | "execute" | "mortal_strike" | "retaliation"
        | "sinister_strike" | "eviscerate" | "blade_flurry" | "rupture" | "gouge"
        | "kidney_shot" | "multi_shot" | "aimed_shot" => T::PHYSICAL,
        "fireball" | "flamestrike" | "fire_blast" | "blast_wave" | "pyroblast" | "immolate"
        | "rain_of_fire" | "hellfire" | "searing_pain" | "explosive_trap" | "flame_shock"
        | "searing_totem" | "magma_totem" => T::FIRE,
        "frostbolt" | "frost_nova" | "blizzard" | "cone_of_cold" | "frost_shock" => T::COLD,
        "lightning_bolt" | "chain_lightning" | "lightning_shield" | "hurricane" => {
            T::LIGHTNING | T::NATURE
        }
        "serpent_sting" | "earth_shock" | "wrath" | "insect_swarm" | "entangling_roots"
        | "thorns" | "rejuvenation" => T::NATURE,
        "volley" | "concussive_shot" | "arcane_missiles" | "arcane_explosion" | "arcane_shot"
        | "moonfire" | "starfire" => T::ARCANE,
        "smite" | "holy_fire" | "holy_nova" | "power_word_shield" | "hammer_of_wrath"
        | "consecration" | "holy_shock" | "exorcism" | "holy_wrath" | "hammer_of_justice"
        | "divine_shield" | "judgement" => T::HOLY,
        "shadow_word_pain" | "mind_blast" | "psychic_scream" | "shadow_bolt" | "corruption"
        | "curse_of_agony" | "death_coil" | "shadowburn" => T::SHADOW,
        _ => return None,
    })
}

pub fn mobility(spell: u32) -> Option<(&'static str, Tags)> {
    use Tags as T;
    let key = match spell {
        100 => "charge",
        1044 => "blessing_of_freedom",
        5118 => "aspect_of_the_cheetah",
        2983 => "sprint",
        586 => "fade",
        2645 => "ghost_wolf",
        1953 => "blink",
        5484 => "howl_of_terror",
        1850 => "dash",
        _ => return None,
    };
    let mut tags = T::UTILITY | school(key)?;
    tags = tags
        | if spell == 100 {
            T::ATTACK | T::MELEE
        } else {
            T::SPELL
        };
    if spell != 5484 {
        tags = tags | T::MOVEMENT;
    }
    if spell != 1953 {
        tags = tags | T::DURATION;
    }
    if spell == 5484 {
        tags = tags | T::AREA;
    }
    Some((key, tags))
}

pub fn for_spell(def: &AbilityDef) -> Tags {
    if matches!(
        def.key,
        "tame_beast" | "stomp" | "kill_command" | "bestial_wrath"
    ) {
        let base = Tags::MINION | Tags::PHYSICAL;
        return match def.key {
            "stomp" => base | Tags::ATTACK | Tags::DAMAGE | Tags::AREA | Tags::DURATION,
            "kill_command" => base | Tags::ATTACK | Tags::DAMAGE | Tags::MOVEMENT,
            "bestial_wrath" => base | Tags::SPELL | Tags::BUFF | Tags::UTILITY | Tags::DURATION,
            _ => base | Tags::SPELL | Tags::UTILITY,
        };
    }
    if def.key == "totemic_recall" {
        return Tags::SPELL | Tags::NATURE | Tags::UTILITY | Tags::TOTEM;
    }
    if let Some(spell) = super::super::procs::find(def.key) {
        return spell.tags;
    }
    use Tags as T;
    let attack = matches!(
        def.key,
        "cleave"
            | "mangle"
            | "rip"
            | "ferocious_bite"
            | "wand_attack"
            | "bloodthirst"
            | "revenge"
            | "mongoose_bite"
            | "fan_of_knives"
            | "garrote"
            | "rake"
            | "lava_lash"
            | "hammer_of_the_righteous"
            | "frost_trap"
            | "rend"
            | "claw"
            | "overpower"
            | "raptor_strike"
            | "wing_clip"
            | "backstab"
            | "hemorrhage"
            | "swipe"
            | "stormstrike"
            | "heroic_strike"
            | "whirlwind"
            | "thunder_clap"
            | "sweeping_strikes"
            | "demoralizing_shout"
            | "execute"
            | "mortal_strike"
            | "retaliation"
            | "sinister_strike"
            | "eviscerate"
            | "blade_flurry"
            | "rupture"
            | "gouge"
            | "kidney_shot"
            | "arcane_shot"
            | "multi_shot"
            | "serpent_sting"
            | "volley"
            | "explosive_trap"
            | "concussive_shot"
            | "aimed_shot"
    );
    let mut tags = if attack { T::ATTACK } else { T::SPELL };
    if (def.damage > 0.0 && !matches!(def.kind, Kind::Shield | Kind::Heal)) || def.dot > 0.0 {
        tags = tags | T::DAMAGE;
    }
    tags = tags | school(def.key).unwrap_or_default();
    if def.kind == Kind::Melee
        || matches!(def.key, "revenge" | "fan_of_knives")
        || matches!(
            def.key,
            "execute" | "rupture" | "whirlwind" | "blade_flurry" | "sweeping_strikes"
        )
    {
        tags = tags | T::MELEE;
    }
    if def.kind == Kind::Bolt || def.key == "searing_totem" {
        tags = tags | T::PROJECTILE;
    }
    if matches!(
        def.kind,
        Kind::Nova | Kind::Cone | Kind::Ground | Kind::GroundSelf | Kind::Aura
    ) || (def.radius > 0.0
        && matches!(
            def.kind,
            Kind::Bolt | Kind::Strike | Kind::Melee | Kind::Totem | Kind::Heal
        ))
        || matches!(def.key, "living_bomb" | "seed_of_corruption")
    {
        tags = tags | T::AREA;
    }
    if def.kind == Kind::Chain {
        tags = tags | T::CHAIN;
    }
    if def.dot > 0.0 || def.kind == Kind::Dot {
        tags = tags | T::DOT;
    }
    if def.duration > 0.0
        || matches!(
            def.effect,
            super::Effect::Slow(..)
                | super::Effect::Weaken(..)
                | super::Effect::Root(_)
                | super::Effect::Stun(_)
                | super::Effect::Fear(_)
        )
    {
        tags = tags | T::DURATION;
    }
    if def.kind == Kind::Totem || def.totem.0 != 0 {
        tags = tags | T::TOTEM;
    }
    if matches!(def.key, "explosive_trap" | "frost_trap") {
        tags = tags | T::TRAP;
    }
    if matches!(def.kind, Kind::Aura | Kind::Orbit) {
        tags = tags | T::AURA;
    }
    if matches!(def.kind, Kind::Shield | Kind::Immune | Kind::Thorns) {
        tags = tags | T::GUARD;
    }
    if def.kind == Kind::Heal {
        tags = tags | T::HEAL;
    }
    super::super::support_spells::buff_tags(def, tags)
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Filter {
    pub all: Tags,
    pub any: Tags,
    pub exclude: Tags,
    pub skill: Option<&'static str>,
}
impl Filter {
    /// Proc filters treat physical spells and magical melee skills as weapon skill hits.
    /// Ordinary damage/stat modifiers keep the authored tags through `matches`.
    pub fn matches_hit(self, key: &str, tags: Tags) -> bool {
        self.matches(
            key,
            if weapon_hit(tags) {
                tags | Tags::ATTACK
            } else {
                tags
            },
        )
    }
    pub const fn requiring(all: Tags) -> Self {
        Self {
            all,
            any: Tags(0),
            exclude: Tags(0),
            skill: None,
        }
    }
    pub fn matches(self, key: &str, tags: Tags) -> bool {
        tags.contains(self.all)
            && (self.any.0 == 0 || tags.intersects(self.any))
            && !tags.intersects(self.exclude)
            && self.skill.is_none_or(|s| s == key)
    }
}
