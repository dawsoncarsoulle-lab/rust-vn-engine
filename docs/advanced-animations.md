# Advanced animation authoring / Animations avancées

The animation sampler is shared by the editor, native runtime and Web runtime.
Old preset curves and saved tween descriptions remain readable.

Le même échantillonneur anime l’éditeur, le moteur natif et le Web.
Les anciennes courbes prédéfinies et sauvegardes restent lisibles.

## RVN

```rvn
function smooth(t) {return t*t*(3-2*t)}
function flight(seconds) {
    return motion_parallel([
        motion_spline(seconds, [
            {"x":0,"y":0}, {"x":250,"y":-150}, {"x":500,"y":0}
        ], motion_curve("smooth",65)),
        motion_tween(seconds,{}, {"rotation":30},motion_bezier(0.25,0.1,0.75,0.9))
    ])
}
label start
scene "background.png"
motion.play("background",flight(3))
"Moving"
motion.wait("background")
```

`motion_spline(seconds, points, curve)` passes through each x/y point with
Catmull–Rom interpolation. Coordinates are offsets in the existing 1920×1080
reference pixels; timing is uniform between successive control points, not
constant-distance speed. Combine a spline with opacity/rotation/etc. using
`motion_parallel`; another branch must not also write x/y.

`motion_bezier(x1,y1,x2,y2)` defines a cubic Bézier timing curve. Its endpoints
are (0,0) and (1,1). All controls are finite and in [0,1]; crossed x controls
are supported. It can replace any preset curve in a tween or spline.

`motion_curve("function", samples)` calls a one-parameter RVN calculation
function at uniformly spaced times between 0 and 1. It returns finite progress
in [0,1], starts at 0 and ends at 1. Non-monotonic progress is supported.
The 2–257 samples are linearly interpolated; choose 65 for a small smooth
curve, or 257 for finer detail. This is a sampled function, not an arbitrary
per-frame callback. No Python is involved. Narrative operations and randomness
are forbidden; the ordinary shared operation/call-depth budgets remain active
across the entire sampling operation. Invalid curves fail before playback.

Splines allow 2–256 points in ±500000 reference pixels; direct visual lists
currently allow up to 128 editable points. Longer or computed paths can be
created through calculation/collection Blueprints. Duration and nesting limits
remain those of ordinary composable animations. Samples and path points are
saved with each running track, so seeking, save/load and rollback require no
function re-evaluation and produce the same pose.

`motion_spline(durée, points, courbe)` traverse chaque point x/y avec une
interpolation Catmull–Rom. Les coordonnées sont des décalages en pixels de
référence 1920×1080. Le temps est réparti entre les points, pas selon la distance.
Une branche parallèle peut animer rotation/opacité/etc., mais pas réécrire x/y.

`motion_bezier(x1,y1,x2,y2)` personnalise le rythme avec des contrôles dans
[0,1], entre (0,0) et (1,1). Les contrôles x croisés sont acceptés.

`motion_curve("fonction", échantillons)` évalue une fonction de calcul RVN à
un paramètre, de 0 à 1. Sa progression finie reste dans [0,1], commence à 0 et
termine à 1 ; elle peut avancer puis reculer. Ses 2–257 échantillons sont
interpolés linéairement : 65 convient aux courbes simples, 257 affine les détails.
Ce n’est pas un rappel arbitraire à chaque image. Ni Python, ni opération
narrative, ni hasard ; les budgets de calcul et de profondeur habituels restent
partagés sur toute l’évaluation. Une courbe invalide est rejetée avant lecture.

Les splines acceptent 2–256 points dans ±500000 pixels ; les listes directement
éditables visuellement sont limitées à 128 points. Les chemins calculés ou plus
longs restent réalisables dans les graphes de calcul/collections. Points,
échantillons et horloges sont sauvegardés dans la piste : chargement, retour
arrière et déplacement temporel n’appellent pas de nouveau la fonction.

## Blueprints and preview / Blueprints et aperçu

**Spline trajectory**, **Bézier curve** and **Custom curve** are real pure
function nodes with the approved green Unreal-inspired material. Numeric pins
remain numeric. The Curve input is a wildcard because it accepts an existing
preset string or a curve descriptor dictionary, never a dictionary disguised
as String. The custom-function inspector offers only one-parameter functions.

