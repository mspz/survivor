//! Specialty points (SP): account-wide meta progression. Every character's experience earns SP
//! (1 XP = 0.1 SP, raised by the achievements' bonus), spent in promotion trees whose ranks
//! apply to every run. A guest sends its ranks with its save; the host applies them to its run.
use super::spell_engine::{Change, Filter, Modifier, Tags};
use super::{achievements::Achievements, data, talents, Rng, Run, Stats};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

mod trees;
pub(super) mod ui;
pub(super) use trees::{cost, find, Effect, Promotion, Unlock, PROMOTIONS};

const FILE: &str = "survivors-specialty.toml";
const SOURCE: u64 = 0x5200_0000;
pub(super) const SP_PER_XP: f64 = 0.1;
/// The reroll, banish and skip uses a run starts with before any rank.
const BASE_DRAFT_USES: u8 = 5;

pub(super) type Ranks = BTreeMap<String, u8>;

#[derive(Default, Serialize, Deserialize)]
struct Save {
    version: u32,
    /// Fractional, so small gains add up.
    #[serde(default)]
    earned: f64,
    /// The experience already converted, per character id.
    #[serde(default)]
    credited: BTreeMap<String, u64>,
    #[serde(default)]
    ranks: Ranks,
}

#[derive(Resource)]
pub(super) struct Specialty {
    save: Save,
    pub revision: u64,
    dirty: bool,
    /// The ranks' hero effects, kept in step with the ranks.
    applied: Applied,
}

impl Default for Specialty {
    fn default() -> Self {
        let save = crate::local_state::home()
            .and_then(|home| crate::local_state::read_to_string(&home.join(FILE)).ok())
            .and_then(|text| match toml::from_str::<Save>(&text) {
                Ok(save) => Some(save),
                Err(error) => {
                    warn!("Ignoring {FILE}: {error}");
                    None
                }
            })
            .unwrap_or_default();
        let applied = effects(&save.ranks).0;
        Self {
            save,
            revision: 1,
            dirty: false,
            applied,
        }
    }
}

/// `WOW_SURVIVORS_UNLOCK_ALL=1` opens every promotion, to try the late nodes.
fn unlock_all() -> bool {
    crate::webenv::var("WOW_SURVIVORS_UNLOCK_ALL").is_some_and(|v| v != "0")
}

pub(super) fn unlocked(promotion: &Promotion, achievements: &Achievements) -> bool {
    match promotion.unlock {
        Unlock::Always => true,
        _ if unlock_all() => true,
        Unlock::Campaign => data::CAMPAIGN
            .iter()
            .all(|map| achievements.counter(&format!("map_wins:{map}")) > 0),
        Unlock::Future(_) => false,
    }
}

