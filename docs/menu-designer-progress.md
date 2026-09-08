# Éditeur de menus et imagemaps — état au 8 septembre 2026

Cette livraison fournit un parcours visuel utilisable pour créer, personnaliser,
prévisualiser, enregistrer et partager les interfaces de jeu. Voir le
[relevé de livraison](menu-designer-validation-2026-09-08.md) pour les preuves
actuelles et les limites de qualification ; ce journal conserve aussi les étapes
antérieures du chantier.

## État actuel — suivi de la continuation

Le [guide de l’éditeur](menu-designer-guide.md) décrit les commandes actuelles.
Les sections plus anciennes ci-dessous sont un journal de chantier ; leurs
listes de fonctionnalités manquantes ne sont pas toutes encore d’actualité.

Ajouts présents : canvas d’interactions et événements de page, sons depuis les
assets, choix de cibles compatibles, navigation clavier/manette des réglages,
styles de bordure/focus, sélection multiple, hiérarchie, composants de cartes,
actions Sauvegarder/Charger séparées, historique multiligne et galerie verrouillée,
dialogue/choix/commandes rapides personnalisés, trois mises en page distinctes,
ancres visuelles, champs de marges/espacement/ordre de focus, images et polices
choisies dans les assets, import d’images par dépôt, aperçu natif isolé.

L’aperçu propose des emplacements vides ou des données remplies. L’ouverture et
la fermeture ont été vérifiées depuis les boutons ; les sauvegardes générées
restent dans son dossier temporaire. Le composant en cours d’édition n’ajoute
pas de page de jeu à l’export. Les images locales peuvent remplacer ou masquer
une image héritée. Les références inconnues ou incompatibles sont refusées.

Vérifications récentes : 67 tests du cœur, 20 tests du moteur et 82 tests du
modèle de menus ; 24 tests de l’éditeur. Parcours natifs des commandes rapides avec position narrative
inchangée, confirmations d’écrasement (annuler/confirmer), historique long,
choix de la troisième réponse et trois destinations d’imagemap avec menus
personnalisés. Les tests sont exécutés sur des projets temporaires.

Ajouts de la continuation : transparence héritée et réglable en pourcentage,
alignement/retour à la ligne, déplacement de la vue au bouton central et cadrage
de sélection, conteneurs défilants imbriqués avec découpage et routage de la
molette, barres de défilement saisissables, liste de langues native, indicateurs
de cases à cocher, modèles de réponses et d’historique avec hauteur automatique.
La police de secours de la composition est maintenant celle du moteur.
Un modèle de réponse a été créé et enregistré depuis l’éditeur, puis joué avec
une réponse sur trois lignes. La sélection de langue au clavier et l’annulation
par clic extérieur, ainsi que le glissement de la barre jusqu’au bas, ont été
testés sans modification de la position narrative.

Les effets de fondu, déplacement, échelle et couleur disposent de durée,
courbe et valeurs de départ/arrivée dans les nœuds d’interaction. Le test
natif des quatre effets simultanés atteint les valeurs finales, sans dérive
de position ni avancement du scénario. Leur état survit à une reconstruction
du menu provoquée par le chargement d’une ressource. La durée a été modifiée
et enregistrée depuis l’inspecteur de l’éditeur.

Les cartes peuvent désormais proposer Sauvegarder, Charger, Supprimer et
Protéger séparément. Un parcours natif vérifie le verrouillage, les contrôles
désactivés, le chargement toujours autorisé, le déverrouillage et l’annulation
puis la confirmation de suppression. La reprise pointe uniquement vers une
sauvegarde existante. La protection est persistée séparément de l’histoire.
Le chargement rapide passe aussi par une confirmation ; annuler conserve le
dialogue. Les miniatures de sauvegarde manuelle utilisent maintenant une
capture du dernier écran de jeu correspondant, jamais celle du menu ouvert.
Le fichier de sauvegarde conserve ses données narratives et son horodatage.

Les confirmations de nouvelle partie et de reprise utilisent désormais la
même protection, y compris sans page de confirmation personnalisée. Un test
natif vérifie annulation, reprise et redémarrage, sans modifier la sauvegarde.
Un autre vérifie trente langues, le défilement automatique vers l’option courante,
le bouclage clavier et la validation. Les graphes sur panneaux et images sont
testés au pointeur ; les zones de survol ne reçoivent pas de focus clavier.
Le collage dans les composants garde une racine unique. Dupliquer, annuler,
rétablir et enregistrer ont été vérifiés depuis le clavier dans l’éditeur ;
un champ de l’inspecteur ne déclenche pas ces raccourcis de composition.

