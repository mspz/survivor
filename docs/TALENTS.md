# Talents Survivors

Le premier milestone adapte le sort de depart a la specialisation principale.
Il faut acheter ce milestone : les passifs seuls ne changent pas le sort.
Parmi les arbres ayant ce milestone, celui avec le plus de points gagne ;
une egalite favorise l'arbre le plus a gauche. Sans milestone, le sort de classe revient.
Le remplacement conserve le rang et le cooldown restant, et fusionne les doublons.

| Specialisation | Nouveau sort de depart |
|---|---|
| Warrior Protection | Thunder Clap |
| Paladin Holy / Protection / Retribution | Holy Shock / Consecration / Judgement |
| Hunter Beast Mastery / Survival | Multi Shot / Raptor Strike |
| Priest Shadow | Shadow Word Pain |
| Shaman Enhancement / Restoration | Stormstrike / Searing Totem |
| Mage Arcane / Fire | Arcane Missiles / Fireball |
| Warlock Affliction | Corruption |
| Druid Balance / Feral / Restoration | Starfire / Claw / Insect Swarm |

Les autres arbres gardent le sort de classe deja adapte a leur style.
Le druide Restauration debloque aussi Rejuvenation avec Bloom, tout en gardant
une attaque de Nature pour le solo. Les tooltips des premiers milestones indiquent
le remplacement. Le meme code s'applique a l'hote et aux invites ; les sorts
remplaces utilisent les cartes, statistiques et interfaces des sorts ordinaires.

La selection des expeditions presente une liste ordonnee avec l'etat de chaque
destination, son lieu et les niveaux des ennemis. Le panneau de droite decrit
la destination choisie et son objectif ; les actions restent en bas. Sur une
fenetre etroite, les details passent sous la liste. Le defilement est conserve
quand la selection change. Training Grounds a sa section Practice en tete pour
les personnages TEST_, distincte de la campagne.

Un personnage nomme `TEST_...` commence au niveau de personnage 60, avec 51 points
de talents. Le prefixe est aussi reconnu au chargement des sauvegardes. Ces
personnages ont acces a toutes les expeditions de la campagne sans victoire prealable,
ainsi qu'a `Training Grounds` depuis Stormwind. Cela s'applique aussi aux personnages
de test deja sauvegardes, sans modifier leur progression enregistree.
Le mannequin immortel reste pres du depart. Le mannequin, le boss et le centre
du pack forment un triangle equilateral de 60 yards de cote. Le pack contient
40 monstres en formation compacte de 8 par 5. Le boss reapparait 3 secondes apres
sa mort, chaque monstre apres 1 seconde. Tous les ennemis de test sont sans IA :
ils restent sur place, ne choisissent pas de cible et n'attaquent jamais. Leurs
statistiques ne croissent pas avec le temps, et l'expedition n'a pas de
victoire automatique. Les coups, critiques, procs et DoT sur le mannequin sont
comptabilises integralement par le damage meter, meme au-dela de ses points de vie.

Les familiers et demons choisissent aleatoirement parmi trois cibles proches du
combat de leur proprietaire, en privilegiant celles a portee. Ils conservent
leur choix entre 1,2 et 2,4 secondes pour eviter les changements incessants,
mais remplacent immediatement une cible morte ou disparue. Le premier coup
est decale de 0,05 a 0,55 seconde entre les invocations ; la cadence suivante
reste celle du sort. Ce choix est simule par l'hote, aussi pour les invites.
Au-dela de 100 yards de leur proprietaire, les invocations sont teleportees pres
de lui. Leur ancienne cible est oubliee, sans reinitialiser leur cooldown d'attaque.
Les pets ignorent aussi les ennemis au-dela de 100 yards du proprietaire et
abandonnent immediatement une cible qui sort de ce rayon, puis reviennent vers lui.

