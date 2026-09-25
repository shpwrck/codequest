//! Deterministic randomized ("property") tests for the release's pure logic:
//! choice shuffling, mastery runes, answer evidence, the provider-response
//! parser and its acceptance policy, the cartridge question queue, saves,
//! provenance parsing, and text wrapping.
//!
//! Each property runs a few hundred cases drawn from a seeded SplitMix64
//! generator, so every run checks the same inputs and a failure names the
//! property seed and case number that reproduce it. No test dependency is
//! needed.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Debug;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};

use crate::engine::{quiz_question_fits, wrap_text};
use crate::learning::{self, AnswerEvidence, Concept, LensRecord, Mastery, MASTERY_THRESHOLDS};
use crate::provenance;
use crate::questions::{self, QChoice, QQuestion, SavedQuestionBatch, SavedQuizProgress};
use crate::save;

/// Cases per property. Every property together stays well under a second.
const CASES: usize = 300;

/// SplitMix64: tiny, fast, and statistically sound enough for test inputs.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        let mut rng = Self(seed);
        rng.next_u64();
        rng
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut mixed = self.0;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        mixed ^ (mixed >> 31)
    }

    /// A value in `0..bound`; `bound` must be positive.
    fn below(&mut self, bound: usize) -> usize {
        assert!(bound > 0, "empty range");
        (self.next_u64() % bound as u64) as usize
    }

    /// A value in `low..=high`.
    fn range(&mut self, low: usize, high: usize) -> usize {
        low + self.below(high - low + 1)
    }

    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }

    /// A `u32` biased toward both ends of its range, where saturation lives.
    fn edgy_u32(&mut self) -> u32 {
        match self.below(4) {
            0 => self.below(12) as u32,
            1 => u32::MAX - self.below(12) as u32,
            _ => self.next_u64() as u32,
        }
    }
}

/// One independently seeded generator per case, so a failing case can be
/// replayed alone from its number.
fn cases(seed: u64, count: usize) -> impl Iterator<Item = (usize, Rng)> {
    (0..count).map(move |case| {
        let case_seed = seed ^ (case as u64 + 1).wrapping_mul(0xD1B5_4A32_D192_ED03);
        (case, Rng::new(case_seed))
    })
}

/// Any character: printable ASCII most often, plus Unicode whitespace,
/// control characters, typographic punctuation, combining marks, and
/// characters outside the Basic Multilingual Plane.
fn arbitrary_char(rng: &mut Rng) -> char {
    match rng.below(10) {
        0..=4 => char::from(b' ' + rng.below(95) as u8),
        5 => *rng.pick(&[
            ' ', '\t', '\n', '\r', '\u{a0}', '\u{85}', '\u{2003}', '\u{2028}', '\u{3000}',
        ]),
        6 => char::from(rng.below(32) as u8),
        7 => *rng.pick(&[
            '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}', '\u{2026}', '\u{2014}', '\u{2212}',
            '\u{a9}', '\u{e9}', '\u{df}', '\u{301}', '\u{7f}',
        ]),
        8 => char::from_u32(0x80 + rng.below(0x780) as u32).unwrap_or('?'),
        _ => *rng.pick(&[
            '\u{1F600}',
            '\u{4E2D}',
            '\u{FEFF}',
            '\u{200B}',
            '\u{10FFFF}',
            '\u{0}',
        ]),
    }
}

fn arbitrary_text(rng: &mut Rng, max_chars: usize) -> String {
    let length = rng.range(0, max_chars);
    (0..length).map(|_| arbitrary_char(rng)).collect()
}

/// `stem` as a provider or an older build might spell it: random ASCII case
/// and random runs of (Unicode) whitespace between and around its words.
fn respelled(rng: &mut Rng, stem: &str) -> String {
    const GAPS: [&str; 5] = [" ", "  ", "\t", "\n", " \u{3000}"];
    let mut text = String::new();
    if rng.chance(20) {
        text.push_str(rng.pick(&GAPS));
    }
    for (index, word) in stem.split_whitespace().enumerate() {
        if index > 0 {
            let gap = if rng.chance(70) {
                " "
            } else {
                *rng.pick(&GAPS)
            };
            text.push_str(gap);
        }
        for character in word.chars() {
            text.push(if rng.chance(30) {
                character.to_ascii_lowercase()
            } else {
                character
            });
        }
    }
    if rng.chance(20) {
        text.push_str(rng.pick(&GAPS));
    }
    text
}

// ---------------------------------------------------------------------------
// learning::presentation_order
// ---------------------------------------------------------------------------

fn slot_of(order: [usize; 4], source: usize) -> usize {
    order
        .iter()
        .position(|shown| *shown == source)
        .expect("every source choice has a slot")
}

/// The app's acceptance of one reply before any repair: accepted questions, or
/// an error when none survive.
fn parse_generated_questions(
    response: &str,
    count: usize,
) -> Result<Vec<questions::QQuestion>, String> {
    let batch = questions::parse_generated_batch(response, count)?;
    if batch.accepted.is_empty() {
        Err("INCOMPLETE OR INVALID QUESTIONS".to_string())
    } else {
        Ok(batch.accepted)
    }
}

