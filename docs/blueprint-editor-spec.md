# Éditeur Blueprint rust-VN

## Objectif

L'éditeur Blueprint est un frontend visuel natif, écrit entièrement en Rust avec
Makepad. Il ne remplace ni le parseur, ni le LSP, ni l'exécution Bevy. Son rôle
est de transformer un graphe visuel en script `.rvn` lisible, puis de faire
valider ce texte par les outils existants de rust-VN.

Principes non négociables :

- rendu fidèle aux Blueprints d'Unreal Engine 5 ;
- script `.rvn` généré déterministe et lisible par un humain ;
- aucune logique d'exécution du moteur dupliquée dans l'éditeur ;
- validation par reparsing avec `rvn_parser`, puis par les contrôles de
  `rvn check` ;
- identifiants stables pour les nœuds, pins et connexions ;
- toute variable réutilisable est d'abord déclarée dans le panneau Variables,
  puis manipulée uniquement par de vrais nœuds GET/SET ;
- toute valeur ponctuelle reste une constante typée visible et branchée ;
- compatibilité aller-retour entre le graphe enregistré et le texte généré.

## Langage visuel

Chaque construction du langage doit être représentable, mais pas par un widget
Rust spécifique. Un renderer générique affiche des définitions de nœuds issues
d'un catalogue déclaratif.

```text
NodeDefinition
├── identifiant stable
├── version de schéma
├── catégorie
├── titre, description et mots-clés
├── couleur d'en-tête
├── pins typés et valeurs par défaut
├── règles de connexion
├── propriétés éditables
└── émission vers l'AST et le texte rust-VN
```

Le catalogue doit couvrir exhaustivement les variantes de `Statement` :

- projet : `Use`, `Init`, `Config`, `CharacterCreate` ;
- narration : `Dialogue`, `Choice` ;
- état : `SetVar` ;
- flux : `If`, `Label`, `Jump`, `Call`, `Return`, `Timer`, `TimerCancel` ;
- scène : `Scene`, `CinematicShow`, `CinematicHide`, `UnlockEnding` ;
- personnages : `ShowSprite`, `HideSprite`, `MoveSprite`, `SpriteAnimate`,
  `SpriteStopAnimation`, `SpriteEffect`, `MethodCall` ;
- audio : `MusicPlay`, `MusicStop`, `MusicVolume`, `SfxPlay`, `SfxStop`,
  `VoicePlay`, `VoiceStop` ;
- interaction : `Imagemap`, `TypewriterSet`, `TypewriterSpeed`.

Les expressions utilisent des nœuds purs et réutilisables :

- littéraux `Int`, `Float`, `Bool`, `Str` et listes ;
- lecture de variable normale ou `persistent.*` ;
- opérateurs arithmétiques `+`, `-`, `*`, `/` ;
- comparaisons `==`, `!=`, `<`, `<=`, `>`, `>=` ;
- logique `and`, `or`, `not` et négation ;
- appels de fonctions, accès à un index et interpolation de texte.

`Position`, `Transition` et les chemins d'assets sont prioritairement des
valeurs de pin avec éditeur intégré. Ils peuvent également être exposés sous
forme de nœuds de valeur lorsqu'une composition visuelle est utile.

## Catégories et couleurs

La catégorie organise le menu de création. Sur le graphe, la couleur reprend
la sémantique visuelle UE5 : événement rouge, appel de fonction/action bleu,
nœud pur vert, GET/SET teinté par le type de variable. Elle ne doit jamais
modifier la couleur de type des pins.

| Catégorie | Accent principal | Usage |
| --- | --- | --- |
| Structure | `#59636f` | fichiers, init, config, labels |
| Événements | `#9e1b1b` | points d'entrée d'un graphe |
| Narration | `#713f82` | dialogue, narrateur, choix |
| Flux | `#a9681d` | branchement, jump, call, return, timers |
| Variables | `#276b61` | get, set, variables persistantes |
| Expressions | `#315f38` | calculs, logique, comparaisons, fonctions |
| Personnages | `#356d42` | sprites, émotions et animations |
| Scène/Cinématique | `#27657a` | backgrounds, CG et transitions visuelles |
| Audio | `#594b83` | musique, SFX, voix et volume |
| Interaction/UI | `#8a5727` | imagemap, hotspots et typewriter |
| Progression | `#8b7023` | fins et déblocages persistants |

