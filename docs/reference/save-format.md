# Cartridge save format

This is the reference for the save file CODE QUEST ADVANCE keeps for each
cartridge (a git repository loaded as a game). It describes the format as
implemented in `src-tauri/src/save.rs`, `questions.rs`, `learning.rs` and
`lib.rs` on branch `claude/codequest-opus-improvements-0069d2`. Where a
behavior follows from the code but no test covers it, the text says so.

## 1. Location and naming

A save sits next to the cartridge directory, like an emulator save. It is not
inside the directory. `save::path_for` appends `.sav` to the directory's own
name:

| Cartridge directory | Save file              |
|---------------------|------------------------|
| `/games/demo`       | `/games/demo.sav`      |
| `/games/demo.v2`    | `/games/demo.v2.sav`   |

- The extension is appended, not substituted, so a dotted name keeps its dots
  (test `save_path_appends_to_a_dotted_cartridge_name`).
- `build_cartridge` canonicalizes the path first, so the save belongs to the
  resolved directory, not to a symlink or relative spelling of it.
- Loading a cartridge always opens or creates its save
  (`SaveFile::open_or_create`). A fresh save is
  `{"schema_version": 1, "data": {}}` (test
  `cartridge_load_creates_an_emulator_style_save_file`).
- The save never enters the repository. It holds no repository content, only
  generated questions and learner progress.

## 2. The envelope

```json
{
  "schema_version": 1,
  "data": { "<namespace>.<name>": <any JSON value>, ... }
}
```

| Field            | Type            | Rules |
|------------------|-----------------|-------|
| `schema_version` | integer (`u32`) | Required. This build reads and writes only `1` (`SCHEMA_VERSION`). |
| `data`           | object          | Optional on read (defaults to `{}`). Maps namespaced keys to arbitrary JSON values. |

- `data` is a `BTreeMap`, so keys are written in sorted (byte) order.
- Files are written as pretty-printed JSON (2-space indent) plus a trailing
  newline.
- Each subsystem owns its keys, and no key is ever interpreted by another
  subsystem. Unknown keys, for example `quest.progress` or keys written by a
  newer build, are carried through every update unchanged (tests
  `namespaced_data_survives_updates_from_other_game_systems`,
  `generated_ai_batch_is_saved_without_replacing_other_game_data`).
- Keys can be any string. The property tests use `""`, keys with spaces,
  Cyrillic and emoji keys.

## 3. Reading, and refusal of damaged saves

`read_document` classifies the file bytes:

| File content                                        | Result |
|-----------------------------------------------------|--------|
| Missing                                             | Created empty (`create_new`, pretty JSON, fsync) while holding the save lock. |
| Empty, or ASCII whitespace only                     | Treated as a fresh save (a create that never got its first write). The file is not rewritten until an update changes a value (test `an_empty_save_file_starts_a_fresh_save`). |
| Valid envelope, `schema_version == 1`               | Loaded. |
| Valid envelope, any other `schema_version`          | Refused: `UNSUPPORTED CARTRIDGE SAVE`. |
| Anything else (truncated JSON, a JSON array, missing `schema_version`) | Refused: `CARTRIDGE SAVE IS CORRUPT`. |

A refused save is never replaced or "repaired". Every writer starts from the
document it just read, so treating a damaged file as empty would let the next
update overwrite the whole save with one key. A corrupt or unsupported save
therefore makes the cartridge load fail with that message, and the file stays
byte-for-byte as it was (tests
`a_save_that_does_not_parse_is_refused_and_left_untouched`, and
`lib.rs::a_corrupt_save_stops_the_load_without_being_rewritten`).

Error strings the save layer can return:

| Message | Cause |
|---------|-------|
| `COULD NOT READ CARTRIDGE SAVE` | I/O error reading or stat-ing the file |
| `COULD NOT CREATE CARTRIDGE SAVE` | The first-time create failed |
| `CARTRIDGE SAVE IS CORRUPT` | The file is not a save document |
| `UNSUPPORTED CARTRIDGE SAVE` | `schema_version` is not `1` |
| `CARTRIDGE SAVE DATA IS UNREADABLE` | An update's key holds a value that does not deserialize as the expected type |
| `COULD NOT SERIALIZE CARTRIDGE SAVE` | Serializing a value failed |
| `COULD NOT WRITE CARTRIDGE SAVE` | The temp file write, fsync or rename failed |

