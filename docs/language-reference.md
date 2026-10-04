# RVN Language Reference

This document describes the `.rvn` scripting language in full. It is the authoritative reference for every statement, expression, and directive the engine understands.

If you are new to RVN, read [Getting Started](getting-started.md) first — this page is a reference, not a tutorial.

For the new source-linked visual workflows, use these bilingual guides:
[reusable interfaces and Designer](programmable-ui-authoring.md),
[advanced animation curves, splines and scene preview](advanced-animations.md),
and [layer discovery, variants and RVN selectors](layered-characters-advanced.md).
These guides are also included beside this reference in the installed examples.

En français : les guides ci-dessus expliquent le Designer d’interfaces,
les trajectoires et courbes avancées, et les personnages multicouches avec
découverte d’images, variantes et fonctions de sélection. Le RVN et les
Blueprints modifient les mêmes définitions ; les limites y sont précisées.

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

### Collection edits / Modification des listes

Collection functions return a **new list**. Assign the result with `set` to
change game state; ordinary saves and rollback then include the change.
Indices start at zero. Invalid indices, types or argument counts are errors,
not silently clamped values. `list_slice` excludes the end index.

Ces fonctions renvoient une **nouvelle liste** : affectez le résultat avec
`set` pour modifier l'état du jeu. Les sauvegardes et le retour arrière
conservent ces changements. Les indices commencent à zéro. Un indice, un
type ou un nombre d'arguments invalide produit une erreur. La borne finale
de `list_slice` est exclue.

| Function / Fonction | Result / Résultat |
|---|---|
| `list_append(list, value)` | Append one item / Ajouter un élément à la fin |
| `list_insert(list, index, value)` | Insert before index; index may equal length / Insérer avant l'indice, y compris à la fin |
| `list_remove(list, index)` | Remove one item by index / Retirer l'élément à cet indice |
| `list_set(list, index, value)` | Replace one existing item / Remplacer un élément existant |
| `list_concat(list, other)` | Concatenate two lists / Concaténer deux listes |
| `list_slice(list, start, end)` | Extract a range / Extraire une plage |

```rvn
init { set inventory = ["letter"] }
label start
    set inventory = list_append(inventory, "key")
    if contains(inventory, "key") {
        narrator "Items: [len(inventory)]"
    }
    return
```

In Blueprints, use the **Function** node, set its RVN function name and wire
the list/value/index arguments, then connect its result to a variable setter.
`+` adds arguments. Import now supports calls, list expressions and index
access, including nested expressions. List and dictionary expressions in
`init` are represented by connected value nodes.

Dans les Blueprints, utilisez le nœud **Fonction**, renseignez son nom RVN,
reliez les arguments puis sa sortie à une affectation de variable. Le bouton
`+` ajoute des arguments. L'import prend aussi en charge les expressions de
listes et les accès par index. Les expressions de listes et dictionnaires
dans `init` sont représentées par des nœuds de valeurs connectés.

### Functions and loops / Fonctions et boucles

```rvn
function total(items, bonus) {
    set result = bonus
    for item in items { set result = result + item }
    return result
}

init { set prices = [3, 5, 8] }
label start
    set cost = total(prices, 2)
    "Total: [cost]"
    set count = 0
    while count < 3 { set count = count + 1 }
```

`function` declarations are top-level. Parameters and variables assigned in
a function are local; they do not alter game variables. Functions may read
game variables and call other calculation functions. A reached `return`
must supply a value. Dialogue, choices, timers, persistent writes and other
narrative commands are not permitted in calculations. Narrative `call` and
bare `return` remain separate from function calls and `return expression`.

`for name in list { ... }` visits a snapshot of a list; `while condition {
... }` checks the condition before every iteration. Narrative loops may
contain dialogue and choices, unlike calculation functions. Execution
without interaction is bounded to 100,000 computation steps, calculation
calls to depth 64 and narrative calls to depth 128. Exceeding a limit is an
error instead of freezing the game. Integer overflow is also an error.

Dans une `function`, les paramètres et les affectations sont locaux : ils ne
modifient pas les variables du jeu. La fonction peut lire ces variables et
appeler d'autres fonctions de calcul, mais pas afficher un dialogue,
présenter un choix ou modifier les données persistantes. Un `return` atteint
doit fournir une valeur. `Call` / `Return` narratifs restent distincts.
`for` parcourt une copie de la liste ; `while` vérifie sa condition à chaque
tour. Les limites d'exécution et d'appels produisent une erreur explicite.

