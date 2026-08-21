# Oracle Quiz

This is the human design brief for the quiz flow represented by
[`docs/examples/CODEQUEST.toml`](../examples/CODEQUEST.toml). The brief explains
intent and implementation status; the manifest supplies stable scene,
mechanic, and art IDs. Schema-v1 storyboard metadata is validated and carried
into the engine, but it does not dynamically create these screens.

## Experience frame

**Pitch:** The player binds a small code-seer, consults an Oracle that is
reading the inserted software project, and proves their understanding through
an endless sequence of increasingly difficult conceptual questions. Real
question-generation latency becomes an honest ritual instead of an unexplained
loading screen.

**Player outcome:** Build a durable mental model of a project's purpose,
responsibilities, interactions, invariants, and tradeoffs—not memorize file
names or repository trivia.

**Design pillars**

1. The Oracle tells the truth: show only states the engine knows and never
   fabricate percent-complete progress.
2. Every transition carries player identity or learning state forward; the
   quiz should feel like one run, not a stack of forms.
3. Native 240×160 readability wins over visual density, motion, or extra copy.

**Non-goals:** No town or overworld layer, inventory economy, timed-answer
pressure, generated filler questions, or claim that manifest metadata already
implements Bevy screens.

## Session and loops

- **Session shape:** Title → quiz menu → hero creation → Oracle → questions →
  level-up or game-over. A run lasts until the player loses three hearts or
  returns to the menu.
- **Core loop:** Consult Oracle → receive one valid question → choose an answer
  → read feedback → continue or return to the Oracle.
- **Progression loop:** Survive a six-question batch → raise difficulty → mark
  a level-up → begin or await the next batch.
- **Success:** Correct answers add score and streak; completing a batch raises
  the level.
- **Failure/recovery:** A wrong answer costs one heart and reveals the correct
  choice. At zero hearts, show the final score and a one-button replay path.
- **Latency loop:** Request early and prefetch during play. Enter the Oracle
  only when no valid unanswered question is ready; failed batches retry there
  instead of becoming generic trivia.

## Scene storyboard

| ID | Purpose | Player actions and feedback | Exit and next scenes | Mechanics | Art |
|---|---|---|---|---|---|
| `title` | Establish the cartridge as an invitation from the Oracle. | A/Start begins; title remains readable at native scale. | `quiz-menu` | `navigate-menu` | `title-mark` |
| `quiz-menu` | Explain the run and offer a safe return. | D-pad selects; A/Start confirms; B returns. | `character-creation` or `title` | `navigate-menu` | — |
| `character-creation` | Give the player identity while the first question request is already in flight. | Change name, class, and style with an immediate hero preview. | `oracle` | `customize-hero` | `hero-set` |
| `oracle` | Turn real generation latency into anticipation without deception. | A animates the hero; truthful loading/retry/ready copy remains visible; B returns safely. | `quiz` or `quiz-menu` | `consult-oracle` | `hero-set`, `oracle-sanctum` |
| `quiz` | Test one durable project concept. | D-pad selects; A commits; text and color reveal correct/wrong. | `oracle`, `level-up`, or `game-over` | `answer-question` | `hero-set`, `quiz-frame` |
| `level-up` | Recognize a completed batch while the next batch is prepared. | A/Start continues after level and batch feedback. | `oracle` | `navigate-menu` | `hero-set` |
| `game-over` | Close the run and make replay obvious. | Show final score; A/B/Start returns to the menu. | `quiz-menu` | `navigate-menu` | `hero-set` |

All scenes are reachable from `title`. The `oracle` → `quiz` loop is deliberate;
`game-over`, menu back actions, and Oracle B provide clear exits.

## Oracle micro-storyboard

| Beat | Trigger | Presentation | Player agency | Status |
|---|---|---|---|---|
| Arrival | Enter `oracle`. | Hero reaches the dais; Oracle focus settles. Keep the existing minimum dwell so instant results do not flash past. | A triggers a cosmetic jump. | Implemented in basic form. |
| Scrying | Request is in flight. | Indeterminate motion plus `THE ORACLE CONSULTS CLAUDE`; never show a percentage. | A remains cosmetic; B returns. | Implemented in basic form; richer art/copy proposed. |
| Clouded vision | A batch returns empty or invalid. | Distinct retry copy and shape; expose that another attempt will happen. | B remains available. | Retry timing implemented; distinct presentation proposed. |
| Vision ready | A valid unanswered question exists. | Motion converges and a text/check glyph confirms readiness before transition. | No confirmation required. | Automatic transition implemented; ready beat proposed. |
| Long wait | Scrying continues beyond the normal beat. | Calm loop, sparse rotating copy, and an explicit safe-exit hint. No fake scan steps. | B returns to the menu. | Exit implemented; long-wait presentation proposed. |

