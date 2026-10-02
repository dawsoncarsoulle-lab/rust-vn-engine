# Reusable interfaces and visual authoring / Interfaces réutilisables et édition visuelle

## English

Screens, components, styles and event handlers remain RVN expressions, not an
editor-only data format. These additional UI properties have defaults when
reading older descriptions; the graph/save migrations for other features are
documented separately. The source-first visual designer edits the same component
tree as the Blueprint graph; a calculated property or generated list stays a
connected expression. It is never replaced by its current preview value.

For a control whose actual 2D drawing and pointer/key behavior are programmable,
use the [Canvas contract](programmable-components.md) and its independent saved
instance state. Existing controls remain usable alongside it.

```rvn
function card_style() {
    return {"padding": 12, "radius": 14, "border_width": 2,
            "border_color": [0.1, 0.6, 0.9, 1],
            "hover_background": [0.2, 0.4, 0.6, 1]}
}
function item_card(item) {
    return component(item["id"], "button", {
        "text": item["name"], "style": [card_style(), {"font_size": 28}],
        "event_data": item, "events": {"click": "select_item"}
    }, [])
}
handler select_item(event) { set selected = event["data"]["id"] }
```

`style` accepts one dictionary or a list of up to 32 dictionaries. Later
dictionaries override earlier ones; properties written directly on the
component override every style. Styles cannot define identity, children,
bindings, content or handlers. Use regular reusable functions, parameters,
`dict_set`, `if` and bounded loops for those. `event_data` contains game-domain
data for reused components; handlers receive it as `event["data"]`. It defaults
to an empty dictionary and must contain RVN values (no JSON null).

Shared visual properties:

| Group | Properties |
| --- | --- |
| Size and position | `rect`, `width`, `height`, `min_width`, `min_height`, `max_width`, `max_height` |
| Containers | `padding`, `spacing`, `margin`, `align`, `justify`, `wrap`, `clip`, `scroll` |
| Text | `font`, `font_size`, `foreground`, `text_align` |
| Material | `background`, `opacity`, `border_width`, `border_color`, `radius`, `auto_background` |
| Interaction feedback | `hover_background`, `pressed_background`, `focus_color` |

Colors are RGBA lists with four numbers from 0 to 1. `margin` is a four-number
list in top/right/bottom/left order. `align` is `start`, `center`, `end` or
`stretch`; `justify` adds `space_between`, `space_around`, `space_evenly` to
`start`, `center`, `end`. `text_align` is `left`, `center` or `right`.

Geometry uses the existing 1920×1080 reference. A nested `rect` is relative to
its parent's outer top-left, not to the screen. The shared layout routine
returns absolute rectangles for the designer and both desktop/Web renderers.
Text has a deterministic intrinsic box; use explicit width and height for
exact composition independent of fonts. Hidden children occupy no layout
space; overflow remains scrollable when a container enables `scroll`.
`opacity` multiplies down the component tree. `auto_background: false` allows
a truly transparent control instead of the backward-compatible default fill.
`font` names a project-relative TTF/OTF asset, inherited by children unless they
choose another font. Browser text fields use the same loaded font as the
canvas; missing fonts are reported. Keep the licenses for fonts you distribute.
Authored text sizes follow the reference-to-window scale; small text is not
silently enlarged to 14 pixels. A minimum of one rendered pixel prevents
degenerate glyphs, and the player's accessibility text scale still applies.

The pinned Bevy 0.14.2 UI shader is corrected at initialization for its reversed
antialias `clamp` arguments. Without this correction WebGL2 could paint a
border over the entire background. Only the known shader expression changes;
an unrecognized dependency shader requires verification instead of silently
certifying its appearance. Existing Bevy license notices remain applicable.

The familiar defaults remain: controls are at least 180×44 reference pixels,
have a 6-pixel radius, and show a visible keyboard focus outline. Do not erase
focus feedback when designing custom controls. Component IDs must remain
unique and stable, including components created by reusable functions.

There are at most 512 components, 32 nesting levels and 32 simultaneously open
screens. Style names are checked, dimensions/colors are validated, and event
data has a 64-KiB budget and 16 nesting levels. Event evaluation retains the
existing instruction/call limits and transactional save/rollback semantics.
These extensions do not introduce Python, arbitrary render plugins, or promise
that every Ren'Py custom displayable can be expressed.

