# RVN

**A modern visual novel engine built in Rust — for authors who think like engineers.**

RVN is a structured, tooling-first visual novel engine with a clean scripting language, a built-in static checker, a full language server, and first-class support for both desktop and web deployment. It is designed for people who want to write stories _and_ ship software — without compromising on either.

```bash
rvn new my_project
rvn check .
rvn build .
rvn build --target web .
```

## Why RVN?

Most visual novel toolkits were designed for a different era of software development. They work — but they tend to treat projects as loose collections of scripts, assets, and configuration files with no common contract between them. Errors surface at runtime. Broken asset references go unnoticed until playtest. There is no concept of project validity that can be checked independently from running the game.

RVN takes a different approach.

It treats a visual novel project as a **well-defined artifact** — one that can be parsed, resolved, validated, and built with the same discipline you'd apply to any software project. The scripting language has a proper grammar and AST. Asset references are resolved and checked. Locale coverage is verified. Control flow is analyzed. The entire project can be validated in milliseconds before a single frame is rendered.

RVN also believes that web deployment is not an afterthought. Building for the browser is a first-class target, not a plugin or an export option bolted on later.

---

## RVN vs Ren'Py

Ren'Py is a mature, proven tool with a large community. If it fits your workflow, use it.

RVN is a different kind of engine, built with different priorities:

|                             | RVN                                      | Ren'Py                |
| --------------------------- | ---------------------------------------- | --------------------- |
| **Implementation language** | Rust                                     | Python                |
| **Script validation**       | Static, before runtime                   | Runtime               |
| **Web target**              | Native (`rvn build --target web`)        | Third-party / limited |
| **Architecture**            | Layered workspace crates                 | Monolithic framework  |
| **Asset checking**          | Compile-time, with file:line diagnostics | Runtime               |
| **Locale coverage**         | Statically verified                      | Manual                |
| **Control flow analysis**   | Dead code, unreachable labels            | None                  |
| **Language server**         | Full LSP (`rvn-lsp`)                     | None                  |
| **Editor support**          | Zed (Tree-sitter) + VSCode               | Basic                 |
| **Build pipeline**          | Explicit CLI (`rvn build`)               | Integrated launcher   |

RVN is not positioned as a replacement for Ren'Py. It is a **serious alternative with a different philosophy**: more explicit, more structured, more oriented toward developer tooling, and built for modern deployment targets including the browser.

---

## Core Strengths

### Rust-Based Runtime

RVN's engine is built on Rust and [Bevy](https://bevyengine.org/), a modern, data-driven game engine. This gives RVN a performance-oriented foundation with predictable resource usage, minimal startup overhead, and a clean separation between engine concerns and script logic. The architecture is explicit and auditable — there is no hidden runtime magic.

### Static Project Validation with `rvn check`

`rvn check` is one of RVN's most distinctive capabilities, and the feature that most clearly separates it from runtime-only engines.

Before your game runs a single line, `rvn check` performs a full static analysis of your project:

- **Parse errors** — detected with precise file, line, and column information
- **Unknown jump/call targets** — references to labels that don't exist, with fuzzy-match suggestions
- **Missing assets** — backgrounds, sprites, CG images, music, and sound effects verified on disk
- **Missing locale keys** — any dialogue or choice text not covered by a locale file is flagged
- **Unused locale keys** — stale translation keys that no script refers to
- **Undefined character IDs** — using a character before declaring it
- **Unused characters and variables** — declared but never referenced
- **Unassigned variables** — used before being set
- **Dead code** — statements after an unconditional `jump` or `return`
- **Unreachable labels** — labels the engine can never reach from the entry point
- **Control flow analysis** — full reachability pass across the resolved script graph
- **Invalid text tags** — malformed `{color}`, `{speed}`, `{shake}`, `{pause}` etc.
- **Orphan scripts** — `.rvn` files in your project not included by the main script graph
- **Theme and config validation** — `theme.toml` field types, anchors, colors, deprecated keys
- **Imagemap hotspot overlap detection**
- **Duplicate label and duplicate choice text detection**

Diagnostics are emitted in the style of a modern compiler — with severity, source location, source line, caret underlining, notes, and actionable suggestions:

```
error[unknown-label-target]: label introuvable
 --> scripts/chapter1/02_exploration.rvn:47:5
   |
47 |     jump exploratoin
   |          ^^^^^^^^^^^
  = note: le label `exploratoin` n'existe pas dans le projet
  = suggestion: voulez-vous dire `exploration` ?
```

