---
name: codequest-game-designer
description: Design and revise CODE QUEST game experiences as connected storyboards, gameplay mechanics, art requirements, and valid CODEQUEST.toml manifests. Use this skill whenever the user asks to storyboard a quiz or quest, invent or change a game type, plan scenes or game flow, define gameplay rules, identify art or UI needs, or author/revise CODEQUEST.toml—even when they call it a game concept, flow, pitch, or content plan instead of a design task.
compatibility: Requires repository file access and Python 3.11+ for the bundled TOML validator.
---

# CODE QUEST game designer

Turn a game idea into one coherent, inspectable design graph. Produce both a
human-readable brief and the engine manifest so creative intent, asset work,
and implementation stay linked.

## Load the project contract first

Before designing, read these files from the repository root when present:

1. `docs/reference/codequest-toml.md` — authoritative schema and runtime support
2. `docs/examples/CODEQUEST.toml` — complete working example
3. An existing `CODEQUEST.toml` — preserve compatible IDs and user decisions
4. Relevant game code or screenshots — only when needed to distinguish current
   behavior from a proposal

If the repository contract differs from examples in this skill, the repository
contract wins. Never invent fields or game types that the current schema rejects.

## Establish the design target

Extract known answers from the conversation and existing files before asking
questions. Resolve these design inputs:

- Player fantasy: who the player feels like they are
- Player outcome: what the player should learn, practice, or accomplish
- Game loop: the repeated decision/action/reward cycle
- Session shape: approximate duration, ending, replay motivation
- Platform constraints: controls, resolution, text limits, accessibility
- Production boundary: design only, implementation-ready, or art generation too

Ask one concise scoping question only when a missing answer would materially
change the game loop or asset scope. Otherwise state a reasonable assumption and
draft something concrete the user can react to.

## Design in linked layers

### 1. Frame the experience

Write a one-paragraph pitch, three design pillars, the learning/playing outcome,
and explicit non-goals. Favor observable player behavior over mood words alone.

### 2. Define the gameplay loop

Describe:

- Core loop: what repeats every 10–90 seconds
- Progression loop: what changes across a run
- Success, failure, recovery, and replay conditions
- Resources and state the player can understand
- Inputs and feedback for each player decision

Every mechanic should create a decision, enforce a rule, or communicate useful
feedback. Remove mechanics that only rename a screen transition.

### 3. Storyboard the scenes

Give each scene a stable kebab-case ID. For every scene capture:

- Purpose in the player's emotional or learning arc
- Entry state and information the player already has
- Player goal and available actions
- Mechanics active in the scene
- State changes, feedback, and exit conditions
- Next scenes, including failure and back paths
- Art/UI requirements referenced by stable IDs

Check that every scene is reachable from `game.start_scene` and that every loop
has a deliberate exit or replay purpose.

### 4. Specify mechanics

Use one stable ID per reusable mechanic. Write rules in testable language. Pair
each rule with the input that invokes it and feedback that lets the player
understand the result. Call out timing, scoring, difficulty, failure, and edge
cases when they matter.

### 5. Build the art requirement ledger

Treat art as production requirements, not decoration. For each item define its
kind, scene usage, player-facing purpose, required states/variants, spatial or
palette constraints, and acceptance criteria. Reuse one art ID across scenes
when it is truly the same asset.

Do not claim an asset exists merely because the design names it. If the user
asks to generate art, invoke the available image-generation workflow separately
and update the ledger with the actual output path and status.

### 6. Audit design versus runtime

Classify each important element as:

- Implemented: verified in current code or behavior
- Configured/executable: a schema-v2 scene handler or transition the engine runs
- Configured/metadata: a mechanic or art requirement retained for production
- Proposed: requires engine/schema/art work

Schema v2 compiles `game.start_scene`, scene handlers, and semantic transitions
into the Bevy engine's finite-state machine. Handlers are trusted built-in
behaviors; the manifest can reorder or reuse them but cannot define arbitrary
code or create a new renderer. Mechanic and art tables remain validated design
metadata. Schema v1 remains compatible metadata and uses the built-in flow.

When the desired game type is unsupported, finish the design brief but do not
put an invalid type in `CODEQUEST.toml`. Explain the runtime gap and propose the
smallest schema/engine increment needed before authoring that manifest change.

## Produce the artifacts

Create or revise:

1. `docs/game-design/<game-slug>.md` using
   the skill-local `assets/game-design-brief.md` as the structure
2. The target cartridge's root `CODEQUEST.toml`, following the repository's
   current contract

Keep the brief richer than the TOML. The brief explains intent, pacing,
production status, and implementation gaps; the manifest is the stable graph
the engine can validate. Preserve unrelated existing design decisions and show
material changes clearly.

## Validate before handoff

Run the bundled fast validator:

```bash
python3 .agents/skills/codequest-game-designer/scripts/validate_codequest.py CODEQUEST.toml
```

When working in the CODE QUEST engine repository, also run the Rust contract
tests from `src-tauri/`:

```bash
cargo test codequest --lib
```

Then perform a traceability pass:

- Every start/transition target resolves and every scene is reachable.
- Every transition signal is emitted by its scene's handler.
- Every scene mechanic and art reference resolves.
- Every mechanic is used or explicitly marked future work.
- Every art item names at least one player-facing need.
- Every scene appears in the brief and manifest with the same ID.
- Implemented/configured/proposed claims match repository evidence.

End with a short handoff: the experience designed, files changed, what the
engine can run today, and the smallest next implementation/art decisions.
