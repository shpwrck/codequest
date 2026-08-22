# Oracle Quiz

This is the human design brief for the quiz flow represented by the repository's
root [`CODEQUEST.toml`](../../CODEQUEST.toml) and its maintained
[`docs/examples/CODEQUEST.toml`](../examples/CODEQUEST.toml) copy. Keeping the
manifest at the root makes CODE QUEST itself the reference cartridge for
dogfooding engine, scene-graph, and future presentation-template changes. The
brief explains intent and implementation status; the manifest supplies stable
scene, mechanic, and art IDs. Its schema-v2 handlers and transitions compile
into the engine's scene machine; typed visual templates select trusted built-in
renderer assets, while mechanics and template-less art remain production
metadata.

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
4. Stall-scene inputs are transition-safe: A and Start are inactive so a held
   confirmation cannot answer a question that appears mid-input; B has one
   explicit, input-edge back route.
5. Every reachable scene is authored: credits, waits, menus, rewards, and
   failure states receive the same static, motion, sound, closure, and
   progression scrutiny as the quiz itself.

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
  a level-up → visibly deepen the Oracle bond → begin or await the next batch.
- **Success:** Correct answers add score and streak; completing a batch raises
  the level.
- **Failure/recovery:** A wrong answer costs one heart and reveals the correct
  choice. At zero hearts, show the final score and a one-button replay path.
- **Latency loop:** Request the first batch as soon as the cartridge is
  accepted, then continue behind the copyright, fanfare, title, menu, and hero
  creation. In Oracle Datafall, move into falling data to collect it on contact
  while moving away from magenta corruption glyphs.
  Enter the Oracle only when no valid unanswered question is ready; failed
  batches retry there instead of becoming generic trivia.

## Polish direction and felt progression

The visual system is dark code-fantasy rendered at native 240×160: ink and
indigo establish space, cyan communicates data and selection, gold communicates
earned revelation, and red/green remain restrained gameplay signals. Frames,
glyphs, characters, and environments share one rune-and-circuit shape language.
The brightest cyan/gold combination and densest detail are reserved for earned
peaks; glow is a state, not a default decoration.

The implemented Oracle presentation uses repository-owned illustrated plates
and live hero/portrait sprites from `src-tauri/assets/oracle/`. Runtime text,
focus, loading, correctness, counters, and progression remain separate from the
source art. Every live foreground is assigned to a bounded panel or playfield;
layout checks reject overflow and sibling overlap, while palette checks require
readable contrast against the immediate panel fill.

The run has three perceivable presentation tiers in addition to its numeric
difficulty:

| Tier | Question focus | Environment and motion | Sound | Carry-forward |
|---|---|---|---|---|
| Initiate — level 1 | Purpose and responsibilities | Sparse cyan nodes; the Oracle eye is mostly dormant. | Dry UI ticks, one data pulse, thin ambience. | The chosen hero and first lit node persist through Oracle, quiz, and results. |
| Adept — levels 2–3 | Component interactions and tradeoffs | Gold branches join the cyan frame; transitions gain one extra anticipation beat. | A second motif voice and stronger correct-answer cadence enter. | The expanded crest remains visible in the next Oracle and quiz frames. |
| Oracle-bound — level 4+ | Invariants and design rationale | Cyan and gold converge into the complete Oracle crest; reward motion reaches its maximum controlled intensity. | Full constrained arrangement and final reward cadence. | The complete crest holds until game-over or a clearly signaled new-run reset. |

Crossing a tier is celebrated on `level-up` and established as the new visual
baseline afterward. A new run visibly returns to Initiate. Typed template
selection and the three visual tiers are implemented; audio progression remains
proposed until the runtime owns sound playback.

## Scene storyboard