Les graphes de clic des boutons de confirmation remplacent maintenant leur
action simple. Le parcours natif vérifie une variable locale modifiée par ce
graphe avant la reprise ; les opérations qui contourneraient la confirmation
sont refusées atomiquement. Les trois modèles ont été installés, enregistrés
et exportés depuis l’interface dans un projet temporaire. Un second projet
a importé deux fois le design Science-fiction, conservé ses pages initiales
et été rouvert. L’examen du paquet détaille les copies et conflits de rôles.
Un parcours supplémentaire a exporté depuis l’interface une image, une police
et sa licence, puis importé le paquet dans le second projet. L’image importée
s’affiche dans le canvas ; les trois fichiers sont identiques aux originaux
et leurs références utilisent le nouveau dossier indépendant.

Les zones défilantes ont aussi été exercées pendant un dialogue : la saisie
de leur barre ne fait pas avancer le scénario et la molette montante n’ouvre
pas l’historique. Le script `tools/verify_menu_native.sh` regroupe les parcours
isolés de pointeur, langues, confirmations simples/avec graphe et défilement.
Il conserve captures et journaux dans un nouveau dossier temporaire.

Les ombres de cadre sont paramétrables (couleur, décalage et diffusion) depuis
l’inspecteur. Le moteur et le canvas partagent leurs couches de rendu ; elles
suivent les animations et les dimensions mesurées des cartes. Une ombre a été
configurée et enregistrée depuis l’interface. Le parcours natif des effets
inclut maintenant une ombre animée.

Les dialogues trop longs disposent d’une zone défilante : suivi de l’écriture,
remontée par molette/barre ou Page précédente, puis remise à zéro au dialogue
suivant. Le test natif lit vingt paragraphes et revient à un dialogue court,
sans avancement parasite. Ces deux parcours sont intégrés au script de tests.

Les confirmations ont maintenant leurs événements d’ouverture/fermeture,
survol/sortie et focus, y compris sur un panneau interactif. Le parcours natif
vérifie que son clic ne valide pas l’opération. Le focus suit les identifiants
et l’ordre des éléments, plutôt que deux positions fixes.

Les événements de manette passent par la même consommation que les touches,
pour éviter une double activation à la fermeture d’une liste. Les parcours
natifs vérifient trente langues et l’initialisation du focus des commandes
rapides avec des événements de manette injectés ; cela ne constitue pas un
essai avec un périphérique physique.

L’inspecteur distingue héritage et exception locale, permet de rétablir une
seule couleur, et montre les états focus/sélection. Les barres ont désormais
leurs couleurs, largeur et arrondi personnalisables et partageables. Leur
réglage a été enregistré depuis l’interface et leur rendu vérifié dans le jeu.

Le parcours manette des douze réponses cible la dernière, la rend visible et
vérifie sa destination originale. Les cadres neuf zones ont été comparés aux
calculs du moteur et rendus en formats large et vertical ; leurs découpes source
ne varient plus avec la résolution. Les éléments de confirmation masqués ou
désactivés par un graphe quittent la navigation, avec réparation du focus.

Les erreurs d’exécution de l’aperçu disposent d’un diagnostic indépendant du
scénario. Le bouton « Erreur de l’aperçu » ouvre le graphe et centre le nœud
fautif ; une boucle de deux nœuds a été testée depuis l’éditeur, avant et après
fermeture du moteur. La limite accepte exactement 256 étapes terminées et
interrompt une boucle sans valider ses variables locales.

La fermeture de l’éditeur protège désormais aussi les menus non enregistrés,
y compris par son bouton de barre de titre. Le parcours natif crée une page,
tente de fermer, puis abandonne explicitement la modification ; le fichier
initial reste identique octet pour octet. Enregistrer refuse également les
champs en cours non appliqués, au lieu de les oublier silencieusement.

