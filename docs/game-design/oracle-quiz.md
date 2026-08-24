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
behind the cartridge, then unfolds through an original five-scene code-fantasy fanfare before
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
6. Every visible metric belongs to the Oracle world and has staged meaning:
   themed runes replace generic pips, exact thresholds change feedback or
   reward, and bare counters are not accepted as finished progression.

**Non-goals:** No town or overworld layer, inventory economy, timed-answer
pressure, generated filler questions, borrowed characters or compositions,
unsupported copyright claims, or claim that manifest metadata already
creates new renderer code.

## Session and loops

- **Session shape:** Copyright → five-scene opening story → title → quiz menu → hero
  creation → Oracle → questions → level-up or game-over. A run lasts until the
  player's three ward seals break or they return to the menu.
- **Core loop:** Consult Oracle → receive one valid question → choose an answer
  → read feedback → continue or return to the Oracle.
- **Progression loop:** Survive a six-question batch → raise difficulty → mark
  a level-up → visibly deepen the Oracle bond → begin or await the next batch.
- **Success:** Correct answers build flow; streaks 3 and 6 raise the score
  multiplier to x2 and x3, cumulative score awakens Insight Runes at 300, 900,
  and 1800, and completing a batch raises the level.
- **Failure/recovery:** A wrong answer costs one ward, resets flow, and reveals
  the correct choice. At zero wards, show the final score, earned Insight Rune,
  and a one-button replay path.
- **Save continuity:** Committing an answer records the question in the
  cartridge save. Later runs and launches compare normalized text and filter all
  recorded questions. Wards, score, flow, and presentation remain run-specific;
  hero identity is not save-backed across app launches.
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

### Tracked-metric threshold contract

Every player-visible quantity has an owned direction and staged response. The
raw number remains when precision matters, but it is paired with Oracle-shaped
runes so the HUD reads as part of the illustrated world rather than debug text.

| Metric | Desired direction | Exact stages | Reward or consequence | Cap and reset |
|---|---|---|---|---|
| Wards (health) | Keep high | 3 full → 2 strained → 1 fractured → 0 broken | Three cyan/gold ward runes lose fill and change state; the review banner names strain, fracture, or break. Zero ends the run. | Capped at 3; restored by a new run. |
| Flow (correct-answer streak) | Build high | 0–2 = x1, 3–5 = x2, 6+ = x3 | Each correct answer awards 100 × the active multiplier; the header and review response establish the new flow stage. | Multiplier caps at x3; a wrong answer or new run resets it. |
| Insight score | Build high | 300 = Rune I, 900 = Rune II, 1800 = Rune III | Three header runes awaken at exact crossings, score color advances, and a crossing banner names the earned rune. | Visual rank caps at Rune III while the readable score continues to 9999; a new run resets both. |
| Data charge | Build high | 3, 6, and 9 collected shards | One of three bottom-strip charge runes lights at each threshold. This is expressive reward only and never changes question generation or quiz score. | Rune meter caps at 3; count displays to 99; both reset on a new run. |
| Corruption hits | Keep low | 0 intact; first seal breaks at 1, second at 3, third at 5 | Three containment runes visibly fracture/extinguish in stages, rewarding a clean wait while warning before breach. This remains isolated from quiz health. | Breach display caps after 5; count displays to 99; both reset on a new run. |
| Questions/batches | Complete six | Every 6 answered questions completes a batch | A batch-complete scene raises level and holds the new bond state before continuation. | Continues while questions are available; new run resets batch and level. |
| Oracle bond level | Build high | level 1 Initiate; 2–3 Adept; 4+ Oracle-bound | Palette, crest geometry, circuit density, reward frame, and final result change—not just the number. | Visual tier caps at Oracle-bound; new run resets to Initiate. |

## Scene storyboard

