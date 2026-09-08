# Éditeur visuel — livraison et vérifications du 8 septembre 2026

## Utilisation

Relancer l’éditeur release, ouvrir un projet puis **Menus du jeu**.
Les [trois projets d’exemple](../examples/menu-designs/README.md) sont entièrement
éditables et fournissent chacun onze pages ainsi qu’un paquet `.rvnuitheme`.
Ils ont été créés, personnalisés, enregistrés et exportés dans l’interface native,
sans écriture manuelle de leur composition JSON.

Le [guide](menu-designer-guide.md) décrit les calques, ancres, composants, styles,
cartes liées aux données, interactions, effets, aperçu isolé et partage de designs.

## Corrections finales

- Saisie hexadécimale du sélecteur de couleur : position correcte, pas de clic
  traversant vers l’inspecteur, une seule étape d’annulation par geste.
- Styles nommés et renommables ; partage d’une seule couleur sans remplacer
  les dimensions, polices ou autres états ; exceptions locales conservées.
- Événements des contrôles hérités résolus comme leurs valeurs et leurs actions.
- Recadrage des images en mode Remplir, sans couper leurs enfants ou ombres.
- Champs automatiques des cartes : le texte peut grandir, les champs inférieurs
  suivent, les rangées s’écartent et la liste défile si nécessaire. Les cartes
  anciennes réglées sur Fixe ne sont pas converties silencieusement.
- Fenêtre de jeu redimensionnable ; reconstruction des menus sans avancement
  narratif pendant le changement de taille.

## Preuves reproductibles

| Vérification | Résultat |
|---|---|
| Tests du workspace RVN | 317 réussis |
| Tests de l’éditeur natif | 24 réussis |
| Parcours moteur | 26 réussis, dont les trois destinations d’imagemap |
| Parcours éditeur | Création, trois modèles, export, deux imports, conflits, annuler/rétablir, styles, couleur et réouverture réussis |
| Comparaisons natives | Trois designs à 1280×720, 1920×1080 et 2560×1080 réels, puis réduction et agrandissement pendant la pause |

Rapports de cette livraison : `/tmp/rvn-menu-editor.YSSM3p`,
`/tmp/rvn-menu-native.1FblUD` et `/tmp/rvn-menu-preview-designs.e8qly5`.
Les dossiers temporaires contiennent les captures et les journaux ; les scripts
ci-dessous permettent de les recréer lorsqu’ils auront été nettoyés par le système.
Les exécutables de l’éditeur et du workspace RVN ont été reconstruits en release.
Le moteur issu de la reconstruction finale a de nouveau passé le parcours natif
à 1920×1080 puis 960×600 et 1280×720 : `/tmp/rvn-menu-release-proof.wsV3c4`.

Les parcours moteur couvrent notamment les confirmations de partie et de
sauvegarde, protection/suppression/chargement, miniatures, contrôles locaux,
réglages, langues, défilement, choix désactivés, textes longs, effets et navigation.
Ils utilisent des projets, sauvegardes et données persistantes temporaires.

Exécuter les trois scripts successivement depuis la racine `rust-VN` :

```
bash tools/verify_menu_editor.sh
bash tools/verify_menu_native.sh
bash tools/verify_menu_designs.sh
```

Chaque script imprime son dossier de rapports et ferme ses propres fenêtres.
Le dernier capture les trois projets livrés, vérifie la présence du résumé long
sans coupure, conserve le document source et contrôle la position narrative.
Il demande 1280×720, 1920×1080 et 2560×1080, puis réduit chaque fenêtre à 960×600
avant de la rétablir à 1280×720. Les rapports indiquent les dimensions réellement
obtenues, pas seulement les dimensions demandées.

## Périmètre de validation

Les tests de clavier/manette injectent des événements : aucune manette physique
n’était disponible pour une qualification matérielle. La mise à l’échelle testée
est 1 ; un changement physique de DPI, d’autres systèmes d’exploitation et tous
les périphériques possibles ne sont pas certifiés par cette livraison.

Les exemples prouvent des structures et styles distincts. Ils ne constituent pas
des reproductions pixel-perfect des deux captures fournies. Un rendu papier avec
cadres irréguliers et écriture manuscrite nécessite les images et polices adéquates,
que l’utilisateur peut choisir dans les assets.

Le scénario original de « La Dernière Archiviste » et ses sauvegardes n’ont pas
été utilisés comme espace de test ni modifiés pendant cette continuation.
