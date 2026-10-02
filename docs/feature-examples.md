# rust-VN feature examples

These seven projects demonstrate the extended RVN engine and Blueprint editor.
Use the supplied **0.2.7-beta.2** candidate, not public 0.2.5. Copy an example to a
writable folder outside the application before editing it. Each project has
English and French locales and its own walkthrough.

## Start with the editor

1. On Windows, copy a folder from `examples` to your Documents. With the Linux
   AppImage, extract the examples into a **new** folder using
   `./rust-VN-Editor.AppImage --appimage-extract-and-run --examples "My RVN examples"`.
   Replace the AppImage filename with the one you downloaded. An existing
   destination is refused; its contents are never replaced.
2. Open the editor, choose **Open**, then select the copied project folder or
   its `rvn.toml`. The supplied examples are already linked to `main.rvn`.
3. Choose a graph in **Graphs**. Functions, reusable screens, handlers and
   narrative labels use the same canvas and typed connections.
4. Save an edit, inspect `main.rvn`, then choose **Play**. Its dropdown also
   offers a preview in your default browser.

| Folder | What to try | Blueprint entry points |
| --- | --- | --- |
| `interfaces` | Inventory, journal, puzzle, text input and long lists | Screens and event handlers |
| `custom-components` | Pure 2D drawing, two independent interactive controls, transforms/clips and local saved state | Canvas, drawing functions, primitives and state command |
| `animations` | Parameterized motion, frames, pause, save and rollback | Animation nodes and their inspector preview |
| `layered-characters` | Independent outfits, expressions and accessories | Composition and layer nodes |
| `videos` | VP8/Vorbis video, pause, seek, subtitles and cinematic playback | Video commands and media inspector |
| `accessibility` | Keyboard focus, large text, contrast, reduced motion and speech | Accessibility commands and component labels |
| `atlas-des-brumes` | Original illustrated adventure: identity, 18-slot inventory, map travel, dial puzzle and two conclusions | `start`, `atlas`, `atlas_draw`, event handlers and the interface Designer |

**Atlas:** open the copied `atlas-des-brumes` folder in the editor and choose
**Play**. Its three-page menu supports the mouse, Q/E, PageUp/PageDown, arrows
and Enter. Close the Atlas to explore the harbor, meet Maëlys and progress the
quest. The project includes its game media and credits, but no personal saves,
previous exports or development QA. See its `README.md` and `README-UI.md`.

For your own script project, linking is explicit. From the editor's **Scripts**
panel, select its RVN source and confirm linking. From a terminal, the equivalent
is `rvn blueprint link "project folder"`; use `--source main.rvn` to select a
different project-relative file. Linking preserves authored text and old graph
files, creates safety copies and refuses to replace an existing link.
See `LANGUAGE-REFERENCE.md` for supported authoring operations and current limits.
The adjacent bilingual guides explain [interface Designer](programmable-ui-authoring.md),
[programmable 2D components](programmable-components.md),
[advanced animations](advanced-animations.md) and
[layered character discovery and variants](layered-characters-advanced.md).
Start with the short [authoring UI guide](authoring-ui-guide.md) for the
Designer/Graph workflow, typed selectors, keyboard editing and Apply/Save.
The [interface Web validation report](programmable-ui-validation-2026-10-01.md)
identifies the exact standalone fixture tested in Chrome and Firefox.
The [custom-component validation report](programmable-components-validation-2026-10-02.md)
records nine real integration checks per browser and the exact tested file hashes.

## Play and export without the editor

On Windows, open a terminal in the application folder:

```powershell
.\rvn_bevy.exe "C:\Users\You\Documents\interfaces"
.\rvn.exe check --strict "C:\Users\You\Documents\interfaces"
.\rvn.exe build --target desktop "C:\Users\You\Documents\interfaces"
.\rvn.exe build --target web "C:\Users\You\Documents\interfaces"
```

With the Linux AppImage:

```sh
./rust-VN-Editor.AppImage --appimage-extract-and-run --engine "My RVN examples/interfaces"
./rust-VN-Editor.AppImage --appimage-extract-and-run --cli check --strict "My RVN examples/interfaces"
./rust-VN-Editor.AppImage --appimage-extract-and-run --cli build --target web "My RVN examples/interfaces"
```

