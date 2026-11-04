# Controls

Zones unlock in menu order: win a run (survive 15 minutes) to unlock the next zone. Deaths and abandoned runs do not unlock zones. Progress is saved between launches.

## Characters, sanctuary and talents

The game opens on character selection. Create a named character, choose race, gender and
class, then enter the world. Up to ten characters can coexist. Each character has its own
persistent level, talents and sequential zone unlocks. Later characters start at level 1.

Entering the world takes you to **Stormwind City**, an offline sanctuary shared by all races.
The character menu uses Classic client artwork with a stationary equipped-character preview,
a scrollable roster on the right and **Enter World** below the model. Use the arrows underneath
to rotate the view. Creation, expedition, pause and option panels share the native WoW frames
and red buttons loaded from your local game data.
Walk around using the normal controls. **Expeditions** opens the zone list; select an unlocked
zone and start a run. Results and abandoning a run return to Stormwind. **Log out** returns
to character selection. Pausing/resuming in the sanctuary preserves your position.

Open **Talents** or press **T** in Stormwind to spend points or reset your build for free.
During expeditions the window is read-only. The character's class is fixed; the three bottom
tabs switch its specializations. Classic frame textures, specialization backgrounds, icons,
rank badges and prerequisite arrows are loaded from the installed client. Green means an
available rank, gold means fully ranked and grey means unavailable. Hover an icon for the
current bonus, next-rank bonus, affected branch spells and requirements.

Each specialization has 19 nodes in four columns and seven tiers. Tiers require 0/5/10/15/20/25/30
points in earlier tiers. Only arrow-linked nodes additionally require a completed prerequisite.
Most talents have five ranks; chain and fork keystones and the final starting-spell unlock have
one. The three trees share the character's budget of up to 51 points (one per level from 10 to 60).
Talents resolve through the shared spell engine, with compatible alternatives for other tags.

Earn 1 character XP per kill, 1 per five seconds survived, 100 per boss and 500 per victory.
Deaths and abandoned runs keep earned XP. Run level and passive stacks reset for each expedition;
character XP and talents remain. XP is saved in batches and when leaving active play.

Save files live under `benilla-config`: `survivors-characters.toml` stores the roster;
`characters/<id>-talents.txt` stores that character's XP and v3 talent allocation;
`characters/<id>-zones.txt` stores that character's victories. The first character created
inherits legacy account XP and zone unlocks. The new tree layout refunds old v1/v2 talent
allocations while retaining XP. Legacy files are kept. Later characters inherit nothing.
Bindings, audio, display and mouse preferences remain application-wide.

Run passives are either generic character bonuses (XP, movement, health, regeneration, armour, pickup range) or spell-engine modifiers. Choices and rank upgrades require a currently owned skill that actually benefits: class-pool skills you have not learned do not qualify. Schools, projectiles, chains, forks, melee targets, damage over time and repeat chance follow this rule. A capped effect is not offered again. Run passives have no gameplay rank cap and there is no six-passive slot limit. Each new stack adds its per-stack bonus, so XP bonuses continue to create useful level-ups after spells reach their maximum rank. Cards and tooltips show unlimited stack counters, and the HUD wraps passive icons onto additional rows. Existing engine limits (such as 100% critical chance and bounded projectile graphs) still apply; already saturated effects are not offered as wasted upgrades.

Extra projectiles, extra melee targets, fork and chain passives are Legendary (orange), including further stacks. Each card rolls rarity first: 50% Common, 30% Uncommon, 15% Epic, 5% Legendary, then picks uniformly within that category. Rarity weights are normalized among categories with eligible cards for the current offer type. Entries cannot repeat within the same three-card offer. Numerical passives exist in all four rarities, granting x1/x2/x3/x4 of their base value. Mixed rarities accumulate on the same passive; the four coverage effects remain Legendary only. Critical multiplier starts at 200%; Deadly Precision adds 10/20/30/40 percentage points.

