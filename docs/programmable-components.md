# Composants 2D programmables / Programmable 2D components

## Français

Un **Canvas** complète les contrôles existants : une fonction RVN produit son
dessin, des gestionnaires modifient son état local. Le moteur reste en Rust.
Aucun Python, bibliothèque système arbitraire ou compatibilité Ren’Py.

```rvn
function draw_counter(state, props, frame) {
    return [canvas_rect([0,0,frame["width"],frame["height"]],props["color"],12),
            canvas_text(""+state["count"],[20,20],[1,1,1,1],32)]
}
handler increment(event) {
    ui.set_state(event["screen"],event["element"],
                 dict_set(event["state"],"count",event["state"]["count"]+1))
}
screen counter() {
    return component("counter","canvas",{
        "width":320,"height":180,"draw":"draw_counter",
        "props":{"color":[0.1,0.3,0.5,1]},"state":{"count":0},
        "accessible_label":"Counter","events":{"pointer_down":"increment"}
    },[])
}
```

`draw(state,props,frame)` prend exactement trois paramètres. State est
sauvegardé par instance, props est un dictionnaire de paramètres calculés,
frame fournit width/height et time en secondes de simulation. La fonction
retourne des primitives ; fonction absente ou liste vide signifie dessin vide.
Canvas est un composant feuille, sans enfants.

Le dessin est **pur** : aucune modification de variables globales/interfaces,
aucune consommation aléatoire ou lecture directe souris/clavier. Utilisez un
gestionnaire et l’état sauvegardé. Les références draw littérales inconnues,
mal paramétrées ou impures sont diagnostiquées au contrôle du projet ; les
références calculées sont vérifiées à l’exécution. Une erreur est atomique.

### Primitives

| Fonction | Paramètres |
| --- | --- |
| `canvas_rect` | rectangle, couleur, rayon |
| `canvas_ellipse` | rectangle, couleur |
| `canvas_line` | points `[x,y]`, couleur, épaisseur |
| `canvas_polygon` | points, couleur ; polygone simple, concave accepté |
| `canvas_text` | texte, position `[x,y]`, couleur, taille |
| `canvas_image` | ressource, rectangle |
| `canvas_group` | transformation, découpage, primitives |
| `canvas_hit` | identifiant de zone, rectangle |

Coordonnées locales, origine en haut à gauche, unités de référence 1920 × 1080.
Rectangle `[x,y,width,height]`, couleur `[r,g,b,a]` entre 0 et 1. Transformation
`[x,y,scale_x,scale_y,rotation_degrees,opacity]`. Découpage `[]` ou rectangle local.
Transformations, opacités et découpages imbriqués se composent et servent aussi
à tester les zones Hit. Le bord du Canvas et les découpages des parents limitent
le dessin. Hit est invisible ; la dernière zone visible valide gagne.

Le texte suit la police héritée, accepte `\n`, sans minimum de taille imposé.
Traduisez dans votre logique ou via des composants Text localisés : une
primitive ne reçoit pas implicitement une clé de localisation. Images relatives
au **dossier d’assets** : `"emblem.png"`, pas un second préfixe assets. Utilisez
PNG/JPEG/WebP pris en charge ; rasterisez les SVG sources avant export. Les
ressources absentes déclenchent un diagnostic, pas un remplacement silencieux.

### Événements, état et édition

Événements : pointer_down/move/up/cancel, wheel, tick, key. Le gestionnaire reçoit
screen/element/kind/value/key/data, plus state/props/frame. Value du pointeur
contient x/y locaux, pointer/button/captured/modifiers(ctrl/shift/alt/meta), dx/dy
de molette et hit lorsqu’une zone est touchée. Button vaut notamment left/right/
middle/touch. Value clavier contient pressed/code/modifiers : testez pressed
pour différencier appui et relâchement. Tick fournit dt ; frame.time est avancé.

`ui.set_state(screen,element,dictionary)` remplace cette seule instance, dans
un gestionnaire ou la narration. `dict_set` conserve les autres clés. Gardez
des IDs stables et uniques ; fermer détruit les instances, rouvrir reprend les
états initiaux. Capture_pointer et consume_input sont vrais par défaut : un
glissement continue hors Canvas sans avancer le dialogue. La capture s’annule
si le contrôle devient indisponible ou la fenêtre perd le focus. Chargement et
rollback invalident les captures sans événement ancien vers l’état restauré.
Fournissez labels accessibles, focus et raccourcis : dessiner un bouton ne lui
donne pas automatiquement une sémantique accessible.
Tab reste la navigation d’interface ; F5/F6 et Échap restent les raccourcis du
jeu sur le Canvas. Un gestionnaire clavier global d’écran peut les remplacer
explicitement. Le focus accessible Web du Canvas ne doit pas voler le focus
graphique pendant un glissement.

Ticks : temps de simulation sauvegardé, pas horloge système, dt ≤ 0,25 s. Le
runtime suspend la livraison en pause, défocalisé ou dans le menu sauvegarde ;
les Canvas masqués/couverts par une interface modale supérieure ne progressent
pas. Aucune entrée de rollback par frame.

Dans **Interfaces**, choisissez **Composant personnalisé** (Canvas) et sa fonction,
puis **Graphe de dessin → Ouvrir le dessin**.
Les trois paramètres, huit primitives et commande d’état restent de vrais
nœuds/pins. Props/état initial sont des dictionnaires connectés. Une expression
calculée ne devient pas un instantané de l’aperçu. L’aperçu ne joue pas le jeu
et n’exécute pas les gestionnaires. **Appliquer** met à jour Graph ;
**Enregistrer** écrit RVN. Vérifiez les interactions avec Jouer et l’export.