![rvn check compiler-style suggestion](.images/rvn_check_suggestion.png)

Pass `--strict` to treat warnings as errors, making `rvn check` suitable as a hard CI gate.

This means you can integrate `rvn check` into your CI pipeline, run it as a pre-build gate, and catch entire categories of errors before your testers ever see them.

### Language Server Protocol (`rvn-lsp`)

RVN ships a full LSP implementation — `rvn-lsp` — that brings IDE-grade tooling to any LSP-compatible editor.

![RVN LSP in Zed](.images/lsp.png)

Capabilities:

- **Diagnostics** — published on open, change, and save. Same analysis engine as `rvn check`.
- **Hover** — inline documentation for keywords, builtins (`music`, `sfx`, `typewriter`), character methods, labels, and variables.
- **Completion** — context-aware suggestions for keywords, label names, character ids, and methods. Triggered on `.` and space.
- **Go to definition** — jump to a `label` declaration from any `jump` or `call`.
- **Find references** — all `jump`/`call` sites for a label, or all usages of a character.
- **Document & workspace symbols** — all labels and characters, searchable across the project.
- **Rename** — atomic rename of a label or character across all project files.
- **Code actions (quickfix)** — typo suggestions for unknown labels/characters, automatic `set` insertion for bare assignments.

### Editor Extensions

#### Zed

RVN ships a native Zed extension (`rvn-zed-extension`) powered by a Tree-sitter grammar (`tree-sitter-rvn`):

- Full syntax highlighting (keywords, labels, characters, strings, text tags, interpolations)
- Bracket matching and auto-close
- Automatic indentation
- Document outline (labels and characters)
- Snippets for `label`, `choice`, `if/else`, `dialogue`
- Automatic `rvn-lsp` integration

![RVN syntax highlighting in Zed](.images/highlight.png)

#### VSCode

A VSCode extension is also available with syntax highlighting and snippets for `.rvn` files.

### Desktop + Web in a Single Command

RVN compiles to native desktop applications and to WebAssembly for browser deployment, from the same project, with the same source:

```bash
# Desktop build → dist/<name>-<platform>/
rvn build .

# Web build → dist-web/<name>/ (WASM + JS glue + index.html)
rvn build --target web .
```

The web build uses `wasm32-unknown-unknown` + `wasm-bindgen` and produces a self-contained directory ready to serve:

```bash
cd dist-web/my_project/
python3 -m http.server 8000
```

There is no separate web renderer, no special scripting mode for the browser, no assets to convert. The same `.rvn` scripts, audio, and assets that run on desktop compile cleanly to the web target.

### Themable Title Screen

The title screen is fully configurable in `theme.toml` — no code required:

```toml
[title_screen]
enabled      = true
music        = "music/theme_intro.ogg"
button_order = ["continue", "new_game", "load", "settings", "gallery", "quit"]

[title_screen.background]
path = "backgrounds/title_forest.png"
mode = "cover"  # cover | contain | stretch

[title_screen.title]
anchor    = "top_center"
offset_y  = 78.0
font_size = 56.0
color     = "#F8F5ED"

[title_screen.buttons.style]
background_color = "#14211CCC"
hover_color      = "#20392FEE"
pressed_color    = "#315B49FF"
```

<!-- SCREENSHOT: title screen in-game showing configured background, title text, and buttons -->

### Layered, Auditable Architecture

RVN is structured as a Rust workspace with clear separation of concerns:

- **`rvn_parser`** — lexer, parser, AST, `use` resolution. Error-recovering: reports multiple issues in a single pass, continues parsing after syntax errors. No runtime dependencies.
- **`rvn_core`** — engine logic, evaluator, save/load, locale, text tags, rollback. No renderer.
- **`rvn_bevy`** — Bevy-based renderer and runtime. Depends on core, not the other way around.
- **`rvn_cli`** — `rvn new`, `rvn check`, `rvn build`, `rvn run`. Orchestrates the above.
- **`rvn_lsp`** — standalone LSP server. Uses `rvn_parser` directly; no Bevy dependency.

This means the parser can be used independently, the core can be tested without a renderer, and the renderer can be swapped without touching script logic.

### Built-In VN Systems

RVN ships with production-ready implementations of the systems that visual novel engines require:

- **Save / load** — multiple save slots with metadata
- **Autosave, quicksave, quickload** — available out of the box
- **Continue** — resume from the last played position
- **Auto-advance** — configurable per-player, speed saved in persistent data
- **Persistent data** — data that survives across playthroughs
- **CG gallery** — automatically populated as cinematics are unlocked
- **Endings gallery** — track and display unlocked endings
- **Localisation** — multi-locale support (FR/EN included), statically verified
- **Text tags** — `{color=...}`, `{pause=...}`, `{speed=...}`, `{shake}` with inline validation
- **Audio** — music and SFX with real crossfade on desktop and web
- **Title screen** — fully configurable via `theme.toml`
- **Sprite animations** — `shake`, `bounce`, `pulse` with validated parameters

---

## Features

- `.rvn` scripting language with a proper grammar, lexer, AST, and error-recovering parser
- `use` directives with wildcard support (`use "chapter1/*"`)
- `rvn check` — full static project analysis with compiler-quality diagnostics, `--strict` mode
- `rvn build` — native desktop build
- `rvn build --target web` — WebAssembly + browser build via `wasm-bindgen`
- `rvn dev` — hot-reload development mode (restarts on script save)
- `rvn new` — scaffolds a complete showcase project, not a blank slate
- Full LSP server (`rvn-lsp`) — hover, completion, goto definition, references, rename, quickfix
- Zed extension with Tree-sitter grammar and `rvn-lsp` integration
- VSCode extension with syntax highlighting and snippets
- Scene management with transitions (`fade`, `dissolve`)
- Sprite system with emotions, positions (left / center / right / custom 0.0–1.0), and animations
- Imagemap support (clickable regions on backgrounds) with overlap detection
- Variable system with `set` / `if` / `choice` branching — full expression evaluator
- `init` blocks for global character and variable declarations
- `call` / `return` for reusable script subroutines
- Cinematic (CG image) display with gallery unlocking
- Rich text tags (`{color}`, `{speed}`, `{shake}`, `{pause}`) with LSP and `rvn check` validation
- Text interpolation with inline expressions (`[score * 2 + bonus]`)
- Music and SFX with real crossfade support
- Typewriter speed control, auto-advance
- Rollback support
- Multi-locale project structure with TOML locale files, statically verified

---

## Quick Start