Talent branches use the spell engine: projectile/melee-target bonuses unlock at ranks 3 and 5 on compatible talents; single-rank keystones grant chain or fork with compatible alternatives for other branches. Tooltips show current and next effects. Defensive and XP training remain character-wide.

**Azeroth Survivors** — third-person movement and mouse camera controls.

| When | Key / Input | Action |
|---|---|---|
| Creation | Click a race, gender and class; type a name | create a persistent character |
| Character selection | **Enter** (or **Enter World**) | enter Stormwind on the selected character |
| Stormwind | **Expeditions** | choose a zone and start a run |
| Playing | **W / S** or **up / down** | move forward / backward |
| Playing | **A / D** or **left / right** | turn; strafe while holding right mouse |
| Playing | **Q / E** | strafe left / right |
| Playing | **both mouse buttons** | move forward |
| Playing | **Space** | jump |
| Playing | **F** (from level 5) | your class's movement spell: Blink, Charge… aim at the cursor |
| Any | **G** | toggle the picture polish (`?grade=0` starts it off) |
| Playing | **mouse wheel** | zoom, from over the shoulder out to 70 yd |
| Playing | **left drag** | orbit without turning the hero |
| Playing | **right drag** | turn the camera and hero together |
| Playing | **Esc** | pause |
| Playing | — | abilities cast themselves (no aim) |
| Level up | **1 / 2 / 3** (or click a card) | pick an upgrade |
| Paused | **Esc** | resume |
| Game over | **Enter** (or **Return to Stormwind**) | return to the sanctuary |
| Any | minimap **+ / −** / minimize | minimap zoom / collapse |

The camera starts behind the hero in third person. Open **Esc > Options > Mouse** to adjust mouse sensitivity (0.1x to 3.0x). This setting is saved across launches. In the menu the camera slowly orbits the
preview hero. Your characters and their progression are remembered across reloads.

## Pause options

Open **Esc > Options** for four tabs: key bindings (two keys per action), sound
(master/effects/music/ambience volumes and toggles), mouse sensitivity (0.1-3.0),
and display (outgoing damage, healing, incoming damage, critical-only outgoing numbers,
colour/sharpening, windowed mode). Each tab can be reset. Settings apply immediately
and save automatically. Esc closes Options while keeping the run paused; press Esc
again to resume. During binding capture Esc cancels and Backspace clears the slot.
Conflicting assignments swap. Esc remains available as a pause fallback.

Bindings and combat-number preferences live in `survivors-options.txt` in the local
state directory; sensitivity retains `survivors-mouse.txt`, and sound/video use the
engine's existing CVar configuration. Mouse gestures and menu confirmation remain fixed.


Reward weights are configurable in `config/survivors-rewards.toml` (rarities, spells, passives). Restart after editing; no rebuild is needed. See `docs/SPELL_ENGINE.md` for loading paths and validation.


Repeat chance stacks beyond 100%: 130% guarantees two total casts and has a 30% chance of a third; 230% guarantees three and has a 30% chance of a fourth. Copies are spaced by 0.18 seconds, keep the original cooldown running and never generate further copies.

## Character sheet and damage meter

Living bosses each have a health bar centred at the top of the screen, below the clock.
Multiple boss bars stack vertically, with each boss's name, current/max HP and percentage.
They track bosses outside the camera too and disappear on death or when leaving the expedition.