Le tirage des cartes et de leurs ameliorations tient compte des sorts de talent
appris, meme si aucun sort classique ne possede les tags correspondants. Les
demonistes peuvent ainsi obtenir Projectiles, Chaines et Fork pour leurs demons,
et les saignements ou poisons de talent rendent les cartes DoT utiles. Retirer
le dernier sort compatible retire ces cartes du tirage. Une invocation permanente
ne justifie pas a elle seule une carte Duree ; son effet temporaire doit en profiter.

Les sorts de proc appris apparaissent dans la barre de sorts en haut a gauche,
avec un repere P, et dans la categorie Spells de la fiche du personnage. Le
survol et la fiche partagent les memes statistiques resolues. Les valeurs
variables indiquent leur reference (par exemple pour 100 degats declencheurs).
Leurs valeurs suivent l'equipement et les cartes ; retirer le talent retire
l'icone. Ces sorts ne consomment pas d'emplacement de competence ou de carte.

Les jalons sur impact utilisent les tags de degats de la competence qui touche,
sans distinction Attack/Spell, classe d'origine ou forme (projectile, zone, etc.).
Un proc Feu accepte tout impact avec Fire ; Physique exige Physical, et une
condition de melee exige aussi Melee. Un proc sans restriction d'ecole accepte
les impacts de toutes les ecoles, physiques ou magiques. Les compteurs tous les X
impacts suivent ces memes conditions. Les degats d'invocations et des autres
sorts de proc sont aussi acceptes. Chaque impact conserve les jalons deja visites
dans sa chaine : un jalon ne peut pas se redeclencher dans sa propre descendance.
Les nouvelles attaques repartent avec une chaine vide. Les ticks de DoT ne sont
pas de nouveaux impacts. Les procs de bijoux conservent leur protection anti-boucle.

Les effets declenches par les jalons sont des sorts internes definis dans `procs.rs`.
Ils ne prennent aucun emplacement de sort et ne sont pas proposes comme competences.
Leurs tags de gameplay sont independants des sorts DBC utilises pour le visuel :

| Effet | Tags principaux |
|---|---|
| Saignement | Attack, Physical, Damage over time, Duration |
| Explosion de saignement | Spell, Physical, Area |
| Poison | Spell, Nature, Damage over time, Duration |
| Ignite | Spell, Fire, Damage over time, Duration |
| Peste | Spell, Shadow, Damage over time, Duration |
| Morsure du familier | Attack, Melee, Physical |
| Projectile du demon | Spell, Fire, Projectile |
| Projectile du totem | Spell, Nature, Lightning, Projectile, Totem |

Les bonus compatibles des cartes, de l'equipement et des talents passent par le
meme moteur que les sorts visibles : puissance, ecole, degats periodiques, duree,
zone, critique, repetition, projectiles, chaines et division selon les tags.
Un effet calcule en pourcentage utilise les degats du coup declencheur comme base,
puis recoit ses propres bonus. Une invocation utilise ses degats de base fixes.
Les degats periodiques, copies et propagations conservent leur calcul initial ;
ils ne reappliquent pas les multiplicateurs a chaque tick ou rebond.
Les choix de cartes tiennent aussi compte des sorts internes appris.
Les effets permanents n'expirent pas et les bonus de duree concernent les effets
temporaires. Les chaines de procs de talent ne peuvent pas boucler sur un jalon deja visite.
Les explosions de mort ont une base plafonnee a 300 avant les bonus compatibles.
La portee de base des soins et buffs de groupe est de 12 metres avant les bonus de zone.

Le premier jalon Farouche du druide, Blood Claws, donne une forme feline permanente
en plus du saignement. La forme suit les talents de chaque joueur, y compris les
invites. Retirer le talent restaure le corps d'origine sans modifier l'equipement.

Les trois spécialisations de la classe sont visibles simultanément. Chaque arbre
contient 19 talents sur sept rangées : 16 passifs de statistiques à cinq rangs et
trois jalons à un point. Les jalons nécessitent respectivement 10, 20 et 30 points
dans les rangées précédentes. Les deux derniers exigent aussi le jalon précédent.
Le budget de 51 points au niveau 60 est commun aux trois arbres.

