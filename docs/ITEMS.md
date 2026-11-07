# Equipment and boss loot

All **21 variants per family** unlock at **1, 3, 6, 9, ... 60**, configured in each family's `tiers`
array in `config/survivors-item-bases.toml`. The highest unlocked tier supplies the base name,
WoW appearance and icon. Armour, weapons and shields progress from simple styles to elaborate
endgame styles; rings, necklaces, trinkets and caster focuses also receive tiered icons.
Shirts and tabards remain cosmetic. All tiers share their family's drop weight and affix pool,
so adding variants does not make that equipment family more common.
Equipping a modeled helmet hides styled hair, including helmets with an empty WoW hair mask;
removing it restores the hairstyle. Facial hair and ears still use the helmet's own masks.

A tier's `base_armour` and `base_damage` describe its power **at its unlock level**. Only the
remaining levels apply the usual 1.015 scaling; the full item-level multiplier is not applied
twice. Affixes continue to scale separately. The family ID remains stable, and the rolled name,
icon, appearance and intrinsic values are saved on each item. Existing saved equipment keeps
its appearance and stats. Custom tables without `tiers` keep their family appearance and
standard intrinsic scaling within the item-level cap.
The former level-100 variants now unlock at 60 with their original intrinsic values, icons
and appearances preserved. New loot is capped at item level 60; level-59 boss chests produce
level-60 items so the final variants are obtainable. Ordinary level-59 monsters still drop
level-59 items. Previously saved items keep their stored values, including older higher-level rolls.

Weapons have intrinsic `base_damage = { min = 6, max = 10 }` ranges in the base TOML, separate
from affixes. They scale with item level like armour and are saved with the item. Older weapons
without this field receive defaults by type; starting weapons also have intrinsic damage.
One value is rolled per attack activation and added flat before increased/more damage and
critical multipliers. Main/off-hand ranges add together for melee attacks; Hunter ranged attacks
use the ranged slot. Staves and intelligence weapons also have ranges, but these only affect
Attack hits: spell damage, healing, absorption and existing damage-over-time values are unchanged.
Tooltips show the weapon's range; the skill sheet shows its applicable range and average damage.

New characters start with equipped Common (white) chest and leg pieces and a class-appropriate
weapon: a two-handed sword for Warriors, a mace for Paladins, a bow for Hunters, two daggers
for Rogues, and a caster staff for Priests, Shamans, Mages, Warlocks and Druids. Armour uses the
class's highest permitted material. These level-one starting items have no affixes or stat
bonuses; they use the existing base appearances. The kit is saved once per new character,
never regenerated on login or after unequipping, and is not added to older characters.

Press **B** (rebindable) or **Bags** to open the independent inventory window, anchored to the
right above the action bar. **C** independently toggles the character sheet. Either or both
may be open; each close button affects only its own window. Combat resumes when all panels close.
Click a bag item to inspect all its affixes, then choose **Equip** and the destination slot.
Click a worn item to inspect or unequip it. Two rings and two trinkets can be worn independently;
Replacing an item from the bag puts the old one into the incoming item's exact bag position,
including with right-click and drag-to-equip. The order is saved. A single item cannot occupy two slots.
Right-click a bag item to equip it into the **first compatible slot**, even if that slot is
occupied and the second is free: Ring 1 / Trinket 1. Right-click worn gear to unequip its
actual slot. Left-drag an item onto a compatible equipment slot to choose the destination.
**Alt + left-click** permanently deletes a bag or equipped item and saves the removal.
Dragging between two occupied compatible slots swaps their items. Drop worn gear into the bag
to unequip it, or drop a bag item onto another bag item to swap their order. Empty bag cells
append to the compact bag list. Outside/incompatible drops, right-click during a drag, closing
the sheet and losing window focus cancel safely. The item remains in its original location
until a valid drop is saved; the cursor icon is only a visual copy.
The Bagnon-style bag window has 112 visible slots (14 columns × 8 rows) with 32-pixel cells per page
and automatically adds pages; loot is never rejected
because the current page is full. Hover an item for its tooltip.
The item name uses its rarity colour; type, base armour/damage, all prefixes and all suffixes
are white. Holding Shift adds a divider and equipment comparison, with all green gains before
all red losses. Select an item for equip actions and hover for its details. Class
lists and stat explanations are omitted; incompatible items show a red warning instead.
Character and inventory windows use Classic MPQ artwork for the paper doll, window background,
border corners/edges, empty bag slots, quickslot frames, red buttons and close buttons.

