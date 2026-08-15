# RVN Language Reference

This document describes the `.rvn` scripting language in full. It is the authoritative reference for every statement, expression, and directive the engine understands.

If you are new to RVN, read [Getting Started](getting-started.md) first — this page is a reference, not a tutorial.

---

## Structure

An RVN project is a collection of `.rvn` script files connected by `use` directives. One file is the **main script** (declared in `rvn.toml`); all others are pulled in via `use`.

```rvn
use "characters.rvn"
use "chapter1/*"

label start
    // ... your scene ...
```

### Comments

Single-line comments start with `//` and run to the end of the line:

```rvn
// This is a comment.
eileen "This line runs."  // This is also a comment.
```

### Indentation

Indentation is **not significant** — the parser uses keywords and braces, not whitespace, to determine structure. You can indent however you like. The convention in the template is 4 spaces per block level.

---

## `use` — Importing Scripts

Pulls other `.rvn` files into the current compilation unit. Supports single files and wildcards.

```rvn
use "characters.rvn"
use "common/setup.rvn"
use "chapter1/*"        // every .rvn in scripts/chapter1/, alphabetical order
```

Wildcard imports load every `.rvn` file in the directory, in alphabetical order. Paths are relative to the directory of the main script (or the current file for nested `use`).

Files that are not reachable from the main script via `use` are flagged as **orphan scripts** by `rvn check`.

---

## `init` — Global Initialization

An `init` block runs before any label. Use it to declare characters and set initial variable values.

```rvn
init {
    character.create("eileen", "Eileen")
    set route = "inconnue"
    set trust = false
    set found_clue = false
}
```

`init` blocks can appear in any script file. They are collected and executed in source order before the `start` label is reached.

---

## `character.create` — Declaring Characters

Creates a character with an internal ID and a display name.

```rvn
init {
    character.create("eileen", "Eileen")
    character.create("marc", "Marc le Mystérieux")
}
```

- The **ID** (`"eileen"`) is used in script statements: `eileen.show(...)`, `eileen "dialogue"`.
- The **display name** (`"Eileen"`) is shown in the textbox nameplate at runtime.

Characters must be declared before use. `rvn check` flags undefined character IDs and unused declarations.

---

## `label` — Named Anchors

A label marks a position in the script that can be jumped to or called.

```rvn
label start
    // statements here

label chapter2_intro
    // statements here
```

Labels are global across all imported scripts. No two labels may share the same name — `rvn check` flags duplicates.

The entry point is determined by `start_label` in `rvn.toml` (usually `"start"`).

---

## `jump` — Unconditional Goto

Transfers control to another label. Any statements after a `jump` in the same block are **dead code** and flagged by `rvn check`.

```rvn
jump chapter2_intro
```

---

## `call` / `return` — Subroutines

`call` jumps to a label but remembers the return position. `return` goes back to the statement after the `call`.

```rvn
label legend_intro
    eileen "Before answering, she asks that a story be told."
    call clearing_legend
    eileen "And now, the clearing can deliver its verdict."
    jump conclusion

label clearing_legend
    eileen "They say a traveler once lost the name of their village here."
    return
```

`call`/`return` uses a call stack, so subroutines can be nested.

---

## `set` — Assigning Variables

Sets a variable to the value of an expression. Variables are dynamically typed (bool, int, float, string).

```rvn
set route = "light"
set score = 42
set bonus = score * 2 + 1
set trust = true
set has_clue = found_clue and trust
```

Bare assignments without `set` (`route = "light"`) are a parse error; `rvn check` suggests adding `set`.

### Persistent Variables

Variables prefixed with `persistent.` are stored separately and survive across save/load cycles and playthroughs. Use them for unlockable routes, playthrough counters, true-ending flags, and any state that should persist between games.

```rvn
set persistent.playthroughs = 1
set persistent.trust_route_unlocked = true
set persistent.total_endings_seen = persistent.total_endings_seen + 1
```

Persistent variables can be read in any expression:

```rvn
if persistent.trust_route_unlocked {
    eileen "You've been here before."
}
```

They are stored in `persistent.json` alongside the engine's internal persistent data (seen CGs, endings, settings). Regular variables (without the `persistent.` prefix) are reset on each new game or load.

---

## Expressions

RVN has a full expression evaluator with standard precedence:

| Precedence | Operators | Description |
|---|---|---|
| Lowest | `or` | Logical OR (short-circuit) |
| | `and` | Logical AND (short-circuit) |
| | `not` | Logical NOT |
| | `==` `!=` `<` `<=` `>` `>=` | Comparison |
| | `+` `-` | Addition / subtraction |
| Highest | `*` `/` | Multiplication / division |
| | unary `-` | Negation |

Literals: integers (`42`), floats (`3.14`), booleans (`true`/`false`), strings (`"text"`), and variable references (`score`).

```rvn
set total = base + bonus * multiplier
set eligible = score >= 50 and not failed
set label_text = "Chapter " + chapter_number
```

Type coercion rules:
- `+` on two strings concatenates them.
- `+` on a string and a number converts the number to string.
- Division by zero is a runtime error.

---

