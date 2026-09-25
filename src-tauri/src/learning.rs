//! The learning model shared by question generation, cartridge saves, and the
//! engine.
//!
//! Every CODE QUEST question assesses one conceptual *lens* on a project. Each
//! choice can carry a short rationale, so feedback explains why the correct
//! answer holds and which misconception a wrong answer reveals. Committed
//! answers become evidence, and evidence fills per-lens mastery runes at
//! explicit thresholds, gated by recent accuracy and open misses. Nothing here
//! depends on Tauri or Bevy.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Characters available to one rationale line inside the lesson panel.
pub const RATIONALE_COLUMNS: usize = 34;
/// Lines a single rationale may occupy inside the lesson panel.
pub const RATIONALE_ROWS: usize = 3;
/// The longest rationale that always fits the lesson panel, however its
/// words fall, provided no single word is longer than a line.
///
/// Word wrapping only moves a word down when it does not fit beside the
/// line before it, so any two consecutive lines hold at least
/// [`RATIONALE_COLUMNS`] characters of words between them. One line more
/// than [`RATIONALE_ROWS`] therefore takes a whole column for each pair of
/// lines (plus one character for an unpaired last line) and a space between
/// every two lines: 71 characters for three lines of 34, so 70 always fit.
pub const RATIONALE_MAX_CHARS: usize =
    RATIONALE_COLUMNS * (RATIONALE_ROWS + 1) / 2 + (RATIONALE_ROWS + 1) % 2 + RATIONALE_ROWS - 1;
/// Evidence counts that awaken a lens's first, second, and third mastery rune.
pub const MASTERY_THRESHOLDS: [u32; 3] = [1, 3, 5];

/// A durable way of understanding a software project. The lenses deepen with
/// the Oracle bond: Initiate questions establish purpose and responsibilities,
/// Adept questions connect interactions and tradeoffs, and Oracle-bound
/// questions probe invariants and ask the player to predict consequences.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Concept {
    Purpose,
    Responsibility,
    Interaction,
    Invariant,
    Tradeoff,
}

impl Concept {
    pub const ALL: [Self; 5] = [
        Self::Purpose,
        Self::Responsibility,
        Self::Interaction,
        Self::Invariant,
        Self::Tradeoff,
    ];

    /// Short uppercase label that fits native HUD rows.
    pub fn label(self) -> &'static str {
        match self {
            Self::Purpose => "PURPOSE",
            Self::Responsibility => "ROLES",
            Self::Interaction => "FLOWS",
            Self::Invariant => "INVARIANTS",
            Self::Tradeoff => "TRADEOFFS",
        }
    }

    /// Lenient parser for provider output: accepts case, plural, and common
    /// synonyms, and rejects anything that does not map to a lens.
    pub fn parse(value: &str) -> Option<Self> {
        let normalized = value.trim().to_ascii_lowercase().replace(['_', ' '], "-");
        match normalized.trim_end_matches('s') {
            "purpose" | "goal" => Some(Self::Purpose),
            "responsibility" | "responsibilitie" | "role" | "ownership" => {
                Some(Self::Responsibility)
            }
            "interaction" | "flow" | "data-flow" | "collaboration" => Some(Self::Interaction),
            "invariant" | "guarantee" | "constraint" => Some(Self::Invariant),
            "tradeoff" | "trade-off" | "design-rationale" | "rationale" => Some(Self::Tradeoff),
            _ => None,
        }
    }

    /// The lenses a question batch at `level` should emphasise. This mirrors
    /// the Initiate (1), Adept (2-3), and Oracle-bound (4+) presentation tiers.
    pub fn focus_for_level(level: u32) -> &'static [Self] {
        match level {
            0 | 1 => &[Self::Purpose, Self::Responsibility],
            2 | 3 => &[Self::Interaction, Self::Tradeoff],
            _ => &[Self::Invariant, Self::Tradeoff],
        }
    }
}

/// Graded outcomes a lens remembers for its accuracy gates.
pub const RECENT_CAPACITY: u8 = 8;
/// Newest graded outcomes the rune II and rune III gates read.
pub const MASTERY_GATE_WINDOW: u8 = 5;

