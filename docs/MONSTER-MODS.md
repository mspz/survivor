# Mods des monstres

Le mod Épineux (renvoi direct de dégâts) est retiré. Son identifiant reste réservé
pour lire les anciennes sauvegardes ; il est ignoré au chargement et au tirage.

Le pool contient 58 mods cumulables : des bonus de stats permanents et des mods
spéciaux avec déclenchements, compteurs ou compétences.
Leurs valeurs sont dans `config/survivors-monster-mod-effects.toml` ; leurs poids
nommés et leur répartition sont dans `config/survivors-monster-mods.toml`.
Un poids nul désactive un mod. Les mods sont tirés sans doublons, jusqu'à cinq par
créature. Les anciennes listes de cinq ou 29 poids restent lisibles.
Une même entité peut avoir plusieurs mods différents, mais **un seul exemplaire de
chaque mod**. Leur stockage est un ensemble de bits ; ajouter plusieurs fois le
même mod ne multiplie pas son effet. Les cumuls temporaires de Vengeur des morts
appartiennent à l'effet de ce mod unique. Artillerie et Artillerie majeure sont deux
mods distincts et peuvent donc coexister.
Chaque mod spécial a un poids de **1**, contre **2** pour un mod de stats : il est
donc deux fois moins probable individuellement, à compatibilité identique.
Cela concerne aussi les anciens Explosif, Soigneur, Protecteur et Porte-étendard.

Les calculs communs de vie, taille, vitesse, dégâts par école, critique, défense,
contrôles et vol de vie sont regroupés dans `monster_stats::Bonuses`, indépendamment
du propriétaire. `monster_specials::player` raccorde le même catalogue aux joueurs,
avec le même runtime de boucliers, adaptation, blocage et cumuls temporaires.
Le Trône du Dévoreur n'est pas implémenté par ce système.

## Mods portés par un joueur

Neuf mods de pression sont réservés aux monstres : **Bombardier**, **Rémanence**,
**Débusqueur**, **Chasseur de mages**, **Briseur de concentration**, **Rabatteur**,
**Assiégeant**, **Salve de siège** et **Profanateur**. Ils sont exclus du vol et
ignorés par l'attribution de mods au joueur, y compris au chargement des snapshots.
Bombardier ne peut apparaître que sur un monstre caster, même en solo.

Leurs zones sont annoncées, verrouillées et répliquées par l'hôte. Le multiplicateur
commun `monster_ground_damage_multiplier` vaut **1.5** dans
`config/survivors-monster-mod-effects.toml` : 150 % des dégâts de chaque effet.
Bombardier remplace les projectiles par des impacts de rayon 3 m, annoncés pendant
1,2 s, avec +300 % de dégâts avant ce multiplicateur (×6 au total).
Rémanence laisse trois secondes de sol brûlant après les impacts, sans récursion.

Débusqueur marque après deux secondes dans un rayon de 3 m, puis explose une seconde
plus tard. Chasseur de mages vise le joueur le plus éloigné à portée et bondit sur
sa position annoncée. Briseur compte deux sorts effectivement lancés sans déplacement
de plus de 3 m ; son impact annoncé bloque les nouveaux sorts pendant une seconde
s'il inflige des dégâts. Les compétences automatiques n'ont pas de barre d'incantation.
Rabatteur annonce trois couloirs en éventail, avec des passages libres entre eux.
Assiégeant laisse une zone de cinq secondes, limitée à deux zones par porteur.
Salve de siège annonce trois impacts espacés de 0,6 s ; les suivants verrouillent
séparément une position prédite à partir du déplacement courant de la cible.
Profanateur laisse une grande zone pendant huit secondes ; ses dégâts augmentent
chaque seconde passée dedans et le compteur personnel se réinitialise en sortant.

`run.monster_mods.grant(mods, now, duration, position)` attribue un ou plusieurs mods.
`Some(secondes)` donne une durée ; `None` donne un mod permanent pour la partie.
Reprendre un mod prolonge sa durée sans augmenter sa puissance ni réinitialiser
ses compteurs. Un mod expiré cesse de modifier les calculs et disparaît de la barre.
Le changement de partie remet cet état à zéro ; aucune sauvegarde d'équipement
ne permet au client d'attribuer lui-même des mods.

Les 49 mods sont attribuables au joueur, indépendamment des restrictions de tirage
des monstres. Les projectiles et rebonds s'appliquent aux compétences compatibles,
les répétitions utilisent le système de répétition des compétences, les dégâts
ajoutés gardent leur école et les défenses s'appliquent aux vrais impacts et DoT.
Les déclenchements de déplacement ignorent les téléportations et charges forcées.
Les contrôles entrants passent par `monster_mods.control`, qui applique Inébranlable.

