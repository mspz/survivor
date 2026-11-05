# Multijoueur natif Survivors

Une instance héberge le monde et peut accueillir sept invités (huit joueurs au
total). Chaque machine doit disposer de ses propres données WoW 1.12.1. Le mode
sans argument reste le solo, avec son serveur dans le même processus.

## Salons depuis la sélection du personnage

Lancer le jeu normalement :

```powershell
cargo survivors
```

Après avoir choisi un personnage, trois actions sont proposées :

- **Entrer dans le monde** démarre une partie solo.
- **Créer un salon** héberge une partie sur le port TCP fixe 8091.
- **Rejoindre un salon** ouvre une fenêtre pour saisir ou coller l’IPv4 de l’hôte.

Tous les joueurs se connectent au même réseau Hamachi. L’hôte crée un salon et
communique son IPv4 affichée dans Hamachi (par exemple `25.12.34.56`). Les invités
saisissent cette adresse seule, sans port, dans **Rejoindre un salon**. Ils peuvent
utiliser **Coller l’IPv4** ou `Ctrl+V`, puis **Rejoindre**. L'entrée dans le monde attend
la confirmation de l'hôte ; la connexion reste annulable. Une faute de saisie,
un salon fermé ou un hôte inaccessible affiche une erreur dans le menu.

Le panneau du salon indique le nombre de joueurs et permet de **Quitter le salon**.
Quitter revient à la sélection du personnage ; il est
possible de créer, rejoindre ou lancer un solo sans redémarrer le jeu.

L’hôte écoute sur toutes les interfaces, y compris Hamachi. Autoriser le jeu dans
le pare-feu Windows sur le réseau utilisé ; le port TCP 8091 doit être accessible.
Une IPv4 de réseau local fonctionne également. Il n’y a plus de code généré,
de détection automatique d’adresse ni de variable `SURVIVORS_ADVERTISE`.
La connexion est directe, sans annuaire public ni relais Internet. Un seul salon
peut écouter sur le port 8091 d’une machine à la fois.

## Héberger et rejoindre en ligne de commande

Sur la machine hôte, depuis le dépôt :

```powershell
cargo survivors -- --host
```

L'hôte écoute sur TCP 8091. Pour choisir une adresse ou un autre port :

```powershell
cargo survivors -- --host 0.0.0.0:8091
```

Sur chaque machine invitée, remplacer l'adresse par l'IP de la machine hôte :

```powershell
cargo survivors -- --join 192.168.1.20:8091
```

Pour deux instances sur la même machine, utiliser `127.0.0.1:8091`. Les options
équivalentes pour les lanceurs intégrés sont `SURVIVORS_HOST` et `SURVIVORS_JOIN` ;
elles s'excluent mutuellement. Aucun compte de realm WoW n'est nécessaire.

Chaque joueur choisit son personnage au menu et entre dans le monde. L'hôte
choisit l'expédition et la lance ; les invités suivent sa carte. Le titre de la
fenêtre indique le rôle et la connexion. Le port TCP choisi doit être accessible
depuis les machines invitées. Cette version vise le réseau local ou un VPN ; elle
n'ajoute ni annuaire public, ni traversée NAT, ni authentification Internet.

## Règles de la session

Le serveur de l'hôte décide des positions, vitesses, collisions de terrain,
sauts, mobilités, dégâts, points de vie, sorts, ennemis, butin et récompenses.
Les invités transmettent leurs intentions et leur sauvegarde locale. Chaque invité possède
son état de partie, ses cooldowns, ses points de vie, son XP et ses cartes de
récompense. Une récompense est validée par l'hôte contre l'offre courante ; un
ancien clic ne peut pas appliquer une deuxième récompense.

Les ennemis ciblent le joueur vivant le plus proche. La mort de l'hôte ne termine
pas la partie tant qu'un invité est vivant. Une nouvelle expédition réinitialise
les personnages de la session. Les changements de carte décidés par l'hôte
concernent tout le groupe. En multijoueur, aucun joueur ne peut mettre la partie
en pause : menus, inventaire, talents et choix de récompense laissent le combat
continuer. Le solo conserve sa pause habituelle.

Un personnage ayant des récompenses de niveau à choisir est invulnérable jusqu'à
la validation du dernier choix. Cette protection est individuelle, gérée par
l'hôte pour chaque joueur, et ne consomme ni bouclier ni charge de blocage.
Les monstres, boss compris, ignorent également ce personnage et ses provocations
pendant ce choix. Ils ciblent un autre joueur disponible ou restent sur place.
Pendant ce temps, ce personnage ne lance plus de sorts et ses dégâts sont neutralisés,
y compris ceux des effets périodiques déjà posés. Les autres joueurs restent actifs.

L'XP ramassée est partagée avec tous les joueurs connectés, sans division et sans
limite de distance, y compris les personnages morts en attente de résurrection.
Chaque personnage conserve ses bonus personnels d'XP. Les gemmes d'XP restent
fixes et sont ramassées immédiatement à portée d'aimant ; leurs animations et
particules sont désactivées sur l'hôte et les invités.

