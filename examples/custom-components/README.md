# Composants personnalisés / Custom components

## Français

Deux instances d’un curseur dessiné et piloté en RVN. Déplacez leur poignée,
utilisez la molette, ou sélectionnez un curseur et utilisez les flèches,
Début/Fin. **Tab** change de contrôle. **F5** sauvegarde rapidement et **F6**
demande confirmation avant le chargement. Le chargement et le retour arrière
restaurent les états locaux ; aucune primitive GPU n’est enregistrée.

Le champ Note utilise le contrôle de saisie standard. Essayez un texte Unicode,
déplacez le curseur de saisie puis **Maj+Tab** pour retourner au second Canvas.

Le dessin montre rectangles, ellipse, ligne, polygone concave, texte, image,
transformation et découpage imbriqués. Le groupe rotatif est volontairement
découpé : l’emblème ne dépasse pas son rectangle. Le PNG runtime est généré
depuis le SVG source original ; aucun asset tiers n’est requis.

Dans le Designer, sélectionnez le Canvas puis sa fonction de dessin ; le Graphe
édite les primitives et les gestionnaires. Les paramètres `state`, `props`,
`frame` restent des pins, pas du code caché. Les valeurs calculées ne doivent pas
être remplacées par des instantanés pendant une modification visuelle.

Voir [le contrat bilingue](../../docs/programmable-components.md). Ce projet ne
promet ni shaders arbitraires, ni 3D, ni compatibilité Ren’Py/Python.

## English

Two instances of an RVN-drawn interactive slider. Drag a handle, scroll, or focus
a slider and use arrows, Home/End. **Tab** switches controls. **F5** quicksaves;
**F6** asks before loading. Loading and rollback restore local state without
serializing GPU resources or calculated drawing commands.

The Note field is a standard input. Try Unicode text, move its caret, then use
**Shift+Tab** to return to the second Canvas.

The drawing demonstrates rectangles, ellipse, line, concave polygon, text,
image, nested transforms and clipping. The rotated group deliberately clips
the emblem. The runtime PNG is rasterized from the original SVG source and
requires no third-party assets.

Choose a Canvas drawing function in Designer, then use Graph for primitives and
handlers. `state`, `props`, `frame` remain ordinary typed pins. Visual edits must
not freeze calculated properties into their preview values.

See the [bilingual contract](../../docs/programmable-components.md). Arbitrary
shaders, 3D and Ren’Py/Python compatibility are not part of this example.
