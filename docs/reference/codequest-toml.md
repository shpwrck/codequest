# `CODEQUEST.toml` contract

`CODEQUEST.toml` is an optional, versioned cartridge manifest at the root of a
git repository. The application reads and validates it when the repository is
inserted, compiles its scene graph, and passes the resulting finite-state
machine to the Bevy engine. Invalid configuration refuses the cartridge with an
`INVALID CODEQUEST.toml` error. Unknown fields are rejected so misspellings
cannot silently change a game.

The complete executable v2 example is
[`docs/examples/CODEQUEST.toml`](../examples/CODEQUEST.toml). The Rust tests and
the game-designer validator both parse it, so the example and engine contract
cannot drift without failing verification.

## Runtime support

| Field | Runtime behavior |
|---|---|
| `schema_version` | `2` enables executable scenes and transitions. Legacy schema `1` remains readable as metadata. |
| `game.type` | Selects the trusted `quiz` or `quest` gameplay systems and limits which scene handlers are valid. |
| `game.title` | Overrides the repository-directory cartridge title when present. |
| `game.summary` | Validated and retained as design metadata. |
| `game.start_scene` | Names the first scene in the executable v2 machine. |
| `scenes[].handler` | Selects a trusted Rust scene implementation. It does not load code from the cartridge. |
| `scenes[].transitions` | Routes semantic engine signals to target scenes, with optional timing gates. |
| `mechanics` | Validated design requirements referenced by scenes; they do not dynamically create code. |
| `art` | Validated production requirements. An optional typed `template` selects a safe renderer asset built into CODE QUEST; entries without it remain metadata. |

Without `CODEQUEST.toml`, the engine builds the same finite-state machine from
its quiz or quest template. A repository with `CODEQUEST.md` uses the legacy
quest-battle detection; any other repository uses quiz mode. A manifest takes
precedence over that detection.

The manifest is data only. It cannot provide JavaScript, load engine plugins,
replace the game loop, or execute arbitrary conditions. Quest commands continue
to come from the engine's existing repository inspection.

## Top-level contract

```toml
schema_version = 2

[game]
type = "quiz"                 # required: "quiz" or "quest"
title = "MY CODE QUEST"       # optional
summary = "What players do."  # optional
start_scene = "title"         # required in v2
```

Scenes, mechanics, and art are arrays of tables. IDs must be non-empty and
unique within their array. Every reference must resolve.

## Executable scenes

A v2 scene separates three concerns:

- `kind` is author-facing classification such as `title`, `challenge`, or
  `reward`.
- `handler` chooses a built-in renderer and gameplay behavior.
- `transitions` say where the machine goes when that handler emits a semantic
  signal.

This means a designer can reorder scenes, create loops, insert another instance
of a handler, or change timing without adding another hard-coded screen enum.

```toml
[[scenes]]
id = "quiz"
title = "Quiz"
kind = "challenge"
handler = "concept-quiz"
summary = "Test one durable concept about the project."
mechanics = ["answer-question"]
art = ["quiz-frame"]

[[scenes.transitions]]
signal = "needs-question"
target = "oracle"

[[scenes.transitions]]
signal = "hearts-empty"
target = "game-over"
```

Each signal may appear at most once per scene. A transition may set
`after_ticks` to prevent that signal from advancing until the scene has been
active for the given number of 60 fps ticks. The `elapsed` signal is generated
by the scene clock and always requires `after_ticks`:

```toml
[[scenes.transitions]]
signal = "continue"
target = "title"
after_ticks = 90

[[scenes.transitions]]
signal = "elapsed"
target = "title"
after_ticks = 330
```

Cycles and branches are allowed. Every declared scene must be reachable from
`game.start_scene`; terminal scenes may omit transitions.

### Handlers and signals

