# Oracle Quiz

This is the human design brief for the quiz flow represented by
[`docs/examples/CODEQUEST.toml`](../examples/CODEQUEST.toml). The brief explains
intent and implementation status; the manifest supplies stable scene,
mechanic, and art IDs. Its schema-v2 handlers and transitions compile into the
engine's scene machine; mechanics and art remain linked production metadata.

## Experience frame

**Pitch:** A copyright-style repository chronicle names the people and history
behind the cartridge, then erupts into an original code-fantasy fanfare before
the player binds a small code-seer, consults an Oracle, and proves their
understanding through increasingly difficult conceptual questions. The first
question request runs behind the opening spectacle, and any remaining latency
becomes Oracle Datafall: a Left/Right falling-object game in which the hero
dodges bugs and runs into data while Claude works.

**Player outcome:** Build a durable mental model of a project's purpose,
responsibilities, interactions, invariants, and tradeoffs—not memorize file
names or repository trivia.

**Design pillars**

1. The Oracle tells the truth: show only states the engine knows and never
   fabricate percent-complete progress.
2. Every transition carries player identity or learning state forward; the
   quiz should feel like one run, not a stack of forms.
3. Earn spectacle from real repository provenance, while native 240×160
   readability wins over visual density, motion, or extra copy.
4. Stall-scene inputs are transition-safe: A, B, and Start are inactive so a
   held face button cannot answer a question that appears mid-input.

**Non-goals:** No town or overworld layer, inventory economy, timed-answer
pressure, generated filler questions, borrowed characters or compositions,
unsupported copyright claims, or claim that manifest metadata already
creates new renderer code.

## Session and loops

- **Session shape:** Copyright → opening fanfare → title → quiz menu → hero
  creation → Oracle → questions → level-up or game-over. A run lasts until the
  player loses three hearts or returns to the menu.
- **Core loop:** Consult Oracle → receive one valid question → choose an answer
  → read feedback → continue or return to the Oracle.
- **Progression loop:** Survive a six-question batch → raise difficulty → mark
  a level-up → begin or await the next batch.
- **Success:** Correct answers add score and streak; completing a batch raises
  the level.
- **Failure/recovery:** A wrong answer costs one heart and reveals the correct
  choice. At zero hearts, show the final score and a one-button replay path.
- **Latency loop:** Request the first batch as soon as the cartridge is
  accepted, then continue behind the copyright, fanfare, title, menu, and hero
  creation. In Oracle Datafall, move into falling data to collect it on contact
  while moving away from crossed bugs.
  Enter the Oracle only when no valid unanswered question is ready; failed
  batches retry there instead of becoming generic trivia.

## Scene storyboard

| ID | Purpose | Player actions and feedback | Exit and next scenes | Mechanics | Art |
|---|---|---|---|---|---|
| `copyright` | Credit the repository's authors and real timeline while the first question request starts. | Read the provenance card; A/Start skips after its minimum dwell. Never infer a legal owner from commit authorship. | `opening-fanfare` | `present-copyright` | `copyright-card` |
| `opening-fanfare` | Turn early generation time into a finite original spectacle. | Watch a dark-to-bright code-fantasy sequence; A/Start skips after the readable opening impact. | `title` | `play-opening-fanfare` | `opening-fanfare` |
| `title` | Resolve the fanfare into an invitation from the Oracle. | A/Start begins; title remains readable at native scale. | `quiz-menu` | `navigate-menu` | `title-mark` |
| `quiz-menu` | Explain the run and offer a safe return. | D-pad selects; A/Start confirms; B returns. | `character-creation` or `title` | `navigate-menu` | — |
| `character-creation` | Give the player identity while the first question request is already in flight. | Change name, class, and style with an immediate hero preview. | `oracle` | `customize-hero` | `hero-set` |
| `oracle` | Turn real generation latency into a safe, active interstitial. | Left/Right changes lanes; data scores on contact and bugs count as hits. Every other control is inactive. The top quiz header holds Oracle/loading context; the bottom game strip holds counters and controls. | Automatically enters `quiz` when a valid question is ready. | `consult-oracle` | `hero-set`, `oracle-sanctum` |
| `quiz` | Test one durable project concept. | D-pad selects; A commits; text and color reveal correct/wrong. | `oracle`, `level-up`, or `game-over` | `answer-question` | `hero-set`, `quiz-frame` |
| `level-up` | Recognize a completed batch while the next batch is prepared. | A/Start continues after level and batch feedback. | `oracle` | `navigate-menu` | `hero-set` |
| `game-over` | Close the run and make replay obvious. | Show final score; A/B/Start returns to the menu. | `quiz-menu` | `navigate-menu` | `hero-set` |

All scenes are reachable from `copyright`. The opening path is finite, and the
`oracle` → `quiz` loop is deliberate. The Oracle has no manual exit because all
four directions are gameplay inputs; game-over and menu back actions provide
the run's explicit exits.

## Opening micro-storyboard