Every boss drops one item by default. The loot appears as an Elwynn wooden treasure chest at
the boss's death position; approach
within 3 yards to collect it. At the results screen, remaining boss loot is recovered automatically,
including the final boss's item. Each ordinary monster also has a fixed **1 in 2,000 (0.05%)**
chance to drop the same equipment chest, with item level based on that monster's level.
Magic Find affects the item's quality, not this chest chance. Multiplayer personal loot uses
the same chance for each player. The XP magnet collects
XP only. Items still on the floor when abandoning an expedition are left behind.

Loot first chooses its target class: 80% the player class, otherwise uniformly one of the eight other classes. Rarity, a compatible base and compatible affixes are then rolled:

| Rarity | Default weight | Affixes |
|---|---:|---|
| Common | 60 | One primary stat |
| Uncommon | 30 | One primary stat and one secondary stat |
| Epic | 10 | Stamina, one other primary stat and two secondary stats |
| Legendary | 0 | Reserved; not generated or implemented |

Primary stats are stamina, strength, agility and intelligence. One stamina grants one maximum
life. Each strength point grants 1% increased attack damage to Warriors and Paladins; agility
grants the same to Rogues, Hunters, Shamans and Druids. Each intelligence point grants 1% increased
spell damage and spell healing to Mages, Priests, Warlocks, Shamans, Paladins and Druids. Other classes
gain no offensive benefit from these stats.
Primary stats are filtered by the target class and material as described below.
Direct life, flat attack damage and increased damage affixes (including school damage) have
been removed from the default loot pool. Conditional damage, damage over time and all other
mechanics remain secondary. Healing received is separate from spell healing and remains secondary.
Existing saved items keep their rolls and effects; the new allocation applies to new drops.

Affixes are rolled without repeating a `group`, with weighted selection from compatible affixes
and a random value in the configured range. Each item keeps its generated name, icon, rarity,
item level and rolled effects. Item level is metadata for now; ranges come from the loot table.
The target class is saved with the item and also selects class-spell procs; it does not bind the item to that class. Equipment permissions determine who can wear it.

Equipped spell modifiers enter the shared spell engine under one replaceable equipment source.
Their tags are matched against each skill. Unequipping replaces the source, removing stale bonuses.
Flat life is added before percentage life bonuses; armour is additive. Equipping during a run
does not heal the character; removing life gear clamps current HP to the new maximum.
Weapons additionally allow attack speed. The default weapon affixes require
the `attack` tag. Attack speed divides attack cooldowns separately from cooldown recovery and
does not accelerate spells without `attack`. Weapon bonuses from equipped weapon slots combine.

## Equipment types and class permissions

Warriors and Paladins can wear plate, mail, leather and cloth. Hunters and Shamans can wear
mail, leather and cloth; Rogues and Druids leather and cloth; Priests, Mages and Warlocks cloth.
Equipment permissions apply to buttons, right-click, drag-and-drop and both sides of a slot swap.

Drop rules are narrower than wearing permissions:

| Material | Target classes | Allowed offensive primary stats |
|---|---|---|
| Plate | Warrior, Paladin | Strength for Warrior; intelligence for Paladin |
| Mail | Warrior, Paladin, Hunter, Shaman | Strength for Warrior; agility for Hunter/Shaman; intelligence for Shaman/Paladin |
| Leather | Hunter, Rogue, Shaman, Druid | Agility; intelligence for Shaman/Druid only |
| Cloth | Druid, Priest, Mage, Warlock | Intelligence |

Stamina is available on every base and guaranteed when two primary stats are rolled (Epic).
Accessories and weapons use the target class's offensive primary stats: strength for Warrior,
agility for Hunter/Rogue/Shaman/Druid, intelligence for Paladin/Shaman/Druid/Priest/Mage/Warlock.
All new Paladin-targeted equipment rolls intelligence rather than strength, including melee
weapons, shields and bows. Stamina remains available. Previously saved strength gear stays usable.