## 4. Writing: atomic replacement, serialized read-modify-write

All writes go through `save::update(cartridge, key, change)`:

1. Take the process-wide save lock (`save_write_lock`, a static `Mutex`). A
   poisoned lock is recovered, because it guards no data.
2. Re-read the document from disk (or create it). Writers never write back an
   old in-memory snapshot.
3. Deserialize `data[key]` as the caller's type `T`. A missing key starts
   from `T::default()`. A present value that does not deserialize as `T`
   aborts with `CARTRIDGE SAVE DATA IS UNREADABLE`, and the value is left
   alone rather than reset (test `updates_refuse_to_replace_a_value_they_cannot_read`).
4. Apply `change`, then re-serialize the value.
5. If the value did not change, do not write anything (test
   `unchanged_updates_do_not_rewrite_the_save`). Otherwise replace
   `data[key]` and persist.

Persisting is an atomic replace. The whole document is written to a
`tempfile::NamedTempFile` in the save's own directory, fsynced, then renamed
over the save. A crash leaves either the old file or the new one, never a
partial one.

Consequences:

- Concurrent updates to the same key or to different keys never lose each
  other. Tests: `concurrent_updates_to_shared_and_separate_keys_never_lose_each_other`
  (6 threads x 8 writes), `concurrent_batches_and_answers_all_reach_the_save`,
  and the randomized `save_updates_round_trip_under_random_key_and_value_sequences`.
- A stale `SaveFile` snapshot cannot overwrite a newer update to another key
  (test `a_stale_snapshot_cannot_overwrite_an_update_to_another_key`).
- Progress is written off the frame loop. `answer_recorder_with` feeds one
  `cqa-save-writer` thread through a channel, which applies each
  `learning::ProgressEvent` (a committed answer or a Codex reveal) in the
  order it happened. The recorder ignores write errors (`let _ =`), so an
  event that cannot be saved is dropped silently.
- Limitations (derived from the code, not tested): the lock only serializes
  writers inside one process, so two running app instances could still race
  on one save. The containing directory is not fsynced after the rename.

`SaveFile` is the read side. It holds a snapshot taken at open, and `get::<T>`
returns `None` when a key is missing or does not deserialize as `T`. Callers
use their defaults in that case. `SaveFile::set` exists only in tests, to stage
saves written by other builds.

## 5. Keys used by this build

| Key | Owner | Written by this build? | Type |
|-----|-------|------------------------|------|
| `ai.question_batches` | questions | Yes, append-only | array of batch objects |
| `claude.question_batches` | questions (legacy) | No, read only | array of batch objects |
| `quiz.progress` | questions / learning | Yes | progress object |

The lesson journal is rebuilt at load from the batches and `quiz.progress`
(see 5.4). Only one Codex action is stored: revealing a pending lesson's
answer (`peeked_questions` in 5.2). Merely opening a page records nothing.
`quest.progress` shows up only in tests, as an example of a foreign namespace
that updates must preserve.

### 5.1 `ai.question_batches` and legacy `claude.question_batches`

Each element is one generated batch:

```json
{ "level": 1, "questions": [ <question>, ... ] }
```

| Field | Type | Rules |
|-------|------|-------|
| `level` | `u32` | The Oracle bond level the batch was generated for. On load, `0` drops the batch. |
| `questions` | array | Defaults to `[]` on load. On load, a batch with no playable questions is dropped. |

Writing (`persist_ai_question_batch`):

- New batches go only to `ai.question_batches`, the provider-neutral key.
  `claude.question_batches` comes from earlier Claude-only builds and is never
  written again.
- A batch must have `level > 0`, at least one question, and every question
  must pass the payload v2 generation policy (see 6.2). Anything else is
  rejected with `INVALID AI QUESTION BATCH`.