In Blueprints, **Graphs → + → New computation function** creates a separate
function graph. Select its entry to edit the name and comma-separated
parameters in Details. Connect a **Return value** node to its execution flow
and result. **While** and **For each** have separate body and completed pins.
Use a **Function** value node to call the calculation from a story or another
function. Parameters are available as local variable references.

Dans les Blueprints : **Graphes → + → Nouvelle fonction de calcul**. Sélectionnez
l'entrée pour régler son nom et ses paramètres dans Détails. Reliez le
nœud **Retourner une valeur** au flux et au résultat. Les nœuds **Tant que**
et **Pour chaque** séparent le corps et la sortie après la boucle.

### Dictionaries / Dictionnaires

```rvn
init { set quest = {"name": "Find the letter", "done": false} }
label start
    set quest = dict_set(quest, "done", true)
    "[quest[\"name\"]]"
```

Keys are strings. Duplicate literal keys and missing indexed keys are errors.
Operations return copies, as with lists. `dict_get(map, key, fallback)` uses
the fallback if a key is absent; `dict_at(map, key)` and `map[key]` are strict.
`dict_set(map, key, value)` inserts/replaces a key; `dict_remove(map, key)`
removes it. `dict_keys(map)` and `dict_values(map)` return lists in sorted-key
order. `len(map)` counts keys and `contains(map, key)` checks membership.

Les clés sont des textes. Les opérations renvoient une copie.
`dict_get` accepte une valeur de remplacement ; `dict_at` et `map[key]`
signalent une clé manquante. `dict_keys` / `dict_values` suivent l'ordre trié
des clés. Dans les Blueprints, le nœud **Index / key** accepte un indice
entier de liste ou une clé texte de dictionnaire.

### Reusable screens and handlers / Interfaces et gestionnaires

```rvn
screen name_prompt(title) {
    return component("prompt", "column", {"width": 560, "padding": 24}, [
        component("heading", "text", {"text": title}, []),
        component("name", "input", {"binding": "player_name",
            "accessible_label": "Your name"}, []),
        component("done", "button", {"text": "Done",
            "events": {"click": "close_prompt"}}, [])
    ])
}
handler close_prompt(event) { ui.close(event["screen"]) }
init { set player_name = "Camille" }
label start
    ui.open("name_prompt", ["Who are you?"], true, 1)
    ui.focus("name_prompt", "name")
    "Welcome, [player_name]."
```

A `screen` is a calculation that returns exactly one root component. Parameters
and local calculations do not modify game variables. Use functions, dictionaries,
`if`, `for` and `while` to build reusable and conditional trees. `component(id,
kind, properties, children)` builds the same description used by Blueprints.
Component IDs must be unique within a screen and stable across redraws; list
positions are usually not suitable identities when the list can be reordered.

Supported kinds are `panel`, `column`, `row`, `text`, `image`, `button`, `input`,
`select`, `toggle` and `slider`. Only containers accept children. Input bindings
name game variables: input → text, select → a stable option string, toggle →
Boolean, slider → a number in `[min, max]`. Unbound control values also survive
save/load and temporary conditional hiding. `options` contain saved values;
`option_labels` or `option_keys` contain display labels/translations.

`ui.open(name, arguments, modal, layer)` opens or replaces a named instance.
When called by a source game-menu presenter, the new instance belongs to that
presenter: its children close together, and these temporary menu screens are
excluded from narrative saves and rollback. `ui.open_story(name, arguments,
modal, layer)` explicitly opens a narrative instance instead. It takes the
same four arguments and checks, persists with the story and survives closing
the presenter. Use it when a quick action opens an inventory, map or other
independent screen. It grants no additional save, load or confirmation rights.
Higher layers appear above lower layers; later instances resolve equal layers.
`ui.close(name)` closes an instance and `ui.focus(name, element)` requests focus
on an enabled, visible control. A modal screen prevents story advancement and
interaction with lower screens. Multiple nonmodal screens can coexist.

A `handler name(event)` accepts one event dictionary: `screen`, `element`,
`kind`, `value` and `key`. Declare local temporary values with `local`;
`set` changes a game variable unless that name is a parameter or declared local.
Handlers may calculate, change bindings and open/close/focus screens, but cannot
block on dialogue, choices or a narrative call. Failure or an execution limit
leaves the previous game state intact. Calculation functions, screen functions,
handlers and narrative `call`/`return` are distinct operations.

