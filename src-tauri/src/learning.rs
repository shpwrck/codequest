//! The learning model shared by question generation, cartridge saves, and the
//! engine.
//!
//! Every CODE QUEST question assesses one conceptual *lens* on a project. Each
//! choice can carry a short rationale, so feedback explains why the correct
//! answer holds and which misconception a wrong answer reveals. Committed
//! answers become evidence, and evidence fills per-lens mastery runes at
//! explicit thresholds. Nothing here depends on Tauri or Bevy.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Characters available to one rationale line inside the lesson panel.
pub const RATIONALE_COLUMNS: usize = 34;
/// Lines a single rationale may occupy inside the lesson panel.
pub const RATIONALE_ROWS: usize = 3;
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

/// What the engine learned when the player committed an answer.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AnswerEvidence {
    /// The question text exactly as generated; saves normalize it for identity.
    pub question: String,
    pub concept: Option<Concept>,
    pub correct: bool,
    /// True when this attempt re-asked a question the player previously missed.
    pub review: bool,
    /// The source choice index the player picked, recorded only on a miss so
    /// the Codex can show the misconception it reveals.
    pub picked: Option<usize>,
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
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct LensRecord {
    /// Correct answers on a question's first attempt.
    #[serde(default)]
    pub first_try: u32,
    /// Correct answers to a question the player had previously missed.
    #[serde(default)]
    pub redeemed: u32,
    /// Wrong answers, counting every attempt.
    #[serde(default)]
    pub missed: u32,
    /// Correct answers given after revealing the answer in the Codex. They
    /// show the lesson was relearned, so they are not mastery evidence.
    /// Omitted from saves while zero, so unchanged progress keeps its shape.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub relearned: u32,
}

fn is_zero(value: &u32) -> bool {
    *value == 0
}

impl LensRecord {
    /// Evidence of understanding: first-try successes plus redemptions made
    /// without revealing the answer first. Relearning is not evidence.
    pub fn evidence(&self) -> u32 {
        self.first_try.saturating_add(self.redeemed)
    }

    /// Number of mastery runes lit (0-3) at the declared thresholds.
    pub fn stage(&self) -> usize {
        MASTERY_THRESHOLDS.partition_point(|threshold| self.evidence() >= *threshold)
    }

    pub fn record(&mut self, evidence: &AnswerEvidence) {
        match (evidence.correct, evidence.review) {
            (true, _) if evidence.peeked => self.relearned = self.relearned.saturating_add(1),
            (true, false) => self.first_try = self.first_try.saturating_add(1),
            (true, true) => self.redeemed = self.redeemed.saturating_add(1),
            (false, _) => self.missed = self.missed.saturating_add(1),
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
    fn lens_records_separate_first_try_success_redemption_and_misses() {
        let mut mastery = Mastery::new();
        let mut evidence = AnswerEvidence {
            question: "WHY?".into(),
            concept: Some(Concept::Invariant),
            correct: false,
            review: false,
            picked: Some(2),
            peeked: false,
        };
        record_evidence(&mut mastery, &evidence);
        evidence.correct = true;
        evidence.picked = None;
        evidence.review = true;
        record_evidence(&mut mastery, &evidence);
        evidence.review = false;
        record_evidence(&mut mastery, &evidence);
        // Reading the answer in the Codex first turns a redemption, or even a
        // fresh-looking success, into relearning.
        evidence.peeked = true;
        record_evidence(&mut mastery, &evidence);
        evidence.review = true;
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
                relearned: 2,
            }
        );
        assert_eq!(record.evidence(), 2);
        assert_eq!(record.stage(), 1, "relearning never lights a rune");
        assert_eq!(mastery.len(), 1);
    }

    #[test]
    fn mastery_runes_light_at_the_exact_declared_thresholds() {
        let stage = |first_try| {
            LensRecord {
                first_try,
                ..LensRecord::default()
            }
            .stage()
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
