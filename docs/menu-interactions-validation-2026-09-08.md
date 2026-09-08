# Corrections du canvas Interactions — 8 septembre 2026

## Périmètre

Correction du verrouillage de navigation par une saisie non validée,
commandes persistantes de validation/annulation, recherche depuis une sortie
ou un clic droit, pan droit/central, zoom centré sur le pointeur et cadrage.
Le format des menus et le moteur narratif ne sont pas modifiés.

La recherche présente les neuf opérations d’interface existantes : les pins
de ces graphes sont des pins d’exécution. Elle ne propose pas les valeurs ou
les nœuds narratifs incompatibles. Les paramètres restent configurés dans
l’inspecteur avant l’ajout, afin de valider les références avant mutation.

## Vérifications

- Tests release de l’éditeur : 28 réussis, dont quatre tests nouveaux sur la
  recherche et les liaisons (entrée, branches indépendantes, rejet des liens
  invalides et des opérations sans changement).
- `tools/verify_menu_interactions.sh` : test natif reproductible dans une copie
  temporaire du modèle Sobre, sans écriture dans le projet de l’utilisateur.
- Recherche « son » depuis un pin, annulation sans changement du document.
- Ajout positionné et connecté, puis annulation/rétablissement en une étape.
- Changement de l’opération vers animation, boutons accessibles à 1280×650,
  pan droit et zoom malgré la saisie ; Échap et enregistrement conservent les
  opérations, positions et connexions précédentes.
- Pan central et cadrage clavier sans modification des données.
- Captures inspectées et exécutable release reconstruit. Instances de test fermées.

Cette validation porte sur les corrections ci-dessus ; elle ne constitue pas
une nouvelle certification de toutes les fonctions de personnalisation.

## Passe suivante : contexte sélectionné et lancement de l’histoire

- Les événements sont filtrés par propriétaire, avec un sélecteur séparé pour
  les événements de page. Test natif : Continuer/button_0 avec survol → Nouvelle
  partie/button_1 sans événement → retour à button_0 ; le graphe attendu revient.
- Les modes Design et Interactions conservent désormais des caméras distinctes.
- Bouton Jouer/Arrêter dans le canvas principal : compilation en mémoire, aucun
  écrasement de l’export existant, copie temporaire des menus et traductions,
  ressources du projet en lecture, sauvegardes et progression de test isolées.
- Le moteur est lancé puis arrêté réellement depuis le bouton. Comparaison des
  empreintes et inventaires avant/après : tous les fichiers source sont inchangés.
  Journal : `/tmp/rvn-game-test-364872-1788847855247205344/preview.log`.
- Tests release RVN : **319 réussis** ; tests éditeur : **30 réussis**.
- La nouvelle matrice exerce **7 événements × 9 opérations**, vérifie le ciblage,
  la préservation du document et les branches vraie/fausse. Elle ne remplace pas
  les essais du rendu et des périphériques.
- Suite native moteur : **26 parcours réussis** sur la seconde exécution complète,
  rapport `/tmp/rvn-menu-native.OpOQ81`. Le premier passage de pagination a échoué
  dans une assertion de page ; sa relance isolée puis la suite sans autres fenêtres
  de test ont réussi. La cause exacte de cette intermittence n’est pas établie.
- Régression éditeur : `/tmp/rvn-interactions.95tZaG`, reproduisible avec
  `bash tools/verify_menu_interactions.sh`.

Le bilan fonctionnel et les limites sont détaillés dans
[État de l’éditeur et comparaison avec Ren’Py](editor-status-and-renpy-2026-09-08.md).