/// Why a question is being asked, which decides what a correct answer proves.
///
/// Only a success that cannot lean on a lesson read moments ago is mastery
/// evidence: a first attempt (`Fresh`), or a missed question checked again in
/// a later launch (`Spaced`). A retry of a miss inside the same launch
/// (`InSession`), later in the run or in another run, follows a lesson card
/// the player has just read, so its success is *relearning*: it clears the
/// pending review but lights no rune.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Review {
    /// The first time the player meets this question.
    #[default]
    Fresh,
    /// A retry of a miss made earlier in this launch.
    InSession,
    /// A question missed or relearned in an earlier launch, checked again.
    Spaced,
}

impl Review {
    /// Whether this attempt re-asks a question the player previously missed.
    pub fn is_review(self) -> bool {
        self != Self::Fresh
    }
}

/// What the engine learned when the player committed an answer.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AnswerEvidence {
    /// The question text exactly as generated; saves normalize it for identity.
    pub question: String,
    pub concept: Option<Concept>,
    pub correct: bool,
    /// Why the question was asked; see [`Review`].
    pub review: Review,
    /// The source choice index the player picked, recorded only on a miss so
    /// the Codex can show the misconception it reveals.
    pub picked: Option<usize>,
    /// The text of the choice at `picked`. A stem repeated across batches can
    /// list its choices in another order, so a reload finds the pick by text.
    pub picked_choice: Option<String>,
    /// True when the player revealed this pending lesson's answer in the Codex
    /// before this attempt, so a correct answer is relearning, not evidence.
    pub peeked: bool,
}

/// A learner-progress change the engine asks its host to persist.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProgressEvent {
    /// A committed answer.
    Answered(AnswerEvidence),
    /// The player revealed a pending lesson's answer in the Codex. The
    /// question text is exactly as generated; saves normalize it for identity.
    Peeked { question: String },
}

impl From<AnswerEvidence> for ProgressEvent {
    fn from(evidence: AnswerEvidence) -> Self {
        Self::Answered(evidence)
    }
}

/// Accumulated evidence for one lens on one cartridge.
///
/// Every field defaults, so records saved by earlier builds load unchanged.
/// Such a record has no recent outcomes, so its accuracy gates pass until new
/// answers arrive.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct LensRecord {
    /// Correct answers on a question's first attempt.
    #[serde(default)]
    pub first_try: u32,
    /// Correct answers, in a later launch, to a question previously missed.
    #[serde(default)]
    pub redeemed: u32,
    /// Wrong answers, counting every attempt.
    #[serde(default)]
    pub missed: u32,
    /// Relearning, not evidence: correct same-launch retries of a miss, and
    /// correct answers given after revealing the answer in the Codex.
    #[serde(default)]
    pub relearned: u32,
    /// The newest graded outcomes as a shift register: bit 0 is the newest,
    /// and a set bit is a correct answer. Relearning successes are not graded
    /// here, because they follow a lesson card or Codex page that gave the
    /// answer.
    #[serde(default)]
    pub recent: u8,
    /// How many bits of `recent` hold outcomes (0 to [`RECENT_CAPACITY`]).
    #[serde(default)]
    pub recent_len: u8,
}

impl LensRecord {
    /// Evidence of understanding: first-try successes plus redemptions in a
    /// later launch made without revealing the answer first. Relearning is
    /// excluded.
    pub fn evidence(&self) -> u32 {
        self.first_try.saturating_add(self.redeemed)
    }

    /// Runes the evidence volume alone would light (0-3) at the declared
    /// thresholds, before the accuracy and open-miss gates.
    pub fn volume_stage(&self) -> usize {
        MASTERY_THRESHOLDS.partition_point(|threshold| self.evidence() >= *threshold)
    }