| ID | Purpose | Player actions and feedback | Exit and next scenes | Mechanics | Art |
|---|---|---|---|---|---|
| `copyright` | Credit the repository's authors and real timeline while the first question request starts. | Read a deliberately dormant archive composition; A/Start skips after its minimum dwell. Never infer a legal owner from commit authorship. | `opening-fanfare` | `present-copyright` | `copyright-card`, `opening-soundscape` |
| `opening-fanfare` | Introduce the code-seer and the source ember at a deliberately restrained baseline. | Watch the lone cyan ember in the dormant cathedral; A/Start becomes a visible skip after the 1.5-second minimum dwell. | Timed exit to `archive-answer`; skip to `title`. | `play-opening-fanfare` | `opening-source`, `opening-soundscape` |
| `archive-answer` | Turn discovery into a causal response from the world. | The same code-seer reaches toward the ember as the altar and nearest monoliths answer in cyan. | Timed exit to `memory-vault`; A/Start skips to `title`. | `play-opening-fanfare` | `opening-signal`, `opening-soundscape` |
| `memory-vault` | Reveal that the cathedral contains the repository's living history. | Follow the code-seer through opened archive doors into a canyon of commit constellations. | Timed exit to `convergence`; A/Start skips to `title`. | `play-opening-fanfare` | `opening-archive`, `opening-soundscape` |
| `convergence` | Join source energy and earned knowledge without spending the climax early. | Cyan enters from the left, gold from the right, and an incomplete Oracle eye forms around a dark seed. | Timed exit to `oracle-awakening`; A/Start skips to `title`. | `play-opening-fanfare` | `opening-convergence`, `opening-soundscape` |
| `oracle-awakening` | Resolve the story in the existing hero image instead of using it as the whole intro. | The complete cyan-and-gold Oracle sigil ignites around the code-seer; the frame reaches the sequence's maximum contrast. | Timed or A/Start exit to `title`. | `play-opening-fanfare` | `opening-fanfare`, `opening-soundscape` |
| `title` | Resolve the fanfare into an invitation from the Oracle. | A/Start begins; the redrawn Oracle motif and title remain readable without glow. | `quiz-menu` | `begin-from-title` | `title-mark`, `ui-soundscape` |
| `quiz-menu` | Explain the run and offer a safe return. | D-pad selects; A/Start confirms; B returns; focus is visible by shape and color. | `character-creation` or `title` | `navigate-menu` | `menu-frame`, `ui-soundscape` |
| `character-creation` | Give the player identity while the first question request is already in flight. | Change name, path, and aura through disjoint, centered identity rows; the hero's visible feet stay grounded on the atelier stage; aura selects an authored hero colorway without procedural equipment overlays. | `oracle` | `customize-hero` | `hero-set`, `character-frame`, `ui-soundscape` |
| `oracle` | Turn real generation latency into a safe, active interstitial. | Left/Right changes lanes; data fills charge runes at 3/6/9 and bug hits break containment runes at 1/3/5. A/Start remain inactive; B abandons the wait safely. The top header holds truthful Oracle context and the bottom strip holds themed instruments plus controls. | Automatically enters `quiz` when a valid question is ready; B returns to `quiz-menu`. | `consult-oracle` | `hero-set`, `oracle-sanctum`, `oracle-soundscape`, `run-progression` |
| `quiz` | Test one durable project concept. | D-pad selects; A commits; B abandons the run; ward, flow, score-rune, text, shape, animation, and sound states reveal consequence and reward. | `oracle`, `level-up`, `game-over`, or `quiz-menu` | `answer-question` | `hero-set`, `quiz-frame`, `quiz-soundscape`, `run-progression` |
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
| Copyright card | Before the six-second story | Begin almost black. The repository title, up to three author lines, and earliest → latest commit dates are engraved in a dim archive frame with no emissive glow. Show a literal © owner only when an explicit repository notice supplies it. | The first request has already started when the cartridge was accepted. | A/Start becomes available after the text has had one readable second. |
| `opening-fanfare` — Source ember | 0.0–1.6s | A lone code-seer enters a vast dormant code-cathedral and discovers one cyan source ember over an ancient altar. The Oracle and gold are absent. | Continue silently; no percentage, spinner claim, or completion implication. | A/Start becomes available after 1.5 seconds and skips directly to title. |
| `archive-answer` — Archive answer | 1.6–2.7s | The code-seer reaches out. One cyan pulse travels through the altar and wakes the nearest archive monoliths. | Continue in the background; cache an early result without interrupting the sequence. | A/Start skips directly to title. |
| `memory-vault` — Memory vault | 2.7–3.8s | The monoliths split into enormous vault doors. The code-seer crosses a bridge beneath a deep constellation of commits and branching memory. | Continue generation without changing timing. | A/Start skips directly to title. |
| `convergence` — Convergence | 3.8–4.9s | Cyan source streams and restrained gold knowledge streams meet in a circular chamber. Only an incomplete eye appears inside the still-dark Oracle seed. | Continue generation without implying readiness. | A/Start skips directly to title. |
| `oracle-awakening` — Oracle climax | 4.9–6.0s | The existing `awakening.png` composition resolves the story: the complete cyan/gold Oracle eye ignites above the same code-seer at maximum luminance. | Finishing this beat never promises that questions are ready. | A/Start or the timer exits to title. |
| Title handoff | After 6.0s | Energy clears completely. The title scene redraws the eye motif at a restrained baseline; no fanfare layer leaks across the state boundary. | Continue generation through title, menu, and hero creation if needed. | A/Start begins the normal menu flow. |