Palette des pins :

| Type | Couleur |
| --- | --- |
| exécution | blanc `#eef0f0` |
| booléen | rouge `#c2070a` |
| chaîne ou chemin de ressource | cyan `#0f94c2` |
| entier | turquoise `#1fc79c` |
| flottant ou position | jaune `#ffad1f` |
| enum ou transition | bleu `#5273e0` |
| liste | violet `#a458d0` |
| texte interpolé | magenta `#f505b8` |

## Grammaire visuelle validée

- grille grise infinie calculée dans un fragment shader ;
- corps sombres légèrement translucides et vitreux ;
- ombres gaussiennes, contour gris fin et rayon UE de 7 px pour les fonctions ;
- en-têtes de fonction bleus de 29 px avec icône `ƒ`, sans sous-titre de catégorie ;
- GET et constantes sous forme de capsules de 38 px, rayon 14 px, teintées par type ;
- SET avec flux d'exécution, nom intégré et valeur typée, sans fausse broche de nom ;
- sélection par contour et halo orange ;
- pins d'exécution pentagonaux et pins de données circulaires en SDF ;
- pins encastrés dans le nœud, à neuf unités du bord ;
- câble passant sous la surface vitreuse jusqu'au centre du pin ;
- câbles Bézier avec halo sombre et cœur coloré ;
- ordre de rendu : grille, câbles, corps, contrôles, pins, texte.

Les opérateurs utilisent un gabarit UE dédié, distinct des autres nœuds
compacts : corps arrondi de 64 px, largeur de 122 px pour un symbole simple et
142 px pour un comparateur à deux caractères, pins à 21 px du bord, entrées à
17/47 px et sortie centrée. `AND`/`OR` utilisent le format 204 × 72 px avec
cases booléennes visibles et bouton fonctionnel « Ajouter une broche » ; chaque
opérande supplémentaire est replié dans l'expression rust-VN générée.

Interactions de connexion :

- une zone de hit élargie entoure chaque pin sans modifier sa géométrie SDF ;
- le câble provisoire suit le pointeur et s'aimante sur une pin compatible ;
- le tirage fonctionne depuis une sortie ou une entrée ;
- tirer une entrée déjà câblée détache sa source afin de pouvoir la reconnecter ;
- une entrée conserve au maximum une source et une liaison identique n'est jamais dupliquée ;
- les auto-connexions d'un nœud vers lui-même sont refusées.

## Modèle de graphe

Le document persistant est séparé de sa représentation Makepad :

```text
GraphDocument
├── graph_id
├── kind: Init | Label | Function | Imagemap
├── nodes: BTreeMap<NodeId, GraphNode>
├── pins: BTreeMap<PinId, GraphPin>
├── edges: BTreeMap<EdgeId, GraphEdge>
├── compteurs monotones d'identifiants
└── schema_version
```

Le viewport, les caches de hit-test et l'historique d'annulation appartiennent à
la session d'éditeur et ne sont pas sérialisés dans le document. Les
commentaires sont actuellement des propriétés de nœud ; les groupes visuels
seront une extension de schéma dédiée.

Les sous-graphes d'expressions sont des DAG. Le graphe de contrôle global peut
contenir des retours logiques : `jump` et `call` sont donc stockés comme des
références symboliques à des labels, et non comme de longs câbles entre fichiers.

Les connexions doivent vérifier : direction, cardinalité, compatibilité de type,
cycle interdit dans une expression et portée de la variable ou du label.

Une connexion de données efface toute ancienne valeur implicite de sa broche.
Les valeurs significatives héritées sont migrées vers des constantes visibles ;
une entrée obligatoire non branchée produit un diagnostic et ne peut jamais
générer silencieusement une chaîne vide.