| ID | Purpose | Player actions and feedback | Exit and next scenes | Mechanics | Art |
|---|---|---|---|---|---|
| `copyright` | Credit the repository's authors and real timeline while the first question request starts. | Read a deliberately dormant archive composition; A/Start skips after its minimum dwell. Never infer a legal owner from commit authorship. | `opening-fanfare` | `present-copyright` | `copyright-card`, `opening-soundscape` |
| `opening-fanfare` | Turn early generation time into a finite original spectacle. | Watch light propagate from one cyan node through two sigils and crescendo into the complete Oracle; A/Start skips after the impact is readable. | `title` | `play-opening-fanfare` | `opening-fanfare`, `opening-soundscape` |
| `title` | Resolve the fanfare into an invitation from the Oracle. | A/Start begins; the redrawn Oracle motif and title remain readable without glow. | `quiz-menu` | `begin-from-title` | `title-mark`, `ui-soundscape` |
| `quiz-menu` | Explain the run and offer a safe return. | D-pad selects; A/Start confirms; B returns; focus is visible by shape and color. | `character-creation` or `title` | `navigate-menu` | `menu-frame`, `ui-soundscape` |
| `character-creation` | Give the player identity while the first question request is already in flight. | Change name, path, and aura through centered identity rows; aura selects an authored hero colorway without procedural equipment overlays. | `oracle` | `customize-hero` | `hero-set`, `character-frame`, `ui-soundscape` |
| `oracle` | Turn real generation latency into a safe, active interstitial. | Left/Right changes lanes; data scores on contact and bugs count as hits. A/Start remain inactive; B abandons the wait safely. The top header holds truthful Oracle context and the bottom strip holds gameplay and exit controls. | Automatically enters `quiz` when a valid question is ready; B returns to `quiz-menu`. | `consult-oracle` | `hero-set`, `oracle-sanctum`, `oracle-soundscape`, `run-progression` |
| `quiz` | Test one durable project concept. | D-pad selects; A commits; B abandons the run; text, shape, animation, and sound reveal correct/wrong. | `oracle`, `level-up`, `game-over`, or `quiz-menu` | `answer-question` | `hero-set`, `quiz-frame`, `quiz-soundscape`, `run-progression` |
| `level-up` | Recognize a completed batch and establish a visibly stronger Oracle bond. | A/Start continues after the reward has a readable hold. | `quiz` or `oracle` | `continue-after-reward` | `hero-set`, `reward-frame`, `progression-soundscape`, `run-progression` |
| `game-over` | Close the run, show what was earned, and make replay obvious. | Show final score, level, and completed crest state; A/B/Start returns to the menu and clearly resets progression. | `quiz-menu` | `replay-run` | `hero-set`, `result-frame`, `progression-soundscape`, `run-progression` |

All scenes are reachable from `copyright`. The opening path is finite, and the
`oracle` → `quiz` loop is deliberate. Every scene now has a player or timed exit,
and Oracle B-back closes the otherwise indefinite loading/retry state without
allowing A/Start to leak into an arriving question.

## Opening micro-storyboard

The pacing grammar comes from observing two GBA openings locally: one uses an
immediate animated confrontation before a silhouette and title reveal, while
the other lets a luminous emblem, restrained motion, and an idle vignette build
tone. CODE QUEST uses original symbols, staging, and art rather than copying
their characters or layouts.

| Beat | Target time | Presentation | Question-generation behavior | Player agency |
|---|---:|---|---|---|
| Copyright card | 0.0–1.5s | Begin almost black. The repository title, up to three author lines, and earliest → latest commit dates are engraved in a dim archive frame with no emissive glow. Show a literal © owner only when an explicit repository notice supplies it. | The first request has already started when the cartridge was accepted. | A/Start becomes available after the text has had one readable second. |
| First signal | 1.5–3.0s | The archive clears to dormant cathedral silhouettes. One cyan commit node appears, then light travels through a sparse constellation; real tag or release landmarks may answer with a restrained pulse. | Continue silently; no percentage, spinner claim, or completion implication. | A/Start advances to the fanfare. |
| Sigil propagation | 3.0–5.0s | Two abstract code sigils remain mostly dark while cyan and gold segments illuminate outward from the commit path. Their readable silhouettes precede the bright collision. | Continue in the background; cache an early result without interrupting the sequence. | A/Start may skip after the impact is readable. |
| Oracle crescendo | 5.0–7.0s | The sigils collide once. Their joined crest branches like a commit graph and resolves into the complete cyan/gold Oracle eye—the opening's brightest, densest frame. Hold long enough to read it. Reduced motion uses clean staged cuts and fades. | Finishing this beat never promises that questions are ready. | No input required. |
| Title handoff | 7.0–8.5s | Energy recedes while the eye silhouette remains. The title scene starts from a fresh clear and redraws that motif at a restrained baseline; no fanfare layer leaks across the state boundary. | Continue generation through title, menu, and hero creation if needed. | A/Start begins the normal menu flow. |