L’import de `theme.toml` reprend maintenant fond, titre, couleurs et ordre des
boutons, visibilité, libellés, dimensions, ancrages, polices, boîte de dialogue
et choix. Un rapport liste les propriétés non converties et les ajustements
de lisibilité. Le thème fourni avec le moteur a été importé et enregistré
depuis l’éditeur, puis son titre et son dialogue ont été rendus dans le moteur.
Les ancrages étirés acceptent leurs vrais décalages, y compris négatifs ; les
dimensions sont validées contre le parent et vérifiées à trois résolutions.

Les décorations de cartes disposent de conditions d’affichage : emplacement
vide/rempli/protégé, illustration verrouillée/déverrouillée. Les composants
imbriqués reçoivent les mêmes données et une exception « Toujours » peut
remplacer une règle héritée. Le parcours natif vérifie les six cartes et les
cibles actives ; le réglage a été enregistré et rouvert dans l’éditeur.
La suite native regroupe maintenant 24 scénarios, dont la pagination
personnalisée et le diagnostic des boucles entre pages.
Les 26 parcours ont été rejoués avec succès ; les captures et journaux de
cette exécution sont dans `/tmp/rvn-menu-native.89ZZrB`. Six tests d’intégration
supplémentaires couvrent les composants mesurés et les contrôles locaux.

L’inspecteur possède cinq rubriques avec validation/annulation de saisie
toujours accessibles. Annuler et rétablir conservent la page et la sélection
par identifiant : le contrôle natif d’une police 20 → 22 → 20 → 22 reste sur
le composant et son champ, sans retour au menu principal.

Les listes déroulantes sont limitées à la fenêtre et défilent lorsqu’elles
dépassent sa hauteur. La dernière des 41 pages a été sélectionnée dans le
contrôle natif. Les 24 actions restent intégralement visibles ; l’action de
pagination numérotée a été configurée et enregistrée depuis l’inspecteur.

Les composants peuvent être créés depuis une sélection. Les nouvelles instances
héritent séparément du texte, de l’action et de la donnée liée ; les anciennes
valeurs locales restent inchangées. Un test dans l’éditeur a vérifié propagation,
exception locale et rétablissement de l’héritage. L’insertion récursive est
refusée avant publication. Les raccourcis sur les textes sont maintenant
activables au même titre que les images et boutons ; un parcours natif ouvre
les réglages sans avancer l’histoire.

La fermeture depuis les menus vérifie également les graphes narratifs restés
ouverts. Le test natif a déplacé un nœud, ouvert les menus puis tenté de fermer :
l’application est revenue au graphe avec son avertissement et un fichier de
récupération, sans supprimer la modification. Les saisies non validées empêchent
aussi le changement d’espace. La récupération concerne tous les onglets modifiés.

Les sauvegardes rapides et automatiques possèdent désormais leurs propres
miniatures de jeu. Un parcours natif compare deux scènes successives et vérifie
que la sauvegarde rapide conserve sa première image. Une capture arrivée en
retard ne peut pas écraser la miniature d’une sauvegarde plus récente.

Les couleurs du texte normal, survolé, pressé, sélectionné, désactivé et focalisé
sont éditables et héritables. Le moteur a été testé dans ces états, sans changer
le document source ni la position narrative. Les listes de langues reprennent
la même palette. Le sélecteur chromatique reste dans la fenêtre ; son placement
près de l’inspecteur a été vérifié à 1280 × 650. Le champ de texte accepte les
retours à la ligne (Ctrl+Entrée valide la saisie).

La grille partagée entre composition et moteur ne réserve plus de cellule aux
enfants masqués. Les styles, ancres et déplacements de calques d’un composant
sont validés comme un espace de création, sans exiger les contrôles d’une page
de jeu. L’annulation mémorise aussi l’association des espaces aux composants.
Le parcours visuel a remplacé un modèle, annulé, enregistré, rétabli, puis
annulé de nouveau : les composants reviennent sans exporter leurs espaces
techniques comme pages de jeu. L’ajout d’un enfant à un composant non conteneur
est refusé avant mutation, avec une explication.

Le bouton « Ajuster la hauteur au texte » conserve largeur et contenu ; un
libellé de quatre lignes a été ajusté, enregistré, annulé et rétabli dans un
composant depuis l’éditeur. Les cartes héritent aussi de la transparence et de
l’état désactivé de leur liste, dans la composition comme dans le moteur.
Un parcours supplémentaire vérifie qu’une liste de choix désactivée ignore
clic et touche numérique, qu’une liste masquée ignore Entrée, puis que sa
réactivation conserve la destination de la troisième réponse.

