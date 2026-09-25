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
metadata. Sound is derived by the engine from game state, so the manifest's
audio entries are production requirements rather than runtime switches.

## Experience frame

**Pitch:** A copyright-style repository chronicle names the people and history
behind the cartridge, then unfolds through an original five-scene code-fantasy fanfare before
the player binds a small code-seer, consults an Oracle, and proves their
understanding through increasingly difficult conceptual questions. The first
question request runs behind the opening spectacle, and any remaining latency
becomes Oracle Datafall: a Left/Right falling-object game in which the hero
dodges bugs and runs into data while the verified battery provider works.

**Learner:** A developer who can read code but is new to, or returning to, the
inserted repository. They may know its language and framework; they do not yet
know why the project is shaped the way it is.

**Learning objective:** After a run, the learner can explain the project's
purpose and which component owns each duty, predict how components collaborate
when a request or event moves through them, and justify the invariants and
tradeoffs that constrain a change, distinguishing each from a plausible
misconception.

**Target mental model:** A durable model of the project across five concept
lenses (`learning::Concept`): purpose, responsibilities (`ROLES`), interactions
(`FLOWS`), invariants, and tradeoffs. Concretely, a responsibility map (what
owns which decision), an interaction model (what calls or informs what, and in
which direction), and the invariants and tradeoffs that explain why the
boundaries sit where they do—not file names or repository trivia.

**Likely misconception:** Surface models that confuse where code lives or what
a name suggests with what a component is responsible for, for example
believing a presentation layer owns state because it displays it. The
generation prompt asks for every distractor to be a misconception a newcomer
might really hold, and acceptance requires a rationale for every choice, so a
wrong answer names the model the player held.

**Evidence of learning:** A correct commitment to a conceptual question on its
first attempt, or a *redemption*: a correct answer, in a later launch, to a
previously missed question, presented in a different choice order from its
first showing. A miss's review copy always returns after exactly three other
questions (`RETRY_GAP`); when the batch ends sooner, the copy carries into the
next batch (waiting for the next delivery if none is queued) and the current
batch still closes on time. A correct same-launch retry is *relearning*
(`learning::Review::InSession`): it clears the pending review and is bannered
`REDEEMED`, but it follows the lesson card that just showed the answer, so it
lights no rune. Every missed question, including a relearned one, returns once
as a spaced check in a later launch, and only a correct answer there retires
it and counts as a redemption. A delivered question whose stem the lesson
journal already holds is never a first try: it is graded as relearning too.
Score, flow, survival, and Datafall counts are not evidence.

**Mastery criterion:** Per lens, evidence (first-try successes plus
later-launch redemptions) sets how many runes volume alone would wake: 1, 3,
and 5 (`learning::MASTERY_THRESHOLDS`). Two gates read the lens's newest five
graded outcomes (first tries, spaced checks, and every miss; same-launch
relearning is not graded): rune II needs at least 60% recent accuracy, and
rune III needs at least 80% and no pending review on the lens
(`LensRecord::stage_with`). A rune the volume earned but a gate holds back is
drawn *cracked* (amber outline split by a crack, no core) rather than going
dark, so a slip reads as a review due, not lost progress. A lens with three
lit runes is therefore mastered: recent accuracy is high and nothing on it is
pending. Saves from earlier builds carry no recent outcomes, so only the
pending-review gate applies to them until new answers arrive.

**Transfer task:** At Oracle-bound levels (4 and above), the generation prompt
requires at least two PREDICT questions per six-question batch. Each describes
a plausible change or failure the repository does not document—a removed
check, a reordered step, a crash mid-operation, a new kind of input—and asks
what the design implies, answered from invariants and tradeoffs rather than
recalled wording. The requirement lives in the prompt: acceptance does not
check the `PREDICT:` prefix, and a PREDICT question counts as evidence for
whichever lens it names.

**Player goal:** Bind a code-seer to the Oracle, survive trials with three
wards, deepen the Oracle bond, and fill the Codex with awakened lens runes.

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
  → read the lesson card → continue or return to the Oracle.
- **Progression loop:** Survive a full six-question batch, including any review
  copies its misses inserted within the retry gap → raise difficulty and
  deepen the batch's concept lenses → mark a level-up with the batch's
  first-try recap and the lenses the next batch focuses on → visibly deepen
  the Oracle bond → begin or await the next batch.
- **Success:** Correct answers build flow; streaks 3 and 6 raise the score
  multiplier to x2 and x3, cumulative score awakens Insight Runes at 300, 900,
  and 1800, and completing a batch raises the level. First-try successes and
  later-launch redemptions also wake per-lens mastery runes at 1, 3, and 5
  evidence, gated by recent accuracy and pending reviews.
- **Failure/recovery:** A wrong answer costs one ward, resets flow, and opens a
  lesson card that explains the chosen misconception before the correct answer
  and its rationale. A survivable miss returns as a review three questions
  later, in the next batch if needed. At zero wards, show the final score,
  earned Insight Rune, the run's learning ledger (first-try successes,
  corrected misses, open reviews, and the lens that woke, or the Codex while
  reviews are open), and a steady one-button replay path; the unredeemed miss
  waits for a later run.
- **Save continuity:** Committing an answer records it in the cartridge save
  (`quiz.progress`) on a background writer. A first-try success or a correct
  spaced check retires the question for later runs and launches; a miss keeps
  it queued as a review, and a correct same-launch retry moves it to
  `relearned_questions`, queued once more as a spaced check for the next
  launch. Per-lens mastery (first-try, redeemed, relearned, and missed counts,
  plus the newest eight graded outcomes) persists per cartridge, and cartridge
  load rebuilds the lesson journal from the saved batches and that progress.
  Every new field defaults, so older saves load unchanged. Wards,
  score, flow, and presentation remain run-specific; hero identity is not
  save-backed across app launches.
- **Latency loop:** Request the first batch as soon as the cartridge is
  accepted, then continue behind the copyright, fanfare, title, menu, and hero
  creation. In Oracle Datafall, move into falling data to collect it on contact
  while moving away from magenta corruption glyphs.
  Enter the Oracle only when no valid unanswered question is ready; failed
  batches retry there instead of becoming generic trivia.

## Pedagogy map

