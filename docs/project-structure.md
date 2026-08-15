# Project Structure

An RVN project is a directory containing a `rvn.toml` manifest, script files, assets, locales, and a theme configuration. This document describes every file and directory the engine expects.

---

## Directory Layout

```
my_game/
├── rvn.toml              # Project manifest
├── theme.toml            # UI and title screen configuration
├── scripts/              # .rvn script files
│   ├── main.rvn          # Entry script (declared in rvn.toml)
│   ├── common/
│   │   ├── characters.rvn
│   │   └── setup.rvn
│   └── chapter1/
│       ├── 01_introduction.rvn
│       ├── 02_exploration.rvn
│       └── ...
├── assets/               # All game assets
│   ├── backgrounds/      # .png, .jpg, .jpeg, .webp
│   ├── sprites/          # sprites/<character>/<emotion>.png
│   ├── cgs/              # Cinematic images
│   ├── music/            # .ogg, .mp3, .wav, .flac
│   ├── sfx/              # Sound effects
│   └── ui/               # Textbox images, etc.
├── locales/              # Translation files
│   ├── en.toml
│   └── fr.toml
└── saves/                # Save files (created at runtime)
```

---

## `rvn.toml` — Project Manifest

The root configuration file. Required.

```toml
[project]
title = "My Visual Novel"
main_script = "scripts/main.rvn"
start_label = "start"

[window]
width = 1280
height = 720

[paths]
assets = "assets"
locales = "locales"
theme = "theme.toml"
saves = "saves"
```

### `[project]`

| Field | Type | Default | Description |
|---|---|---|---|
| `title` | string | — | Game title (shown on title screen if no `[title_screen.title]` text is set) |
| `main_script` | string | `"scripts/main.rvn"` | Entry script file |
| `start_label` | string | — | Label where execution begins (usually `"start"`) |

### `[window]`

| Field | Type | Default | Description |
|---|---|---|---|
| `width` | int | — | Window width in pixels |
| `height` | int | — | Window height in pixels |

### `[paths]`

All paths are relative to the project root.

| Field | Default | Description |
|---|---|---|
| `assets` | `"assets"` | Directory for images and audio |
| `locales` | `"locales"` | Directory for translation `.toml` files |
| `theme` | `"theme.toml"` | Theme configuration file |
| `saves` | `"saves"` | Directory for save files (created at runtime) |

---

## `theme.toml` — UI Configuration

Controls the visual style of the textbox, choice buttons, and title screen. All colors are hex `#RRGGBB` or `#RRGGBBAA`.

### `[textbox]`

```toml
[textbox]
background_color = "#00000000"
height           = 174.0
padding          = 72.0
image_path       = "ui/textbox2.png"    # optional background image
```

### `[text.name]` and `[text.dialogue]`

```toml
[text.name]
font_size = 24.0
color     = "#BFEBC8"
# font_path = "fonts/name.ttf"   # optional custom font

[text.dialogue]
font_size = 22.0
color     = "#F6F1E8"
# font_path = "fonts/dialogue.ttf"
```

### `[choice]`

```toml
[choice]
background_color = "#18251FDD"
text_color       = "#F8F5ED"
number_color     = "#F0C85A"
font_size        = 20.0
# font_path = "fonts/choice.ttf"
```

### `[title_screen]`

```toml
[title_screen]
enabled      = true
music        = "music/theme_intro.ogg"
button_order = ["continue", "new_game", "load", "settings", "gallery", "quit"]
```

Valid buttons: `continue`, `new_game`, `load`, `settings`, `gallery`, `quit`.

### `[title_screen.background]`

```toml
[title_screen.background]
path = "backgrounds/title_forest.png"
mode = "cover"    # cover | contain | stretch
```

### `[title_screen.title]`

```toml
[title_screen.title]
anchor    = "top_center"
offset_x  = 0.0
offset_y  = 78.0
font_size = 56.0
color     = "#F8F5ED"
```

Valid anchors: `top_left`, `top_center`, `top_right`, `center_left`, `center`, `center_right`, `bottom_left`, `bottom_center`, `bottom_right`.

If `text` is omitted, the title falls back to `[project].title` from `rvn.toml`.

### `[title_screen.buttons]`

```toml
[title_screen.buttons]
anchor   = "center"
offset_x = 0.0
offset_y = 54.0
spacing  = 10.0
width    = 304.0
height   = 48.0
font_size = 22.0
```

### `[title_screen.buttons.visibility]`

```toml
[title_screen.buttons.visibility]
continue  = true
new_game  = true
load      = true
gallery   = true
settings  = true
quit      = true
```

### `[title_screen.buttons.labels]`

Custom text for each button. These are also used as locale keys if localization is active.

```toml
[title_screen.buttons.labels]
continue  = "Continue"
new_game  = "New Game"
load      = "Load"
gallery   = "Gallery"
settings  = "Settings"
quit      = "Quit"
```

### `[title_screen.buttons.style]`

```toml
[title_screen.buttons.style]
background_color = "#14211CCC"
hover_color      = "#20392FEE"
pressed_color    = "#315B49FF"
text_color       = "#F8F5ED"
```

`rvn check` validates theme fields: color format, anchor names, background modes, button names, font sizes, and flags deprecated keys.

---

## Assets

All assets live under the `assets/` directory (or whatever `[paths].assets` specifies).

### Backgrounds

Place in `assets/backgrounds/`. Referenced by `scene "backgrounds/forest.png"` in scripts.

Supported formats: `.png`, `.jpg`, `.jpeg`, `.webp`.

### Sprites

Place in `assets/sprites/<character_id>/<emotion>.png`. For example, `assets/sprites/eileen/happy.png` is loaded by `eileen.show("happy")`.

If no emotion is specified, the engine looks for `default.png`.

### CGs (Cinematics)

Place in `assets/cgs/`. Referenced by `cinematic "demo"`, which loads `assets/cgs/demo.png`.

### Music

Place in `assets/music/`. Referenced by `music.play("theme.ogg")` or `music.play("music/battle.mp3")`.

Supported formats: `.ogg`, `.mp3`, `.wav`, `.flac`.

### Sound Effects

Place in `assets/sfx/` (or anywhere under `assets/`). Referenced by `sfx.play("click.ogg")`.

---

## Locales

Translation files are `.toml` files in the `locales/` directory. Each file represents one language.

```toml
# locales/en.toml
[strings]
"Hello world" = "Hello world"
"Welcome" = "Welcome"
```

```toml
# locales/fr.toml
[strings]
"Hello world" = "Bonjour le monde"
"Welcome" = "Bienvenue"
```

### How It Works

1. Every dialogue line and choice label in your scripts becomes a **locale key** (the literal text).
2. At runtime, the engine looks up the key in the active locale's `[strings]` table.
3. If no translation is found, the original text is shown.
4. `rvn check` verifies that every key used in scripts has a translation in every locale file, and flags unused locale keys.

### Interpolation in Locales

Dialogue with interpolation (`"Score: [score]"`) uses the full text including brackets as the key. The `[score]` portion is still evaluated at runtime.

---

## Saves

The `saves/` directory is created at runtime. It contains:

- `slot_XX.json` — save data for each slot
- Slot metadata (chapter label, timestamp)
- Autosave, quicksave, and continue slots

Save data includes the full game state: current position, variables, sprites, music, and typewriter settings.

Persistent data (data that survives across playthroughs, like unlocked endings and CG gallery) is stored separately and is not affected by save/load.