#[test]
fn presentation_order_is_a_stable_permutation_that_moves_every_choice_on_retry() {
    for (case, mut rng) in cases(0x0DE5, CASES) {
        let identity = if rng.chance(50) {
            arbitrary_text(&mut rng, 40)
        } else {
            questions::question_identity(&respelled(&mut rng, "WHO OWNS THE GAME LOOP?"))
        };
        let attempt = rng.edgy_u32();
        let order = learning::presentation_order(&identity, attempt);
        let context = format!("case {case}: {identity:?} attempt {attempt}");

        let mut sorted = order;
        sorted.sort_unstable();
        assert_eq!(sorted, [0, 1, 2, 3], "not a permutation, {context}");
        assert_eq!(
            order,
            learning::presentation_order(&identity, attempt),
            "unstable, {context}"
        );
        // The rotation has period four, including across u32 wraparound.
        assert_eq!(
            order,
            learning::presentation_order(&identity, attempt.wrapping_add(4)),
            "{context}"
        );

        let next = learning::presentation_order(&identity, attempt.wrapping_add(1));
        for source in 0..4 {
            assert_ne!(
                slot_of(order, source),
                slot_of(next, source),
                "choice {source} kept its slot on retry, {context}"
            );
        }

        // Four consecutive attempts show the answer in every slot once.
        let answer_slots = (0..4)
            .map(|offset| {
                slot_of(
                    learning::presentation_order(&identity, attempt.wrapping_add(offset)),
                    0,
                )
            })
            .collect::<HashSet<_>>();
        assert_eq!(answer_slots.len(), 4, "{context}");
    }
}

// ---------------------------------------------------------------------------
// learning::LensRecord and record_evidence
// ---------------------------------------------------------------------------

fn thresholds_reached(evidence: u32) -> usize {
    MASTERY_THRESHOLDS
        .iter()
        .filter(|threshold| evidence >= **threshold)
        .count()
}

#[test]
fn lens_stage_matches_the_declared_thresholds_and_never_falls() {
    for (case, mut rng) in cases(0x57A6, CASES) {
        let record = LensRecord {
            first_try: rng.edgy_u32(),
            redeemed: rng.edgy_u32(),
            missed: rng.edgy_u32(),
        };
        let context = format!("case {case}: {record:?}");
        let evidence = record.evidence();
        assert_eq!(
            evidence,
            record.first_try.saturating_add(record.redeemed),
            "{context}"
        );
        assert_eq!(record.stage(), thresholds_reached(evidence), "{context}");
        assert!(record.stage() <= MASTERY_THRESHOLDS.len(), "{context}");

        for (correct, review) in [(true, false), (true, true), (false, false), (false, true)] {
            let mut next = record;
            next.record(&AnswerEvidence {
                question: "WHY?".into(),
                concept: Some(Concept::Invariant),
                correct,
                review,
            });
            let step = format!("{context} after correct={correct} review={review}");
            assert!(next.stage() >= record.stage(), "stage fell, {step}");
            assert_eq!(next.stage(), thresholds_reached(next.evidence()), "{step}");
            if !correct {
                assert_eq!(next.evidence(), evidence, "a miss is not evidence, {step}");
                assert_eq!(next.missed, record.missed.saturating_add(1), "{step}");
            } else if evidence < u32::MAX {
                assert_eq!(next.evidence(), evidence + 1, "{step}");
                let crossed = MASTERY_THRESHOLDS.contains(&(evidence + 1));
                assert_eq!(
                    next.stage(),
                    record.stage() + usize::from(crossed),
                    "a rune wakes exactly at a threshold, {step}"
                );
            }
        }
    }
}

fn arbitrary_concept(rng: &mut Rng) -> Option<Concept> {
    if rng.chance(20) {
        None
    } else {
        Some(*rng.pick(&Concept::ALL))
    }
}

#[test]
fn record_evidence_counts_every_answer_exactly_once() {
    for (case, mut rng) in cases(0xE71D, CASES) {
        let mut mastery = Mastery::new();
        let mut expected: BTreeMap<Concept, LensRecord> = BTreeMap::new();
        let mut lensed_answers = 0u32;
        let mut stages: BTreeMap<Concept, usize> = BTreeMap::new();
        for step in 0..rng.range(0, 40) {
            let evidence = AnswerEvidence {
                question: arbitrary_text(&mut rng, 12),
                concept: arbitrary_concept(&mut rng),
                correct: rng.chance(60),
                review: rng.chance(40),
            };
            learning::record_evidence(&mut mastery, &evidence);
            if let Some(concept) = evidence.concept {
                lensed_answers += 1;
                let model = expected.entry(concept).or_default();
                match (evidence.correct, evidence.review) {
                    (true, false) => model.first_try += 1,
                    (true, true) => model.redeemed += 1,
                    (false, _) => model.missed += 1,
                }
                let stage = mastery[&concept].stage();
                let previous = stages.insert(concept, stage).unwrap_or(0);
                assert!(stage >= previous, "case {case} step {step}: rune went dark");
            }
        }
        assert_eq!(mastery, expected, "case {case}");
        let total = mastery
            .values()
            .map(|record| record.first_try + record.redeemed + record.missed)
            .sum::<u32>();
        assert_eq!(
            total, lensed_answers,
            "case {case}: an answer was double counted"
        );
        for record in mastery.values() {
            assert_eq!(record.stage(), thresholds_reached(record.evidence()));
        }
    }
}

const PROGRESS_STEMS: [&str; 6] = [
    "WHY WRITE SAVES ATOMICALLY?",
    "WHO OWNS THE RULES?",
    "WHAT DOES THE SAVE LOCK GUARD?",
    "WHY KEEP THE SHELL THIN?",
    "   ",
    "",
];

