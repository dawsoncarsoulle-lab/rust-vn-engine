# Personnaliser les menus

Ouvrez un projet dans l’éditeur Blueprint, puis **Menus du jeu**. Les menus sont
indépendants des graphes narratifs. L’interface du jeu ne doit pas ressembler à
l’éditeur : vous choisissez ses images, ses couleurs et sa disposition.

## Commencer

Le sélecteur de **mise en page** propose Sobre, Illustré et Science-fiction.
Il remplace la composition après confirmation, avec possibilité d’annuler.
Le sélecteur de **thème** change les styles partagés sans déplacer les éléments
ni modifier leurs textes ou leurs actions. Une page personnalisée peut aussi
partir d’un canvas vide.

Chaque page a un nom et un rôle. Le rôle indique au moteur quand l’afficher :
menu principal, pause, sauvegarde, chargement, réglages, galerie, historique,
confirmation, dialogue, réponses aux choix ou commandes rapides. Un rôle absent
utilise l’interface de secours du moteur. Deux pages ne peuvent pas occuper le
même rôle. Les pages personnalisées s’ouvrent avec l’action **Ouvrir une page**.

## Composer une page

La colonne gauche comporte quatre onglets : **Pages**, **Calques**, **Outils**
et **Modèles**. Ils séparent la navigation, l’arborescence, les éléments à ajouter
et les composants réutilisables.

L’inspecteur est divisé en cinq rubriques : **Apparence et styles**,
**Texte et typographie**, **Position et conteneur**, **Images et cadres**,
**Données et actions**. Les boutons **Appliquer les propriétés** et
**Annuler la saisie** restent visibles pendant le défilement. Annuler la saisie
restaure les champs du document ; Annuler/Rétablir dans la barre d’outils agit
sur les modifications déjà appliquées et conserve la sélection si elle existe.
Le champ de texte accepte plusieurs paragraphes : Entrée ajoute une ligne,
Ctrl+Entrée applique le texte. Les autres champs conservent la validation par Entrée.

Sélectionnez un élément sur le canvas ou dans l’arborescence. Glissez-le pour le
déplacer et utilisez sa poignée orange pour le redimensionner. Maj-clic ajoute
des éléments à la sélection. Les commandes permettent de copier, dupliquer,
aligner et répartir la sélection. Les cadres orange ne sont pas visibles en jeu.

Dans **Calques**, déposez sur le bord supérieur ou inférieur d’une ligne pour
réordonner les éléments ; le trait bleu indique la destination. Déposez au
centre d’un conteneur pour y imbriquer le calque. Sa position est conservée
dans les panneaux libres ; les conteneurs automatiques pilotent leur placement.
Les parents verrouillés et les cycles sont refusés. Chaque déplacement est
annulable en une étape. Les chevrons replient les groupes ; la recherche révèle
les résultats et leurs parents sans effacer vos groupes repliés.

Après un clic dans le canvas : Ctrl+C copie, Ctrl+V colle, Ctrl+D duplique,
Ctrl+A sélectionne, Suppr retire la sélection, Ctrl+Z annule, Ctrl+Y rétablit
et F cadre la sélection. Ces raccourcis ne capturent pas les champs de saisie.
Dans un composant, l’ajout et le collage placent les éléments sous sa racine.
Une seconde racine ou une racine manquante bloque l’enregistrement avec une
explication, sans perdre le dernier document enregistré.

Le bouton central de la souris déplace la vue sans déplacer les éléments.
**Cadrer la sélection** zoome sur l’élément ou le groupe ; **Cadrer la page**
rétablit la vue complète. La zone défilante découpe ses enfants. Dans le jeu,
la molette et la barre verticale permettent de parcourir son contenu, y compris
lorsque plusieurs zones sont imbriquées.

**Opacité du groupe (%)** règle la transparence de l’élément et de ses enfants.
La valeur peut être héritée d’un style partagé ou remplacée localement.

Les champs X, Y, largeur, hauteur, marges et espacement se valident avec Entrée
ou **Appliquer les propriétés**. Une saisie validée ou un geste correspond à une
étape d’annulation. Appliquez la saisie avant de changer de sélection.