If the first batch is ready early, it waits safely for the player. If it is
still unavailable after hero creation, the existing Oracle scene communicates
the real wait and retry states. The opening never stretches itself to fake a
dependency on generation.

## Oracle micro-storyboard

| Beat | Trigger | Presentation | Player agency | Status |
|---|---|---|---|---|
| Arrival | Enter `oracle`. | Reset the hero to center, clear in-flight objects, and show the real Claude status. Keep the existing minimum dwell so instant results do not flash past. | Left/Right begins moving immediately; B returns to the quiz menu; all other controls remain inactive. | Implemented. |
| Datafall | Request is in flight. | Authored cyan-and-gold crystal shards and asymmetric magenta corruption glyphs fall through deterministic lanes. Data and bug-hit counts persist across Oracle visits; three charge runes light at 3/6/9 data while three containment runes break at 1/3/5 hits. | Move into data to collect it automatically; move away from bugs. | Implemented and breakpoint-tested. |
| Clouded vision | A batch returns empty or invalid. | `CLAUDE RETRYING` distinguishes the real retry delay without a fake percentage. Falling-object play continues. | Left/Right remain available. | Implemented. |
| Vision ready | A valid unanswered question exists. | `QUESTION READY` may appear during the minimum dwell, then the scene transitions automatically. | No confirmation required; held D-pad inputs cannot answer the quiz. | Implemented. |
| Long wait | Scrying continues beyond the normal beat. | The same deterministic play loop continues under truthful status copy, with no invented scan steps. | Keep playing until the question arrives. | Implemented. |

The Oracle never rewards a slow response, suggests that Datafall speeds up the
model, or hides a failed request behind invented progress. Datafall score and
collisions are deliberately isolated from quiz wards, score, question timing,
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

- **Decision:** Watch the complete six-second spectacle or skip after
  its opening impact.
- **Inputs:** A or Start.
- **Rules:** The sequence is finite and deterministic. Completion never implies
  question readiness. Reduced motion changes transitions, not duration or data.
- **Feedback:** Carry one code-seer through five distinct authored frames:
  source ember, archive answer, memory vault, convergence, and complete Oracle.
  Each cut changes place or causality—not merely brightness—and total luminance
  rises monotonically into the existing climax. Sound grows from one node pulse
  to the full Oracle cadence in sync with that arc.

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
  for final confirmation. Keep the heading disjoint from the first identity row,
  center `BIND` in the button's usable interior, and ground the hero's visible
  feet on the stage support line.

### `consult-oracle`

- **Decision:** Choose a lane, dodge corruption glyphs, and collide with crystal
  data shards.
- **Inputs:** Left and Right move. B returns to the quiz menu. Up, Down, A,
  Start, and shoulders are inactive.
- **Rules:** Drops use deterministic lanes and alternate data/bug types. Data
  overlap increments a cosmetic data counter; bug overlap increments a cosmetic
  hit counter. Active drops reset on each Oracle entry, while data/hit counters
  persist for the current quiz run. Data lights charge runes at 3, 6, and 9;
  bug hits break containment runes at 1, 3, and 5. Stay until a valid
  unanswered question exists; empty results retry. No Datafall state affects
  generation, difficulty, quiz score, wards, or wait duration. A B press exits
  on its input edge and clears the active run, so the wait is never inescapable.
- **Feedback:** Data uses an authored cyan-and-gold crystal silhouette; bugs
  use an asymmetric magenta corruption silhouette. Keep `DATAFALL` and truthful
  loading/retry/ready text in the top header. Compose raw two-digit counts,
  three charging data runes, three breakable containment runes, and move/back
  controls in the bottom strip without collisions. Give each breakpoint, retry,
  ready state, and back action a distinct response while keeping ambience below
  quiz feedback.