### Protection et compatibilité

32 Canvas ouverts, 1 024 primitives/dessin, 4 096/évaluation complète, profondeur
16, 256 points/ligne ou polygone ; budget partagé 100 000 étapes et 64 appels.
Coordonnées/tailles bornées ; NaN, couleurs invalides, polygones intersectés et
chemins hors projet refusés. État : dictionnaire ≤128 clés, profondeur16, 64Kio.
Ces limites protègent le jeu, sans promettre des performances identiques partout.
Texte : masque ≤4096 pixels par axe/16 mégapixels et somme des surfaces de
glyphes ≤64 × 1024 × 1024 pixels. L’aperçu CPU a un budget de rasterisation
supplémentaire ; un dessin trop coûteux affiche une erreur, il n’est pas tronqué.

Sauvegardes **9** : états/temps conservés ; formats antérieurs lisibles hydratés
par défaut si données absentes. GPU/dessin/capture non sauvegardés, dessin
reconstruit. Graphe **5** : migration protégée par sauvegarde de sécurité.
Compatibilité après modification de l’histoire toujours vérifiée.

[custom-components](../examples/custom-components/README.md) montre deux curseurs
indépendants et toutes les primitives. Pas de shaders arbitraires, 3D, plugins
natifs ou parité totale des displayables Ren’Py. Windows exige un test réel.

## English

A **Canvas** adds a pure RVN drawing function and saved instance-local state to
existing controls. Engine/renderers remain Rust. No Python, arbitrary system
libraries or Ren’Py script compatibility. The code and primitive table above
apply in both languages.

`draw(state,props,frame)` takes exactly three parameters and returns primitives.
State is saved per instance, props is calculated, frame supplies width/height
and simulation time. No function or an empty list means empty drawing; Canvas
has no children. Drawing cannot mutate globals/interfaces/RNG or query live
input. Use handlers and saved state. Literal callback references are checked
for name, arity and transitive purity; calculated references are runtime-checked.
Errors never publish a partially changed state.

Local geometry has top-left origin in 1920 × 1080 reference units. Rectangles
are `[x,y,width,height]`, RGBA colors 0–1. Groups take
`[x,y,scale_x,scale_y,rotation_degrees,opacity]`, then `[]` or a local clip.
Nested transforms/clips affect drawing and invisible Hit regions; last visible
overlapping Hit wins. Canvas/ancestor clipping applies. Simple concave polygons
work; intersecting ones fail. Text uses inherited fonts, explicit `\n`, authored
size without a readability floor. Localize in your logic or ordinary Text
controls; primitives do not invent locale keys. Image paths are relative to
the **asset directory**, e.g. `"emblem.png"`. Use PNG/JPEG/WebP; rasterize SVG
before export. Missing resources fail visibly.

Events are pointer_down/move/up/cancel, wheel, tick, key. Alongside screen,
element, kind, value, key, data, handlers receive state/props/frame. Pointer
value contains local x/y, pointer/button/captured/modifiers, wheel dx/dy and hit
when applicable. Keyboard value contains pressed/code/modifiers; inspect
pressed for key releases. Tick has dt and frame.time is already advanced.
`ui.set_state(screen,element,dictionary)` replaces one instance; `dict_set`
retains other keys. IDs must stay stable/unique. Closing discards instances;
reopening uses authored defaults. Capture/consumption default true. Unavailable
controls or window-focus loss cancel capture. Load/rollback invalidate captures
without stale events. Supply labels, focus and keyboard handlers: a drawn
button is not automatically accessible.
Tab remains interface navigation; F5/F6 and Escape remain game shortcuts on
the Canvas. A global screen key handler may explicitly override them. Web
Canvas accessibility focus must not steal graphical focus during a drag.

Ticks use saved simulation time with dt ≤ 0.25 s, not wall clock. Runtime pauses
delivery while unfocused, paused or in save menus; hidden/modal-covered Canvases
do not advance. No rollback-history entry is created per frame.

In **Interfaces**, choose **Custom component** (Canvas)/function, then
**Drawing graph → Open drawing**. Parameters,
primitives and state command remain Graph nodes/pins; props/initial state stay
connected dictionaries. Computed values are never frozen into preview data.
Preview does not run handlers. Apply updates Graph, Save writes RVN. Test actual
interactions in Play and standalone exports.

Safety limits: 32 Canvases, 1,024 primitives/drawing, 4,096/evaluation, depth16,
256 points/line/polygon, shared100,000 steps and64calls. Bounded coordinates/
sizes; invalid numbers/colors, intersecting polygons and escaping paths fail.
State: dictionary ≤128keys, depth16, 64KiB. Not identical-performance guarantees.
Text masks: ≤4096 pixels/axis and16megapixels; total glyph bounding-box area
≤64×1024×1024 pixels. CPU preview has an additional raster work budget;
expensive drawings produce an explicit error rather than being truncated.
Save **9** retains state/clocks; readable older formats hydrate absent defaults.
GPU/drawing/capture are not saved; drawing is rebuilt. Graph **5** migrates with
backups. Edited-story compatibility remains checked. The
[example](../examples/custom-components/README.md) includes original SVG+PNG.
Not arbitrary shaders, 3D/native plugins or full Ren’Py parity. Actual Windows
is required for Windows validation.
