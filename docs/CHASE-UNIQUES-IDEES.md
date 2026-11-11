# Idées de chase uniques

Quinze chase uniques dont ce document décrit les règles en jeu.
Les réglages sont dans `config/survivors-chase-uniques.toml`.
Les noms d'objets et de sorts, ainsi que les propositions
de tooltips, sont en anglais ; les explications de conception restent en français.
Les valeurs décidées ci-dessous remplacent les anciennes pistes contradictoires.
Les points encore ouverts sont indiqués explicitement.

Objectif : des uniques très puissants qui changent une règle fondamentale du
personnage et permettent de construire un build autour d'eux dès leur obtention.

## Répartition des sources de loot

Chaque unique est exclusif à un donjon. Les boss éligibles, chances de drop et
seuils de difficulté restent à définir. Aucun n'appartient au pool global.

| Donjon exclusif | Chase uniques | Orientation |
|---|---|---|
| Blackrock Depths | Skin of the Unconquered ; Tyrant's Bulwark | Défense et blocage. |
| Lower Blackrock Spire | HEAD HUNTER ; Blood of the Last King | Mods volés et conversion en vie. |
| Dire Maul East | Ring of Absolute Hunger ; Crown of Cold Ashes | Vol de vie et propagation de DoT. |
| Dire Maul West | Hourglass of Stolen Time ; Legacy of the Nine ; The Faceless Visage | Charges et choix de compétences. |
| Scholomance | Pact of Two Lives ; Testament of the First Mage ; The Patient Executioner | Coûts, ressources et cooldowns. |
| Stratholme | Gospel of the Single Spell ; Hands of Sacrilege ; Oath of the Final Second | Spécialisation, soins et dégâts différés. |

Deux ou trois uniques par donjon, sans imposer les mêmes taux de drop.
L'affectation du nouvel objet The Patient Executioner à Scholomance est proposée.

## HEAD HUNTER — Belt

Ancien nom : Le Trône du Dévoreur. **Donjon :** Lower Blackrock Spire.

**Effet retenu :** tuer un monstre possédant au moins un mod donne **25 % de
chance de voler un seul de ses mods pendant 2 secondes de base**, affectées par
les bonus de durée du joueur (par exemple +50 % de durée donne 3 secondes).
La durée est calculée au moment du vol. Plusieurs mods différents
peuvent coexister. Reprendre le même renouvelle sa durée sans cumuler sa puissance
ni dupliquer son aura. Les mods spéciaux volés conservent leur visuel sous les pieds.
Le vol utilise le raccordement des mods au joueur et leurs auras existantes.

**Tooltip proposé :**
> Killing an enemy with at least one modifier has a 25% chance to steal one of its modifiers for a base duration of 2 seconds, affected by your Duration modifiers.
> Stealing a modifier you already have refreshes its duration. Identical modifiers do not stack.

Aucun plafond de mods différents : tous peuvent être actifs, avec une instance par mod.

**Builds visés :** nettoyage rapide, densité et maintien de nombreux mods différents.

**À préciser :** sélection du mod parmi ceux du monstre, absence de plafond de mods
simultanés et restrictions éventuelles selon la source. La durée, la chance et
le nombre de mods volés par déclenchement sont fixés ci-dessus.

## Gospel of the Single Spell — Two-Handed Weapon

Ancien nom : L’Évangile du Sort Unique. **Donjon :** Stratholme.

**Effet retenu :** une seule compétence offensive équipée, avec **×3 dégâts**.
Les compétences défensives et de mobilité restent disponibles. Un seul bonus
supplémentaire est actif, selon les tags de la compétence et cette priorité :

1. AoE : **50 % more Area of Effect** (multiplicateur ×1,5 de la statistique de zone).
2. DoT : prolifération sur des cibles encore vivantes, sans attendre leur mort.
3. Projectile : **3 forks**.
4. Compétence ciblée sans projectile, attaque ou sort : **3 cibles** au total.

Si plusieurs catégories correspondent, seule la première de cette liste s'applique.
Le bonus n'augmente plus avec les emplacements vides et aucun bonus de récupération
des cooldowns n'est retenu. Les améliorations se concentrent sur l'unique compétence.

**Tooltip proposé :**
> You can equip only one offensive skill. It deals 200% more Damage.
> That skill gains only the first applicable benefit: 50% more Area of Effect; Damage over Time proliferation from living enemies; 3 Forks; or 3 total targets for non-projectile targeted skills.

**Builds visés :** Fireball, Cleave ou Blizzard comme unique compétence offensive.

