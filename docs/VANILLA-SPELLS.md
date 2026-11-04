# Sorts supplémentaires de Vanilla

Deux sorts sont ajoutés au pool de récompenses de chaque classe. Ils peuvent être
appris et améliorés comme les sorts existants, dans la limite habituelle des six
sorts actifs. Leur poids de récompense et leur multiplicateur d'équilibrage valent 1.

| Classe | Sorts | Spell.dbc |
| --- | --- | --- |
| Guerrier | Pourfendre, Fulgurance | 772, 7384 |
| Paladin | Lumière sacrée, Sceau de piété | 635, 20154 |
| Chasseur | Attaque du raptor, Coupure d'ailes | 2973, 2974 |
| Voleur | Attaque sournoise, Hémorragie | 53, 16511 |
| Prêtre | Fouet mental, Rénovation | 15407, 139 |
| Chaman | Frappe-tempête, Vague de soins inférieurs | 17364, 8004 |
| Mage | Brûlure, Barrière de glace | 2948, 11426 |
| Démoniste | Drain de vie, Siphon de vie | 689, 18265 |
| Druide | Balayage, Rétablissement | 779, 8936 |

Les identifiants utilisent les icônes et chaînes visuelles des DBC locaux, avec
les animations de mêlée existantes pour les attaques physiques. Aucun effet graphique
externe n'est ajouté.

Les mécaniques sont adaptées au lancement automatique de Survivors : pas de condition
de position pour Attaque sournoise ni d'esquive préalable pour Fulgurance ; le Sceau
de piété inflige des pulsations sacrées à courte portée ; Fouet mental et les drains
utilisent le système de dégâts périodiques. Les sorts de soin se déclenchent sous
85 % de vie, avec le système existant de soins dans le temps. Rétablissement étale
ses soins sur huit secondes. Barrière de glace utilise l'absorption existante.

`config/survivors-balance.toml` règle leur puissance ;
`config/survivors-rewards.toml` règle leur fréquence d'apparition.