If the first batch is ready early, it waits safely for the player. If it is
still unavailable after hero creation, the existing Oracle scene communicates
the real wait and retry states. The opening never stretches itself to fake a
dependency on generation.

## Oracle micro-storyboard

| Beat | Trigger | Presentation | Player agency | Status |
|---|---|---|---|---|
| Arrival | Enter `oracle`. | Reset the hero to center, clear in-flight objects, and show the real Claude status. Keep the existing minimum dwell so instant results do not flash past. | Left/Right begins moving immediately; B returns to the quiz menu; all other controls remain inactive. | Implemented. |
| Datafall | Request is in flight. | Authored cyan-and-gold crystal shards and asymmetric magenta corruption glyphs fall through deterministic lanes. Data and bug-hit counters persist across Oracle visits in the current quiz run. | Move into data to collect it automatically; move away from bugs. | Implemented. |
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
  timing; do not present generation progress. Use a dry archival tick per reveal
  and intentional near-silence so the first fanfare tone has room to matter.

### `play-opening-fanfare`

- **Decision:** Watch the complete five-to-seven-second spectacle or skip after
  its opening impact.
- **Inputs:** A or Start.
- **Rules:** The sequence is finite and deterministic. Completion never implies
  question readiness. Reduced motion changes transitions, not duration or data.
- **Feedback:** Dark silhouettes resolve through a repository crest and commit
  constellation into the Oracle sigil, then cut cleanly to the title scene.
  Sound grows from one node pulse to the full Oracle cadence in sync with that
  luminance arc.

### `navigate-menu`

- **Decision:** Begin, continue, replay, or return.
- **Inputs:** D-pad, A, B, Start.
- **Rules:** One option is visibly focused; a held button cannot confirm twice.
- **Feedback:** Move focus on the input edge and flash confirmation once.
  Navigation, confirm, cancel, and unavailable use distinct one-shot cues and
  never stack across a transition.

### `begin-from-title`

- **Decision:** Accept the Oracle's invitation and enter the quiz menu.
- **Inputs:** A or Start.
- **Rules:** Continue on the input edge only; stay idle when no playable
  cartridge is present.
- **Feedback:** Keep the prompt subordinate to the title and emit one clean
  confirmation that ends before menu input begins.

### `customize-hero`

- **Decision:** Choose name, path, and aura for the run.
- **Inputs:** D-pad, A, B, Start.
- **Rules:** Values wrap through finite lists and remain cosmetic.
- **Feedback:** Update each centered identity label immediately; aura changes the
  authored hero colorway while name and path remain textual identity. Do not
  layer lower-fidelity procedural accessories or weapons over the hero. Pair
  each changed trait with one short timbral variant and reserve the Oracle motif
  for final confirmation.

### `consult-oracle`

- **Decision:** Choose a lane, dodge corruption glyphs, and collide with crystal
  data shards.
- **Inputs:** Left and Right move. B returns to the quiz menu. Up, Down, A,
  Start, and shoulders are inactive.
- **Rules:** Drops use deterministic lanes and alternate data/bug types. Data
  overlap increments a cosmetic data counter; bug overlap increments a cosmetic
  hit counter. Active drops reset on each Oracle entry, while data/hit counters
  persist for the current quiz run. Stay until a valid
  unanswered question exists; empty results retry. No Datafall state affects
  generation, difficulty, quiz score, hearts, or wait duration. A B press exits
  on its input edge and clears the active run, so the wait is never inescapable.
