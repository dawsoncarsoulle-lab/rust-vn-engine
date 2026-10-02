# Inventory, journal and puzzle / Inventaire, journal et puzzle

Development example for reusable RVN screens. It requires the development
engine and editor containing the programmable-interface changes, not the
published 0.2.5 editor. No third-party images, audio or video are required.

Exemple de développement pour les écrans RVN réutilisables. Il nécessite le
moteur et l'éditeur de développement avec ces nouvelles fonctions, pas
l'éditeur public 0.2.5. Aucune image, musique ou vidéo externe n'est nécessaire.

## Run / Lancer

From the engine repository / Depuis le dépôt du moteur :

```sh
cargo build -p rvn_cli -p rvn_bevy
target/debug/rvn check --strict examples/interfaces
target/debug/rvn_bevy examples/interfaces
```

For a standalone Web export / Pour un export Web autonome :

```sh
target/debug/rvn build --target web examples/interfaces
```

Serve the generated directory through HTTP, using the directory printed by
the command. Opening `index.html` as a local file is not supported. Development
builds need the WebAssembly target and a matching `wasm-bindgen` CLI version.

Servez le dossier généré en HTTP, en reprenant le chemin affiché par la
commande. L'ouverture directe de `index.html` n'est pas prise en charge.
La compilation de développement nécessite la cible WebAssembly et la version
de `wasm-bindgen` correspondant aux dépendances du moteur.

## Try / Essayer

1. Start a new game. The journal and inventory are open together; the inventory
   is modal, so interacting with it does not advance the dialogue underneath.
2. Edit the name, including accents. The journal is bound to `player_name`.
3. Toggle the hint, select one of 25 reading-theme values and drag the slider.
   These controls demonstrate saved variable bindings. The theme selector does
   **not** change the application's theme; that would require an authored handler.
4. Open the archive puzzle. Try an incorrect answer, then `dawn` (case-insensitive).
   The completed quest appears in the journal. Without hints, the answer remains
   `DAWN`.
5. Open the long inventory. Scroll through 40 generated entries or navigate
   with Tab / Shift+Tab; keyboard focus brings the focused entry into view.
6. Save, close, reopen and load. Check the name, controls, quest completion and
   open screens. Rollback restores game values, open screens and random state;
   mouse scrolling is transient presentation state, not story data.
7. Select Letter or Key to close the inventory, then continue the dialogue.

En français : démarrez une partie, modifiez le nom et les contrôles, ouvrez le
puzzle, essayez une mauvaise réponse puis `dawn`. Le journal affiche la quête
terminée. L'inventaire long contient 40 éléments ; utilisez la molette ou
Tab / Maj+Tab. Vérifiez sauvegarde, fermeture, chargement et retour arrière.
Enfin, choisissez Lettre ou Clé pour fermer l'inventaire et lire la suite.
Le sélecteur de thème illustre une liaison de variable, sans modifier le thème
de l'application. Le défilement à la souris n'est pas une donnée narrative.

## Script and Blueprints / Script et Blueprints

`main.rvn` contains calculation functions, four parameterized/reusable screens
and event handlers. `locales/en.toml` and `locales/fr.toml` keep display labels
separate from stable control values and component identities.

In the development editor, link `main.rvn` from the Scripts panel only after
saving existing graphs. Screens and handlers become separate graph tabs.
Select a **UI component** node for its controls, localization keys, bindings,
layout and event handlers. A screen returns its root component through
**Return value**. Narrative graphs open it using **Open interface**.

Dans l'éditeur de développement, enregistrez les graphes existants avant de
lier `main.rvn` depuis Scripts. Les interfaces et gestionnaires apparaissent
dans des onglets séparés. Sélectionnez un nœud **Composant d'interface** pour
ses contrôles, traductions, liaisons, disposition et événements. Une interface
renvoie son composant racine via **Retourner une valeur** ; le graphe narratif
l'affiche avec **Ouvrir une interface**.

Linking is explicit and backed up. Do not overwrite an externally modified
script when the editor reports a conflict. The current source-linking
boundaries are documented in the language reference; they are not a claim
that the full extension programme has been delivered.

La liaison est explicite et crée des copies de sécurité. Un conflit avec un
script modifié à l'extérieur doit être résolu sans écrasement. Les limites de
liaison actuelles sont décrites dans la référence du langage ; cet exemple
ne signifie pas que l'ensemble du programme d'extension est livré.