| Stage | Player experience | Concept or skill | Evidence/feedback | Scaffolding |
|---|---|---|---|---|
| Introduction | Initiate batches (level 1) ask about purpose and responsibilities while the opening, menu, and Datafall wait carry the fiction. | Purpose and responsibilities: the prompt asks for at least four of six questions on the level's focus lenses (`Concept::focus_for_level`). | First-try answers; every commitment opens a lesson card with the answer's rationale. | Choices are shown in a stable per-question shuffle (`learning::presentation_order`) so answer position carries no signal; the prompt asks for plausible distractors of similar length; there are no timers. |
| Scaffolded practice | A miss shows the committed pick in red with the misconception it represents, then the answer in green with why it holds. The question returns as a review after exactly three other questions with every choice moved. | The misconception named by the chosen distractor. | A correct same-launch review is bannered `REDEEMED` and recorded as relearning; only a correct spaced check in a later launch is a redemption and mastery evidence. | The card ignores input for 45 ticks and then waits for A or Start; a review copy that would land past the batch end carries into the next batch, so the gap never shrinks and the batch still levels up on time. |
| Independent practice | Adept batches (levels 2–3) shift to interactions and tradeoffs; flow multipliers reward consecutive correct answers. | Interactions and tradeoffs. | Per-lens first-try, redeemed, relearned, and missed counts plus the newest eight graded outcomes in the save. | Each request names the weakest lens (most misses relative to evidence) and asks for at least one question on it, and lists up to 24 earlier stems the provider must not repeat. |
| Assessment | The Codex mastery page shows every lens's three runes and an amber pending-review count; learned lesson pages reread question, answer, and rationale marked `LEARNED`; a pending lesson (`REVIEW PENDING`) is a self-test that shows the player's wrong pick and its misconception and seals the answer until A. The quiz menu summarizes `LESSONS NN  REVIEW NN`. | All five lenses. | Runes at exactly 1, 3, and 5 evidence, with rune II gated at 60% and rune III at 80% recent accuracy plus no pending review; a gated rune shows cracked. Pending reviews clear when a miss is answered correctly; a correct answer after revealing the answer is relearning, not evidence. | The journal can be reread from the menu before any run; missed questions persist across launches until a spaced check answered without a peek retires them. |
| Transfer | Oracle-bound batches (level 4+) focus on invariants and tradeoffs and include at least two PREDICT questions about undocumented changes or failures. | Invariants and tradeoffs applied to a new situation. | A correct prediction is evidence for the lens the question names. | Rationales explain the implication, so a wrong prediction still teaches the invariant. |

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
| Initiate — level 1 | Purpose and responsibilities | Sparse cyan nodes; the Oracle eye is mostly dormant. | Two-voice Datafall loop and two-voice reward cadence; the replay cadence always returns to this thin timbre. | The chosen hero and first lit node persist through Oracle, quiz, and results. |
| Adept — levels 2–3 | Component interactions and tradeoffs | Gold branches join the cyan frame; transitions gain one extra anticipation beat. | A third voice joins the Datafall loop and the reward cadence. | The expanded crest remains visible in the next Oracle and quiz frames. |
| Oracle-bound — level 4+ | Invariants and tradeoffs, including PREDICT transfer questions | Cyan and gold converge into the complete Oracle crest; reward motion reaches its maximum controlled intensity. | Four-voice Datafall loop and the full Oracle reward cadence. | The complete crest holds until game-over or a clearly signaled new-run reset. |

Crossing a tier is celebrated on `level-up` and established as the new visual
baseline afterward. A new run visibly returns to Initiate. Typed template
selection, the three visual tiers, and the tiered Datafall and reward
arrangements are implemented.

### Tracked-metric threshold contract

Every player-visible quantity has an owned direction and staged response. The
raw number remains when precision matters, but it is paired with Oracle-shaped
runes so the HUD reads as part of the illustrated world rather than debug text.