#[test]
fn quiz_progress_lets_the_latest_attempt_decide_and_never_double_counts() {
    for (case, mut rng) in cases(0x960F, CASES) {
        let mut progress = SavedQuizProgress::default();
        let mut latest: HashMap<String, bool> = HashMap::new();
        let mut expected_mastery = Mastery::new();
        for step in 0..rng.range(0, 30) {
            let stem = *rng.pick(&PROGRESS_STEMS);
            let evidence = AnswerEvidence {
                question: respelled(&mut rng, stem),
                concept: arbitrary_concept(&mut rng),
                correct: rng.chance(55),
                review: rng.chance(40),
            };
            progress.record(&evidence);
            let identity = questions::question_identity(&evidence.question);
            if !identity.is_empty() {
                latest.insert(identity, evidence.correct);
                learning::record_evidence(&mut expected_mastery, &evidence);
            }

            let context = format!("case {case} step {step}: {progress:?}");
            let answered = progress
                .answered_questions
                .iter()
                .map(|question| questions::question_identity(question))
                .collect::<Vec<_>>();
            let missed = progress
                .missed_questions
                .iter()
                .map(|question| questions::question_identity(question))
                .collect::<Vec<_>>();
            let answered_set = answered.iter().collect::<HashSet<_>>();
            let missed_set = missed.iter().collect::<HashSet<_>>();
            assert_eq!(answered_set.len(), answered.len(), "duplicate, {context}");
            assert_eq!(missed_set.len(), missed.len(), "duplicate, {context}");
            assert!(answered_set.is_disjoint(&missed_set), "{context}");
            assert!(!answered_set.contains(&String::new()), "{context}");
            assert!(!missed_set.contains(&String::new()), "{context}");
        }

        let retired = progress.retired();
        let expected_retired = latest
            .iter()
            .filter(|(_, correct)| **correct)
            .map(|(identity, _)| identity.clone())
            .collect::<HashSet<_>>();
        let expected_missed = latest
            .iter()
            .filter(|(_, correct)| !**correct)
            .map(|(identity, _)| identity.clone())
            .collect::<HashSet<_>>();
        let missed = progress
            .missed_questions
            .iter()
            .map(|question| questions::question_identity(question))
            .collect::<HashSet<_>>();
        assert_eq!(retired, expected_retired, "case {case}");
        assert_eq!(missed, expected_missed, "case {case}");
        assert_eq!(progress.mastery, expected_mastery, "case {case}");

        let saved = serde_json::to_value(&progress).unwrap();
        let reloaded = serde_json::from_value::<SavedQuizProgress>(saved).unwrap();
        assert_eq!(
            reloaded, progress,
            "case {case}: progress must survive a save"
        );
    }
}

// ---------------------------------------------------------------------------
// questions::parse_generated_questions and the acceptance policy
// ---------------------------------------------------------------------------

/// Words that keep a question conceptual.
const SAFE_WORDS: [&str; 40] = [
    "THE",
    "ENGINE",
    "OWNS",
    "EVERY",
    "RULE",
    "SHELL",
    "DRAWS",
    "FRAMES",
    "WHY",
    "DOES",
    "ONE",
    "OWNER",
    "KEEPS",
    "RULES",
    "SAVES",
    "ARE",
    "WRITTEN",
    "ATOMICALLY",
    "SO",
    "A",
    "CRASH",
    "CANNOT",
    "CORRUPT",
    "STATE",
    "WHAT",
    "HAPPENS",
    "IF",
    "INPUT",
    "QUEUE",
    "CACHE",
    "TRADES",
    "MEMORY",
    "FOR",
    "SPEED",
    "LOCK",
    "GUARDS",
    "WRITES",
    "TO",
    "OF",
    "AND",
];

/// Words the policy must reject or normalize: locations, state-in-time
/// trivia, typography, and characters the handheld font cannot draw.
const RISKY_WORDS: [&str; 22] = [
    "SRC/MAIN.RS",
    "README",
    "2021",
    "V1.2.3",
    "ABC1234",
    "WHERE IS",
    "HOW MANY",
    "COMMITS",
    "AUTHOR",
    "WHICH FILE",
    ".GITIGNORE",
    "C:\\TEMP",
    "\u{201C}WHY\u{201D}",
    "WAIT\u{2026}",
    "CAF\u{c9}",
    "\u{1F600}",
    "NODE.JS",
    "READ/WRITE",
    "SUPERCALIFRAGILISTICEXPIALIDOCIOUS",
    "\u{2014}",
    "ENGINE.RS",
    "\t",
];

const CONCEPT_SPELLINGS: [&str; 10] = [
    "responsibility",
    "Responsibilities",
    "purpose",
    "Trade-offs",
    "INVARIANTS",
    "data flow",
    "Goal",
    "file layout",
    "",
    "trivia",
];

fn phrase(rng: &mut Rng, min_words: usize, max_words: usize) -> String {
    let mut text = String::new();
    for index in 0..rng.range(min_words, max_words) {
        if index > 0 {
            text.push_str(if rng.chance(90) { " " } else { "  " });
        }
        let word = if rng.chance(2) {
            *rng.pick(&RISKY_WORDS)
        } else {
            *rng.pick(&SAFE_WORDS)
        };
        if rng.chance(30) {
            text.push_str(&word.to_lowercase());
        } else {
            text.push_str(word);
        }
    }
    text
}