Component `events` map `click`, `activate`, `focus`, `change` and `key` to handler
names. Activation falls back to `click` if no `activate` handler exists. Root
`open` and `close` events run once per lifecycle operation, not every redraw.
Keyboard events first use the focused control's handler, otherwise the root's
handler. Default Tab/Shift+Tab navigation follows `focus_order` (negative values
exclude a control), then authored order. Disabled/hidden ancestors exclude all
their children from interaction and focus.

Geometry uses the existing 1920×1080 reference: optional `rect: [x,y,w,h]` for
absolute placement, otherwise flex containers with `width`, `height`, `padding`
and `spacing`. `scroll` accepts `none`, `vertical`, `horizontal` or `both`.
Top-level flow containers automatically scroll when the window cannot fit them.
`foreground`/`background` are RGBA lists in `[0,1]`; `font_size` is positive.
Image resources must stay relative to the asset directory, without parent
traversal or external URLs. The checker reports missing literal image paths;
the renderer reports asynchronous resource failures instead of silently hiding them.

Localize `text`, `placeholder` and `accessible_label` using `text_key`,
`placeholder_key` and `accessible_label_key`. A translation can interpolate game
variables. Language changes refresh open screens without resetting bindings.
Descriptions are bounded to 512 components, 32 levels and 256 selection options.
The game allows at most 32 open screen instances. An unsupported renderer
produces a capability error before committing the open operation.

Dans une `screen`, les calculs restent locaux et renvoient un composant racine.
Construisez ses enfants avec fonctions, listes, dictionnaires, conditions et
boucles. Gardez des identifiants uniques et stables. `binding` lie un contrôle à
une variable du jeu ; les valeurs des sélections restent indépendantes des
libellés traduits. `ui.open`, `ui.close` et `ui.focus` gèrent les écrans, leur
modalité, leur couche et leur focus. Un gestionnaire reçoit un dictionnaire
d'événement et ne peut pas bloquer l'histoire ; une erreur conserve l'état
précédent. Les événements, traductions, dimensions et limites ci-dessus sont
identiques dans le script et dans la description compilée des Blueprints.

In the development editor, use **Graphs → + → New reusable screen** or **New
event handler**. Screens return a **UI component** through **Return value**;
handlers have an event parameter and execution flow. Component Details expose
type, identity, binding, value/options, translations, layout and event handlers.
**Open interface**, **Close interface** and **Focus interface element** are
narrative/handler nodes. List and dictionary nodes construct repeated content.
The existing game-menu designer and its canvas are not replaced.

Dans l'éditeur de développement : **Graphes → + → Nouvelle interface
réutilisable** ou **Nouveau gestionnaire d'événement**. L'interface renvoie un
**Composant d'interface** via **Retourner une valeur**. L'inspecteur expose
identité, type, liaison, contrôles, traductions, disposition et événements.
Le concepteur de menus existant et son canvas restent disponibles. Voir
[`examples/interfaces`](../examples/interfaces/README.md) pour l'inventaire,
le journal et le puzzle. Ces ajouts sont en développement, pas encore une
version publique validée sur toutes les plateformes.

### Composable animations / Animations composables

Named RVN functions can return seekable animation descriptions. The same
descriptions and sampler are used by desktop, Web, Blueprints and the editor
preview; they do not rely on the number of rendered frames.

```rvn
function arrive(seconds) {
    return motion_parallel([
        motion_tween(seconds, {"x":-160,"opacity":0}, {"x":100,"opacity":1}, "ease_out"),
        motion_tween(seconds, {"rotation":-10}, {"rotation":0}, "ease_in_out")
    ])
}
label start
scene "room.png"
iris.show("neutral") at left
motion.play("sprite:iris", arrive(2))
"The animation continues during this dialogue."
motion.wait("sprite:iris")
"The animation has finished."
motion.stop("sprite:iris")
```

Targets are `background`, `sprite:<character>` and
`ui:<screen>/<component>`. The target must currently exist. Frame playback
on an interface requires an image component. Hiding a target cancels its
animation; a later reappearance does not resurrect the cancelled track.

| Constructor | Behaviour |
| --- | --- |
| `motion_tween(seconds, from, to, curve)` | Interpolate the specified channels; omitted start channels capture the preceding pose. |
| `motion_sequence([…])` | Run steps in order, inheriting their final poses. |
| `motion_parallel([…])` | Run independent channels together; conflicting writes are rejected. |
| `motion_pause(seconds)` | Hold the preceding pose. |
| `motion_repeat(times, motion)` | Repeat; `0` means endless. Each cycle restarts from its captured base. |
| `motion_frames(["one.png", "two.png"], fps)` | Play asset-relative images in order. |

