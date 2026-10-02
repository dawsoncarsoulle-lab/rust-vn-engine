# L’Atlas des Brumes — interface jouable

L’Atlas est un véritable écran RVN : son dessin utilise un Canvas et des
primitives 2D, ses champs utilisent les contrôles natifs du moteur, et ses
actions passent par les gestionnaires d’événements. Ce n’est pas une page HTML
imitant un jeu. Les images, personnages et lieux sont originaux ; aucun asset
de Nintendo n’est utilisé.

## Utilisation

L’inventaire est le volet initial. Identité, Inventaire et Carte restent visibles
ensemble : le volet actif est devant, les autres deviennent des aperçus latéraux.
Cliquez un aperçu ou un onglet, ou utilisez **Q / E**, **Page précédente /
Page suivante**. Les transitions respectent le réglage de mouvements réduits.

Dans l’inventaire, les flèches déplacent la sélection dans la grille **6 × 3**.
Un clic sélectionne un emplacement ; **Entrée** ou le bouton de confirmation
équipe, examine ou utilise l’objet. Les emplacements non possédés restent gris
et ne peuvent pas être utilisés. L’élixir guérit un point et consomme une dose,
mais n’est pas gaspillé si la vitalité est déjà complète. Le sifflet possédé
joue sa mélodie et en conserve le résultat pour l’histoire.

Sur la carte, un clic sélectionne une destination ; les flèches parcourent les
six lieux. Survoler un marqueur affiche son nom. Le cartouche inférieur expose
la condition d’accès, y compris au clavier. **Entrée** ou **Voyager** valide la
route et ferme l’Atlas ; une route verrouillée ne peut pas être contournée par
le raccourci. **Échap** ou **Retour** annule sans demander de voyage.

L’identité contient deux vrais champs de saisie et une liste d’origines.
**Q / E restent des lettres dans ces champs**, pas des commandes de changement
de page. **Tab / Maj+Tab** parcourent les contrôles. Entrée dans un champ de nom
rend le focus à l’Atlas. Chaque nom est limité à 24 caractères pour préserver
cette mise en page. Les réglages de contraste, taille du texte, mouvement et
lecture vocale sont liés aux variables sauvegardées. La lecture vocale décrit
également les objets et lieux parcourus au clavier, si le service de voix de
la plateforme est disponible.

Le HUD montre l’identité, la vitalité, les éclats, l’équipement, le lieu et
l’objectif courant. Il ne vole pas le focus et ne permet pas d’ouvrir un menu
pendant une conversation. L’histoire garde le contrôle du moment où l’Atlas
peut être utilisé.

## Source éditable

**[main.rvn](main.rvn) est la source complète, canonique et liée à l’éditeur.** Elle contient
la narration, les calculs, les écrans et les gestionnaires, afin que tout soit
accessible dans les Blueprints et les créateurs visuels actuels. Ouvrez le
projet, puis Interfaces → `atlas` ou `atlas_hud`. Le Canvas conserve sa référence
à `atlas_draw` / `atlas_hud_draw` ; ses calculs restent de vrais graphes.

Le dépôt source du moteur conserve `authoring/atlas.rvn.part`,
`authoring/features.rvn.part` et `authoring/story.rvn.part` comme fragments
initiaux archivés. Ils ne sont pas nécessaires au paquet installé et ne sont
pas des scripts supplémentaires à importer.
Après une modification dans l’éditeur, **ne les réassemblez pas au-dessus de
`main.rvn`** : cela écraserait votre modification.
Appliquer change le graphe ; Enregistrer écrit la source liée. Modifier un
aperçu ne transforme pas automatiquement les calculs en valeurs figées.

## Contrat avec l’histoire

Écran modal `atlas()`, racine **`atlas_root`**, Canvas **`atlas_surface`**,
référence de dessin `atlas_draw(state, props, frame)`. Écran de HUD `atlas_hud()`,
racine `atlas_hud_root`, Canvas `atlas_hud_surface`. Repère de dessin
**1920 × 1080** ; mise à l’échelle par le renderer, sans HTML.

L’histoire initialise et possède ces variables :

| Variables | Contrat |
| --- | --- |
| `first_name`, `last_name` | Chaînes modifiables, 24 caractères maximum chacune |
| `origin` | `rivage`, `hautes_terres` ou `brumes` ; labels affichés séparément |
| `atlas_page`, `atlas_view` | Page 0/1/2 ; position animée numérique, initialement 1/1.0 |
| `atlas_selected`, `atlas_marker` | Indices d’objet 0–17 et de lieu 0–5 |
| `atlas_inventory` | Identifiants stables, dans l’ordre des 18 emplacements ci-dessous |
| `atlas_quantities` | Liste de 18 entiers ≥0 ; zéro = non possédé |
| `atlas_equipped` | ID stable de l’objet équipé, initialement `boussole` |
| `atlas_health`, `atlas_max_health`, `atlas_gold` | Vitalité, maximum et éclats |
| `atlas_location`, `atlas_destination` | Lieu courant ; demande de voyage, vide si annulée |
| `atlas_open`, `atlas_menu_enabled` | Visibilité du menu et autorisation narrative d’ouverture |
| `atlas_notice`, `atlas_quest` | Retour d’action et objectif courant |
| `atlas_melody_played` | Vrai après utilisation d’un sifflet possédé |
| `atlas_reduced_motion`, `atlas_high_contrast`, `atlas_self_voicing` | Préférences booléennes, initialement fausses |
| `atlas_text_scale` | Taille relative du texte ; curseur de démo 0.85–1.25, initialement 1.0 |