**À préciser :** définition des tags éligibles, fonctionnement exact des 3 forks,
rayon et fraction de prolifération, cartes, procs, copies et compétences accordées
par d'autres objets. Le bonus de zone n'est pas un multiplicateur de rayon décidé.

## Pact of Two Lives — Ring

Ancien nom : Le Pacte des Deux Vies. **Donjon :** Scholomance.

**Effet retenu :** chaque lancement de sort coûte **5 % de vie** et ajoute des
dégâts de base égaux à **20 % de vie**. Ces dégâts ajoutés adoptent l'école du
sort : Fire pour Fireball, Cold pour Frostbolt, etc. La dépense de vie ne compte
pas comme un coup reçu. Les anciennes mécaniques de sursoin en réserve et de
puissance selon la vie récemment dépensée sont supprimées.

Un sort ne part pas si la vie disponible ne permet pas de payer son coût.
Si plusieurs sorts sont prêts, traiter les niveaux décroissants (6 avant 5,
etc.) ; à niveau égal, conserver l'ordre existant du tri. Recalculer la capacité
à payer après chaque lancement.

**Tooltip proposé :**
> Casting a spell costs 5% of your Maximum Life and adds 20% of that Life as base Damage matching the spell's damage type.
> Spells cannot cast if you cannot pay their Life cost. Higher-level spells take priority; ties use their existing order.

Le coût et les dégâts ajoutés sont calculés sur la vie maximale. Le coût ne peut pas tuer.


**Builds visés :** vie, soins et régénération pour alimenter des sorts renforcés.

Le coût ne peut pas tuer : il faut conserver strictement plus de vie que le coût.
Soins et utilitaires paient aussi ; seuls les offensifs gagnent des dégâts.
Les répétitions natives ne repayent pas ; les charges du Sablier paient.

## Hourglass of Stolen Time — Trinket

Ancien nom : Le Sablier du Temps Volé. **Donjon :** Dire Maul West.

**Effet retenu :** les compétences offensives stockent plusieurs utilisations ;
leur récupération continue même lorsqu'une utilisation est disponible.
La décharge supplémentaire se déclenche **uniquement contre les boss**, jamais
contre les élites ordinaires, pour conserver les charges. Les utilisations
supplémentaires déclenchées infligent **20 % des dégâts initiaux** (80 % less Damage,
et non une réduction de seulement 20 %). Les répétitions ne génèrent pas de charges.
Les lancements normaux restent disponibles hors des combats de boss.

**Tooltip proposé :**
> Offensive skills can store additional uses as they recover.
> Against bosses, unleash stored uses in a burst. These additional uses deal 80% less Damage.
> Stored uses are not unleashed against other enemies. Repeats do not generate uses.

Chaque instance stocke `max(1, floor(2 / cooldown effectif))` utilisations.

**Builds visés :** accumulation puis rafale sur boss.

**À préciser :** capacité de charges, cadence, accumulation hors combat,
identification du boss ciblé et coûts des utilisations supplémentaires.

## Skin of the Unconquered — Body Armour

Ancien nom : La Peau de l’Invaincu. **Donjon :** Blackrock Depths.

**Effet retenu :** **200 % des dégâts bloqués** alimentent une réserve.
Chaque sort lancé consomme **10 % de la réserve actuelle** et ajoute la quantité
consommée en dégâts de base adaptés aux tags de ce sort. La réserve restante
est conservée. Les dégâts absorbés restent une source envisagée, dont le taux
n'a pas été fixé par la note. Aucun malus interdisant les critiques n'est retenu.

Exemple : bloquer 100 dégâts ajoute 200 à une réserve vide ; le prochain sort
consomme 20 et reçoit 20 dégâts de base supplémentaires, laissant 180 en réserve.

**Tooltip proposé :**
> Store 200% of Damage you Block as a reserve.
> Casting a spell consumes 10% of your current reserve and adds the consumed amount as base Damage matching that spell's damage type.

**Builds visés :** blocage et lancement de sorts pour convertir la défense en dégâts.

**À préciser :** conserver ou supprimer l'alimentation par absorption et son taux,
capacité de réserve et éventuelle progression avec armure/vie, expiration, sorts
éligibles et allocation aux DoT/multicibles. Un même dégât ne doit pas être compté
deux fois comme bloqué et absorbé. Le tooltip ne promet pas encore l'absorption.

## Legacy of the Nine — Amulet

Ancien nom : L’Héritage des Neuf. **Donjon :** Dire Maul West.

**Effet retenu :** les compétences de toutes les classes peuvent apparaître dans
les choix, **sans prérequis d'arme**. Leurs poids habituels ne changent pas ;
l'élargissement du pool dilue naturellement les probabilités individuelles.
Aucun bonus de dégâts ou de soins selon les classes représentées. Le nombre
habituel d'emplacements est conservé. Les compétences gardent leurs tags et leurs
conditions autres que les prérequis d'arme. L'objet n'accorde pas les talents.