- **Feedback:** Data uses an authored cyan-and-gold crystal silhouette; bugs
  use an asymmetric magenta corruption silhouette.
  Keep `ORACLE DATAFALL`, truthful loading/retry/ready text, and animated dots
  in the top 12-pixel quiz header. Keep counters plus move/back controls in the
  bottom game strip. Give data, bug, retry, ready, and back distinct cues while
  keeping the ambience below quiz feedback.

### `answer-question`

- **Decision:** Commit to one of four conceptual answers.
- **Inputs:** D-pad, A, B.
- **Rules:** Exactly four distinct choices, one correct answer, a maximum of
  four 32-character question lines, and 31 characters per choice. Wrong costs
  one heart; correct adds score and streak. After commitment, hold the revealed
  answer for 45 ticks with every input intentionally inactive.
- **Feedback:** Keep the correct choice visible in green after either result;
  show an incorrect committed choice in red. Do not add redundant correct/wrong
  words. Use visibly spaced glyphs plus distinct cursor, commit, low-heart, and
  batch-complete cues. Replace the active answer/back prompts with `REVIEW
  ANSWER` during the input lock.

### `continue-after-reward`

- **Decision:** Continue after reading the earned level and presentation tier.
- **Inputs:** A or Start.
- **Rules:** Keep the first 60 ticks non-interactive, then continue on the input
  edge. Route to `quiz` when a valid question is ready and to `oracle`
  otherwise. A held confirmation cannot answer the next question.
- **Feedback:** Telegraph, celebrate, and hold the new crest state once, then
  use one tier-specific continuation cue.

### `replay-run`

- **Decision:** Close the completed run and return to the quiz menu.
- **Inputs:** A, B, or Start.
- **Rules:** Return on the input edge. Beginning the next run resets run counters
  and the presentation tier.
- **Feedback:** Hold final score, level, and earned crest before prompting, then
  use one result-to-menu cue and make the eventual Initiate reset explicit.

## Art requirement ledger

| ID | Kind | Used by scenes | Purpose and required states | Constraints | Status |
|---|---|---|---|---|---|
| `copyright-card` | UI | `copyright` | Establish authorship and history; title, primary authors, date range, optional explicit notice, and missing-data states. | Legible at 240×160; never infer legal ownership; body text stays at native size. | `oracle-chronicle` implemented; dormant luminance and staged reveal tested. |
| `opening-fanfare` | Scene/VFX | `opening-fanfare` | Create anticipation from a completely dormant cathedral through first signal, sigil propagation, one collision, repository crest, and Oracle crescendo. | Original characters/composition; five-to-seven seconds; maximum cyan/gold only at the climax; no full-frame flashes; clean title handoff. | `oracle-awakening` implemented; dormant-to-crescendo luminance tested. |
| `title-mark` | Logo/UI | `title` | Identify the cartridge and Oracle motif; idle and prompt-pulse states. | Legible at 240×160 without glow. | `oracle-title` implemented. |
| `menu-frame` | UI | `quiz-menu` | Carry the cathedral/rune language into focused and idle menu states. | Focus differs by pointer, shape, and color; no unowned empty space. | `oracle-menu` implemented. |
| `character-frame` | UI/scene | `character-creation` | Stage the customizable hero inside the same world with centered name, path, and aura rows plus loading, retry, and ready states. | Labels and values remain native-scale and centered in their measured interiors; status is truthful. | `oracle-atelier` implemented. |
| `hero-set` | Sprite set | `character-creation`, `oracle`, `quiz`, `level-up`, `game-over` | Carry identity through the run with authored aura colorways plus idle, dodge, reward, and defeat variants. | Consistent silhouette across palettes/backgrounds; no procedural accessory or weapon overlays. | `oracle-hero` implemented with authored colorways, portrait, and defeat variants. |
| `oracle-sanctum` | Scene/UI | `oracle` | Present Datafall, loading, retry, ready, and B-back as one place: moving hero, authored data shards, corruption glyphs, counters, and a crest that reflects run tier. | Fits 240×160; sprites stay inside the playfield and differ by silhouette, value, and hue; Oracle/loading information stays in the top header while gameplay counters and move/back controls stay in the bottom strip. | `oracle-sanctum` implemented with authored drop sprites and three visual tiers. |
| `quiz-frame` | HUD/UI | `quiz` | Hold question, four choices, focus, hearts, score, streak, and answer review. | Honor text limits; preserve one pixel between adjacent glyph cells; keep focus shaped while correctness uses concise green/red choice text. | `oracle-trial` implemented with spaced copy, shaped focus, and color-only answer review. |
| `reward-frame` | Scene/UI | `level-up` | Telegraph the threshold, celebrate it once, and establish the new Oracle-bond baseline. | No rapid full-background flashing; tier and reward remain readable. | `oracle-ascension` implemented with tiered crest and readable hold. |
| `result-frame` | Scene/UI | `game-over` | Resolve the run with hero state, final score, level, completed crest, and an obvious reset/replay path. | Defeat is clear without erasing earned progress; all accepted replay inputs are visible. | `oracle-aftermath` implemented with preserved tier and defeated hero. |
| `run-progression` | Presentation system | `oracle`, `quiz`, `level-up`, `game-over` | Select Initiate, Adept, and Oracle-bound frame/crest states so progression survives through the final result. | At least two non-numeric channels change; resets deterministically on a new run. | `oracle-progression` implemented; palette, circuit density, and crest geometry change by level. |

