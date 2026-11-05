# Healer party buffs

Healing spells grant nearby living allies, including the caster, a blessing for 8 seconds
within 12 yards at rank one. They also cast at full health to maintain that blessing.
Discipline priests do not grant blessings: Discipline is considered the active priest build
when it has at least as many invested points as Holy and at least one Discipline point.

| Source | Damage | Specialty bonus at rank one |
| --- | --- | --- |
| Paladin: Holy Light | +8% | 20% less damage taken |
| Holy Priest: Flash Heal, Renew | +20% | Stronger damage support |
| Shaman: Lesser Healing Wave | +8% | +20% cooldown recovery and +5% offensive spell repeat chance |
| Druid: Rejuvenation, Regrowth | +8% | +20% movement speed and +5 percentage points to all elemental resistances |

The first Holy Paladin, Holy Priest and Restoration Shaman milestone also grants the matching
permanent support aura. Druids gain their aura with Gift of Nature (Living Grove also provides
it). These auras stop applying when the source dies or moves out of range. Spell blessings
already received remain until their duration expires. Spell and talent tooltips show these
effects; learned proc spell tooltips show the aura's current scaled values.

Buff power is a separate **20-percentage-point spell payload**. It uses Intelligence,
flat spell damage, global spell power, school modifiers (Holy or Nature), skill-specific
modifiers, and normal spell-rank scaling. For example, +10 flat spell damage turns 20 points
into 30 points before increased/more modifiers. Percentage values are converted only after
resolving the payload. Area and Duration modifiers affect range and duration. Weapons,
critical hits and outgoing class-damage balance do not affect buff power. The payload never
hits enemies and never triggers on-hit effects.

Received support damage is excluded when calculating another support buff, preventing
self-amplifying refresh loops. Different families combine; within one family only the
strongest current source applies. Recasting refreshes that caster's blessing instead of
adding a stack. A weaker caster cannot extend a stronger caster's expired blessing.

To bound late-run scaling, primary power is capped at 200 percentage points per family.
Damage support follows that power (other families grant 40% as much damage as Holy Priest).
Paladin damage reduction is capped at 50%, Shaman recovery at 100%, Druid movement at 60%
and its resistance bonus at 25 points. The existing 75% total resistance cap still applies,
including elemental damage over time. Cooldown recovery is increased recovery speed rather
than a direct cooldown reduction: +20% recovery divides the cooldown by 1.2.

Support runs on the authoritative host for every party member. Bonuses are replicated in
snapshots so guests use the same displayed combat stats as the host.

Targeted verification: `scripts/test-changed.ps1 --scope support --offline`.