Tween channels: `x`, `y` (reference pixels), `scale_x`, `scale_y`, `rotation`
(clockwise degrees), `opacity`, `tint_r`, `tint_g`, `tint_b`, `tint_a`,
`pivot_x`, `pivot_y`. Scale must be positive; opacity, tint and normalized
pivot channels range from 0 to 1. Curves: `linear`, `ease_in`, `ease_out`,
`ease_in_out`. A completed animation holds its last pose. `motion.stop`
removes its contribution and restores the ordinary target appearance.
Replacing a track captures its current pose, so omitted starting channels
do not jump back to the original pose. Waiting for an endless track is an
error, not a narrative freeze. Handlers may play and stop tracks, but may
not wait for them.

Clocks, captured poses and pending narrative waits are saved and restored
by loading and rollback. Execution limits reject malformed descriptions,
non-finite values, conflicting parallel channels, unreachable steps after
an endless repeat and unsafe asset paths. Missing images produce a runtime
diagnostic. The renderer must explicitly support this capability.

Blueprints provide **Play / Stop / Wait for animation**, **Tween**, **Pause**,
**Sequence**, **Parallel**, **Repeat** and **Frames** nodes. The tween inspector
lets you enable channels and edit their values without replacing connected
computed expressions. **Animation preview** supports play/pause, scrubbing,
arrow-key seeking and temporary function parameters. Parameter edits affect
only the preview, not the story. Preview currently uses a geometric card or
the authored frame images, not a full running scene. See
[`examples/animations`](../examples/animations/README.md).

Les fonctions nommées renvoient les mêmes descriptions en RVN et en
Blueprints. Les séquences, animations parallèles, pauses, répétitions et
images successives utilisent un échantillonnage commun sur desktop et Web.
`motion.play` démarre sans bloquer ; `motion.wait` attend une animation finie ;
`motion.stop` restaure l'apparence normale. La disparition d'une cible annule
son animation. Sauvegarde, chargement et retour arrière restaurent les
horloges, les poses et l'attente narrative. L'inspecteur propose les canaux
de transformation et un aperçu avec lecture/pause, chronologie et paramètres
temporaires. Une ressource manquante, une cible invalide ou une attente
infinie produit une erreur explicite. Ce lot est encore en validation et
n'est pas une nouvelle version publique.

### Portable videos / Vidéos portables

Native builds enable `rvn_bevy/video` (or `rvn_cli/video`) and use the separately
built shared FFmpeg runtime from `tools/build-ffmpeg-lgpl.sh`. Web exports use the
browser media APIs. The supported initial format is **WebM, VP8 video and optional
Vorbis audio**, not arbitrary `.mp4` files. Admission checks reject other codecs,
external paths and malformed headers before playback. Exported desktop games
include the shared libraries, original corresponding sources, build script,
notices and integrity manifests; they do not require a system FFmpeg command.

Les versions natives activent `rvn_bevy/video` ou `rvn_cli/video`. Le format
initial pris en charge est **WebM VP8 avec audio Vorbis facultatif**. Les exports
Web utilisent le navigateur. Les bibliothèques FFmpeg partagées et leurs sources,
licences, script de construction et empreintes accompagnent les jeux desktop.
Les autres codecs ne sont pas annoncés comme compatibles.

```rvn
init { set movies_finished = 0 }
handler movie_end(event) { set movies_finished = movies_finished + 1 }
function introduction() {
    return video_clip("movies/intro.webm", {
        "rect": [320, 80, 640, 360],
        "volume": 0.7,
        "skippable": true,
        "keep_last_frame": true,
        "poster": "movies/poster.png",
        "fallback": "movies/unavailable.png",
        "on_end": "movie_end",
        "subtitles": [
            {"start": 0, "end": 1.5, "text": "intro.subtitle"}
        ]
    })
}
label start
video.play("intro", introduction())
"Dialogue can continue while the video plays."
video.pause("intro")
video.seek("intro", 0.8)
video.volume("intro", 0.3)
video.resume("intro")
video.wait("intro")
"The finite video has completed."
video.stop("intro")
```