Le canvas est une projection de `GraphDocument`, jamais une seconde source de
vérité. Un nœud créé reçoit immédiatement ses `NodeId`/`PinId`; un déplacement
met à jour sa position persistante et chaque câble conserve son `EdgeId`. Les
nœuds de la scène visuelle UE servant de référence restent explicitement hors
document pendant la migration et ne peuvent pas être connectés aux nœuds RVN.

## Transpilation

Pipeline prévu :

1. validation structurelle du graphe ;
2. résolution des références de labels, personnages, variables et assets ;
3. ordonnancement stable des nœuds d'exécution ;
4. reconstruction des expressions avec leur précédence ;
5. émission déterministe du texte `.rvn` ;
6. reparsing avec `rvn_parser` ;
7. analyse sémantique équivalente à `rvn check` ;
8. écriture atomique uniquement si aucune erreur bloquante n'existe.

Le script généré reste visible en lecture seule dans l'éditeur. Les diagnostics
doivent pouvoir sélectionner le nœud et le pin responsables.

## Coquille de l'éditeur

Disposition générale :

```text
┌──────────────────────────────────────────────────────────────┐
│ Projet │ Édition │ Affichage │ Transpiler │ Valider │ Export │
├──────────────┬───────────────────────────────┬───────────────┤
│ Projet RVN   │ Onglets et fil d'Ariane       │ Inspecteur    │
│              ├───────────────────────────────┤               │
│ Scripts      │                               │ Propriétés    │
│ Labels       │      BlueprintCanvas          │ du nœud       │
│ Personnages  │                               │ sélectionné   │
│ Variables    │                               │               │
│ Assets       │                               │ Pins/défauts  │
│ Locales      │                               │               │
├──────────────┴───────────────────────────────┴───────────────┤
│ Diagnostics │ Script généré │ Recherche │ Journal CLI       │
└──────────────────────────────────────────────────────────────┘
```

À conserver de l'ergonomie Unreal :

- onglets de graphes et fil d'Ariane ;
- panneaux redimensionnables et repliables ;
- palette contextuelle avec recherche ;
- sélection multiple, copier/coller et annuler/refaire ;
- commentaires, groupes colorés, minimap et cadrage ;
- inspecteur contextuel et diagnostics navigables.

À ne pas reprendre : viewport 3D, composants Actor, réplication réseau,
paramètres de classes Unreal, simulation physique, compilation C++ et sélection
d'objet de débogage.

Fonctions spécifiques à rust-VN :

- explorateur de fichiers `.rvn` ;
- inventaire des labels, personnages, émotions et variables persistantes ;
- navigateur d'assets avec miniatures et contrôle des chemins ;
- éditeur de traductions et clés de locale ;
- éditeur de dialogue et interpolation ;
- options et pins dynamiques pour `choice` et `imagemap` ;
- vues Diagnostics, Script généré, Recherche et Journal CLI ;
- actions Enregistrer, Transpiler, Valider et Exporter `.rvn`.

## Découpage Rust actuel

```text
rust-VN/
├── rvn_graph/src/{types,catalog,pin_catalog,document,codegen}.rs
├── rvn_cli/src/blueprint.rs
├── rvn_parser/
└── docs/blueprint-editor-spec.md

makepad/apps/blueprint_demo/
├── src/main.rs
└── src/workspace.rs
```

`rvn_graph` ne dépend d'aucune API graphique. L'application Makepad projette ce
modèle, gère la session et les interactions, puis délègue la validation et la
transpilation au backend partagé avec le CLI.

## Ordre d'implémentation

1. coquille visuelle de l'éditeur autour du canvas validé ;
2. modèle persistant avec identifiants stables ;
3. catalogue des nœuds rust-VN et palette contextuelle ;
4. création, suppression et connexion interactives ;
5. inspecteur typé et pins dynamiques ;
6. transpileur, reparsing et diagnostics ;
7. explorateur de projet, assets et localisation ;
8. sauvegarde, migrations de schéma et tests aller-retour.

