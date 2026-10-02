# Composable animations / Animations composables

Development example: it requires the extended engine and Blueprint editor,
not the published 0.2.5. All illustrations are original geometric SVGs and
their rasterized PNGs, distributed under this repository's MIT license.
No external artwork, music or video is required.

Exemple de développement : utilisez le moteur et l'éditeur Blueprint étendus,
pas la version publique 0.2.5. Les illustrations sont des formes géométriques
originales, en SVG et PNG, sous la licence MIT de ce dépôt.

## Run / Lancer

```sh
cargo build -p rvn_cli -p rvn_bevy
target/debug/rvn check --strict examples/animations
target/debug/rvn_bevy examples/animations
```

For Web, run `target/debug/rvn build --target web examples/animations` and
serve the generated directory over HTTP. Do not open its HTML as a local file.

Pour le Web, utilisez la commande ci-dessus avec `build --target web`, puis
servez le dossier généré en HTTP ; n'ouvrez pas directement le fichier HTML.

## Try / Essayer

1. Start a new game. The background tint, sprite and interface image move
   independently. The image alternates between two frames.
2. Save while moving, let the animation progress, then load. Positions,
   opacity, frames and animation clocks must return to the saved values.
3. Advance the first dialogue before the animation ends. The story waits
   for the sprite, then continues. The final pose remains visible.
4. Advance again. A new sequence moves the sprite, pauses, returns and stops.
   Stop restores its authored base appearance, rather than adding offsets.
5. Advance to hide the sprite and close the screen; roll back to restore
   both the targets and their animation state.
6. In the editor, link `main.rvn` explicitly as the authored source. The
   `entrance` function, `motion_panel` screen and `start` label are editable
   Blueprints. Select an animation node and open its inspector preview.
   Try Play, Pause, seeking and changing the `seconds` preview argument.
   Preview values do not alter the story or execute narrative commands.
7. Continue after the targets disappear: the token returns and follows a
   four-point spline. `gentle_curve(t)` controls time, and a cubic Bézier
   controls rotation. In `curved_flight`, select the Spline node and open
   **Edit trajectory**: add/remove points, drag them, or use Ctrl+arrows.
   Opening **Play animation** previews the actual preceding background,
   characters and open screens; narrative choices are selected explicitly.

1. Lancez une partie : le fond, le personnage et l'image d'interface s'animent
   indépendamment, et l'image alterne entre deux trames.
2. Sauvegardez en mouvement, puis chargez : les horloges, positions, opacités
   et trames doivent revenir à leur état sauvegardé.
3. Avancez avant la fin : le récit attend le personnage, puis continue.
4. Avancez encore : une nouvelle séquence déplace le personnage, marque une
   pause, revient et s'arrête. L'arrêt restaure son apparence de base.
5. Masquez le personnage et fermez l'écran, puis revenez en arrière pour
   restaurer les cibles et leur état d'animation.
6. Liez explicitement `main.rvn` dans l'éditeur. La fonction `entrance`,
   l'écran et le récit sont des Blueprints. L'aperçu permet lecture/pause,
   déplacement temporel et réglage de `seconds`, sans modifier le récit.
7. Continuez après la disparition : le jeton revient sur une spline de
   quatre points, avec `gentle_curve(t)` et une courbe Bézier pour la rotation.
   Dans `curved_flight`, sélectionnez la spline et **Modifier la trajectoire** :
   ajout/retrait et déplacement des points, à la souris ou avec Ctrl+flèches.
   Le nœud **Lire une animation** ouvre un aperçu du contexte réel du récit,
   avec son fond, ses personnages et ses interfaces ; les choix sont explicites.

## Validation scope / Périmètre de validation

Unit tests cover the shared sampler, bounds, constructors, typed pins,
round trips, source comments, replacement, cancellation, saves and rollback.
Native QA records real rendered frames and checks applied transforms.
This example alone is not evidence of Windows or Firefox validation;
those require running the exact exported candidate on those platforms.

Les tests automatisés ne remplacent pas l'essai du paquet exact sur Windows
et dans Firefox. N'annoncez pas ces plateformes comme validées sur la seule
base d'une compilation ou d'un test Linux.