/// A provider question object, usually well formed, sometimes not.
fn generated_question_value(rng: &mut Rng, stems: &mut Vec<String>) -> Value {
    let q = if !stems.is_empty() && rng.chance(15) {
        let stem = rng.pick(stems.as_slice()).clone();
        respelled(rng, &stem)
    } else {
        let stem = format!(
            "{}{}",
            phrase(rng, 3, 9),
            if rng.chance(80) { "?" } else { "" }
        );
        stems.push(stem.clone());
        stem
    };
    let choice_count = *rng.pick(&[4, 4, 4, 4, 4, 4, 4, 3, 5, 0]);
    let style = rng.below(12);
    let choices = (0..choice_count)
        .map(|_| {
            let text = phrase(rng, 1, 4);
            let why = phrase(rng, 3, 12);
            let plain = style == 0 || (style == 1 && rng.chance(50));
            if plain {
                json!(text)
            } else if rng.chance(3) {
                json!({ "text": text })
            } else {
                json!({ "text": text, "why": why })
            }
        })
        .collect::<Vec<_>>();
    let answer = if rng.chance(90) {
        rng.below(4)
    } else {
        rng.range(4, 9)
    };
    let mut value = json!({ "q": q, "choices": choices, "answer": answer });
    if rng.chance(90) {
        value["concept"] = json!(*rng.pick(&CONCEPT_SPELLINGS));
    }
    if rng.chance(8) {
        let object = value.as_object_mut().unwrap();
        match rng.below(5) {
            0 => {
                object.remove("q");
            }
            1 => {
                object.insert("q".into(), Value::Null);
            }
            2 => {
                object.insert("answer".into(), json!(-1));
            }
            3 => {
                object.insert("answer".into(), json!("0"));
            }
            _ => {
                object.insert("choices".into(), json!("A, B, C, D"));
            }
        }
    }
    value
}

/// A whole provider reply: an array of question objects, possibly wrapped in
/// prose or code fences, preceded by other brackets, or cut off.
fn provider_response(rng: &mut Rng) -> String {
    let mut stems = Vec::new();
    let values = (0..rng.range(0, 6))
        .map(|_| generated_question_value(rng, &mut stems))
        .collect::<Vec<_>>();
    let body = if rng.chance(50) {
        serde_json::to_string(&values).unwrap()
    } else {
        serde_json::to_string_pretty(&values).unwrap()
    };
    let mut response = match rng.below(4) {
        0 => body,
        1 => format!("Here are the questions [as requested]:\n```json\n{body}\n```\n"),
        2 => format!("[\"not questions\", 1] and then {body}"),
        _ => format!("Sure! {body} Let me know [if] you need more."),
    };
    if rng.chance(25) {
        let cut = rng.below(response.chars().count() + 1);
        response = response.chars().take(cut).collect();
    }
    response
}

/// Random character edits: deletions, insertions, and duplicated slices.
fn fuzzed(rng: &mut Rng, text: &str) -> String {
    let mut characters = text.chars().collect::<Vec<_>>();
    for _ in 0..rng.range(1, 8) {
        let at = rng.below(characters.len() + 1);
        match rng.below(3) {
            0 if at < characters.len() => {
                let end = (at + rng.range(1, 12)).min(characters.len());
                characters.drain(at..end);
            }
            1 if at < characters.len() => {
                let end = (at + rng.range(1, 12)).min(characters.len());
                let slice = characters[at..end].to_vec();
                characters.splice(at..at, slice);
            }
            _ => characters.insert(at, arbitrary_char(rng)),
        }
    }
    characters.into_iter().collect()
}

const JSON_TOKENS: [&str; 27] = [
    "[",
    "]",
    "{",
    "}",
    ",",
    ":",
    "\"q\"",
    "\"choices\"",
    "\"answer\"",
    "\"concept\"",
    "\"text\"",
    "\"why\"",
    "\"WHY DOES THE ENGINE OWN RULES?\"",
    "0",
    "-1",
    "3",
    "1e999",
    "18446744073709551616",
    "null",
    "true",
    "\"",
    "\\",
    "\"\\ud800\"",
    "```json",
    "\n",
    " ",
    "\u{201C}",
];

fn json_token_soup(rng: &mut Rng) -> String {
    (0..rng.range(0, 60))
        .map(|_| *rng.pick(&JSON_TOKENS))
        .collect()
}

const PARSE_ERRORS: [&str; 3] = [
    "NO JSON IN RESPONSE",
    "UNPARSEABLE QUESTIONS",
    "INCOMPLETE OR INVALID QUESTIONS",
];
const CONCEPT_KEYS: [&str; 5] = [
    "purpose",
    "responsibility",
    "interaction",
    "invariant",
    "tradeoff",
];

/// Everything an accepted batch promises the engine and the save.
fn assert_accepted_batch(accepted: &[QQuestion], count: usize, context: &str) {
    assert!(!accepted.is_empty(), "empty success, {context}");
    assert!(
        accepted.len() <= count,
        "over the requested count, {context}"
    );
    let mut identities = HashSet::new();
    for question in accepted {
        let context = format!("{context}\n{question:?}");
        assert!(
            questions::generated_question_is_acceptable(question),
            "{context}"
        );
        assert!(
            questions::question_is_acceptable(question),
            "an accepted question would be dropped on reload, {context}"
        );
        let choices = question.choice_texts();
        assert!(
            quiz_question_fits(&question.q, &choices, question.answer),
            "{context}"
        );
        assert!(
            question
                .concept
                .as_deref()
                .is_some_and(|concept| CONCEPT_KEYS.contains(&concept)),
            "lens not normalized, {context}"
        );
        assert!(
            question
                .choices
                .iter()
                .all(|choice| matches!(choice, QChoice::Explained { .. })),
            "{context}"
        );
        let rationales = question.rationales();
        assert_eq!(rationales.len(), choices.len(), "{context}");
        assert!(
            rationales.iter().all(|why| learning::rationale_fits(why)),
            "{context}"
        );
        for text in std::iter::once(&question.q)
            .chain(&choices)
            .chain(&rationales)
        {
            assert!(
                text.chars()
                    .all(|character| (' '..='~').contains(&character)),
                "unrenderable {text:?}, {context}"
            );
            assert_eq!(*text, text.to_ascii_uppercase(), "{context}");
            assert_eq!(
                text.split_whitespace().collect::<Vec<_>>().join(" "),
                *text,
                "{context}"
            );
        }
        assert!(
            identities.insert(questions::question_identity(&question.q)),
            "duplicate identity, {context}"
        );
    }
}