## Interface CLI

Le document persistant utilise l'extension `.rvngraph` et contient le JSON
versionné de `GraphDocument`. Le frontend Makepad peut prévisualiser le résultat
sans écriture avec :

```text
rvn blueprint transpile scripts/chapter.rvngraph --stdout
```

L'export définitif remplace atomiquement le `.rvn` seulement après validation du
graphe, émission et reparsing réussi :

```text
rvn blueprint transpile scripts/chapter.rvngraph -o scripts/chapter.rvn
```

L'éditeur accepte `--graph <chemin.rvngraph>`. Sans fichier existant, il ouvre
un graphe narratif rust-VN d'exemple entièrement transpilable. Raccourcis du
document : `Ctrl+S` enregistrer, `Ctrl+O` recharger, `Ctrl+Z` annuler et
`Ctrl+Shift+Z` ou `Ctrl+Y` rétablir. L'historique conserve au plus 100 snapshots
de `GraphDocument`; le viewport et les caches de rendu n'en font pas partie.

## Interactions et flux de production implémentés

- Le clic gauche sélectionne un nœud ; Maj/Ctrl-clic étend la sélection et le glisser déplace tout le groupe. Le clic-glissé sur le vide trace une sélection rectangulaire.
- Le déplacement des nœuds est aimanté à une grille de 16 unités. Le clic droit-glissé panoramique le canvas ; un clic droit sans déplacement ouvre la palette contextuelle.
- Tirer un câble puis le relâcher dans le vide ouvre une palette filtrée aux seuls nœuds et pins compatibles. Le câble temporaire est volontairement plus épais que les câbles établis.
- Les interactions du graphe sont strictement découpées à la zone visible de la grille : les panneaux, l'inspecteur et les barres d'outils ne capturent jamais une opération du canvas.
- Le survol d'un nœud ou d'une pin produit un retour visuel et une infobulle typée. La bulle de commentaire permet d'ajouter puis d'éditer une annotation depuis l'inspecteur.
- `Suppr`/`Retour arrière` supprime le nœud sélectionné et ses connexions ; l’entrée du graphe est protégée.
- `Ctrl+C`/`Ctrl+V` copie et colle un nœud avec ses propriétés et valeurs de pins.
- `Ctrl+D` duplique immédiatement le nœud sélectionné.
- `Échap` annule un tirage de câble ou ferme la palette contextuelle.
- Un clic sur une propriété de l’inspecteur ouvre son édition typée ; `Entrée` valide et `Échap` annule.
- **Enregistrer** sérialise le `.rvngraph` par remplacement atomique.
- **Transpiler** valide le DAG, génère le script et l’affiche dans « Script généré ».
- **Valider** affiche les diagnostics structurels et les erreurs de génération/parsing.
- **Exporter .rvn** écrit le script généré à côté du graphe par remplacement atomique.

Les erreurs de chargement, sauvegarde, édition typée et transpilation sont visibles dans les diagnostics et la barre d’état. L’inspecteur dérive ses champs des propriétés et pins du catalogue et prend en charge `bool`, `int`, `float`, `string` et listes de chaînes. La modification des options d’un choix conserve les identifiants et câbles des options communes.

## Valeurs visibles et sémantique des connexions

Une valeur métier ne doit jamais être cachée dans une pin non connectée. À l'ouverture, l'éditeur matérialise les anciens défauts significatifs sous forme de nœuds typés et les connecte à leur consommateur : texte, personnage, variable, scène, transition et littéral. Le document devient modifié mais n'est pas sauvegardé sans action explicite de l'utilisateur.

- `Dialogue` reçoit un `Texte` et un `Personnage` visibles.
- `Définir une variable` reçoit une référence de variable et une valeur visible ; la liste `VARIABLES` du panneau gauche crée les références, tandis que le nœud d'affectation conserve la responsabilité de modifier la valeur.
- `Changer de scène` reçoit un asset de scène et un nœud de transition. Un double-clic sur l'asset ouvre le navigateur des assets réels du projet ; le prochain fichier choisi alimente ce nœud.
- `Personnage` expose le personnage et son émotion par défaut dans un nœud de valeur au dessin proche du `SET` d'Unreal.
- Chaque transition (`none`, `fade`, `dissolve`, glissements, zooms, `wipe`, `blur`) possède son propre nœud compact. Sa durée est éditable en millisecondes et le script natif émis utilise la forme `fade(500)`.

