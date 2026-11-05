# Expedition difficulty

The expedition table deliberately favours being above the zone's level range and farming
its early bosses before attempting a clear. Suggested persistent character levels are 12+,
22+, 32+, 42+, 52+, and 60 respectively, with equipment from the current zone. These are
tuning targets, not entry restrictions or a guarantee of clearing every class/build.
This first hard tuning pass still needs live playtesting.

The table below records the initial hard-tuning preset. The live TOML is authoritative and
may contain further operator adjustments.

| Zone | Suggested level | Monster HP at 15 min | Monster damage | Boss HP at 15 min | Boss damage |
| --- | --- | --- | --- | --- | --- |
| Elwynn Forest | 12+ | 288 | 36 | 9,000 | 114 |
| Westfall | 22+ | 576 | 59.4 | 18,000 | 193.8 |
| Redridge Mountains | 32+ | 1,036.8 | 93.6 | 34,200 | 319.2 |
| Duskwood | 42+ | 1,785.6 | 140.4 | 61,200 | 490.2 |
| Stranglethorn Vale | 52+ | 2,880 | 208.8 | 103,500 | 741 |
| Blasted Lands | 60, strong loadout | 4,608 | 306 | 162,000 | 1,083 |

Values are solo, before armour, resistances and final damage reduction. Spawn health and damage
rise linearly from x1 to x3 across 15 minutes, then stop increasing. A monster's maximum and
current health are never rescaled after it appears; only damage continues following the timer.
Existing armour remains
5 per monster level and 10 per boss level; elemental resistance adds 0.2 percentage points
per monster level and 0.35 per boss level. Zone final reduction remains 0/10/20/30/40/50%,
unaffected by penetration. Monster population grows from 18 to 280 before zone quantity
multipliers (1.05 to 1.55), with a 450-monster cap.

Compared with the previous table, end-of-run monster HP is 3 to 4.8 times higher and monster
damage 6 to roughly 11.9 times higher. Boss HP is 5 to roughly 5.6 times higher, with some
middle tiers higher still. Defensive equipment, area damage, movement and early boss loot
are intended to matter, alongside permanent and expedition-level bonuses.

## Rare ranged casters

After Elwynn, each ordinary monster spawn has a 2% chance to be replaced by a ranged caster
(`ranged_monster_chance = 0.02`). This applies to normal waves and encirclement rings, never
bosses or training actors. It is a probability per spawn, rather than a fixed population quota.

| Zone | Caster | Damage school |
| --- | --- | --- |
| Westfall | Riverpaw Mystic | Nature (Wrath) |
| Redridge | Blackrock Summoner | Fire (Fireball) |
| Duskwood | Skeletal Mage | Cold (Frostbolt) |
| Stranglethorn | Hakkari Shadowcaster | Shadow (Shadow Bolt) |
| Blasted Lands | Shadowsworn Cultist | Fire (Fireball) |

Casters approach to 16 yards, shoot within 20 yards and have a 3-second attack interval.
Projectiles use the spell's client travel speed (24 yards/second if unavailable) and damage
lands on arrival. They use the horde's nearest-living-player targeting and obey taunts, stuns,
fear and Fade. Their hits use the matching elemental resistance, absorption, immunity and
general damage reduction, rather than physical armour or melee block. They receive the same
zone/time/party health tuning as other ordinary monsters and replace a spawn rather than
adding to population. Verify with `scripts/test-changed.ps1 --scope ranged --offline`.

## Multiplayer

Only enemy health receives the party multiplier; damage does not increase with player count.
The exact rule is **x1 solo; otherwise 2.5 x (connected players - 1)**:
two players x2.5, three x5, four x7.5, five x10. Fallen connected players still count.
The multiplier applies to ordinary monsters and bosses immediately on spawn. Joining or
leaving only changes the health of future spawns: existing monsters keep their maximum and
remaining health. Training actors bypass these adjustments.

Settings live in `config/survivors-expeditions.toml` and reload when a run starts. Older custom
tables that omit `multiplayer_health_per_extra_player` use 2.5 by default.

Targeted verification: `scripts/test-changed.ps1 --scope difficulty --offline`.
