# Exported interface integration fixture

This is an owned test project, not a user project. It exercises reusable styles,
font inheritance, exact small typography, local/nested rectangles, wrapping, event data, bindings,
native browser inputs, keyboard focus, quicksave/quickload and missing-resource
diagnostics. The test runner opens the exported game independently of the editor.

Copy the three project files into a new temporary directory. Supply a licensed
DejaVu Serif font as `assets/fonts/DejaVuSerif.ttf`; on Debian/Ubuntu it is
typically `/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf`. The binary is not
vendored here. Preserve its license if distributing the test export.

Build from the engine workspace:

```sh
cargo run --release -p rvn_cli -- build --target web /absolute/temporary/project
node tools/test-web-interface-export.mjs /absolute/temporary/project/dist-web/Interface_authoring_Web_QA /absolute/evidence/directory
```

The runner needs `playwright` and `pngjs`, system Chrome and Playwright Firefox
(`playwright install firefox`). It resolves normal Node dependencies first;
`RVN_QA_NODE_MODULES` may point to an existing dependency directory. On this
desktop it also recognizes the bundled workspace dependency runtime. Set
`RVN_CHROME_BIN` when Chrome is not `/usr/bin/google-chrome`.

The runner uses local loopback HTTP and software WebGL in both real browsers,
writes package SHA-256 fingerprints, pass/fail details and screenshots, and
closes only its own browser sessions. It never publishes a game or changes an
open editor/project. Windows desktop execution is a separate test.