`video_clip(source, properties)` is a portable value. The commands `video.play`,
`pause`, `resume`, `stop`, `skip`, `seek`, `volume` and `wait` are narrative
operations, not calls allowed inside pure calculation functions. Their Blueprint
nodes use typed Video Clip pins. Every operation is importable/exportable; a
literal clip exposes resource search, playback flags, volume, layer, poster,
fallback, mask, interface target, handler references and rectangle controls.
Computed properties stay connected expressions and are not overwritten by an
inspector edit. See `examples/videos` for an original procedural test clip.

`video_clip` produit une valeur réutilisable. Les commandes vidéo sont des
opérations narratives : elles ne sont pas utilisables dans les fonctions de
calcul pur. Les nœuds Blueprint correspondants possèdent des pins typés. Les
propriétés calculées restent des expressions connectées ; l’inspecteur ne les
remplace pas silencieusement. L’exemple `examples/videos` utilise une vidéo de
test procédurale originale.

Property / Propriété | Meaning / Signification
--- | ---
`rect` | `[x,y,width,height]` in the authored 1280×720 scene / dans la scène de référence
`layer` | Scene order −1024…1024 / ordre d’affichage
`target` | `ui:<screen>/<component>` embeds in an open interface / intégration dans une interface ouverte
`cinematic` | Finite blocking scene video; cannot loop or target UI / cinématique bloquante, sans boucle ni cible UI
`skippable` | Allows `video.skip` and Space/Enter during a cinematic / saut autorisé
`looping` | Loops until explicitly stopped; do not wait for automatic completion / boucle jusqu’à l’arrêt explicite
`keep_last_frame` | Keeps the finished picture without restarting its sound / conserve la dernière image sans relancer le son
`poster`, `fallback` | Project-relative waiting/error images / images d’attente et de remplacement
`mask` | Separate synchronized grayscale VP8 WebM with identical dimensions and sufficient duration / masque vidéo synchronisé
`subtitles` | Ordered, non-overlapping `start`/`end` seconds and localized `text` keys / sous-titres ordonnés et localisables
`on_end`, `on_error` | RVN event-handler names; terminal callback emitted once / gestionnaires appelés une seule fois par fin ou erreur

Positions, pause state, descriptors and the last-frame state are included in
versioned saves and rollback. Decoder handles and audio objects are never saved.
Loading resumes with one player/voice; loading an ended clip restores its picture
without replaying sound or the end event. Closing a targeted interface cancels
its video. A browser autoplay refusal pauses narration and presents a localized,
keyboard-focusable resume button instead of pretending playback succeeded.

Les sauvegardes et le retour arrière conservent la position, la pause et les
descriptions vidéo. Un chargement recrée un seul lecteur ; une vidéo terminée ne
relance ni son audio ni son événement de fin. Fermer l’interface cible annule sa
vidéo. Un refus de lecture automatique du navigateur affiche un bouton de reprise
localisé et accessible au clavier.

Resource limits: 1 GiB per local file, 3840×2160 decoded pixels, 24 hours per
clip, 1024 subtitle cues and a bounded decoder queue. Unsupported or missing
resources produce diagnostics/error events; they are not silent successes.
Native Linux playback and browser playback have been exercised. A successful
cross-build of the Windows decoder is **not** evidence of a Windows runtime
test; platform release validation remains separate.

### Saves and random state / Sauvegardes et hasard

`random(min, max)` / `rand(min, max)` include both integer endpoints. The
random stream is saved and restored by loading and rollback. Rendering an
interpolated dialogue repeatedly does not consume another gameplay draw.
A failed calculation does not advance the random stream.

Extended saves record a version and a prepared-story identity. Saves from a
changed story are rejected without replacing the current state. Whitespace
and comments do not change that identity. Legacy saves remain readable with
default values and an explicit **compatibility unchecked** result. This is
not a guarantee that an old save still makes sense after changing a story.

Le hasard est conservé par la sauvegarde et le retour arrière. Un affichage
répété ne tire pas une nouvelle valeur. Les sauvegardes étendues vérifient
l'identité de l'histoire ; une histoire modifiée est refusée sans remplacer
l'état courant. Les anciennes sauvegardes restent lisibles, avec une
indication explicite que leur compatibilité ne peut pas être confirmée.

### Source editing APIs / API d'édition du code source

`rvn_parser::parse_spanned` exposes top-level statement byte ranges.
`SourceDocument::replace_statement` preserves surrounding bytes and rejects
concurrent changes, invalid edits, injected statements and loss of internal
comments. An unchanged statement keeps its original formatting.
`rvn_graph::reimport_script` preserves graph identities and matches node
signatures to retain node/pin/edge identities and layout. Duplicate signatures
are matched in original ID order; renamed labels currently count as new graphs.