Les auras bénéfiques incluent la source et les joueurs alliés proches. Porte-étendard
accélère aussi les serviteurs à portée. Les effets offensifs d'un joueur ciblent les
monstres, jamais son groupe ; Oppressant affaiblit les monstres proches. Les soins
copiés ne se recopient pas, les dégâts ajoutés ne recréent pas d'autres dégâts ajoutés.
Les infections conservent leur durée restante et leur propriétaire lors du transfert.

La barre de buffs existante affiche une entrée nommée par mod, une description
et sa durée restante, ainsi que la réserve du bouclier actif. L'hôte transmet
les buffs et les données de mods aux invités : leurs statistiques et descriptions
de compétences reflètent aussi les mods attribués côté serveur.

## Auras visuelles des mods rares

Chaque mod de poids 1 a son propre motif animé sous les pieds de son porteur.
Les mods de stats communs n'ajoutent pas d'aura. Plusieurs mods rares donnent
plusieurs anneaux de tailles différentes ; reprendre un mod ne duplique pas son aura.
Le joueur local qui reçoit un mod utilise le même visuel. Les invités voient aussi
les auras des monstres répliqués, à partir des mods et de la vie envoyés par l'hôte.

Les textures viennent des effets au sol des auras de paladin installées : Dévotion
(465), Vindicte (7294), résistances à l'ombre (19876), au froid (19888), au feu
(19891), et Sainteté (20218). Les modèles se chargent par le système d'assets ;
les textures restent partagées en lecture seule. Le matériau et le shader des mods
sont séparés des vraies auras de paladin. Un motif procédural reste visible pendant
le chargement de la texture.

| Mod rare | Signature au sol |
|---|---|
| Explosif | Soleil fissuré orange, douze rayons irréguliers. |
| Soigneur | Croix verte et quatre pétales. |
| Protecteur | Bouclier bleu dans un octogone. |
| Porte-étendard | Étendard doré, losange et chevrons opposés. |
| Ralliement | Trois ondes de cri ambrées. |
| Vengeur des morts | Crâne violet sombre, orbites et dents. |
| Gardien | Six petits boucliers cyan en orbite. |
| Lien de sang | Deux gouttes rouges reliées par un lien pulsant. |
| Électrostatique | Six éclairs bleus anguleux. |
| Magnétique | Deux pôles violets et lignes de champ. |
| Chargeur | Trois chevrons orange orientés vers l'avant. |
| Sillage brûlant | Spirale de huit langues de feu. |
| Insaisissable | Trois croissants turquoise qui s'estompent. |
| Seconde peau | Trois couches d'écailles hexagonales émeraude. |
| Vengeur | Spirale rouge de réserve et contour à quatre pointes. |
| Riposte | Lames argentées croisées et garde courbe. |
| Adaptatif | Prisme à six écoles dont la couleur évolue. |
| Égide | Bouclier nacré dans une coque à quatre pétales. |
| Bastion | Fort carré et créneaux de bronze. |
| Prédateur | Trois griffures et œil félin. |
| Pestiféré | Signe biologique vert à trois lobes. |
| Hémorragique | Trois gouttes cramoisies et anneau dentelé. |
| Incendiaire | Cinq flammes dressées et un foyer central. |
| Artillerie majeure | Quatre réticules violets séparés. |
| Écho | Trois fronts d'onde décalés et deux points en orbite. |

Les animations suivent le temps de la partie et se figent pendant une pause.
Les empreintes suivent le terrain, sont retirées à la mort ou à la perte du mod,
et sont masquées dans le hub et les menus. Le rendu est purement cosmétique :
les rayons affichés ne représentent pas la portée des compétences ou des auras.
Les 25 styles partagent leurs matériaux entre porteurs ; le rendu regroupe les
empreintes par style et réutilise les échantillons du sol.

## Catalogue et valeurs initiales