impl Specialty {
    pub fn rank(&self, id: &str) -> u8 {
        self.save.ranks.get(id).copied().unwrap_or(0)
    }
    pub fn earned(&self) -> u64 {
        self.save.earned.max(0.0) as u64
    }
    pub fn spent(&self) -> u64 {
        spent(&self.save.ranks)
    }
    pub fn available(&self) -> u64 {
        self.earned().saturating_sub(self.spent())
    }
    pub fn ranks(&self) -> &Ranks {
        &self.save.ranks
    }
    /// This player's own effects, for what stays on their machine: talents, runes, crafting.
    pub fn applied(&self) -> &Applied {
        &self.applied
    }
    pub fn buy(&mut self, id: &str, achievements: &mut Achievements) -> Result<(), String> {
        let (promotion, node) = find(id).ok_or("Unknown node")?;
        if !unlocked(promotion, achievements) {
            return Err(format!("{} is locked", promotion.name));
        }
        let rank = self.rank(id);
        if rank >= node.ranks {
            return Err("Already at its top rank".into());
        }
        let price = cost(promotion, node, rank);
        if self.available() < price {
            return Err(format!("Needs {} SP", super::achievements::group(price)));
        }
        self.save.ranks.insert(id.to_owned(), rank + 1);
        achievements.add("sp_spent", price);
        self.changed();
        Ok(())
    }
    /// Refund a whole promotion, for free.
    pub fn reset(&mut self, promotion: &Promotion) {
        for node in promotion.nodes {
            self.save.ranks.remove(node.id);
        }
        self.changed();
    }
    fn changed(&mut self) {
        self.revision += 1;
        self.dirty = true;
        self.applied = effects(&self.save.ranks).0;
    }
    fn save(&mut self) {
        self.dirty = false;
        self.save.version = 1;
        let Some(home) = crate::local_state::home() else {
            return;
        };
        match toml::to_string(&self.save) {
            Ok(text) => {
                if let Err(error) = crate::local_state::write_queued(&home.join(FILE), &text) {
                    warn!("Could not save specialty points: {error}");
                }
            }
            Err(error) => warn!("Could not encode specialty points: {error}"),
        }
    }
    /// Convert a character's new experience. Its whole past experience counts the first time
    /// it is seen, so the points arrive for every character already played.
    fn credit(&mut self, character: u64, xp: u64, bonus: f32) {
        let key = character.to_string();
        let done = self.save.credited.get(&key).copied().unwrap_or(0);
        if xp <= done {
            if xp < done {
                self.save.credited.insert(key, xp);
            }
            return;
        }
        let gained = (xp - done) as f64 * SP_PER_XP * (1.0 + f64::from(bonus.max(0.0)));
        self.save.earned += gained;
        self.save.credited.insert(key, xp);
        self.revision += 1;
        self.dirty = true;
    }
}

fn spent(ranks: &Ranks) -> u64 {
    ranks
        .iter()
        .filter_map(|(id, &rank)| {
            let (promotion, node) = find(id)?;
            Some((0..rank.min(node.ranks)).map(|r| cost(promotion, node, r)).sum::<u64>())
        })
        .sum()
}

/// The ranks as a guest sends them: `id:rank` pairs.
pub(super) fn encode(ranks: &Ranks) -> String {
    ranks
        .iter()
        .filter(|(_, &r)| r > 0)
        .map(|(id, r)| format!("{id}:{r}"))
        .collect::<Vec<_>>()
        .join(",")
}
pub(super) fn decode(text: &str) -> Ranks {
    text.split(',')
        .filter_map(|pair| {
            let (id, rank) = pair.split_once(':')?;
            let (_, node) = find(id)?;
            Some((id.to_owned(), rank.parse::<u8>().ok()?.min(node.ranks)))
        })
        .collect()
}

/// The hero-side effects of the ranks; the spell effects are spell modifiers.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Applied {
    pub version: Option<u64>,
    pub max_health: f32,
    pub movement: f32,
    pub pickup: f32,
    pub experience: f32,
    pub armour: f32,
    pub regeneration: f32,
    pub resistance: f32,
    pub damage_reduction: f32,
    pub gold: f32,
    pub magic_find: f32,
    pub luck: f32,
    /// Extra reroll, banish and skip uses.
    pub draft: [u8; 3],
    /// Multi-crits are open (Cascade).
    pub multi_unlocked: bool,
    /// Added to the third of the crit chance each further multi-crit rolls with.
    pub multi_crit: f32,
    pub multi_chain: u8,
    /// Added to each multi-crit's damage share (half the hit at base).
    pub multi_power: f32,
    pub rune_upgrade: f32,
    pub rune_tier: f32,
    pub craft_luck: f32,
    pub talent_points: u8,
    pub starting_cards: u8,
}

impl Applied {
    pub fn apply_stats(&self, s: &mut Stats) {
        s.max_hp *= 1.0 + self.max_health;
        s.speed *= 1.0 + self.movement;
        s.magnet *= 1.0 + self.pickup;
        s.growth += self.experience;
        s.armor *= 1.0 + self.armour;
        s.regen += s.max_hp * self.regeneration;
        s.resistances.add_all(self.resistance * 100.0);
        s.damage_reduction += self.damage_reduction;
    }
}