## Sound requirement ledger

Sound entries are currently `art.kind = "audio"` production metadata. The
engine does not load or play them yet.

| ID | Used by scenes | Player-facing purpose | Cues/loops and variants | Constraints and acceptance | Status |
|---|---|---|---|---|---|
| `opening-soundscape` | `copyright`, `opening-fanfare` | Make the dormant-to-Oracle reveal audible and give the credits intentional restraint. | Archival tick, first-node pulse, branching sequence, sigil collision, Oracle cadence, clean tail. | Constrained chip-style palette; starts near silent; fullest arrangement only at the crescendo; reduced-audio variant. | Proposed |
| `ui-soundscape` | `title`, `quiz-menu`, `character-creation` | Make focus, choice, cancel, customization, and confirmation instantly legible. | Navigate, confirm, cancel, unavailable, trait variants, begin-run cadence. | One input edge produces at most one cue; no cue crosses scenes unintentionally. | Proposed |
| `oracle-soundscape` | `oracle` | Separate Datafall play from truthful loading state without overwhelming it. | Low ambience; data, bug, retry, ready, and B-back cues; three progression-tier variants. | Loops stop on quiz/menu transition; status remains readable when muted. | Proposed |
| `quiz-soundscape` | `quiz` | Clarify cursor movement, answer commitment, result, danger, and batch completion. | Cursor, commit, correct, wrong, low-heart, streak, batch-complete cues. | Correct/wrong never rely on sound alone; prevent stacked result cues. | Proposed |
| `progression-soundscape` | `level-up`, `game-over` | Make thresholds, earned tier, defeat, and reset feel conclusive. | Telegraph, reward cadence per tier, defeat fall, score hold, replay/reset cadence. | Reward arrangement grows by tier; new run audibly returns to Initiate. | Proposed |

## Whole-game polish matrix