    /// Correct answers among the newest `window` graded outcomes, and how many
    /// outcomes that window actually holds: `(right, of)`.
    pub fn recent_accuracy(&self, window: u8) -> (u8, u8) {
        let of = window.min(self.recent_len).min(RECENT_CAPACITY);
        let mask = if of >= 8 { u8::MAX } else { (1u8 << of) - 1 };
        ((self.recent & mask).count_ones() as u8, of)
    }

    /// The accuracy the rune gates read: `(right, of)` over the newest
    /// [`MASTERY_GATE_WINDOW`] graded outcomes.
    ///
    /// A save from an earlier build holds evidence the window never saw. Those
    /// successes are older than every recorded outcome, so they fill the
    /// window's older slots: an upgraded veteran's first miss weighs exactly
    /// as it would on a record that saw every answer, not as its whole sample.
    /// A record that saw every answer has none (all of its evidence is still
    /// in the window until it fills), so this is its plain recent accuracy.
    pub fn gate_accuracy(&self) -> (u32, u32) {
        let (right, of) = self.recent_accuracy(MASTERY_GATE_WINDOW);
        let unrecorded = if self.recent_len >= RECENT_CAPACITY {
            0
        } else {
            let (recorded, _) = self.recent_accuracy(RECENT_CAPACITY);
            self.evidence().saturating_sub(u32::from(recorded))
        };
        let earlier = u32::from(MASTERY_GATE_WINDOW - of).min(unrecorded);
        (u32::from(right) + earlier, u32::from(of) + earlier)
    }

    /// Lit mastery runes (0-3) for a lens with `outstanding` open misses.
    ///
    /// Volume sets the ceiling, then two gates read the newest
    /// [`MASTERY_GATE_WINDOW`] graded outcomes (see [`Self::gate_accuracy`]):
    /// - rune II stays lit only while recent accuracy is at least 60%;
    /// - rune III also needs at least 80% and no open miss on the lens.
    ///
    /// A record with no recent outcomes (a save from an earlier build) passes
    /// both accuracy gates; only the open-miss gate applies to it.
    pub fn stage_with(&self, outstanding: usize) -> usize {
        let mut stage = self.volume_stage();
        let (right, of) = self.gate_accuracy();
        if stage >= 2 && of > 0 && right * 5 < of * 3 {
            stage = 1;
        }
        if stage == 3 && (outstanding > 0 || (of > 0 && right * 5 < of * 4)) {
            stage = 2;
        }
        stage
    }

    /// Runes the volume earned that the gates hold back. They show cracked:
    /// a review is due, not progress lost.
    pub fn cracks_with(&self, outstanding: usize) -> usize {
        self.volume_stage() - self.stage_with(outstanding)
    }

    fn push_recent(&mut self, correct: bool) {
        self.recent = (self.recent << 1) | u8::from(correct);
        self.recent_len = self.recent_len.saturating_add(1).min(RECENT_CAPACITY);
    }

    /// Routes one committed answer. A first-try success and a later-launch
    /// redemption are evidence; a same-launch retry success, or any success
    /// after a Codex peek, is relearning. Every graded outcome except that
    /// relearning enters the recent window.
    pub fn record(&mut self, evidence: &AnswerEvidence) {
        match (evidence.correct, evidence.review) {
            (true, _) if evidence.peeked => self.relearned = self.relearned.saturating_add(1),
            (true, Review::Fresh) => {
                self.first_try = self.first_try.saturating_add(1);
                self.push_recent(true);
            }
            (true, Review::InSession) => self.relearned = self.relearned.saturating_add(1),
            (true, Review::Spaced) => {
                self.redeemed = self.redeemed.saturating_add(1);
                self.push_recent(true);
            }
            (false, _) => {
                self.missed = self.missed.saturating_add(1);
                self.push_recent(false);
            }
        }
    }
}

/// Per-lens mastery for one cartridge.
pub type Mastery = BTreeMap<Concept, LensRecord>;

/// Records `evidence` into `mastery` when the question named its lens.
pub fn record_evidence(mastery: &mut Mastery, evidence: &AnswerEvidence) {
    if let Some(concept) = evidence.concept {
        mastery.entry(concept).or_default().record(evidence);
    }
}