**Tooltip proposé :**
> Skills from every class can appear in your skill choices.
> Skills have no weapon requirements. Their normal selection weights and your skill slot limit are unchanged.

**Builds visés :** combinaisons de compétences entre classes.

**À préciser :** compétences partagées, autres conditions d'utilisation et retrait
de l'amulette après l'apprentissage de compétences hors classe.

## Blood of the Last King — Helmet

Ancien nom : Le Sang du Dernier Roi. **Donjon :** Lower Blackrock Spire.

**Nouvel effet :** les modificateurs **increased Damage sont convertis en
increased Maximum Life à 10 % d'efficacité**. Par exemple, 100 % increased Damage
devient 10 % increased Maximum Life et ne donne plus son bonus de dégâts.
Le concept précédent remplaçant les dégâts de base par la vie maximale est abandonné.

**Tooltip proposé :**
> Increases to Damage are converted to increases to Maximum Life at 10% of their value.

**Builds visés :** convertir un investissement offensif en très grande vie.

**À préciser :** catégories d'increased Damage concernées (génériques, école,
attaque, sort, DoT), bonus conditionnels et réductions éventuelles. Les multiplicateurs
more/less ne sont pas des increased : aucune conversion de ceux-ci n'est décidée.
Ne pas convertir plusieurs fois un même bonus à cause de plusieurs tags.

## Hands of Sacrilege — Gloves

Ancien nom : Les Mains du Sacrilège. **Donjon :** Stratholme.

**Effet retenu :** chaque soin produit inflige aussi des dégâts Holy autour de son
bénéficiaire, même à pleine vie. Les compétences de soin peuvent se déclencher
en combat sans attendre une blessure. Un soin de zone déclenche **une activation
par cible soignée** : les explosions peuvent toucher plusieurs fois le même ennemi
lorsqu'il est proche de plusieurs bénéficiaires (shotgun autorisé).

**Tooltip proposé :**
> Your healing also deals Holy Damage to enemies near each recipient, even at full Life.
> Healing skills can cast in combat at full Life. Area healing triggers once per recipient; overlapping explosions can hit the same enemy.

**Builds visés :** soins de groupe, soins périodiques et alliés regroupés au contact.

**À préciser :** taux de conversion, rayon, soin théorique ou effectif, régénération,
vol de vie, soins de serviteurs et copies. Les dégâts convertis ne doivent pas
recréer indéfiniment des soins puis des dégâts via le vol de vie.

## Oath of the Final Second — Ring

Ancien nom : Le Serment de la Dernière Seconde. **Donjon :** Stratholme.

**Effet retenu :** les dégâts reçus sont étalés sur **4 secondes** au lieu d'être
retirés immédiatement. **Aucun effacement de dette par les dégâts ou les soins.**
Les soins et la régénération restaurent normalement la vie pendant le prélèvement ;
si la récupération ne suffit pas, les dégâts différés peuvent tuer le joueur.
Chaque nouveau coup possède son propre étalement : il ne repousse pas les précédents.

**Tooltip proposé :**
> Damage you take is dealt to you over 4 seconds instead of immediately.
> Deferred Damage cannot be cancelled and can kill you. Healing restores Life normally without reducing the remaining deferred Damage.

**Builds visés :** régénération et soins capables de compenser des dégâts lissés.

**À préciser :** ordre après défenses/absorption, DoT entrants, immunité et retrait
de l'anneau. Le prélèvement ne doit ni se différer à nouveau ni redéclencher les
effets de réception d'un coup. Les dépenses de vie restent distinctes des dégâts.

## The Faceless Visage — Helmet

Ancien nom : Le Visage de Personne. **Donjon :** Dire Maul West.

**Effet retenu :** un sort déjà appris peut être proposé comme une nouvelle instance
pour un emplacement libre, **sans apparition garantie**. Les poids de tirage ne
changent pas. Chaque instance possède **son propre niveau**, ses améliorations
et son cooldown. Les cartes peuvent améliorer une instance précise. Chaque copie
occupe un emplacement normal ; l'objet ne remplit pas automatiquement la barre.

**Tooltip proposé :**
> Skills you already know can appear again as new copies in your skill choices, with unchanged selection weights.
> Each copy occupies a skill slot and has its own level, upgrades and cooldown. Duplicate choices are not guaranteed.

**Builds visés :** plusieurs instances de Fireball ou Blizzard spécialisées différemment.

