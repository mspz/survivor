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