| Metric | Desired direction | Exact stages | Reward or consequence | Cap and reset |
|---|---|---|---|---|
| Wards (health) | Keep high | 3 full → 2 strained → 1 fractured → 0 broken | Three cyan/gold ward runes lose fill and change state; the lesson-card banner names strain, fracture, or break, and a heartbeat bed runs on the last ward. Zero ends the run. | Capped at 3; restored by a new run. |
| Flow (correct-answer streak) | Build high | 0–2 = x1, 3–5 = x2, 6+ = x3 | Each correct answer awards 100 × the active multiplier; the header and lesson-card banner establish the new flow stage. | Multiplier caps at x3; a wrong answer or new run resets it. |
| Insight score | Build high | 300 = Rune I, 900 = Rune II, 1800 = Rune III | Three header runes awaken at exact crossings, score color advances, and a crossing banner names the earned stage (`INSIGHT II RISES`); a lens-rune wake on the same commit takes the banner and the cue. | Visual rank caps at Rune III while the readable score continues to 9999; a new run resets both. |
| Data charge | Build high | 3, 6, and 9 collected shards | One of three bottom-strip charge runes lights at each threshold. This is expressive reward only and never changes question generation or quiz score. | Rune meter caps at 3; count displays to 99; both reset on a new run. |
| Corruption hits | Keep low | 0 intact; first seal breaks at 1, second at 3, third at 5 | Three containment runes visibly fracture/extinguish in stages, rewarding a clean wait while warning before breach. This remains isolated from quiz health. | Breach display caps after 5; count displays to 99; both reset on a new run. |
| Questions/batches | Complete six | A batch holds six generated questions; each survivable miss inserts its review copy three questions later and moves every batch end at or past it, so a copy inside the batch extends it and a later copy joins the next batch (or waits for the next delivery); a batch completes at its retry-shifted end and only once it holds a full six | A batch-complete scene raises level and holds the new bond state before continuation. | Continues while questions are available; new run resets batch and level, and unanswered or unredeemed questions carry into it. |
| Oracle bond level | Build high | level 1 Initiate; 2–3 Adept; 4+ Oracle-bound | Palette, crest geometry, circuit density, reward frame, final result, generation focus lenses, and arrangement voices change—not just the number. | Visual tier caps at Oracle-bound; new run resets to Initiate. |
| Lens mastery (per lens) | Build high | Evidence (first-try successes plus later-launch redemptions; a correct answer after revealing the answer in the Codex is relearning, not evidence) 1 = rune I, 3 = rune II, 5 = rune III; rune II also needs 60% of the newest five graded outcomes correct, and rune III 80% plus no pending review on the lens | The Codex mastery page lights the lens's runes in a color that advances with the stage (cyan, amber, magenta), and the templated lesson-card footer shows the same meter beside the lens label. A rune the volume earned but a gate holds back is drawn cracked in amber, and the Codex totals row then reads `CRACKED = REVIEW DUE`. A rise of the gated stage is the run's loudest moment: the top banner (`FLOWS RUNE II`), a blinking footer rune through the input hold (cracked runes beside it hold still), and its own two-note lens-wake chime, each outranking every score threshold. | Caps at three runes while evidence keeps counting; recent outcomes keep the newest eight; persists per cartridge across runs and launches and is never reset by a new run. |
| Pending reviews | Keep at zero | 0 = `ALL CLEAR`; each outstanding miss adds one | An amber `!N` beside the lens on the Codex mastery page, `REVIEW NN` in the Codex totals (while no rune is cracked) and quiz-menu subtitle, and `REVIEW PENDING` (or `PENDING PEEKED` once its answer was revealed) on the lesson page; a correct retry clears its entry. | Displays to 99; persists across launches until each miss is answered correctly. |

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
| `quiz-menu` | Explain the run, summarize the lesson journal, and offer a new run or the Codex. | D-pad selects; A/Start confirms; B returns to the title; the subtitle reads `LESSONS NN  REVIEW NN` (or `ALL CLEAR`) once lessons exist; focus is visible by shape and color. | `character-creation`, `codex`, or `title` | `navigate-menu` | `menu-frame`, `ui-soundscape` |
| `codex` | Make learning evidence visible and give every miss an informed-retry path. | Page 0 shows each lens with three mastery runes (1/3/5 evidence, cracked where an accuracy or pending-review gate holds one back) and an amber `!N` pending-review count; later pages show one lesson each: lens, question, green answer, and rationale for a lesson marked `LEARNED` in cyan (plus `ONCE CHOSE:` when a misconception is remembered); a lesson marked `REVIEW PENDING` in amber shows `YOU CHOSE`, the red `-` pick, and its misconception, with the answer sealed behind `A:REVEAL ANSWER`. Left/Up/L and Right/Down/R page with wrap and reseal; A/Start reveal a pending lesson's answer and are otherwise inactive; B returns. | `quiz-menu` | `review-lessons` | `codex-frame`, `ui-soundscape` |
| `character-creation` | Give the player identity while the first question request is already in flight. | Change name, path, and aura through disjoint, centered identity rows; the hero's visible feet stay grounded on the atelier stage; aura selects an authored hero colorway without procedural equipment overlays. | `oracle` | `customize-hero` | `hero-set`, `character-frame`, `ui-soundscape` |
| `oracle` | Turn real generation latency into a safe, active interstitial. | Left/Right changes lanes; data fills charge runes at 3/6/9 and bug hits break containment runes at 1/3/5. A/Start remain inactive; B abandons the wait safely. The top header holds truthful Oracle context and the bottom strip holds themed instruments plus controls. The header's second line says why the last request failed and when it retries (`TIMED OUT - RETRY IN 5S`), or otherwise recalls one journal lesson (`REVIEW` or `RECALL`). | Automatically enters `quiz` when a valid question is ready; B returns to `quiz-menu`. | `consult-oracle` | `hero-set`, `oracle-sanctum`, `oracle-soundscape`, `run-progression` |
| `quiz` | Test one durable project concept and explain the answer. | D-pad selects among shuffled choices; A commits; a lesson card then replaces the choices with the misconception and answer rationales and continues on A/Start after its 45-tick hold; B twice within 90 ticks leaves the run; ward, flow, score-rune, lens-rune, text, shape, and sound states reveal consequence and reward. The header shows batch progress (`TRIAL 2/7`, growing as misses insert retries), a returning review copy is labeled `RETRY` in amber before it is answered, and a miss's lesson card says when it returns (`BACK IN N`, `UP NEXT`, `LATER` while its retry waits for the next delivery, or `NEXT RUN`). | `oracle`, `level-up`, `game-over`, or `quiz-menu` | `answer-question` | `hero-set`, `quiz-frame`, `quiz-soundscape`, `run-progression` |
| `level-up` | Recognize a completed batch, recap it, and establish a visibly stronger Oracle bond. | The heading reads `ORACLE BOND ASCENDS` when the level crosses into a new tier and `ORACLE BOND DEEPENS` within one; the recap box shows the batch's `1ST TRY a/b`; until the gate opens the footer names the next batch's lenses (`NEXT: FLOWS+TRADEOFFS`, from `Concept::focus_for_level`). A/Start continue once the manifest's `after_ticks` gate opens (60 ticks here), when the continue prompt appears; the scene continues on its own at 180 ticks. | `quiz` or `oracle` | `continue-after-reward` | `hero-set`, `reward-frame`, `progression-soundscape`, `run-progression` |
| `game-over` | Close the run, show what was earned and learned, and make replay obvious. | Show final score, Insight Rune, tier, and level, then the run's learning ledger: `1ST TRY a/b`, `REDEEMED NN`, `REVIEW NN` (journal lessons still pending) or `ALL CLEAR`, and the lens whose mastery rose most this run with its rune stage, or `SEE CODEX` while reviews are open; the `A/B/START:MENU` prompt never blinks. A/B/Start returns to the menu and clearly resets progression. | `quiz-menu` | `replay-run` | `hero-set`, `result-frame`, `progression-soundscape`, `run-progression` |

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
| Arrival | Enter `oracle`. | Reset the hero to center, clear in-flight objects, and show the real selected-provider status. Keep the existing minimum dwell so instant results do not flash past. | Left/Right begins moving immediately; B returns to the quiz menu; all other controls remain inactive. | Implemented. |
| Datafall | Request is in flight. | Authored cyan-and-gold crystal shards and asymmetric magenta corruption glyphs fall through deterministic lanes. Data and bug-hit counts persist across Oracle visits; three charge runes light at 3/6/9 data while three containment runes break at 1/3/5 hits. | Move into data to collect it automatically; move away from bugs. | Implemented and breakpoint-tested. |
| Clouded vision | A question request fails: no new questions, a rejected batch, or a provider error. | `<PROVIDER> RETRYING` distinguishes the real retry delay without a fake percentage. For the 5-second retry delay the header's second line names the failure's category in amber with a whole-second countdown (`<REASON> - RETRY IN NS`, the reason cut to 24 characters). Once the retry is in flight the line returns to recall, or reads `<REASON> - RETRYING` when the journal is empty. Falling-object play continues. | Left/Right remain available. | Implemented. |
| Recall | The journal holds lessons and no failure is waiting out its delay. | The header's second line recalls one lesson with its lens and answer: an outstanding miss as amber `REVIEW`, a cleared lesson as cyan `RECALL`. Outstanding misses come first, each group newest first, and every wait starts from the top. Each lesson holds for 240 ticks (4 seconds) and types in at two characters per tick; under reduced motion it appears whole. | None; recall is read-only and records no evidence. | Implemented. |
| Vision ready | A valid unanswered question exists. | `QUESTION READY` may appear during the minimum dwell, then the scene transitions automatically. | No confirmation required; held D-pad inputs cannot answer the quiz. | Implemented. |
| Long wait | Scrying continues beyond the normal beat. | The same deterministic play loop continues under truthful status copy, with no invented scan steps. | Keep playing until the question arrives. | Implemented. |

The Oracle never rewards a slow response, suggests that Datafall speeds up the
model, or hides a failed request behind invented progress. Datafall score and
collisions are deliberately isolated from quiz wards, score, question timing,
and the selected provider retries.

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
  question readiness. Reduced motion freezes decorative motion—blinking
  prompts stay lit and settling motions show their end state—without changing
  duration, timing gates, input, or data.
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