The pacing grammar comes from observing two GBA openings locally: one uses an
immediate animated confrontation before a silhouette and title reveal, while
the other lets a luminous emblem, restrained motion, and an idle vignette build
tone. CODE QUEST uses original symbols, staging, and art rather than copying
their characters or layouts.

| Beat | Target time | Presentation | Question-generation behavior | Player agency |
|---|---:|---|---|---|
| Copyright card | 0.0–1.5s | Repository title, up to three author lines, and earliest → latest commit dates appear as a high-contrast provenance card. Show a literal © owner only when an explicit repository notice supplies it. | The first request has already started when the cartridge was accepted. | A/Start becomes available after the text has had one readable second. |
| Timeline traversal | 1.5–3.0s | A light travels through a sparse commit constellation; real tag or release landmarks may flare when available. Overflow authors use a second card rather than smaller text. | Continue silently; no percentage, spinner claim, or completion implication. | A/Start advances to the fanfare. |
| Sigil encounter | 3.0–5.0s | Two abstract code sigils enter as silhouettes, collide once, and turn the impact into the cartridge-colored repository crest. | Continue in the background; cache an early result without interrupting the sequence. | A/Start may skip after the impact is readable. |
| Oracle ignition | 5.0–7.0s | The crest branches like a commit graph, folds into the Oracle eye, and holds on its own dark composition. Reduced motion uses three clean cuts and fades. | Finishing this beat never promises that questions are ready. | No input required. |
| Title handoff | 7.0–8.5s | The fanfare ends on its own dark frame; the title scene starts from a fresh clear with no fanfare overlays. | Continue generation through title, menu, and hero creation if needed. | A/Start begins the normal menu flow. |

If the first batch is ready early, it waits safely for the player. If it is
still unavailable after hero creation, the existing Oracle scene communicates
the real wait and retry states. The opening never stretches itself to fake a
dependency on generation.

## Oracle micro-storyboard

| Beat | Trigger | Presentation | Player agency | Status |
|---|---|---|---|---|
| Arrival | Enter `oracle`. | Reset the hero to center, clear in-flight objects, and show the real Claude status. Keep the existing minimum dwell so instant results do not flash past. | Left/Right begins moving immediately; every other control remains inactive. | Implemented. |
| Datafall | Request is in flight. | Boxed data packets and crossed bugs fall through deterministic lanes. Data and bug-hit counters persist across Oracle visits in the current quiz run. | Move into data to collect it automatically; move away from bugs. | Implemented. |
| Clouded vision | A batch returns empty or invalid. | `CLAUDE RETRYING` distinguishes the real retry delay without a fake percentage. Falling-object play continues. | Left/Right remain available. | Implemented. |
| Vision ready | A valid unanswered question exists. | `QUESTION READY` may appear during the minimum dwell, then the scene transitions automatically. | No confirmation required; held D-pad inputs cannot answer the quiz. | Implemented. |
| Long wait | Scrying continues beyond the normal beat. | The same deterministic play loop continues under truthful status copy, with no invented scan steps. | Keep playing until the question arrives. | Implemented. |

The Oracle never rewards a slow response, suggests that Datafall speeds up the
model, or hides a failed request behind invented progress. Datafall score and
collisions are deliberately isolated from quiz hearts, score, question timing,
and Claude retries.

## Mechanics

### `present-copyright`

- **Decision:** Read the repository provenance or advance after a minimum dwell.
- **Inputs:** A or Start.
- **Rules:** Use repository-derived author names and earliest/latest commit
  dates. Credit up to three git authors using git shortlog's commit-count
  ranking. Display explicit copyright ownership only when a repository notice
  provides it. Begin generation at cartridge acceptance, not at scene exit.
- **Feedback:** Reveal authors and timeline landmarks with fixed, readable
  timing; do not present generation progress.

### `play-opening-fanfare`

- **Decision:** Watch the complete five-to-seven-second spectacle or skip after
  its opening impact.
- **Inputs:** A or Start.
- **Rules:** The sequence is finite and deterministic. Completion never implies
  question readiness. Reduced motion changes transitions, not duration or data.
- **Feedback:** Dark silhouettes resolve through a repository crest and commit
  constellation into the Oracle sigil, then cut cleanly to the title scene.

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

- **Decision:** Choose a lane, dodge crossed bugs, and collide with boxed data.
- **Inputs:** Left and Right move. Up, Down, A, B, Start, and shoulders are
  inactive.
- **Rules:** Drops use deterministic lanes and alternate data/bug types. Data
  overlap increments a cosmetic data counter; bug overlap increments a cosmetic
  hit counter. Active drops reset on each Oracle entry, while data/hit counters
  persist for the current quiz run. Stay until a valid
  unanswered question exists; empty results retry. No Datafall state affects
  generation, difficulty, quiz score, hearts, or wait duration.