| Handler | Game family | Signals it can emit |
|---|---|---|
| `repository-credits` | shared | `continue`, `elapsed` |
| `opening-fanfare` | shared | `continue`, `elapsed` |
| `title` | shared | `continue` |
| `quiz-menu` | quiz | `new-run`, `back` |
| `character-creation` | quiz | `hero-ready`, `back` |
| `oracle` | quiz | `questions-ready`, `back` |
| `concept-quiz` | quiz | `needs-question`, `batch-complete`, `hearts-empty`, `back` |
| `level-up` | quiz | `questions-ready`, `needs-question` |
| `game-over` | quiz | `replay` |
| `quest-select` | quest | `quest-selected`, `back` |
| `battle` | quest | `victory`, `defeat` |
| `victory` | quest | `continue` |
| `defeat` | quest | `continue` |

A transition using a signal that its handler cannot emit is rejected at
cartridge load time. Quiz-only handlers are rejected in quest games and vice
versa.

## Mechanics

A mechanic captures reusable rules and feedback. These fields are currently
validated design metadata; `inputs`, `rules`, and `feedback` default to empty
lists.

```toml
[[mechanics]]
id = "answer-question"
summary = "Choose one of four answers."
inputs = ["d-pad", "a", "b"]
rules = ["Exactly one answer is correct.", "A wrong answer costs one heart."]
feedback = ["Reveal correctness immediately."]
```

## Art requirements

Art entries name production needs. `kind` is author-defined, `requirements`
defaults to an empty list, and the optional `template` field selects a trusted
visual template compiled into the application. A cartridge can choose and reuse
these templates, but cannot provide executable drawing code or read arbitrary
asset paths.

```toml
[[art]]
id = "quiz-frame"
kind = "ui"
summary = "Question, answer, score, streak, and heart presentation."
template = "oracle-trial"
requirements = ["Fits 240x160.", "Keeps focus visible."]
```

The built-in Oracle template catalog is:

- `oracle-chronicle`
- `oracle-awakening`
- `oracle-title`
- `oracle-menu`
- `oracle-atelier`
- `oracle-hero`
- `oracle-sanctum`
- `oracle-trial`
- `oracle-ascension`
- `oracle-aftermath`
- `oracle-progression`

Scene handlers activate a visual template only when the current scene references
the corresponding art entry. `oracle-hero` and `oracle-progression` are shared
systems: once selected by the cartridge, they preserve the chosen hero and
Initiate/Adept/Oracle-bound visual tier across the scenes that use them. Unknown
template names are rejected instead of falling back silently.

Renderer changes can emit all thirteen reachable Oracle scenes—including the
five-beat opening story—as native 240×160 PPM files for visual review without
packaging the application:

```bash
CQA_VISUAL_PREVIEW_DIR=/tmp/codequest-previews \
  cargo test --manifest-path src-tauri/Cargo.toml \
  oracle_templates_produce_nine_distinct_native_scene_frames --lib
```

## Schema v1 compatibility

Schema v1 remains supported so existing cartridges continue to load. In v1,
scenes use `next = ["scene-id"]`; the graph, mechanics, and art are validated
and retained as metadata, while the engine uses its built-in machine template.
V1 cannot use `handler` or `transitions`. V2 cannot use `next`.

## Validation rules

- `schema_version` must be `1` or `2`.
- `game.type` must be `quiz` or `quest`.
- IDs cannot be blank or duplicated within their category.
- All scene, mechanic, and art references must resolve.
- `art[].template`, when present, must name a built-in visual template.
- Every v2 scene must be reachable from `game.start_scene`.
- V2 handlers must belong to the selected game family, transitions must use a
  signal their handler emits, and duplicate signal routes are rejected.
- `elapsed` transitions require `after_ticks`.
- Optional title/summary text cannot be blank when present. Required display
  and summary fields cannot be blank.
- Unknown keys at every level are errors.

Adding a new field, handler, signal, or game type is a contract decision and
must arrive with engine behavior, an updated example, and parser tests.