| Clé de poids | Nom affiché | Effet |
|---|---|---|
| `colossus` | Colosse | Vie maximale ×2,5, taille ×1,25 ; conserve l'XP ×3 de l'ancien Elite. |
| `sprinter` | Sprinter | Vitesse de déplacement ×1,5. |
| `explosive` | Explosif | Explosion de feu après la mort, avec avertissement au sol. |
| `healer` | Soigneur | Aura de soin : 10 % de vie maximale toutes les 4 secondes aux alliés proches et à la source. Les autres boss restent exclus. |
| `protector` | Protecteur | Aura : 25 % de réduction supplémentaire aux alliés proches et à la source. Les autres boss restent exclus. |
| `frenzied` | Frénétique | Vitesse d'attaque, vitesse d'incantation et récupération des sorts ×1,4. Les avertissements des boss conservent leur durée. |
| `brutal` | Brutal | Dégâts physiques ×1,4. |
| `pyromancer` | Pyromancien | 35 % des dégâts de base ajoutés en feu sur les impacts directs. |
| `glacial` | Glacial | 35 % des dégâts de base ajoutés en froid sur les impacts directs. |
| `storm` | Orageux | 35 % des dégâts de base ajoutés en foudre/nature sur les impacts directs. |
| `profane` | Profane | 35 % des dégâts de base ajoutés en ombre sur les impacts directs. |
| `venomous` | Venimeux | Dégâts périodiques ×1,75. |
| `precise` | Précis | +15 points de chance de critique sur les impacts directs. |
| `devastating` | Dévastateur | +75 points de multiplicateur critique ; multiplicateur final ×2,25 avec Précis. |
| `artillery` | Artillerie | Deux projectiles supplémentaires vers des cibles distinctes disponibles. |
| `conductor` | Conducteur | Deux rebonds supplémentaires vers des cibles distinctes disponibles, avec délai de trajet. |
| `gigantic` | Gigantesque | Surface des zones ×1,75, donc dimensions linéaires ×√1,75. Les avertissements et les collisions utilisent la même échelle. |
| `long_range` | Longue portée | Portée de mêlée, portée et distance de maintien des casters, portée des sorts de boss ×1,4. Les cônes, charges et anneaux s'étendent aussi ; les explosions des mortiers conservent leur rayon. |
| `armoured` | Cuirassé | Dégâts physiques subis ×0,8, soit 20 % de dégâts en moins. |
| `fireproof` | Ignifugé | Dégâts de feu subis ×0,8. |
| `winter` | Hivernal | Dégâts de froid subis ×0,8. |
| `insulated` | Isolant | Dégâts de foudre/nature subis ×0,8, une seule fois pour cette école commune. |
| `occult` | Occulte | Dégâts d'ombre et d'arcane subis ×0,8. |
| `unstoppable` | Inébranlable | Durée des contrôles divisée par deux ; intensité des ralentissements divisée par deux. |
| `regenerating` | Régénérant | Régénère 5 % de sa vie maximale par seconde ; ne ressuscite pas. |
| `vampiric` | Vampirique | Récupère 5 % des dégâts directs réellement retirés à la vie des joueurs ; absorption, évitement et sur-dégâts ne soignent pas. |
| `banner` | Porte-étendard | Aura de vitesse d'action ×1,2, incluant sa source. |
| `oppressor` | Oppressant | Les joueurs proches infligent 15 % de dégâts en moins, impacts et DoT compris. |

Les auras identiques ne se cumulent pas. Porte-étendard et Oppressant ont un rayon
de 10 unités par défaut. Les auras sont recalculées avec un index spatial et cessent
d'agir dès que leur source meurt ou sort de portée.
La vitesse de déplacement reste soumise au plafond de l'endless, aura comprise.

## Mods spéciaux ajoutés

Les valeurs suivantes sont réglables dans la section `[special]` du fichier
d'effets. Les rayons de soutien et d'attraction valent 10 unités par défaut.

