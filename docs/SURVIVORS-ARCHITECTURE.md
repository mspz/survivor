# Survivors : client et serveur local

Le mode solo compose deux modules dans le même processus Bevy :

| Module | Responsabilité |
|---|---|
| `survivors/client.rs` | Entrées clavier/souris, caméra, HUD, menus et présentation. |
| `survivors/server.rs` | État de partie (`Run`), statistiques, RNG, cycle de vie et orchestration de la simulation. |
| `survivors/session.rs` | Registre des connexions, commandes par client et transport local par appels de fonctions. |
| `survivors/mod.rs` | Composition des deux modules et contexte commun du mode. |

Chaque frame suit cet ordre : lecture des entrées → envoi de la commande →
réception serveur → mobilité → déplacement → caméra. Après les entrées viennent
les ennemis, les sorts, les dégâts, le butin et les points de vie. Les conditions
de pause et de chargement de la simulation restent celles du mode solo.

Une commande exprime des intentions (axes, rotation, saut, mobilité, visée).
Elle ne contient ni vitesse, ni dégâts, ni position à imposer. Le serveur calcule
le déplacement à partir de ses statistiques et du terrain. Les axes sont résolus
après la mobilité pour conserver l'orientation d'une charge ou d'un Blink.

Le serveur démarre avec un registre `Sessions` vide. Chaque appel à `connect`
crée un identifiant `ClientId` et retourne un handle `LocalSession` associé à
une boîte de commandes indépendante. Le frontend solo initialise un seul handle ;
le registre peut en accueillir plusieurs sans remplacer la connexion existante.

Le client appelle `LocalSession::submit(&mut sessions, commande)`, puis le serveur
réceptionne les commandes de toutes les connexions dans la même frame, sans socket
ni thread. `ServerInput::for_client(id)` sélectionne uniquement les commandes de
ce client. Le dernier envoi remplace seulement son intention non consommée.
Sans nouvel envoi, ce client reçoit une commande neutre, sans bloquer les autres.

`Sessions::disconnect(id, &mut input)` supprime les commandes en attente et celles
de la frame courante. Un ancien handle ne peut plus envoyer de commandes, même
après une nouvelle connexion : les identifiants ne sont jamais réutilisés pendant
la vie du serveur. Les erreurs d'envoi signalent une connexion fermée ; le frontend
solo cesse simplement d'alimenter la simulation si son handle est invalidé.

Le transport natif hébergé est décrit dans [MULTIPLAYER.md](MULTIPLAYER.md).
`multiplayer/native.rs` associe une socket à chaque handle et conserve les états
individuels dans `Companions`. Les règles existantes de déplacement, mobilité,
sorts, vitaux et collecte tournent dans le contexte de chaque personnage, puis
restaurent le contexte local avant les systèmes de présentation. Les GUID sont
alloués par le monde de l'hôte, y compris pour les effets des invités. Les dégâts,
zones, totems et dégâts périodiques conservent leur propriétaire.

Les invités désactivent la simulation de combat locale et appliquent les états reçus à des
entités de rendu distinctes. `native/prediction.rs` prédit leur déplacement et leur
saut avec les règles partagées de `hero::step_motion`, puis réconcilie les états
autoritaires. Seul leur propre avatar reçoit le marqueur `Hero`,
pour la caméra et l'interface. Les autres avatars utilisent le même pipeline
d'animation, sans être pris pour le joueur local.

Les fichiers des personnages restent sur leurs clients. Le protocole transmet un
build de confiance (inventaire, talents et apparence) à l'hôte ; `native/saves.rs`
applique cette copie sans lui associer un chemin de sauvegarde côté hôte. Le
client enregistre l'XP, les victoires et le butin dans ses fichiers habituels.
Les reçus de progression et les accusés de butin évitent les doubles écritures
quand un état est renvoyé. Ils ne constituent pas un contrôle anti-triche.

L'hôte partage toujours son monde ECS entre simulation et présentation. Un serveur
dédié nécessitera d'extraire les dépendances au renderer et au chargement du
terrain. Le mode solo conserve ses accès directs aux ressources ; dans le mode
invité, les intentions de déplacement et les récompenses passent par le protocole serveur.
Les menus sont locaux et ne suspendent jamais la simulation multijoueur. Les nombres
de combat sont répliqués avant filtrage pour respecter les préférences de chaque client.