## `if` / `else` — Conditional Branching

```rvn
if trust {
    eileen.show("happy") at center with dissolve
    eileen "Your trust makes the passage brighter."
} else {
    eileen.show("serious") at center with dissolve
    eileen "Your caution slows the echo, but makes it clearer."
}
```

The `else` block is optional. Conditions are expressions evaluated as booleans (non-zero numbers and non-empty strings are truthy).

---

## `choice` — Player Choices

Presents the player with a list of options. Each option has a label and a body that runs when selected.

```rvn
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

- Choice labels can contain [interpolation](#text-interpolation): `"Open the [color] door"`.
- An optional `if condition` after the label makes the choice appear only when the condition is true:

```rvn
choice {
    "Enter the vault" if has_key => {
        jump vault
    }
    "Force the lock" => {
        jump lockpick
    }
}
```

  If `has_key` is false, only "Force the lock" appears. If all options are filtered out, the choice is skipped entirely.

- `rvn check` flags duplicate choice text within the same `choice` block and empty `choice` blocks.

---

## Dialogue

A character ID followed by a string produces a line of dialogue.

```rvn
eileen "Welcome to The Clearing of Echoes."
eileen "Every decision leaves a mark more lasting than a step in the moss."
```

A string without a character ID is narrator text:

```rvn
"The official RVN demo is over."
"You can return to the title, open the gallery, and check the unlocked ending."
```

### Text Interpolation

Square brackets `[...]` inside a dialogue string are evaluated as expressions and inserted into the text:

```rvn
eileen "Current route: [route]."
eileen "Your score is [score * 2 + bonus] points."
```

### Escape Sequences

The following escape sequences are supported inside dialogue strings:

| Escape | Result |
|---|---|
| `\n` | Newline (line break within the dialogue) |
| `\t` | Tab |
| `\"` | Literal double quote |
| `\\` | Literal backslash |

```rvn
eileen "Line one.\nLine two."
eileen "She said \"hello\" and left."
```

Bevy's text rendering handles word-wrapped line breaks automatically based on the textbox width. Explicit `\n` forces a line break at that position.

### Text Tags

Inline formatting tags modify how text is rendered. Tags are enclosed in `{...}` and must be closed with `{/tagname}`:

| Tag | Syntax | Effect |
|---|---|---|
| Color | `{color=#ff6b6b}red text{/color}` | Tints the enclosed text |
| Speed | `{speed=0.6}slow text{/speed}` | Changes typewriter speed for the enclosed text |
| Shake | `{shake}shaking text{/shake}` | Applies a shake effect |
| Pause | `{pause=0.4}` | Pauses the typewriter for N seconds |

```rvn
eileen "There is one thing I must admit: {color=#ff6b6b}the clearing remembers everything{/color}."
eileen "Wait{pause=0.4}... listen."
eileen "{speed=0.6}Some echoes only answer words spoken slowly.{/speed}"
eileen "{shake}And some do not want to be woken.{/shake}"
```

`rvn check` validates all text tags — malformed tags, invalid colors, and unclosed tags are flagged with file:line diagnostics.

---

## `scene` — Background Change

Changes the displayed background image with an optional transition.

```rvn
scene "backgrounds/clearing_day.png" with dissolve
scene "backgrounds/forest.png" with fade
```

Paths are relative to the `assets/` directory. `rvn check` verifies that the file exists on disk (with extension `.png`, `.jpg`, `.jpeg`, or `.webp`).

---

## Sprite Commands

### `show` — Display a Character Sprite

```rvn
eileen.show("neutral") at left with dissolve
eileen.show("happy") at center
eileen.show("think")
```

- **Emotion** (`"neutral"`, `"happy"`, ...): selects the sprite image from `assets/sprites/<character>/<emotion>.png`.
- **Position** (`at left/center/right` or `at(0.35)`): where on screen the sprite appears. `left` = 0.2, `center` = 0.5, `right` = 0.8. Custom positions use a 0.0–1.0 float.
- **Transition** (`with dissolve/fade`): how the sprite appears.

If position is omitted, the sprite keeps its current position (or defaults to center if new).

### `hide` — Remove a Character Sprite

```rvn
eileen.hide() with dissolve
```

### `move` — Move a Sprite to a New Position

```rvn
eileen.move() at center with dissolve
eileen.move() at left
```

### `animate` — Play a Sprite Animation

```rvn
eileen.animate("shake")
eileen.animate("bounce", loop: true, duration: 2.0, height: 30)
eileen.animate("pulse", duration: 1.5, scale: 1.1)
```

Supported animations and their parameters:

| Animation | Parameters | Description |
|---|---|---|
| `shake` | `loop`, `duration`, `intensity` | Horizontal shake |
| `bounce` | `loop`, `duration`, `height` | Vertical bounce |
| `pulse` | `loop`, `duration`, `scale` | Scaling pulse |

### `stop_animation` — Stop All Sprite Animations

```rvn
eileen.stop_animation()
```

---

## Audio Commands

### `music.play` — Play Background Music

```rvn
music.play("theme.ogg") with fade
music.play("music/battle.mp3")
```

