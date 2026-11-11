# Expéditions

Le parcours humain comporte six expéditions, débloquées dans cet ordre :

| Niveaux | Expédition |
|---|---|
| 0–9 | Elwynn Forest |
| 10–19 | Westfall |
| 20–29 | Redridge Mountains |
| 30–39 | Duskwood |
| 40–49 | Stranglethorn Vale — Gurubashi Arena |
| 50–59 | Blasted Lands — The Dark Portal |

Ces tranches définissent la campagne du jeu, adaptée au parcours humain de WoW.
Les anciennes cartes ne sont plus proposées. Les sauvegardes gardent leurs identifiants de zone.
Le terrain d'entraînement reste réservé aux personnages `TEST_` et ignore la difficulté.

Modifier `config/survivors-expeditions.toml` avant de démarrer une nouvelle expédition.
`WOW_EXPEDITION_CONFIG` permet de choisir un autre fichier. En multijoueur, l'hôte applique ses réglages.
Le fichier intégré sert de secours si le fichier externe manque ou est invalide.

## Bases et mods des monstres

Les bases sont mêlée, caster et boss. Le pool contient 49 mods distincts et
cumulables, sans doublons. Colosse remplace l'ancien effet Elite.
Les noms affichent tous les mods ; leurs effets conservent leurs réglages dans
`config/survivors-monster-mod-effects.toml`.
Le catalogue et les compatibilités sont décrits dans [MONSTER-MODS.md](MONSTER-MODS.md).
Chaque spécial a un poids de tirage de 1, contre 2 pour un mod de stats.

`config/survivors-monster-mods.toml` règle les poids et la répartition. Redémarrer
après modification ; en multijoueur, seul l'hôte tire les mods.

| Zone de campagne | Monstres avec des mods | Maximum de mods |
|---|---|---|
| 1 | 3 % | 1 |
| 2 | 6 % | 1 |
| 3 | 9 % | 1 |
| 4 | 12 % | 1 |
| 5 | 12 % | 2 |
| 6 | 15 % | 3 |

Les boss de campagne et les acteurs d'entraînement n'ont aucun mod.
Les casters gardent leur propre probabilité d'apparition, indépendante des mods.

En endless, la progression dépend du numéro d'étape, pas de la carte tirée :

Les monstres de mêlée, casters et boss commencent au niveau 60 et gagnent un niveau
à chaque changement de zone : 60, 61, 62, etc., même lors du retour sur une carte
déjà visitée. Leurs coffres suivent ce niveau, dans la limite globale d'ilvl du loot.

- Monstres : 25 % au départ, +5 points par étape jusqu'à 100 % ; maximum de
  1 mod à l'étape 1, 2 à l'étape 4, puis +1 toutes les 3 étapes, jusqu'à 5.
  Parmi les monstres modifiés, le minimum augmente de 1 toutes les 6 étapes.
- Boss : 15 % au départ, +2,5 points par étape jusqu'à 75 % ; maximum de
  1 mod à l'étape 1, 2 à l'étape 7, puis +1 toutes les 6 étapes, jusqu'à 5.
  Parmi les boss modifiés, le minimum augmente de 1 toutes les 12 étapes.

En donjon, 30 % des monstres tirent 1 à 4 mods et 20 % des boss en tirent 1 à 2.
Le réglage « Additional monster mods » ajoute 1 mod garanti à chaque monstre et
boss par rang (3 rangs), ainsi que +2 ilvl par rang. Le total reste limité au pool
de mods activés et au plafond de 5 par créature. Les réglages propres au donjon
sont dans `config/dungeons/*.toml`.

## Statistiques et population

Les quatre valeurs globales de vie et dégâts définissent les statistiques de base.
Chaque section `[[zones]]`, dans l'ordre du tableau, règle séparément :

- `monster_quantity` : multiplicateur de la population et des vagues en cercle ;
- `monster_health`, `monster_damage` : multiplicateurs de vie et dégâts des monstres ;
- `boss_quantity` : nombre de boss par vague (0 à 32) ;
- `boss_health`, `boss_damage` : multiplicateurs de vie et dégâts des boss.

`population_start` et `population_end` règlent la population souhaitée du début à la fin.
`max_monsters` plafonne les monstres ordinaires vivants, y compris les vagues en cercle.
Les boss apparaissent toutes les 135 secondes, pendant une expédition de 15 minutes.

`end_multiplier = 2.0` augmente linéairement la vie et les dégâts de 100 % au départ
à **200 % à 15 minutes**, puis plafonne. Cela affecte aussi les ennemis déjà présents,
en conservant leur proportion de vie restante. Les modèles de monstres changent au fil
du temps, sans ajouter une seconde courbe de multiplication aux statistiques.
