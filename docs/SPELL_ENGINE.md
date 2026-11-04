# Survivors spell engine

`crates/benilla-app/src/survivors/spell_engine/` owns spell tags, modifier stacking,
resolved cast values, critical hits, damage resolution and projectile target graphs.
It has no Bevy systems, asset loading, real-time clock or visual effects. The target planner
uses Bevy's `Entity` and `Vec3` as identifiers and coordinates.

`abilities.rs` is the ECS adapter: it gathers living enemies, executes plans, schedules
missile impacts and link visuals, creates ground areas/totems, and changes actor state.
The HUD resolves spells through the same engine. Talent branch power is already supplied
through the modifier API; extra projectiles and other mechanics require an authored source.

## Tags

All 73 auto-cast abilities and 9 mobility skills are classified in `spell_engine/tags.rs`, with a test that rejects
missing schools. School classifications follow the local 1.12.1 `Spell.dbc`; Nature lightning
skills also have a Lightning tag. Cold corresponds to WoW's Frost school.

Tags describe the mode's mechanics, not just its visual effects. A Volley is an Attack,
Area and Arcane ability implemented as a ground area, not a collection of targetable projectiles.
A totem-summoning ability is tagged Totem; it is not itself a Projectile.

| Axis | Tags |
|---|---|
| Origin | Attack or Spell |
| Mechanics | Melee, Projectile, Area, Chain, Damage over time, Duration, Totem, Trap, Aura, Guard, Healing, Utility, Movement |
| Damage school | Physical, Fire, Cold, Nature, Arcane, Holy, Shadow; Lightning is a Nature subset |

`Filter.all` requires every selected tag. `Filter.any` requires at least one when nonempty.
`Filter.exclude` vetoes a match. `Filter.skill` optionally restricts the modifier to one ability key.
Mechanic compatibility is enforced even for an unfiltered modifier: extra projectiles never
increase a melee attack's target count. Extra melee targets apply to finite-target melee skills;
melee area skills already affect every enemy in their area.

## Adding a modifier source

Use `Run::replace_spell_modifiers` so tooltip caches are invalidated. Each equipment slot,
passive or talent source needs a stable ID; replacing it removes its previous rolls. IDs
`0x1000_0000..=0x1000_0002` are reserved for the active class's three talent branches.
Run passives reserve `0x2000_0000 + passive_index`. `passives::sync` replaces these sources
after a pick; generic character stats are not duplicated in the engine. Talent descriptions
and modifier generation share `talent_effects.rs`.

The run reward pool previews each proposed modifier against the currently owned skills and
the learned mobility skill, using all current modifier sources. A modifier is offered only
when the next rank changes a resolved skill. This enforces tags, mechanic compatibility and
caps, including not offering Fire damage to a physical-only build or critical chance to a
DoT-only build. Eligible class-pool spells that have not been learned are not considered.

`passives::legendary` classifies AdditionalProjectiles, AdditionalMeleeTargets, Fork and
AdditionalChains as legendary run rewards, for acquisition and stack upgrades alike.
Each confirmed level-up pick alternates the next offer between active spells (learning or
upgrading) and passives, starting with active spells. The state is stored on the run, not
inferred from its level, so several queued levels alternate correctly. Once no active card
is eligible (all active slots maxed and no new spell can be learned), every offer is passive.
If configuration weights disable an entire kind, offers fall back to the other available kind.

Within that kind, the game rolls rarity, then a card by its configured relative weight.
Default rarity weights are 50 Common, 30 Uncommon, 15 Epic, 5 Legendary. Empty or disabled
categories are omitted and the remaining weights are normalized for each card. For example,
active rewards have no Common cards, so Common receives no weight on an active turn.
Entries do not repeat within one offer. A scarce pool may offer fewer than three distinct
cards; it never mixes active spells and passives to fill the row.

All numerical passives have one variant in every rarity: Common x1, Uncommon x2,
Epic x3, Legendary x4, using the authored value as the Common baseline. Their power
units accumulate on one passive entry and one engine source across mixed rarities.
Drawing one removes all four variants from the current offer. The four coverage
modifiers above remain Legendary only. CriticalMultiplier adds percentage points to
the base 200% critical multiplier and is snapshotted for direct, ground and totem hits.
Spell rewards use these same buckets, with their rank and targeting determining rarity.

```rust,ignore
use super::spell_engine::{Change, Filter, Modifier, Tags};

let filter = Filter {
    all: Tags::ATTACK | Tags::PROJECTILE,
    ..Default::default()
};
run.replace_spell_modifiers(42, [
    Modifier { source: 42, filter, change: Change::AdditionalProjectiles(2) },
    Modifier { source: 42, filter, change: Change::MoreDamage(-0.20) },
]);

// Applies to fire damage from attacks or spells, including its damage-over-time component.
run.replace_spell_modifiers(43, [Modifier {
    source: 43,
    filter: Filter { all: Tags::FIRE, ..Default::default() },
    change: Change::IncreasedDamage(0.30),
}]);

// Unequip or expire this source, without leaving a stale modifier behind.
run.replace_spell_modifiers(42, []);
```

Run passives use 64-bit stack counters with no gameplay rank cap or six-slot restriction. Change::scaled accepts the full stack count, with saturating integer conversion for bounded projectile mechanics. Generic stats and scalable damage bonuses keep growing; compatibility previews still reject bonuses that have reached an engine limit. Fast Learner grants +10% run XP per stack (for example, 20 stacks grant +200%).

## Stacking and execution

- `FlatDamage` is added after rank scaling and before damage multipliers.
- `IncreasedDamage` values add to the existing increased-power pool; each `MoreDamage`
  contributes its own multiplier, `1 + value`. Negative values express reduced/less damage.