Pour une liste de cartes, **Données et actions** propose des champs distincts
**Colonnes**, **Lignes** et **Nombre d’emplacements**, avec la capacité et le
nombre de pages calculés. **Remplissage des cartes** choisit une lecture par
lignes (1, 2, 3 sur la première ligne) ou par colonnes (1, 2 dans la première
colonne). Les identifiants des sauvegardes et leurs opérations ne changent pas.

Les neuf boutons d’ancrage choisissent le bord ou le centre de référence sans
déplacer l’élément à la résolution de travail. Les enfants d’un conteneur
horizontal, vertical ou en grille suivent son placement automatique. Leurs
ancres ne remplacent pas ce placement. Les cycles d’imbrication sont refusés.

Le champ **Ordre clavier / manette** règle l’ordre de navigation des contrôles.
Masquer, désactiver et verrouiller sont distincts : le verrouillage protège
l’élément contre les déplacements dans l’éditeur ; la désactivation concerne
son utilisation dans le jeu.

## Cartes de sauvegarde et de galerie

Pour une carte de sauvegarde ou de galerie, ouvrez **Modifier le composant**,
sélectionnez son texte, son image ou son conteneur, puis **Données et actions**.
**Afficher dans la carte lorsque…** peut dépendre d’un emplacement vide,
d’une sauvegarde présente/protégée, ou d’une illustration verrouillée/déverrouillée.
Vous pouvez ainsi dessiner séparément un cadenas, un message « emplacement vide »
et une miniature. La condition du conteneur concerne également ses enfants.
**Hériter du composant** conserve la règle partagée ; **Toujours** la remplace
localement. Une carte entièrement masquée ne conserve pas de cible cliquable.

### Dessiner la pagination

Sur une page contenant une liste de sauvegardes, ajoutez des boutons ordinaires
et choisissez **Page précédente**, **Page suivante**, **Première page**,
**Dernière page** ou **Aller à la page numéro** dans **Données et actions**.
Pour la dernière action, renseignez le numéro à partir de 1. Vous pouvez changer
librement le texte, l’image, la forme et la position de ces boutons. Le même
choix existe dans un nœud Action du mode Interactions ; son champ Valeur reçoit
le numéro demandé. Un graphe de clic remplace l’action simple, sans double effet.

L’ajout d’une pagination personnalisée retire les flèches automatiques de la
liste et libère leur espace. Un Texte lié à **Numéro de page des sauvegardes**
affiche la page courante. Les limites sont vérifiées et les pages Sauvegarder
et Charger mémorisent séparément la page consultée. Une pagination personnalisée
demande une seule liste de sauvegardes sur la page, pour éviter toute ambiguïté.

Dans une carte de sauvegarde, un élément **Texte** peut recevoir la donnée
**Numéro d’emplacement**. Le moteur fournit le véritable numéro, y compris
après un changement de page (7 à 12 sur la deuxième page de six cartes).
Placez à côté un texte fixe « Fichier » ou « Emplacement » si vous souhaitez
un libellé. Cette donnée appartient au modèle de carte, pas à un texte isolé
sur une page : l’éditeur signale les liaisons sans contexte de liste.

Le sélecteur de données filtre ses propositions selon le contrôle : miniature
pour une image, numéro/date/résumé pour un texte, opérations d’emplacement pour
un bouton. Les données de carte sont proposées dans l’espace du composant.

## Dessiner les réponses et l’historique

Sélectionnez la liste concernée, puis **+ Modèle de réponse** ou
**+ Ligne d’historique**. Le composant créé est lié à cette liste. Utilisez
**Modifier le composant** pour changer son fond, sa police ou ajouter des images.
Le texte lié à **Texte de réponse** ou **Texte d’historique** reçoit le contenu
du scénario ; sa ligne grandit automatiquement au lieu de couper le texte.
Les indices et destinations des réponses restent ceux du scénario.

Pour un texte ou un bouton ordinaire, **Texte et typographie → Ajuster la hauteur
au texte** mesure le contenu affiché avec sa police et sa largeur actuelles.
Appliquez d’abord votre saisie. L’opération garde la largeur, se défait en une
étape avec **Annuler**, et ne modifie pas le contenu. Une hauteur étirée par les
ancres doit d’abord être remplacée par un ancrage fixe. Ce réglage ponctuel ne
remplace pas la hauteur automatique des réponses et de l’historique : vérifiez
les traductions dans l’aperçu moteur.