Les nœuds `Condition` et `Choix` conservent leurs points de convergence dans le modèle structuré, mais ces pins techniques ne sont jamais affichées. Le canvas montre uniquement `Vrai`/`Faux` pour une condition et les libellés métier des options pour un choix ; les câbles de convergence sont reconstruits visuellement vers la suite du flux.

Le bouton `+` d'un choix crée une nouvelle option, son pin d'exécution et une condition facultative sans valeur `false` implicite. Une condition absente signifie que l'option reste disponible.

## Nœuds mathématiques et logiques

La palette présente un nœud compact et vitreux par opération rust-VN, avec le symbole réel comme titre plutôt qu'un générique « Opérateur binaire » :

- arithmétique : `+`, `−`, `×`, `÷` et négation unaire ;
- comparaison : `==`, `!=`, `<`, `<=`, `>`, `>=` ;
- logique : `AND`, `OR`, `NOT`.

Les anciens nœuds génériques binaires et unaires restent lisibles pour la compatibilité des fichiers existants, mais ne sont plus proposés dans la palette normale.

## Couverture du transpileur

Le backend émet et reparse les instructions de structure, narration, flux, variables, scènes/cinématiques, personnages/sprites, progression, timers, audio, imagemap et typewriter. Les expressions prises en charge sont les littéraux, variables, opérateurs binaires/unaires, appels de fonction, listes et index. Les nœuds internes `BranchEnd` structurent les branches sans apparaître sur le canvas.

## Workspace et édition multi-document

- L’éditeur remonte depuis `--graph` jusqu’au premier `rvn.toml`, puis applique les chemins `[paths].assets` et `[paths].locales` ; sans manifeste, le dossier du graphe devient la racine.
- L’index ignore `.git`, `target` et les dossiers d’IDE. Il classe réellement graphes, scripts, assets, locales et fichiers de configuration.
- Les quatre sections de l’explorateur affichent les fichiers indexés. Un graphe s’ouvre dans un onglet conservant son document et son indicateur de modification.
- `Ctrl+Shift+F` recherche sans distinction de casse dans les chemins et contenus textuels, avec fichier, ligne et aperçu. Un résultat `.rvngraph` peut être ouvert directement.
- `Ctrl+A` sélectionne tous les nœuds. Maj/Ctrl-clic étend ou réduit la sélection ; un rectangle de sélection est tracé sur le canvas ; le déplacement agit sur tout le groupe. Alt-glisser conserve le panoramique.

## Propriétés dynamiques avancées

- `Choice.options` conserve les pins existants lors d’un renommage et ajoute/supprime les pins nécessaires.
- `SpriteAnimate.params` accepte une liste `nom=valeur`, par exemple `loop=true, speed=1.5`.
- `Imagemap.hotspots` accepte `nom:x1:y1:x2:y2`, plusieurs entrées étant séparées dans la liste.
- `MethodCall` expose désormais un argument et une transition dans ses pins typés.
- Ces valeurs sont émises dans le script natif puis contrôlées par le parseur ; un encodage dynamique invalide devient un diagnostic de transpilation.

## Migrations `.rvngraph`

Le chargement passe par une chaîne de migration avant la désérialisation stricte. La migration initiale `v0 → v1` ajoute la version, les propriétés optionnelles et reconstruit les compteurs d’identifiants à partir des IDs stables existants. Les versions plus récentes que l’éditeur restent refusées explicitement afin d’éviter toute perte de données.

## Roadmap produit priorisée — audit Blueprint UE5

