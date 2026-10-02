# Programmable UI validation — 2026-10-01

## Scope / Périmètre

This report identifies a **standalone Web test export**, not a claim that every
example, the final installer, Windows or assistive technology has been tested.
The renderer was executed in real Chrome and Firefox on Linux, with software
WebGL, outside the editor. The owned source fixture is
`tools/fixtures/interface-web`; its README explains reproduction and font
licensing. No user project was changed.

Ce rapport identifie un **export Web autonome de test**, pas une certification
de tous les exemples, de l’installateur final, de Windows ou des technologies
d’assistance. Les essais ont exécuté réellement Chrome et Firefox sous Linux,
avec WebGL logiciel, indépendamment de l’éditeur. Aucun projet utilisateur n’a
été modifié. La source reproductible est `tools/fixtures/interface-web`.

## Exact tested package / Paquet essayé

Run: **2026-10-01 20:11:49–20:12:20 UTC**. Built by `rvn_cli` in release mode for
Web, using `wasm-bindgen 0.2.118`. Both browsers executed the same exported bytes.

| File | SHA-256 |
| --- | --- |
| `game_bg.wasm` (50,979,760 bytes) | `6a8213d14a95485aea7e589e11266949a2d02fcd5201850f9261fd226254ec80` |
| `game.js` | `71dac875918c4a92b48f598af2735501b0be40e738e71eaca4ff7b2be4832399` |
| `main.rvn` | `24f94d1f6b35b8066a08294ca0af3c8c5f57587cbcc9420538d75146fefaa276` |
| `rvn.toml` | `4e3251231e5d1dadcb35f7611e29d828076dd5af3936e636e5478d327da2906e` |
| `theme.toml` | `684ee5c577ab9c7f8991a057b7942aa8ecd9f34baf9ffcb95020b12fda0c0e8d` |
| `assets/fonts/DejaVuSerif.ttf` | `8f2c103bfa3fd5de71f1b92b18f21906b5a26871fb7e19a9a4c9af539c3cc7ab` |

| Browser | Result |
| --- | --- |
| Chrome 153.0.8010.47 | 11/11 integration checks passed |
| Firefox 153.0 | 11/11 integration checks passed |

Rebuilding the runtime can change these fingerprints. Check the exact new
export rather than assigning this result to unrelated bytes.

Une reconstruction peut changer ces empreintes : rejouez les tests sur le
nouvel export au lieu d’attribuer cette preuve à d’autres fichiers.

## Real-browser checks / Contrôles réels

1. The game starts without the editor and initializes bound controls.
2. The custom inherited project font loads in canvas text and native browser
   fields; the authored centered field retains its alignment.
3. Pixel checks preserve distinct background, focus border and rounded corners.
   Background is `[20,28,38,255]`; the focused border is `[255,191,51,255]`.
4. Authored 6-pixel text scales to 4 CSS pixels at 1280×720, rather than being
   enlarged by a hidden 14-pixel minimum. Both browsers show native input text
   at 4 pixels and actual canvas glyphs occupying four visible pixel rows.
5. Nested absolute fields match shared reference geometry within one rounded
   pixel, including parents with borders.
6. Clicking the actual canvas button supplies `event_data` to the RVN handler
   and updates the bound selected item to `ruby`.
7. Editing `Éloïse` preserves the native input element during reactive refresh.
8. Tab follows authored focus order without trapping the player in a field.
9. F5 writes the current variables and open screen to the browser quicksave.
10. After changing the value, F6 and **explicit confirmation** restore `Éloïse`,
   `ruby`, the open screen, inherited font and working bindings.
11. A real font-request 404 produces a readable visible error card naming the
    screen, component and missing asset; the invalid editable fields disappear.
    It is not treated as a successful fallback.

Les mêmes onze contrôles ont réussi dans les deux navigateurs : démarrage,
liaisons, police héritée, couleurs/coins, géométrie, clic avec données,
saisie Unicode persistante, focus clavier, sauvegarde/chargement confirmé et
diagnostic visible d’une police absente, et petite typographie à son échelle
exacte (6 pixels de référence → 4 pixels de rendu). Les captures ont été relues, dont le
jeu restauré et l’erreur de ressource.

The machine-local JSON evidence and screenshots from this run are retained at
`/tmp/rvn-interface-web-qa-aWDQXQeh/evidence`. They include the initial screen,
edited values, load confirmation, restored game and missing-font error for each
browser. `tools/test-web-interface-export.mjs` regenerates these artifacts.

Optional locale/menu/asset-metadata 404s use documented defaults; required
resources must not 404 in the healthy run. Winit’s intentional control-flow
exception and Firefox’s deprecated debug-renderer extension warning are recorded
separately from errors. No healthy-run runtime/page errors were recorded.

## Supporting tests / Tests complémentaires

`cargo test -p rvn_ui -p rvn_core -p rvn_graph` passes (one existing ignored
test). The tests include defaults, style cascades, reusable parameterized
screens, event data, safe fonts, collection/layout bounds, save/load/rollback,
source comments, raw dictionary authoring, shared-expression copy-on-write,
atomic errors, reparenting and duplication.

The eight `rvn_bevy` programmable-UI unit tests pass. The browser-input adapter
test (`tools/test-web-inputs.mjs`) also verifies persistent input identity,
selection/IME protection, focus, alignment, safe font URLs, cache reuse and font
load-error reporting. These headless tests alone are not visual validation;
the real-browser checks above provide that separate evidence.

The pinned Bevy 0.14.2 `src/render/ui.wgsl` had reversed antialias `clamp`
arguments. The runtime corrects only that known expression, rejects an unknown
or ambiguous shader, and accepts an already-corrected single expression. Unit
tests cover the guard and pixel checks cover real WebGL rendering. Registry
sources are not modified or vendored; existing Bevy MIT/Apache-2.0 notices remain
applicable. Blueprint nodes use Makepad and are unaffected by this renderer fix.

Windows execution, a physical GPU matrix, every browser/voice and full Ren’Py
parity are **not** established by these results.

Ces résultats ne prouvent **pas** une exécution Windows réelle, tous les GPU,
tous les navigateurs/voix ni une parité complète avec Ren’Py.