| Scene | Static | Motion | Sound | Mechanical closure | Felt progression | Evidence/status |
|---|---|---|---|---|---|---|
| `copyright` | Dim archive frame with hierarchy equal to gameplay; no glow. | Credits reveal in fixed readable steps, then clear cleanly. | Intentional near-silence; archival ticks remain proposed. | A/Start after minimum dwell or timed exit; missing provenance has explicit copy. | Establishes the dormant baseline and repository identity. | Visual template implemented and native-frame tested; audio proposed. |
| `opening-fanfare` | Cathedral, sigils, commit path, and crest use the shared code-fantasy language. | Dormant → first node → propagation → collision → complete Oracle → title handoff. | Layered cadence remains proposed. | Skip and elapsed paths land on a fresh title frame. | Establishes the visual range the run later earns back. | Visual template and luminance arc implemented/tested; audio proposed. |
| `title` | Restrained Oracle motif and legible title at native scale. | One controlled eye/prompt pulse never competes with title. | Title loop and start cue remain proposed. | A/Start continues; unavailable cartridge state remains honest. | Returns to an Initiate baseline while preserving the Oracle promise. | Visual template implemented/tested; audio proposed. |
| `quiz-menu` | Rune frame and shaped focus for both choices. | Focus moves on input edge. | Navigate, confirm, and cancel cues remain proposed. | Begin and back are explicit; held input cannot double-confirm. | New run previews the Initiate palette and reset. | Visual template implemented/tested; audio proposed. |
| `character-creation` | Authored hero, centered identity rows, and Oracle status form one staged composition. | Aura changes the authored colorway; identity rows react immediately; begin has one clean handoff. | Trait and begin cues remain proposed. | Every row stays centered and contained; B returns; loading/retry/ready states are truthful. | Establishes identity that remains visible across the run. | Visual template implemented/tested; audio proposed. |
| `oracle` | Sanctum, playfield, status, counters, controls, and tier crest remain distinct. | Datafall, status changes, and tier motif have owned loops and exits. | Ambience and action cues remain proposed. | Questions-ready enters quiz; B abandons safely; all other controls are intentionally inactive. | Crest, circuit density, and palette reflect all three tiers. | Visual template/progression implemented/tested; audio proposed. |
| `quiz` | Question, choices, hero token, score, hearts, and tier frame remain readable with one-pixel glyph spacing. | Cursor, commit, 45-tick review hold, color change, heart loss, and batch threshold have causal timing. | Quiz cues remain proposed. | Green/red answer copy appears without redundant result words; every automatic outcome routes visibly. | Question concepts deepen and the earned tier persists. | Spaced visual template and concise answer review implemented/tested; audio proposed. |
| `level-up` | Hero and newly expanded crest dominate; level text supports rather than carries reward. | Crest growth → hero rise → one-second hold → continue; stable background avoids flashing. | Reward cadence remains proposed. | A/Start is inactive for 60 ticks, then routes to ready quiz or Oracle wait. | Explicit threshold celebration establishes the new visual baseline. | Visual template/progression implemented/tested; audio proposed. |
| `game-over` | Defeated hero, final score, level, and earned crest share one conclusive frame. | Energy recedes without erasing the crest; replay resets it on the next run. | Defeat/result/reset cues remain proposed. | The visible A/B/Start prompt returns to menu; the next new run resets all run state. | Shows the exact visual tier reached before reset. | Visual template/progression implemented/tested; audio proposed. |

## Runtime traceability

