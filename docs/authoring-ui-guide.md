# Les nouveaux espaces d’édition / New authoring workspaces

## Français

**Le Designer règle la présentation ; le Graphe règle la logique.** Les deux
modifient le même RVN et conservent les expressions calculées. Le design des
nœuds validé n’a pas été remplacé par un nouvel aspect.

### Interfaces

Tous les espaces utilisent les mêmes contrôles gris/cyan, recherche, sections
contextuelles et focus visible. **F6** active la navigation de barre d’outils ;
**Tab / Maj+Tab** et les flèches parcourent les actions, **Entrée / Espace**
activent la sélection, **Échap** revient. Les menus Jouer/Exporter sautent les
actions désactivées. Une partie desktop déjà ouverte doit être arrêtée avant
un nouveau lancement ; l’aperçu Web a sa propre action d’arrêt.

Ouvrez **Interfaces** puis utilisez **Écrans** pour choisir un écran. Appliquez
les modifications locales avant de changer d’écran. La palette ajoute
les contrôles ; la hiérarchie sélectionne les éléments. Les détails sont
regroupés en composant, contenu, disposition, apparence, événements et
accessibilité. Recherchez une propriété pour la retrouver sans déplier tous les
groupes. Les propriétés sont filtrées selon le contrôle : une image ne propose
pas les options d’un sélecteur.

Les listes recherchables choisissent les images, polices, variables,
gestionnaires et styles existants. Les interrupteurs se cochent directement ;
les nombres se modifient dans leurs champs et les couleurs se choisissent dans
la palette, avec une valeur personnalisée si nécessaire. **Tab / Maj+Tab**
parcourt les outils et propriétés ; **Entrée** active, **Échap** annule la
saisie. Un champ lié affiche la valeur initialisée de sa variable, sans
permettre de la remplacer par un instantané dans le Designer.

Glissez un composant pour le déplacer ou son coin orange pour le redimensionner.
La disposition automatique confie à nouveau sa position au conteneur.
**Réutiliser** insère une fonction de composant avec ses paramètres ; les
styles peuvent aussi être partagés. **Imbriquer** change de parent et **Ctrl+D**
duplique avec de nouveaux identifiants. Une liste ou propriété calculée reste
signalée comme telle : ouvrez son Graphe, elle n’est jamais aplatie en données
statiques.

### Animations, personnages et accessibilité

L’aperçu d’animation et l’aperçu vidéo partagent une présentation sobre : scène,
paramètres, lecture/pause et timeline. La vidéo reste silencieuse pendant cet
aperçu. Une animation peut afficher sa scène narrative ; si un choix est
nécessaire, le parcours est demandé. L’éditeur de trajectoire permet de déplacer
les points visuellement ou au clavier.

La composition affiche séparément attributs d’aperçu, calques et propriétés du
calque sélectionné. La recherche filtre calques et attributs. Les attributs
essayés ne remplacent pas les valeurs par défaut sans action explicite.
Les réglages d’accessibilité ont leur propre aperçu ; la voix dépend du système
et n’est pas simulée dans cette fenêtre.

### Appliquer n’est pas encore enregistrer

**Appliquer** valide la copie de travail et met à jour le graphe. **Enregistrer
le projet** met ensuite à jour le `.rvn`. **Ctrl+Z** annule dans l’espace
d’édition ; **Ctrl+Maj+Z / Ctrl+Y** rétablit. Fermer une copie modifiée demande
confirmation. Un conflit, une expression invalide ou une ressource absente
reste une erreur visible : aucune réussite silencieuse ni écrasement du graphe
concurrent. Testez ensuite avec **Jouer**, puis dans le jeu exporté.

Voir les guides [interfaces](programmable-ui-authoring.md),
[composants programmables](programmable-components.md),
[animations](advanced-animations.md) et [personnages](layered-characters-advanced.md)
pour les contrats précis. La [validation Web](programmable-ui-validation-2026-10-01.md)
décrit les essais réels, sans certification Windows ou parité totale Ren’Py.

## English

All workspaces share gray/cyan controls, search, contextual sections and visible
focus. **F6** enters toolbar navigation; **Tab / Shift+Tab** and arrows move,
**Enter / Space** activate and **Escape** returns. Play/Export menus skip
disabled actions. Stop an existing desktop test before launching another;
the Web preview has its own stop action.

**Designer edits presentation; Graph edits behavior.** Both edit the same RVN
and retain calculated expressions. The approved Blueprint-node design remains
unchanged.

In **Interfaces**, use **Screens** to choose a screen. Apply local changes
before switching screens. Use the palette to add
controls and the hierarchy to select them. Details are grouped into component,
content, layout, appearance, events and accessibility. Property search reveals
matches without expanding every group. Only relevant properties appear: an
image does not expose a selector’s options.

Searchable choices select project images/fonts, variables, handlers and styles.
Toggle switches directly, edit numeric fields and choose colors from the
palette or enter a custom value.
**Tab / Shift+Tab** traverses tools and properties, **Enter** activates and
**Escape** cancels an edit. A bound field previews its initialized variable;
Designer will not replace that binding with a frozen value.

Drag a component or its orange resize corner. Automatic layout returns placement
to its container. **Reuse** inserts a parameterized component function; styles
can be shared too. **Nest** changes parent and **Ctrl+D** duplicates with fresh
IDs. Calculated properties/lists remain connected expressions: open Graph to
edit them, rather than flattening them into static preview data.

Animation and video previews share a quiet scene/parameters/playback/timeline
layout. Video preview is silent. An animation can display its narrative scene
and asks for choices when a route is needed. Path points can be edited visually
or from the keyboard. Composition separates preview attributes, layers and the
selected layer’s properties, with searchable lists. Trying an attribute does
not silently replace authored defaults. Accessibility has a separate settings
preview; system voices are not simulated in that window.

**Apply** validates local edits and updates the graph. **Save the project**
then updates `.rvn`. **Ctrl+Z** undoes local edits; **Ctrl+Shift+Z / Ctrl+Y**
redoes. Closing a modified working copy asks for confirmation. Conflicts,
invalid expressions and missing resources remain visible errors, never silent
success or an overwrite of a concurrently edited graph. Test with **Play**
and then test the standalone export.

See the [interface](programmable-ui-authoring.md), [programmable component](programmable-components.md), [animation](advanced-animations.md)
and [character](layered-characters-advanced.md) contracts and the
[real-browser validation report](programmable-ui-validation-2026-10-01.md).
This is not Windows certification or a claim of complete Ren’Py parity.
