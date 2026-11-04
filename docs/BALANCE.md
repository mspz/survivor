# Équilibrage des classes et sorts

Modifier `config/survivors-balance.toml`, puis relancer le jeu.
Le fichier embarqué sert de repli si le fichier local manque ou est invalide.
`WOW_BALANCE_CONFIG` permet de choisir un autre chemin.

Chaque classe possède `multiplier` et une table `spells` contenant ses sorts.
Tous les multiplicateurs livrés valent `1.0`. Une entrée omise vaut également `1.0`.

```toml
[classes.warrior]
multiplier = 1.2

[classes.warrior.spells]
cleave = 0.8
```

Dans cet exemple, les effets du guerrier sont multipliés par 1,2 et Cleave par
1,2 × 0,8 = 0,96. Les valeurs autorisées vont de 0 à 1000, sans nombres infinis.
Un nom de classe ou de sort invalide rejette le fichier entier avec un avertissement.

Le facteur s'applique à la puissance finale après les bonus : dégâts directs,
dégâts périodiques, soins, absorption et dégâts renvoyés. Les délais, durées,
portées, chances de critique, contrôles et statistiques du personnage restent inchangés.
Les tooltips et les aperçus des améliorations utilisent les mêmes valeurs que les sorts lancés.

## Multiplicateurs par tag

La section globale `[tags]` contient les 24 tags reconnus, tous à `1.0` par défaut.
Un tag omis vaut `1.0`. Chaque tag présent sur le sort apporte son propre facteur :
`classe × sort × tag1 × tag2 × …`.

```toml
[tags]
attack = 1.0
spell = 1.1
fire = 1.2
area = 0.9
```

Un sort de feu de zone reçoit ici `1.1 × 1.2 × 0.9 = 1.188`, avant les facteurs
de classe et de sort. Un sort portant deux écoles reçoit les deux facteurs.
Les noms correspondent aux tags du moteur : `cold` pour le givre, `dot` pour les
dégâts périodiques, `heal` pour les soins et `guard` pour la protection.

Comme les autres multiplicateurs, ils modifient uniquement la puissance : le tag
`area` ne change pas le rayon et `duration` ne change pas la durée. Les noms inconnus
ou valeurs invalides rejettent le fichier avec retour aux réglages embarqués.