Le dessin de l’Atlas reçoit ces valeurs dans `props`. L’état par instance du
Canvas contient seulement `hover`, l’indice du marqueur survolé, ou -1. Le temps
de transition est fourni par les ticks de simulation bornés du moteur ; aucune
horloge système n’est lue. Variables et état local suivent sauvegarde/chargement
et retour arrière.

### Emplacements et routes

| Indice | Objet | Indice | Objet | Indice | Objet |
| --- | --- | --- | --- | --- | --- |
| 0 | `boussole` | 6 | `potion` | 12 | `relique` |
| 1 | `lanterne` | 7 | `cle` | 13 | `cristal` |
| 2 | `grappin` | 8 | `corde` | 14 | `fiole` |
| 3 | `carnet` | 9 | `monocle` | 15 | `plume` |
| 4 | `sceau` | 10 | `ocarina` (sifflet) | 16 | `coquillage` |
| 5 | `carte` | 11 | `amulette` | 17 | `fragment` |

Départ : boussole, carnet, carte, trois élixirs, corde, plume et conque.
Les autres objets sont gagnés au cours de l’aventure ; changer un ID ou l’ordre
de la liste demande de mettre à jour le contrat, pas seulement une icône.

| Indice | Destination | Condition d’accès |
| --- | --- | --- |
| 0 | `havre` | Ouverte |
| 1 | `observatoire` | Lanterne possédée, quantité index 1 >0 |
| 2 | `clairiere` | Ouverte |
| 3 | `archives` | Sceau possédé, index 4 >0 |
| 4 | `falaises` | Grappin possédé, index 2 >0 |
| 5 | `tour` | Clé index 7 et cristal index 13 possédés |

Ces gardes sont vérifiées dans **les deux** chemins d’activation, bouton et
clavier. L’histoire peut en outre demander un objet équipé pour une rencontre.

### Opérations publiques

`atlas_tab` reçoit `event["data"]["page"]` ; `atlas_step` reçoit `delta` -1/+1.
`atlas_key` traite le clavier du Canvas. Les boutons natifs ont leur propre
gestionnaire clavier pour éviter une double activation par Entrée.
`atlas_confirm_item` applique `atlas_item_result` ; `atlas_confirm_travel` valide
les possessions, écrit destination/lieu et ferme le menu. Aucun gestionnaire
ne force un `jump` narratif.

Le gestionnaire `atlas_accessibility(event)` appelle :

```rvn
accessibility.configure({
    "reduced_motion":atlas_reduced_motion,
    "high_contrast":atlas_high_contrast,
    "text_scale":atlas_text_scale,
    "self_voicing":atlas_self_voicing
})
```

Les préférences explicites du joueur restent prioritaires selon le contrat
du moteur. Le label narratif de même nom dans la source applique
ces réglages après les scènes ; un handler n’est pas une fonction de calcul.

L’exploration attend la durée de vie du menu avec les opérations existantes :

```rvn
ui.open("atlas",[],true,20)
motion.play("ui:atlas/atlas_root",motion_pause(86400))
motion.wait("ui:atlas/atlas_root")
ui.close("atlas")
// Ici, l’histoire branche sur atlas_destination, sans clic supplémentaire.
```

La fermeture retire la cible d’animation ; l’horloge suivante libère l’attente.
Ce garde-fou est fini : 24 heures de temps de simulation, pas une attente infinie.
Un menu secondaire au-dessus ou une pause du jeu suspend le temps selon les
règles du runtime.

### Ressources

Chemins relatifs au **dossier d’assets**, jamais avec un second préfixe `assets/` :
`ui/frame.png`, `maps/world.png`, `portraits/player.png` et
`icons/{boussole,lanterne,grappin,sceau,cle,cristal}.png`. Les autres pictogrammes
sont dessinés en RVN. Une image manquante est une erreur visible, pas un
remplacement silencieux. Le monde est affiché en 1044 ×522, au ratio du PNG,
et les positions de ses six marqueurs utilisent le repère nominal 1020 ×530.

## Portée et validation

La démo est conçue en français. Les traductions anglaises disponibles ne
constituent pas une traduction exhaustive des dessins Canvas. Les textes de
Canvas sont explicitement produits en RVN ; ils ne reçoivent pas une traduction
automatique en inventant une clé de locale.

Les contrôles automatiques indépendants sont conservés dans `qa/` dans le dépôt
source du moteur, pas dans le paquet installé. Ils utilisent le vrai analyseur,
les vrais graphes et le vrai moteur, avec un renderer sans fenêtre. Ils ne
prouvent pas à eux seuls les pixels, le focus clavier natif, le son ou le
décodage vidéo : les captures et vérifications d’applications doivent rester
des preuves séparées, propres au paquet effectivement exécuté.
Aucune validation Windows n’est revendiquée sans exécution Windows réelle.

L’Atlas illustre les capacités actuelles et leurs limites : primitives 2D,
état sauvegardé, événements bornés et contrôles accessibles. Ce n’est pas une
compatibilité avec Ren’Py, un moteur 3D, ni un système de shaders arbitraires.