**Hauteur → Automatique — hauteur minimale** fait cette adaptation à chaque
changement de contenu ou de largeur, dans le canvas et le moteur. La hauteur
saisie devient un minimum. Dans un conteneur vertical ou une grille, les
éléments suivants se déplacent avec le texte. Dans une composition libre,
prévoyez l’espace nécessaire ou utilisez un conteneur vertical. Le réglage est
aussi disponible pour les textes et boutons des composants de carte. La zone
de dialogue riche conserve sa hauteur et son défilement dédiés.
Si vous ajoutez un champ lié au nom dans une ligne d’historique, le champ de
texte ne répète plus ce nom. Sans champ séparé, il conserve « Nom : texte ».
Le narrateur sans nom ne produit pas de préfixe vide.

Dans le moteur, les cases à cocher affichent leur état et le sélecteur de langue
ouvre une liste. Flèches puis Entrée permettent de choisir ; Échap ou un clic
extérieur annulent. Le sélecteur utilise les langues disponibles du projet.

## Images, polices et styles

Choisissez une image ou une police dans les ressources du projet. Une image
externe peut être déposée sur le canvas ou dans le champ d’image ; une copie
est importée, son original reste intact. Actualisez les ressources après un
ajout externe. Les modes d’image sont proportionnel, remplissage recadré,
étirement et cadre en neuf zones.
Les quatre bordures de découpe sont exprimées en pixels de l’image source :
elles ne changent pas avec la résolution du jeu. Le centre et les côtés
s’étirent, tandis que les coins conservent leur découpe. Dans un composant,
déposer une image ajoute un enfant à sa racine, sans casser sa structure.

Choisissez d’abord l’état à personnaliser, puis la couleur : normal, survolé,
pressé, désactivé, texte, bordure, focus ou sélection. Le sélecteur propose le
cercle chromatique et les valeurs de couleur. Les arrondis et bordures ont
leurs propres champs.

Les couleurs du texte possèdent aussi leurs états **Texte survolé**, **Texte
pressé**, **Texte désactivé**, **Texte focus** et **Texte sélectionné**. Elles
suivent le style partagé, sauf exception locale. La commande de rétablissement
ne retire que l’exception de l’état choisi. Les listes de langues reprennent
la palette du sélecteur. Les couleurs écrites dans les balises du dialogue
restent indépendantes de ces états de contrôles.

Un style partagé s’applique à plusieurs éléments. **Créer un style de cette
sélection** capture son apparence ; **Mettre à jour le style partagé** propage
la modification aux éléments qui l’utilisent. Les exceptions locales restent
prioritaires. **Rétablir le style hérité** retire les exceptions de la sélection.

Une instance de composant hérite de son image. Choisir une autre image crée
une exception ; choisir **Aucune image** masque explicitement l’image héritée.
Rétablir l’héritage permet de suivre à nouveau le composant.

### Créer votre propre composant

Sélectionnez un bouton, un texte ou un groupe dessiné, ouvrez **Modèles**, donnez
un nom au composant puis choisissez **Créer depuis la sélection**. L’original
reste en place. **Insérer une instance** ajoute un exemplaire réutilisable ;
**Modifier le composant** ouvre sa composition partagée. Une insertion dans
le composant lui-même, directement ou indirectement, est refusée.

Les nouvelles instances héritent du texte, de l’action et de la donnée liée.
Modifier l’un de ces champs dans une instance crée une exception locale, sans
changer les autres. L’inspecteur indique **Hérité du composant** ou
**Exception locale** ; les boutons **Rétablir** réactivent cet héritage séparément.
Les anciennes instances conservent leurs valeurs locales pour ne pas changer
les menus déjà enregistrés.

Les composants reprennent la présentation et les actions simples. Les graphes
d’interaction restent attachés aux pages : la capture d’un élément portant
un tel graphe est refusée avec une explication, et non copiée partiellement.

## Cartes de sauvegarde et galerie

Créez une carte dans **Composants**, puis **Modifier le composant**. Choisissez
ce composant comme modèle de la liste. Le moteur répète la carte et remplit
ses données : miniature, date, résumé, illustration, titre ou disponibilité.
L’espace temporaire d’édition du composant n’est pas exporté comme page de jeu.