### `review-lessons`

- **Decision:** Study a lens or a missed lesson before the next run.
- **Inputs:** D-pad and L/R page; B returns. A or Start reveals a pending
  lesson's sealed answer; on every other page they are inactive.
- **Rules:** Read the cartridge's lesson journal and per-lens mastery; the
  Codex never changes mastery or the question deck. Mastery evidence is
  first-try successes plus redeemed misses from a later launch; same-launch
  retries count as relearning. Volume wakes the first, second, and third rune
  at exactly 1, 3, and 5 evidence; rune II also needs 60% of the newest five
  graded outcomes correct, and rune III 80% plus no pending review on the
  lens. A rune the gates hold back is drawn cracked, and while any rune is
  cracked the totals row reads `CRACKED = REVIEW DUE` in amber. Lessons appear
  oldest first; paging wraps between the mastery page and the newest lesson.
  A pending lesson opens sealed: it shows `YOU CHOSE`, the player's wrong
  pick, and the misconception it reveals (or `THINK, THEN A:REVEAL` for a miss
  recorded before picks were saved), and the answer panel reads
  `A:REVEAL ANSWER`. A reveals a pending lesson's answer; revealing before the
  review counts the next correct answer as relearning, which is saved but is
  not evidence, and the question returns once more as a spaced check. The
  first reveal marks the lesson `PENDING PEEKED` and persists; every page turn
  and every visit seals the page again. The next attempt at the question
  clears the peek. A learned lesson that remembers a misconception adds
  `ONCE CHOSE:` and the pick below its rationale. An empty journal says
  `NO LESSONS YET` and explains that answering trials writes lessons.
- **Instructional role:** Turns a wrong answer from lost health into a
  retrieval-practice prompt: the player faces their own misconception and
  recalls the answer before seeing it, and the lens that needs attention is
  visible, so the retry is informed instead of a second guess, and reading the
  answer first is never mistaken for evidence.
- **Feedback:** Pending reviews are an amber count beside the lens runes and an
  amber `REVIEW PENDING` (or `PENDING PEEKED`) status on the lesson, never
  color alone; the pick carries a `-` marker as well as red. Revealing plays
  one soft reveal cue. Legacy lessons without a rationale say so explicitly.

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
- **Instructional role:** Retrieval practice during the wait. Datafall itself
  keeps the real generation wait active and honest, while the Oracle line
  rereads journal lessons, outstanding misses first. Neither Datafall nor the
  recall line produces learning evidence, and Datafall counts never enter
  mastery.
- **Feedback:** Data uses an authored cyan-and-gold crystal silhouette; bugs
  use an asymmetric magenta corruption silhouette. Keep `DATAFALL` and truthful
  loading/retry/ready text in the top header. The header's second line, the
  Oracle line, shows the failure's category while a failed request waits out
  its 5-second delay: `TIMED OUT`, `CLI UNAVAILABLE`, `RATE LIMITED`,
  `PROVIDER OVERLOADED`, `LOGIN NEEDED`, `BATTERIES UNVERIFIED`,
  `NOT A GIT REPO`, `CALL FAILED`, `REJECTED BATCH`, `SAVE FAILED`,
  `AI DISABLED`, `NO NEW QUESTIONS`, or `GENERATION FAILED`, followed by
  `- RETRY IN NS`. Otherwise it recalls one journal lesson every 240 ticks
  (`ORACLE_RECALL_TICKS`) as `REVIEW <LENS>: <ANSWER>` for an outstanding miss
  and `RECALL <LENS>: <ANSWER>` for a cleared lesson, dropping the lens label
  before any of the answer would be cut. It is omitted when there is neither a
  journal nor a failure, and a ready question hides the failure. Compose raw
  two-digit counts, three charging data runes, three breakable containment runes, and move/back
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
  Commitment immediately records the answer in the cartridge save: a correct
  first try or spaced check retires the question (by normalized text) for later
  runs and launches, a miss keeps it in the save's missed list so it returns as
  a review, and a correct same-launch retry moves it to the relearned list so
  it returns once as a spaced check in the next launch. Choices appear in a
  stable order derived from the
  normalized question and its attempt number, so the provider's answer
  position never predicts the answer and every retry moves every choice. After
  commitment, hold the lesson card for 45 ticks with every input intentionally
  inactive, then wait for A or Start. A survivable miss inserts a review copy
  after exactly three other questions and shifts every batch end at or past
  it: a copy past the current batch end joins the next batch, and one past the
  deck's end waits until a delivery reaches its due index and lands exactly
  there, so a fresh question always comes first after the Oracle wait. A new run drops those
  waiting copies and requeues their pending lessons instead. A miss that breaks
  the last ward schedules no in-run retry; it stays
  pending for a later run. Each commitment updates the
  lens mastery record and upserts the lesson journal entry. B arms a 90-tick
  in-scene confirmation; a second B inside the window leaves the run, and any
  other button or the timeout disarms it.
- **Instructional role:** Assesses one lens per question and turns every
  commitment into explained feedback. A miss names the misconception behind
  the chosen distractor before the correct reasoning, then schedules an
  informed retry. A correct same-launch retry is recorded as relearning; the
  spaced check in a later launch is the redemption that counts as evidence.