#[test]
fn provider_replies_yield_only_questions_that_pass_the_acceptance_policy() {
    let mut accepted_replies = 0;
    let mut rejected_replies = 0;
    for (case, mut rng) in cases(0xA11E, CASES) {
        let response = provider_response(&mut rng);
        let count = rng.range(0, 8);
        let context = format!("case {case}, count {count}: {response}");
        match parse_generated_questions(&response, count) {
            Ok(accepted) => {
                accepted_replies += 1;
                assert_accepted_batch(&accepted, count, &context);
                // Normalization is idempotent: an accepted batch re-parses to
                // itself, so saved questions never drift between builds.
                let again = serde_json::to_string(&accepted).unwrap();
                assert_eq!(
                    parse_generated_questions(&again, accepted.len()),
                    Ok(accepted.clone()),
                    "{context}"
                );
                assert_eq!(
                    questions::retain_acceptable_questions(accepted.clone()),
                    accepted,
                    "{context}"
                );
            }
            Err(error) => {
                rejected_replies += 1;
                assert!(PARSE_ERRORS.contains(&error.as_str()), "{error}: {context}");
            }
        }
    }
    // Both outcomes must be common, or the property checks nothing.
    assert!(
        accepted_replies >= CASES / 10,
        "{accepted_replies} accepted"
    );
    assert!(
        rejected_replies >= CASES / 10,
        "{rejected_replies} rejected"
    );
}

#[test]
fn provider_reply_parsing_never_panics_on_garbage_or_truncation() {
    for (case, mut rng) in cases(0x6A2B, CASES * 2) {
        let response = match rng.below(4) {
            0 => arbitrary_text(&mut rng, 200),
            1 => json_token_soup(&mut rng),
            _ => {
                let reply = provider_response(&mut rng);
                fuzzed(&mut rng, &reply)
            }
        };
        let count = rng.range(0, 8);
        let context = format!("case {case}, count {count}: {response:?}");
        match parse_generated_questions(&response, count) {
            Ok(accepted) => assert_accepted_batch(&accepted, count, &context),
            Err(error) => assert!(PARSE_ERRORS.contains(&error.as_str()), "{error}: {context}"),
        }
    }
}

// ---------------------------------------------------------------------------
// questions::cartridge_questions
// ---------------------------------------------------------------------------

const CARTRIDGE_STEMS: [&str; 9] = [
    "WHY DOES THE ENGINE OWN EVERY RULE?",
    "WHO OWNS GAMEPLAY STATE?",
    "WHY WRITE SAVES ATOMICALLY?",
    "WHAT DOES THE SAVE LOCK GUARD?",
    "WHY KEEP THE SHELL THIN?",
    "WHAT BREAKS IF TWO WRITERS RACE?",
    "WHY SHUFFLE THE CHOICES?",
    "WHAT DOES A LENS RUNE MEAN?",
    "  ",
];

fn saved_question(rng: &mut Rng, stem: &str) -> QQuestion {
    let explained = rng.chance(70);
    let choices = (0..4)
        .map(|index| {
            let text = format!("CHOICE {index} {}", rng.below(100));
            if explained {
                QChoice::Explained {
                    text,
                    why: phrase(rng, 3, 8),
                }
            } else {
                QChoice::Plain(text)
            }
        })
        .collect();
    QQuestion {
        q: respelled(rng, stem),
        concept: explained.then(|| CONCEPT_KEYS[rng.below(CONCEPT_KEYS.len())].to_string()),
        choices,
        answer: rng.below(4),
    }
}

fn arbitrary_mastery(rng: &mut Rng) -> Mastery {
    (0..rng.below(4))
        .map(|_| {
            (
                *rng.pick(&Concept::ALL),
                LensRecord {
                    first_try: rng.below(8) as u32,
                    redeemed: rng.below(8) as u32,
                    missed: rng.below(8) as u32,
                },
            )
        })
        .collect()
}

fn listed_questions(rng: &mut Rng) -> Vec<String> {
    (0..rng.range(0, 6))
        .map(|_| {
            let stem = *rng.pick(&CARTRIDGE_STEMS);
            respelled(rng, stem)
        })
        .collect()
}

