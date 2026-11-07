# Uniques : catalogue et exécution

Les 160 définitions approuvées restent dans `idees-100-legendaires.txt`. Leur identité sauvegardée est un numéro stable ; l'Archive des Milestones (301) sauvegarde aussi son tirage parmi 81 paliers. Une seule copie de chaque unique peut être équipée. Les conversions absolues d'école sont exclusives.

Le code est dans `crates/benilla-app/src/survivors/items/legendary/` :

- `catalogue.rs` conserve les noms, emplacements, descriptions et affinités de classe.
- `casting.rs` résout les profils, copies, délais, emplacements et sélections de cibles.
- `periodic.rs` gère les familles d'altérations, transferts et budgets de ticks.
- `effects.rs` conserve les marques, réserves de soins, boucliers et interactions entre écoles.
- `milestones.rs` applique les paliers prêtés, leurs dépendances minimales et leur retrait.
- `events.rs` traite les 30 procs 311–340 avec des compteurs et délais propres au propriétaire.
- `movement.rs` applique les déplacements avec vérification du terrain et de la limite de zone.

Les projectiles restent ciblés. Une copie ou une explosion différée choisit ses victimes vivantes à son exécution ; elle ne crée pas de collisions physiques de projectile. Une cible guidée disparue annule son impact. Les invocations utilisent les composants et modèles existants.

Chaque impact et DoT conserve son origine : compétence, talent, objet ou unique. Les dégâts déclenchés par un unique ne peuvent pas créer des chaînes supplémentaires de procs, de talents ou de récupération par élimination. Une réserve transfère son budget déjà résolu ; elle ne réapplique pas les bonus de dégâts du propriétaire.

Le butin utilise `legendary = 5` dans `config/survivors-item-bases.toml` : 5 % des objets générés avant Magic Find, indépendamment de l'Ilvl. La probabilité devient `min(5 % × (1 + MF), 100 %)`, avec une MF interne de 1 pour +100 %. Les autres raretés gardent leur progression par Ilvl et MF. Les objets de blocage utilisent une base de bouclier. Une base de livre tenu en main secondaire permet les autres effets de cet emplacement sans permission de double arme. Les nouveaux personnages `TEST_` reçoivent quatre bijoux non équipés à +10 %, +100 %, +1 000 % et +10 000 % de MF.

Les vérifications couvrent notamment les 81 tirages de l'Archive, leur retrait, les origines des procs, les compteurs, les budgets périodiques, la sauvegarde et les systèmes Bevy de sorts transformés. Les tests ne remplacent pas une vérification visuelle en partie ni l'équilibrage des combinaisons d'uniques.