Les numéros d’emplacement sont des données de texte éditables dans les cartes.
Le parcours de pagination vérifie aussi les numéros réellement affichés sur
les pages 1, 2 et 3. Les sélecteurs de données et de réglages utilisent la même
compatibilité de types que la validation. Une image ne propose pas de données
textuelles, une case à cocher ne propose pas de volume, et un curseur permet
la vitesse automatique. Le choix du numéro de carte et le remplacement par
une liaison de réglage ont été enregistrés dans le contrôle natif, puis annulés.

Les calques peuvent maintenant être déposés avant/après une ligne ou dans
un conteneur avec un indicateur bleu. La capture du pointeur empêche le panneau
de défiler pendant ce geste. Réordonner, imbriquer sans déplacer visuellement,
enregistrer, annuler et rétablir ont été testés dans la fenêtre native.
Les groupes sont repliables ; rechercher un enfant montre aussi ses parents,
puis effacer la recherche restaure les replis. Ces états ne modifient pas le
document enregistré.

Les dimensions de liste sont maintenant trois champs dédiés, avec capacité
et nombre de pages calculés. Le remplissage par lignes ou par colonnes garde
les mêmes emplacements et actions. Ces deux dispositions passent le parcours
natif de pagination ; changer de disposition, enregistrer puis annuler a aussi
été exercé depuis l’éditeur.

Les images peuvent différer pour les états normal, survolé, pressé, désactivé,
focus et sélection. Une exception peut masquer l’image pour un état seulement.
Styles, composants, export et remappage des ressources conservent ces variantes.
Le contrôle visuel a modifié puis rétabli l’image de survol sans toucher à
l’image normale. Le parcours moteur vérifie les textures et leur visibilité,
le focus clavier et l’absence d’avancement du scénario.

La hauteur automatique des textes et boutons utilise maintenant les mesures
réelles de la police. Les conteneurs suivent leur hauteur ; les cartes mesurent
leurs données après résolution des liaisons. Le parcours natif vérifie un texte
long puis court, sans changement du document source ni du scénario. La
conversion de taille entre les unités de police de Makepad et Bevy a été
corrigée ; les retours à la ligne ont été comparés visuellement.

Des cases à cocher, curseurs et sélecteurs indépendants des réglages du jeu sont
maintenant configurables depuis l’inspecteur. Leurs variables apparaissent dans
les cibles des interactions ; valeurs booléennes et options proposent un
sélecteur dédié. Les valeurs vivent uniquement dans la session d’interface.
Le parcours natif vérifie clic, glissement, sélection au clavier, événements
sans double déclenchement, document source intact et réglages/scénario inchangés.
Une case locale a été créée depuis la palette ; les trois options d’un
sélecteur ont été modifiées, enregistrées puis annulées dans l’éditeur.

La qualification matérielle reste distincte de ces parcours fonctionnels :
manette physique, changements de DPI et autres systèmes d’exploitation n’ont
pas été certifiés. Les exemples livrés ne sont pas des copies pixel-perfect
des deux captures de référence.

### Dernière validation livrée

- Trois projets autonomes dans `examples/menu-designs/`, créés avec le lanceur
  puis enregistrés et exportés avec l’éditeur : Sobre, Illustré et Science-fiction.
  Leurs paquets reproduisent exactement les documents sauvegardés et leurs médias.
- `tools/verify_menu_editor.sh` pilote les vrais contrôles natifs : création,
  trois modèles, export, examen des conflits, deux imports, annuler/rétablir,
  style nommé, renommage et réouverture sans perte. Exécution réussie :
  `/tmp/rvn-menu-editor.YSSM3p`.
- Le renommage de style met à jour les pages et composants, préserve les
  exceptions locales et refuse les conflits. Les boutons signalent une sélection
  manquante. L’historique retrouve la page concernée après un import.
- Les événements des contrôles hérités sont validés à partir du composant résolu,
  dans l’éditeur comme dans le moteur. Le sélecteur local et le réglage de langue
  hérités sont exercés par les parcours natifs.
- Le mode Remplir recadre les images de panneaux, normales ou interactives,
  sans couper leurs enfants ni leurs ombres. Le rendu natif a été inspecté.
- Le sélecteur de couleur utilise le rectangle final de sa fenêtre flottante :
  la saisie hexadécimale ne ferme plus le sélecteur et aucun clic ne traverse
  vers l’inspecteur. Saisie, annulation, rétablissement et réouverture sont testés.
