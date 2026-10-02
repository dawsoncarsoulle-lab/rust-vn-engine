# Advanced layered characters / Personnages multicouches avancés

## Français

`layered_image(dimensions, attributs_par_défaut, calques)` reste compatible.
Un quatrième argument facultatif regroupe `variants`, `rules` et `selector`.
Ces descriptions sont de simples dictionnaires RVN : aucune dépendance Python.

```rvn
function accessoires(attributs) {
    if attributs["tenue"] == "manteau" { return {"accessoire":"badge"} }
    return {"accessoire":""}
}
function portrait() {
    return layered_image([600,1000],
        {"tenue":"chemise","expression":"neutre","accessoire":"","variant":""},
        image_layers("iris", [
            "iris__corps.png",
            "iris__tenue__chemise.png", "iris__tenue__manteau.png",
            "iris__expression__neutre.png", "iris__expression__sourire.png",
            "iris__soir__expression__sourire.png", "iris__accessoire__badge.png"
        ]), {
            "variants":{"soir":{"tenue":"manteau"}},
            "rules":{"01_soir":{"when":{"variant":"soir"},"set":{"expression":"sourire"}}},
            "selector":"accessoires"
        })
}
label start
character.compose("iris",portrait())
iris.show()
character.attributes("iris",{"variant":"soir"})
```

### Images et ordre

- `préfixe__identité.png` : calque toujours présent.
- `préfixe__groupe__attribut.png` : attribut d'un groupe exclusif.
- `préfixe__variante__groupe__attribut.png` : remplacement du calque de base
  ayant le même groupe et le même attribut, uniquement pour cette variante.
- PNG, JPEG et WebP sont acceptés. Les chemins restent relatifs au projet.
  L'ordre de la liste est l'ordre de dessin, pas un classement alphabétique
  imposé. Les doublons de chemins sont ignorés ; identités ambiguës et noms
  malformés sont des erreurs. Les images non préfixées sont ignorées.

La liste explicite d'images rend les projets exportés déterministes sur desktop
et Web : le jeu ne lit pas arbitrairement un dossier au moment de l'exécution.
Le concepteur découvre les véritables images du projet et écrit cette liste.
L'ajout d'un fichier après export nécessite une nouvelle découverte ou un ajout
dans la liste. Un calque explicite `image_layer` accepte aussi `variant` dans
ses propriétés. Les images manquantes sont signalées, même sur un calque inactif.

### Sélection

Une mise à jour conserve les groupes non mentionnés. Si elle contient `variant`,
le préréglage correspondant est appliqué d'abord ; les valeurs de la mise à jour
explicite passent ensuite. Les règles sont appliquées **une fois**, dans l'ordre
alphabétique de leur nom : `when` exige tous ses attributs, `unless` bloque si
au moins un de ses attributs correspond, puis `set` remplace les groupes choisis.
Le préfixe numérique `01_`, `02_` permet de rendre la priorité évidente.
Les règles et sélecteurs ne peuvent pas changer récursivement `variant`.

Enfin, `selector` nomme une fonction de calcul RVN à **un paramètre** : elle reçoit
les attributs proposés et retourne un dictionnaire de modifications. Une fonction
peut consulter les variables du jeu, employer conditions, dictionnaires et
boucles bornées. Ses variables restent locales : aucune mutation narrative.
Un résultat inconnu/non textuel, une fonction absente ou une boucle infinie
produit un diagnostic sans modification partielle du personnage ou du hasard.
Les limites communes sont 100 000 opérations de calcul et 64 appels imbriqués.
Les attributs résolus, descriptions, règles et variantes sont sauvegardés et
restaurés par chargement et rollback ; le sélecteur n'est pas rejoué au chargement.
Les anciennes sauvegardes reçoivent des options vides.

### Éditeur

Dans le graphe `portrait`, sélectionnez **Composition multicouche**, puis ouvrez
**Aperçu de la composition**. Le menu **⋯** propose découverte, préréglages,
règles guidées et sélecteur RVN avec recherche. Choisir un attribut ne modifie
pas silencieusement les valeurs par défaut. **Appliquer** valide les changements
locaux ; annuler/rétablir fonctionne pendant l'édition. Le graphe garde les
commentaires et positions existants lors de la synchronisation vers `.rvn`.

Les calques calculés par `image_layers` ne sont pas réécrits en calques explicites
quand vous éditez leurs variantes. Le nœud **Découvrir les calques** conserve un
préfixe et une liste d'images éditables dans les Blueprints. Pour régler finement
l'ordre d'une découverte, modifiez l'ordre de cette liste ; pour une disposition
manuelle indépendante, utilisez des nœuds **Calque d'image**.

## English

The original three-argument `layered_image(size, defaults, layers)` remains
compatible. Its optional fourth `options` dictionary contains named `variants`,
named `rules` and an RVN `selector` function. No Python is involved.

`image_layers(prefix, paths)` discovers unconditional `prefix__id` images,
exclusive `prefix__group__attribute` images, and
`prefix__variant__group__attribute` variant overrides. Paths remain project
relative. PNG/JPEG/WebP are accepted. The authored list order is drawing order;
duplicate paths are ignored, ambiguous identities/malformed matching names are
errors, and unrelated images are ignored. The designer writes an explicit list
of actual project images, making exported desktop/Web games deterministic.
Adding assets after export requires updating the list or discovering again.
Missing resources are reported even for inactive layers.

Updates preserve unmentioned groups. A selected variant first applies its
preset; explicit patch values override that preset. Named rules run once in
lexical order (`01_`, `02_` clarify priority): all `when` conditions must match,
any matching `unless` blocks a rule, and `set` changes the selected groups.
Variant-specific art overrides only matching base group/attribute art.

Finally a one-parameter RVN selector receives proposed attributes and returns
a dictionary patch. It uses the existing bounded computation engine (100,000
operations, 64 nested calls), with local variables and no narrative mutation.
Rules/selectors cannot recursively change `variant`. Invalid return values,
missing functions and nonterminating computation fail atomically. Resolved
attributes, rules and definitions persist through save/load and rollback;
load does not rerun the selector. Old saves default to empty options.

In the composition designer, **⋯** offers project discovery, save/remove variant,
guided group/value rule creation/removal, and searchable one-parameter selector
functions. Changes are staged until **Apply**, with undo/redo and conflict
checks. Preview selections do not silently replace authored defaults. Discovery
stays a real **Discover image layers** Blueprint, not opaque generated JSON;
its image list and prefix remain editable. Reordering that list controls drawing
order. Explicit **Image layer** nodes remain available for fully manual designs.
Source synchronization preserves comments and existing node placement.

This is an RVN feature contract, not Ren'Py script compatibility or a promise
to reproduce every layeredimage extension or Python API.