The melee bases include one-handed swords, axes, maces and daggers, plus two-handed swords,
axes, maces and polearms. Every attack class (Warrior, Paladin, Hunter, Rogue, Shaman, Druid)
can use the one-handed weapons. Rogues cannot equip two-handed weapons (including polearms
and staves); their targeted loot excludes these bases. Their offensive primary is strength for Warrior and agility
for Hunter/Rogue/Shaman/Druid. Paladin weapons roll intelligence only. One-handed swords, axes
and maces can also roll intelligence when targeted to Shaman; daggers and two-handed melee
weapons otherwise remain attack-only.
Two-handed caster staves are available to Paladin, Shaman, Druid, Priest, Mage and Warlock,
with intelligence as their only offensive primary. Like wands, staves exclude attack-only
secondaries. Stamina remains available on all weapon bases.

Equipping a two-handed weapon or staff in main hand returns the off-hand item to the bag.
An off-hand weapon or shield cannot be equipped until the two-handed weapon is removed.
These rules apply to right-click, equip buttons, drag-and-drop and saved loadouts.
The Fury capstone Titan's Grip permits a Warrior to equip two two-handed melee
weapons. Staves remain excluded. Losing that talent returns an incompatible
off-hand weapon to the bag without deleting it.
One-handed weapons fit either hand, but only Warriors, Rogues, Shamans and Hunters can put
one in the off-hand slot. Right-click still chooses the first compatible slot (main hand);
drag to off hand or use its equip button to choose dual wield. Shields fit off hand only and
are restricted to Warriors, Paladins and Shamans. Bows are available to Warriors, Paladins, Hunters, Shamans and
Rogues. Wands use the ranged slot and are available to every intelligence class: Paladin,
Shaman, Druid, Priest, Mage and Warlock. Bows roll offensive primaries for attacks (strength
for Warrior, agility for Hunter/Rogue/Shaman), or intelligence for Paladin; wands roll intelligence. Stamina remains
available on both. Wands cannot roll attack-only secondaries; bows cannot roll spell-only
secondaries.

Shields roll base block from `base_block = 0.25` to `base_block_max = 0.40` (25–40%) independently of their rarity's affix count. The
`block_chance` secondary rolls 2–8 percentage points on shields only. Base and secondary chances
add and clamp to 75% overall. A successful block cancels an enemy melee blow before armour
and spell absorption, consuming neither HP nor absorption. Unequipping removes both bonuses.
Weapon attack speed, flat attack damage and leech cannot roll on a shield. The character sheet
shows total block chance; item tooltips show base block and the rolled secondary separately.

Bases use `equipment_type`: `plate`, `mail`, `leather`, `cloth`, `universal`, `melee_one_hand`, `hybrid_one_hand`, `two_hand`, `staff`, `shield`,
`bow` or `wand`. Existing `one_hand` items retain their original generic permissions; new attack-only one-handed bases use `melee_one_hand`, while swords, axes and maces use `hybrid_one_hand` to allow intelligence for Paladin/Shaman. Old saves without this field remain `legacy` equipment with their saved effects.
Typed gear that is no longer wearable is returned to the bag when that character's inventory loads. New loot bases must have a concrete equipment type; old untyped configuration files fall back to the current embedded table without being overwritten.
## Worn appearances

The world hero, character-menu preview and character sheet use the same live player equipment
pipeline: body texture compositing, equipment geosets, helmets, shoulders and held models.
Only equipped items contribute appearances. Unequipping clears the corresponding visible slot;
a two-handed weapon also clears the off-hand visual. An empty inventory shows the stock bare
character body, without the class trainer's baked clothes or fixed weapons. The selected NPC's
race, sex, face, skin and hair customization are retained on the playable body model.

Each visible base defines `appearance` in `survivors-item-bases.toml`, for example:

```toml
appearance = { display_id = 1542, inventory_type = 13, item_class = 2, subclass = 7, material = 1, sheath = 3 }
```

`display_id` references the player's own `ItemDisplayInfo.dbc`, not a model bundled in the repo.
The inventory type places the model, while class/subclass/material/sheath select grip, ready
animations and stow placement. Starting human Warrior/Rogue/Mage/Paladin items supply the simple
sword, shield, dagger, staff, maul and several garment looks (`CharStartOutfit.dbc`). Other slots
use basic A-series armour and weapon models; level-one humans have no plate or shoulder outfit.
Jewellery (neck, rings and trinkets) has no worn model and does not create a visible attachment.