### `answer-question`

- **Decision:** Commit to one of four conceptual answers.
- **Inputs:** D-pad, A, B.
- **Rules:** Exactly four distinct choices, one correct answer, a maximum of
  four 31-character question lines, and 31 characters per choice. Wrong costs
  one ward and resets flow. Correct builds flow; streaks 0–2, 3–5, and 6+ award
  x1, x2, and x3 score. Score awakens Insight Runes at 300, 900, and 1800.
  Commitment immediately records the question in the cartridge save; future
  runs or launches compare normalized text and skip every recorded question.
  After commitment, hold the revealed answer for 45 ticks with every input
  intentionally inactive.
- **Feedback:** Keep the correct choice visible in green after either result;
  show an incorrect committed choice in red. Do not add redundant correct/wrong
  words. Use visibly spaced glyphs that begin beyond the plate's left divider,
  keeping every ornament outside glyph and inter-glyph cells, plus distinct
  cursor, commit, low-ward, and batch-complete cues. Replace generic health
  stars with three Oracle ward runes; compose ward, flow multiplier, three
  Insight Rune marks, and raw score across the header. During the input lock,
  replace active controls with the owned review, ward, flow, or exact rune-
  crossing banner.

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
| `opening-source` | Scene | `opening-fanfare` | Establish the code-seer, dormant cathedral, and isolated source ember. | Authored 240×160 plate; no Oracle or gold; quiet skip region. | `awakening-source.png` implemented and native-inspected. |
| `opening-signal` | Scene | `archive-answer` | Show the code-seer's action causing the altar and monoliths to wake. | Authored 240×160 plate; same character/world; more cyan without Oracle or gold. | `awakening-signal.png` implemented and native-inspected. |
| `opening-archive` | Scene | `memory-vault` | Open repository history into a bridge-and-commit-constellation vista. | Authored 240×160 plate; distinct depth composition; restrained amber nodes only. | `awakening-archive.png` implemented and native-inspected. |
| `opening-convergence` | Scene | `convergence` | Bring cyan and gold together around an incomplete Oracle seed. | Authored 240×160 plate; distinct circular chamber; no completed sigil or maximum white. | `awakening-convergence.png` implemented and native-inspected. |
| `opening-fanfare` | Scene/VFX | `oracle-awakening` | Resolve the five-scene story in the complete Oracle crescendo. | Existing authored 240×160 plate; brightest cyan/gold only here; clean title handoff. | `awakening.png` retained as the climax; five-beat luminance and distinctness tested. |
| `title-mark` | Logo/UI | `title` | Identify the cartridge and Oracle motif; idle and prompt-pulse states. | Legible at 240×160 without glow. | `oracle-title` implemented. |
| `menu-frame` | UI | `quiz-menu` | Carry the cathedral/rune language into focused and idle menu states. | Focus differs by pointer, shape, and color; no unowned empty space. | `oracle-menu` implemented. |
| `character-frame` | UI/scene | `character-creation` | Stage the customizable hero inside the same world with centered name, path, and aura rows plus loading, retry, and ready states. | Heading and rows are pairwise disjoint; labels and actions center in measured usable interiors; the hero's visible-alpha feet meet the stage support line; status is truthful. | `oracle-atelier` implemented with native layout assertions. |
| `hero-set` | Sprite set | `character-creation`, `oracle`, `quiz`, `level-up`, `game-over` | Carry identity through the run with authored aura colorways plus idle, dodge, reward, and defeat variants. | Consistent silhouette across palettes/backgrounds; no procedural accessory or weapon overlays. | `oracle-hero` implemented with authored colorways, portrait, and defeat variants. |
| `oracle-sanctum` | Scene/UI | `oracle` | Present Datafall, loading, retry, ready, and B-back as one place: moving hero, authored drops, staged data-charge and corruption-containment instruments, and a tier crest. | Fits 240×160; sprites stay contained and differ by silhouette/value/hue; top status remains distinct; raw counts and themed three-rune meters remain disjoint from centered controls at two digits. | `oracle-sanctum` implemented with authored drop sprites, themed threshold runes, exact breakpoint tests, and three visual tiers. |
| `quiz-frame` | HUD/UI | `quiz` | Hold question, four choices, focus, ward health, flow multiplier, score, Insight Runes, and answer review. | Honor text limits and plate-divider clearance; replace generic pips with three stateful Oracle runes; preserve raw score while exact flow/score thresholds change reward, fill, color, and concise response copy. | `oracle-trial` implemented with ornament-disjoint copy, shaped focus, themed instrumentation, staged scoring, and breakpoint tests. |
| `reward-frame` | Scene/UI | `level-up` | Telegraph the threshold, celebrate it once, and establish the new Oracle-bond baseline. | No rapid full-background flashing; tier and reward remain readable. | `oracle-ascension` implemented with tiered crest and readable hold. |
| `result-frame` | Scene/UI | `game-over` | Resolve the run with hero state, final score, earned Insight Rune, level, completed crest, and an obvious reset/replay path. | Defeat is clear without erasing earned progress; all accepted replay inputs are visible. | `oracle-aftermath` implemented with preserved tier, score-rune rank, and defeated hero. |
| `run-progression` | Presentation system | `oracle`, `quiz`, `level-up`, `game-over` | Own every tracked metric's direction, thresholds, crossing response, cap, reset, and carry-forward state. | At least two non-numeric channels change; exact breakpoints are tested; all run metrics reset deterministically. | Bond, ward, flow, Insight Rune, data-charge, and corruption-containment stages are implemented; tier audio remains proposed. |