### Using Designer and Graph

Open **Interfaces** in the editor, then use **Screens** to choose a reusable
screen. Apply local edits before switching screens. If the project has no
screen yet, Interfaces creates a screen with
a root panel. **Parameters** changes the preview arguments, not the function's
signature or the saved game. A parameterized reusable component or style asks
for each RVN argument when inserted; arguments remain real graph expressions.

Search the palette and click a component to add it inside the selected
panel/row/column (or the root if a leaf is selected). Select via the hierarchy
or the preview. Drag to position an authored component; drag its bottom-right
corner to resize. The automatic-layout property returns placement to its
container. Nested coordinates remain relative to the parent. **Reparent**
chooses another authored container; **Ctrl+D** duplicates the selection with
new IDs, retaining reusable functions and styles.

The inspector edits text, dimensions, layout, appearance, accessibility and
interaction feedback. The color palette includes presets and custom
`#RRGGBB` or `#RRGGBBAA` values. Image/font assets,
variables, events and shared styles use searchable choices instead of requiring
resource paths or handler names to be memorized. Toggle properties directly;
use Enter or Tab to commit a text field and Escape to cancel it. The inspector
can scroll to additional properties. Event choices refer to RVN handlers;
bindings refer to game variables, not temporary preview fields.

**Graph** applies the working Designer edit and opens the same Blueprint
logic. Use calculation nodes for conditions, loops, collections and generated
components, and handler graphs for events. A computed component/property may
be previewed without being directly editable: Designer diagnoses unsupported
direct edits rather than flattening its function or list into sampled data.
Edit that connected expression in Graph. The existing Game menus workspace
continues to edit the built-in title/pause/save menus separately.

**Undo / Redo** (Ctrl+Z / Ctrl+Shift+Z or Ctrl+Y) restores the working tree;
one drag is one edit. **Apply** or Ctrl+S validates and applies it to the
project's graph. Save the project afterward to update `.rvn` through its
source-preserving transaction. **Close** asks before discarding a changed
working copy. If the underlying graph changed meanwhile, application refuses
to overwrite it and requests conflict resolution. Invalid layout, expressions
or resources stay visible as errors; they do not replace the last valid tree.

## Français

Écrans, composants, styles et gestionnaires restent des expressions RVN. Le
concepteur visuel et les Blueprints modifient la même logique ; une propriété
calculée, un composant réutilisable ou une liste produite par une boucle reste
une expression connectée. L'aperçu ne remplace jamais le code par sa valeur du
moment. Ces propriétés UI supplémentaires ont des valeurs par défaut lors de
la lecture d'anciennes descriptions ; les migrations de graphe et sauvegarde
liées aux autres fonctionnalités sont documentées séparément.

Pour programmer le dessin 2D et les interactions d’un contrôle, consultez le
[contrat Canvas](programmable-components.md) : état indépendant sauvegardé,
primitives et événements. Les contrôles existants restent disponibles à côté.

`style` accepte un dictionnaire ou une liste de 32 dictionnaires maximum,
appliqués de gauche à droite. Les propriétés inscrites directement sur le
composant sont prioritaires. Un style ne change que l'apparence et la
disposition, pas les identifiants, événements, liaisons ou enfants. Les
fonctions RVN réutilisables, paramètres, `dict_set`, conditions et boucles
bornées construisent ces éléments de logique. `event_data` transmet un
dictionnaire au gestionnaire, accessible par `event["data"]` ; sa valeur par
défaut est un dictionnaire vide.

Le tableau ci-dessus donne les propriétés communes. Couleurs : listes RGBA
de quatre nombres entre 0 et 1. Marges : haut, droite, bas, gauche. `align`
accepte `start`, `center`, `end`, `stretch`. `justify` accepte `start`, `center`,
`end`, `space_between`, `space_around`, `space_evenly`. `text_align` accepte
`left`, `center`, `right`.

