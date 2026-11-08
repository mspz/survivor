//! Authoritative simulation and lifecycle, called in process for solo play.
use super::*;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(PostUpdate, forms::sync.run_if(multiplayer::authoritative));
    app.init_resource::<Run>()
        .init_resource::<boss_spells::Warnings>()
        .init_resource::<support_spells::Debuffs>()
        .init_resource::<talents::Account>()
        .init_resource::<abilities::PendingHits>()
        .init_resource::<enemies::Spawner>()
        .init_resource::<training::Arena>()
        .add_systems(
            OnEnter(Phase::Playing),
            training::reset.run_if(|run: Res<Run>| run.elapsed == 0.0),
        )
        .add_message::<abilities::Hit>()
        .add_message::<enemies::Loot>()
        .add_message::<healing::Credit>()
        .init_resource::<Rng>()
        .init_resource::<session::Sessions>()
        .init_resource::<session::ServerInput>()
        .add_systems(
            OnExit(Phase::Playing),
            talents::flush.run_if(multiplayer::authoritative),
        )
        .add_systems(
            Update,
            (session::receive, mobility::cast_mobility, hero::drive_hero)
                .chain()
                .run_if(multiplayer::authoritative)
                .in_set(session::Stage::Simulate),
        )
        .add_systems(
            Update,
            talents::credit
                .after(hero::hero_vitals)
                .run_if(multiplayer::authoritative),
        )
        .add_systems(
            Update,
            (
                enemies::spawn_enemies,
                training::tick,
                (enemies::scale_enemies, boss_spells::tick, special_mobs::tick, enemies::protect_spawns).chain(),
                enemies::move_enemies,
                talent_runtime::tick,
                items::legendary::events::tick,
                items::legendary::periodic::tick_world,
                items::legendary::effects::tick,
                abilities::cast_abilities,
                items::legendary::movement::tick,
                abilities::tick_areas,
                items::legendary::casting::tick,
                abilities::apply_hits,
                talent_runtime::support,
                enemies::tick_marks,
                enemies::drop_loot,
                enemies::collect_gems,
                hero::hero_vitals,
                healing::credit,
                (revival::tick, combat_ui::track_effects).chain(),
            )
                .chain()
                .after(WorldStage::Input)
                .run_if(multiplayer::simulating)
                .run_if(multiplayer::authoritative)
                // Entering a battleground raises its loading screen: the run waits behind it.
                .run_if(|l: Res<crate::loading_screen::LoadingScreen>| !l.covering()),
        )
        .add_systems(
            Update,
            enemies::reap_dead
                .after(WorldStage::Input)
                .run_if(multiplayer::authoritative),
        )
        .add_systems(Update, enemies::still_gems.after(WorldStage::Input))
        .add_systems(
            OnEnter(Phase::Playing),
            (unpause, talents::prepare).run_if(multiplayer::authoritative),
        )
        .add_systems(
            OnEnter(Phase::LevelUp),
            (pause, hud::roll_offer).run_if(multiplayer::authoritative),
        )
        .add_systems(
            OnEnter(Phase::Paused),
            pause.run_if(multiplayer::authoritative),
        )
        .add_systems(
            OnEnter(Phase::CharacterSheet),
            pause.run_if(multiplayer::authoritative),
        )
        .add_systems(
            OnEnter(Phase::GameOver),
            (unpause, progression::finish_run).run_if(multiplayer::authoritative),
        )
        .add_systems(
            OnEnter(Phase::Menu),
            (unpause, reset_run, characters::logout)
                .chain()
                .run_if(multiplayer::authoritative),
        )
        .add_systems(
            OnEnter(Phase::Hub),
            unpause.run_if(multiplayer::authoritative),
        )
        .add_systems(
            OnEnter(Phase::Hub),
            (reset_run, characters::arrive)
                .chain()
                .run_if(multiplayer::authoritative)
                .run_if(|selection: Res<Selection>, run: Res<Run>| {
                    !selection.in_hub || run.abilities.is_empty()
                }),
        );
}

/// An ability the hero has, with its own cooldown.
pub(super) struct Owned {
    pub def: &'static AbilityDef,
    pub level: u8,
    pub cd: f32,
}