| Clé | Nom | Effet initial |
|---|---|---|
| `rally` | Ralliement | En passant sous 35 % de vie, protège les alliés proches, soi compris, à hauteur de 20 % de sa vie maximale pendant 5 s ; cri averti qui repousse les adversaires de 4 unités. Recharge 20 s. |
| `death_avenger` | Vengeur des morts | Chaque allié mort à proximité donne pendant 8 s +10 % de dégâts, +8 % de vitesse d'action et 12 % de réduction des dégâts. Maximum 5 cumuls et 60 % de réduction. Un cadavre ne déclenche qu'une fois. |
| `guardian` | Gardien | Toutes les 6 s, donne à un allié proche sans protection un bouclier de 15 % de la vie maximale du Gardien, durant 5 s. |
| `blood_link` | Lien de sang | Copie 30 % des soins effectivement reçus vers l'allié proche le plus blessé ; les copies ne se recopient pas. Toutes les 6 s, soigne les alliés proches et soi-même de **50 % de la vie maximale de la source**, jamais de celle des cibles. |
| `electrostatic` | Électrostatique | Après 8 unités parcourues à pied, s'arrêter prépare une nova de foudre : rayon 8, dégâts ×1,2. |
| `magnetic` | Magnétique | Toutes les 8 s, avertissement de 1 s puis attraction de 3 unités des adversaires proches. |
| `charger` | Chargeur | Après 12 unités parcourues à pied, charge de 6 unités vers une cible proche, puis impact de mêlée ×1,5. Préavis 0,5 s ; déplacement 0,3 s. |
| `burning_wake` | Sillage brûlant | Dépose du feu après 3 unités parcourues, au plus une zone par 0,75 s. Rayon 2,5 ; durée 3 s ; 25 % des dégâts de base par seconde. Les téléportations et charges forcées n'accumulent pas de distance. |
| `elusive` | Insaisissable | Après 2 s de déplacement continu, esquive le prochain coup direct. Une seule charge ; les DoT ne la consomment pas. |
| `second_skin` | Seconde peau | En passant sous 30 % de vie, gagne un bouclier de **100 % de sa vie maximale**, durant 5 s. Recharge 30 s ; aucun sauvetage d'un coup déjà létal. |
| `avenger` | Vengeur | Stocke les dégâts réellement retirés à la vie, jusqu'à 50 % de vie maximale. À 20 %, prépare une onde physique infligeant la réserve accumulée, puis la vide ; recharge 3 s. Les absorptions ne remplissent pas la réserve. |
| `riposte` | Riposte | +50 points de blocage des attaques, plafond 75 %. Bloquer prépare une prochaine attaque de mêlée ×2 pendant 5 s, consommée une seule fois. |
| `adaptive` | Adaptatif | Perdre de la vie à cause d'une école donne pendant 6 s **50 % de dégâts en moins** de cette école, impacts et DoT compris. La nouvelle école remplace la précédente. |
| `aegis` | Égide | Après 5 s sans perdre de vie, gagne un bouclier de 25 % de vie maximale pendant 8 s. |
| `bastion` | Bastion | Immobile : +25 % d'armure par seconde, jusqu'à +100 %. En mouvement : conserve le bonus 2 s, puis le perd progressivement. |
| `predator` | Prédateur | Dégâts ×1,5 contre une cible sans allié vivant à moins de 6 unités. |
| `plagued` | Pestiféré | Les impacts directs appliquent une infection de nature pendant 5 s : 15 % des dégâts du coup par seconde. À la mort de la victime, transfert au voisin vivant le plus proche avec la durée restante. |
| `hemorrhagic` | Hémorragique | Les attaques physiques appliquent un saignement de 4 s : 20 % des dégâts du coup par seconde. Les victimes qui saignent subissent 25 % de dégâts de mêlée supplémentaires. |
| `incendiary` | Incendiaire | 25 % de chance d'embraser sur un impact direct : 20 % des dégâts du coup par seconde pendant 4 s. Une victime qui meurt en brûlant laisse une zone de feu. |
| `greater_artillery` | Artillerie majeure | **4 projectiles supplémentaires** vers des cibles distinctes. Se cumule avec Artillerie : jusqu'à 7 cibles au total, cible initiale comprise. |
| `echo` | Écho | **100 % de répétition**, une seule fois : attaques de mêlée et sorts des casters après 0,18 s ; sorts de boss après 1 s. Les sorts persistants de boss se répètent 1 s après leur fin. Chaque répétition conserve ses avertissements et ne peut pas se répéter elle-même. |

Porte-étendard reste l'aura de vitesse d'action du catalogue ci-dessus ; il appartient
aussi aux spéciaux. Les auras identiques ne se cumulent pas. Les boucliers partagent
une réserve d'absorption : une protection plus faible ne remplace pas une plus forte.
Les infections, saignements et embrasements ne déclenchent pas d'autres afflictions.
Les zones de feu déjà déposées survivent à leur source ; les nouvelles compétences
actives sont annulées si leur source meurt ou est interrompue.

## Compatibilité des tirages

- Artillerie, Artillerie majeure et Conducteur : casters en groupe multijoueur, afin de disposer de
  cibles secondaires. Une activation ne touche pas plusieurs fois la même cible.
- Venimeux : boss de donjon utilisant les zones persistantes à impulsions.
- Gigantesque : boss ou monstre déjà Explosif, pour avoir une vraie zone à agrandir.
- Dévastateur : monstre ayant déjà Précis, pour disposer d'une chance de critique.
- Brutal, Oppressant, Riposte et Hémorragique : bases mêlée ou boss.
- Chargeur : base mêlée.
- Les autres mods restent disponibles sur les trois bases.

Un mod incompatible est exclu du tirage, sans consommer un emplacement. Les boss
de campagne et les acteurs d'entraînement restent sans mods.

## Défenses et dégâts ajoutés

Cuirassé et les défenses élémentaires ne donnent ni armure ni résistance. Leur
facteur ×0,8 s'applique après les défenses ordinaires, aux coups comme aux DoT.
Il n'est pas affecté par la pénétration d'armure ou le plafond des résistances.
Les prévisions qui évitent d'empiler des DoT déjà létaux utilisent aussi ce facteur.

Les dégâts élémentaires ajoutés conservent leur propre école et ses défenses.
Ils ne deviennent pas physiques avec Brutal et ne génèrent pas d'autres dégâts
ajoutés. Un blocage de mêlée bloque également les composantes élémentaires du coup.