## Sound requirement ledger

Sound entries are currently `art.kind = "audio"` production metadata. The
engine does not load or play them yet.

| ID | Used by scenes | Player-facing purpose | Cues/loops and variants | Constraints and acceptance | Status |
|---|---|---|---|---|---|
| `opening-soundscape` | `copyright` and all five opening story scenes | Make the dormant-to-Oracle reveal audible and give the credits intentional restraint. | Archival tick, source pulse, archive answer, vault branches, convergence, Oracle cadence, clean tail. | Constrained chip-style palette; starts near silent; fullest arrangement only at the crescendo; reduced-audio variant. | Proposed |
| `ui-soundscape` | `title`, `quiz-menu`, `character-creation` | Make focus, choice, cancel, customization, and confirmation instantly legible. | Navigate, confirm, cancel, unavailable, trait variants, begin-run cadence. | One input edge produces at most one cue; no cue crosses scenes unintentionally. | Proposed |
| `oracle-soundscape` | `oracle` | Separate Datafall play from truthful loading state without overwhelming it. | Low ambience; data, bug, retry, ready, and B-back cues; three progression-tier variants. | Loops stop on quiz/menu transition; status remains readable when muted. | Proposed |
| `quiz-soundscape` | `quiz` | Clarify cursor movement, answer commitment, result, danger, and batch completion. | Cursor, commit, correct, wrong, low-ward, flow-stage, batch-complete cues. | Correct/wrong never rely on sound alone; prevent stacked result cues. | Proposed |
| `progression-soundscape` | `level-up`, `game-over` | Make thresholds, earned tier, defeat, and reset feel conclusive. | Telegraph, reward cadence per tier, defeat fall, score hold, replay/reset cadence. | Reward arrangement grows by tier; new run audibly returns to Initiate. | Proposed |

## Whole-game polish matrix

