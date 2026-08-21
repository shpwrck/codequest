# `CODEQUEST.toml` contract

`CODEQUEST.toml` is an optional, versioned cartridge manifest at the root of a
git repository. The application reads and validates it when the repository is
inserted, then passes the typed configuration to the Bevy engine. Invalid
configuration refuses the cartridge with an `INVALID CODEQUEST.toml` error;
unknown fields are rejected so misspellings cannot silently change a design.

The complete v1 example is [`docs/examples/CODEQUEST.toml`](../examples/CODEQUEST.toml).
It is parsed by the Rust test suite, so the example and loader cannot drift
without failing CI.

## Runtime support in schema version 1

| Field | Runtime behavior today |
|---|---|
| `schema_version` | Must be `1`. Other versions are rejected. |
| `game.type` | Selects `quiz` or `quest` mode. |
| `game.title` | Overrides the repository-directory cartridge title when present. |
| `game.summary` | Validated and retained as design metadata. |
| `game.start_scene` | Validated against `scenes` and retained as design metadata. |
| `scenes`, `mechanics`, `art` | Validated as a connected storyboard and passed to the engine, but do not construct screens dynamically yet. The current copyright/fanfare sequence is implemented as hard-coded engine states. |

Without `CODEQUEST.toml`, existing behavior remains unchanged: a repository
with `CODEQUEST.md` uses quest-battle mode and any other repository uses quiz
mode. A manifest takes precedence over that legacy detection.

The manifest is data only. It cannot provide JavaScript, load engine plugins,
or replace the game loop. Quest commands continue to come from the engine's
existing repository inspection.

## Top-level contract

```toml
schema_version = 1

[game]
type = "quiz"                 # required: "quiz" or "quest"
title = "MY CODE QUEST"       # optional
summary = "What players do."  # optional
start_scene = "title"         # required when [[scenes]] exist
```

The remaining top-level arrays are optional. IDs must be non-empty and unique
within their array. Every reference must resolve.

## Scenes

A scene describes one player-facing beat and links the storyboard together.
`kind` is an author-defined classification such as `title`, `menu`,
`character-creation`, `interstitial`, `challenge`, `reward`, or `result`.

```toml
[[scenes]]
id = "quiz"
title = "Quiz"
kind = "challenge"
summary = "Test one durable concept about the project." # optional
mechanics = ["answer-question"] # optional references
art = ["quiz-frame"]            # optional references
next = ["oracle", "game-over"] # optional scene references
```

When at least one scene exists, `game.start_scene` is required and must name a
scene. Cycles and branches are allowed because games commonly revisit a scene
or have success and failure paths.

## Mechanics

A mechanic captures the reusable rules and feedback that make a scene
playable. `inputs`, `rules`, and `feedback` default to empty lists.

```toml
[[mechanics]]
id = "answer-question"
summary = "Choose one of four answers."
inputs = ["d-pad", "a", "b"]
rules = ["Exactly one answer is correct.", "A wrong answer costs one heart."]
feedback = ["Reveal correctness immediately."]
```

## Art requirements

Art entries name the assets a design needs without coupling the contract to a
particular image-generation or asset pipeline. `kind` is author-defined;
`requirements` defaults to an empty list.

```toml
[[art]]
id = "quiz-frame"
kind = "ui"
summary = "Question, answer, score, streak, and heart presentation."
requirements = ["Fits 240x160.", "Keeps keyboard focus visible."]
```

## Validation rules

- `schema_version` must equal `1`.
- `game.type` must be `quiz` or `quest`.
- IDs cannot be blank or duplicated within their category.
- `game.start_scene`, scene transitions, mechanic references, and art
  references must point to declared IDs.
- Optional title/summary text cannot be blank when present. Scene titles and
  kinds, mechanic summaries, and art kinds/summaries cannot be blank.
- Unknown keys at every level are errors.

These rules define the authoring contract. Adding a new field or game type is
a schema-version decision and should arrive with engine behavior, an updated
example, and parser tests.