#[test]
fn cartridge_queue_skips_retired_questions_and_orders_batches_by_level() {
    for (case, mut rng) in cases(0xCA27, CASES) {
        let batches = (0..rng.range(0, 6))
            .map(|_| SavedQuestionBatch {
                level: rng.range(1, 6) as u32,
                questions: (0..rng.range(0, 5))
                    .map(|_| {
                        let stem = *rng.pick(&CARTRIDGE_STEMS);
                        saved_question(&mut rng, stem)
                    })
                    .collect(),
            })
            .collect::<Vec<_>>();
        // Saves from earlier builds may list one question in both lists.
        let progress = SavedQuizProgress {
            answered_questions: listed_questions(&mut rng),
            missed_questions: listed_questions(&mut rng),
            mastery: arbitrary_mastery(&mut rng),
            ..SavedQuizProgress::default()
        };
        let context = format!("case {case}: {batches:?}\n{progress:?}");

        let retired = progress.retired();
        let missed = progress
            .missed_questions
            .iter()
            .map(|question| questions::question_identity(question))
            .filter(|identity| !identity.is_empty())
            .collect::<HashSet<_>>();
        // Model: each identity once, at the level of the batch that first
        // generated it, in generation order.
        let mut seen = HashSet::new();
        let mut first_seen = Vec::new();
        for batch in &batches {
            for question in &batch.questions {
                let identity = questions::question_identity(&question.q);
                if seen.insert(identity.clone()) {
                    first_seen.push((identity, batch.level));
                }
            }
        }
        let mut expected_queue = first_seen
            .iter()
            .filter(|(identity, _)| !retired.contains(identity))
            .cloned()
            .collect::<Vec<_>>();
        expected_queue.sort_by_key(|(_, level)| *level);
        let expected_lessons = first_seen
            .iter()
            .filter(|(identity, _)| retired.contains(identity) || missed.contains(identity))
            .map(|(identity, _)| (identity.clone(), missed.contains(identity)))
            .collect::<Vec<_>>();

        let loaded = questions::cartridge_questions(batches.clone(), progress.clone());

        let queued = loaded
            .questions
            .iter()
            .map(|question| questions::question_identity(&question.question))
            .collect::<Vec<_>>();
        assert!(
            queued.iter().all(|identity| !retired.contains(identity)),
            "a retired question was queued, {context}"
        );
        assert_eq!(
            queued.iter().collect::<HashSet<_>>().len(),
            queued.len(),
            "a question was queued twice, {context}"
        );
        assert_eq!(
            queued,
            expected_queue
                .iter()
                .map(|(identity, _)| identity.clone())
                .collect::<Vec<_>>(),
            "{context}"
        );
        for question in &loaded.questions {
            let identity = questions::question_identity(&question.question);
            assert_eq!(question.review, missed.contains(&identity), "{context}");
        }

        assert_eq!(
            loaded.batch_ends.len(),
            loaded.batch_levels.len(),
            "{context}"
        );
        assert!(
            loaded.batch_ends.windows(2).all(|pair| pair[0] < pair[1]),
            "batch ends must strictly increase, {context}"
        );
        assert!(
            loaded.batch_ends.first().is_none_or(|first| *first > 0),
            "{context}"
        );
        assert_eq!(
            loaded.batch_ends.last().copied().unwrap_or(0),
            loaded.questions.len(),
            "{context}"
        );
        assert!(
            loaded
                .batch_levels
                .windows(2)
                .all(|pair| pair[0] <= pair[1]),
            "{context}"
        );
        let mut start = 0;
        for (end, level) in loaded.batch_ends.iter().zip(&loaded.batch_levels) {
            assert!(
                expected_queue[start..*end]
                    .iter()
                    .all(|(_, queued_level)| queued_level == level),
                "a batch mixes levels, {context}"
            );
            start = *end;
        }

        let lessons = loaded
            .lessons
            .iter()
            .map(|lesson| {
                (
                    questions::question_identity(&lesson.question),
                    lesson.outstanding,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(lessons, expected_lessons, "{context}");
        assert_eq!(loaded.mastery, progress.mastery, "{context}");
    }
}

// ---------------------------------------------------------------------------
// save::update and SaveFile
// ---------------------------------------------------------------------------

const SAVE_KEYS: [&str; 7] = [
    "quiz.progress",
    "ai.question_batches",
    "quest.progress",
    "",
    "a key with spaces",
    "\u{41a}\u{43b}\u{44e}\u{447}",
    "\u{1F600}",
];

fn temporary_cartridge_path(case: usize) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "codequest-property-save-{}-{unique}-{case}",
        std::process::id()
    ))
}

/// Runs one typed `save::update` and checks it against the model: a missing
/// value starts from the default, a readable value is changed and stored, and
/// an unreadable one is reported and left alone.
fn checked_update<T>(
    model: &mut BTreeMap<String, Value>,
    cartridge: &Path,
    key: &str,
    change: impl FnOnce(&mut T),
) where
    T: Serialize + DeserializeOwned + Default + Clone + PartialEq + Debug,
{
    let expected_before = match model.get(key) {
        None => Some(T::default()),
        Some(stored) => serde_json::from_value::<T>(stored.clone()).ok(),
    };
    let result = save::update(cartridge, key, |value: &mut T| {
        let before = value.clone();
        change(value);
        (before, value.clone())
    });
    match expected_before {
        Some(expected) => {
            let (before, after) = result.expect("a readable value updates");
            assert_eq!(before, expected, "{key:?}");
            model.insert(key.to_string(), serde_json::to_value(after).unwrap());
        }
        None => assert_eq!(
            result.unwrap_err(),
            "CARTRIDGE SAVE DATA IS UNREADABLE",
            "{key:?}"
        ),
    }
}

