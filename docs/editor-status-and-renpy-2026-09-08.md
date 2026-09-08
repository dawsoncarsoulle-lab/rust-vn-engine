# État réel de l’éditeur et comparaison avec Ren’Py

État au 8 septembre 2026, après correction du contexte des interactions et ajout
du lancement du jeu. Les anciens journaux de progression sont historiques :
leurs listes de manques ne doivent pas être considérées comme l’état actuel.

## Ce que l’on peut créer

Un visual novel complet : dialogues, embranchements, variables et comparaisons,
labels, sauts, appels/retours, plusieurs fins, décors, sprites et effets, musique,
sons, voix, transitions et cartes cliquables. Le catalogue Blueprint expose ces
constructions. Une histoire longue n’exige pas à elle seule une fonction spéciale
de Ren’Py ; la complexité des mécaniques et de l’interface détermine les limites.

L’interface est personnalisable par pages, disposition, ancres et conteneurs,
images et cadres neuf zones, couleurs, polices, états interactifs, styles partagés,
composants, cartes et listes liées aux données. Dialogue, réponses, historique,
sauvegardes et galerie disposent de contenus dynamiques. Les images de référence
très illustrées nécessitent leurs propres ressources graphiques et polices :
l’éditeur compose ces ressources, il n’est pas un logiciel de dessin.

## Limites fonctionnelles actuelles

1. **Images différentes pour chaque réponse du scénario** : la liaison de réponse
   proposée est `choice.text`, pas un couple texte/image par option. Un cadre et
   une image communs au modèle sont possibles ; les trois cartes Frog/Shark/Fox
   avec illustration propre à chaque réponse ne sont pas entièrement disponibles
   par cette liaison visuelle.
2. **Catégories de sauvegarde Auto / Rapide / Manuelle dans les listes visuelles** :
   pagination et commandes rapides existent, mais pas une interface complète de
   catégories reliées à chaque banque comme dans les références fournies.
3. **Animation de l’interface** : fondu, déplacement, échelle et couleur avec durée
   et interpolation. Pas de timeline à clés ni d’équivalent complet d’ATL.
4. **Logique de jeu personnalisée** : les graphes d’interface manipulent leurs
   variables locales et des commandes autorisées, pas directement les variables
   narratives. Un inventaire complexe, un combat, un mini-jeu temps réel ou des
   widgets entièrement nouveaux ne sont pas garantis sans extension du moteur.
5. **Débogage intégré** : lancement/arrêt, diagnostics et journal, plus l’overlay
   moteur existant. Pas encore de points d’arrêt sur les nœuds avec surbrillance
   du nœud exécuté et inspection synchronisée dans l’éditeur.

## Comparaison avec Ren’Py

Pour un VN classique à routes et fins multiples, les briques fondamentales sont
présentes. Cela ne signifie pas qu’un projet Ren’Py arbitraire puisse être importé
ou reproduit sans travail supplémentaire.

Ren’Py offre un langage d’écrans et des actions personnalisables en Python :
[Screens and Screen Language](https://www.renpy.org/doc/html/screens.html).
Il propose aussi des transformations/animations ATL :
[Transforms](https://www.renpy.org/doc/html/transforms.html), et des objets de rendu
programmables pouvant servir à des interactions ou mini-jeux :
[Creator-Defined Displayables](https://www.renpy.org/doc/html/cdd.html).
Notre éditeur visuel n’offre pas aujourd’hui toute cette liberté programmable.
Son avantage est de composer les fonctionnalités exposées sans écrire ces écrans
à la main, pas d’avoir déjà atteint une parité totale avec Ren’Py.

## Priorités avant une version stable

- Continuer les tests de changements de contexte et de saisies incomplètes :
  les bugs récents montrent que la couverture précédente ne suffisait pas.
- Qualifier des parties longues et des évolutions de projet après création de
  sauvegardes ; ne pas confondre réussite d’un parcours avec toutes ses variantes.
- Qualifier manettes physiques, différentes échelles DPI et systèmes cibles.
- Finaliser et tester la distribution autonome sur chaque plateforme annoncée.
- Livrer les deux manques de personnalisation des réponses/sauvegardes ci-dessus
  si l’objectif est de reproduire intégralement les références de l’utilisateur.

Conclusion : une bêta utilisable pour des VN complets dans le périmètre exposé,
pas encore un substitut universel à Ren’Py ni une version stable certifiée.