- Une couleur peut être partagée seule, sans remplacer les polices ou les
  autres propriétés du style. Le projet Illustré a été personnalisé en variante
  claire avec ces commandes, enregistré et réexporté depuis l’éditeur.
- Éditeur et moteur reconstruits en release ; 24 tests de l’éditeur réussis.
- 317 tests du workspace RVN réussis. Le contrôle des trois projets vérifie
  aussi les dimensions positives et l’absence d’éléments hors écran à
  1280×720, 1920×1080 et 2560×1080, ainsi que l’import répété des paquets.
- `tools/verify_menu_designs.sh` a réalisé neuf captures par lancement sur les
  trois designs, soit neuf lancements natifs : `/tmp/rvn-menu-preview-designs.e8qly5`.
  Les sauvegardes de démonstration sont explicitement créées dans le dossier
  QA ; le garde-fou de l’aperçu utilisateur n’est pas assoupli. Aucun document
  de menu n’a changé et les passages dans les menus n’ont pas avancé le scénario.
  Dimensions réellement obtenues pour chaque design : 1280×720, 1920×1080 et
  2560×1080, facteur d’échelle 1. Chaque lancement vérifie aussi une réduction
  effective à 960×600 puis un retour à 1280×720 pendant la pause. Le test attend
  l’ouverture de la fenêtre avant de demander sa taille, car le bureau peut
  restaurer une dimension initiale plus petite.
- Les 26 parcours moteur passent après l’adaptation des cartes :
  `/tmp/rvn-menu-native.1FblUD`. Le parcours complet de l’éditeur passe après
  les mêmes changements : `/tmp/rvn-menu-editor.YSSM3p`.

Cette comparaison a révélé des résumés coupés dans les cartes à hauteur fixe.
Le calcul partagé sait maintenant agrandir les champs automatiques, décaler
ceux placés en dessous, ajuster la carte puis les rangées de sa grille. Le
moteur rend la liste défilable si nécessaire ; sa pagination suit les cartes.
Les anciens documents gardent leur choix Fixe/Automatique, tandis que les
nouvelles cartes proposent des textes automatiques. Les projets d’exemple
ont été réglés depuis l’inspecteur, sauvegardés puis réexportés.

Les captures de référence restent des objectifs de liberté graphique, pas des
ressources incorporées aux modèles. Les essais de manette utilisent des événements
injectés ; aucun périphérique physique ni changement réel du facteur d’échelle
de l’écran n’est certifié par ces tests.

## Journal des étapes précédentes

## Extension version 2 — en cours

Les deux références de sauvegardes (sombre/cyan et papier/manuscrit) sont des critères de personnalisation **structurelle**, pas seulement deux palettes de couleurs. Leur reproduction complète dans les outils reste à valider.

Ajouts en cours :

- Migration en mémoire des documents v1, copie exacte `.rvnui.v1.bak` avant leur premier enregistrement v2.
- Styles partagés et exceptions locales ; trois palettes initiales, qui ne sont pas encore les trois modèles complets promis.
- Arborescence sélectionnable et filtrable, déplacement entre conteneurs par glisser-déposer, refus des cycles ; conteneurs horizontaux, verticaux et grilles.
- Composants de cartes de sauvegarde, champs liés à la miniature, à la date et au résumé ; grille configurable et pagination dans le moteur. La miniature utilise actuellement la référence d’image enregistrée, pas une nouvelle capture complète de l’écran.
- Aperçu des cartes avec des données de démonstration indépendantes des vraies sauvegardes. Sélectionner une liste puis créer une carte la relie automatiquement en grille de six cases (3 × 2). « Modifier le composant » ouvre sa composition ; les changements se propagent aux instances. L’espace de travail du composant reste pour l’instant une page technique enregistrée dans le document.
- Images proportionnelles, recadrées, étirées ou cadres en neuf zones ; chemin de police personnalisable. Les textures de papier, cadres irréguliers et polices doivent être fournies comme ressources indépendantes : une capture de menu n’est pas décomposée automatiquement.
- Export/import de paquets `.rvnuitheme` avec ressources et licences voisines ; import dans un nouveau dossier et renommage des définitions, sans remplacement des originaux. Le choix des rôles des pages importées reste à exposer dans l’interface.