- Damage modifiers cannot increase healing or shield absorption. `IncreasedEffect` is the
  explicit power modifier used by existing talents to preserve their utility-skill scaling.
- Increased/More DoT Damage affect only the damage-over-time component of a DoT-tagged skill.
- Additional projectiles, chains and melee targets add. Fork counts add generations.
- Increased Area scales surface area: +100% area multiplies the radius by `sqrt(2)`.
  The legacy `Stats.area` input retains its original radius semantics; current area passives
  and talents use tag-filtered `IncreasedArea` instead.
- Cooldown recovery divides the rank/passive-adjusted cooldown by `1 + recovery`.
  Critical chance is additive and clamped to 0..1.
- Forks happen before chains. A fork creates up to two distinct child targets, with 70%
  inherited damage each. A chain inherits 85% damage per hop. Children inherit DoT scaling.
- Each projectile/chain cast reserves its primary targets and visits each direct target once.
  Splash remains a separate effect and can overlap between impacts, as before.
- Target selection is snapshotted at cast time. This is a homing target graph, not a physical
  projectile collision/piercing simulation. Impact damage and secondary-launch visuals are
  scheduled using travel times. New links use a 12-yard search range; native chain spells keep
  their existing jump range and 0.08-second hop timing.
- Repeat chance has no 100% cap: each whole 100% guarantees one extra cast, and the remainder
  rolls for one additional copy. For example, 130% means two total casts guaranteed, with a 30%
  chance of three; 230% means three guaranteed, with a 30% chance of four.
  Repeats are spaced by at least 0.18 seconds, reselect targets, do not
  reset the original cooldown, and cannot trigger another repeat. Guard, Healing, Aura and
  Totem skills do not repeat. The nine manual mobility skills never repeat either; their duration, cooldown recovery and fear area use the same tag-filtered modifier sources.
- Ground areas and totems snapshot their resolved power, radius, duration and critical chance
  when created. Existing instances are not retroactively changed by later modifier sources.

Per-cast limits are 64 direct hits, 32 added projectiles/melee targets, 16 added chains, and
3 fork generations. Repeat jobs are compressed into one queue entry per owned ability, with
a saturating 64-bit pending count. New base casts merge their repeats into that entry instead
of allocating one entry per copy or dropping copies at the former 32-job limit. At most one
pending copy per ability executes per frame; very large bursts are spread over time.
These bounds are part of the engine contract, independent of item balance.

## Checks

The `survivors::spell_engine::tests` tests cover tag coverage, legacy rank scaling, school
filters, mechanic compatibility, additive/multiplicative stacking, source replacement,
area scaling, chain/fork timings, cycle/work limits, repeats, crits, execute and mitigation.
The existing talent tests verify the real source integration.

## Reward configuration

Edit `config/survivors-rewards.toml`, then restart the game from the repository folder.
No rebuild is required for runtime file changes. The same file is embedded as a fallback
when distributing only the executable. TOML is already a runtime dependency of the client.

- `[rarities]`: relative weights for `common`, `uncommon`, `epic`, `legendary` (50/30/15/5 by default).
- `[spells]`: one stable ability key per spell, weight 1 by default. Its weight applies to both
  acquisition and rank upgrades, within the rarity assigned by the existing rank rules.
- `[passives]`: one stable passive key, weight 1 by default. The weight applies to all four
  numerical rarity variants, and to acquisition/stack upgrades for legendary coverage passives.

A weight of 3 has three times the selection probability of 1 in the same category.
Weights are normalized, not percentages that must sum to 100. Zero disables a card or rarity.
Absent card keys default to 1, including future new cards. Tags, owned spells and engine caps
still filter eligibility before selection. Different rarities of the same numerical passive
cannot appear together. Empty or disabled rarity categories are omitted before the configured weights are normalized. If nothing can be offered, the existing Healthstone recovery card is used.

Loading order (once at application startup):

1. `WOW_REWARDS_CONFIG`, if set, names an explicit file.
2. Native: `config/survivors-rewards.toml` relative to the working directory, if present.
3. `survivors-rewards.toml` in the existing `benilla-config` local-state directory; if absent,
   the game creates it from the embedded default. Browser builds use the same local-state store.
4. Embedded defaults if storage is unavailable.

An invalid or unreadable selected file logs a warning and uses embedded defaults without
rewriting the invalid file. Version must be 1; unknown card keys, negative/non-finite weights,
and an all-zero rarity table are rejected. This prevents typos silently changing drop odds.

Example override from PowerShell:

```powershell
$env:WOW_REWARDS_CONFIG = "C:\path\to\survivors-rewards.toml"
cargo run --release -p warcraft-survivors
```

## Character talent layouts

Equipment uses the same engine: see [ITEMS.md](ITEMS.md) for boss drops, prefix/suffix generation,
the item configuration and per-character inventory. Weapon `IncreasedAttackSpeed` applies only
to `Attack` skills and divides their cooldown separately from cooldown recovery; `FlatDamage`
is added to hit damage before increased/more multipliers. Flat life and armour feed character stats.
`on_hit.rs` resolves target-health bonuses, armour penetration and tag-gated leech/ailment chances.
Item buffs use a separate replaceable engine source; trinket spell casts reuse targeting and hit
resolution, carry a triggered flag to prevent proc chains and keep normal skill cooldowns intact.

Talent progress is scoped to the active character, not the roster. `talents.rs` defines 19 nodes per specialization, seven tiers and three linked milestones. `talent_design.rs` authors the 27 identities and their 81 milestones; `talent_runtime.rs` owns each player's stateful mechanics. All three trees are shown together. V4 saves refund previous layouts while keeping XP. The sanctuary applies previews immediately; starting an expedition copies the build and replaces all three talent modifier sources. Talents are read-only during an expedition. See [TALENTS.md](TALENTS.md).