| Element | Status | Evidence or required work |
|---|---|---|
| Manifest title and `quiz`/`quest` type | Implemented | Parsed at cartridge load and used by the engine. |
| Scene graph | Configured/executable | Schema-v2 handlers and semantic transitions are validated, compiled, and executed by the engine. |
| Mechanic and presentation graph | Mixed | Mechanics and audio remain metadata; typed visual templates are parsed, validated, and executed by scene renderers. |
| First question request at cartridge acceptance | Implemented | Empty quiz cartridges call the question effect immediately when inserted. |
| Repository authors, timeline, and explicit copyright extraction | Implemented | Cartridge preparation reads sanitized git shortlog/history data and scans bounded LICENSE/COPYRIGHT/NOTICE files. Commit authors are never treated as legal owners. |
| Copyright and opening-fanfare screens | Implemented with asset-backed templates | Trusted Bevy handlers render the illustrated chronicle and staged awakening before `Title`; manifest timing gates control skip/auto-advance while fanfare/title frames remain separate. |
| Title, menu, hero creation, Oracle, quiz, level-up, and game-over screens | Implemented | Trusted handlers own input and rendering while the manifest routes their semantic events. |
| First request, prefetch, invalid-batch retry, and Oracle hold | Implemented | Engine question effects, pending batches, and retry timer. |
| Oracle Datafall with safe recovery | Implemented in this pass | Held horizontal movement, deterministic falling objects, automatic data/bug collision counters, split top/bottom HUD, and B-back close the indefinite wait. |
| Safe Oracle-to-quiz input boundary | Implemented | A/Start are ignored in Oracle; B exits to the menu; held D-pad controls have no answer action after the automatic transition. |
| Quiz result and reward input boundaries | Implemented in this pass | The 45-tick answer reveal replaces active controls, and level-up enforces a 60-tick hold before A/Start can leave. |
| Truthful multi-state Oracle presentation | Implemented with asset-backed templates | Loading, retry, and ready copy derives from actual engine state; B provides recovery from a permanently unavailable generator. A dedicated disabled explanation remains future work. |
| Concise answer review and reduced motion | Partially implemented | Green/red answer copy, shaped focus, stable level-up, and staged opening motion are implemented; a user-selectable reduced-motion setting remains proposed. |
| Visual templates selected from manifest | Implemented in this pass | Eleven typed built-in templates are selected by `art[].template`; Oracle templates composite native illustrated plates and live state, unknown names fail validation, and untemplated cartridges keep their legacy renderers. |
| Whole-game sound design and playback | Configured/metadata; runtime proposed | Every scene references an audio requirement, but the current engine has no sound asset selection or playback system. |
| Felt presentation progression | Visual runtime implemented; audio proposed | Initiate, Adept, and Oracle-bound change palette, circuit density, crest geometry, and reward/result presentation; native-frame tests verify a non-numeric final-tier channel. |

## Implementation slices

1. **Completed — Repository provenance pass:** Derive bounded author credits,
   earliest/latest commit dates, and any explicit copyright notice during
   cartridge preparation; add parser/sanitization tests.
2. **Completed — Opening state pass:** Add trusted `Copyright` and `OpeningFanfare`
   handlers before `Title`, preserve the already-early question request, and
   test minimum dwell, auto-advance, skip, and distinct rendered phases.
3. **Completed — Oracle Datafall pass:** Add Left/Right data collection and
   bug-dodging play; isolate its counters from quiz state; split quiz context
   from gameplay HUD and add framebuffer-level tests.
4. **Completed — FSM closure pass:** Add Oracle B-back recovery, own the quiz
   answer-review lock, enforce the level-up hold, display every accepted
   game-over input, and test runtime/template/manifest routes.
5. **Partially completed — Feedback/accessibility pass:** Correctness labels,
   shaped focus, staged motion, and native-scale assertions are implemented; a
   user-facing reduced-motion setting remains.
6. **Continuity pass:** Show the previous lesson and batch status during a wait
   using state the engine already owns.
7. **Completed — Whole-game presentation pass:** Eleven typed built-in visual
   templates cover all nine scenes, beginning with the dormant
   `copyright-card` and culminating in the Oracle crescendo.
8. **Sound runtime pass:** Add bounded template-audio selection and playback,
   then implement every entry in the sound ledger with scene-owned loop exits.
9. **Partially completed — Felt-progression pass:** Tiered visuals carry the
   crest, palette, circuit density, and hero identity across Oracle, quiz,
   reward, and results; tiered audio remains part of the sound runtime pass.
10. **Completed — Executable scene graph:** Add schema v2 handlers, semantic
   transitions, timing gates, reachability validation, built-in quiz/quest
   templates, and schema-v1 compatibility.

## Open decisions

- Should the generated question payload eventually include a short explanation,
  or is revealing the correct choice enough feedback at this resolution?
- Which run records, if any, should persist per cartridge across launches?
- Should B from an active quiz abandon immediately, or use an in-scene
  confirmation state before leaving the run?