Vérification automatisée de cette extension : 21 tests `rvn_ui` passent en release, couvrant notamment migration/sauvegarde, héritage, cycles, proportions, neuf zones, liaisons des cartes et création de cartes dans un ancien document sans styles. Cela ne valide pas encore le parcours complet demandé ni une fidélité visuelle aux deux références.

Contrôle natif v2 : création d’une grille de six cartes depuis la liste sélectionnée, affichage de leurs données de démonstration, ouverture du composant avec cadrage agrandi et enregistrement réussi d’un document migré avec sa sauvegarde v1. Les exécutables release ont été reconstruits. Le parcours moteur Réglages → Retour → Galerie → Retour → Nouvelle partie passe ; un rafraîchissement excessif lié aux images, susceptible de perdre les activations, a été corrigé. Instances de test fermées. Les contrôles natifs des textures neuf zones, polices personnalisées et cartes avec de vraies sauvegardes restent à effectuer.

Limites importantes : propriétés encore saisies dans des champs avec validation explicite, états/bordures/effets pas tous rendus, pas de boutons Sauver/Charger/Supprimer indépendants dans chaque carte ni de protection d’emplacement, pas encore de prévisualisation moteur intégrée ou de canvas d’interactions. La sélection multiple, les réglages glissables, les modèles complets, l’interface narrative personnalisée et la validation souris/clavier/manette restent à terminer.

## Accès

### Outils de composition supplémentaires

- Maj + clic dans la composition ajoute ou retire un élément de la sélection. Déplacer un élément sélectionné déplace le groupe ; un parent et son enfant ne sont pas déplacés deux fois.
- La barre de composition propose l’alignement sur les six axes et une répartition horizontale/verticale à espacement égal. Les éléments doivent partager un conteneur libre ; les conteneurs automatiques gardent la priorité.
- Aimantation optionnelle sur 8 pixels pour déplacement/redimensionnement ; boutons de zoom, Ctrl + molette et cadrage de la page.
- Copier/coller interne à l’éditeur, duplication et suppression du groupe. Le collage crée des identifiants uniques et conserve les enfants, styles et actions. Les interactions de page ne sont pas clonées avec les éléments.
- Cliquer sur l’échantillon de couleur ouvre le sélecteur avec cercle chromatique, RGBA et hexadécimal. Choisir d’abord fond normal, survolé, pressé, désactivé ou texte. Le rendu suit immédiatement la couleur ; fermer le sélecteur crée une seule étape d’annulation. Les autres propriétés saisies doivent être appliquées avant ce geste.
- « Aperçu moteur » démarre une copie des menus en mémoire avec une scène de démonstration, dans un dossier temporaire dédié. Les actions de démarrage sur un label y lancent uniquement cette démonstration. Les ressources graphiques sont lues depuis le projet ; les sauvegardes et la progression sont créées dans le dossier temporaire. « Fermer l’aperçu » arrête ce processus. Relancer l’aperçu pour prendre en compte de nouvelles modifications. Les champs en cours de saisie doivent être appliqués, mais il n’est pas nécessaire d’enregistrer le document.

26 tests du modèle partagé passent en release après ces ajouts. Essai natif effectué pour Maj-clic, couleur et annulation, déplacement groupé, alignement puis enregistrement et zoom. Lancement et arrêt de l’aperçu vérifiés depuis les boutons de l’éditeur ; son manifeste dirige les sauvegardes vers son dossier temporaire.

Les contrôles Slider des menus personnalisés ont maintenant une piste glissable pour musique, sons, vitesse du texte et vitesse automatique. La persistance se fait au relâchement. Le test natif injecte un déplacement de curseur et les événements souris, vérifie une valeur finale de musique à 75 % dans `persistent.json`, puis poursuit Réglages → Retour → Galerie → Retour → Nouvelle partie. Le réglage clavier/manette des curseurs reste à terminer ; les boutons moins/plus de l’interface de secours restent inchangés.

Ce lot ne termine pas les interactions Blueprint, les états de focus/manette, les effets, les cartes à actions indépendantes ou les modèles complets.

Reconstruire `makepad-blueprint-demo` en release, ouvrir un projet puis cliquer sur **Menus du jeu**. La composition est indépendante des graphes narratifs. Le bouton **Retour aux graphes** préserve la session de graphes ouverte.