**Prerequisites:** [Rust](https://rustup.rs/) (stable). On Linux you also need `pkg-config` and ALSA headers for audio support (e.g. `sudo apt install pkg-config libasound2-dev` on Debian/Ubuntu).

RVN is not yet published to crates.io. Install from source:

```bash
cargo install --path rvn_cli
```

```bash
# Create a new project (includes a complete playable demo)
rvn new my_project
cd my_project

# Validate the project before running anything
rvn check .

# Run in development mode (hot-reload on script save)
rvn run .

# Build for desktop
rvn build .

# Build for web (outputs to dist-web/)
rvn build --target web .
```

**Web build prerequisites:**

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli
```

Then serve `dist-web/<name>/` with any static server:

```bash
python3 -m http.server --directory dist-web/my_project/
```

**Install the language server:**

```bash
cargo install --path rvn_lsp
# Then configure your editor to use rvn-lsp for *.rvn files
# Zed: install the rvn-zed-extension (shipped in this repo) — rvn-lsp is auto-configured
```

---

## Example Script

```rvn
// characters.rvn
init {
    character.create("eileen", "Eileen")
}

// main.rvn
use "characters.rvn"
use "chapter1/*"

label start
    scene "backgrounds/forest.png" with fade
    music.play("theme.ogg") with fade

    eileen.show("neutral") at left with dissolve
    eileen "Welcome. This forest remembers everything."
    eileen "Every step, every choice — {color=#ff6b6b}it keeps them all{/color}."
    eileen "Attends{pause=0.4}... écoute."

    choice {
        "Follow the light" => {
            set route = "light"
            eileen.show("smile") at center with dissolve
            eileen "A good choice. The path opens for those who trust it."
            jump chapter1_light
        }
        "Stay and observe" => {
            set route = "shadow"
            eileen.show("think") at center with dissolve
            eileen "Wise. Ancient places reward patience."
            jump chapter1_shadow
        }
    }
```

---

## Showcase Template

`rvn new` does not generate a skeleton. It generates a **complete, playable demo project** — _La Clairière des Échos_ — that exercises every major feature of the engine:

- Multi-chapter script structure with `use` directives and wildcards
- Multiple story branches and persistent state
- Sprite emotion system and positional transitions
- Cinematic / CG display with automatic gallery unlocking
- Ending unlock system
- Localisation in English and French, statically verified by `rvn check`
- Configurable title screen with background, logo, music, and button layout
- Text tags (`color`, `pause`, `speed`, `shake`) used in context
- Save / load, autosave, quicksave, continue, auto-advance

The template is designed to be readable, not minimal. It shows how a real project should be structured, and every part of it passes `rvn check` clean.

![RVN check on the showcase template](.images/rvn_check.png)

---

## Workspace Structure

```
rvn/
├── rvn_parser/     # Lexer · Parser · AST · use resolution · error recovery
├── rvn_core/       # Engine logic · Evaluator · Save/Load · Locale · Text tags
├── rvn_bevy/       # Bevy renderer · Desktop runtime · WASM runtime
├── rvn_cli/        # rvn new · rvn check · rvn build · rvn run · rvn dev
└── rvn_lsp/        # Language server · Hover · Completion · Rename · Quickfix
```

### `rvn_parser`

The parsing layer. Takes raw `.rvn` source text and produces a typed AST. Handles `use` resolution, including wildcard glob imports. Supports **error recovery** via `parse_recovering` — the parser continues after syntax errors so `rvn check` and `rvn-lsp` can report multiple issues in a single pass. No runtime or rendering dependencies.

### `rvn_core`

The engine logic layer. Implements the evaluator, variable system, save/load serialization, rollback, persistent data, locale management, and text tag parsing. Completely independent of the renderer — this layer can be tested, fuzzed, and reused without Bevy.

### `rvn_bevy`

The rendering and runtime layer. Implements the Bevy ECS-based renderer for desktop and WASM. Receives high-level engine state from `rvn_core` and translates it into rendered frames. Audio, transitions, sprite management, and UI live here.

### `rvn_cli`

The command-line interface. Implements `rvn new`, `rvn check`, `rvn build`, `rvn run`, and `rvn dev`. `rvn check` in particular is a substantial static analysis pipeline that operates entirely independently of the Bevy runtime.

### `rvn_lsp`

The language server. Built on `tower-lsp`, it indexes the full project on workspace open and publishes live diagnostics as you type. Uses `rvn_parser` and the same analysis engine as `rvn check`. No Bevy or rendering dependency.

---

## Project Status

RVN is in **active development**. The core feature set is implemented and the engine is usable for real projects today.

What works:

- Full scripting language, parser (with error recovery), and AST
- `rvn check` with comprehensive static analysis and `--strict` mode
- Desktop runtime (Linux, macOS, Windows)
- Web runtime (WASM via `wasm-bindgen`)
- `rvn build` and `rvn build --target web`
- `rvn dev` — hot-reload development mode
- Save / load, autosave, continue, persistent data, auto-advance
- CG gallery, endings gallery
- Localisation with static verification (missing and unused keys)
- Text tags, audio with real crossfade, sprites, cinematics
- Themable title screen (background mode, anchors, button order, colors)
- Language server (`rvn-lsp`) with hover, completion, goto definition, find references, rename, quickfix
- Zed extension with Tree-sitter grammar
- VSCode extension
- Showcase template project

Current focus:

- Stability and edge case coverage in `rvn check`
- Build pipeline robustness
- Documentation and examples
- Community tooling and editor integrations

---

## Roadmap

Near-term:

- `rvn check --watch` for live validation during authoring (separate from `rvn dev`)
- Expanded animation primitives
- Video / animated background support
- Save slot metadata and thumbnail previews

Longer-term:

- Visual script graph editor
- Plugin system for custom statement types
- Expanded web deployment options (self-contained single-file HTML export)
- Mobile targets (iOS / Android via Bevy)

The roadmap is intentionally conservative. RVN will not promise features it cannot build and maintain.

---

## Design Goals

- **Explicit over magic.** Every reference in a project is resolvable. No runtime surprises.
- **Tooling as a first-class citizen.** `rvn check` and `rvn-lsp` are not optional lint steps — they are core parts of the engine.
- **Web without compromise.** Desktop and browser are equal deployment targets.
- **Layered architecture.** Parser, core, renderer, CLI, and LSP are separate concerns with clean boundaries.
- **Long-term maintainability.** The codebase is structured to be read, extended, and maintained — not just to work today.

---

## Documentation

- **[Getting Started](docs/getting-started.md)** — installation, creating a project, your first scene
- **[Language Reference](docs/language-reference.md)** — every statement, expression, and text tag
- **[Project Structure](docs/project-structure.md)** — `rvn.toml`, `theme.toml`, assets, and locales

---

## License

Licensed under either of

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)

at your option.

---

_RVN is a project built with the conviction that visual novel tooling deserves the same engineering standards as any other software. If you share that conviction — contributions, issues, and feedback are welcome._
