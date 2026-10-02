# Programmable component validation 2 October 2026

## Scope and result

The standalone `examples/custom-components` Web export passed nine integration
checks in each of real Chrome and Firefox on Linux, outside the editor, using
software WebGL2. This report identifies the exact tested files and the evidence
for programmable drawing, independent instance state, real input and restoration.
It does not certify Windows, all GPUs, assistive technologies or full Ren’Py parity.

L’export Web autonome `examples/custom-components` a réussi neuf contrôles dans
chacun des navigateurs Chrome et Firefox réellement exécutés sous Linux, sans
l’éditeur, avec WebGL2 logiciel. Ce rapport identifie les fichiers exacts testés
et les preuves de dessin programmable, d’état indépendant, d’entrées réelles et
de restauration. Il ne certifie pas Windows, tous les GPU, les technologies
d’assistance ni une parité complète avec Ren’Py.

See [the programmable component guide](programmable-components.md) for the RVN,
Blueprints, drawing and save-state contracts.

## Exact tested export

The run started at **2026-10-01 23:35:51.643 UTC** and finished at
**23:36:29.450 UTC**, or **2 October 2026, 01:35:51.643–01:36:29.450 Europe/Paris**.
Both browsers executed the same export produced by the release CLI and
`wasm-bindgen 0.2.118` from the current runtime source.

Les deux navigateurs ont exécuté le même export. Une reconstruction peut changer
ces empreintes : cette preuve ne s’applique pas automatiquement à d’autres octets.

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `game_bg.wasm` | 52,380,061 | `de62f45d47a6785ab336533cf92e325a13f33002159d5069bb514474379f924c` |
| `game.js` | 111,549 | `0f7660478d7a1892b1057d9429362de7cb2db4eb34f64643a0326d61099ce95d` |
| `main.rvn` | 5,768 | `5027865377fec202af67982fd6fff12e6b43c29f775cb8258297c4a12cee6aa8` |
| `rvn.toml` | 222 | `3a2986db26e67059af6389c230570f9401eb4c64a5db1903fe329c0c991b4274` |
| `theme.toml` | 155 | `13d730938eca7ef7ebc6cdd2e6b8c8b6119afb7c7b1ebb4966c7bad2ddc15749` |
| `assets/emblem.png` | 1,392 | `bda2e6e4fc53b4e15f1d31ddb9e89fd73a289dc2d5eac3b6548e8a5d9fb60dc8` |

| Browser | Result |
| --- | --- |
| Chrome 153.0.8010.47 | 9 of 9 checks passed |
| Firefox 153.0 | 9 of 9 checks passed |

## Real browser checks

1. The exported game starts independently of the editor and draws shapes, actual
   inherited-font glyphs and a transformed group with clipping.
2. The two canvas instances begin at independently authored values `0.25` and
   `0.75`. Their local dictionaries and simulation clocks appear in save format 9.
3. Actual pointer down, movement and release outside the first canvas preserve
   capture. Only the first instance changes, and its `dragging` state is released.
4. Actual wheel input provides a signed delta and updates the local value.
5. Key press and release reach the focused canvas handler with `ArrowLeft` as
   the key code; the other instance retains its value.
6. F5 quicksave and F6 quickload with explicit confirmation restore both local
   states and reconstruct the drawing without retaining obsolete pointer capture.
7. The native browser input accepts `Éloïse 😀` while keeping its DOM identity,
   focus and caret during reactive redraw. Shift+Tab moves back to the canvas;
   an actual ArrowRight then updates that canvas, and the edited string is saved.
8. The exported original PNG loads, with no fatal runtime or page error in the
   healthy run.
9. A real 404 for that required PNG displays a visible error card naming screen
   `custom`, canvas `first` and `emblem.png`, rather than reporting silent success.

Les neuf contrôles ont réussi dans les deux navigateurs : dessin et clip,
états/horloges indépendants, capture pendant le glisser hors du composant, molette,
pression et relâchement clavier, sauvegarde/chargement confirmé, saisie Unicode
avec curseur conservé et retour au canvas, image chargée et erreur visible lorsque
cette ressource manque. Les captures initiales, restaurées et d’erreur ont été
relues visuellement.

The sampled pixels are identical in both browsers: turquoise track
`[13,204,191,255]`, canvas background `[6,11,19,255]`, transformed clipped group
interior `[18,55,92,255]`, and clipped exterior `[6,11,19,255]`. The input caret is
at UTF-16 offset 7 in the edited string after ArrowLeft. These samples supplement
the screenshots; they are not an exhaustive pixel-perfect certification.

Optional `/menus.rvnui` and `/assets/emblem.png.meta` requests return 404 and use
the existing defaults. Winit’s intentional control-flow exception is recorded
as a warning, not a runtime failure. The required PNG does not 404 in the healthy
run; the separate negative test deliberately refuses it.

## Supporting tests and reproduction

The targeted suites passed: eight core canvas tests, two parser callback tests,
seven graph reimport tests and thirteen source-project tests. They cover saved
state and clock restoration, rollback, migration from save format 8, atomic
failure, pure bounded drawing, modal/hidden ticking, explicit renderer capability
errors, all drawing primitives, static callback diagnostics and preservation of
comments, graph identity and branch ownership through source round trips.

Les tests ciblés ont réussi et couvrent notamment sauvegarde, rollback,
migration, erreurs atomiques, calcul pur borné et conservation du code et des
identifiants lors des allers-retours visuels.

To reproduce the browser checks after exporting the example, run:

```sh
node tools/test-web-custom-components.mjs EXPORT_DIRECTORY EVIDENCE_DIRECTORY
```

The runner requires Playwright, pngjs, Chrome and Playwright Firefox. It serves
the standalone export over local HTTP, drives real pointer/keyboard/DOM input,
collects saved data, captures screenshots and records file hashes and browser
versions. It does not call `UiInput` directly or simulate a successful renderer.

The machine-local artifacts from this run are at
`/tmp/rvn-canvas-web-evidence-20261002`, including
`web-custom-components-evidence.json` and each browser’s initial, edited,
load-confirmation, restored and missing-image screenshots. Temporary evidence
directories may be removed by system cleanup; the runner regenerates them.

## Limits of this evidence

Windows must still be executed on a real Windows session. This run does not
validate every hardware GPU, operating-system IME composition service, voice or
screen reader. Unicode editing and focus retention are proven here, not a complete
accessibility certification. Native Linux and editor authoring verification are
separate from these browser checks. No Python integration, unrestricted system
access, mobile support or full Ren’Py compatibility is implied.

Windows exige encore une session Windows réelle. Ces essais ne valident pas
tous les GPU, services de composition IME, voix ou lecteurs d’écran. La saisie
Unicode et le focus sont vérifiés, pas une certification d’accessibilité complète.
Les preuves Linux natif et éditeur sont distinctes. Python, l’accès système
arbitraire, le mobile et une compatibilité complète avec Ren’Py ne sont pas promis.
