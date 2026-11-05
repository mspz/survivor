# Cooperative revival

In an active multiplayer expedition, any living player can revive a fallen ally by remaining
within **5 yards** of their body for **5 continuous seconds**. This is automatic and requires
no learned resurrection spell. Movement inside the radius is allowed. Leaving the radius,
dying or disconnecting resets that rescuer's progress. Switching to another rescuer starts
a fresh five seconds; additional allies do not speed up the channel.

The character returns at **50% maximum health** with **2 seconds of immunity**. Movement,
combat and the remaining expedition resume normally. Death-state health fields are updated
immediately so the character stands back up, and a resurrection visual plays. Pending spell
repeats and stale shield/heal/thorns timers are cleared; levels, equipment and learned skills
remain. A party wipe still ends the expedition; solo play has no proximity revival.

Fallen guests remain in the expedition instead of opening their personal results screen
after three seconds. The fallen player and rescuer see progress in the HUD announcement
area. The host checks authoritative positions and survival state, performs the revival and
replicates health, death state and channel progress to guests.

Targeted verification: `scripts/test-changed.ps1 --scope revival --offline`.