fn arbitrary_json(rng: &mut Rng, depth: usize) -> Value {
    match rng.below(if depth == 0 { 4 } else { 6 }) {
        0 => Value::Null,
        1 => json!(rng.chance(50)),
        2 => json!(rng.next_u64() >> rng.below(64)),
        3 => json!(arbitrary_text(rng, 12)),
        4 => Value::Array(
            (0..rng.below(4))
                .map(|_| arbitrary_json(rng, depth - 1))
                .collect(),
        ),
        _ => Value::Object(
            (0..rng.below(4))
                .map(|_| (arbitrary_text(rng, 6), arbitrary_json(rng, depth - 1)))
                .collect(),
        ),
    }
}

#[test]
fn save_updates_round_trip_under_random_key_and_value_sequences() {
    // Every step touches the disk, so this property runs fewer, longer cases.
    for (case, mut rng) in cases(0x5A7E, 40) {
        let cartridge = temporary_cartridge_path(case);
        let save_path = save::path_for(&cartridge);
        let mut model: BTreeMap<String, Value> = BTreeMap::new();
        for step in 0..rng.range(1, 10) {
            let key = *rng.pick(&SAVE_KEYS);
            let context = format!("case {case} step {step} key {key:?}");
            match rng.below(5) {
                0 => {
                    let by = rng.below(4) as u32;
                    checked_update(&mut model, &cartridge, key, |count: &mut u32| {
                        *count = count.saturating_add(by);
                    });
                }
                1 => {
                    let entry = arbitrary_text(&mut rng, 16);
                    checked_update(&mut model, &cartridge, key, |log: &mut Vec<String>| {
                        log.push(entry);
                    });
                }
                2 => {
                    let text = arbitrary_text(&mut rng, 24);
                    checked_update(&mut model, &cartridge, key, |value: &mut String| {
                        *value = text;
                    });
                }
                3 => {
                    // An update that changes nothing must not rewrite the save.
                    let before = std::fs::read(&save_path).ok();
                    let present = model.contains_key(key);
                    checked_update(&mut model, &cartridge, key, |_: &mut Value| {});
                    if present {
                        assert_eq!(std::fs::read(&save_path).ok(), before, "{context}");
                    }
                }
                _ => {
                    let value = arbitrary_json(&mut rng, 2);
                    let mut snapshot = save::SaveFile::open_or_create(&cartridge).unwrap();
                    snapshot.set(key, &value).unwrap();
                    model.insert(key.to_string(), value);
                }
            }

            let reopened = save::SaveFile::open_or_create(&cartridge).unwrap();
            for key in SAVE_KEYS {
                assert_eq!(
                    reopened.get::<Value>(key).as_ref(),
                    model.get(key),
                    "{context}, reading {key:?}"
                );
            }
            let on_disk: Value =
                serde_json::from_slice(&std::fs::read(&save_path).unwrap()).unwrap();
            assert_eq!(on_disk["schema_version"], json!(1), "{context}");
            assert_eq!(
                on_disk["data"],
                serde_json::to_value(&model).unwrap(),
                "{context}"
            );
        }
        std::fs::remove_file(&save_path).unwrap();
    }
}

// ---------------------------------------------------------------------------
// provenance
// ---------------------------------------------------------------------------

const NOTICE_FRAGMENTS: [&str; 30] = [
    "Copyright",
    "COPYRIGHT",
    "copyright",
    "(c)",
    "(C)",
    "\u{a9}",
    "SPDX-FileCopyrightText:",
    ":",
    " ",
    "  ",
    "\t",
    "2024",
    "1989-2024",
    "Ada Lovelace",
    "Free Software Foundation, Inc.",
    "GNU GENERAL PUBLIC LICENSE",
    "[yyyy]",
    "<year>",
    "{yyyy}",
    "name of copyright owner",
    "notice",
    "holders",
    "and",
    "\u{7}",
    "\u{0}",
    "\u{e9}",
    "\u{1F600}",
    "\r",
    "\n",
    "<ada@example.com>",
];

fn notice_text(rng: &mut Rng) -> String {
    let mut text = String::new();
    for _ in 0..rng.range(0, 40) {
        if rng.chance(75) {
            text.push_str(rng.pick(&NOTICE_FRAGMENTS));
        } else {
            text.push(arbitrary_char(rng));
        }
    }
    text
}

fn is_sanitized(text: &str, max_chars: usize) -> bool {
    text.chars().count() <= max_chars
        && !text.chars().any(char::is_control)
        && text.split_whitespace().collect::<Vec<_>>().join(" ") == text
}

#[test]
fn sanitized_metadata_is_bounded_clean_and_idempotent() {
    for (case, mut rng) in cases(0x5A71, CASES) {
        let text = if rng.chance(50) {
            arbitrary_text(&mut rng, 80)
        } else {
            notice_text(&mut rng)
        };
        let max_chars = rng.range(0, 40);
        let clean = provenance::sanitized_metadata(&text, max_chars);
        let context = format!("case {case}, max {max_chars}: {text:?} -> {clean:?}");
        assert!(is_sanitized(&clean, max_chars), "{context}");
        assert_eq!(
            provenance::sanitized_metadata(&clean, max_chars),
            clean,
            "{context}"
        );
        assert!(
            non_whitespace(&text)
                .chars()
                .filter(|character| !character.is_control())
                .collect::<String>()
                .starts_with(&non_whitespace(&clean)),
            "sanitizing may only drop and cut, {context}"
        );
    }
}