New items persist their rolled appearance independently of later base edits. Existing saved
items without appearance metadata resolve their current base and are migrated on inventory load
without changing their affixes. Older local base tables inherit missing default appearance data
in memory, preserving custom loot weights and ranges. Selecting another character loads only
that character's worn gear; the creation preview starts with empty equipment.
## Configuration

Edit [config/survivors-item-bases.toml](../config/survivors-item-bases.toml) for item bases, boss drop chance and rarity weights, and [config/survivors-item-affixes.toml](../config/survivors-item-affixes.toml) for affix definitions. Restart the game after editing.
Together these files control boss drop chance, rarity weights, base weights, names, icons, slots,
affix weights, ranges, exclusive groups and required skill tags. Primary/secondary allocation follows rarity.
Weights are relative; zero disables an entry. `boss_drop_chance` is a probability from 0 to 1.
Fractions represent secondary percentages (`0.15` means 15%); primary stats, life, armour and target counts
are whole numbers. All listed `tags` must match; an empty list matches all skills, subject to
the spell engine's mechanic compatibility rules.

To add an affix, append an `[[affixes]]` table to `survivors-item-affixes.toml`. For example:

```toml
[[affixes]]
id = "bow_projectiles"
name = "Splitting"
kind = "prefix"
group = "projectiles"
effect = "projectiles"
min = 1
max = 2
weight = 1
tags = ["projectile"]
slots = ["ranged"]
```

Primary effects: `stamina`, `strength`, `agility`, `intelligence` (whole points).

Supported secondary and legacy effects: `damage`, `dot_damage`, `crit_chance`, `crit_multiplier`, `cooldown_recovery`,
`area`, `duration`, `repeat`, `projectiles`, `chain`, `fork`, `melee_targets`, `flat_damage`, `spell_damage`,
`attack_speed`, `life`, `armour`, `life_leech`, `life_on_kill`, `regeneration`,
`movement_speed`, `damage_reduction`, `healing_effectiveness`, `boss_damage`,
`full_life_damage`, `low_life_damage`, `bleed_chance`, `ignite_chance`, `chill_chance`,
`knockback_chance`, `armour_penetration`, `pickup_radius`, `experience`, `luck`, `proc`, `block_chance`.

`spell_damage` adds flat damage to damaging Spell hits before increased/more and critical
multipliers. The default secondary affix rolls 3–9 points. It does not affect Attack hits,
healing, absorption or damage-over-time ticks.
Slots are the snake_case names in the default file.
Weapon-only effects on non-weapon slots are rejected. Global character stats cannot have spell tag filters.
Every enabled base must have enough primary and secondary groups for every enabled rarity, including stamina when two primary stats are required.
Groups cannot mix prefixes and suffixes or primary and secondary stats. Legendary weight must remain zero.

Each file loads independently: `WOW_ITEM_BASES_CONFIG` / `WOW_ITEM_AFFIXES_CONFIG` override, then the corresponding native `config/` file, then its `benilla-config/` copy, then embedded defaults. Missing local copies are created automatically. A valid legacy `benilla-config/survivors-items.toml` is split into the missing local files while preserving its settings and leaving the original intact. `WOW_ITEMS_CONFIG` remains a higher-priority override for a legacy combined file. Invalid configurations are logged and use embedded defaults without overwriting the invalid files. Browser builds use the local-state store rather than a filesystem config directory.

## Character saves

Each character saves inventory and equipped item identities together in
`benilla-config/characters/<id>-items.toml`. Items survive death, returning to the hub, restarting
and changing characters. Existing characters start with an empty bag. Save writes are atomic;
failed writes keep the previous inventory, and failed pickups remain on the ground. An unreadable
save blocks inventory changes rather than overwriting it. Deleting a character removes its item file.
Editing the loot table affects future drops; saved items retain their rolled effects.

## Recovery, defence and impact effects

Weapon life leech applies to direct `Attack` hits, excluding reflected guards and DoT ticks.
It uses effective damage (overkill does not heal), recovers only missing life and never revives
a dead character. Recovery is capped at 20% maximum life per active-game-second window,
including healing-effectiveness bonuses. Life on kill works for direct and periodic kills.
Regeneration is flat HP/s; healing effectiveness also affects regeneration, pickups and skill healing.
Armour bases grant armour independently of affixes: for a level-one chest, Plate grants 100,
Mail 70, Leather 40 and Cloth 20. Other slots have smaller values; shields grant 100 and cloaks
10. Each base's optional `base_armour` setting can override these defaults. Generated armour
scales by `1 + 0.02 × (item level - 1)`, rounded to the nearest point, and is saved on the item.
Existing items without this field use their material and slot defaults. Statless starting kits
remain at zero armour. Equipped base armour, armour affixes, passives and talents add together.