| Scene | Static | Motion | Sound | Mechanical closure | Felt progression | Evidence/status |
|---|---|---|---|---|---|---|
| `copyright` | Dim archive frame with hierarchy equal to gameplay; no glow. | Credits reveal in fixed readable steps, then clear cleanly. | Intentional near-silence; archival ticks remain proposed. | A/Start after minimum dwell or timed exit; missing provenance has explicit copy. | Establishes the dormant baseline and repository identity. | Visual template implemented and native-frame tested; audio proposed. |
| `opening-fanfare` | Lone code-seer, dormant cathedral depth, and one cyan ember establish a clear focal hierarchy. | A 1.6-second held establishing shot gives the story and skip gate time to read. | One source pulse remains proposed. | Elapsed enters `archive-answer`; A/Start becomes visible at 1.5 seconds and skips to title. | Establishes the darkest authored opening baseline. | Dedicated native plate and executable transitions implemented/tested; audio proposed. |
| `archive-answer` | Same code-seer and cathedral, with lit floor paths and waking monolith silhouettes. | The composition itself advances causality; staged cut avoids a full-frame flash. | Altar response remains proposed. | Elapsed enters `memory-vault`; A/Start skips to title. | Cyan expands into a second visual channel: environment response. | Dedicated native plate and executable transitions implemented/tested; audio proposed. |
| `memory-vault` | Foreground doors, bridge, and deep commit constellation create a new three-plane composition. | Staged cut moves the player deeper into the archive. | Branching commit sequence remains proposed. | Elapsed enters `convergence`; A/Start skips to title. | Space, node density, and narrative scale visibly expand. | Dedicated native plate and executable transitions implemented/tested; audio proposed. |
| `convergence` | Circular chamber, same code-seer, opposing cyan/gold streams, and incomplete dark eye. | Staged cut turns propagation into convergence without spending maximum white. | Two-voice convergence remains proposed. | Elapsed enters `oracle-awakening`; A/Start skips to title. | Gold joins cyan and the goal is visibly telegraphed. | Dedicated native plate and executable transitions implemented/tested; audio proposed. |
| `oracle-awakening` | Existing high-detail awakening composition becomes the earned final image. | Center luminance rises during the final hold, then clears cleanly to title. | Full Oracle cadence remains proposed. | Elapsed or A/Start lands on a fresh title frame. | Reaches the opening's brightest/densest state after four distinct scenes. | Existing plate retained; five-frame distinctness and luminance arc implemented/tested; audio proposed. |
| `title` | Restrained Oracle motif and legible title at native scale. | One controlled eye/prompt pulse never competes with title. | Title loop and start cue remain proposed. | A/Start continues; unavailable cartridge state remains honest. | Returns to an Initiate baseline while preserving the Oracle promise. | Visual template implemented/tested; audio proposed. |
| `quiz-menu` | Rune frame and shaped focus for both choices. | Focus moves on input edge. | Navigate, confirm, and cancel cues remain proposed. | Begin and back are explicit; held input cannot double-confirm. | New run previews the Initiate palette and reset. | Visual template implemented/tested; audio proposed. |
| `character-creation` | Authored hero, pairwise-disjoint identity rows, a grounded stage placement, centered action copy, and Oracle status form one staged composition. | Aura changes the authored colorway; identity rows react immediately; begin has one clean handoff. | Trait and begin cues remain proposed. | Every row stays centered and contained; B returns; loading/retry/ready states are truthful. | Establishes identity that remains visible across the run. | Native interior, support-line, and sibling-bound assertions implemented; audio proposed. |
| `oracle` | Sanctum, playfield, status, raw counts, themed charge/containment runes, controls, and tier crest remain distinct. | Exact 3/6/9 gains light charge runes; 1/3/5 hits break containment runes; Datafall and status retain owned exits. | Ambience and threshold cues remain proposed. | Questions-ready enters quiz; B abandons safely; all other controls are intentionally inactive. | Clean play preserves seals while collection fills runes; bond visuals retain all three tiers. | Native meter layout and exact first-breakpoint frame changes implemented/tested; audio proposed. |
| `quiz` | Question, choices, hero token, ward runes, flow multiplier, Insight Rune meter, raw score, and tier frame remain readable; answer copy clears every ornament. | Cursor, commit, 45-tick review, ward loss, x2/x3 flow, 300/900/1800 rune crossings, and batch threshold have causal timing. | Quiz cues remain proposed. | Green/red answer copy plus ward/flow/rune banners appear without redundant result words; every automatic outcome routes visibly. | Score reward changes mechanically at streak thresholds and earned runes persist into results. | HUD siblings, ward states, exact scoring breakpoints, and choice/plate bounds implemented/tested; audio proposed. |
| `level-up` | Hero and newly expanded crest dominate; level text supports rather than carries reward. | Crest growth → hero rise → one-second hold → continue; stable background avoids flashing. | Reward cadence remains proposed. | A/Start is inactive for 60 ticks, then routes to ready quiz or Oracle wait. | Explicit threshold celebration establishes the new visual baseline. | Visual template/progression implemented/tested; audio proposed. |
| `game-over` | Defeated hero, final score, Insight Rune rank, level, and earned crest share one conclusive frame. | Energy recedes without erasing earned bond or score rank; replay resets both on the next run. | Defeat/result/reset cues remain proposed. | The visible A/B/Start prompt returns to menu; the next new run resets all run state. | Shows the exact bond and Insight Rune stages reached before reset. | Visual template/progression and score-rune result implemented/tested; audio proposed. |

## Runtime traceability