The development editor enables **explicit source linking** from the Scripts
panel. Save existing graphs first, choose a self-contained RVN file inside
the project and confirm. The original graph files remain on disk.
The CLI equivalent is `rvn blueprint link "project folder"`, with an optional
`--source main.rvn` project-relative path. This explicit operation can prepare a
script-only project for opening from the editor's home screen. It preserves
authored source and refuses to replace an existing link.
`.rvn-authoring.json` stores the linked filename, last valid source snapshot
and graph presentation. `.rvn-backups` contains safety copies. Authored RVN
is changed only when an explicit visual save passes loss-checked validation.
Compile/preview do not overwrite it. Untouched scopes remain byte-identical;
changed scopes retain existing comments. Invalid external source retains the
last valid graph and reports its source error.

Unsaved graph recovery is offered explicitly when reopening. It includes
incomplete graphs and layout and does not change authored RVN until Save.
A conflicting source or second editing session prevents silent replacement;
recovery can instead be archived in project backups. Interrupted paired
source/presentation writes are recovered using a journal. Reopen after
external edits; dirty tabs require confirmation before reload.

Character declarations can be added, renamed and removed visually from any
linked narrative graph. They are shared across tabs; peer undo histories retain
the updated shared registry rather than restoring an obsolete cast. Removal is
refused if a declaration is still referenced, including in another graph.
Unchanged declarations and their comments retain their original bytes.

Current boundaries: linking unresolved `use` imports, visual scope renaming
and interleaved declarations are refused rather than regenerated.
Multiple authored initialization blocks require
editing in RVN. New scopes and deletion of unreferenced scopes are supported;
adding a label cannot alter an existing implicit fallthrough. These are
development capabilities, not a published final release.

L'éditeur de développement propose une liaison explicite depuis **Scripts**.
Après confirmation, le RVN devient la source ; les anciens fichiers de
graphe restent sur disque. Les commentaires sont conservés et les blocs
non modifiés restent identiques. Une erreur dans le script conserve le
dernier graphe valide. Les copies de sécurité et la récupération protègent
les modifications ; aucun conflit n'est écrasé silencieusement. La liaison
des imports non résolus, le renommage visuel des blocs et les déclarations
entrelacées sont encore refusés explicitement. Les personnages peuvent être
ajoutés, renommés ou supprimés depuis un graphe narratif lié ; leur registre
reste commun aux onglets, y compris après annulation. La suppression d'un
personnage encore référencé est refusée. Aucun ancien projet
n'est converti automatiquement et aucune version publique n'est remplacée.

### Statement table

| Statement | Syntax |
|---|---|
| Use import | `use "path"` or `use "dir/*"` |
| Init block | `init { ... }` |
| Create character | `character.create("id", "Name")` |
| Dialogue | `character_id "text"` or `"narrator text"` |
| Choice | `choice { "option" => { ... } }` |
| Set variable | `set name = expression` |
| Calculation function | `function name(parameters) { ... return expression }` |
| While loop | `while expression { ... }` |
| Iterate list | `for name in expression { ... }` |
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

## Accessibility / Accessibilité

```rvn
accessibility.configure({"text_scale":1.5,"high_contrast":true,"reduced_motion":true})
accessibility.speak("Welcome")
accessibility.stop()
```

`configure` replaces the project's complete policy: omitted properties take
their defaults. It accepts `self_voicing` (default `false`), `text_scale`
(default `1`, range `0.75–2.5`), `high_contrast` and `reduced_motion` (both
default `false`), `speech_rate` (default `1`, range `0.5–2`), `speech_volume`
(default `1`, range `0–1`) and optional `language` (a language tag such as
`fr-FR`). Invalid properties and values produce a diagnostic before changing
game state. These three operations also work in event handlers, but not in
calculation functions or expressions. Their Blueprint equivalents are
**Accessibility settings**, **Speak text** and **Stop speech**.

The player can open the built-in preferences with **F8**, use **Tab / Shift+Tab**
or **↑ / ↓** to navigate, **← / →** to adjust, **Enter** to activate and
**Escape / F8** to close. **V** toggles self-voicing outside editable text
fields. The panel is modal: closing it does not advance the story. Player
preferences are saved independently of narrative saves and override project
defaults; resetting them restores the project's current policy. Story saves
and rollback restore the authored policy without replaying spoken requests.