Press **C** or click **Character** to open the Classic-style paper doll. It contains the live
hero preview (rotate with the two arrows), 19 equipment slots and current character stats.
Equipment includes two rings, two trinkets, neck and weapons; worn items have rarity-coloured frames.
The right panel groups stats into collapsible Primary stats, Defense, Offense, Recovery,
Utility and Skill sections. Click a heading to expand/collapse it and scroll with the wheel.
The Skill section offers your spell icons and their calculated damage, cooldown and bonuses.
Defense shows armour reduction, flat reduction, total reduction, damage taken and effective
health. Sources combine multiplicatively, using the same calculation as incoming damage.
All entries use compact stat/value labels and update while open.
Opening the sheet or inventory pauses the game. C and B toggle their own windows; each close
button affects only its own window. Closing the last open panel resumes the game.
B (rebindable in Options) independently toggles the Bagnon-style inventory anchored on the right.
The bag and character sheet can be opened together or separately; each has its own close button.
The inventory shows 14 × 8 compact slots per page. Select an item, then choose its equipment slot;
click a worn item to inspect or unequip it. Rings and trinkets each have two separate slots.
Left-drag items between the bag and compatible equipment slots; drop onto another bag item
to swap its position. Right-click equips to the first compatible slot (Ring 1 / Trinket 1,
even if occupied) or unequips worn gear. The replaced item takes the incoming item's bag position.
Alt + left-click deletes an item. Invalid/outside drops leave the item in place.
T also pauses while the talent window is open; T, Esc or its close button resumes the game.
Switching between talents, the character sheet and bags keeps the game paused. Talent spending
and free resets remain available in the hub; expedition talents remain read-only.

The **Damage meter** button or **F6** toggles the meter. Drag its title bar to reposition it.
It remains movable/closable in the sanctuary, pause and card-selection screens. It shows each spell's DPS over the last five active game seconds, refreshed four times per
second, and total damage for this expedition. Hits, splash, chain/fork/repeats, ground effects,
DoTs, totems and reflected damage keep their originating ability. It counts effective health
removed and excludes overkill, healing, shields and incoming damage. Total damage in the run
results now uses the same effective-damage definition. Pausing freezes the time window; idle
DPS falls to zero after five active seconds. A new expedition clears the meter.

Character-sheet and meter keys can be rebound in Options. Positions and meter visibility
are application preferences saved automatically in `benilla-config/survivors-panels.txt`.
The character sheet can also be moved by its name bar; camera controls ignore panel drags.

On character selection, select a character and click **Delete character**. Type its exact name, then confirm **Delete permanently**; Esc or Cancel closes the dialog. Deletion removes the character and its personal talent/XP, inventory and zone files. Other characters and application settings are retained. Character ids are never reused, so deleting the last character does not re-enable legacy XP imports.

## Alternating level-up rewards

Run XP requirements grow exponentially: `round(5 + 10 × (1.06^(level − 1) − 1) / 0.06)`.
The opening costs 5 XP, then 15 XP; reaching the next level costs 120 XP at level 10,
343 XP at level 20 and 1,456 XP at level 40. Character meta-progression is separate.

Level-up offers alternate **active spells, then passives**, starting with active spells.
All cards in one offer have the same type. Active turns can teach a new spell while a slot
is free or improve an owned spell. Once all active slots are maxed and no further active
card is available, every level offers passives. Several levels earned at once still alternate
after each confirmed pick. The sequence resets with each new expedition.

Card and rarity weights still come from `config/survivors-rewards.toml`. Rarity weights are
normalized among the non-empty categories of the current type (active spells have no Common
cards). If fewer than three distinct eligible cards remain, fewer cards are shown. If a whole
type is disabled by configuration, the other type is used; Healthstone remains the fallback
only when no configured reward can be offered at all.
## Experience Magnet pickup

A large purple wisp is an Experience Magnet. Its base drop chance is 0.3% per defeated enemy,
five times lower than the ordinary meat chance of 1.5%; guaranteed boss meat is unchanged.
Pick it up normally to instantly collect every XP mote currently on the map, regardless of
distance or whether it was already moving toward the hero. XP bonuses apply once, all gained
levels are queued normally, and the active/passive offer alternation is preserved. Distant
meat and other magnets are not consumed. The pickup is used even when no XP remains.
Hold Shift over an item tooltip to compare it with equipped gear: green gains, red losses.

## Unit frames and targeting (Survivors)

The player frame sits below the action, left of centre. A short left-click on an
enemy or ally selects it and shows a mirrored target frame on the right, with its
portrait, name, level, current/maximum health and active debuff icons with seconds
remaining. Clicking empty ground clears the target. UI clicks and camera drags do
not select units. Selection is local and does not change automatic spell targeting.
Frames and debuffs also work for native multiplayer guests using the host's status.