| Element | Status | Evidence or required work |
|---|---|---|
| Manifest title and `quiz`/`quest` type | Implemented | Parsed at cartridge load and used by the engine. |
| Scene graph | Configured/executable | Schema-v2 handlers and semantic transitions are validated, compiled, and executed by the engine. |
| Mechanic and presentation graph | Mixed | Mechanics and audio remain metadata; typed visual templates are parsed, validated, and executed by scene renderers. |
| First question request at cartridge acceptance | Implemented | Empty quiz cartridges call the question effect immediately when inserted. |
| Repository authors, timeline, and explicit copyright extraction | Implemented | Cartridge preparation reads sanitized git shortlog/history data and scans bounded LICENSE/COPYRIGHT/NOTICE files. Commit authors are never treated as legal owners. |
| Copyright and five-scene opening story | Implemented with asset-backed templates | Trusted Bevy handlers render the chronicle, source ember, archive answer, memory vault, convergence, and Oracle awakening before `Title`; manifest timing gates control per-scene auto-advance and direct skip while fanfare/title frames remain separate. |
| Title, menu, hero creation, Oracle, quiz, level-up, and game-over screens | Implemented | Trusted handlers own input and rendering while the manifest routes their semantic events. |
| First request, prefetch, invalid-batch retry, and Oracle hold | Implemented | Engine question effects, pending batches, and retry timer. |
| Oracle Datafall with safe recovery | Implemented in this pass | Held movement, authored drops, automatic counters, 3/6/9 charge runes, 1/3/5 breakable containment runes, split HUD, and B-back close the indefinite wait. |
| Themed run instrumentation and score thresholds | Implemented in this pass | Oracle ward glyphs replace stars; x1/x2/x3 flow changes score awards; 300/900/1800 Insight Runes change HUD and review feedback; exact breakpoints and native sibling bounds are tested. |
| Safe Oracle-to-quiz input boundary | Implemented | A/Start are ignored in Oracle; B exits to the menu; held D-pad controls have no answer action after the automatic transition. |
| Quiz result and reward input boundaries | Implemented in this pass | The 45-tick answer reveal replaces active controls, and level-up enforces a 60-tick hold before A/Start can leave. |
| Answered-question continuity | Implemented | Answer commitment records question text under `quiz.progress`; cartridge reload compares normalized text and filters recorded questions while serialized save updates preserve the independent Claude-batch namespace. |
| Truthful multi-state Oracle presentation | Implemented with asset-backed templates | Loading, retry, and ready copy derives from actual engine state; B provides recovery from a permanently unavailable generator. A dedicated disabled explanation remains future work. |
| Concise answer review and reduced motion | Partially implemented | Green/red answer copy, shaped focus, stable level-up, and staged opening motion are implemented; a user-selectable reduced-motion setting remains proposed. |
| Visual templates selected from manifest | Implemented | Eleven typed built-in templates are selected by `art[].template`; `oracle-awakening` selects five art-ID-addressed opening plates, other Oracle templates composite their native illustrated plates and live state, unknown names fail validation, and untemplated cartridges keep their legacy renderers. |
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
   templates cover all thirteen reachable scenes, beginning with the dormant
   `copyright-card` and culminating in the fifth opening beat's Oracle
   crescendo.
8. **Sound runtime pass:** Add bounded template-audio selection and playback,
   then implement every entry in the sound ledger with scene-owned loop exits.
9. **Partially completed — Felt-progression pass:** Tiered visuals carry the
   crest, palette, circuit density, and hero identity across Oracle, quiz,
   reward, and results; tiered audio remains part of the sound runtime pass.
10. **Completed — Executable scene graph:** Add schema v2 handlers, semantic
   transitions, timing gates, reachability validation, built-in quiz/quest
   templates, and schema-v1 compatibility.
11. **Completed — Answered-question continuity:** Record committed questions in
    the cartridge save, filter them on later runs and launches, and serialize
    namespace updates so background batch writes cannot erase progress.
12. **Completed — Instrument and threshold pass:** Replace generic health pips
    with Oracle ward runes; attach exact stages to flow, score, Datafall charge,
    corruption containment, batches, and bond; verify native layout and exact
    breakpoint transitions.

## Open decisions

- Should the generated question payload eventually include a short explanation,
  or is revealing the correct choice enough feedback at this resolution?
- Beyond answered-question history, should score, wards, hero identity, or
  presentation tier persist per cartridge across launches?
- Should B from an active quiz abandon immediately, or use an in-scene
  confirmation state before leaving the run?