`speak` resolves localized text, reads it as literal text and can be used even
when automatic self-voicing is off. Automatic reading covers dialogue, choices
and focused controls. Speech is optional: Linux uses an installed Speech
Dispatcher service, Windows uses installed SAPI voices, and Web uses browser
speech synthesis. Missing services or a voice for the requested language
produce a visible error. Availability and voice quality differ by system.
No audio recordings or identical cross-platform voices are bundled.

For reusable screen components, set `accessible_label` or the localized
`accessible_label_key`; selection controls separate stable `options` values
from translated `option_labels` / `option_keys`. `focus_order` controls
navigation, with negative values excluding a component from keyboard focus.
Hidden, disabled and modal-covered controls cannot be activated. Browser text
fields keep native selection, clipboard and IME behavior; assistive actions
use the same validated event path as pointer input.

En français : `configure` remplace tous les réglages du projet ; les propriétés
omises reprennent leurs valeurs par défaut. **F8** ouvre les préférences du
joueur, **Tab / Maj+Tab** ou **↑ / ↓** change le contrôle, **← / →** règle sa
valeur, **Entrée** l'active et **Échap / F8** ferme le panneau sans avancer
l'histoire. Les préférences du joueur sont conservées séparément et prennent
le pas sur le projet. `speak` lit un texte localisé ; `stop` arrête la lecture.
Une voix absente produit un message visible, pas une réussite silencieuse.
Les libellés accessibles et l'ordre de navigation se règlent aussi dans les
propriétés des composants Blueprints.

The bilingual [accessibility example](../examples/accessibility/README.md)
contains a keyboard-operated inventory. These are development capabilities:
native Windows, actual browser/platform combinations and screen-reader
conformance still require release validation. Do not interpret a successful
cross-compilation as a Windows or accessibility certification.

## Blueprint String and Text conversions / Conversions Chaîne et Texte

Blueprint **String** and **Text** (`InterpolatedText`) are distinct pin types.
Connecting them adds a visible **String → Text** or **Text → String** conversion,
with matching types on each cable. Numbers first become String, then Text;
Text first becomes String before integer conversion. These are one atomic
visual edit, so Undo/Redo includes the complete conversion chain.

`string_to_text(value)` and `text_to_string(value)` are explicit one-argument
RVN functions. They accept only string values and preserve the payload verbatim,
including Unicode, empty strings and template markers. They do not stringify
numbers, translate text, or recursively evaluate marker contents. RVN currently
uses a string runtime representation for both: Blueprint Text is a narrative
template, not Unreal's FText with an independent translation identity.
Dialogue interpolation and localization keep their existing rules and keys.
Type-only casts in a dialogue can preserve its RVN source verbatim; the linked
authoring metadata retains those visible casts and their positions.

Graph schema 3 upgrades earlier direct String/Text wires in memory by inserting
visible compatibility casts. Existing node/pin identities and positions remain
unchanged; the destination cable keeps its identity. Legacy compatibility casts
retain the original RVN expression and interpolation. Files are not overwritten
by loading; save through the existing backup/transaction workflow. Old builds
cannot read schema 3 graphs.

En français : **Chaîne** et **Texte** sont maintenant deux types distincts.
Leur connexion ajoute un convertisseur visible, dans une seule action
annulable. Les fonctions RVN `string_to_text(valeur)` et `text_to_string(valeur)`
exigent une chaîne et en conservent exactement le contenu ; elles ne traduisent
pas le texte et n'interprètent pas récursivement les marqueurs. Les règles
d'interpolation et les clés de traduction des dialogues restent inchangées.
Un Texte Blueprint reste un modèle RVN, pas un FText Unreal doté de sa propre
identité de traduction. Le schéma 3 migre les anciennes connexions en mémoire,
sans déplacer les nœuds ni écrire automatiquement dans le projet. Les anciens
exécutables ne peuvent pas lire les graphes de schéma 3.

## Advanced authoring schema / Schéma d’édition avancée

Graph schema 4 adds the optional **Variants and rules** input to legacy
layered-image nodes and changes the animation Curve input from String to Any
so it can carry either an existing preset or a typed curve descriptor. Existing
nodes, pins, edges and positions are retained; an unconnected options input
still emits the original three-argument `layered_image`. Loading migrates only
in memory. Saving uses the existing safety-copy/source transaction; keep those
backups if you need to return to an older executable. Builds predating schema 4
cannot open these new graphs.