Le modèle **Carte avec boutons séparés** contient Sauvegarder, Charger,
Supprimer et Protéger, liés à leur emplacement. Le chargement est désactivé
sur un emplacement vide. Protéger interdit l’écrasement et la suppression,
mais pas le chargement ; Déprotéger autorise de nouveau ces opérations.
Écraser, supprimer ou charger une partie demande confirmation. Annuler
préserve la sauvegarde, la partie et le menu d’origine. Recommencer et continuer
une autre sauvegarde depuis une partie bénéficient de la même protection,
y compris sans page de confirmation personnalisée.

La galerie distingue les illustrations déverrouillées et verrouillées ; une
illustration verrouillée ne montre pas sa miniature. L’historique garde le
retour à la ligne des textes longs. Sur ordinateur, une sauvegarde manuelle
utilise la dernière capture complète correspondant à l’écran de jeu, sans le
menu de sauvegarde. Si cet écran n’a pas encore pu être capturé, le moteur
conserve sa référence de miniature de secours. Les sauvegardes rapides et
automatiques utilisent également une capture du jeu, attachée dès qu’un écran
correspondant est disponible. Si la sauvegarde a été remplacée entre-temps,
la capture différée ne l’écrase pas. Le dialogue, le décor et les personnages
sont inclus, mais pas le menu de sauvegarde. Sans capture disponible, la
référence de secours reste utilisable.

## Interactions

Sélectionnez un contrôle en mode Design, puis passez à **Interactions**.
Le nom et l’identifiant de ce contrôle sont affichés au-dessus de l’inspecteur.
La liste ne contient que ses événements ; si elle est vide, aucun graphe d’un
autre bouton n’est montré. Le sélecteur **Événements de page** donne accès
séparément à l’ouverture et à la fermeture de la page.
Ajoutez son événement : clic, entrée/sortie du survol, focus, changement de
réglage ou ouverture/fermeture de page. Les événements de page n’ont pas de
contrôle cible.
Les images, panneaux et textes peuvent aussi recevoir un graphe de clic ou
de survol. Une zone uniquement sensible au survol n’ajoute pas une étape
inutile au parcours clavier ou manette.

Ajoutez les actions puis reliez leurs sorties blanches aux entrées blanches.
Une condition a deux sorties, Vrai et Faux. Les champs du nœud proposent les
cibles disponibles pour les opérations courantes. Un son se choisit dans les
assets ; le moteur prend en charge Ogg, WAV, MP3 et FLAC.

Navigation du canvas Interactions :

- Glisser avec le bouton droit ou central déplace la vue ; la molette zoome
  autour du pointeur, comme dans le graphe narratif.
- **F** ou **Cadrer la sélection** centre le nœud sélectionné ; **Cadrer la page**
  cadre tout le graphe d’événement.
- Un clic droit sans déplacement ouvre la recherche d’actions. Tirer une
  sortie blanche vers le vide ouvre la même recherche, limitée aux actions
  d’exécution. Tapez un nom, puis choisissez avec la souris ou les flèches et Entrée.
- Configurez l’action proposée dans l’inspecteur puis utilisez **Ajouter ce nœud** :
  le nœud apparaît au point de dépôt et la liaison est créée dans la même étape
  d’annulation. Une configuration invalide ne modifie pas le graphe.
- **Échap** annule la recherche, la liaison ou la saisie en attente.
  **Appliquer**, **Ajouter** et **Annuler la saisie** restent visibles en bas
  de l’inspecteur, même quand les champs d’animation sont ouverts.
- Une saisie en attente protège les modifications du graphe, mais ne bloque
  plus le déplacement de la vue, le zoom ni le cadrage. Validez ou annulez
  cette saisie avant de changer de nœud, d’événement ou de mode.
- **Ctrl+Z**, **Ctrl+Maj+Z** et **Ctrl+Y**, lorsque le canvas est actif,
  annulent/rétablissent les modifications validées. Les mouvements de caméra
  ne créent pas d’étapes d’annulation.

Sans graphe de clic, le bouton utilise son action simple. Avec un graphe de
clic, seul ce graphe s’exécute : il n’y a pas de double activation. Supprimer
le graphe rétablit l’action simple, sans la supprimer. Cette opération est
annulable.