- The stored array is updated as raw `serde_json::Value`, and the new batch
  is appended. Entries this build cannot parse, such as a future
  `{"level":9,"questions":[{"format":3}]}`, are kept as they are (test
  `saving_a_batch_keeps_entries_this_build_cannot_read`).

Loading (`saved_question_batches`):

- Batches from `claude.question_batches` come first, then
  `ai.question_batches`. That puts them oldest first, since the legacy key
  predates the new one (test
  `cartridge_reload_combines_legacy_and_current_ai_batches_oldest_first`).
- If a key is not an array, it counts as empty.
- Parsing is per question. A question that does not deserialize, or fails
  the saved-question policy (6.3), is skipped, and the other questions in its
  batch stay.
- The play queue is sorted by `level` (stable within a level), so lower-level
  batches play first. The lesson journal keeps generation order. A question
  that appears in more than one batch (same identity, see 7) is queued and
  journaled only once.

### 5.2 `quiz.progress`

Learner progress for the cartridge (`SavedQuizProgress`). Every field has a
default, so any subset loads.

| Field | Type | Default | Serialized when | Meaning |
|-------|------|---------|-----------------|---------|
| `answered_questions` | array of strings | `[]` | always | Retired questions. They were answered correctly on a first try or on a spaced (later-launch) check. Saves from earlier builds, which retired every committed answer, are read the same way. Retired questions are never asked again. |
| `missed_questions` | array of strings | `[]` | always | Questions whose latest attempt was wrong. They stay playable and come back as spaced reviews in later launches. |
| `relearned_questions` | array of strings | `[]` | always | Missed questions answered correctly on a retry in the same launch. The lesson counts as learned, but the question comes back once in a later launch, and only that spaced check retires it. |
| `mastery` | object: lens → `LensRecord` | `{}` | always | Per-lens evidence (5.3). |
| `question_attempts` | object: identity → `u32` | `{}` | only when non-empty | Committed attempts for each missed or relearned question, keyed by normalized identity. Choice rotation continues from this count across launches. |
| `missed_picks` | object: identity → `usize` | `{}` | only when non-empty | The source choice index of each question's latest wrong pick, so the Codex self-test can show the misconception after a reload. A later correct answer keeps the entry. On load, an index that is out of range or names the answer is ignored. |
| `peeked_questions` | array of strings | `[]` | only when non-empty | Missed questions whose answer the player revealed in the Codex before answering them again. A reveal is recorded only while the question is still missed, once per identity; the next attempt at the question removes it. |

The three lists store the question text as it was first committed. They are
compared by identity (7), never by raw text. `SavedQuizProgress::record` keeps
the lists disjoint. The latest attempt moves the question into exactly one
list:

| Answer | Review kind | Moves to | `question_attempts` |
|--------|-------------|----------|---------------------|
| correct | `Fresh` (first attempt), no peek | `answered_questions` | entry removed |
| correct | `Spaced` (miss from an earlier launch), no peek | `answered_questions` | entry removed |
| correct | `InSession` (retry of a miss from this launch) | `relearned_questions` | incremented |
| correct | any, after a Codex peek (`AnswerEvidence::peeked`) | `relearned_questions` | incremented |
| wrong | any | `missed_questions` | incremented |

A wrong answer with a recorded pick also sets `missed_picks[identity]`, and
every attempt removes the identity from `peeked_questions`. The app persists
a `learning::ProgressEvent` through `persist_progress`: `Answered` applies
`record`, and `Peeked` applies `record_peek`.

On increment, the count becomes `max(count, 1 if review else 0) + 1`. A review
therefore always shows at attempt 1 or later, even when an earlier build left
no count. Evidence with an empty identity is ignored. Each committed answer is
a single `save::update` of the whole `quiz.progress` value, so the lists and
the mastery can never disagree.

"Retired" means `answered − missed − relearned`, compared by identity. This
covers hand-edited or legacy saves in which the lists overlap.

### 5.3 `mastery` and `LensRecord`