- **Feedback:** Replace the four choice frames with one lesson panel: a miss
  shows the committed pick in red with its rationale (the misconception) and
  then the answer in green with its rationale; a success shows the answer with
  its rationale. `-` and `+` markers repeat the verdict without relying on hue.
  The panel footer names the question's lens with its three mastery runes
  (lit, cracked, or unlit, as on the Codex), centers a miss's amber return
  note (`BACK IN N` intervening questions, `UP NEXT`, `LATER` while the copy
  waits for a delivery to reach its gap, or `NEXT RUN` when the ward broke) in
  the free span for the whole card, and shows `A:CONTINUE` only once input is
  live. When an answer wakes a lens rune, the newest lit footer rune blinks
  through the input hold (solid under reduced motion) and settles lit. A
  correct review attempt shows `REDEEMED` unless a lens rune or Insight
  crossing takes precedence.
  Use visibly spaced glyphs that begin beyond the plate's left divider,
  keeping every ornament outside glyph and inter-glyph cells, plus distinct
  cursor, commit, low-ward, and batch-complete cues. Replace generic health
  stars with three Oracle ward runes; compose batch progress (`TRIAL P/N`, or
  `RETRY P/N` in amber on a review copy), ward, flow multiplier, three Insight
  marks, and raw score across the header. While the lesson card shows, replace
  active controls with one banner in priority order: a woken lens rune
  (`ROLES RUNE I`, in the stage's rune color), an Insight crossing
  (`INSIGHT I RISES`), `REDEEMED`, flow (`FLOW X2`), or `CLEAR SIGHT`; a miss
  names the ward state instead. The word REVIEW on screen only ever refers to
  retries, and RUNE on a banner only ever names a lens.

### `continue-after-reward`

- **Decision:** Continue after reading the earned level and presentation tier.
- **Inputs:** A or Start.
- **Rules:** The scene graph owns the hold: A/Start continue only when the
  current transition would be accepted, so the manifest's `after_ticks` on the
  level-up routes (60 ticks here) sets both the non-interactive hold and when
  the continue prompt appears. After the hold, continue on the input edge; at
  180 ticks the scene continues on its own. Route to `quiz` when a valid
  question is ready and to `oracle` otherwise, requesting a missing batch while
  the reward holds. A held confirmation cannot answer the next question.
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
| `codex-frame` | UI | `codex` | Show lens mastery, pending reviews, and one lesson per page, plus empty-journal and missing-rationale states. | Mastery page sits inside the dormant archive frame; lesson pages overlay the trial chamber's panels while its portrait, brazier, and candles stay visible; worst-case copy is contained, disjoint, and readable against the brightest plate pixel under it. | `oracle-codex` implemented with native layout, exact rune-breakpoint, paging, and empty-state tests. |
| `character-frame` | UI/scene | `character-creation` | Stage the customizable hero inside the same world with centered name, path, and aura rows plus loading, retry, and ready states. | Heading and rows are pairwise disjoint; labels and actions center in measured usable interiors; the hero's visible-alpha feet meet the stage support line; status is truthful. | `oracle-atelier` implemented with native layout assertions. |
| `hero-set` | Sprite set | `character-creation`, `oracle`, `quiz`, `level-up`, `game-over` | Carry identity through the run with authored aura colorways plus idle, dodge, reward, and defeat variants. | Consistent silhouette across palettes/backgrounds; no procedural accessory or weapon overlays. | `oracle-hero` implemented with authored colorways, portrait, and defeat variants. |
| `oracle-sanctum` | Scene/UI | `oracle` | Present Datafall, loading, retry, ready, and B-back as one place: moving hero, authored drops, staged data-charge and corruption-containment instruments, and a tier crest. | Fits 240×160; sprites stay contained and differ by silhouette/value/hue; top status remains distinct; raw counts and themed three-rune meters remain disjoint from centered controls at two digits. | `oracle-sanctum` implemented with authored drop sprites, themed threshold runes, exact breakpoint tests, and three visual tiers. |
| `quiz-frame` | HUD/UI | `quiz` | Hold question, four choices, focus, ward health, flow multiplier, score, Insight Runes, and the lesson card: committed pick and answer with their rationales, `-`/`+` verdict markers, the lens with its mastery runes, and the continue prompt. | Honor text limits and plate-divider clearance; replace generic pips with three stateful Oracle runes; preserve raw score while exact flow/score thresholds change reward, fill, color, and concise response copy; worst-case rationales stay inside the lesson panel. | `oracle-trial` implemented with ornament-disjoint copy, shaped focus, themed instrumentation, staged scoring, lesson-card layout and contrast, and breakpoint tests. |
| `reward-frame` | Scene/UI | `level-up` | Telegraph the threshold, celebrate it once, and establish the new Oracle-bond baseline. | No rapid full-background flashing; tier and reward remain readable. | `oracle-ascension` implemented with tiered crest, tier-crossing `ASCENDS`/within-tier `DEEPENS` heading, first-try batch recap, and next-lens hold footer. |
| `result-frame` | Scene/UI | `game-over` | Resolve the run with hero state, final score, earned Insight Rune, level, completed crest, the run's learning ledger, and an obvious reset/replay path. | Defeat is clear without erasing earned progress; all accepted replay inputs are visible on every frame; worst-case ledger copy stays inside the panel, disjoint, and readable against the brightest plate pixel under it. | `oracle-aftermath` implemented with preserved tier, `INSIGHT` rank, learning ledger, steady prompt, and defeated hero. |
| `run-progression` | Presentation system | `oracle`, `quiz`, `level-up`, `game-over` | Own every tracked metric's direction, thresholds, crossing response, cap, reset, and carry-forward state. | At least two non-numeric channels change; exact breakpoints are tested; all run metrics reset deterministically. | Bond, ward, flow, Insight Rune, data-charge, and corruption-containment stages are implemented; Datafall and reward audio grow by tier. |

## Sound requirement ledger

Sound entries remain `art.kind = "audio"` production metadata: the manifest has
no sound schema or audio template field, so editing an entry changes no sound.
The engine owns the runtime that implements them. `src-tauri/src/audio.rs`
describes a constrained
four-voice chip (two duty-selectable pulses, a triangle-like wave, and noise).
Its director samples an observable-state snapshot every tick and diffs it
against the previous tick, so sound is a pure function of state transitions
exactly like rendering; gameplay code never calls audio. Every note carries its
absolute engine tick, voices are monophonic, and a zero-volume note is a cut, so
a scene's loop ends on the exact tick the scene exits. At most one cue starts
per tick, chosen by priority, so one input edge can never stack cues. Loops and
beds stay at or below chip volume 5 while every cue peaks above it. The shell's
`src/speaker.js` only schedules the engine's notes through WebAudio behind a
gesture-gated audio context and the device's four-detent volume wheel (V cycles
MUTE, LOW, MID, HIGH). Scenes the director does not recognize stay silent.
All compositions are original, in a D minor palette that resolves to D major at
the Oracle's crest.

