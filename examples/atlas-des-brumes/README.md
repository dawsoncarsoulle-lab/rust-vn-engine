# L’Atlas des Brumes

Une aventure jouable originale et un véritable projet RVN/Blueprint, inspirés
de la navigation en volets des menus d’aventure Nintendo 64. Les illustrations,
les objets et le monde sont originaux : aucun asset Nintendo n’est distribué.

## Jouer

Ouvrez ce dossier comme projet dans l’éditeur rust-VN puis cliquez **Jouer**,
ou utilisez un moteur installé avec `rvn run /chemin/vers/atlas-des-brumes`.
Dans le dépôt source, **Jouer.sh** et **Editer.sh** recherchent les applications
dans le PATH puis dans leur emplacement d’installation standard par utilisateur.
Le jeu commence dans l’Atlas ; fermez-le pour explorer le Havre, ou choisissez
une destination sur la carte. La vidéo d’introduction nécessite un runtime avec
la prise en charge vidéo activée.

- **Identité** : prénom, nom, origine et confort de lecture.
- **Inventaire** : 18 emplacements, équipement, soins et objets de quête.
- **Carte** : 6 lieux, descriptions au survol, sélection au clavier et vrais voyages.
- **Q/E**, PageUp/PageDown ou les boutons latéraux changent de volet.
- **Flèches** sélectionnent ; **Entrée** active ; la souris fonctionne également.
- **Échap** ferme l’Atlas. Hors Atlas, Échap ouvre le menu système.
- **F5/F6**, hors Atlas : sauvegarde/chargement rapide. **F8** : préférences du moteur.

Pour commencer : fermez l’Atlas, choisissez **Explorer Le Havre des Brumes**,
puis rencontrez Maëlys. L’objectif courant est affiché sous les pages. Certaines
routes demandent un objet possédé ; les Falaises demandent aussi que le grappin
soit **équipé**, pas seulement dans le sac. La potion se consomme réellement.

Le cadran de l’Observatoire se commande par 1/2/3, flèches, molette ou clics.
Ses indices sont affichés dans le jeu. Deux conclusions sont accessibles,
selon votre relation avec Maëlys et la mélodie retrouvée. Après la conclusion,
la carte et les objets restent disponibles pour continuer les essais.

## Modifier dans l’éditeur

**main.rvn est l’unique source canonique** : interface, dessins, événements,
mini-jeu, personnages, animations et histoire sont dans ce fichier et liés
aux Blueprints du projet. Le nœud de volume audio utilise désormais son vrai
import, sans perdre le script lors d’une édition visuelle.

Le fichier `.rvn-authoring.json` conserve les graphes et leur mise en page,
avec un instantané de cette même source ; il ne remplace jamais `main.rvn`.
Dans le dépôt source uniquement, `authoring/*.rvn.part` conserve les sections de
construction d’origine. Ne les assemblez pas par-dessus votre main.rvn modifié.
Les médias sont dans `assets/`, la présentation narrative dans `theme.toml`.
Le [guide du menu](README-UI.md) décrit les composants et le contrat des objets.

## Capacités mises en jeu

| Capacité | Utilisation dans l’aventure |
| --- | --- |
| Dessin personnalisé RVN | Atlas animé, matériaux, objets et carte, formes/texte/images/groupes/découpage |
| Interactions personnalisées | Survol, zones actives, clavier, molette, événements et mises à jour |
| Contrôles natifs | Champs Unicode, choix d’origine, toggles et taille de texte |
| État et collections | Inventaire/quantités, dictionnaires, calculs, fonctions, boucles, hasard sauvegardé |
| Récit et progression | Choix conditionnels, identité interpolée, six lieux, deux fins, objets utiles |
| Animation | Splines, Bézier, courbe RVN, parallèles, séquences, images, attente/arrêt |
| Personnage multicouche | Tenue, expression et accessoire indépendants ; variante, règle et sélecteur |
| Médias | Décors, musique et effet originaux, vidéo avec sous-titres |
| Sauvegarde/rollback | État du récit, inventaire, composants, animation et hasard restaurables |
| Menus du moteur | Sauvegarder, charger, réglages, historique, galerie et fins débloquées |
| Accessibilité | Navigation au clavier, texte agrandi, contraste, mouvements réduits, voix optionnelle |

L’Atlas est un **écran programmable dans le jeu**, et non un remplacement caché
des menus système. Les scènes de dialogue restent du visual novel ; les voyages
se font par les marqueurs, pas par un monde 3D libre. Les volets utilisent le
dessin 2D du moteur, avec animation et aperçus latéraux, pas un rendu 3D Nintendo.

Le jeu est principalement en français. Les dialogues et contrôles natifs ont
des traductions anglaises ; les dessins Canvas gardent leurs libellés français.
La lecture vocale dépend des voix disponibles sur le système.

## Export et vérification

Ce dossier fournit le projet éditable, pas des binaires précompilés ni des
rapports de sessions locales. Les exports Linux et Web peuvent être créés par
le moteur ou l’éditeur ; un export Web doit être servi en HTTP.

Le dépôt source du moteur conserve des contrôles indépendants dans `qa/`.
Depuis sa racine, `cargo test --release --manifest-path
examples/atlas-des-brumes/qa/Cargo.toml` teste la logique avec un renderer sans
fenêtre. Ces contrôles ne vérifient pas à eux seuls les pixels, le focus natif,
le son ou le décodage vidéo. Ils ne constituent pas une promesse de parité
totale avec Ren’Py ni une certification Windows.

Les illustrations ont été générées spécialement pour cette aventure, puis
inspectées et intégrées comme fichiers de jeu. Provenance et droits :
[ASSET-CREDITS.md](ASSET-CREDITS.md).