Cette règle s’applique aussi aux boutons Confirmer et Retour d’une confirmation.
Leur graphe peut personnaliser la décision, mais ne peut pas lancer directement
une autre opération du jeu. Confirmer exige une activation explicite : le survol
ou l’ouverture de la page ne peuvent pas approuver une opération destructive.

Les variables d’interface sont séparées du scénario. Les valeurs `state.*`
sont en lecture seule. Les boucles d’exécution sont bornées et plusieurs
navigations pour une même activation sont refusées.

## Tester, enregistrer et partager

Dans l’éditeur narratif principal, **▶ Jouer** compile les graphes du projet,
y compris les modifications validées dans les onglets ouverts, puis lance le
moteur natif. Une erreur de graphe empêche le lancement et reste affichée dans
Diagnostics. Il n’est pas nécessaire d’exporter ni d’écraser les fichiers des
graphes. Les menus utilisés sont leur dernière version enregistrée.

La session conserve la configuration de l’histoire, ses médias et son label
de départ, mais utilise un dossier temporaire pour le script, les menus, les
traductions, les sauvegardes et la progression. **Arrêter** ferme cette session.
Le **Journal CLI** affiche les dernières lignes du moteur et indique le fichier
de journal à consulter. Dans le moteur, F9 affiche l’overlay de débogage existant
et F10 demande une étape lorsque cet overlay est ouvert. Il ne s’agit pas encore
d’un débogueur de nœuds connecté à l’éditeur avec points d’arrêt.

Le mode **Aperçu moteur** des menus reste distinct : il teste la composition
avec une scène et des données de démonstration, pas avec l’histoire réelle.

**Depuis le thème** convertit le `theme.toml` du projet en éléments éditables :
fond du titre, titre, ordre/visibilité/libellés et apparence des boutons,
polices, boîte de dialogue et choix. Enregistrez vos menus avant cette opération ;
elle reste annulable. Le rapport dans **Pages** énumère les propriétés non
converties (par exemple la musique du titre ou la couleur séparée des numéros
de choix) et les ajustements de mise en page. Le fichier `theme.toml` original
reste intact. Des valeurs mal formées produisent une erreur explicite.

**Aperçu moteur** ouvre les menus non encore enregistrés dans une scène de
démonstration. Les champs doivent néanmoins avoir été appliqués. Les sauvegardes
et la progression de cet aperçu restent dans un dossier temporaire ; celles du
projet réel ne sont pas utilisées. **Fermer l’aperçu** arrête ce moteur. Relancez
l’aperçu pour prendre en compte de nouvelles modifications.

**Erreur de l’aperçu** ouvre la page et le graphe concernés par une erreur
d’exécution, puis sélectionne et cadre le nœud signalé. La dernière erreur
reste consultable après fermeture de l’aperçu. Une erreur globale de navigation
peut concerner tout un événement : le graphe est alors ouvert sans désigner
arbitrairement un nœud. Ce diagnostic n’enregistre aucune modification.

Testez les textes longs, les menus et les choix à plusieurs résolutions.
Le sélecteur **Données vides / Données remplies** permet de démarrer l’aperçu
avec deux sauvegardes de démonstration, un historique long et, si les assets en
contiennent, une première illustration déverrouillée. Les autres illustrations
restent verrouillées. Ce réglage ne modifie pas le fichier de menus.
Le moteur reste la référence pour le rendu final, notamment le défilement et
l’écriture progressive.

**Enregistrer les menus** valide le document et ses ressources. Une modification
externe sur disque bloque l’écrasement. Le premier enregistrement d’un ancien
document conserve une sauvegarde avant sa conversion.
La fermeture de la fenêtre est suspendue si des menus ou des champs sont
modifiés. Le bandeau propose d’enregistrer, de continuer l’édition ou
d’abandonner explicitement les modifications ; abandonner ne touche pas au
fichier enregistré. Les champs en cours doivent être appliqués ou annulés
avant l’enregistrement.

L’export `.rvnuitheme` transporte le design et ses ressources référencées.
Examinez le contenu avant de confirmer l’import. Les éléments importés sont
renommés pour conserver les pages, styles, composants et médias existants.
Les fichiers de licence adjacents reconnus sont conservés.
L’examen affiche les noms des copies à créer et les rôles déjà occupés.
Dans ce dernier cas, votre page actuelle reste active : attribuez ensuite le
rôle voulu à la copie pour l’utiliser. Le panneau est défilant jusqu’au bouton
**Importer les copies**. Importer deux fois crée deux séries indépendantes.

