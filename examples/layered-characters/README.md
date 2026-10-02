# Layered characters / Personnages multicouches

Development example: use the extended engine and editor, not public 0.2.5.
All images are original geometric SVG illustrations and their rasterized PNGs,
under the repository's MIT license. `stage.png` reuses the original geometric
backdrop from `../animations/assets/backdrop.svg`; no external artwork is used.

Exemple de développement : utilisez le moteur et l'éditeur étendus, pas la
version publique 0.2.5. Les SVG géométriques et leurs PNG sont originaux, sous
la licence MIT du dépôt. Le fond vient du SVG original de l'exemple animations.

## Run / Lancer

```sh
cargo build --release -p rvn_cli -p rvn_bevy
target/release/rvn check --strict examples/layered-characters
target/release/rvn_bevy examples/layered-characters
```

For Web, build with `rvn build --target web` and serve the output over HTTP.
Pour le Web, compilez avec `rvn build --target web`, puis servez le résultat
en HTTP ; n'ouvrez pas directement le fichier HTML.

## Try / Essayer

1. Start a new game: body, shirt and neutral expression are independent layers.
2. Advance: the coat replaces only the outfit, keeping the neutral expression.
3. Advance: the happy expression and badge appear. The badge requires both the
   coat and its accessory attribute; its own animation uses a named layer target.
4. Save during motion, then load: attributes and both animation clocks return.
5. Advance before the badge finishes: the narrative waits, then resumes.
6. Advance: the shirt returns, the badge disappears and the happy face remains.
7. Hide the character and roll back: its composition and selections return.
8. Explicitly link `main.rvn` as the editor's source. The `portrait` function
   is a typed Blueprint. Select a composition or layer node to inspect it and
   preview attribute choices, visibility and missing resources.
9. Continue after hiding: `discovered_portrait` demonstrates automatic image
   attributes, an evening variant, an ordered selection rule, and a bounded RVN
   selection function. The composition designer's **⋯** menu edits all of these
   without entering JSON: discover project images, save/remove a preset, add or
   remove a guided group/value rule, and select a one-parameter RVN function.

1. Lancez une partie : corps, chemise et visage sont des calques distincts.
2. Avancez : le manteau remplace uniquement la tenue, pas l'expression.
3. Avancez : sourire et badge apparaissent. Le badge dépend de la tenue et de
   son attribut ; il s'anime séparément grâce à une cible de calque nommée.
4. Sauvegardez en mouvement, puis chargez : attributs et horloges sont restaurés.
5. Avancez avant la fin du badge : le récit attend, puis reprend.
6. La chemise revient : le badge disparaît, le sourire reste.
7. Masquez le personnage puis revenez en arrière : ses sélections sont restaurées.
8. Liez explicitement `main.rvn` dans l'éditeur. `portrait` devient un Blueprint
   typé. L'inspecteur ouvre l'aperçu des attributs, calques et ressources.
9. Continuez après le masquage : `discovered_portrait` montre la découverte par
   noms d'images, une variante du soir, une règle ordonnée et une fonction RVN
   bornée. Le menu **⋯** du concepteur édite ces options sans JSON : découverte
   des images du projet, enregistrement/suppression de variante, règle guidée
   groupe/attribut et choix d'une fonction RVN à un paramètre.

Discovery keeps the explicitly authored image-list order (and deduplicates
identical paths). The PNG aliases are copies of the original MIT example art.
See [advanced reference](../../docs/layered-characters-advanced.md) for precedence,
asset naming, variants, selector restrictions and backward-compatible saves.

La découverte conserve l'ordre de la liste d'images écrite (sans doublons de
chemins). Les alias PNG reprennent les illustrations originales MIT de cet
exemple. Voir la [référence avancée](../../docs/layered-characters-advanced.md).

## Validation scope / Périmètre de validation

Automated tests cover constructors, independent attributes, schema and resource
validation, typed pins, source-preserving round trips, saves and rollback.
Real-window QA verifies selected layer entities, loaded images and rendered
dialogue, and records screenshots. Windows and browser compatibility require
running the exact exported candidate on those platforms; a Linux build alone
does not validate them.

Les tests automatisés et Linux ne remplacent pas l'essai du paquet exact sur
Windows réel et dans les navigateurs annoncés.
