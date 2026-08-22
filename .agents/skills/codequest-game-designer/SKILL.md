---
name: codequest-game-designer
description: Design, revise, and polish CODE QUEST game experiences as connected storyboards, closed finite-state machines, gameplay and progression systems, visual and sound requirements, and valid CODEQUEST.toml manifests. Use this skill whenever the user asks to storyboard a quiz or quest, invent or change a game type, plan scenes or game flow, define gameplay rules, identify art, animation, audio, or UI needs, audit whether a game feels finished, remove dead or open states, make progression perceptible, or author/revise CODEQUEST.toml—even when they call it a game concept, flow, pitch, finishing pass, or content plan instead of a design task.
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

When the request includes polish, finishing, animation, sound, game feel, or
progression, also read `references/polish.md` in this skill completely before
editing the design.

When the request creates, replaces, or judges visual assets, also read
`references/visual-floor.md` completely. Treat repository-owned native template
plates as the acceptance floor; a procedural palette or glow treatment does not
meet an illustrated asset-backed reference.

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

When the contract supports `art[].template`, prefer a repository-owned built-in
template that satisfies the scene before proposing a new renderer. Treat a
typed built-in template as configured/executable presentation; art entries with
no template remain production metadata.

### 6. Run the whole-game polish audit

For a polish or finishing pass, audit every reachable scene—including credits,
loading/interstitial, failure, and replay scenes—across the five surfaces in
`references/polish.md`: static presentation, motion, sound, mechanical closure,
and felt progression. A silent or visually restrained scene can pass only when
that restraint is specified as an intentional state.

Inventory repository-owned template assets before proposing new ones. Map each
selected asset and required state to stable manifest art IDs, but distinguish
production metadata from assets the runtime actually loads. Add a polish matrix
to the brief. Do not call the game polished while any reachable scene has an
unspecified surface, an unhandled player/system event, or an unowned handoff.

### 7. Audit design versus runtime

Classify each important element as:

- Implemented: verified in current code or behavior
- Configured/executable: a schema-v2 scene handler or transition the engine runs
- Configured/executable presentation: a referenced built-in `art[].template`
- Configured/metadata: a mechanic or template-less art requirement retained
  for production
- Proposed: requires engine/schema/art work

Schema v2 compiles `game.start_scene`, scene handlers, and semantic transitions
into the Bevy engine's finite-state machine. Handlers are trusted built-in
behaviors; the manifest can reorder or reuse them but cannot define arbitrary
code or create a new renderer. Typed art templates select only renderers shipped
with CODE QUEST; mechanics and template-less art remain validated design
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

When the runtime contract has no dedicated sound schema, retain sound needs as
referenced production entries (for example `art.kind = "audio"`) instead of
inventing fields. State plainly that metadata does not play audio. Do not use a
visual `template` name to imply sound playback.

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

When built-in visual templates change, emit and inspect every Oracle scene at
native resolution before handoff:

```bash
CQA_VISUAL_PREVIEW_DIR=/tmp/codequest-previews \
  cargo test --manifest-path src-tauri/Cargo.toml \
  oracle_templates_produce_nine_distinct_native_scene_frames --lib
```

When the repository includes `scripts/compile-oracle-assets.sh`, run it before
the preview test so inspectable PNG sources and embedded runtime buffers cannot
drift.

Then perform a traceability pass:

- Every start/transition target resolves and every scene is reachable.
- Every transition signal is emitted by its scene's handler.
- Every scene mechanic and art reference resolves.
- Every mechanic is used or explicitly marked future work.
- Every art item names at least one player-facing need.
- Every scene appears in the brief and manifest with the same ID.
- Implemented/configured/proposed claims match repository evidence.
- Every reachable scene has explicit static, motion, and sound intent.
- Every player input and asynchronous outcome is handled, ignored deliberately,
  or routed to a visible recovery state; no FSM state is left open.
- Loops, one-shots, held inputs, skip paths, and scene exits hand off cleanly.
- The beginning, middle, and end of a run differ through perceivable feedback,
  not only a larger number.

End with a short handoff: the experience designed, files changed, what the
engine can run today, and the smallest next implementation/art decisions.