Chaque arbre utilise 19 icônes distinctes, choisies en priorité parmi les talents
WoW de sa spécialisation. Les images partagées entre plusieurs rangs de sorts sont
dédupliquées pour éviter les répétitions.

Les passifs donnent désormais, par point : +5% de puissance, +4% de vie maximale,
+3% de célérité, +2% de critique ou +5% de zone d’effet. Selon la spécialisation,
le passif principal peut aussi donner +10% d’armure, +0,6 vie/s ou +3% de vitesse.
Les bonus de vie et d’armure multiplient leurs totaux et suivent les changements
d’équipement ; ils n’ajoutent aucune valeur fixe.
Les rangs existants bénéficient automatiquement de ces bonus renforcés.

Les passifs améliorent les statistiques du personnage. Les jalons modifient les
compétences compatibles avec le thème de la spécialisation. Les détails et chiffres
de chaque effet sont présentés au survol. Les talents se modifient en ville ; la
réinitialisation est gratuite. Les sauvegardes v1 à v3 conservent leur expérience
et remboursent les allocations lors du passage au format v4.

| Classe | Identité des trois arbres |
|---|---|
| Guerrier | Armes : armes et physique ; Fureur : saignements, vitesse et Poigne du titan ; Protection : provocation, blocage et tonnerre |
| Paladin | Sacré : dégâts sacrés et soins ; Protection : consécration et blocage réparateur ; Vindicte : sceaux et attaques de zone |
| Chasseur | Bêtes : loups et saignements ; Précision : projectiles, rebonds et division ; Survie : mêlée, pièges et dégâts périodiques |
| Voleur | Assassinat : poison et contagion ; Combat : vitesse et déluge de lames ; Finesse : critique et acharnement monocible |
| Prêtre | Discipline : détonations de bouclier ; Sacré : soins directs et dégâts sacrés ; Ombre : afflictions et vol de vie |
| Chaman | Élémentaire : chaînes et surcharge ; Amélioration : procs d’armes ; Restauration : totem de soin et de dégâts |
| Mage | Arcanes : puissance et concentration ; Feu : Enflammer et propagation ; Givre : ralentissement, gel et Fracasser |
| Démoniste | Affliction : contagion et explosions ; Démonologie : invocations ; Destruction : sorts lents et dévastateurs |
| Druide | Équilibre : arcanes et étoiles ; Farouche : mêlée, saignements et résistance ; Restauration : soins périodiques et buffs proches |

Les règles sont définies dans `talent_design.rs`, les états par personnage dans
`talent_runtime.rs`. Le moteur de sorts traite les bonus de projectiles, chaînes,
division, vitesse et dégâts. Les impacts déclenchés par un jalon peuvent activer
d'autres jalons compatibles sans boucler sur leur ascendance. Les effets sur la mort ne se répètent
qu’une fois par victime. La base des explosions de mort est plafonnée à 300 dégâts
avant les bonus, pour éviter qu’une vie de boss produise des dégâts illimités.

En multijoueur, l’hôte simule aussi les jalons de chaque invité et transmet les
unités invoquées comme les autres unités du monde. Les soins et buffs de groupe
sont résolus une fois pour l’ensemble des joueurs, avec une portée de base de 12 mètres.
Le bonus monocible se réinitialise quand le joueur change de cible. Les boss
subissent une durée réduite des gels ; la provocation cesse si son propriétaire
meurt ou se déconnecte.

Le compteur de dégâts identifie les effets déclenchés par le nom de leur jalon,
indépendamment du sort utilisé par leur effet. Par exemple, l’explosion de Fureur
apparaît sous **Bloodbath**, distinctement d’un véritable **Arcane Explosion**.
L’attribution suit aussi les dégâts périodiques et les invocations. Les bonus qui
renforcent un sort restent inclus dans les dégâts de ce sort.

Poigne du titan autorise deux armes à deux mains pour le guerrier. Les bâtons
restent exclus de cette permission. Perdre le talent remet la main gauche devenue
incompatible dans les sacs ; les objets ne sont pas détruits.