Chaque joueur ouvre et modifie ses propres options. Les mises à jour de l'hôte
ne referment pas ses menus et ne remplacent pas ses préférences. Les nombres
de dégâts, de soins et de coups critiques sont transmis aux invités ; chacun
les affiche selon ses propres filtres, indépendamment de ceux de l'hôte.

Les sauvegardes appartiennent aux clients, comme en solo. L'invité transmet son
équipement, ses rolls, ses talents, son XP de personnage et son apparence à la
connexion, puis lorsqu'ils changent. L'hôte fait confiance à ces données et n'en
garde qu'une copie en mémoire ; il ne charge ni n'écrit les fichiers du personnage
invité sur sa propre machine. Il n'y a pas de vérification serveur du budget des
talents ou de la provenance des objets : la session est prévue entre amis.

Équiper, remplacer ou retirer un objet pendant une run transmet le nouveau stuff
à l'hôte. À réception, les statistiques du personnage concerné sont recalculées,
sans redémarrer la partie ni ses cooldowns en cours. Les anciens bonus sont
remplacés et les PV courants sont plafonnés si le maximum diminue. Les champs de
vie répliqués sont actualisés dès la réception, même lorsque l'hôte ouvre un menu.

L'XP de personnage, les victoires de zone et les objets gagnés en coopération sont
enregistrés sur le client dans les mêmes fichiers `benilla-config/characters/`
qu'en solo. Chaque invité reçoit ses propres rolls de butin de boss, déposés dans
son inventaire local. Les reçus cumulés d'XP et les identifiants de butin évitent
de créditer deux fois un paquet reçu à nouveau. Le client n'accuse réception d'un
objet qu'après son enregistrement ; l'hôte le conserve dans ses messages en
attente jusque-là. Les sauvegardes locales restent utilisables en solo et lors
d'une nouvelle connexion.

La déconnexion d'un invité retire son personnage, ses effets persistants et ses
commandes. Si l'hôte disparaît, les invités sont déconnectés ; il n'y a pas de
migration d'hôte ni de reprise automatique. Le menu permet de rejoindre
à nouveau, avec un nouvel identifiant, une nouvelle partie et sa sauvegarde locale.
Changer de personnage termine la connexion afin de ne pas mélanger les fichiers
de deux personnages.

## Transport et rendu

Le TCP non bloquant utilise des messages JSON encadrés par une longueur sur
quatre octets, un numéro de protocole et des numéros de commande. Une socket est
associée à un seul identifiant attribué par le serveur : le client ne choisit pas
l'identité du personnage à déplacer. Les axes et valeurs numériques sont validés.
Après 250 ms sans commande, les mouvements sont neutralisés ; une connexion sans
activité réseau est fermée après 20 secondes. Les files et la taille des messages
sont bornées.

Les invités émettent à 30 Hz et l'hôte publie un état complet à 10 Hz. Les états
contiennent les GUID stables, modèles, apparences, équipements visuels, positions,
orientations, états de mouvement et champs de jeu de toutes les unités. Les
événements de sorts, impacts et attaques sont également transmis. Le client
reconstruit ses propres entités et utilise le pipeline existant de rendu et
d'animation ; aucun identifiant d'entité Bevy ne traverse le réseau. Les positions
sont interpolées sur 100 ms pour les autres unités. Le déplacement et le saut
du personnage local sont prédits à chaque frame, avec les mêmes règles que
l'hôte et la vitesse qu'il transmet. À réception d'un état, les mouvements non
acquittés sont rejoués et les petites corrections sont lissées. Les téléportations,
changements de zone et changements d'état de mort réinitialisent cette prédiction.

Le combat et les compétences de mobilité restent validés par l'hôte. Les états
complets ne sont pas encore compressés ni envoyés sous forme différentielle.
Le transport navigateur et le serveur dédié sans renderer ne font pas partie de
ce mode natif hébergé par un joueur.

## Vérification

```powershell
cargo check -p warcraft-survivors
cargo test -p benilla-app --lib survivors::multiplayer
```

Les tests couvrent le protocole, le transport TCP sur boucle locale, deux invités
déplacés indépendamment par les règles existantes, la reconstruction des avatars,
la prédiction sans nouvel état réseau, les menus sans pause, les filtres de
nombres de combat, la déconnexion et le rejet des anciennes offres de récompense. Ils ne remplacent
pas une vérification visuelle avec les données du jeu et plusieurs fenêtres.

## Unit-frame status

Protocol version 6 replicates each selectable unit's name, level, real current and
maximum health, friendly/hostile state and active debuff durations. The host builds
this status from the authoritative run and enemy effects. Guests select targets
locally by clicking their replicated models; no target command is sent to the host.
Player names are included in the join profile. Host and guests must use the same build.