Paths are relative to `assets/`. Music loops by default. If the path starts with `music/`, it is used as-is; otherwise `music/` is prepended. Crossfades are applied when transitioning between tracks.

### `music.stop` — Stop Music

```rvn
music.stop() with fade
```

### `music.volume` — Set Music Volume

```rvn
music.volume(0.5)
```

Volume is a float from `0.0` (silent) to `1.0` (full).

### `sfx.play` / `sfx.stop` — Sound Effects

```rvn
sfx.play("click.ogg")
sfx.stop("click.ogg")
```

Sound effects do not loop. Supported formats: `.ogg`, `.mp3`, `.wav`, `.flac`.

---

## `cinematic` — CG Display

Shows a full-screen cinematic (CG) image, typically for key story moments.

```rvn
cinematic "demo" with fade
eileen "The clearing keeps this memory in its light."
cinematic hide with fade
```

- `cinematic "id"`: displays `assets/cgs/<id>.png`.
- `cinematic hide`: hides the current CG.
- Viewing a CG automatically unlocks it in the **CG gallery**.

---

## `unlock_ending` — Register an Ending

Marks an ending as unlocked for the **endings gallery**.

```rvn
unlock_ending "trust_end"
unlock_ending "caution_end"
```

---

## `imagemap` — Clickable Background Regions

Displays a background with clickable hotspots. Each hotspot has a rectangular area and a body that runs when clicked.

```rvn
imagemap {
    background: "backgrounds/clearing_explore.png"
    hover: "backgrounds/clearing_explore_hover.png"

    hotspot {
        name: "path"
        area: (0, 0, 426, 720)
    } => {
        eileen "The path disappears under the ferns."
    }

    hotspot {
        name: "stone"
        area: (426, 0, 853, 720)
    } => {
        set found_clue = true
        eileen "The stone bears three engraved lines."
    }
}
```

- `background` and `hover` are optional string properties (paths relative to `assets/`).
- Each `hotspot` has a `name` (optional, for display) and an `area` (required, as `(x1, y1, x2, y2)` in pixels).
- The `=> { ... }` block runs when the hotspot is clicked.
- `rvn check` detects overlapping hotspots and empty imagemaps (no hotspots).

---

## `typewriter` — Text Display Control

Controls the typewriter effect for dialogue.

```rvn
typewriter.speed(42)     // 42 characters per second
typewriter.enabled(true) // turn the effect on
typewriter.enabled(false) // turn the effect off (instant text)
```

---

## Transitions

Transitions control how visual changes (scenes, sprites, music) appear:

| Transition | Description | Default duration |
|---|---|---|
| `fade` | Fade to/from black | 500 ms |
| `dissolve` | Cross-fade between states | 300 ms |
| `slideleft` | Slide in from the right / out to the left | 400 ms |
| `slideright` | Slide in from the left / out to the right | 400 ms |
| `slideup` | Slide in from the bottom / out to the top | 400 ms |
| `slidedown` | Slide in from the top / out to the bottom | 400 ms |
| `zoomin` | Scale up from small to normal | 400 ms |
| `zoomout` | Scale from normal to large and fade | 400 ms |
| `wipe` | Wipe transition (alpha-based) | 500 ms |
| `blur` | Blur transition (alpha-based) | 400 ms |
| *(none)* | Instant change | — |

```rvn
scene "backgrounds/forest.png" with fade
eileen.show("neutral") at left with dissolve
music.play("theme.ogg") with fade
```

---

## Positions

| Position | Normalized X | Syntax |
|---|---|---|
| Left | 0.2 | `at left` |
| Center | 0.5 | `at center` |
| Right | 0.8 | `at right` |
| Custom | 0.0–1.0 | `at(0.35)` |

---

## Complete Statement Reference

| Statement | Syntax |
|---|---|
| Use import | `use "path"` or `use "dir/*"` |
| Init block | `init { ... }` |
| Create character | `character.create("id", "Name")` |
| Dialogue | `character_id "text"` or `"narrator text"` |
| Choice | `choice { "option" => { ... } }` |
| Set variable | `set name = expression` |
| If/else | `if expr { ... } else { ... }` |
| Label | `label name` |
| Jump | `jump label` |
| Call | `call label` |
| Return | `return` |
| Scene | `scene "path" with transition` |
| Show sprite | `char.show("emotion") at position with transition` |
| Hide sprite | `char.hide() with transition` |
| Move sprite | `char.move() at position with transition` |
| Animate | `char.animate("name", param: value, ...)` |
| Stop animation | `char.stop_animation()` |
| Play music | `music.play("file") with transition` |
| Stop music | `music.stop() with transition` |
| Music volume | `music.volume(level)` |
| Play SFX | `sfx.play("file")` |
| Stop SFX | `sfx.stop("file")` |
| Cinematic show | `cinematic "id" with transition` |
| Cinematic hide | `cinematic hide with transition` |
| Unlock ending | `unlock_ending "id"` |
| Imagemap | `imagemap { background: "...", hotspot { ... } => { ... } }` |
| Typewriter speed | `typewriter.speed(cps)` |
| Typewriter toggle | `typewriter.enabled(bool)` |
