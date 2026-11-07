# Level bonuses

Level one is the baseline. Each gained expedition level adds **5 maximum health and 2%
increased damage**. Each gained persistent character level adds **10 maximum health and 5%
increased damage**. Both sources add together, independently of learned spell ranks.

For example, character level 10 and expedition level 5 grant +110 maximum health and +53%
increased damage. Level health joins base/equipment health before percentage health modifiers.
Level damage adds to other increased damage and affects attacks, spell hits and damage over
time. It does not increase healing, absorption or percentage support-buff payloads.

Gaining a level grants the corresponding extra current health while preserving missing health;
it does not fully heal or revive the character. Expedition bonuses reset with a new run;
persistent bonuses follow the saved character's experience. The host replicates character and
expedition levels so guests display the same bonuses.

Targeted verification: `scripts/test-changed.ps1 --scope levels --offline`.