fn change(effect: Effect, v: f32) -> Option<Change> {
    Some(match effect {
        Effect::Damage => Change::IncreasedDamage(v),
        Effect::CritChance => Change::CriticalChance(v),
        Effect::CritDamage => Change::CriticalMultiplier(v),
        Effect::Cooldown => Change::IncreasedCooldownRecovery(v),
        Effect::Area => Change::IncreasedArea(v),
        Effect::Duration => Change::IncreasedDuration(v),
        Effect::AttackSpeed => Change::IncreasedAttackSpeed(v),
        Effect::BossDamage => Change::DamageAgainstBosses(v),
        Effect::LifeLeech => Change::LifeLeech(v),
        Effect::Dodge => Change::DodgeChance(v),
        Effect::ProjectileChance => Change::ProjectileChance(v),
        Effect::ChainChance => Change::ChainChance(v),
        Effect::RecastChance => Change::RecastChance(v),
        Effect::DotDamage => Change::IncreasedDotDamage(v),
        Effect::ArmourPenetration => Change::ArmourPenetration(v),
        Effect::MeleeTargetChance => Change::MeleeTargetChance(v),
        Effect::FinalDamage => Change::MoreDamage(v),
        Effect::ForkChance => Change::ForkChance(v),
        Effect::Haste => Change::MoreActionSpeed(v),
        Effect::LowLifeDamage => Change::DamageAgainstLowLife(v),
        Effect::FullLifeDamage => Change::DamageAgainstFullLife(v),
        Effect::IgniteChance => Change::IgniteChance(v),
        Effect::BleedChance => Change::BleedChance(v),
        Effect::BuffEffect => Change::IncreasedBuffEffect(v),
        _ => return None,
    })
}

/// The ranks' hero effects and spell modifiers.
pub(super) fn effects(ranks: &Ranks) -> (Applied, Vec<Modifier>) {
    let mut applied = Applied::default();
    let mut modifiers = Vec::new();
    for (id, &rank) in ranks {
        let Some((_, node)) = find(id) else {
            continue;
        };
        let v = node.value * f32::from(rank.min(node.ranks));
        match node.effect {
            Effect::MaxHealth => applied.max_health += v,
            Effect::Movement => applied.movement += v,
            Effect::Pickup => applied.pickup += v,
            Effect::Experience => applied.experience += v,
            Effect::Armour => applied.armour += v,
            Effect::Regeneration => applied.regeneration += v,
            Effect::Resistance => applied.resistance += v,
            Effect::DamageReduction => applied.damage_reduction += v,
            Effect::GoldFind => applied.gold += v,
            Effect::MagicFind => applied.magic_find += v,
            Effect::CardLuck => applied.luck += v,
            Effect::Rerolls => applied.draft[0] += v as u8,
            Effect::Banishes => applied.draft[1] += v as u8,
            Effect::Skips => applied.draft[2] += v as u8,
            Effect::MultiCritChance => applied.multi_crit += v,
            Effect::MultiCritChain => applied.multi_chain += v as u8,
            Effect::MultiCritUnlock => applied.multi_unlocked = true,
            Effect::MultiCritPower => applied.multi_power += v,
            Effect::RuneUpgrade => applied.rune_upgrade += v,
            Effect::RuneTier => applied.rune_tier += v,
            Effect::CraftLuck => applied.craft_luck += v,
            Effect::TalentPoints => applied.talent_points += v as u8,
            Effect::StartingCards => applied.starting_cards += v as u8,
            Effect::MultiTarget => {
                for change in [
                    Change::ProjectileChance(v),
                    Change::MeleeTargetChance(v),
                    Change::ChainChance(v),
                ] {
                    modifiers.push(Modifier {
                        source: SOURCE,
                        filter: Filter::requiring(Tags(0)),
                        change,
                    });
                }
            }
            effect => {
                if let Some(change) = change(effect, v) {
                    modifiers.push(Modifier {
                        source: SOURCE,
                        filter: Filter::requiring(Tags(0)),
                        change,
                    });
                }
            }
        }
    }
    (applied, modifiers)
}