Cette roadmap transforme l’audit de l’éditeur en quatre jalons ordonnés. La référence
d’interaction et de finition visuelle est le Blueprint Graph Editor d’Unreal Engine 5,
adapté au vocabulaire d’un visual novel plutôt que copié sans discernement. Chaque
jalon doit conserver la compatibilité du format `.rvngraph`, rester testable par le
backend et ne jamais afficher un état « valide » si la génération du script échoue.

### Étape 1 — Rétablir la confiance dans le graphe

- corriger la sémantique de la sortie `value_out` de `SetVariable` et ajouter un test
  de non-régression lorsque cette valeur alimente une expression ;
- repartir sur un nouveau graphe vitrine, transpilable et représentatif du flux
  narration → variable → condition → choix → changement de scène ;
- fournir le nœud pur `Format Text` fidèle à UE5 : patron localisable, arguments
  nommés créés depuis `{Nom}`, valeurs alimentées par des `GET` déclarés dans le
  panneau Variables et sortie directement compatible avec `Dialogue.Texte` ; son
  champ multiligne adapte la largeur puis la hauteur du nœud, replie entre les mots,
  accepte le clic de positionnement et la navigation au curseur (flèches,
  début/fin de ligne, suppression et `Maj+Entrée`) ;
- exécuter automatiquement la validation structurelle et la transpilation à
  l’ouverture ainsi qu’après chaque modification significative ;
- conserver des diagnostics structurés avec sévérité, code, nœud, pin et connexion,
  puis permettre de centrer/sélectionner l’élément concerné depuis le panneau ;
- rendre les reconnexions transactionnelles : tant que la nouvelle liaison n’est pas
  valide et confirmée, l’ancienne liaison reste intacte ;
- garantir la lisibilité des câbles façon UE5 : tangentes horizontales visibles à la
  sortie des nœuds, surbrillance épaissie après survol prolongé, masquage immédiat
  des aides pendant un branchement, occlusion correcte entre nœuds superposés et
  insertion d’un reroute typé par double-clic sur un câble ;
- introduire un véritable point de sauvegarde dans l’historique : annuler ou rétablir
  jusqu’au document sauvegardé doit retirer l’astérisque de modification ;
- prévenir toute perte lors d’un rechargement ou d’une fermeture avec des
  modifications non enregistrées et maintenir une copie de récupération automatique.

**Définition de fini :** le graphe vitrine s’ouvre sans faux voyant vert, se valide,
se transpile, se sauvegarde et s’exporte ; une reconnexion annulée ne détruit aucune
liaison ; l’état modifié suit exactement le contenu ; un diagnostic permet de revenir
à son nœud.

### Étape 2 — Devenir un véritable éditeur

- remplacer les menus, filtres et recherches décoratifs par des commandes réelles ;
- rendre les panneaux redimensionnables, repliables et leurs listes défilables ;
- permettre la fermeture et le débordement des onglets ;
- ajouter `Cadrer tout`, `Cadrer la sélection` et une mini-navigation cohérente ;
- ajouter les menus contextuels de nœud, pin et canvas ;
- copier/coller une sélection complète avec ses câbles internes ;
- fournir une navigation clavier prévisible et documentée.

**Définition de fini :** aucun contrôle visible n’est factice, le projet reste
utilisable sur une petite fenêtre et toutes les opérations principales sont
accessibles à la souris comme au clavier.

### Étape 3 — Retrouver la productivité Blueprint

- cadres de commentaire déplaçables et redimensionnables ;
- nœuds de reroutage, alignement et distribution ;
- favoris, historique récent et palette contextuelle pondérée ;
- recherche des références de variables, labels et assets ;
- signets de graphe et historique de navigation ;
- sous-graphes et fonctions réutilisables ;
- widgets d’inspecteur spécialisés par type plutôt que champs texte génériques.

**Définition de fini :** un graphe narratif volumineux peut être organisé, parcouru
et refactorisé sans manipuler directement le JSON.

### Étape 4 — Dépasser UE5 pour le visual novel

