# Getting Started with RVN

This guide walks you through creating your first visual novel with RVN, from installation to a playable game.

---

## Prerequisites

- [Rust](https://rustup.rs/) (stable). On Linux, also install `pkg-config` and ALSA headers (`sudo apt install pkg-config libasound2-dev` on Debian/Ubuntu).

RVN is not yet on crates.io. Install from source:

```bash
cargo install --path rvn_cli
```

This gives you the `rvn` command-line tool and the `rvn-lsp` language server.

---

## Create a Project

```bash
rvn new my_game
cd my_game
```

This generates **not a skeleton, but a complete playable demo** — _La Clairière des Échos_ — that exercises every major feature of the engine. It is designed to be readable and hackable: read through it, then replace the content with your own story.

## Validate Before Running

```bash
rvn check .
```

This runs the static analyzer over your entire project. It catches parse errors, unknown label targets, missing assets, missing locale keys, dead code, unreachable labels, and more — all before a single frame is rendered. Run it often.

Add `--strict` to treat warnings as errors (useful in CI):

```bash
rvn check --strict .
```

## Run in Development

```bash
rvn run .
```

This launches the game. In `dev` mode, the game hot-reloads when you save a `.rvn` script file — no restart needed.

## Build for Desktop

```bash
rvn build .
```

Outputs a standalone binary to `dist/<name>-<platform>/`.

## Build for the Web

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli
rvn build --target web .
```

Outputs a self-contained web directory to `dist-web/<name>/`. Serve it with any static server:

```bash
python3 -m http.server --directory dist-web/my_game/
```

---

## Your First Scene

Open `scripts/main.rvn` and replace its contents:

```rvn
// characters.rvn
init {
    character.create("alice", "Alice")
}

// main.rvn
use "characters.rvn"

label start
    scene "backgrounds/forest.png" with fade
    music.play("theme.ogg") with fade

    alice.show("neutral") at center with dissolve
    alice "Hello! This is my first RVN scene."
    alice "Every line I say appears in the textbox with my name."

    choice {
        "Ask about the forest" => {
            alice.show("happy") at center with dissolve
            alice "This forest has stood here for centuries."
        }
        "Say goodbye" => {
            alice.show("sad") at center with dissolve
            alice "Already? Well, the forest will be here when you return."
        }
    }

    alice.hide() with dissolve
    music.stop() with fade
```

Run `rvn check .` to validate, then `rvn run .` to play it.

---

## Key Concepts

### Scripts and Labels

Your game is a graph of `.rvn` files connected by `use` directives. Execution begins at the `start` label (configured in `rvn.toml`). `jump` moves between labels; `call`/`return` creates subroutines.

### Characters

Declare characters in an `init` block with an ID and display name. Use the ID in script statements: `alice "dialogue"`, `alice.show("emotion")`, `alice.move()`.

### Variables and Branching

Variables are set with `set` and tested with `if`. They persist for the duration of a playthrough. Choices and conditionals let you branch the story.

```rvn
set trust = true
if trust {
    alice "Thank you for trusting me."
}
```

### Assets

Place images in `assets/backgrounds/`, `assets/sprites/<character>/`, `assets/cgs/`, and audio in `assets/music/` and `assets/sfx/`. `rvn check` verifies that every asset referenced in your scripts exists on disk.

### Localization

Dialogue text and choice labels are automatically used as locale keys. Place translations in `locales/<lang>.toml` under a `[strings]` table. `rvn check` flags missing and unused keys.

```toml
# locales/en.toml
[strings]
"Hello! This is my first RVN scene." = "Bonjour ! C'est ma première scène RVN."
```

### Theming

The title screen, textbox, choice buttons, and colors are all configurable in `theme.toml` — no code required. See [Project Structure](project-structure.md) for details.

---

## Editor Support

### Zed

Install the `rvn-zed-extension` (shipped in this repo). It provides syntax highlighting, snippets, and automatic `rvn-lsp` integration (hover, completion, go-to-definition, rename, quickfix).

### VSCode

A VSCode extension is available with syntax highlighting and snippets for `.rvn` files.

### Language Server

```bash
cargo install --path rvn_lsp
```

Configure your editor to use `rvn-lsp` for `*.rvn` files. Capabilities: diagnostics, hover, completion, go-to-definition, find references, rename, and code actions.

---

## Next Steps

- Read the [Language Reference](language-reference.md) for every statement and expression.
- Read [Project Structure](project-structure.md) to understand `rvn.toml`, `theme.toml`, and the directory layout.
- Study the demo project (`scripts/`) — it demonstrates every feature in context.
- Run `rvn check .` early and often. It is your fastest feedback loop.