La référence reste 1920×1080. Le rectangle d'un enfant est relatif au coin
supérieur gauche extérieur de son parent. Le concepteur et les rendus desktop
et Web utilisent le même calcul de rectangles. La taille intrinsèque du texte
est déterministe ; donnez une largeur et une hauteur pour une composition
précise indépendante de la police. Un enfant masqué ne prend pas de place.
Les conteneurs avec `scroll` conservent leurs contenus débordants accessibles.
L'opacité se multiplie de parent en enfant. `auto_background: false` autorise
un contrôle réellement transparent au lieu du remplissage historique.
`font` désigne une ressource TTF/OTF du projet, héritée par les enfants qui ne
choisissent pas leur propre police. Les champs du navigateur utilisent la
même police chargée que le canvas. Une police absente produit une erreur ;
conservez les licences des polices distribuées avec votre jeu.
La taille du texte suit l’échelle référence/fenêtre : les petits textes ne sont
pas agrandis silencieusement à 14 pixels. Le minimum est un pixel de rendu pour
éviter les glyphes dégénérés ; l’échelle d’accessibilité du joueur reste appliquée.

Le shader UI de la version utilisée, Bevy 0.14.2, est corrigé au démarrage pour
ses arguments `clamp` inversés dans l'anticrénelage. Sans cette correction,
WebGL2 pouvait remplir tout le fond avec la couleur de bordure. Seule
l'expression connue est changée ; un shader de dépendance non reconnu exige
une vérification, sans certification silencieuse du rendu. Les notices de
licence Bevy existantes restent applicables.

Les valeurs par défaut sont conservées : taille minimale des contrôles 180×44,
rayon 6, focus clavier visible. Gardez des identifiants uniques et stables,
y compris dans les composants créés par des fonctions. Les styles inconnus,
couleurs ou dimensions invalides produisent une erreur. Limites : 512
composants, 32 niveaux, 32 écrans ouverts ; données d'événement 64 Kio et 16
niveaux. Sauvegarde, chargement et retour arrière conservent les règles
transactionnelles existantes. Ces ajouts n'intègrent pas Python et ne
promettent pas de reproduire tout composant graphique personnalisé de Ren'Py.

### Utiliser le Designer et le Graphe

Ouvrez **Interfaces** puis choisissez un écran avec **Écrans**. Appliquez les
modifications locales avant de changer d’écran. Sans écran, cet espace en crée
un avec un panneau racine. **Paramètres** règle les arguments
de l’aperçu, sans modifier la signature ni l’état du jeu. **Réutiliser** insère
une fonction de composant et demande ses arguments RVN ; un style réutilisable
paramétré fonctionne de même. Ces arguments restent des expressions connectées.

Recherchez un contrôle dans la palette et cliquez pour l’ajouter au conteneur
sélectionné, sinon à la racine. Sélectionnez-le dans la hiérarchie ou l’aperçu,
glissez-le pour le placer et glissez son coin inférieur droit pour le
redimensionner. **Disposition automatique** confie sa position au conteneur.
**Imbriquer** change de parent ; Ctrl+D duplique avec de nouveaux identifiants.
Les fonctions et styles réutilisés restent partagés.

L’inspecteur règle apparence, disposition, accessibilité et événements.
Les couleurs acceptent `#RRGGBB` ou `#RRGGBBAA`. Images, polices, variables,
gestionnaires et styles se choisissent dans des listes avec recherche.
Entrée/Tab valide une saisie, Échap l’annule ; faites défiler pour les propriétés
suivantes. **Graphe** applique la copie de travail et ouvre les Blueprints de
cette même interface : conditions, boucles, collections, composants calculés
et gestionnaires. Un composant calculé reste calculé ; une édition directe
indisponible produit un diagnostic, jamais une conversion en valeur d’aperçu.
L’espace historique **Menus du jeu** reste distinct.

Ctrl+Z annule, Ctrl+Maj+Z ou Ctrl+Y rétablit. Un glissement est une seule action.
**Appliquer** ou Ctrl+S valide et applique au graphe ; enregistrez ensuite le
projet pour mettre à jour le RVN en conservant les commentaires et le code
inchangé. **Fermer** demande avant d’abandonner une copie modifiée. Un graphe
modifié entre-temps bloque l’application plutôt que l’écraser. Les erreurs
restent visibles sans remplacer la dernière arborescence valide.
