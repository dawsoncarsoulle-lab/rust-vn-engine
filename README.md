# rust-VN engine

A Rust visual novel runtime, scripting language and command-line toolchain.

**In active development.** This repository contains the engine, graph/UI document libraries and developer tools. The separately distributed rust-VN Editor has its own licensing terms and private source repository.

[Editor downloads](https://github.com/dawsoncarsoulle-lab/rust-vn-releases/releases) · [Installation guide](docs/getting-started.md) · [Language reference](docs/language-reference.md) · [Project structure](docs/project-structure.md)

## Features

- Dialogue, choices, variables, conditions, labels and call/return.
- Characters, backgrounds, music, effects and transitions.
- Save/load, persistent progress, rollback and localization.
- Custom menu documents and narrative UI.
- A CLI for project creation, checks, desktop builds and Web exports.
- Graph import/transpilation and language-server tooling.

These are implemented capabilities, not a claim that every combination or platform is production-ready.

## Build from source

Install a Rust toolchain. Linux builds require desktop/audio development packages; for Debian/Ubuntu, start with `pkg-config` and `libasound2-dev`.

```sh
cargo build --release -p rvn_cli -p rvn_bevy
cargo test --release -p rvn_parser -p rvn_core -p rvn_graph -p rvn_ui -p rvn_cli
./target/release/rvn --help
```

Keep the `rvn` CLI and `rvn_bevy` runtime together when packaging. For Web builds, install the `wasm32-unknown-unknown` target and a wasm-bindgen CLI compatible with the version in Cargo.lock. Consult the CLI help for available build options.

The packaged editor includes its runtimes; its users do not need Rust/Cargo.

## Validation status

Linux has received selected automated and graphical checks. Windows binaries are cross-compiled; a previous candidate received partial Windows 11 testing, but that does not validate every newer package. Complete clean-system Linux/Windows qualification, Firefox/Web coverage, update recovery and external-user testing remain incomplete.

The release notes in the [distribution repository](https://github.com/dawsoncarsoulle-lab/rust-vn-releases/releases) identify the exact shipped versions and their limitations. Save regular backups and keep user projects separate from application installations.

Known reports still under investigation include typewriter behaviour after loading and example translation warnings. See [window-close validation](docs/window-close-validation-2026-09-23.md) for the scope of that particular fix.

## Components

| Crate | Role |
| --- | --- |
| rvn_parser | Language parser and script representation |
| rvn_core | Story execution, variables, saves and localization |
| rvn_bevy | Graphical/audio runtime |
| rvn_cli | Project and export commands |
| rvn_graph | Graph documents and script conversion |
| rvn_ui | Menu documents and interaction models |
| rvn_lsp | Language-server integration |

## License and commercial games

The engine is available under [MIT](LICENSE-MIT). Existing MIT OR Apache-2.0 grants remain valid; [Apache-2.0](LICENSE-APACHE) is retained for previously dual-licensed code. Third-party components and media keep their own licenses.

You may use the engine in free or paid games without rust-VN royalties. Your own stories and assets do not become MIT merely because the game uses this engine. Preserve applicable runtime/dependency notices and obtain rights for all included media.

Historical demo assets are still undergoing provenance review. Do not assume the source-code license grants rights to every image, font or audio file. Publication/redistribution of those assets must wait for documented permission.

## Contributions and bug reports

Describe the version, system, reproduction steps and expected/actual behaviour. Do not upload private projects, secrets or personal information.

Changes should include focused tests where possible. Contributions to original engine code should be offered under MIT; preserve existing third-party notices. Avoid unrelated formatting changes or claims of platform support without matching test evidence.

This repository's history includes experimental editor prototypes; these are not the current proprietary editor product.