La colonne gauche présente les pages, les éléments/calques et la palette ; le centre affiche la composition ; la colonne droite contient les propriétés. Les listes déroulantes permettent de retrouver les éléments masqués/verrouillés. Déplacer avec la souris, redimensionner avec la poignée orange. Appliquer les propriétés avant d’enregistrer. Les propriétés non appliquées bloquent les changements de sélection et l’enregistrement, pour éviter leur perte.

## Implémenté

- Pages initiales : titre, pause, sauvegarde, chargement, réglages, galerie, historique, confirmation ; ajout de pages personnalisées.
- Éléments texte, images, panneaux, boutons, contrôles de réglages et listes dynamiques.
- Déplacement, redimensionnement, duplication, ordre des calques, masquage/verrouillage, annulation/rétablissement.
- Ancres et prévisualisation à 1920×1080, 1280×720, 2560×1080.
- Actions prédéfinies en français, navigation entre pages et démarrage sur un label ; confirmation avant de remplacer une partie depuis un menu en cours de jeu.
- Chargement des compositions dans le moteur ; branchement aux opérations de sauvegarde, réglages et galerie existantes. Défilement des listes longues à la molette. Navigation élémentaire Tab/Entrée.
- Police de secours embarquée avec accents, avec sa licence.
- `rvn_ui` partagé : document versionné, calcul des ancres, validation et interpréteur borné d’interactions locales. Les états `state.*` ne sont pas modifiables.
- Enregistrement atomique avec refus d’écraser des changements externes détectés ; validation des ressources et labels de menus lors de l’export projet et au chargement dans le moteur.

Le document est `menus.rvnui` à la racine par défaut. `[paths].menus` dans `rvn.toml` peut désigner un autre chemin relatif. Aucun changement du format `.rvngraph` ou des sauvegardes.

## Imagemaps

Les deux voies de rendu conservent désormais le rectangle de survol indépendant. Clic et survol utilisent la conversion de coordonnées de la caméra. Les bordures sont semi-ouvertes, les clics hors image ignorés, et les dimensions réinitialisées lorsqu’une carte est remplacée. Le survol utilise l’alpha original de l’image.

Dans l’éditeur de zones, le mode survol montre sa propre image et ses propres dimensions. Les zones invalides sont refusées avant modification du graphe. Le chargement d’une image référencée par un asset ne repose plus sur une indexation de cache pouvant paniquer.

## Vérifications effectuées

- Tests release de `rvn_ui`, `rvn_graph` et `rvn_bevy` ; tests du cœur également exécutés pendant cette étape.
- Éditeur release autonome : ouverture des menus, sélection, déplacement, synchronisation des coordonnées dans l’inspecteur, enregistrement et sélection d’une page. Instances de test fermées après les captures.
- Moteur release sur une copie temporaire : actions Réglages → Retour → Galerie → Retour → Nouvelle partie par les composants de boutons réels. Ce test ne simule pas le pointeur système pour ces boutons.
- Imagemap : trois parcours distincts utilisant position du curseur, caméra et événement de clic ; captures de survol et des dialogues de destination antenne/archives/journal.
- Les fichiers originaux de « La Dernière Archiviste » n’ont pas été modifiés.

## À terminer avant de déclarer le plan complet

- Véritable éditeur visuel des graphes d’interactions de menus : le modèle/interpréteur existent, mais aucun canvas de branchement dédié n’est livré ici ; événements ouverture/fermeture encore à relier au moteur.
- Hiérarchie imbriquée, conteneurs de mise en page, sélection multiple, glisser-déposer des assets, gestion ergonomique des couleurs/polices et états de survol dans l’inspecteur.
- Réglage clavier/manette des curseurs, focus stable pendant l’actualisation et styles avancés des contrôles.
- Prévisualisation interactive isolée intégrée, import complet et fidèle du thème (l’import actuel ne reprend que titre et fond).
- Finition des listes : cartes de sauvegarde, miniatures et contenus verrouillés, historique multiligne ; focus clavier visible et suivi du défilement complets.
- Validation exhaustive des transitions entre pages, des confirmations et du redémarrage, sauvegarde/chargement sur une longue histoire, erreurs de ressources et absence de modifications de l’état narratif en pause.
- Comparaison visuelle détaillée à plusieurs tailles et résolutions. Aucun engagement de parité pixel-perfect à ce stade.