Hero armour uses a normalized logarithmic curve: `L = ln(1 + armour / 50)` and damage
reduction is `0.99 × L / (L + 1.5)`. There is no hard armour-reduction cap; the horizontal
asymptote is 99%. 1000 armour gives 66.3%, 5000 gives 74.7% and 30000 gives 80.2%.
Reaching 75% requires about 5379 armour, 80% about 27612, and 85% about 450924: gains become
very expensive above 75%. The sheet displays the total and its reduction.
Secondary damage reduction (separately capped at 75%) and armour multiply their remaining
damage fractions before absorption shields are consumed. Successful blocks still cancel the
whole blow. Enemy armour and penetration keep their existing rules.

Boss/full-life/low-life bonuses are evaluated against the target at impact (and per DoT tick).
Low life means at most 35% HP. Applicable conditional bonuses add together as one extra damage
multiplier, separate from ordinary increased damage. Physical hits face `level × 0.5` armour on
normal enemies and `level` armour on bosses. Their damage is divided by `1 + effective_armour / 100`;
penetration ignores a percentage of that armour, capped at 100%. Other schools and DoTs ignore it.

Bleed requires physical attacks: 20% of the base hit per second for four seconds. Ignite requires
fire: 25% of the base hit per second for three seconds. Neither double-dips conditional impact
bonuses; conditions are evaluated when its damage ticks. They share the existing six-DoT limit.
Chill requires cold hits and slows by 25% for two seconds. Projectile knockback pushes living
non-boss enemies 1.5 yards away from the player; bosses are immune. Bleed/ignite use WoW effects.

## Jewellery luck

Luck rolls only on rings and amulets and adds across equipped items. It improves **card rarity**,
not item drops. Before drawing rarity, weight at rarity index `i` becomes `weight × (1 + luck)^i`
(Common index 0 through Legendary index 3), then normalizes over available categories. Zero
weights stay disabled, card weights inside a rarity remain unchanged, and rarity is still drawn
first. With all four categories available, +20% luck turns the default 5% legendary chance into
approximately 7.4%; it does not grant 20 percentage points of legendary chance.

## Trinket procs

A proc is an ordinary rolled affix restricted to trinkets. The default four variants share
`group = "trinket_proc"`, so one trinket cannot roll multiple procs. Chance is its `min`/`max`
roll; its `proc` payload defines effect, internal cooldown, duration and strength:

```toml
[[affixes]]
id = "custom_class_proc"
name = "of Echoes"
kind = "suffix"
group = "trinket_proc"
effect = "proc"
min = 0.08
max = 0.12
weight = 4
slots = ["trinket"]
proc = { kind = "class_spell", cooldown = 6, duration = 0, power = 0.75 }
```

`kind` supports `damage`, `crit_chance`, `attack_speed` or `class_spell`. The first three grant
temporary spell-engine stats for `duration`; `power = 0.25` means +25%. Class spells use `power`
as a damage multiplier. The default stat variants last 4–5 seconds with 10–12-second ICDs;
the class-spell variant has a 6-second ICD and casts at 75% power.

Class-spell identity is selected from the looting character's class at drop time and saved on
the item, including its class. All nine classes have eligible direct/area/chain spells. A proc
can cast an unlearned spell at rank 1; learned spells use their current rank. Casts use existing
WoW animations and the spell engine (crit, projectiles, chain, fork, area), without learning a
spell, occupying an active slot, consuming its ordinary cooldown or scheduling repeat casts.

Procs roll on effective direct damage, not DoT ticks. Triggered spell hits cannot trigger other
trinkets. Each item/affix has an independent ICD, retained when unequipping/re-equipping during
the run; the trinket must be worn both at the original hit and at proc resolution. A second
activation refreshes its own buff instead of stacking copies. Unequipping removes its buffs.
The active run clock controls duration and ICDs, so C/B/T pauses freeze them. Buffs, ICDs and
queued events reset with a new run; the item's rolled proc remains saved. Tooltips show proc
details, the character sheet shows active buffs, and a short banner announces each activation.