/// One entry in the player's lesson journal: a committed question, the
/// correct answer, and the rationale that explains it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Lesson {
    pub question: String,
    pub answer: String,
    /// Why the correct answer holds; empty for legacy questions without one.
    pub rationale: String,
    pub concept: Option<Concept>,
    /// True while the player's latest attempt at this question was wrong.
    pub outstanding: bool,
    /// The wrong choice the player last picked and the misconception it
    /// reveals (empty for legacy questions without rationales). A later
    /// correct answer keeps it, so a learned lesson can recall it. `None` for
    /// lessons recorded before picks were saved.
    pub misconception: Option<(String, String)>,
    /// True once the player revealed this pending lesson's answer in the
    /// Codex; the next attempt then counts as relearning, not evidence.
    pub peeked: bool,
    /// True while a relearned question waits in the deck for its spaced
    /// check. The lesson reads as learned, but the Codex seals its answer like
    /// a pending review's, so the check is never open-book.
    pub spaced_check: bool,
}

/// Whether a rationale fits the lesson panel without truncation.
pub fn rationale_fits(text: &str) -> bool {
    !text.trim().is_empty()
        && crate::engine::wrap_text(text, RATIONALE_COLUMNS).len() <= RATIONALE_ROWS
}

/// Display order for a question's four choices: `order[slot]` is the source
/// choice index shown in that slot.
///
/// Providers tend to place the correct answer first, so presenting choices in
/// source order would let position replace understanding. The base
/// permutation is derived from the question identity (uniform across the 24
/// orderings of four choices), and each further attempt rotates it by one slot,
/// so a re-asked question always moves every choice, including the answer.
pub fn presentation_order(identity: &str, attempt: u32) -> [usize; 4] {
    // FNV-1a keeps the order stable across runs, launches, and platforms.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in identity.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    // Decode a Lehmer code in 0..24 into a permutation of [0, 1, 2, 3].
    let mut code = (hash % 24) as usize;
    let mut remaining = vec![0, 1, 2, 3];
    let mut base = [0; 4];
    for (slot, radix) in [6, 2, 1, 1].into_iter().enumerate() {
        base[slot] = remaining.remove(code / radix);
        code %= radix;
    }
    let shift = (attempt % 4) as usize;
    std::array::from_fn(|slot| base[(slot + shift) % 4])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concept_parser_accepts_provider_variants_and_rejects_unknown_lenses() {
        assert_eq!(Concept::parse("Purpose"), Some(Concept::Purpose));
        assert_eq!(
            Concept::parse("responsibilities"),
            Some(Concept::Responsibility)
        );
        assert_eq!(Concept::parse("Trade-offs"), Some(Concept::Tradeoff));
        assert_eq!(Concept::parse("INVARIANTS"), Some(Concept::Invariant));
        assert_eq!(Concept::parse("data flow"), Some(Concept::Interaction));
        assert_eq!(Concept::parse("file layout"), None);
        assert_eq!(Concept::parse(""), None);
    }

    #[test]
    fn concept_labels_fit_a_compact_hud_row() {
        for concept in Concept::ALL {
            assert!(concept.label().len() <= 10, "{concept:?}");
        }
    }

    #[test]
    fn level_focus_deepens_with_the_oracle_bond() {
        assert_eq!(
            Concept::focus_for_level(1),
            [Concept::Purpose, Concept::Responsibility]
        );
        assert!(Concept::focus_for_level(3).contains(&Concept::Interaction));
        assert!(Concept::focus_for_level(4).contains(&Concept::Invariant));
        assert_eq!(Concept::focus_for_level(9), Concept::focus_for_level(4));
    }

    #[test]
    fn lens_records_separate_first_try_success_redemption_relearning_and_misses() {
        let mut mastery = Mastery::new();
        let mut evidence = AnswerEvidence {
            question: "WHY?".into(),
            concept: Some(Concept::Invariant),
            correct: false,
            review: Review::Fresh,
            picked: Some(2),
            picked_choice: None,
            peeked: false,
        };
        record_evidence(&mut mastery, &evidence);
        evidence.correct = true;
        evidence.picked = None;
        evidence.review = Review::InSession;
        record_evidence(&mut mastery, &evidence);
        evidence.review = Review::Spaced;
        record_evidence(&mut mastery, &evidence);
        evidence.review = Review::Fresh;
        record_evidence(&mut mastery, &evidence);
        // Reading the answer in the Codex first turns a redemption, or even a
        // fresh-looking success, into relearning.
        evidence.peeked = true;
        record_evidence(&mut mastery, &evidence);
        evidence.review = Review::Spaced;
        record_evidence(&mut mastery, &evidence);
        record_evidence(
            &mut mastery,
            &AnswerEvidence {
                concept: None,
                correct: true,
                ..AnswerEvidence::default()
            },
        );

        let record = mastery[&Concept::Invariant];
        assert_eq!(
            record,
            LensRecord {
                first_try: 1,
                redeemed: 1,
                missed: 1,
                // The same-launch retry and both successes after a peek.
                relearned: 3,
                // Newest first: Fresh success, Spaced success, then the miss.
                // Relearning is not graded.
                recent: 0b011,
                recent_len: 3,
            }
        );
        assert_eq!(record.evidence(), 2, "relearning is not evidence");
        assert_eq!(
            record.volume_stage(),
            1,
            "relearning never lights a rune by volume"
        );
        assert_eq!(mastery.len(), 1);
    }

    fn answer(review: Review, correct: bool) -> AnswerEvidence {
        AnswerEvidence {
            question: "WHY?".into(),
            concept: Some(Concept::Purpose),
            correct,
            review,
            ..AnswerEvidence::default()
        }
    }

    #[test]
    fn same_launch_relearning_never_moves_a_rune_but_a_spaced_redemption_does() {
        let mut record = LensRecord {
            first_try: 2,
            ..LensRecord::default()
        };
        assert_eq!(record.stage_with(0), 1);
        record.record(&answer(Review::InSession, true));
        assert_eq!(
            record.stage_with(0),
            1,
            "an in-session retry lights nothing"
        );
        assert_eq!(record.relearned, 1);
        assert_eq!(record.recent_len, 0, "an in-session success is not graded");
        record.record(&answer(Review::Spaced, true));
        assert_eq!(record.redeemed, 1);
        assert_eq!(
            record.stage_with(0),
            2,
            "a later-launch redemption is evidence"
        );
    }

    fn record_with(first_try: u32, outcomes_oldest_first: &[bool]) -> LensRecord {
        let mut record = LensRecord {
            first_try,
            ..LensRecord::default()
        };
        for correct in outcomes_oldest_first {
            record.push_recent(*correct);
        }
        record
    }

    #[test]
    fn misses_crack_runes_that_volume_alone_would_light() {
        let mut record = LensRecord::default();
        for _ in 0..5 {
            record.record(&answer(Review::Fresh, true));
        }
        for _ in 0..12 {
            record.record(&answer(Review::Fresh, false));
        }
        assert_eq!(record.volume_stage(), 3);
        assert!(record.stage_with(0) <= 1, "{record:?}");
        assert!(record.cracks_with(0) >= 2);
        assert_eq!(record.recent_len, RECENT_CAPACITY, "the window is bounded");
        assert_eq!(record.recent_accuracy(u8::MAX), (0, RECENT_CAPACITY));
    }

    #[test]
    fn an_open_miss_cracks_only_the_third_rune() {
        let record = record_with(5, &[true; 5]);
        assert_eq!(record.stage_with(0), 3);
        assert_eq!(record.cracks_with(0), 0);
        assert_eq!(record.stage_with(1), 2);
        assert_eq!(record.cracks_with(1), 1);
    }

    #[test]
    fn accuracy_gates_hold_at_their_exact_breakpoints() {
        // The newest five outcomes decide; older ones fall out of the window.
        let stage = |newest_five: [bool; 5]| {
            let mut outcomes = vec![false, false, false];
            outcomes.extend(newest_five);
            record_with(9, &outcomes).stage_with(0)
        };
        assert_eq!(stage([true, true, true, true, false]), 3, "4/5 keeps III");
        assert_eq!(stage([true, true, true, false, false]), 2, "3/5 caps at II");
        assert_eq!(stage([true, true, false, false, false]), 1, "2/5 caps at I");
        assert_eq!(stage([false; 5]), 1, "rune I is never gated");
        assert_eq!(record_with(2, &[false, false]).stage_with(0), 1);
    }

    #[test]
    fn recent_window_never_holds_more_than_its_capacity() {
        let mut record = LensRecord::default();
        for index in 0..40 {
            record.record(&answer(Review::Fresh, index % 3 == 0));
            assert!(record.recent_len <= RECENT_CAPACITY);
        }
        assert_eq!(record.recent_len, RECENT_CAPACITY);
        let (right, of) = record.recent_accuracy(MASTERY_GATE_WINDOW);
        assert_eq!(of, MASTERY_GATE_WINDOW);
        assert!(right <= of);
    }

    #[test]
    fn legacy_lens_records_load_and_keep_their_runes() {
        let record: LensRecord =
            serde_json::from_str(r#"{"first_try":4,"redeemed":1,"missed":7}"#).unwrap();
        assert_eq!(record.relearned, 0);
        assert_eq!(record.recent_len, 0);
        assert_eq!(record.stage_with(0), 3, "no recent outcomes: gates pass");
        assert_eq!(
            record.stage_with(2),
            2,
            "an open miss still cracks rune III"
        );
    }

    #[test]
    fn an_upgraded_veterans_first_miss_weighs_as_one_answer_among_its_history() {
        let mut veteran: LensRecord =
            serde_json::from_str(r#"{"first_try":20,"redeemed":0,"missed":0}"#).unwrap();
        veteran.record(&answer(Review::Fresh, false));
        assert_eq!(veteran.volume_stage(), 3);
        assert_eq!(
            veteran.gate_accuracy(),
            (4, 5),
            "earlier successes fill the window"
        );
        assert_eq!(
            (veteran.stage_with(1), veteran.cracks_with(1)),
            (2, 1),
            "one open miss cracks rune III only, as on a record that saw every answer"
        );
        let mut seen_all = LensRecord::default();
        for _ in 0..20 {
            seen_all.record(&answer(Review::Fresh, true));
        }
        seen_all.record(&answer(Review::Fresh, false));
        assert_eq!(veteran.stage_with(1), seen_all.stage_with(1));
        // Further misses still pull the gates down.
        veteran.record(&answer(Review::Fresh, false));
        veteran.record(&answer(Review::Fresh, false));
        assert_eq!(veteran.gate_accuracy(), (2, 5));
        assert_eq!(veteran.stage_with(0), 1);
    }

    #[test]
    fn gate_accuracy_is_plain_recent_accuracy_for_a_record_that_saw_every_answer() {
        let mut record = LensRecord::default();
        for (index, correct) in [true, true, false, true, true, true, false, true, true]
            .into_iter()
            .enumerate()
        {
            record.record(&answer(Review::Fresh, correct));
            let (right, of) = record.recent_accuracy(MASTERY_GATE_WINDOW);
            assert_eq!(
                record.gate_accuracy(),
                (u32::from(right), u32::from(of)),
                "after answer {index}"
            );
        }
    }

    #[test]
    fn mastery_runes_light_at_the_exact_declared_thresholds() {
        let stage = |first_try| {
            LensRecord {
                first_try,
                ..LensRecord::default()
            }
            .stage_with(0)
        };
        assert_eq!(
            [0, 1, 2, 3, 4, 5, 6].map(stage),
            [0, 1, 1, 2, 2, 3, 3],
            "runes wake at {MASTERY_THRESHOLDS:?}"
        );
    }

    #[test]
    fn rationales_must_be_present_and_fit_the_lesson_panel() {
        assert!(rationale_fits(
            "THE ENGINE OWNS RULES, SO THE SHELL CAN ONLY DRAW WHAT IT IS GIVEN."
        ));
        assert!(!rationale_fits("   "));
        assert!(!rationale_fits(&"TOO LONG ".repeat(20)));
    }

    #[test]
    fn every_rationale_within_the_stated_maximum_fits_however_its_words_fall() {
        let words = |lengths: &[usize]| {
            lengths
                .iter()
                .zip(b'A'..)
                .map(|(length, letter)| char::from(letter).to_string().repeat(*length))
                .collect::<Vec<_>>()
                .join(" ")
        };
        assert_eq!(RATIONALE_MAX_CHARS, 70);

        // The worst case: each pair of lines is filled so that the next word
        // just misses (the third word is no shorter than the first, so it
        // cannot join the second line). One character more than the maximum
        // needs a 4th line; shortening any word by one brings it back to 3.
        for first in 1..RATIONALE_COLUMNS {
            for third in first..RATIONALE_COLUMNS {
                let lengths = [
                    first,
                    RATIONALE_COLUMNS - first,
                    third,
                    RATIONALE_COLUMNS - third,
                ];
                let overflow = words(&lengths);
                assert_eq!(overflow.len(), RATIONALE_MAX_CHARS + 1);
                assert!(!rationale_fits(&overflow), "{overflow}");
                for shortened in (0..lengths.len()).filter(|index| lengths[*index] > 1) {
                    let mut lengths = lengths;
                    lengths[shortened] -= 1;
                    let text = words(&lengths);
                    assert!(text.len() <= RATIONALE_MAX_CHARS);
                    assert!(rationale_fits(&text), "{text}");
                }
            }
        }

        // A review's counterexample: 84 characters needed four lines. Cut to
        // the stated maximum, it fits.
        let reviewed =
            "A BYPASS SPLITS THE RULES INCONSISTENTLY BETWEEN PRESENTATION AND ENGINE BOUNDARIES.";
        assert!(!rationale_fits(reviewed));
        let cut = &reviewed[..reviewed[..=RATIONALE_MAX_CHARS].rfind(' ').unwrap()];
        assert!(rationale_fits(cut), "{cut}");

        // Any other arrangement of words up to a line long.
        let mut state = 0x9E37_79B9_7F4A_7C15_u64;
        for _ in 0..20_000 {
            let mut lengths = Vec::new();
            loop {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                let length = 1 + (state % RATIONALE_COLUMNS as u64) as usize;
                let used = lengths.iter().map(|length| length + 1).sum::<usize>();
                if used + length > RATIONALE_MAX_CHARS {
                    break;
                }
                lengths.push(length);
            }
            let text = words(&lengths);
            assert!(rationale_fits(&text), "{text}");
        }
    }

    #[test]
    fn presentation_order_is_a_stable_permutation() {
        let order = presentation_order("WHO OWNS THE GAME LOOP?", 0);
        let mut sorted = order;
        sorted.sort_unstable();
        assert_eq!(sorted, [0, 1, 2, 3]);
        assert_eq!(order, presentation_order("WHO OWNS THE GAME LOOP?", 0));
    }

    #[test]
    fn presentation_order_spreads_first_listed_answers_across_every_slot() {
        let mut slot_counts = [0u32; 4];
        for index in 0..400 {
            let order = presentation_order(&format!("CONCEPT QUESTION {index}"), 0);
            let answer_slot = order.iter().position(|source| *source == 0).unwrap();
            slot_counts[answer_slot] += 1;
        }
        for count in slot_counts {
            assert!((60..=140).contains(&count), "{slot_counts:?}");
        }
    }

    #[test]
    fn every_retry_moves_every_choice_to_a_new_slot() {
        for index in 0..50 {
            let identity = format!("RETRIED QUESTION {index}");
            for attempt in 0..6 {
                let before = presentation_order(&identity, attempt);
                let after = presentation_order(&identity, attempt + 1);
                for source in 0..4 {
                    let slot_before = before.iter().position(|s| *s == source);
                    let slot_after = after.iter().position(|s| *s == source);
                    assert_ne!(slot_before, slot_after, "{identity} attempt {attempt}");
                }
            }
        }
    }
}