#[test]
fn copyright_notices_are_sanitized_declarations_from_the_text() {
    for (case, mut rng) in cases(0xC0B1, CASES) {
        let text = notice_text(&mut rng);
        let Some(notice) = provenance::copyright_notice_in(&text) else {
            continue;
        };
        let context = format!("case {case}: {text:?} -> {notice:?}");
        assert!(is_sanitized(&notice, 96), "{context}");
        assert!(
            text.lines()
                .any(|line| provenance::sanitized_metadata(line, 96) == notice),
            "the notice must be one of the text's own lines, {context}"
        );
        let lowercase = notice.to_ascii_lowercase();
        assert!(
            lowercase.starts_with("copyright")
                || lowercase.starts_with("spdx-filecopyrighttext:")
                || lowercase.starts_with("(c)")
                || notice.starts_with('\u{a9}'),
            "{context}"
        );
        for template in ["[yyyy]", "<year>", "{yyyy}", "name of copyright owner"] {
            assert!(!lowercase.contains(template), "{context}");
        }
        if text
            .to_ascii_uppercase()
            .contains("GNU GENERAL PUBLIC LICENSE")
        {
            assert!(
                !lowercase.contains("free software foundation"),
                "the license steward was credited, {context}"
            );
        }
    }
}

const COUNT_FRAGMENTS: [&str; 7] = [
    "",
    "1",
    "12",
    "007",
    "18446744073709551615",
    "9223372036854775808",
    "99999999999999999999999",
];

#[test]
fn author_ranking_never_panics_and_credits_each_person_once() {
    for (case, mut rng) in cases(0xA07B, CASES) {
        let mut shortlog = String::new();
        for _ in 0..rng.range(0, 8) {
            shortlog.push_str(&" ".repeat(rng.below(6)));
            shortlog.push_str(rng.pick(&COUNT_FRAGMENTS));
            shortlog.push_str(if rng.chance(80) { "\t" } else { " " });
            shortlog.push_str(&notice_text(&mut rng));
            shortlog.push('\n');
        }
        let limit = rng.range(0, 6);
        let authors = provenance::ranked_authors(&shortlog, limit);
        let context = format!("case {case}: {shortlog:?} -> {authors:?}");
        assert!(authors.len() <= limit, "{context}");
        for (index, name) in authors.iter().enumerate() {
            assert!(!name.is_empty(), "{context}");
            assert!(is_sanitized(name, 64), "{context}");
            assert!(
                authors[..index]
                    .iter()
                    .all(|earlier| !earlier.eq_ignore_ascii_case(name)),
                "one person holds two slots, {context}"
            );
        }
    }
}

const AUTHOR_NAMES: [&str; 5] = [
    "Ada Lovelace",
    "Grace Hopper",
    "Katherine Johnson",
    "Linus",
    "Margaret Hamilton",
];

#[test]
fn author_ranking_sums_every_spelling_and_ranks_by_total() {
    for (case, mut rng) in cases(0x5E0B, CASES) {
        let mut shortlog = String::new();
        // Model: first-seen spelling and saturating total per person.
        let mut expected: Vec<(String, u64)> = Vec::new();
        for _ in 0..rng.range(0, 10) {
            let author = *rng.pick(&AUTHOR_NAMES);
            let name = respelled(&mut rng, author)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            let count = match rng.below(5) {
                0 => u64::MAX - rng.below(3) as u64,
                1 => 1 << 63,
                _ => rng.below(40) as u64,
            };
            let email = if rng.chance(50) {
                " <someone@example.com>"
            } else {
                ""
            };
            shortlog.push_str(&format!("{count:>6}\t{name}{email}\n"));
            match expected
                .iter_mut()
                .find(|(known, _)| known.eq_ignore_ascii_case(&name))
            {
                Some((_, total)) => *total = total.saturating_add(count),
                None => expected.push((name, count)),
            }
        }
        expected.sort_by_key(|(_, total)| std::cmp::Reverse(*total));
        let limit = rng.range(0, 6);
        let expected = expected
            .into_iter()
            .take(limit)
            .map(|(name, _)| name)
            .collect::<Vec<_>>();
        assert_eq!(
            provenance::ranked_authors(&shortlog, limit),
            expected,
            "case {case}: {shortlog:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// engine::wrap_text
// ---------------------------------------------------------------------------

fn non_whitespace(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

#[test]
fn wrapped_lines_never_exceed_the_width_and_keep_every_character() {
    for (case, mut rng) in cases(0x3A77, CASES) {
        let mut text = arbitrary_text(&mut rng, 60);
        if rng.chance(40) {
            // Words longer than the width must be split, not overflow.
            text.push(' ');
            text.push_str(&"W".repeat(rng.range(1, 90)));
            text.push_str(&arbitrary_text(&mut rng, 20));
        }
        // Every caller passes a positive literal width.
        let width = rng.range(1, 40);
        let lines = wrap_text(&text, width);
        let context = format!("case {case}, width {width}: {text:?} -> {lines:?}");
        assert!(!lines.is_empty(), "{context}");
        for line in &lines {
            assert!(line.chars().count() <= width, "overlong line, {context}");
            assert_eq!(
                line.split_whitespace().collect::<Vec<_>>().join(" "),
                *line,
                "{context}"
            );
        }
        if text.split_whitespace().next().is_some() {
            assert!(lines.iter().all(|line| !line.is_empty()), "{context}");
        } else {
            assert_eq!(lines, [String::new()], "{context}");
        }
        assert_eq!(
            non_whitespace(&lines.concat()),
            non_whitespace(&text),
            "wrapping lost or invented text, {context}"
        );
    }
}