`mastery` is a `BTreeMap<Concept, LensRecord>`. Keys are the kebab-case
lens names, in enum order: `purpose`, `responsibility`, `interaction`,
`invariant`, `tradeoff`.

| Field | Type | Default | Meaning |
|-------|------|---------|---------|
| `first_try` | `u32` | 0 | Correct answers on a first attempt. |
| `redeemed` | `u32` | 0 | Correct answers, in a later launch, to a question missed before. |
| `missed` | `u32` | 0 | Wrong answers, counting every attempt. |
| `relearned` | `u32` | 0 | Correct same-launch retries, and correct answers given after a Codex peek. These are relearning, not evidence. |
| `recent` | `u8` | 0 | Shift register of the newest graded outcomes. Bit 0 is the newest, and a set bit means correct. Relearning is not graded. |
| `recent_len` | `u8` | 0 | How many bits of `recent` are valid (0 to 8, `RECENT_CAPACITY`). |

Evidence is `first_try + redeemed`, and runes light at 1, 3 and 5. Rune II
needs at least 60% correct over the newest 5 graded outcomes. Rune III needs
at least 80% and no open miss on the lens. A record with `recent_len == 0`,
such as one written by an earlier build, passes both accuracy gates, and only
the open-miss gate applies to it (test
`legacy_lens_records_load_and_keep_their_runes`). Answers to questions
without a known lens do not update `mastery`.

### 5.4 Derived at load, never stored

`load_cartridge_questions` reads the save once and builds these from the
batches and `quiz.progress`:

- the play queue, with each question marked `Fresh` or `Spaced`;
- the batch boundaries and levels;
- the lesson journal, with one entry for each missed, relearned or retired
  question, flagged `outstanding` while it is still missed, carrying the saved
  pick's text and misconception (`missed_picks`) and flagged `peeked` while
  it is outstanding and listed in `peeked_questions`;
- the mastery;
- the attempt counts.

None of these are written back.

## 6. Question payload

### 6.1 Schema (payload v2)

```json
{
  "q": "WHAT SHOULD OWN GAMEPLAY STATE?",
  "concept": "responsibility",
  "choices": [
    { "text": "THE GAME ENGINE",  "why": "THIS NAMES THE MODEL THE DESIGN DEPENDS ON." },
    { "text": "THE DEVICE SHELL", "why": "..." },
    { "text": "THE STYLES",       "why": "..." },
    { "text": "THE VIEW",         "why": "..." }
  ],
  "answer": 0
}
```

| Field | Type | Rules |
|-------|------|-------|
| `q` | string | Required. Question stem. |
| `concept` | string, optional | The lens. Omitted from the output when absent (legacy). New saves store the canonical lens name. |
| `choices` | array | Each element is either a plain string (legacy) or `{ "text": string, "why": string }`. `why` defaults to `""` when missing. `QChoice` is `#[serde(untagged)]`: a string becomes `Plain` and an object becomes `Explained`. |
| `answer` | integer | Index of the correct choice. |

For the correct choice, `why` says why it holds. For a distractor, it names
the misconception the choice represents. Unknown fields in a question object
are ignored when it is parsed. They stay in the file because batches are kept
as raw JSON (5.1).

### 6.2 What this build writes

Questions are normalized when a provider response is parsed, before they
reach the save (`normalized_question`). `persist_ai_question_batch` itself
does not normalize; it only validates.

- `q`, every choice `text` and every `why` have typographic quotes, dashes and
  ellipses folded to ASCII, whitespace collapsed, and letters uppercased.
- `concept` becomes the canonical lens name when `Concept::parse` recognizes
  it. The parser accepts case variants, plurals and synonyms, so
  `Responsibilities` becomes `responsibility` and `Flows` becomes
  `interaction`.

A question is saved only if it passes `generated_question_is_acceptable`:

- `q` wraps into at most 4 lines of 31 columns;
- there are exactly 4 choices, all distinct and non-empty, each at most 31
  characters;