## Effets visuels

Pour une ombre, sélectionnez l’élément puis **Ombre du cadre** dans le
sélecteur d’apparence. Choisissez sa couleur et réglez X, Y et Diffusion.
Une couleur transparente désactive l’ombre. Une ombre partagée peut être
surchargée localement ; **Rétablir le style hérité** retire cette exception.

L’indication sous le sélecteur de couleur distingue une exception locale
d’une valeur héritée. **Rétablir cette propriété** remet uniquement l’état
affiché à sa valeur héritée, sans effacer les autres personnalisations.
Les états Focus clavier et Sélectionné s’affichent directement dans le canvas.

Pour les zones défilantes, listes et champs de dialogue, les entrées
**Défilement** règlent la piste, la poignée et ses états survolé/pressé.
La largeur et l’arrondi se trouvent dans **Barre de défilement**. Le canvas
affiche une poignée de démonstration pendant ce réglage ; dans le jeu, elle
n’apparaît que si le contenu dépasse réellement le cadre. Ces propriétés
peuvent également être enregistrées dans un style partagé.

Dans **Interactions**, choisissez **Animer un élément**, puis la cible, le type,
la durée et la progression. Un fondu utilise 0 à 1 ; un déplacement utilise
X, Y dans les coordonnées du projet ; une échelle de 1 conserve la taille ;
une couleur attend quatre valeurs RGBA entre 0 et 255. Les effets peuvent être
reliés au même événement pour démarrer ensemble. Ils ne modifient pas la
composition enregistrée. Le changement de page ne retarde pas la navigation
pour attendre un effet de fermeture ; aucune timeline à points-clés n’est prévue.

## Dialogues longs et confirmations

Le champ lié à `dialogue.text` garde les effets d’écriture du scénario. S’il
dépasse son cadre, une barre permet de relire le début ; la molette et les
touches Page précédente/Page suivante fonctionnent aussi. Remonter suspend le
suivi automatique. Le dialogue suivant remet la zone en haut.

Les confirmations acceptent les graphes d’ouverture, fermeture, survol,
sortie du survol et focus, en plus du clic. Ces événements peuvent modifier
leur présentation ou jouer un son, mais seuls les clics/validations explicites
peuvent confirmer ou annuler. Échap reste toujours une annulation. Le parcours
clavier suit l’ordre de focus des contrôles ; les simples zones de survol
ne deviennent pas des étapes du parcours.

## Fond de page

Dans **Pages → Fond de la page**, choisissez une couleur et sa transparence.
Le canvas se met à jour pendant le choix ; fermer le sélecteur constitue une
seule étape d’annulation. Cette couleur n’impose aucune image de fond : vous
pouvez placer une image ou une composition de panneaux au-dessus.

## Contrôles propres à votre menu

Ajoutez une **Case à cocher**, un **Curseur** ou un **Sélecteur** depuis Outils.
Dans **Données et actions → Source du contrôle**, choisissez **Variable
d’interface** et donnez-lui un nom, par exemple `ui.indications`.

- Case à cocher : choisissez son état initial.
- Curseur : réglez valeur initiale, minimum, maximum et pas.
- Sélecteur : saisissez une option par ligne, dans l’ordre voulu, puis une
  valeur initiale figurant exactement parmi ces options.

Appliquez les propriétés. Le contrôle est maintenant indépendant des réglages
du jeu. Dans Interactions, sélectionnez **Changement de valeur** : la variable
est mise à jour avant le graphe. Elle apparaît dans les cibles des opérations
**Variable locale** et **Condition** ; un choix Oui/Non ou la liste d’options
évite de saisir les valeurs à la main. L’ouverture d’un sélecteur seule ne
déclenche pas ce graphe. Réaffecter la même valeur ne le déclenche pas deux fois.

Ces valeurs durent pendant la session d’interface, y compris entre ses pages,
mais ne sont pas enregistrées dans les sauvegardes du scénario ou les réglages
globaux. Le préfixe `state.` est réservé aux données du moteur. Deux contrôles
partageant une variable doivent avoir la même configuration. Les réglages du
jeu existants restent disponibles avec l’autre source du contrôle.
À l’import d’un design, ses variables locales sont renommées avec ses copies
de pages et leurs références sont mises à jour : deux designs importés ne
pilotent pas accidentellement les mêmes contrôles. Le rapport indique ces noms.