/// Feed the ranks into a run: its hero effects, its spell modifiers and, before it starts, its
/// draft uses.
pub(super) fn apply(ranks: &Ranks, run: &mut Run, version: u64) {
    let (mut applied, modifiers) = effects(ranks);
    applied.version = Some(version);
    if run.elapsed == 0.0 {
        let extra = applied.draft;
        run.draft_uses = std::array::from_fn(|i| BASE_DRAFT_USES.saturating_add(extra[i]));
        // Head Start, granted once per run whatever the ranks become.
        let head_start = u32::from(applied.starting_cards);
        if run.specialty.starting_cards < applied.starting_cards {
            run.stored_cards = run
                .stored_cards
                .saturating_add(head_start - u32::from(run.specialty.starting_cards));
        }
    }
    run.specialty = applied;
    run.replace_spell_modifiers(SOURCE, modifiers);
}

/// A critical strike's multi-crits, once Cascade is open: the hit crits again with a third of
/// `crit` (the crit chance), the next with a third of that, and so on up to the chain length.
/// Each adds half of the hit (more with Overkill). Returns the factor on the critical damage.
pub(super) fn multi_crit(applied: &Applied, crit: f32, rng: &mut Rng) -> f32 {
    if !applied.multi_unlocked {
        return 1.0;
    }
    let share = (1.0 / 3.0 + applied.multi_crit).clamp(0.0, 0.9);
    let power = 0.5 + applied.multi_power.max(0.0);
    let mut chance = crit.clamp(0.0, 1.0) * share;
    let mut factor = 1.0;
    for _ in 0..1 + u32::from(applied.multi_chain) {
        if rng.f32() >= chance {
            break;
        }
        factor += power;
        chance *= share;
    }
    factor
}

/// Gold earned in a guest's run, raised by its own gold find.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn gold(world: &World, money: u64) -> u64 {
    world.get_resource::<Specialty>().map_or(money, |specialty| {
        let gold = effects(&specialty.save.ranks).0.gold;
        (money as f64 * f64::from(1.0 + gold.max(0.0))).round() as u64
    })
}

fn credit(
    account: Option<Res<talents::Account>>,
    achievements: Res<Achievements>,
    mut specialty: ResMut<Specialty>,
    phase: Res<State<super::Phase>>,
) {
    if let Some(account) = account.as_deref() {
        if let Some(id) = account.character {
            specialty.credit(id, account.xp, achievements.bonus());
        }
    }
    if specialty.dirty && *phase.get() != super::Phase::Playing {
        specialty.save();
    }
}

fn sync(specialty: Res<Specialty>, mut run: ResMut<Run>) {
    if run.specialty.version != Some(specialty.revision) {
        apply(&specialty.save.ranks, &mut run, specialty.revision);
    }
}

/// Hand this player's own bonuses to the systems that live outside the run.
fn share(
    specialty: Res<Specialty>,
    account: Option<ResMut<talents::Account>>,
    runes: Option<ResMut<super::runes::Runes>>,
    inventory: Option<ResMut<super::items::Inventory>>,
) {
    let applied = specialty.applied();
    if let Some(mut account) = account {
        let points = u32::from(applied.talent_points);
        if account.bonus_points != points {
            account.bonus_points = points;
        }
    }
    if let Some(mut runes) = runes {
        if runes.config.upgrade_bonus != applied.rune_upgrade {
            runes.config.upgrade_bonus = applied.rune_upgrade;
            runes.revision += 1;
        }
    }
    if let Some(mut inventory) = inventory {
        if inventory.config.craft_luck != applied.craft_luck {
            inventory.config.craft_luck = applied.craft_luck;
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Specialty>()
        .add_systems(Update, (credit, sync.run_if(super::multiplayer::authoritative), share).chain())
        .add_plugins((ui::plugin, ui::banner_plugin));
}

#[cfg(test)]
mod tests;
