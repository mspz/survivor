# Journal de mort

Le journal conserve les événements de dégâts des dix secondes précédant le coup
fatal. Le récapitulatif ne s'ouvre jamais automatiquement : le bouton
« Journal de mort », intégré à l'écran de mort, permet de l'ouvrir puis de le
rouvrir après fermeture. Le panneau reprend le style sombre à bordure dorée des
Options et du debug du damage meter, avec cartes de résumé et événements défilants.
Échap ou « Fermer » revient à l'écran de mort.

Chaque entrée affiche :

- Le temps restant avant la mort, le coup fatal étant à zéro seconde.
- Le nom et l'identifiant de la source, et son type : mêlée, caster ou boss.
- L'attaque ou l'effet responsable, l'école de dégâts, les critiques et les DoT.
- Les mods du monstre ayant produit les dégâts, même si sa configuration change
  ensuite ou qu'il disparaît. Un monstre sans mods est distingué d'une source
  dont les mods ou le type ne sont pas connus.
- La vie réellement perdue, les dégâts bruts avant les défenses du joueur,
  les absorptions, la vie avant/après et le surplus létal.
- Les blocages, esquives et impacts entièrement absorbés, sans les compter
  comme de la vie perdue. Les coups empêchés par l'immunité ou une sélection de
  récompense ne sont pas des dégâts reçus et ne sont pas enregistrés.

Les composantes de dégâts d'un même coup peuvent apparaître sur plusieurs lignes
si elles appartiennent à des écoles différentes. Le total de dégâts correspond
à la vie réellement perdue, sans ajouter le surplus létal ni les absorptions.

Le journal est figé dès le coup fatal. Après une réanimation, le prochain événement
démarre une nouvelle fenêtre de vie ; le dernier récapitulatif reste conservé
jusqu'à la prochaine mort ou la remise à zéro de la partie. Son bouton n'est
visible que sur l'écran de mort. Une mort sans dégâts
enregistrés affiche explicitement un récapitulatif vide.

En multijoueur, l'hôte enregistre les dégâts dans le journal du joueur concerné.
Chaque invité reçoit seulement son propre rapport, une fois par mort sur la
connexion TCP. Les snapshots suivants conservent le rapport déjà reçu sans le
retransmettre ; une nouvelle partie efface l'ancien rapport.

La mémoire est bornée à 2 048 événements pour la fenêtre courante. Si un combat
extrêmement dense dépasse cette capacité, les plus anciens événements sont
retirés et le récapitulatif signale que la fenêtre est incomplète.