**À préciser :** présentation des copies dans les cartes, plafonds de niveau,
cooldowns internes partagés ou non pour les procs, retrait avec des doublons et
interaction avec Legacy of the Nine. Gospel of the Single Spell conserve sa
restriction à une seule compétence offensive.

## Crown of Cold Ashes — Helmet

Ancien nom : La Couronne des Cendres Froides. **Donjon :** Dire Maul East.

**Effet retenu :** s'applique aux **dégâts périodiques des compétences DoT**, et
non aux afflictions ou poisons appliqués séparément par les impacts, objets ou mods.
Toutes les secondes (cadence indicative), une partie du DoT d'une cible encore
vivante est copiée sur un ennemi proche. Sans voisin valide, la propagation renforce
le DoT d'origine. Les copies peuvent se recopier et gardent leur propriétaire.
L'éligibilité dépend de la compétence DoT : l'école Nature n'est pas exclue en soi.

**Tooltip proposé :**
> Damage over Time from your DoT skills spreads from living enemies to a nearby enemy every second.
> With no nearby enemy, it strengthens the original DoT instead. Copies can spread again.
> Applies to DoT skills, not separately applied ailments or poisons.

Les copies peuvent à leur tour se propager aux pulsations suivantes. 

**Builds visés :** compétences DoT comme moteur de nettoyage et de dégâts sur boss seul.

**À préciser :** liste des compétences DoT, rayon, fraction copiée, voisin choisi,
durée restante et cadence par instance. Plafonner le renforcement sur cible isolée,
définir la fusion des copies et les interactions avec les autres propagations.

## Tyrant's Bulwark — Shield

Ancien nom : Le Rempart du Tyran. **Donjon :** Blackrock Depths.

**Effet retenu :** permet de bloquer les sorts et les dégâts de zone et fixe la
**chance maximale de blocage à 85 %**, sans accorder automatiquement 85 % de chance.
Chaque blocage réussi rend disponible une compétence offensive en recharge.
Une même attaque ne peut déclencher cet effet qu'une fois, même avec plusieurs
composantes de dégâts.

**Tooltip proposé :**
> You can Block spells and area damage. Your maximum Block Chance is 85%.
> Blocking resets the cooldown of one offensive skill. Each incoming attack can trigger this effect only once.

**Builds visés :** blocage élevé et compétences à longue recharge.

**À préciser :** sélection de la compétence réinitialisée, éventuel cooldown interne,
zones persistantes et ticks de DoT. Prévoir les charges, copies, Hourglass of
Stolen Time et Riposte. Le tooltip n'accorde pas explicitement le blocage des DoT.

## Ring of Absolute Hunger — Ring

Ancien nom : L’Anneau de la Faim Absolue. **Donjon :** Dire Maul East.

**Effet retenu :** le vol de vie continue à pleine vie et son excédent alimente une
réserve qui augmente **uniquement les dégâts**, sans vitesse d'action.
La réserve se vide continuellement, de plus en plus vite à mesure qu'elle monte.
Maintenir des valeurs extrêmes demande un apport croissant de vol de vie.

**Tooltip proposé :**
> Life Leech continues at full Life. Excess Life Leech fills a reserve that increases your Damage.
> The reserve drains continuously. The higher it grows, the faster it drains.

**Builds visés :** vol de vie élevé et dégâts soutenus sur cibles résistantes.

**Piste non validée :** perte par seconde `a × réserve + b × réserve²`, avec
`a > 0` et `b > 0`.

**À préciser :** unité, capacité, conversion en dégâts, vol de vie instantané ou
étalé, comportement hors combat et retrait. Définir les cumuls avec Pact of Two
Lives et Hands of Sacrilege.

## Testament of the First Mage — Two-Handed Weapon

Ancien nom : Le Testament du Premier Mage. **Donjon :** Scholomance.

**Effet retenu :** les compétences offensives sans dégâts périodiques perdent leur
cooldown et consomment une réserve de puissance commune. Cette réserve se régénère
naturellement ; la récupération des cooldowns augmente sa régénération.
Chaque compétence conserve un coût lié à sa puissance. Les compétences offensives
avec dégâts périodiques restent hors de cette conversion.

**Tooltip proposé :**
> Offensive skills without Damage over Time have no cooldown and consume Power instead.
> Power regenerates naturally. Cooldown Recovery increases Power regeneration. Each eligible skill has its own Power cost.

Les buffs sont explicitement exclus de cette conversion.

**Builds visés :** alternance entre petites dépenses et sorts puissants.

