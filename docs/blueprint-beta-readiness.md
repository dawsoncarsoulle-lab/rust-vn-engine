# Blueprint : état de la passe bêta du 6 septembre 2026

## Ajouts de cette passe

- Une entrée principale et plusieurs labels nommés dans un même graphe, utilisables avec les sauts/appels RVN.
- Création d’un graphe depuis le `+` de Graphes ; export des graphes du projet et des onglets ouverts dans `project.generated.rvn`.
- Vérification des labels dupliqués et des destinations manquantes avant export. Un fichier homonyme écrit à la main n’est pas écrasé.
- Dans Choix : texte et expression de disponibilité par réponse. Une condition vide laisse la réponse disponible. Les anciennes conditions câblées restent conservées.
- Entrées câblables pour les fonctions et listes ; bouton `+` et nombre d’entrées dans l’inspecteur. Les anciennes listes textuelles sont conservées à la migration.
- Édition visuelle des rectangles d’une carte interactive, avec rectangles de survol indépendants et validation annulable.
- Activation individuelle des propriétés d’effet de sprite et paramètres d’animation typés.
- Nœud dédié `Make Color` (recherche : couleur, RGB), avec R/G/B de 0 à 255, A de 0 à 1 et aperçu carré de 60 unités sans contour. Les anciens nœuds normalisés sont convertis à l’ouverture ; une multiplication explicite par 255 préserve les sources câblées. Les scripts déjà exportés avec `make_color` restent compatibles, les nouveaux utilisent `make_color_rgb`.
- SET et GET : bandeau vitreux fin, silhouette et champs resserrés, couleurs de types conservées.
- Correction du parseur : premier élément des listes conservé et indexation du résultat d’une fonction.

## Choix de la porte

Pour afficher « Ouvrir la porte » même sans clé, laisser sa disponibilité vide. Brancher sa sortie sur une Condition testant `possede_cle`. La branche fausse affiche le dialogue d’échec puis saute vers le label placé avant le menu. La branche vraie ouvre la porte. Une condition de disponibilité `possede_cle` masquerait au contraire cette réponse.

## Vérifications effectuées

- Tests release du parseur, du moteur et de `rvn_graph` : notamment boucle choix → échec → dialogue → retour au choix, labels, export, fonctions/listes, conditions et effets.
- Tests release de l’éditeur et conversion couleur/hexadécimal.
- Instance graphique isolée : champs de choix, dessin et validation des zones, export, sélection d’une couleur et restauration avec un seul Annuler.

## Limites à ne pas confondre avec une validation complète

- C’est une bêta de test, pas une certification de parité totale avec UE5 ni une reproduction pixel-perfect vérifiée.
- Les réponses indisponibles sont masquées selon la sémantique RVN ; un mode grisé avec explication demanderait un ajout au moteur et au rendu.
- Les conditions restent des expressions RVN à saisir ; il n’y a pas encore de constructeur visuel de comparaisons dans chaque réponse.
- L’éditeur de zones permet de dessiner/redessiner les rectangles ; poignées de redimensionnement, déplacement et réorganisation avancée restent à compléter. La liste brute reste accessible.
- L’export assemble les scripts ; ce n’est pas un empaquetage autonome de tous les médias. Les imports externes complexes, notamment à dépendances communes, nécessitent encore des scénarios de validation supplémentaires.
- Une recette sur un jeu complet, avec rendu graphique, médias, sauvegarde/chargement et distributions finales, reste nécessaire avant de déclarer l’éditeur entièrement prêt.