- `answer` is less than 4;
- no location or state-in-time trivia;
- a known lens;
- every choice is `{text, why}` with a `why` that fits 3 lines of 34 columns
  (70 characters always fit) and names no location;
- every string is printable ASCII.

### 6.3 What this build accepts when reading (legacy compatibility)

`question_is_acceptable` applies the same layout and trivia rules as 6.2. On
top of those, a question must be one of these two kinds:

- **Legacy:** every choice is a plain string, and `concept` is absent or a
  known lens. It plays without rationales. Its lesson has an empty rationale,
  and it records no mastery when it has no lens. It round-trips unchanged
  (test `legacy_saved_questions_still_deserialize_and_play_without_rationales`).
- **Complete v2:** a known lens, and every choice explained with a fitting,
  non-empty `why`.

A question that mixes plain and explained choices, has an explained choice
with an empty `why`, or names an unknown lens is skipped on load. It stays in
the file.

## 7. Question identity

```rust
question.split_whitespace().collect::<Vec<_>>().join(" ").to_ascii_uppercase()
```

Identity collapses runs of whitespace, trims both ends, and uppercases ASCII.
Non-ASCII characters pass through unchanged. It is the key for:

- membership in `answered_questions`, `missed_questions`,
  `relearned_questions` and `peeked_questions`;
- the keys of `question_attempts` and `missed_picks`;
- de-duplication across batches, in the prompt's avoided-stems list and in
  repair merges;
- the seed of the stable choice order (`presentation_order`).

For example, `"  what owns gameplay state?  "` and `"WHAT OWNS GAMEPLAY STATE?"`
are the same question. Recording both leaves one entry, the first spelling
(test `answered_question_progress_is_namespaced_and_idempotent`). An empty
identity is never recorded.

## 8. Compatibility rules

**Envelope**

- A missing `data` field defaults to `{}`.
- A `schema_version` other than `1` is refused outright, with no read and no
  write. A build that knows only v1 will not load a save written with a future
  `schema_version`. It fails the cartridge load with `UNSUPPORTED CARTRIDGE SAVE`
  and leaves the file untouched. Bumping `schema_version` is therefore the
  only way to make a breaking change safely.
- Extra top-level envelope fields besides `schema_version` and `data` are
  ignored on read. From the code: they are dropped the next time the file is
  rewritten, because `SaveDocument` does not preserve them.

**Keys in `data`**

- Unknown keys are always preserved.

**`ai.question_batches` / `claude.question_batches`**

- Batch entries and questions this build cannot read are kept in the file
  and skipped when it loads.
- A question valid under an older, looser policy may be skipped by a newer
  build. It is not deleted.

**`quiz.progress`**

- Every field defaults, so progress from any earlier build loads. This covers
  saves with only `answered_questions`, saves without the relearning fields
  or the Codex self-test fields (`missed_picks`, `peeked_questions`), and
  `LensRecord`s without `relearned`, `recent` or `recent_len` (tests
  `progress_from_earlier_builds_loads_as_retired_questions`,
  `legacy_progress_without_relearning_fields_loads`,
  `legacy_progress_without_picks_loads_and_ignores_bad_picks`). A miss saved
  before picks were recorded has no `missed_picks` entry, so its sealed Codex
  page asks the player to think first instead of showing a pick.
- Unknown fields inside `quiz.progress` or a `LensRecord` are ignored on
  read. From the code: they are dropped the next time an answer is recorded,
  because the update deserializes into `SavedQuizProgress` and serializes it
  back. A newer build that adds a field there must accept that an older build
  will strip it.
- A `quiz.progress` value that no longer deserializes at all leaves progress
  unreadable. Examples: a different type, a lens name this build does not
  know such as a future `"security"` key under `mastery`, or a count out of
  range for `u8`/`u32`. In that case (from the code):
  - Reads fall back to empty progress, so every question plays fresh and no
    runes show.
  - Every answer write fails with `CARTRIDGE SAVE DATA IS UNREADABLE`, which
    the recorder ignores.
  - The stored value is never overwritten, so a newer build still finds its
    data intact.

**Choice order**