- **Feedback:** Data uses a boxed silhouette; bugs use a crossed silhouette.
  Keep `ORACLE DATAFALL`, truthful loading/retry/ready text, and animated dots
  in the top 12-pixel quiz header. Keep both counters and the Left/Right prompt
  in the bottom game strip.

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
| `copyright-card` | UI | `copyright` | Establish authorship and history; title, primary authors, date range, optional explicit notice, and overflow page. | Legible at 240×160; never infer legal ownership; body text stays at native size. | Procedural base implemented; overflow polish needed |
| `opening-fanfare` | Scene/VFX | `opening-fanfare` | Create anticipation with silhouette, impact, repository crest, commit constellation, Oracle ignition, and reduced-motion variants. | Original characters/composition; five-to-seven seconds; no full-frame flashes; never overlay the title frame. | Procedural base implemented; reduced-motion polish needed |
| `title-mark` | Logo/UI | `title` | Identify the cartridge and Oracle motif; idle and prompt-pulse states. | Legible at 240×160 without glow. | Basic renderer implemented; art polish needed |
| `hero-set` | Sprite set | `character-creation`, `oracle`, `quiz`, `level-up`, `game-over` | Carry identity through the run; customization, idle, dodge, success, and defeat variants. | Consistent silhouette across palettes/backgrounds. | Procedural base implemented; state polish needed |
| `oracle-sanctum` | Scene/UI | `oracle` | Present Datafall, loading, retry, and ready as one place: moving hero, boxed packets, crossed bugs, counters, animated dots, and horizontal-movement prompt. | Fits 240×160; objects differ by shape and color; Oracle/loading information stays in the top header while gameplay counters/controls stay in the bottom strip. | Procedural Datafall renderer implemented |
| `quiz-frame` | HUD/UI | `quiz` | Hold question, four choices, focus, hearts, score, streak, and result labels. | Honor text limits; focus and correctness cannot rely on color alone. | Core frame implemented; accessibility polish needed |

## Runtime traceability

| Element | Status | Evidence or required work |
|---|---|---|
| Manifest title and `quiz`/`quest` type | Implemented | Parsed at cartridge load and used by the engine. |
| Scene graph | Configured/executable | Schema-v2 handlers and semantic transitions are validated, compiled, and executed by the engine. |
| Mechanic and art graph | Configured/metadata | Parsed, cross-reference validated, and retained as design and production requirements. |
| First question request at cartridge acceptance | Implemented | Empty quiz cartridges call the question effect immediately when inserted. |
| Repository authors, timeline, and explicit copyright extraction | Implemented | Cartridge preparation reads sanitized git shortlog/history data and scans bounded LICENSE/COPYRIGHT/NOTICE files. Commit authors are never treated as legal owners. |
| Copyright and opening-fanfare screens | Implemented in basic form | Trusted Bevy handlers render before `Title`; manifest timing gates control skip/auto-advance while fanfare/title frames remain separate. |
| Title, menu, hero creation, Oracle, quiz, level-up, and game-over screens | Implemented | Trusted handlers own input and rendering while the manifest routes their semantic events. |
| First request, prefetch, invalid-batch retry, and Oracle hold | Implemented | Engine question effects, pending batches, and retry timer. |
| Left/Right-only Oracle Datafall | Implemented | Held horizontal movement, deterministic falling objects, automatic data/bug collision counters, split top/bottom HUD, and framebuffer-level behavior tests. |
| Safe Oracle-to-quiz input boundary | Implemented | Face buttons are ignored in Oracle; held D-pad controls have no answer action after the automatic transition. |
| Truthful multi-state Oracle presentation | Implemented in basic form | Loading, retry, and ready copy derives from actual engine state; an explicit AI-disabled state remains future work. |
| Non-color-only result labels and reduced motion | Proposed | Extend quiz/Oracle rendering and verify at native scale. |
| Art selected from manifest metadata | Proposed | Requires a future engine/schema decision; art entries are requirements only today. |

## Implementation slices

1. **Completed — Repository provenance pass:** Derive bounded author credits,
   earliest/latest commit dates, and any explicit copyright notice during
   cartridge preparation; add parser/sanitization tests.
2. **Completed — Opening state pass:** Add trusted `Copyright` and `OpeningFanfare`
   handlers before `Title`, preserve the already-early question request, and
   test minimum dwell, auto-advance, skip, and distinct rendered phases.
3. **Completed — Oracle Datafall pass:** Replace the A-jump/B-exit waiting room
   with Left/Right-only data collection and bug-dodging play; isolate its
   counters from quiz state; split quiz context from gameplay HUD and add
   framebuffer-level tests.
4. **Feedback/accessibility pass:** Add correctness labels, reduced-motion
   Oracle behavior, and native-scale screenshot assertions.
5. **Continuity pass:** Show the previous lesson and batch status during a wait
   using state the engine already owns.
6. **Art pass:** Produce and verify `copyright-card`, `opening-fanfare`,
   `title-mark`, `oracle-sanctum`, and remaining `hero-set` states against the
   240×160 ledger.
7. **Completed — Executable scene graph:** Add schema v2 handlers, semantic
   transitions, timing gates, reachability validation, built-in quiz/quest
   templates, and schema-v1 compatibility.

## Open decisions

- Should B from an active quiz ask before abandoning score and hearts?
- Should the generated question payload eventually include a short explanation,
  or is revealing the correct choice enough feedback at this resolution?
- Which run records, if any, should persist per cartridge across launches?