Select a Spline node and **Edit trajectory**. Drag a point; **+ Point** appends,
**− Point** or Delete removes (at least two remain). Ctrl+arrows moves a point
by 10 reference pixels, or 1 with Shift. One drag is one undoable edit. Shared
lists/points are copied on the edited connection; computed coordinates are
diagnosed and preserved. Comments, unrelated source and existing graph
placements are retained when saved.

Open preview from **Play animation** for the actual scene before that command:
background, visible sprites/compositions, open interfaces and existing poses.
The isolated core resolves calculations, calls and conditions. When reaching a
choice or imagemap, it asks for the route explicitly; preview never guesses,
plays sound, writes saves or changes the real game. Unreachable scenes, missing
resources or unsupported video-dependent contexts produce a diagnostic. A
standalone animation factory retains its explicitly labelled isolated preview.
Space plays/pauses; the timeline seeks. This is an authoring preview, not a
replacement for testing the exported renderer (text rotation and advanced
control feedback remain runtime-specific).

**Trajectoire spline**, **Courbe Bézier** et **Courbe personnalisée** sont de
vrais nœuds de calcul, avec le matériau vert validé. Les pins numériques gardent
leur type. Le pin Courbe est générique car il accepte une chaîne prédéfinie ou
un dictionnaire de courbe, sans faire passer ce dernier pour une chaîne.
L’inspecteur de fonction ne propose que les fonctions à un paramètre.

Dans **Modifier la trajectoire**, glissez les points, ajoutez/retirez avec les
boutons, ou déplacez avec Ctrl+flèches (10 pixels ; 1 avec Maj). Un glissement
correspond à une seule annulation. Les producteurs partagés sont copiés sur la
connexion éditée ; les coordonnées calculées sont conservées et signalées.
Ctrl+Z annule dans l’aperçu ; Ctrl+Maj+Z ou Ctrl+Y rétablit. Le paramètre
d’aperçu `seconds`/`duration` reçoit 1 si sa valeur par défaut est zéro ; cela
ne modifie ni la fonction RVN ni ses arguments dans le récit.

Depuis **Lire une animation**, l’aperçu restitue la scène avant cette commande :
fond, personnages/compositions visibles, interfaces ouvertes et poses actives.
Le moteur isolé résout calculs, appels et conditions, puis demande explicitement
le chemin des choix/imagemaps. Il ne joue pas de son, n’écrit pas de sauvegarde
et ne modifie jamais le jeu. Ressource absente, scène inaccessible ou contexte
dépendant d’une vidéo non simulée : diagnostic explicite. Une fabrique isolée
garde son aperçu marqué comme tel. L’aperçu n’exempte pas de tester le rendu
exporté ; rotation du texte et retours visuels avancés des contrôles sont propres
au moteur de rendu du jeu.

## Validation

The focused suites are `rvn_ui` motion tests (legacy presets, curves, spline
sampling and invalid descriptors), `rvn_core/tests/advanced_motions.rs` and
`tests/motions.rs` (bounded pure callbacks, randomness rejection, saved sampled
curves and mid-motion save/load/rollback), and the equivalent `rvn_graph`
suites (dedicated nodes, repeated import/export, shared producers, comments,
stable identities and positions). The editor tests isolated parameters and
real preceding scene state, including an explicit branching route. A Bevy
test checks the exact sampled spline-to-transform mapping for sprites and UI.

Native release QA on Linux additionally exercised visible point handles,
dragging, add/remove, undo/redo in the preview and saving back to RVN. The
scene preview was visually checked at 0 and 1 second: its actual background,
sprite and open screen remained present, and seeking moved only the selected
sprite along its authored spline. These checks do not certify an exported
Windows package, Firefox or full Ren’Py ATL compatibility.

Les suites ciblées vérifient ancien format, courbes/splines, fonctions bornées,
rejet du hasard, sauvegarde/chargement/rollback en mouvement et conversion
visuelle sans perte des commentaires, identifiants et positions. Les tests de
l’éditeur couvrent les paramètres isolés, le contexte réel et les choix
explicites ; un test Bevy vérifie les transformations desktop et d’interface.
La QA native Linux vérifie en plus les points visibles et déplaçables,
ajout/retrait, annuler/rétablir, sauvegarde RVN et déplacement temporel de la
bonne cible dans une scène avec fond, personnage et interface réellement
ouverts. Elle ne vaut pas validation du paquet Windows ou de Firefox, ni
promesse d’équivalence avec toute la portée d’ATL.