| ID | Used by scenes | Player-facing purpose | Cues/loops and variants | Constraints and acceptance | Status |
|---|---|---|---|---|---|
| `opening-soundscape` | `copyright` and all five opening story scenes | Make the dormant-to-Oracle reveal audible and give the credits intentional restraint. | Archival tick, source pulse, archive answer, vault branches, convergence, Oracle cadence, clean tail. | Constrained chip-style palette; starts near silent; fullest arrangement only at the crescendo; reduced-audio variant. | Implemented: dry noise ticks on the four chronicle reveals; voices grow 1 → 2 → 3 → 3 → 4 and peak volume rises across the five beats; every beat tails before its scene exits; a skip cuts the fanfare and the title loop waits 30 ticks. The volume wheel's MUTE/LOW detents are the reduced-audio variant. |
| `ui-soundscape` | `title`, `quiz-menu`, `codex`, `character-creation` | Make focus, choice, cancel, customization, paging, and confirmation instantly legible. | Navigate, confirm, cancel, unavailable, page turn, trait variants, begin-run cadence. | One input edge produces at most one cue; no cue crosses scenes unintentionally. | Implemented: title and menu loops; name, path, and aura each answer on their own voice or duty; the Oracle motif is reserved for the bind cadence; the Codex has no loop under reading, pages turn on alternating pitches, opening it confirms and leaving it cancels; presses that change nothing, including A/Start in the Codex, get a soft unavailable cue; engine tests script every edge. |
| `oracle-soundscape` | `oracle` | Separate Datafall play from truthful loading state without overwhelming it. | Low ambience; data, bug, retry, ready, and B-back cues; three progression-tier variants. | Loops stop on quiz/menu transition; status remains readable when muted. | Implemented: the Datafall loop has two, three, and four voices at Initiate, Adept, and Oracle-bound; the 3/6/9 charge and 1/3/5 seal thresholds replace the plain data and bug cues; retry, ready, and leave derive from the truthful status line; B-back cuts the loop on its exit tick. |
| `quiz-soundscape` | `quiz` | Clarify cursor movement, answer commitment, result, danger, and batch completion. | Cursor, commit, correct, redeemed, lens-wake, wrong, low-ward, flow-stage, leave-warning, batch-complete cues. | Correct/wrong never rely on sound alone; prevent stacked result cues. | Implemented: every result opens with the same commit click, then plays exactly one variant (lens wake, a rising two-note D-major motif that climbs with the stage, over Insight rune awaken over redeemed over flow-stage over correct; ward break over low-ward over wrong); a heartbeat bed runs on the last ward; the first B of a leave plays a warning; the lesson card is silent and its ignored presses get no unavailable cue; continuing plays the question reveal; a new cue cancels the previous cue's remaining notes. |
| `progression-soundscape` | `level-up`, `game-over` | Make thresholds, earned tier, defeat, and reset feel conclusive. | Telegraph, reward cadence per tier, defeat fall, score hold, replay/reset cadence. | Reward arrangement grows by tier; new run audibly returns to Initiate. | Implemented: batch-complete telegraph, then a two/three/four-voice reward cadence that resolves inside the 60-tick hold; tier-specific continuation; defeat fall and a quiet score-hold chord; the replay cadence always uses the thin Initiate timbre. |

## Whole-game polish matrix

| Scene | Static | Motion | Sound | Mechanical closure | Felt progression | Evidence/status |
|---|---|---|---|---|---|---|
| `copyright` | Dim archive frame with hierarchy equal to gameplay; no glow. | Credits reveal in fixed readable steps, then clear cleanly. | Intentional near-silence with one dry archival tick per reveal. | A/Start after minimum dwell or timed exit; missing provenance has explicit copy. | Establishes the dormant baseline and repository identity. | Visual template implemented and native-frame tested; audio implemented/tested. |
| `opening-fanfare` | Lone code-seer, dormant cathedral depth, and one cyan ember establish a clear focal hierarchy. | A 1.6-second held establishing shot gives the story and skip gate time to read. | One source pulse and its echo on a single voice. | Elapsed enters `archive-answer`; A/Start becomes visible at 1.5 seconds and skips to title. | Establishes the darkest authored opening baseline. | Dedicated native plate and executable transitions implemented/tested; audio implemented/tested. |
| `archive-answer` | Same code-seer and cathedral, with lit floor paths and waking monolith silhouettes. | The composition itself advances causality; staged cut avoids a full-frame flash. | The altar answers the pulse a fifth above on a second voice. | Elapsed enters `memory-vault`; A/Start skips to title. | Cyan expands into a second visual channel: environment response. | Dedicated native plate and executable transitions implemented/tested; audio implemented/tested. |
| `memory-vault` | Foreground doors, bridge, and deep commit constellation create a new three-plane composition. | Staged cut moves the player deeper into the archive. | Two echoed commit-branch arpeggios over a vault bass. | Elapsed enters `convergence`; A/Start skips to title. | Space, node density, and narrative scale visibly expand. | Dedicated native plate and executable transitions implemented/tested; audio implemented/tested. |
| `convergence` | Circular chamber, same code-seer, opposing cyan/gold streams, and incomplete dark eye. | Staged cut turns propagation into convergence without spending maximum white. | A descending and a rising pulse meet in unison over the bass. | Elapsed enters `oracle-awakening`; A/Start skips to title. | Gold joins cyan and the goal is visibly telegraphed. | Dedicated native plate and executable transitions implemented/tested; audio implemented/tested. |
| `oracle-awakening` | Existing high-detail awakening composition becomes the earned final image. | Center luminance rises during the final hold, then clears cleanly to title. | The full four-voice Oracle cadence, tailing before the title. | Elapsed or A/Start lands on a fresh title frame. | Reaches the opening's brightest/densest state after four distinct scenes. | Existing plate retained; five-frame distinctness and luminance arc implemented/tested; audio implemented/tested. |
| `title` | Restrained Oracle motif and legible title at native scale. | One controlled eye/prompt pulse never competes with title. | A restrained title loop after a clean breath; one confirm cue. | A/Start continues; unavailable cartridge state remains honest. | Returns to an Initiate baseline while preserving the Oracle promise. | Visual template implemented/tested; audio implemented/tested. |
| `quiz-menu` | Rune frame and shaped focus for both choices. | Focus moves on input edge. | Menu bed plus navigate, confirm, cancel, and unavailable cues. | Begin and back are explicit; held input cannot double-confirm. | New run previews the Initiate palette and reset. | Visual template implemented/tested; audio implemented/tested. |
| `codex` | Archive frame holds five lens rows and totals; lesson pages hold counter, lens runes, question, answer, and rationale in bounded panels; a sealed pending page holds the reveal prompt, the pick, and a three-row misconception in the same panels. | Page turns are instant input-edge cuts; there is no idle animation to compete with reading. | Entry confirm, alternating page-turn cues, a soft reveal cue when A unseals a pending answer, unavailable cue for A/Start elsewhere, and back cancel; no ambience competes with reading. | B returns to the menu; A/Start reveal a sealed pending answer and are otherwise inactive; paging wraps and reseals; the empty journal has explicit guidance. | Runes wake at 1/3/5 evidence per lens, crack while an accuracy or pending-review gate holds them back, and pending reviews clear as misses are answered correctly; a correct answer after a reveal is relearning, not evidence. | Visual template, legacy renderer, and breakpoint tests implemented; page and back cues implemented. |
| `character-creation` | Authored hero, pairwise-disjoint identity rows, a grounded stage placement, centered action copy, and Oracle status form one staged composition. | Aura changes the authored colorway; identity rows react immediately; begin has one clean handoff. | Per-row trait timbres and the Oracle-motif begin cadence. | Every row stays centered and contained; B returns; loading/retry/ready states are truthful. | Establishes identity that remains visible across the run. | Native interior, support-line, and sibling-bound assertions implemented; audio implemented/tested. |
| `oracle` | Sanctum, playfield, status, the Oracle line (a failure reason with its retry countdown, or one recalled lesson), raw counts, themed charge/containment runes, controls, and tier crest remain distinct. | Exact 3/6/9 gains light charge runes; 1/3/5 hits break containment runes; a newly recalled lesson types in at two characters per tick and appears whole under reduced motion; Datafall and status retain owned exits. | Tier-grown Datafall ambience; data, bug, rune, seal, retry, ready, and leave cues. | Questions-ready enters quiz; B abandons safely; all other controls are intentionally inactive. | Clean play preserves seals while collection fills runes; bond visuals retain all three tiers. | Native meter layout and exact first-breakpoint frame changes implemented/tested; Oracle line containment and reveal tested (`the_oracle_line_stays_contained_disjoint_and_readable`, `recalled_lessons_are_written_in_but_stay_static_under_reduced_motion`); audio implemented/tested. |
| `quiz` | Batch progress (`TRIAL P/N` or amber `RETRY P/N`), question, choices, hero token, ward runes, flow multiplier, Insight Rune meter, raw score, and tier frame remain readable; answer copy clears every ornament; the lesson card holds worst-case rationales, `-`/`+` verdict markers, and the lens-rune footer inside one panel. | Cursor, commit, 45-tick lesson-card hold, ward loss, x2/x3 flow, 300/900/1800 rune crossings, and batch threshold have causal timing. | Cursor, one commit-and-result cue (including redeemed and lens wake), leave warning, last-ward heartbeat, question reveal; the lesson card itself is silent. | The lesson card shows the misconception and answer rationales under the lens-rune/Insight/`REDEEMED`/flow/ward banner, with a miss's return note on the footer, then waits for A/Start; B leaves only on a second press within 90 ticks; every automatic outcome routes visibly. | Score reward changes mechanically at streak thresholds, lens runes wake on the card footer, and earned runes persist into results. | HUD siblings, ward states, exact scoring breakpoints, choice/plate bounds, and lesson-card containment and contrast implemented/tested; audio implemented/tested. |
| `level-up` | Hero and newly expanded crest dominate; level and first-try recap text support rather than carry reward, and the hold footer names the next lenses. | Crest growth → hero rise → one-second hold → continue; stable background avoids flashing. | Batch telegraph, tier-grown reward cadence, tier continuation cue. | A/Start wait for the manifest's 60-tick gate, then route to a ready quiz or the Oracle wait; the scene continues on its own at 180 ticks. | Explicit threshold celebration establishes the new visual baseline. | Visual template/progression implemented/tested; audio implemented/tested. |
| `game-over` | Defeated hero, final score, Insight Rune rank, level, earned crest, and the run's learning ledger share one conclusive frame. | Energy recedes without erasing earned bond or score rank; replay resets both on the next run. | Defeat fall, score-hold chord, Initiate replay cadence. | The steadily lit A/B/Start prompt returns to menu; the next new run resets all run state, including the ledger. | Shows the exact bond and Insight Rune stages reached before reset. | Visual template/progression and score-rune result implemented/tested; audio implemented/tested. |