/// The hero's derived numbers, from the base and the passives.
#[derive(Clone, Copy)]
pub(super) struct Stats {
    pub attack_power: f32,
    pub spell_power: f32,
    pub attack_speed: f32,
    pub might: f32,
    pub level_damage: f32,
    pub max_hp: f32,
    pub speed: f32,
    pub regen: f32,
    pub haste: f32,
    pub area: f32,
    pub magnet: f32,
    pub growth: f32,
    pub armor: f32,
    pub resistances: resistances::Resistances,
    pub damage_reduction: f32,
    pub crit: f32,
}

pub(super) const BASE_HP: f32 = 120.0;
pub(super) const BASE_SPEED: f32 = 7.0;
pub(super) const BASE_MAGNET: f32 = 4.5;

/// One run's state.
#[derive(Resource)]
pub(super) struct Run {
    pub(super) draft_uses: [u8; 3],
    pub(super) banished_cards: std::collections::HashSet<(bool, &'static str)>,
    pub(super) support: super::support_spells::Runtime,
    pub(super) extra_spells: super::extra_spells::Runtime,
    pub owner: Option<u64>,
    pub(super) balance: balance::Config,
    pub(super) expeditions: expedition_config::Config,
    pub(super) expedition_map: usize,
    pub(super) expedition_players: usize,
    pub effect_meter: combat_ui::EffectMeter,
    pub meter: combat_ui::Meter,
    pub boss_meter: combat_ui::Meter,
    pub healing_meter: combat_ui::Meter,
    pub taken_meter: combat_ui::Meter,
    pub taken_source: &'static str,
    pub spell_modifiers: spell_engine::Modifiers,
    pub(super) equipment_version: Option<(Option<u64>, u64)>,
    pub(super) equipment_hp: f32,
    pub(super) equipment_armour: f32,
    pub(super) weapon_damage: items::weapons::Loadout,
    pub(super) equipment: items::Bonuses,
    /// The equipped account rune's hero-stat effects (its spell effects are spell modifiers).
    pub(super) rune: runes::Applied,
    pub(super) item_runtime: items::combat::Runtime,
    pub(super) recasts: spell_engine::Recasts,
    pub(super) talent_build: [[u8; talents::NODES]; 3],
    pub(super) account_credited: u64,
    pub(super) meta_level: u32,
    pub(super) meta_kill_penalty: u64,
    pub class: &'static ClassDef,
    pub race: u8,
    pub elapsed: f32,
    pub kills: u32,
    pub level: u32,
    pub xp: f32,
    pub hp: f32,
    pub abilities: Vec<Owned>,
    /// Skills switched off on the training ground (right click on their icon); they do not cast.
    pub(super) disabled_spells: std::collections::HashSet<&'static str>,
    pub(super) starter_spell: Option<&'static str>,
    pub passives: Vec<(&'static PassiveDef, u64)>,
    pub shield: f32,
    pub shield_t: f32,
    pub immune_t: f32,
    pub thorns: f32,
    pub thorns_dot: Option<spell_engine::PeriodicSnapshot>,
    pub thorns_t: f32,
    pub thorns_spell: u32,
    pub heal_rate: f32,
    pub heal_t: f32,
    pub heal_credit: Option<(Option<u64>, &'static str)>,
    pub heal_triggered: bool,
    pub(super) talent_runtime: talent_runtime::Runtime,
    pub(super) proc_runtime: super::procs::Runtime,
    pub(super) healer_buffs: healer_buffs::State,
    pub(super) legendary: items::legendary::Runtime,
    /// The class mobility's cooldown and running effect.
    pub(super) mobile: mobility::MobilityState,
    pub pending_levels: u32,
    pub stored_cards: u32,
    pub auto_draw_cards: bool,
    /// Reward categories alternate per confirmed pick, independent of queued XP levels.
    pub(super) next_passive: bool,
    pub bosses: u32,
    pub rings: u32,
    pub won: bool,
    /// Seconds since the hero fell, while the death plays out.
    pub dead_t: Option<f32>,
    pub(super) revival: revival::State,
    pub next_guid: u64,
    pub damage_done: f64,
    /// Bumped whenever abilities or passives change, so the HUD rebuilds the icon rows once.
    pub version: u32,
    /// A centre-screen announcement and its seconds left.
    pub banner: Option<(String, f32)>,
}

impl Default for Run {
    fn default() -> Self {
        Self {
            support: default(),
            draft_uses: [5; 3],
            banished_cards: default(),
            extra_spells: default(),
            owner: None,
            balance: balance::Config::default(),
            expeditions: expedition_config::Config::default(),
            expedition_map: data::CAMPAIGN[0],
            expedition_players: 1,
            effect_meter: default(),
            meter: combat_ui::Meter::default(),
            boss_meter: default(),
            healing_meter: default(),
            taken_meter: default(),
            taken_source: "Damage taken",
            spell_modifiers: spell_engine::Modifiers::default(),
            equipment_version: None,
            equipment_hp: 0.0,
            equipment_armour: 0.0,
            weapon_damage: items::weapons::Loadout::default(),
            equipment: items::Bonuses::default(),
            rune: runes::Applied::default(),
            item_runtime: items::combat::Runtime::default(),
            recasts: spell_engine::Recasts::default(),
            talent_build: [[0; talents::NODES]; 3],
            account_credited: 0,
            meta_level: 1,
            meta_kill_penalty: 0,
            class: &data::CLASSES[0],
            race: 0,
            elapsed: 0.0,
            kills: 0,
            level: 1,
            xp: 0.0,
            hp: BASE_HP,
            abilities: Vec::new(),
            disabled_spells: default(),
            starter_spell: None,
            passives: Vec::new(),
            shield: 0.0,
            shield_t: 0.0,
            immune_t: 0.0,
            thorns: 0.0,
            thorns_dot: None,
            thorns_t: 0.0,
            thorns_spell: 0,
            heal_rate: 0.0,
            heal_t: 0.0,
            heal_credit: None,
            heal_triggered: false,
            talent_runtime: default(),
            proc_runtime: default(),
            healer_buffs: default(),
            legendary: default(),
            mobile: mobility::MobilityState::default(),
            pending_levels: 0,
            stored_cards: 0,
            auto_draw_cards: true,
            next_passive: false,
            bosses: 0,
            rings: 0,
            won: false,
            dead_t: None,
            revival: default(),
            next_guid: 1,
            damage_done: 0.0,
            version: 0,
            banner: None,
        }
    }
}

impl Run {
    pub fn resolve_spell(&self, def: &AbilityDef, level: u8) -> spell_engine::Resolved {
        self.resolve_spell_with_weapon_roll(def, level, 0.5)
    }
    pub fn stats(&self) -> Stats {
        let (level_health, level_damage) = level_stats::bonuses(self.level, self.meta_level);
        let mut s = Stats {
            attack_power: self.support.power(self.elapsed, "battle_shout")
                + self.support.power(self.elapsed, "blessing_of_might")
                + self.support.power(self.elapsed, "strength_of_earth_totem")
                + self.support.power(self.elapsed, "savage_roar"),
            spell_power: self.support.power(self.elapsed, "arcane_intellect"),
            attack_speed: self.support.power(self.elapsed, "slice_and_dice"),
            might: 1.0,
            level_damage,
            max_hp: BASE_HP + self.equipment_hp + level_health,
            speed: BASE_SPEED,
            regen: 0.0,
            haste: 0.0,
            area: 1.0,
            magnet: BASE_MAGNET,
            growth: 1.0,
            armor: self.equipment_armour,
            resistances: self.equipment.resistances,
            damage_reduction: 0.0,
            crit: 0.05,
        };
        // Tanky classes start tougher, cloth starts quicker.
        match self.class.id {
            1 | 2 => s.max_hp += 40.0,
            7 | 11 | 3 | 4 => s.max_hp += 15.0,
            _ => s.speed += 0.5,
        }
        s.speed *= 1.0 + self.equipment.movement;
        s.regen += self.equipment.regeneration;
        s.magnet *= 1.0 + self.equipment.pickup;
        s.growth += self.equipment.experience;
        s.speed *= 1.0 + self.rune.movement;
        s.growth += self.rune.experience;
        let mut passive_regeneration = 0.0;
        for &(p, level) in &self.passives {
            let data::PassiveEffect::Stat(stat, per_level) = p.effect else {
                continue;
            };
            let v = per_level * level as f32;
            match stat {
                Stat::AllResistance => s.resistances.add_all(v),
                Stat::DamageReduction => s.damage_reduction += v,
                Stat::Might => s.might += v,
                Stat::MaxHp => s.max_hp *= 1.0 + v,
                Stat::Speed => s.speed *= 1.0 + v,
                Stat::Regen => passive_regeneration += v,
                Stat::Haste => s.haste += v,
                Stat::Area => s.area += v,
                Stat::Magnet => s.magnet *= 1.0 + v,
                Stat::Growth => s.growth += v,
                Stat::Armor => s.armor += v,
                Stat::Crit => s.crit += v,
            }
        }
        talents::apply_stats(self, &mut s);
        s.speed *= 1.0 + self.healer_buffs.bonuses.speed;
        s.resistances.add_all(self.healer_buffs.bonuses.resistance);
        s.armor += self.support.power(self.elapsed, "devotion_aura")
            + self.support.power(self.elapsed, "mark_of_the_wild")
            + self.support.power(self.elapsed, "demon_armor");
        s.resistances
            .add_all(self.support.power(self.elapsed, "mark_of_the_wild") * 0.25);
        s.speed *= 1.0
            + self.support.power(self.elapsed, "aspect_of_the_pack")
            + self.support.power(self.elapsed, "stampeding_roar");
        s.regen += s.max_hp * passive_regeneration;
        s
    }

    pub fn replace_spell_modifiers(
        &mut self,
        source: u64,
        modifiers: impl IntoIterator<Item = spell_engine::Modifier>,
    ) {
        self.spell_modifiers.replace_source(source, modifiers);
        self.version = self.version.wrapping_add(1);
    }
    pub fn guid(&mut self) -> u64 {
        self.next_guid += 1;
        self.next_guid
    }
    pub fn heal(&mut self, amount: f32) -> f32 {
        let healed = self.recover(amount);
        self.healing_meter
            .record_hit("self_healing", self.elapsed, healed, false, false);
        healed
    }
    /// Incoming healing; the caster owns its meter attribution.
    pub fn recover(&mut self, amount: f32) -> f32 {
        if self.hp <= 0.0 || self.dead_t.is_some() || !amount.is_finite() {
            return 0.0;
        }
        let recovered = (amount.max(0.0)
            * (1.0
                + self.equipment.healing
                + self.support.power(self.elapsed, "demon_armor") / 400.0)
                .max(0.0))
        .min((self.stats().max_hp - self.hp).max(0.0));
        self.hp += recovered;
        recovered
    }
}

/// xorshift64*: the mode's dice, seeded from the clock.
#[derive(Resource)]
pub(super) struct Rng(pub(super) u64);

impl Default for Rng {
    fn default() -> Self {
        let seed = web_time::SystemTime::now()
            .duration_since(web_time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        Self(seed | 1)
    }
}

impl Rng {
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// Uniform in `[0, 1)`.
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f32()
    }
    pub fn index(&mut self, len: usize) -> usize {
        (self.next_u64() % len.max(1) as u64) as usize
    }
}

pub(super) fn pause(mut time: ResMut<Time<Virtual>>, role: Option<Res<multiplayer::Role>>) {
    if role.is_some_and(|r| *r != multiplayer::Role::Solo) {
        time.unpause();
    } else {
        time.pause();
    }
}

pub(super) fn unpause(mut time: ResMut<Time<Virtual>>) {
    time.unpause();
}

/// Back at the menu: everything the last run spawned goes, and the run starts over.
pub(super) fn reset_run(
    mut commands: Commands,
    spawned: Query<Entity, With<RunEntity>>,
    mut run: ResMut<Run>,
    mut pending: ResMut<abilities::PendingHits>,
    mut spawner: ResMut<enemies::Spawner>,
    mut selection: ResMut<Selection>,
) {
    for e in &spawned {
        commands.entity(e).despawn();
    }
    // Guids stay unique across runs: caches keyed by guid may outlive a despawn by a frame.
    let next_guid = run.next_guid;
    *run = Run {
        next_guid,
        ..Run::default()
    };
    pending.0.clear();
    pending.1.clear();
    *spawner = enemies::Spawner::default();
    selection.sheet_open = false;
    selection.bags_open = false;
    selection.ui_captured = false;
    selection.pointer_captured = false;
    selection.in_hub = true;
    selection.talents_open = false;
    // The preview hero is respawned for the look still picked.
    selection.version += 1;
}