- `question_attempts` is absent in older saves. Reviews then start at
  attempt 1.

## 9. Annotated example

This save combines state the tests build. It has:

- a legacy batch under `claude.question_batches`
  (`cartridge_reload_restores_legacy_claude_batches`);
- a v2 batch under `ai.question_batches` (`engine_state_question`, saved by
  `persist_ai_question_batch` at level 2);
- a foreign key (`generated_ai_batch_is_saved_without_replacing_other_game_data`);
- the exact `quiz.progress` value asserted by
  `progress_from_earlier_builds_loads_as_retired_questions`, after one miss
  on a `tradeoff` question.

Keys appear in the sorted order the writer produces.

```json
{
  "schema_version": 1,
  "data": {
    "ai.question_batches": [
      {
        "level": 2,
        "questions": [
          {
            "q": "WHAT SHOULD OWN GAMEPLAY STATE?",
            "concept": "responsibility",
            "choices": [
              { "text": "THE GAME ENGINE",  "why": "THIS NAMES THE MODEL THE DESIGN DEPENDS ON." },
              { "text": "THE DEVICE SHELL", "why": "THIS NAMES THE MODEL THE DESIGN DEPENDS ON." },
              { "text": "THE STYLES",       "why": "THIS NAMES THE MODEL THE DESIGN DEPENDS ON." },
              { "text": "THE VIEW",         "why": "THIS NAMES THE MODEL THE DESIGN DEPENDS ON." }
            ],
            "answer": 0
          }
        ]
      }
    ],
    "claude.question_batches": [
      {
        "level": 1,
        "questions": [
          {
            "q": "WHY KEEP THE DEVICE SHELL THIN?",
            "choices": [
              "TO CENTRALIZE GAME RULES",
              "TO DUPLICATE GAME STATE",
              "TO HIDE ENGINE OUTPUT",
              "TO BYPASS THE ENGINE"
            ],
            "answer": 0
          }
        ]
      }
    ],
    "quest.progress": { "bosses": 2 },
    "quiz.progress": {
      "answered_questions": ["WHAT SHOULD OWN GAMEPLAY STATE?"],
      "missed_questions": ["WHY KEEP THE SHELL THIN?"],
      "relearned_questions": [],
      "mastery": {
        "tradeoff": {
          "first_try": 0,
          "redeemed": 0,
          "missed": 1,
          "relearned": 0,
          "recent": 0,
          "recent_len": 1
        }
      },
      "question_attempts": { "WHY KEEP THE SHELL THIN?": 1 }
    }
  }
}
```

Notes on the example, from top to bottom:

1. `schema_version` must be exactly `1`, or the save is refused (section 3).
2. `ai.question_batches` is the only batch key this build appends to. Every
   choice is `{text, why}`, and `concept` holds the canonical lens name. All
   text is uppercase ASCII because provider output is normalized at parse
   time.
3. `claude.question_batches` is legacy and read-only. Its choices are plain
   strings and it has no `concept`. It still plays, without rationales or
   mastery, and it loads before the `ai.` batches.
4. `quest.progress` is a foreign namespace. Updates to other keys carry it
   through untouched.
5. `answered_questions` retires its question. Here that question is the v2
   one, so the v2 question leaves the queue and stays in the journal.
6. `missed_questions` holds a question with no batch in this save. That is
   fine: progress is keyed by identity, not by batch. A matching stem in any
   batch would come back as a `Spaced` review with an `outstanding` lesson.
7. `relearned_questions` is serialized even when it is empty.
8. `mastery.tradeoff` records one miss. `recent = 0` with `recent_len = 1`
   means one graded outcome, and it was wrong.
9. `question_attempts` is keyed by normalized identity and appears only when
   non-empty. The miss's next review continues the choice rotation from
   attempt 1.
10. `missed_picks` and `peeked_questions` are absent because they are empty:
    this miss was recorded without a pick, and nothing was revealed in the
    Codex. A miss committed in the app would add
    `"missed_picks": { "WHY KEEP THE SHELL THIN?": <choice index> }`.