Reduced motion applies to every row above. The shell forwards the system's
`prefers-reduced-motion` preference, and the engine then freezes its decorative
motion clock: blinking prompts stay lit, heroes stop bobbing, starfields stop
scrolling, and settling motions show their end state. Scene timing, timing
gates, input, sound, and data are unchanged, so each row's information and
duration survive the reduced variant. There is no in-game toggle.

The screen transcript also applies to every row. The engine
(`src-tauri/src/engine/transcript.rs`) derives plain-language sentences from
the same state the renderers use, and the shell places them in a visually
hidden, polite live region through the `engine_transcript` command. The
transcript is republished only when its words change, so decorative motion is
never announced: a new screen is read whole, and a change within a screen,
such as focus moving to another choice, is read on its own. A new scene or
handler needs its own transcript sentences, which is engine work.

## Runtime traceability

| Element | Status | Evidence or required work |
|---|---|---|
| Manifest title and `quiz`/`quest` type | Implemented | Parsed at cartridge load and used by the engine. |
| Scene graph | Configured/executable | Schema-v2 handlers and semantic transitions are validated, compiled, and executed by the engine. |
| Mechanic and presentation graph | Mixed | Mechanics and `kind = "audio"` art entries remain validated metadata: the engine derives sound from state per trusted handler, and no manifest field selects or changes it. Typed visual templates are parsed, validated, and executed by scene renderers. |
| First question request at cartridge acceptance | Implemented | Empty quiz cartridges call the question effect immediately when inserted. |
| Question generation | Implemented | Each request carries the anonymized project brief (`repo_context.rs`: README, design notes, manifest summary, and up to 30 `COMPONENT n` skeletons with every path withheld) and the learner state (weakest lens, up to 24 earlier stems) to the selected CLI on stdin under a 120-second process-tree deadline. Payload v2 requires a known lens and a fitting, location-free rationale for every choice; when a delivery falls short, a question whose only failures are mechanical (question, choice, or rationale length, a missing rationale, an unknown lens, or non-ASCII text) gets one repair call through the same CLI, which uses what is left of the same deadline and is skipped when less than 10 seconds remain, so one request costs at most two CLI calls. A repair counts only if it keeps its answer index and passes the full policy. Trivia, malformed, and unrepaired questions are dropped individually, and a short delivery tops the open batch up to a full six. |
| Level focus and transfer prompts | Implemented in the prompt | `Concept::focus_for_level` asks for at least four of six questions on the level's focus lenses, and level 4+ asks for at least two PREDICT questions. Acceptance checks that each question names a known lens; it does not enforce the focus share or the `PREDICT:` prefix. |
| Repository authors, timeline, and explicit copyright extraction | Implemented | Cartridge preparation reads sanitized git shortlog/history data and scans bounded LICENSE/COPYRIGHT/NOTICE files. Commit authors are never treated as legal owners. |
| Copyright and five-scene opening story | Implemented with asset-backed templates | Trusted Bevy handlers render the chronicle, source ember, archive answer, memory vault, convergence, and Oracle awakening before `Title`; manifest timing gates control per-scene auto-advance and direct skip while fanfare/title frames remain separate. |
| Title, menu, hero creation, Oracle, quiz, level-up, and game-over screens | Implemented | Trusted handlers own input and rendering while the manifest routes their semantic events. |
| First request, prefetch, invalid-batch retry, and Oracle hold | Implemented | Engine question effects, pending batches, and retry timer. |
| Oracle Datafall with safe recovery | Implemented | Held movement, authored drops, automatic counters, 3/6/9 charge runes, 1/3/5 breakable containment runes, split HUD, and B-back close the indefinite wait. |
| Themed run instrumentation and score thresholds | Implemented | Oracle ward glyphs replace stars; x1/x2/x3 flow changes score awards; 300/900/1800 Insight Runes change HUD and review feedback; exact breakpoints and native sibling bounds are tested. |
| Safe Oracle-to-quiz input boundary | Implemented | A/Start are ignored in Oracle; B exits to the menu; held D-pad controls have no answer action after the automatic transition. |
| Quiz result and reward input boundaries | Implemented | The 45-tick lesson hold replaces active controls and then waits for A/Start; B leaves an active question only through a 90-tick confirmation. Level-up continuation asks the scene graph (`can_signal`), so the manifest's `after_ticks` sets both the input hold and the continue prompt (60 ticks here); the scene continues on its own at 180 ticks. |
| Lesson card, shuffled choices, and spaced retry | Implemented | Committed answers show the misconception and answer rationales in a composed lesson panel; display order follows `learning::presentation_order`; a survivable miss inserts a review copy after exactly `RETRY_GAP` (3) other questions, carrying into the next batch when needed; mastery and the lesson journal update on every commitment. Native layout, contrast, and flow tests cover worst-case copy (`lesson_cards_render_the_misconception_and_the_answer`, `a_missed_concept_is_journaled_retried_redeemed_and_reread_in_the_codex`). |
| Learner persistence | Implemented | Each commitment updates `quiz.progress` in one atomic save update: a first-try or spaced success moves the question to the retired list, a same-launch retry success to the relearned list, and a miss to the missed list (the latest attempt decides), and lens mastery records first-try, redeemed, relearned, and missed counts plus the newest eight graded outcomes. Cartridge load drops retired questions, queues missed and relearned ones as spaced reviews, journals both, and plays lower-level batches first. Serialized updates preserve the independent AI-batch namespace; answered lists from earlier builds load as retired, and the legacy Claude batch key is still read. |
| Truthful multi-state Oracle presentation | Implemented with asset-backed templates | Loading, retry, and ready copy derives from actual engine state; B provides recovery from a permanently unavailable generator. The Oracle line names each failure's category, such as `AI DISABLED`, `TIMED OUT`, or `REJECTED BATCH`, with a retry countdown, and otherwise recalls journal lessons (`failure_reasons_are_sanitized_to_one_short_device_line`, `the_oracle_line_stays_contained_disjoint_and_readable`). |
| Concise answer review and reduced motion | Implemented | Green/red lesson copy with `-`/`+` markers, shaped focus, stable level-up, and staged opening motion are implemented. The shell forwards the system's `prefers-reduced-motion` preference (`engine_set_reduced_motion`), and the engine freezes decorative motion while scene timing, input, and data stay identical (`reduced_motion_freezes_decorative_motion_but_keeps_scene_timing`). There is no in-game toggle. The engine's screen transcript (`engine/transcript.rs`, published through `engine_transcript` into a polite live region) reads every screen to screen readers, whole on entry and then only its changes (`a_new_screen_is_read_whole_and_a_focus_move_reads_only_the_new_focus`, `every_screen_has_a_transcript_that_does_not_panic`). |
| Visual templates selected from manifest | Implemented | Twelve typed built-in templates are selected by `art[].template`; `oracle-awakening` selects five art-ID-addressed opening plates, other Oracle templates composite their native illustrated plates and live state, unknown names fail validation, and untemplated cartridges keep their legacy renderers. |
| Oracle Codex lesson journal | Implemented | The `codex` handler and `open-codex` menu signal route the menu to a mastery-and-lesson journal whose pending pages are sealed self-tests; menus without the route keep `RETURN TO TITLE`. |
| Whole-game sound design and playback | Implemented | The engine's audio director derives every cue and scene loop from per-tick state snapshots and publishes tick-stamped chip notes through the bounded `engine_audio` queue; the shell speaker plays them behind a gesture-gated context and the persisted volume wheel. Scenes still select audio by trusted handler, not by manifest template. |
| Felt presentation progression | Implemented | Initiate, Adept, and Oracle-bound change palette, circuit density, crest geometry, reward/result presentation, and the voice count of the Datafall and reward arrangements; native-frame and audio tests verify non-numeric final-tier channels. |

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
5. **Completed — Feedback/accessibility pass:** Correctness labels and
   markers, shaped focus, staged motion, native-scale assertions, reduced
   motion that follows the system preference, and a screen transcript for
   screen readers are implemented.