The Oracle never rewards a slow response, suggests that jumping speeds up the
model, or hides a failed request behind invented progress.

## Mechanics

### `navigate-menu`

- **Decision:** Begin, continue, replay, or return.
- **Inputs:** D-pad, A, B, Start.
- **Rules:** One option is visibly focused; a held button cannot confirm twice.
- **Feedback:** Move focus on the input edge and flash confirmation once.

### `customize-hero`

- **Decision:** Choose name, class, and style for the run.
- **Inputs:** D-pad, A, B, Start.
- **Rules:** Values wrap through finite lists and remain cosmetic.
- **Feedback:** Update sprite, accessory, weapon, label, and palette immediately.

### `consult-oracle`

- **Decision:** Wait with a responsive ritual or leave safely.
- **Inputs:** A for a cosmetic action; B to return.
- **Rules:** Stay until a valid unanswered question exists. A does not affect
  generation, difficulty, score, or wait duration. Empty results retry.
- **Feedback:** Loading, retry, and ready must differ in text and shape, not
  color or motion alone.

### `answer-question`

- **Decision:** Commit to one of four conceptual answers.
- **Inputs:** D-pad, A, B.
- **Rules:** Exactly four distinct choices, one correct answer, a maximum of
  four 37-character question lines, and 35 characters per choice. Wrong costs
  one heart; correct adds score and streak.
- **Feedback:** Keep the correct choice visible after either result. Label
  correctness in addition to red/green color.

## Art requirement ledger

| ID | Kind | Used by scenes | Purpose and required states | Constraints | Status |
|---|---|---|---|---|---|
| `title-mark` | Logo/UI | `title` | Identify the cartridge and Oracle motif; idle and prompt-pulse states. | Legible at 240×160 without glow. | Needed |
| `hero-set` | Sprite set | `character-creation`, `oracle`, `quiz`, `level-up`, `game-over` | Carry identity through the run; customization, idle, jump, success, and defeat variants. | Consistent silhouette across palettes/backgrounds. | Procedural base implemented; state polish needed |
| `oracle-sanctum` | Scene/UI | `oracle` | Make arrival, scrying, retry, ready, and long-wait states feel like one place. | Reserve clear status, hero, and Oracle regions; reduced-motion state required. | Basic renderer implemented; redesign needed |
| `quiz-frame` | HUD/UI | `quiz` | Hold question, four choices, focus, hearts, score, streak, and result labels. | Honor text limits; focus and correctness cannot rely on color alone. | Core frame implemented; accessibility polish needed |

## Runtime traceability

| Element | Status | Evidence or required work |
|---|---|---|
| Manifest title and `quiz`/`quest` type | Implemented | Parsed at cartridge load and used by the engine. |
| Scene, mechanic, and art graph | Configured | Parsed, cross-reference validated, and retained; not executed dynamically in schema v1. |
| Title, menu, hero creation, Oracle, quiz, level-up, and game-over screens | Implemented | Hard-coded `Screen` states, input handling, advancement, and render functions. |
| First request, prefetch, invalid-batch retry, and Oracle hold | Implemented | Engine question effects, pending batches, and retry timer. |
| Truthful multi-state Oracle presentation | Proposed | Add distinct in-flight/retry/ready/AI-disabled UI and tests to the existing Oracle state. |
| Non-color-only result labels and reduced motion | Proposed | Extend quiz/Oracle rendering and verify at native scale. |
| Art selected from manifest metadata | Proposed | Requires a future engine/schema decision; art entries are requirements only today. |

## Implementation slices

1. **Oracle state pass:** Keep the hard-coded Oracle screen; add distinct
   in-flight, retry, ready, and AI-disabled presentation using actual engine
   state and retain the B exit.
2. **Feedback/accessibility pass:** Add correctness labels, reduced-motion
   Oracle behavior, and native-scale screenshot assertions.
3. **Continuity pass:** Show the previous lesson and batch status during a wait
   using state the engine already owns.
4. **Art pass:** Produce and verify `title-mark`, `oracle-sanctum`, and remaining
   `hero-set` states against the 240×160 ledger.
5. **Future dynamic-scenes decision:** Only version the schema if cartridges
   truly need to drive scene execution; do not reinterpret v1 metadata.

## Open decisions

- Should B from an active Oracle/quiz run return immediately or ask before
  abandoning score and hearts?
- Should the generated question payload eventually include a short explanation,
  or is revealing the correct choice enough feedback at this resolution?
- Which run records, if any, should persist per cartridge across launches?
