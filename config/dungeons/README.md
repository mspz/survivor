# Équilibrage des donjons

Chaque TOML règle un seul donjon, indépendamment de `survivors-expeditions.toml`.
Redémarrer le jeu après modification. Les valeurs intégrées servent de secours si un fichier est absent ou invalide. Un fichier invalide est signalé dans les logs.

- `[combat]` : statistiques de base, progression dans le temps, population, multijoueur, armure et résistances.

`[combat].health_per_level` et `damage_per_level` ajoutent chacun 2 % par niveau au-dessus du niveau 1, pour les mêlées, casters et boss. Le multiplicateur est `1 + taux × (niveau réel − 1)` : ×2,18 au niveau 60, ×3,18 au niveau 110. Il se cumule avec les autres multiplicateurs du donjon. La campagne utilise ces paramètres dans `survivors-expeditions.toml`, l'Endless dans `[combat]` de `survivors-endless.toml`.
- `[zone]` : multiplicateurs appliqués aux statistiques de base et quantité de boss par vague.
- `[[monsters]]` et `[[bosses]]` : niveau, vitesse, taille et multiplicateurs individuels de vie, dégâts et XP. Les noms identifient les créatures existantes et doivent être conservés.
- Les effets historiques, les nouveaux bonus permanents `[stats]` et les lanceurs de sorts `[ranged]` sont partagés dans `../survivors-monster-mod-effects.toml`. Le pool contient 29 mods cumulables, indépendants des bases mêlée, caster et boss. Colosse remplace Elite ; `[elites]` conserve ses réglages de vie, taille et XP.
- `../survivors-monster-mods.toml` : probabilité, nombre et poids des mods en campagne, en donjon et en endless. Les boss de campagne n'ont jamais de mods.
- `[modifiers]` : effets par rang, bonus d'ilvl et multiplicateur d'XP. L'ordre des six bonus d'ilvl correspond aux six premières options du menu. La septième, « Additional monster mods », utilise `monster_mods_per_rank` et `monster_mod_item_level_per_rank` : par défaut, +1 mod garanti par monstre et boss et +2 ilvl par rang, jusqu'à trois rangs. Les mods ne se répètent pas et le total est limité au nombre de mods activés (cinq au maximum).
- `[spells]` : seuils de phases, dégâts par impact, fréquence des impacts, avertissements, recharges et dimensions des zones.

Les sorts de boss explosent à la fin de `windup`, quand leur zone est entièrement remplie. Par défaut, `duration = 0.2` conserve uniquement un bref flash d'impact : chaque zone inflige un seul impact. Une durée supérieure à `pulse_interval` réactive les dégâts répétés dans la zone.

La vie et les dégâts d'une créature partent de `[combat] × [zone] × multiplicateur individuel`, puis reçoivent les multiplicateurs de temps, de groupe et de modificateurs applicables.
Les résistances utilisent des points de pourcentage ; les autres pourcentages utilisent des fractions (`0.25 = 25 %`).
Les tableaux de trois valeurs correspondent aux trois phases de vie. Les tableaux de cinq sorts suivent : zone principale, cercles ciblés, anneau, croix, secteur.
`duration_seconds` fixe la dur?e des vagues (720 secondes par d?faut). Les 8 vagues de boss arrivent ? 12,5 %, 25 %, ?, 100 % de cette dur?e. Les boss de la liste sont repris dans l?ordre si n?cessaire. ? 100 %, les apparitions cessent ; la victoire exige la mort de tous les boss et de tous les monstres restants.
`xp_multiplier` des créatures règle les gemmes d'XP de la partie ; `character_xp_multiplier` règle l'XP permanente du personnage et des paragons.

La cinquième option, « Extra modifiers on boss », ajoute `boss_mods_per_rank` mods garantis par boss et par rang (1 par défaut), cumulables avec « Additional monster mods », dans la limite de cinq mods distincts. Elle ne donne plus de sorts supplémentaires. Fleet monsters donne +60 % de vitesse par rang, Brutal monsters +100 % de dégâts, Frenzy +100 % de vitesse d’attaque et de récupération des sorts ; Hasty boss spells multiplie le temps d’avertissement par 0,5 à chaque rang.