**À préciser :** capacité, régénération, coûts, cadence maximale et priorité
d'autocast. Aucune infinité de lancements par frame. Définir canalisations, zones
persistantes, DoT ajoutés, serviteurs, répétitions et coûts associés. Les deux
armes Testament of the First Mage et Gospel of the Single Spell ne sont normalement
pas équipables ensemble.

## The Patient Executioner — Trinket

**Nouveau concept ; nom et emplacement proposés. Donjon proposé :** Scholomance.

**Effet retenu — courbe quadratique renforcée :** multiplier les dégâts d'une compétence
par **`1 + C + 0,1 × C²`**, où `C` est son cooldown en secondes.
Le coefficient quadratique accentue progressivement les longs cooldowns.
Le multiplicateur reste strictement supérieur à `1 + C` pour tout cooldown positif,
et vaut ×1 à cooldown nul. Les coefficients restent une proposition d'équilibrage.

| Cooldown | Multiplicateur de dégâts |
|---|---|
| 0 s | ×1 |
| 0,1 s | ×1,101 |
| 1 s | ×2,1 |
| 2 s | ×3,4 |
| 5 s | ×8,5 |
| 10 s | ×21 |

**Tooltip proposé :**
> Your skills deal more Damage the longer their cooldown.

**Point à revoir :** l'objectif noté était de favoriser les builds low cooldown,
mais la formule augmente les dégâts par lancement des compétences à long cooldown.
La courbe conserve cette direction. Préciser si « low cooldown » désigne plutôt
une faible récupération des cooldowns. Vérifier les cooldowns extrêmes avant de
fixer un éventuel plafond, tout en respectant le minimum `1 + C`.

**À préciser :** cooldown de base ou effectif après récupération, moment de lecture,
DoT, charges, répétitions, cooldown nul avec Testament of the First Mage et resets
de Tyrant's Bulwark. Le tooltip final devra indiquer la base choisie, sans confondre
la durée normale du cooldown avec le temps restant avant le prochain lancement.

## Choix de première implémentation

- IDs stables 401–415 dans l'ordre des fiches, niveau de base minimum 60.
  Poids par chase : 1/10 contre 1 pour chaque unique ordinaire (dix fois plus rare), exclusivement
  dans son donjon, y compris pour les récompenses personnelles en groupe.
- HEAD HUNTER sélectionne uniformément un mod du monstre, sans plafond de mods différents.
- Skin ne stocke que les dégâts bloqués, sans plafond de réserve. Les absorptions
  n'alimentent pas la réserve et les critiques restent disponibles.
- Hands : 100 % du soin théorique, rayon 6 yards. Une explosion par bénéficiaire,
  sans nouveaux déclenchements sur impact issus des dégâts convertis.
- Crown : pulsation chaque seconde, rayon 8 yards, copie à 25 %, durée restante
  conservée. Les copies peuvent se propager. Une instance propagée par propriétaire,
  compétence et cible ; les arrivées concurrentes conservent la plus forte.
  Le renforcement isolé est plafonné à trois fois la puissance initiale.
- Bulwark réinitialise la plus longue recharge offensive restante ; les DoT ne
  sont pas bloquables. Le plafond de 85 % n'accorde pas de blocage gratuit.
- Hunger : chaque équivalent de vie maximale stocké donne 100 % more Damage.
  La réserve normalisée H perd `0,1H + 0,1H²` par seconde, en et hors combat.
  Le débit de vol de vie conserve son plafond natif. Aucun gain de vitesse.
- Testament : 100 Power, régénération 25/s ; coût de 10 Power par seconde de
  cooldown effectif d'origine, minimum 1 et maximum 100. Buffs, serviteurs et zones
  persistantes exclus. Intervalle technique minimal de 0,1 s entre activations.
- Executioner utilise le cooldown effectif après récupération, jamais le temps
  restant. Le Testament compte comme cooldown nul pour son multiplicateur.
- Gospel conserve la première compétence offensive d'une barre déjà remplie :
  les suivantes sont inactives, sans être supprimées. Le choix de nouvelles
  compétences offensives est bloqué tant qu'une offensive est déjà possédée.
- Les dettes d'Oath continuent après retrait de l'anneau, sans seconde mitigation
  ni déclenchement de coups reçus. Le journal conserve la source et ses mods.

## Obtention et rareté à définir

- Rareté distincte dans le pool des uniques pour que ces objets restent des chases.
- Définir les boss, taux, seuils d'ilvl/difficulté et éventuelle protection contre la malchance.
- Une source de raid nécessite une décision distincte des exclusivités de donjon.
- Les tooltips sont des propositions de texte ; les valeurs et règles encore ouvertes
  restent ajustables dans la configuration dédiée. Les tooltips du catalogue décrivent les effets en jeu.
