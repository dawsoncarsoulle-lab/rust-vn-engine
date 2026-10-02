# Type and pin coherence — 2 October 2026

## Scope / Périmètre

This report covers the local **0.2.7-beta.2** editor update: inferred pin types,
variable scopes, English technical type labels and bounded sidebar text.
Approved node geometry, materials and palette RGB values are unchanged.

Cette mise à jour corrige la cohérence des types, broches, fils et inspecteurs.
Elle ne change pas le design validé. Les paramètres réellement inconnus restent
`Wildcard` : ils ne deviennent pas des `String` par défaut. Les variables globales
au type stable conservent ce type dans les gestionnaires. Une variable RVN qui
change effectivement de type reste dynamique avec un avertissement.

In source-linked projects, globals are shared across the story, while function
parameters and explicit locals keep their scope. Standalone graph documents do
not silently share a registry; the inspector says `Document · global` for them.
Display specialization of generic pins does not narrow authoring constraints.
Legacy numeric RVN inputs retain their accepted behavior; newly authored scalar
Integer-to-Float links insert a visible conversion.

## Exact installed package

Verified on Linux at **2026-10-02 08:43 UTC** (10:43 Europe/Paris).

Local installation identifier: `20261002-103029-c8udt5p7`.
The machine-specific installation path is not distributed in this report.

At that verification time, the local `current` symlink pointed to this release,
the prior release was preserved, and the installed editor matched the final
release build byte for byte. This historical check does not identify the latest
GitHub release or an installation on another machine.

| File | SHA-256 |
| --- | --- |
| `rust-vn-editor` | `1bfbd4bded28e1e76d111c718a3d1eaa6726970305c82cf83341d9eac3f99633` |
| `rvn` | `fe140db355877f48debb0571fcadaa4cb945cda968a5f6f59aeff16586abab99` |

## Automated checks

| Suite | Passed | Failed | Deliberately ignored |
| --- | ---: | ---: | ---: |
| Editor, release with video feature | 231 | 0 | 4 |
| rvn_graph, release, full suite | 183 | 0 | 1 |
| rvn_cli, release | 42 | 0 | 0 |
| Desktop packaging | 12 | 0 | 0 |

**468 passed; 0 failed.** Ignored opt-in fixtures are not counted as passes.
The new graph tests include stale handler registries, String dictionary keys,
unknown outputs, reroute retargeting, mixed variable writes, explicit numeric
conversion, compatible legacy numeric inputs and bounded inference over a graph
of more than 100,000 nodes. Editor tests cover a single type/palette authority,
effective diagnostics, compatible replacement of specialized keys, English
labels, clipping and Unicode-safe ellipsis.

The installed CLI passed strict checks and fresh source linking on copies of all
six packaged examples. All **50 packaged graphs**, schema 5, match fresh
generation from that CLI. Source snapshots equal `main.rvn`; relinking leaves
authored source unchanged. Global counters remain Integer, global strings remain
String, and untyped callback parameters remain Wildcard across the handlers.

## Native visual verification

The exact installed editor was launched with `--remote`, isolated settings and
owned temporary documents. The user's existing editor and projects were untouched.
Both owned processes (81342 and 81825) exited normally after `/gq`; no QA window
was left running. Ordinary screenshot captures succeeded, and `/log` contained
no errors. The final idle-frame grab in `/gq` timed out on this Linux backend,
but its quit completed with process exit 0; it is not used as visual evidence.

Verified at 1280 × 650:

- `"width"` String capsule, wire and Index/key input are all magenta.
- Integer counter, literal `1` and addition input/output use the same turquoise.
- A stale magenta `value` getter now appears gray, matching its Wildcard inspector
  and parameter scope; no fake default `0` is presented as the callback argument.
- English technical names fit inside the French type picker and variable rows.
- A real source-linked project shows `Project · shared` for the counter, with the
  same Integer type in the narrative and `slider_key` handler. Handler validation
  succeeds and generated RVN is available.

The wider 1920 × 976 and short 1280 × 650 French/English label checks used the
release build with identical label/rendering code. They cover String/Text
inspectors, searchable type picker, long-name ellipsis and F2 rename bounds.

Historical machine-local evidence, not distributed by the public engine
repository (temporary directories may be cleaned by the system):

- `/tmp/makepad-remote/rust-vn-editor-81342/grab-w0-00001.png` — dictionary key.
- `/tmp/makepad-remote/rust-vn-editor-81342/grab-w0-00004.png` — Integer addition.
- `/tmp/makepad-remote/rust-vn-editor-81342/grab-w0-00005.png` — unknown parameter.
- `/tmp/makepad-remote/rust-vn-editor-81342/grab-w0-00006.png` — English type names.
- `/tmp/makepad-remote/rust-vn-editor-81825/grab-w0-00002.png` — shared scope.
- `/tmp/makepad-remote/rust-vn-editor-81825/grab-w0-00008.png` — handler validated.
- `/tmp/rvn-type-coherence-editor-tests-20261002.log`.
- `/tmp/rvn-type-coherence-graph-final-20261002.log`.
- `/tmp/rvn-type-coherence-cli-tests-20261002.log`.
- `/tmp/rvn-type-coherence-install-20261002.log`.
- `/tmp/rvn-beta2-package-audit.KG3AHZ4g` — six isolated example copies and audit.

Copies of the key screenshots and this report were retained with the locally
verified installed package under `type-coherence-validation/`. These private
session files are not published here and are not public evidence for another
build.

## Limits / Limites

This run certifies the focused editor correction on Linux, not a new Windows or
browser certification. Type colors apply to data pins, wires and value accents;
node-title categories have a separate convention, as in Unreal. No Unreal assets
were copied and this report is not an exhaustive pixel-perfect certification.
Previous feature/runtime evidence remains separate and applies only to its
identified binaries and exports.

Ces essais ne certifient pas Windows ni les navigateurs pour cette mise à jour.
Ils ne prétendent pas à une parité complète avec Ren’Py. Les vérifications
précédentes du moteur restent identifiées séparément.