En français : le schéma 4 ajoute le pin optionnel **Variantes et règles** aux
anciennes compositions et rend le pin Courbe générique pour accepter soit une
courbe prédéfinie, soit une description de courbe. Identifiants, câbles et
positions existants sont conservés. Sans options connectées, le RVN garde les
trois arguments d’origine. La migration à l’ouverture reste en mémoire ;
l’enregistrement passe par les copies de sécurité et la transaction existantes.
Un ancien exécutable ne peut pas ouvrir un graphe de schéma 4.

## Programmable 2D components / Composants 2D programmables

`component(id,"canvas",properties,[])` accepts `draw` (user function name,
exactly three parameters: state, props, frame), dictionary `props`, dictionary
initial `state`, and boolean `capture_pointer`/`consume_input` (default true).
Drawing is pure and returns lists built with `canvas_rect` (3 arguments),
`canvas_ellipse` (2), `canvas_line` (3), `canvas_polygon` (2), `canvas_text` (4),
`canvas_image` (2), `canvas_group` (3), `canvas_hit` (2). The shared geometry
supports nested affine transforms, clipping and invisible hit regions.

`ui.set_state(screen,element,dictionary)` is a handler/narrative command, never
a calculation expression. Canvas pointer_down/move/up/cancel, wheel, tick and
key events expose local state/props/frame. Literal callback names, arity and
purity are statically checked; dynamic references retain runtime checks. Save
format 9 retains local state and simulation time, hydrating absent older data.
Graph schema 5 adds primitive/state nodes; loading preserves existing nodes,
IDs, pins and positions, with source-first backup protection on saving.

En français : Canvas utilise une fonction de dessin pure à trois paramètres
state/props/frame, des primitives 2D et des gestionnaires d’événements. La
commande `ui.set_state(écran,élément,dictionnaire)` remplace l’état d’une instance
sans modifier les autres. Les sauvegardes 9 conservent état et temps ; le schéma
5 ajoute les nœuds sans redessiner ni déplacer les anciens. Voir le
[contrat complet FR/EN](programmable-components.md) et les limites explicites :
ceci n’est ni Python, ni des shaders natifs arbitraires, ni une parité Ren’Py.

### Source game-menu presenters

A menu document can opt into a source `screen` for each `source_screens` key:
`title`, `pause`, `save`, `load`, `settings`, `gallery`, `history`, `confirm`,
`dialogue`, `choices`, and `quick_actions`. An absent mapping keeps the existing
menu page. The editor changes this association through the menu document's
normal undo and save operations; it does not convert that document into a
snapshot of evaluated components.

Each presenter is a zero-argument screen or a screen taking one context
argument. The context contains the real role and origin (`title` or
`in_game`), preferences, save slots and protection/compatibility, dialogue,
visible choices, history, unlocked gallery entries and the current confirmation
token. `diagnostic` contains a local host error, or an empty string. It is input
for expressions and functions, not an editable game-state snapshot. Designer
sample data is only a preview.

Pure constructors return a nominal `MenuRequest`: `menu_action`,
`menu_start_scene`, `menu_open_page`, `menu_slot`, `menu_protect`,
`menu_save_page`, `menu_number`, `menu_bool`, `menu_language`, `menu_advance`,
`menu_skip_typewriter`, `menu_choose`, `menu_gallery_cg`, `menu_gallery_tab`,
`menu_confirm`, and `menu_cancel`. Bind a request in a control's `event_data`,
then call `menu.execute(event["data"]["request"])` from its handler. Ordinary
live narrative screens may use these requests too. A screen function remains
pure; it cannot execute a request while producing components.

The host checks request types, ranges, actual game phase and the exact living
screen instance before committing the handler's globals, random state or UI.
Confirmation requires the real `confirm` presenter and its exact, non-replayable
token. A request from a closed screen, retired presenter or changed choice list
is rejected. Rejection keeps the game usable and exposes a diagnostic; a valid
following action clears it. Save/load and preferences use the existing game
operations and protection rules. A presenter cannot supply arbitrary file
paths or acquire extra authority by editing its context.

Choice indices are zero-based indices of the visible list. Conditions filter
both displayed labels and destinations. The first visible option has index 0
and numerical shortcut 1, even when earlier authored responses are hidden.
System modal presenters and their children stay above narrative screens;
dialogue, choices and quick actions remain below a narrative modal inventory
or map. These transient presentations do not consume narrative screen order
or enter a story save.