Web exports must be served over HTTP, not opened as local HTML files. The
editor's Web preview starts a local server for you. Desktop video playback
requires the supplied shared libraries; keep their notices with exported games.

## Controls and licensing

**F5** saves quickly, **F6** loads, and **Backspace** rolls back when the story
owns keyboard input. Text fields use Backspace for editing. **F8** opens the
accessibility panel; **V** toggles speech outside text fields. Voices depend on
the platform's speech service and can be unavailable.

The example scripts and original geometric illustrations are MIT licensed.
The small feature examples use a generated geometric video test pattern and a
synthesized tone, not third-party footage. Atlas includes original generated
illustrations, synthesized music and a prologue built from its harbor image;
see `atlas-des-brumes/ASSET-CREDITS.md`. Keep `LICENSE-MIT.txt` when redistributing these examples.
The editor remains proprietary; engine, FFmpeg and dependency licenses are
separate. These examples are not a claim of native Windows, Firefox or assistive
technology certification. Test the exact candidate on those systems before
advertising support.

## Démarrer en français

Ces sept projets accompagnent **0.2.7-beta.2**, pas la version
publique 0.2.5. Copiez un exemple dans un dossier personnel avant de le modifier.
Sous Windows, les exemples sont dans le dossier du logiciel. Sous Linux,
`--examples "Mes exemples RVN"` extrait ceux de l'AppImage dans un nouveau dossier,
sans remplacer de fichiers existants.

Dans l'éditeur, choisissez **Ouvrir**, puis le dossier copié ou `rvn.toml`.
Le script `main.rvn` est déjà lié aux Blueprints. **Graphes** donne accès aux
fonctions, écrans, gestionnaires et labels. Enregistrez une modification,
vérifiez le script, puis utilisez **Jouer** ou son aperçu dans le navigateur.
Chaque dossier contient un parcours en français et en anglais.

**L’Atlas des Brumes :** ouvrez la copie de `atlas-des-brumes`, puis cliquez
**Jouer**. Identité, inventaire et carte se parcourent à la souris, avec Q/E,
PageUp/PageDown, les flèches et Entrée. Fermez l’Atlas pour explorer le Havre et
rencontrer Maëlys ; le cadran et les deux conclusions se débloquent dans le récit.
Médias et crédits sont inclus, sans sauvegardes personnelles, anciens exports ni
outils de QA. Ses guides `README.md` et `README-UI.md` expliquent le jeu et son menu.

Pour un projet personnel, sélectionnez le script dans **Scripts** et confirmez
la liaison. La commande équivalente est `rvn blueprint link "dossier du projet"`.
Les anciens graphes et les commentaires sont conservés, des copies de sécurité
sont créées et une liaison existante n'est jamais remplacée silencieusement.
Consultez `LANGUAGE-REFERENCE.md` pour les possibilités et limites actuelles.
Les guides adjacents détaillent le [Designer d’interfaces](programmable-ui-authoring.md),
les [composants 2D programmables](programmable-components.md),
les [animations avancées](advanced-animations.md) et les
[personnages multicouches](layered-characters-advanced.md).
Le [guide court d’édition](authoring-ui-guide.md) présente Designer/Graphe,
sélecteurs, clavier et Appliquer/Enregistrer. Le
[rapport de validation Web](programmable-ui-validation-2026-10-01.md) identifie
précisément le projet autonome essayé dans Chrome et Firefox.
Le [rapport des composants programmables](programmable-components-validation-2026-10-02.md)
consigne les neuf essais réels par navigateur et les empreintes des fichiers testés.

**F5** sauvegarde, **F6** charge et **Retour arrière** restaure le récit lorsque
la narration possède le clavier. Dans un champ, cette touche efface du texte.
**F8** ouvre l'accessibilité et **V** active la voix hors des champs. La voix
dépend des services disponibles sur le système.

Le code et les illustrations originales des exemples sont sous MIT. Les petits
exemples vidéo utilisent une mire géométrique et un son synthétisé. L’Atlas inclut
ses illustrations originales générées, sa musique synthétisée et un prologue
issu de son image du port ; consultez `atlas-des-brumes/ASSET-CREDITS.md`.
Conservez les licences et notices
avec les jeux exportés. Les essais Linux ne certifient pas Windows réel,
Firefox ou les technologies d'assistance.
