# Équilibrage des donjons

Chaque TOML règle un seul donjon, indépendamment de `survivors-expeditions.toml`.
Redémarrer le jeu après modification. Les valeurs intégrées servent de secours si un fichier est absent ou invalide. Un fichier invalide est signalé dans les logs.

- `[combat]` : statistiques de base, progression dans le temps, population, multijoueur, armure et résistances.
- `[zone]` : multiplicateurs appliqués aux statistiques de base et quantité de boss par vague.
- `[[monsters]]` et `[[bosses]]` : niveau, vitesse, taille et multiplicateurs individuels de vie, dégâts et XP. Les noms identifient les créatures existantes et doivent être conservés.
- Les variantes `[elites]`, `[sprinters]`, `[explosives]`, `[healers]`, `[protectors]` et les lanceurs de sorts `[ranged]` sont partages dans `../survivors-monster-types.toml`.
- `[modifiers]` : effets par rang, bonus d'ilvl et multiplicateur d'XP. L'ordre des six bonus d'ilvl correspond à l'ordre du menu.
- `[spells]` : seuils de phases, dégâts par impact, fréquence des impacts, avertissements, recharges et dimensions des zones.

La vie et les dégâts d'une créature partent de `[combat] × [zone] × multiplicateur individuel`, puis reçoivent les multiplicateurs de temps, de groupe et de modificateurs applicables.
Les résistances utilisent des points de pourcentage ; les autres pourcentages utilisent des fractions (`0.25 = 25 %`).
Les tableaux de trois valeurs correspondent aux trois phases de vie. Les tableaux de cinq sorts suivent : zone principale, cercles ciblés, anneau, croix, secteur.
`duration_seconds` fixe la dur?e des vagues (720 secondes par d?faut). Les 8 vagues de boss arrivent ? 12,5 %, 25 %, ?, 100 % de cette dur?e. Les boss de la liste sont repris dans l?ordre si n?cessaire. ? 100 %, les apparitions cessent ; la victoire exige la mort de tous les boss et de tous les monstres restants.
`xp_multiplier` des créatures règle les gemmes d'XP de la partie ; `character_xp_multiplier` règle l'XP permanente du personnage et des paragons.