## Images interactives

**Images et cadres → Image de l’état** sélectionne Normal, Survolé, Pressé,
Désactivé, Focus clavier ou Sélectionné. Choisissez ensuite une image dans les
assets, ou **Aucune image** pour la masquer dans cet état seulement. L’image
normale n’est pas remplacée lorsque vous modifiez un autre état.
**Rétablir l’image héritée de cet état** retire seulement cette exception.
Ces images font partie des styles partagés et des paquets `.rvnuitheme`.
Sans image spécifique, l’état conserve son image de repli ; l’appui et le focus
peuvent reprendre l’image de survol. La composition affiche l’état sélectionné,
tandis que l’aperçu moteur réagit réellement à la souris et au clavier.

## Projets d’exemple et vérification

### Nommer les styles partagés

Sélectionnez un élément, puis **Apparence et styles**. Saisissez par exemple
« Boutons papier » dans **Nom du style à créer / nouveau nom**, puis choisissez
**Créer un style de cette sélection**. Pour renommer un style existant,
sélectionnez un de ses éléments, saisissez le nouveau nom et utilisez
**Renommer le style partagé** : les références des pages et composants sont
mises à jour ensemble. Les exceptions locales sont conservées, les conflits
de noms sont refusés et l’opération reste annulable.

Pour partager uniquement la couleur en cours, utilisez **Appliquer cette
propriété au style** sous le sélecteur de couleur. Contrairement à une capture
complète du style, cette commande ne remplace pas les polices, les dimensions
ou les autres états. Les éléments qui ont leur propre exception gardent celle-ci.

### Résumés et titres longs dans les cartes

Dans **Modèles → Modifier le composant**, sélectionnez le résumé ou le titre,
puis **Texte et typographie → Hauteur → Automatique — hauteur minimale**.
La valeur de hauteur reste un minimum : le texte peut grandir, les champs placés
en dessous sont décalés et la carte s’agrandit. Les rangées suivantes gardent
leur ordre et se décalent sans recouvrir la précédente. Si le contenu dépasse
la liste, le moteur permet de le faire défiler, y compris vers un bouton ciblé
au clavier. Les nouvelles cartes utilisent ce réglage pour leurs textes liés.
Les anciennes cartes à hauteur fixe conservent ce choix jusqu’à sa modification.

### Exemples et tests

Les projets [Sobre, Illustré et Science-fiction](../examples/menu-designs/README.md)
ont été créés et exportés depuis l’interface. Chaque projet fournit onze pages
éditables et un paquet partageable ; le modèle Illustré montre comment choisir
un média existant sans l’imposer aux nouveaux projets.

Le test `bash tools/verify_menu_editor.sh` pilote une instance native indépendante :
création d’un projet, installation et export des trois modèles, import répété,
annulation et rétablissement. Il ne touche pas aux projets récents de l’utilisateur.
Le test `bash tools/verify_menu_native.sh` exerce les menus dans le moteur, avec
des parties temporaires, puis ferme ses fenêtres.
Le test `bash tools/verify_menu_designs.sh` charge les trois projets livrés sans
réécrire leur composition et réalise neuf captures par modèle à trois tailles
demandées. Son rapport indique les dimensions réellement accordées par le bureau,
qui peut limiter les grandes fenêtres. Il vérifie aussi que les menus n’avancent
pas le scénario et que leur document enregistré reste intact. Les données de
sauvegarde de ces essais sont créées uniquement dans leur dossier temporaire.
Chaque parcours réduit la fenêtre à 960×600 pendant la pause, puis la rétablit
à 1280×720. La fenêtre de jeu est redimensionnable par défaut ; une configuration
existante peut explicitement fixer `[window].resizable = false`.
Lancez ces trois tests l’un après l’autre pour qu’ils ne se disputent pas le focus
du bureau.

## Limites actuelles

Ce guide décrit les outils présents, pas une certification de l’éditeur
complet. La couverture exhaustive des confirmations, périphériques et
résolutions demande encore du travail. Les deux exemples
de menus de référence n’ont pas encore été reproduits intégralement avec les
outils livrés.