6. **Completed — Continuity pass:** The waiting Oracle's second header line
   uses state the engine already owns: while a failed request waits out its
   retry delay it names the failure with a `RETRY IN NS` countdown, and
   otherwise it recalls journal lessons every 240 ticks, outstanding misses
   first (`REVIEW`), then cleared lessons (`RECALL <LENS>`). Reduced motion
   shows each lesson whole instead of typing it in. Batch progress stays on
   the quiz header (`TRIAL P/N`).
7. **Completed — Whole-game presentation pass:** Twelve typed built-in visual
   templates cover all fourteen reachable scenes, beginning with the dormant
   `copyright-card` and culminating in the fifth opening beat's Oracle
   crescendo.
8. **Completed — Sound runtime pass:** An engine-owned chip director implements
   every entry in the sound ledger with scene-owned loop exits, one cue per
   input edge, and tier-grown arrangements; manifest-selected audio templates
   remain future work.
9. **Completed — Felt-progression pass:** Tiered visuals carry the crest,
   palette, circuit density, and hero identity across Oracle, quiz, reward, and
   results, and the Datafall and reward arrangements gain a voice per tier.
10. **Completed — Executable scene graph:** Add schema v2 handlers, semantic
   transitions, timing gates, reachability validation, built-in quiz/quest
   templates, and schema-v1 compatibility.
11. **Completed — Answered-question continuity:** Record committed answers in
    the cartridge save, retire correct answers for later runs and launches,
    keep misses queued for review until redeemed, and serialize namespace
    updates so background batch writes cannot erase progress.
12. **Completed — Instrument and threshold pass:** Replace generic health pips
    with Oracle ward runes; attach exact stages to flow, score, Datafall charge,
    corruption containment, batches, and bond; verify native layout and exact
    breakpoint transitions.
13. **Completed — Learning-model pass:** Tag questions with concept lenses and
    per-choice rationales (payload v2), replace the answer review with the
    lesson card, shuffle choices per question and attempt, schedule spaced
    retries inside the batch, record lens mastery and the lesson journal, add
    the Oracle Codex, deepen lens focus by level with PREDICT transfer at level
    4+, and adapt each request to the weakest lens and earlier stems.

## Open decisions

- Beyond learner progress (retired and missed questions, lens mastery), should
  score, wards, hero identity, or presentation tier persist per cartridge
  across launches?
- Rune III now requires no pending review and 80% recent accuracy, so three
  lit runes already mean mastered. Should a lens also need transfer evidence
  (a correct PREDICT answer) before its third rune lights?
- Should acceptance enforce the level's focus-lens share and the PREDICT count,
  rejecting or topping up a batch that misses them, or keep both as prompt
  guidance?

Resolved: every choice carries a short rationale shown on the lesson card, and
B from an active quiz uses an in-scene 90-tick confirmation before leaving.
