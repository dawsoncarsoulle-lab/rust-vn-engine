# Public source boundary

This is the public engine repository, created from the cleaned main-branch
history of the private development repository. Old pull-request references
were not imported. Legacy test media, media-bearing menu archives and old
packaged demos were excluded from this history as a precaution, not as a
finding of infringement.

The CLI now includes a text-only starter. The separately packaged editor's
full example is not replaced by this starter. Its media have separate credits.

The historical editor experiments in this repository are not the current
proprietary rust-VN Editor. Existing license grants remain valid.

DejaVu Sans is retained with `rvn_bevy/resources/DejaVuSans-LICENSE.txt`.
Third-party crate licenses remain applicable; engine MIT does not relicense
dependencies or grant rights to media supplied by users.

Development clones made before the history cleanup must not be merged or
force-pushed into this repository. Start from this repository and reapply
only the required source changes.

## Checks and remaining debt

The public-source preparation passed 296 tests across parser, core, graph, UI
and CLI, and a newly generated CLI project passed with zero errors/warnings.
This is not a native Windows or browser qualification.

The initial GitHub workflow inherited formatting and Clippy style debt. The
source was formatted; Clippy warnings remain visible in CI but are no longer
promoted wholesale to errors. Formatting, compilation and tests remain
blocking. Do not describe this repository as warning-free.