- aperçu instantané de la scène et lecture depuis le nœud sélectionné ;
- points d’arrêt, pas-à-pas et surveillance des variables ;
- carte des chemins narratifs et couverture des choix ;
- miniatures d’assets et édition visuelle des zones d’une imagemap ;
- vue localisation avec textes manquants et variantes de langue ;
- diagnostics des labels, assets et références devenus orphelins.

**Définition de fini :** l’auteur peut écrire, prévisualiser, tester et localiser une
séquence narrative complète sans quitter l’éditeur Blueprint.

### Références visuelles UE5 retenues

- le flux d’exécution blanc utilise des pins en forme de flèche ;
- les pins et câbles de données gardent une couleur stable liée au type ;
- les événements ont un en-tête rouge, les fonctions impures un en-tête bleu, les
  branches un en-tête gris et les opérations pures restent compactes ;
- les nœuds de variable `GET` sont compacts, tandis que `SET` expose entrée et sortie
  d’exécution ainsi que la valeur réutilisable ;
- une conversion automatique doit être courte, lisible et placée sur le câble sans
  masquer les nœuds qu’elle relie ;
- la grille, la sélection orange, les bulles d’aide et le feedback de connexion font
  partie du langage visuel fonctionnel, pas d’un simple habillage.

### Passe d’ergonomie UE5 — parité des interactions

Cette passe complète l’étape 1 sans modifier le schéma `.rvngraph` :

- une machine d’état unique distingue le clic du déplacement (`4 px`), le panning,
  la sélection rectangulaire, le branchement, le glissement de variable, l’édition
  de texte et les menus ; le curseur revient toujours à la flèche à la fin ou à
  l’annulation d’un geste et la main fermée n’apparaît qu’après un vrai panning ;
- les nœuds suivent librement le pointeur pendant le glissement puis se calent sur
  une grille de 16 unités au relâchement ; Maj ajoute à la sélection et Ctrl la
  bascule, sans confondre un simple clic avec un déplacement ;
- le clic droit immobile ouvre la palette, tandis que le clic droit ou central
  déplacé pan le graphe ; la molette anime le zoom autour du pointeur sur environ
  `90 ms`, entre 25 et 200 %, avec trois niveaux de détail ; `Home`, `A` et `F`
  cadrent le graphe ou la sélection ;
- les zones actives des pins restent au minimum à `22 px` écran. Pendant un
  branchement, les destinations compatibles sont éclairées, les autres atténuées,
  une destination interdite seule affiche le curseur d’interdiction et un dépôt
  dans le vide ouvre la palette filtrée avec connexion automatique ; Alt-clic sur
  un pin rompt ses connexions ;
- une reconnexion invalide ou annulée conserve la liaison d’origine. Les câbles
  reliés au pin survolé sont renforcés, le survol stable d’un câble l’épaissit après
  `220 ms`, et les transitions de pins, câbles et bordures sont animées ;
- les documentations attendent `450 ms` de stabilité, utilisent une bulle sombre
  compacte et disparaissent au moindre déplacement significatif, zoom, panning,
  branchement ou menu ;
- l’édition des variables n’autorise qu’un overlay à la fois. Le menu de type se
  place à droite du panneau, se retourne si nécessaire, possède recherche et
  navigation clavier, et absorbe le premier clic extérieur. Le glissement n’est
  actif qu’après le seuil ; Ctrl crée directement un `GET`, Alt un `SET` ;
- les panneaux gauche et droit, les lignes, les champs, les pins et la palette sont
  recalibrés pour une lecture proche d’UE5 à 1920×1080. Le bouton `+` des variables
  devient blanc au survol, comme dans le panneau « Mon Blueprint » d’UE5.

Les raccourcis et gestes de référence suivent la documentation officielle UE5 :
glisser une variable ouvre `Get/Set`, Ctrl-glisser force `Get`, Alt-glisser force
`Set`, Alt-clic sur un pin rompt ses liaisons et déposer un câble dans le vide ouvre
une liste filtrée d’actions. Le Ctrl-glisser de toutes les liaisons d’un pin reste
une amélioration ciblée à ajouter pour reproduire aussi ce raccourci avancé d’UE5.
