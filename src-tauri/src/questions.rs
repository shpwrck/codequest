//! The question domain: the saved-question model, the acceptance policy that
//! keeps questions conceptual and displayable, provider prompt construction,
//! provider-response parsing, and the learner progress kept in cartridge saves.
//!
//! Generated questions use payload v2, where every choice carries a short
//! rationale and every question names its conceptual lens. Saves written by
//! earlier builds hold plain-string choices without a lens; those legacy
//! questions still load and play, just without explanations. Nothing here
//! depends on Tauri.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::learning::{self, AnswerEvidence, Concept, Lesson, Mastery};
use crate::{engine, save};

pub(crate) const AI_QUESTION_BATCHES_KEY: &str = "ai.question_batches";
pub(crate) const LEGACY_CLAUDE_QUESTION_BATCHES_KEY: &str = "claude.question_batches";
pub(crate) const QUIZ_PROGRESS_KEY: &str = "quiz.progress";
/// Previously generated stems the prompt lists so a provider does not repeat
/// them.
pub(crate) const MAX_AVOIDED_STEMS: usize = 24;
/// Upper bound, in bytes, for a whole generation prompt. Prompts travel on the
/// provider's stdin, so no command-line limit applies; this cap only keeps a
/// request proportionate. It leaves the instructions and learner state room
/// beside a brief that fills its own budget, so a full brief is never trimmed.
pub(crate) const MAX_PROMPT_BYTES: usize = 2 * crate::repo_context::BRIEF_BUDGET;

/// One answer choice. Payload v2 pairs the text with `why`: for the correct
/// choice, why it holds; for a distractor, the misconception it represents.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(untagged)]
pub(crate) enum QChoice {
    /// Legacy saves: the choice text alone.
    Plain(String),
    Explained {
        text: String,
        #[serde(default)]
        why: String,
    },
}

impl QChoice {
    pub(crate) fn text(&self) -> &str {
        match self {
            Self::Plain(text) | Self::Explained { text, .. } => text,
        }
    }

    pub(crate) fn why(&self) -> Option<&str> {
        match self {
            Self::Plain(_) => None,
            Self::Explained { why, .. } => Some(why),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct QQuestion {
    pub(crate) q: String,
    /// The lens this question assesses, as a provider spelled it; see
    /// [`QQuestion::lens`]. Absent from legacy saves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) concept: Option<String>,
    pub(crate) choices: Vec<QChoice>,
    pub(crate) answer: usize,
}

impl QQuestion {
    pub(crate) fn choice_texts(&self) -> Vec<String> {
        self.choices
            .iter()
            .map(|choice| choice.text().to_string())
            .collect()
    }

    pub(crate) fn lens(&self) -> Option<Concept> {
        self.concept.as_deref().and_then(Concept::parse)
    }

    fn has_plain_choices(&self) -> bool {
        self.choices
            .iter()
            .all(|choice| matches!(choice, QChoice::Plain(_)))
    }

    fn has_explained_choices(&self) -> bool {
        self.choices
            .iter()
            .all(|choice| matches!(choice, QChoice::Explained { .. }))
    }

    /// One rationale per choice in choice order, or none for legacy questions.
    pub(crate) fn rationales(&self) -> Vec<String> {
        if self.choices.is_empty() || !self.has_explained_choices() {
            return Vec::new();
        }
        self.choices
            .iter()
            .map(|choice| choice.why().unwrap_or_default().to_string())
            .collect()
    }

    pub(crate) fn quiz_question(&self, review: bool) -> engine::QuizQuestion {
        engine::QuizQuestion {
            question: self.q.clone(),
            choices: self.choice_texts(),
            answer: self.answer,
            concept: self.lens(),
            rationales: self.rationales(),
            review,
        }
    }

    fn lesson(&self, outstanding: bool) -> Lesson {
        let correct = self.choices.get(self.answer);
        Lesson {
            question: self.q.clone(),
            answer: correct.map(QChoice::text).unwrap_or_default().to_string(),
            rationale: correct
                .and_then(QChoice::why)
                .unwrap_or_default()
                .to_string(),
            concept: self.lens(),
            outstanding,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct SavedQuestionBatch {
    pub(crate) level: u32,
    pub(crate) questions: Vec<QQuestion>,
}

/// Learner progress for one cartridge, stored under [`QUIZ_PROGRESS_KEY`].
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub(crate) struct SavedQuizProgress {
    /// Retired questions: answered correctly, or recorded by earlier builds
    /// that retired every committed answer. They are never asked again.
    #[serde(default)]
    pub(crate) answered_questions: Vec<String>,
    /// Questions whose latest attempt was wrong. They stay playable and return
    /// for review until the player redeems them.
    #[serde(default)]
    pub(crate) missed_questions: Vec<String>,
    #[serde(default)]
    pub(crate) mastery: Mastery,
    /// Committed attempts per normalized identity for each missed question,
    /// so its choice rotation continues across launches. Saves from earlier
    /// builds have none; their reviews start at attempt 1.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) question_attempts: BTreeMap<String, u32>,
}

impl SavedQuizProgress {
    /// Applies one committed answer. The latest attempt decides where a
    /// question lives, so the retired and missed lists never share a question.
    pub(crate) fn record(&mut self, evidence: &AnswerEvidence) {
        let identity = question_identity(&evidence.question);
        if identity.is_empty() {
            return;
        }
        let (target, other) = if evidence.correct {
            (&mut self.answered_questions, &mut self.missed_questions)
        } else {
            (&mut self.missed_questions, &mut self.answered_questions)
        };
        other.retain(|question| question_identity(question) != identity);
        if !target
            .iter()
            .any(|question| question_identity(question) == identity)
        {
            target.push(evidence.question.clone());
        }
        if evidence.correct {
            // A retired question is never asked again.
            self.question_attempts.remove(&identity);
        } else {
            // The engine presents a review at attempt 1 or later, even when
            // an earlier build left no count for it.
            let attempts = self.question_attempts.entry(identity).or_default();
            *attempts = (*attempts)
                .max(u32::from(evidence.review))
                .saturating_add(1);
        }
        learning::record_evidence(&mut self.mastery, evidence);
    }

    fn missed(&self) -> HashSet<String> {
        identities(&self.missed_questions)
    }

    /// Identities that must not be asked again.
    pub(crate) fn retired(&self) -> HashSet<String> {
        let missed = self.missed();
        identities(&self.answered_questions)
            .into_iter()
            .filter(|identity| !missed.contains(identity))
            .collect()
    }
}

fn identities(questions: &[String]) -> HashSet<String> {
    questions
        .iter()
        .map(|question| question_identity(question))
        .filter(|identity| !identity.is_empty())
        .collect()
}

pub(crate) fn question_identity(question: &str) -> String {
    question
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_uppercase()
}

/// The canonical save spelling of a lens (its serde name).
fn concept_key(concept: Concept) -> &'static str {
    match concept {
        Concept::Purpose => "purpose",
        Concept::Responsibility => "responsibility",
        Concept::Interaction => "interaction",
        Concept::Invariant => "invariant",
        Concept::Tradeoff => "tradeoff",
    }
}

/// Every lens's save spelling, as the provider schema lists them.
fn lens_names() -> String {
    Concept::ALL
        .iter()
        .map(|concept| concept_key(*concept))
        .collect::<Vec<_>>()
        .join("|")
}

/// Extensions that make a dotted word a file name rather than prose.
const FILE_EXTENSIONS: [&str; 60] = [
    "RS", "JS", "MJS", "CJS", "JSX", "TS", "TSX", "MTS", "CTS", "PY", "PYI", "GO", "RB", "JAVA",
    "KT", "KTS", "SCALA", "SWIFT", "C", "H", "CC", "CPP", "CXX", "HPP", "HH", "CS", "FS", "PHP",
    "PL", "LUA", "DART", "EX", "EXS", "ERL", "HS", "SH", "BASH", "ZSH", "PS1", "BAT", "CSS",
    "SCSS", "LESS", "HTML", "HTM", "VUE", "SVELTE", "MD", "RST", "TXT", "TOML", "JSON", "YAML",
    "YML", "XML", "INI", "LOCK", "SQL", "PROTO", "SAV",
];
/// Dotted technology names that read like file names but name a concept.
const TECHNOLOGY_NAMES: [&str; 8] = [
    "NODE.JS",
    "VUE.JS",
    "NEXT.JS",
    "NUXT.JS",
    "THREE.JS",
    "D3.JS",
    "CHART.JS",
    "EXPRESS.JS",
];
/// Configuration dotfiles that only name a location.
const DOTFILES: [&str; 12] = [
    "GITIGNORE",
    "GITATTRIBUTES",
    "GITMODULES",
    "GITHUB",
    "ENV",
    "EDITORCONFIG",
    "NPMRC",
    "PRETTIERRC",
    "ESLINTRC",
    "DOCKERIGNORE",
    "VSCODE",
    "CARGO",
];
/// Directory names that turn a single slash into a path (`SRC/ENGINE`), while
/// ordinary pairs such as `READ/WRITE` remain prose.
const DIRECTORY_NAMES: [&str; 30] = [
    "SRC",
    "SRC-TAURI",
    "LIB",
    "LIBS",
    "BIN",
    "DOC",
    "DOCS",
    "TEST",
    "TESTS",
    "SPEC",
    "APP",
    "APPS",
    "PKG",
    "CMD",
    "INTERNAL",
    "SCRIPTS",
    "PUBLIC",
    "ASSETS",
    "DIST",
    "BUILD",
    "TARGET",
    "VENDOR",
    "NODE_MODULES",
    "CONFIG",
    "UTILS",
    "COMPONENTS",
    "PACKAGES",
    "CRATES",
    "EXAMPLES",
    "TOOLS",
];
/// Phrases that ask where code lives, how big it is, or what the repository
/// looked like at some moment. They are matched as whole words, so ordinary
/// nouns (a save FILE, a hot PATH, the LATEST attempt, a transaction COMMIT)
/// stay available to conceptual questions.
const TRIVIA_PHRASES: [&str; 95] = [
    // Locations and structure.
    "WHERE DOES",
    "WHERE DO",
    "WHERE IS",
    "WHERE ARE",
    "LIVES IN",
    "LIVE IN",
    "LOCATED IN",
    "FILE NAME",
    "FILE NAMES",
    "FILENAME",
    "FILENAMES",
    "FILE PATH",
    "FILE PATHS",
    "FILE EXTENSION",
    "FILE EXTENSIONS",
    "FILE STRUCTURE",
    "FILE LAYOUT",
    "DIRECTORY STRUCTURE",
    "DIRECTORY LAYOUT",
    "FOLDER STRUCTURE",
    "FOLDER LAYOUT",
    "REPOSITORY STRUCTURE",
    "REPOSITORY LAYOUT",
    "REPO STRUCTURE",
    "REPO LAYOUT",
    "PROJECT STRUCTURE",
    "PROJECT LAYOUT",
    "SOURCE TREE",
    "ROOT DIRECTORY",
    "ROOT FOLDER",
    "SUBDIRECTORY",
    "SUBDIRECTORIES",
    "SUBFOLDER",
    "SUBFOLDERS",
    "README",
    // Counts and sizes.
    "HOW MANY",
    "HOW LARGE",
    "HOW BIG",
    "HOW LONG IS",
    "LINES OF CODE",
    "LINE COUNT",
    "LINE NUMBER",
    "LINE NUMBERS",
    "FILE SIZE",
    "FILE SIZES",
    "NUMBER OF FILES",
    "NUMBER OF LINES",
    "KILOBYTES",
    "MEGABYTES",
    // Version-control history.
    "COMMITS",
    "COMMIT MESSAGE",
    "COMMIT MESSAGES",
    "COMMIT HASH",
    "COMMIT HISTORY",
    "COMMIT LOG",
    "WHICH COMMIT",
    "WHAT COMMIT",
    "FIRST COMMIT",
    "LAST COMMIT",
    "LATEST COMMIT",
    "RECENT COMMIT",
    "INITIAL COMMIT",
    "COMMITTER",
    "COMMITTERS",
    "GIT HISTORY",
    "GIT LOG",
    "GIT BLAME",
    "WHICH BRANCH",
    "WHAT BRANCH",
    "CURRENT BRANCH",
    "MAIN BRANCH",
    "MASTER BRANCH",
    "DEFAULT BRANCH",
    "FEATURE BRANCH",
    "BRANCH NAME",
    "PULL REQUEST",
    "PULL REQUESTS",
    // People.
    "AUTHOR",
    "AUTHORS",
    "CONTRIBUTOR",
    "CONTRIBUTORS",
    "MAINTAINER",
    "MAINTAINERS",
    "WHO WROTE",
    "WHO CREATED",
    // Recency and time.
    "MOST RECENT",
    "MOST RECENTLY",
    "RECENTLY",
    "LATEST VERSION",
    "LATEST RELEASE",
    "LAST MODIFIED",
    "LAST UPDATED",
    "WHEN WAS",
    "WHEN WERE",
    "WHAT YEAR",
];
/// Nouns that make "WHICH ..." or "WHAT ..." a location question when they
/// follow within two words, as in WHICH SOURCE FILE or WHAT CONFIG FOLDER.
const LOCATION_NOUNS: [&str; 10] = [
    "FILE",
    "FILES",
    "FILENAME",
    "DIRECTORY",
    "DIRECTORIES",
    "FOLDER",
    "FOLDERS",
    "PATH",
    "PATHS",
    "EXTENSION",
];

/// Strips surrounding quotes, brackets, and sentence punctuation from one
/// whitespace-separated word while keeping a leading dot (`.RS`, `./SRC`).
fn word_core(word: &str) -> &str {
    const WRAPPERS: &[char] = &[
        '"', '\'', '`', '(', ')', '[', ']', '{', '}', '<', '>', ',', ';', ':', '!', '?', '*',
    ];
    let mut core = word.trim_start_matches(WRAPPERS);
    loop {
        let trimmed = core.trim_end_matches(WRAPPERS).trim_end_matches('.');
        if trimmed.len() == core.len() {
            return core;
        }
        core = trimmed;
    }
}

fn is_file_name(word: &str) -> bool {
    if TECHNOLOGY_NAMES.contains(&word) {
        return false;
    }
    let Some((stem, extension)) = word.rsplit_once('.') else {
        return false;
    };
    FILE_EXTENSIONS.contains(&extension)
        && (stem.is_empty()
            || (stem.chars().any(|c| c.is_ascii_alphanumeric())
                && stem
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))))
}

fn is_path(word: &str) -> bool {
    if word.contains('\\') || word.contains("://") {
        return true;
    }
    if !word.contains('/') {
        return false;
    }
    let segments = word.split('/').collect::<Vec<_>>();
    segments.len() > 2
        || segments.iter().any(|segment| {
            segment.is_empty()
                || matches!(*segment, "." | ".." | "~")
                || is_file_name(segment)
                || DIRECTORY_NAMES.contains(segment)
        })
}

/// Whether one word names a path, a file, a bare file extension, or a
/// configuration dotfile.
pub(crate) fn is_location_word(word: &str) -> bool {
    let upper = word.to_ascii_uppercase();
    let core = word_core(&upper);
    core.chars()
        .any(|character| character.is_ascii_alphanumeric())
        && (is_path(core)
            || is_file_name(core)
            || core
                .strip_prefix('.')
                .is_some_and(|name| DOTFILES.contains(&name)))
}

/// Whether `text` cites a location: a path, a file name, or an extension.
fn cites_location(text: &str) -> bool {
    text.split_whitespace().any(is_location_word)
}

/// Year, dotted release version, or abbreviated commit hash.
fn is_moment_word(word: &str) -> bool {
    let core = word_core(word);
    let year = core.len() == 4
        && core
            .parse::<u32>()
            .is_ok_and(|year| (1990..=2039).contains(&year));
    let version = {
        let digits = core.strip_prefix('V').unwrap_or(core);
        let parts = digits.split('.').collect::<Vec<_>>();
        parts.len() >= 3
            && parts
                .iter()
                .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
    };
    let hash = (7..=40).contains(&core.len())
        && core.bytes().all(|b| b.is_ascii_hexdigit())
        && core.bytes().any(|b| b.is_ascii_digit())
        && core.bytes().any(|b| b.is_ascii_alphabetic());
    year || version || hash
}

/// Whether `text` asks about, or answers with, locations or state-in-time
/// facts instead of concepts.
fn cites_trivia(text: &str) -> bool {
    let upper = text.to_ascii_uppercase();
    if cites_location(&upper) || upper.split_whitespace().any(is_moment_word) {
        return true;
    }
    let words = upper
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    let padded = format!(" {} ", words.join(" "));
    TRIVIA_PHRASES
        .iter()
        .any(|phrase| padded.contains(&format!(" {phrase} ")))
        || words.iter().enumerate().any(|(index, word)| {
            matches!(*word, "WHICH" | "WHAT")
                && words[index + 1..]
                    .iter()
                    .take(2)
                    .any(|next| LOCATION_NOUNS.contains(next))
        })
}

fn renderable(text: &str) -> bool {
    text.chars()
        .all(|character| (' '..='~').contains(&character))
}

/// Question and choice text fit the quiz layout and test concepts.
fn is_displayable_and_conceptual(question: &QQuestion) -> bool {
    let choices = question.choice_texts();
    engine::quiz_question_fits(&question.q, &choices, question.answer)
        && !std::iter::once(&question.q)
            .chain(&choices)
            .any(|text| cites_trivia(text))
}

/// Payload v2: a lens the learning model knows and a fitting, location-free
/// rationale for every choice.
fn has_complete_rationales(question: &QQuestion) -> bool {
    question.lens().is_some()
        && question.has_explained_choices()
        && question.choices.iter().all(|choice| {
            choice
                .why()
                .is_some_and(|why| learning::rationale_fits(why) && !cites_location(why))
        })
}

/// Policy for questions read back from a save: complete v2 questions, or
/// legacy questions whose choices are plain strings.
pub(crate) fn question_is_acceptable(question: &QQuestion) -> bool {
    let legacy =
        question.has_plain_choices() && (question.concept.is_none() || question.lens().is_some());
    is_displayable_and_conceptual(question) && (legacy || has_complete_rationales(question))
}

/// Policy for newly generated questions: payload v2 is required and every
/// string must render in the handheld font.
pub(crate) fn generated_question_is_acceptable(question: &QQuestion) -> bool {
    is_displayable_and_conceptual(question)
        && has_complete_rationales(question)
        && renderable(&question.q)
        && question
            .choices
            .iter()
            .all(|choice| renderable(choice.text()) && choice.why().is_some_and(renderable))
}

/// Keeps each acceptable generated question once; failing questions are
/// dropped individually so the valid ones in a mixed batch survive.
pub(crate) fn retain_acceptable_questions(questions: Vec<QQuestion>) -> Vec<QQuestion> {
    let mut seen = HashSet::new();
    questions
        .into_iter()
        .filter(generated_question_is_acceptable)
        .filter(|question| seen.insert(question_identity(&question.q)))
        .collect()
}

pub(crate) fn accepted_question_batch(
    questions: Vec<QQuestion>,
    expected_count: usize,
) -> Option<Vec<QQuestion>> {
    let valid = retain_acceptable_questions(questions)
        .into_iter()
        .take(expected_count)
        .collect::<Vec<_>>();
    (!valid.is_empty()).then_some(valid)
}

/// A text field of a question, named by its JSON path in the provider schema
/// (choices counted from 0), so a repair prompt can point at it exactly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Field {
    Question,
    Choice(usize),
    Why(usize),
}

impl std::fmt::Display for Field {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Question => write!(formatter, "\"q\""),
            Self::Choice(index) => write!(formatter, "choices[{index}].text"),
            Self::Why(index) => write!(formatter, "choices[{index}].why"),
        }
    }
}

/// One specific way a generated question fails
/// [`generated_question_is_acceptable`], with the measurements a repair
/// needs. Character counts are of the normalized text the display draws.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Violation {
    /// The question wraps into more than the quiz's question rows.
    QuestionTooLong { chars: usize, lines: usize },
    /// A choice is wider than one choice row.
    ChoiceTooLong { choice: usize, chars: usize },
    /// A choice has no rationale (a plain string, or an empty `why`).
    WhyMissing { choice: usize },
    /// A rationale wraps into more than the lesson panel's rows.
    WhyTooLong {
        choice: usize,
        chars: usize,
        lines: usize,
    },
    /// The lens is absent or names no lens [`Concept::parse`] knows. Lens
    /// spellings it can map are canonicalized before any check runs.
    UnknownLens { given: Option<String> },
    /// Characters the handheld font cannot draw, in order of appearance.
    NotAscii { field: Field, characters: Vec<char> },
    /// The field asks about or answers with a location or a state-in-time
    /// fact. The question itself is wrong, so it is never repaired.
    Trivia { field: Field },
    /// The question is structurally broken (choice count, answer index, empty
    /// or repeated text). Fixing that means writing a new question.
    Malformed(&'static str),
}

impl Violation {
    /// Whether rewording fields in place can fix this without changing what
    /// the question asks.
    pub(crate) fn is_mechanical(&self) -> bool {
        !matches!(self, Self::Trivia { .. } | Self::Malformed(_))
    }

    /// One instruction line for a repair prompt.
    pub(crate) fn describe(&self) -> String {
        match self {
            Self::QuestionTooLong { chars, lines } => format!(
                "{} is {chars} characters and wraps into {lines} lines of {}; the limit is {} lines (about 100 characters).",
                Field::Question,
                engine::QUIZ_QUESTION_COLUMNS,
                engine::QUIZ_QUESTION_ROWS,
            ),
            Self::ChoiceTooLong { choice, chars } => format!(
                "{} is {chars} characters; the limit is {}.",
                Field::Choice(*choice),
                engine::QUIZ_CHOICE_CHARS
            ),
            Self::WhyMissing { choice } => format!(
                "{} is missing; write one of at most 90 characters.",
                Field::Why(*choice)
            ),
            Self::WhyTooLong {
                choice,
                chars,
                lines,
            } => format!(
                "{} is {chars} characters and wraps into {lines} lines of {}; the limit is {} lines (about 90 characters).",
                Field::Why(*choice),
                learning::RATIONALE_COLUMNS,
                learning::RATIONALE_ROWS,
            ),
            Self::UnknownLens { given } => {
                let lenses = lens_names();
                match given
                    .as_deref()
                    .map(str::trim)
                    .filter(|lens| !lens.is_empty())
                {
                    Some(lens) => format!(
                        "\"concept\" is \"{lens}\", which is not a lens; use exactly one of {lenses}."
                    ),
                    None => format!("\"concept\" is missing; use exactly one of {lenses}."),
                }
            }
            Self::NotAscii { field, characters } => format!(
                "{field} contains {}, which the display cannot draw; use plain ASCII.",
                characters
                    .iter()
                    .map(|character| format!("'{character}' (U+{:04X})", u32::from(*character)))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Trivia { field } => {
                format!("{field} cites a location or a state-in-time fact.")
            }
            Self::Malformed(reason) => format!("the question is malformed: {reason}."),
        }
    }
}

/// Non-ASCII characters in `text`, each once, in order of appearance.
fn unrenderable_characters(text: &str) -> Vec<char> {
    let mut characters = Vec::new();
    for character in text.chars() {
        if !(' '..='~').contains(&character) && !characters.contains(&character) {
            characters.push(character);
        }
    }
    characters
}

/// Every reason [`generated_question_is_acceptable`] rejects `question`; the
/// list is empty exactly when the question is accepted. It measures with the
/// same primitives as the acceptance checks and never relaxes them.
pub(crate) fn question_violations(question: &QQuestion) -> Vec<Violation> {
    let mut violations = Vec::new();
    let choices = question.choice_texts();

    // The quiz layout (engine::quiz_question_fits).
    if question.q.trim().is_empty() {
        violations.push(Violation::Malformed("THE QUESTION IS EMPTY"));
    }
    let lines = engine::wrap_text(&question.q, engine::QUIZ_QUESTION_COLUMNS).len();
    if lines > engine::QUIZ_QUESTION_ROWS {
        violations.push(Violation::QuestionTooLong {
            chars: question.q.chars().count(),
            lines,
        });
    }
    if choices.len() != 4 {
        violations.push(Violation::Malformed("A QUESTION NEEDS EXACTLY 4 CHOICES"));
    }
    if question.answer >= choices.len() {
        violations.push(Violation::Malformed("THE ANSWER IS NOT A CHOICE INDEX"));
    }
    let distinct = choices
        .iter()
        .map(|choice| choice.trim().to_ascii_uppercase())
        .collect::<HashSet<_>>();
    if distinct.len() != choices.len() {
        violations.push(Violation::Malformed("TWO CHOICES REPEAT EACH OTHER"));
    }
    for (index, text) in choices.iter().enumerate() {
        if text.trim().is_empty() {
            violations.push(Violation::Malformed("A CHOICE IS EMPTY"));
        }
        let chars = text.chars().count();
        if chars > engine::QUIZ_CHOICE_CHARS {
            violations.push(Violation::ChoiceTooLong {
                choice: index,
                chars,
            });
        }
    }

    // Concepts only (cites_trivia, cites_location).
    if cites_trivia(&question.q) {
        violations.push(Violation::Trivia {
            field: Field::Question,
        });
    }
    for (index, text) in choices.iter().enumerate() {
        if cites_trivia(text) {
            violations.push(Violation::Trivia {
                field: Field::Choice(index),
            });
        }
    }

    // Payload v2 (has_complete_rationales).
    if question.lens().is_none() {
        violations.push(Violation::UnknownLens {
            given: question.concept.clone(),
        });
    }
    for (index, choice) in question.choices.iter().enumerate() {
        let Some(why) = choice.why() else {
            violations.push(Violation::WhyMissing { choice: index });
            continue;
        };
        if why.trim().is_empty() {
            violations.push(Violation::WhyMissing { choice: index });
        } else if !learning::rationale_fits(why) {
            violations.push(Violation::WhyTooLong {
                choice: index,
                chars: why.chars().count(),
                lines: engine::wrap_text(why, learning::RATIONALE_COLUMNS).len(),
            });
        }
        if cites_location(why) {
            violations.push(Violation::Trivia {
                field: Field::Why(index),
            });
        }
    }

    // The handheld font (renderable).
    let texts = std::iter::once((Field::Question, question.q.as_str())).chain(
        question
            .choices
            .iter()
            .enumerate()
            .flat_map(|(index, choice)| {
                std::iter::once((Field::Choice(index), choice.text()))
                    .chain(choice.why().map(|why| (Field::Why(index), why)))
            }),
    );
    for (field, text) in texts {
        let characters = unrenderable_characters(text);
        if !characters.is_empty() {
            violations.push(Violation::NotAscii { field, characters });
        }
    }
    violations
}

/// Whether one repair call could turn `question` into an accepted question:
/// it fails, and only for mechanical reasons.
pub(crate) fn is_repairable(question: &QQuestion) -> bool {
    let violations = question_violations(question);
    !violations.is_empty() && violations.iter().all(Violation::is_mechanical)
}

/// Folds typographic punctuation into ASCII, collapses whitespace, and
/// uppercases, matching what the handheld font can draw.
fn display_text(text: &str) -> String {
    let mut ascii = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '\u{2018}' | '\u{2019}' | '\u{201B}' | '\u{2032}' => ascii.push('\''),
            '\u{201C}' | '\u{201D}' | '\u{2033}' => ascii.push('"'),
            '\u{2010}'..='\u{2015}' | '\u{2212}' => ascii.push('-'),
            '\u{2026}' => ascii.push_str("..."),
            other => ascii.push(other),
        }
    }
    ascii
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_uppercase()
}

fn normalized_question(question: QQuestion) -> QQuestion {
    let concept = question.concept.map(|concept| {
        Concept::parse(&concept).map_or_else(
            || concept.trim().to_string(),
            |lens| concept_key(lens).to_string(),
        )
    });
    QQuestion {
        q: display_text(&question.q),
        concept,
        choices: question
            .choices
            .into_iter()
            .map(|choice| match choice {
                QChoice::Plain(text) => QChoice::Plain(display_text(&text)),
                QChoice::Explained { text, why } => QChoice::Explained {
                    text: display_text(&text),
                    why: display_text(&why),
                },
            })
            .collect(),
        answer: question.answer,
    }
}

/// Reads the complete JSON values of an array that starts right after its
/// opening bracket, stopping at the first malformed or truncated element so a
/// response cut off mid-array still yields its finished questions.
fn leading_array_values(after_bracket: &str) -> Vec<serde_json::Value> {
    let mut values = Vec::new();
    let mut rest = after_bracket;
    loop {
        rest = rest.trim_start();
        if rest.is_empty() || rest.starts_with(']') {
            return values;
        }
        let mut stream = serde_json::Deserializer::from_str(rest).into_iter::<serde_json::Value>();
        match stream.next() {
            Some(Ok(value)) => values.push(value),
            _ => return values,
        }
        rest = rest[stream.byte_offset()..].trim_start();
        match rest.strip_prefix(',') {
            Some(after_comma) => rest = after_comma,
            None => return values,
        }
    }
}

/// Finds the question array in free-form provider text: the first bracket
/// whose elements are question objects, ignoring prose and code fences.
fn question_values(response: &str) -> Result<Vec<serde_json::Value>, String> {
    let mut saw_bracket = false;
    for (index, _) in response.match_indices('[') {
        saw_bracket = true;
        let values = leading_array_values(&response[index + 1..]);
        if values.iter().any(|value| value.get("q").is_some()) {
            return Ok(values);
        }
    }
    Err(if saw_bracket {
        "UNPARSEABLE QUESTIONS"
    } else {
        "NO JSON IN RESPONSE"
    }
    .to_string())
}

/// One provider response, parsed and normalized: the questions the policy
/// accepts, and the questions it rejected.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct GeneratedBatch {
    /// At most the requested count of acceptable questions, each once.
    pub(crate) accepted: Vec<QQuestion>,
    /// Readable questions that failed [`generated_question_is_acceptable`],
    /// in response order. Objects that are not questions at all are dropped.
    pub(crate) rejected: Vec<QQuestion>,
}

impl GeneratedBatch {
    /// The playable questions, or an error when none survived.
    pub(crate) fn into_questions(self) -> Result<Vec<QQuestion>, String> {
        if self.accepted.is_empty() {
            Err("INCOMPLETE OR INVALID QUESTIONS".to_string())
        } else {
            Ok(self.accepted)
        }
    }

    /// The one repair call worth making for this batch: none when it is
    /// already full or no rejection is repairable. At most `count` questions
    /// are sent, each once, and none that repeats an accepted question.
    pub(crate) fn repair_request(&self, count: usize) -> Option<RepairRequest> {
        if self.accepted.len() >= count {
            return None;
        }
        let mut seen = self
            .accepted
            .iter()
            .map(|question| question_identity(&question.q))
            .collect::<HashSet<_>>();
        let originals = self
            .rejected
            .iter()
            .filter(|question| is_repairable(question))
            .filter(|question| seen.insert(question_identity(&question.q)))
            .take(count)
            .cloned()
            .collect::<Vec<_>>();
        (!originals.is_empty()).then(|| RepairRequest {
            prompt: repair_prompt(&originals),
            originals,
        })
    }

    /// Adds the acceptable repairs in `reply` to the accepted questions. A
    /// repair counts only when it echoes the `id` of a question `request`
    /// sent and keeps that question's answer index; each sent question adds at
    /// most one repair; the batch never exceeds `count` or repeats a question.
    /// Returns how many repairs were added.
    pub(crate) fn merge_repairs(
        &mut self,
        request: &RepairRequest,
        reply: &str,
        count: usize,
    ) -> usize {
        let Ok(values) = question_values(reply) else {
            return 0;
        };
        let mut seen = self
            .accepted
            .iter()
            .map(|question| question_identity(&question.q))
            .collect::<HashSet<_>>();
        let mut repaired = HashSet::new();
        let before = self.accepted.len();
        for value in values {
            if self.accepted.len() >= count {
                break;
            }
            let Some(id) = value
                .get("id")
                .and_then(serde_json::Value::as_u64)
                .and_then(|id| usize::try_from(id).ok())
            else {
                continue;
            };
            let Some(original) = id
                .checked_sub(1)
                .and_then(|index| request.originals.get(index))
            else {
                continue;
            };
            let Ok(question) = serde_json::from_value::<QQuestion>(value) else {
                continue;
            };
            let question = normalized_question(question);
            if question.answer == original.answer
                && generated_question_is_acceptable(&question)
                && !repaired.contains(&id)
                && seen.insert(question_identity(&question.q))
            {
                repaired.insert(id);
                self.accepted.push(question);
            }
        }
        self.accepted.len() - before
    }
}

/// Parses and normalizes one provider response, keeping at most `count`
/// acceptable questions and every rejected one. Errors only when the response
/// holds no question array at all.
pub(crate) fn parse_generated_batch(
    response: &str,
    count: usize,
) -> Result<GeneratedBatch, String> {
    let (accepted, rejected) = question_values(response)?
        .into_iter()
        .filter_map(|value| serde_json::from_value::<QQuestion>(value).ok())
        .map(normalized_question)
        .partition(generated_question_is_acceptable);
    Ok(GeneratedBatch {
        accepted: accepted_question_batch(accepted, count).unwrap_or_default(),
        rejected,
    })
}

/// A second chance for questions rejected on mechanics alone.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RepairRequest {
    /// The questions sent for repair; the prompt numbers them from 1 as `id`.
    pub(crate) originals: Vec<QQuestion>,
    pub(crate) prompt: String,
}

/// The repair schema: a question in the generation schema plus the `id` the
/// reply must echo. Choices always appear as objects, so a legacy plain choice
/// shows where its missing rationale goes.
#[derive(Serialize)]
struct RepairItem<'a> {
    id: usize,
    q: &'a str,
    concept: &'a str,
    choices: Vec<RepairChoice<'a>>,
    answer: usize,
}

#[derive(Serialize)]
struct RepairChoice<'a> {
    text: &'a str,
    why: &'a str,
}

/// A precise repair prompt for `originals`: for each question, exactly which
/// fields break which limit and by how much, then the question itself to keep
/// verbatim otherwise, in the generation schema plus an `id`.
pub(crate) fn repair_prompt(originals: &[QQuestion]) -> String {
    let questions = originals
        .iter()
        .enumerate()
        .map(|(index, question)| {
            let id = index + 1;
            let fixes = question_violations(question)
                .iter()
                .map(|violation| format!("- {}", violation.describe()))
                .collect::<Vec<_>>()
                .join("\n");
            let item = RepairItem {
                id,
                q: &question.q,
                concept: question.concept.as_deref().unwrap_or_default(),
                choices: question
                    .choices
                    .iter()
                    .map(|choice| RepairChoice {
                        text: choice.text(),
                        why: choice.why().unwrap_or_default(),
                    })
                    .collect(),
                answer: question.answer,
            };
            let json = serde_json::to_string(&item).unwrap_or_default();
            format!("QUESTION id {id}\nFIX:\n{fixes}\nQUESTION JSON:\n{json}")
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    let lenses = lens_names();
    format!(
        "You wrote quiz questions for a retro handheld game, and the display rejected the ones below. Each one fails only on mechanics. Fix ONLY the fields listed under FIX. Keep every other field exactly as written: the same question meaning, the same concept, the same choices in the same order, and the same answer index.\n\nDISPLAY LIMITS (hard: anything longer is discarded, not shortened):\n- \"q\" must wrap into at most {question_rows} lines of {question_columns} characters (about 100 characters).\n- Each choice \"text\" must be at most {choice_chars} characters; aim for 2 to 5 words and at most 28. Put nuance in its why, never in the choice text.\n- Each \"why\" must wrap into at most {why_rows} lines of {why_columns} characters (about 90 characters).\n- \"concept\" is exactly one of {lenses}.\n- Plain ASCII only: no curly quotes, long dashes, arrows, or accented letters.\n\nShorten by rewording, not by truncating words or sentences. Never add file names, paths, versions, or dates. Before answering, count the characters of every field you rewrite.\n\n{questions}\n\nRespond with ONLY a JSON array of the fixed questions, no prose and no code fences, one object per question above, each keeping its \"id\":\n[{{\"id\":N,\"q\":\"...\",\"concept\":\"...\",\"choices\":[{{\"text\":\"...\",\"why\":\"...\"}},{{\"text\":\"...\",\"why\":\"...\"}},{{\"text\":\"...\",\"why\":\"...\"}},{{\"text\":\"...\",\"why\":\"...\"}}],\"answer\":N}}]",
        question_rows = engine::QUIZ_QUESTION_ROWS,
        question_columns = engine::QUIZ_QUESTION_COLUMNS,
        choice_chars = engine::QUIZ_CHOICE_CHARS,
        why_rows = learning::RATIONALE_ROWS,
        why_columns = learning::RATIONALE_COLUMNS,
    )
}

/// Parses one saved batch leniently: a question this build cannot read or no
/// longer accepts is skipped without discarding its batch mates.
fn saved_batch(value: serde_json::Value) -> Option<SavedQuestionBatch> {
    #[derive(Deserialize)]
    struct StoredBatch {
        level: u32,
        #[serde(default)]
        questions: Vec<serde_json::Value>,
    }
    let stored = serde_json::from_value::<StoredBatch>(value).ok()?;
    let questions = stored
        .questions
        .into_iter()
        .filter_map(|question| serde_json::from_value::<QQuestion>(question).ok())
        .filter(question_is_acceptable)
        .collect::<Vec<_>>();
    (stored.level > 0 && !questions.is_empty()).then_some(SavedQuestionBatch {
        level: stored.level,
        questions,
    })
}

/// Saved batches, oldest first: batches from the legacy Claude-only key
/// predate every batch under the provider-neutral key.
pub(crate) fn load_saved_question_batches(path: &Path) -> Result<Vec<SavedQuestionBatch>, String> {
    Ok(saved_question_batches(&save::SaveFile::open_or_create(
        path,
    )?))
}

fn saved_question_batches(save: &save::SaveFile) -> Vec<SavedQuestionBatch> {
    [LEGACY_CLAUDE_QUESTION_BATCHES_KEY, AI_QUESTION_BATCHES_KEY]
        .into_iter()
        .flat_map(|key| save.get::<Vec<serde_json::Value>>(key).unwrap_or_default())
        .filter_map(saved_batch)
        .collect()
}

pub(crate) fn persist_ai_question_batch(
    path: &Path,
    level: u32,
    questions: &[QQuestion],
) -> Result<(), String> {
    if level == 0 || questions.is_empty() || !questions.iter().all(generated_question_is_acceptable)
    {
        return Err("INVALID AI QUESTION BATCH".to_string());
    }
    let batch = serde_json::to_value(SavedQuestionBatch {
        level,
        questions: questions.to_vec(),
    })
    .map_err(|_| "COULD NOT SERIALIZE CARTRIDGE SAVE".to_string())?;
    // Stored batches stay raw JSON here, so entries this build cannot parse
    // are carried forward instead of being dropped by the append.
    save::update(
        path,
        AI_QUESTION_BATCHES_KEY,
        |batches: &mut Vec<serde_json::Value>| batches.push(batch),
    )
}

pub(crate) fn load_quiz_progress(path: &Path) -> Result<SavedQuizProgress, String> {
    Ok(quiz_progress(&save::SaveFile::open_or_create(path)?))
}

fn quiz_progress(save: &save::SaveFile) -> SavedQuizProgress {
    save.get::<SavedQuizProgress>(QUIZ_PROGRESS_KEY)
        .unwrap_or_default()
}

/// Records one committed answer: progress lists and lens mastery update in a
/// single atomic save update.
pub(crate) fn persist_answer_evidence(
    path: &Path,
    evidence: &AnswerEvidence,
) -> Result<(), String> {
    save::update(
        path,
        QUIZ_PROGRESS_KEY,
        |progress: &mut SavedQuizProgress| progress.record(evidence),
    )
}

/// Everything a cartridge load hands the engine from the save.
#[derive(Debug, Default)]
pub(crate) struct CartridgeQuestions {
    pub(crate) questions: Vec<engine::QuizQuestion>,
    pub(crate) batch_ends: Vec<usize>,
    /// The generation level of each batch, parallel to `batch_ends`.
    pub(crate) batch_levels: Vec<u32>,
    pub(crate) lessons: Vec<Lesson>,
    pub(crate) mastery: Mastery,
    /// Committed attempts per normalized identity of each missed question.
    pub(crate) attempts: HashMap<String, u32>,
}

/// Builds the playable queue and lesson journal from saved batches (oldest
/// first). Retired questions leave the queue; missed questions stay in it as
/// reviews. Every recorded question becomes one journal lesson, in the order
/// it was generated. A question repeated across batches is queued and
/// journaled once. The queue plays lower-level batches first (stable within a
/// level), so a new Initiate run never opens on leftover Oracle-bound questions.
pub(crate) fn cartridge_questions(
    batches: Vec<SavedQuestionBatch>,
    progress: SavedQuizProgress,
) -> CartridgeQuestions {
    let retired = progress.retired();
    let missed = progress.missed();
    let mut seen = HashSet::new();
    let mut loaded = CartridgeQuestions {
        mastery: progress.mastery,
        attempts: progress.question_attempts.into_iter().collect(),
        ..CartridgeQuestions::default()
    };
    let mut queued = Vec::new();
    for batch in batches {
        let mut playable = Vec::new();
        for question in batch.questions {
            let identity = question_identity(&question.q);
            if !seen.insert(identity.clone()) {
                continue;
            }
            let outstanding = missed.contains(&identity);
            if outstanding || retired.contains(&identity) {
                loaded.lessons.push(question.lesson(outstanding));
            }
            if !retired.contains(&identity) {
                playable.push(question.quiz_question(outstanding));
            }
        }
        if !playable.is_empty() {
            queued.push((batch.level, playable));
        }
    }
    queued.sort_by_key(|(level, _)| *level);
    for (level, playable) in queued {
        loaded.questions.extend(playable);
        loaded.batch_ends.push(loaded.questions.len());
        loaded.batch_levels.push(level);
    }
    loaded
}

/// Everything a cartridge load needs from its save, from one read of it.
pub(crate) fn load_cartridge_questions(path: &Path) -> Result<CartridgeQuestions, String> {
    let save = save::SaveFile::open_or_create(path)?;
    Ok(cartridge_questions(
        saved_question_batches(&save),
        quiz_progress(&save),
    ))
}

/// Maps a freshly generated batch to engine questions, leaving out any the
/// player already retired. A regenerated stem the player missed arrives as a
/// review, as [`cartridge_questions`] queues it, so answering it counts as a
/// redemption.
pub(crate) fn playable_new_questions(
    questions: Vec<QQuestion>,
    progress: &SavedQuizProgress,
) -> Vec<engine::QuizQuestion> {
    let retired = progress.retired();
    let missed = progress.missed();
    questions
        .into_iter()
        .filter_map(|question| {
            let identity = question_identity(&question.q);
            (!retired.contains(&identity))
                .then(|| question.quiz_question(missed.contains(&identity)))
        })
        .collect()
}

/// What the next batch should know about the player.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct LearnerState {
    /// The lens with the most misses relative to its evidence.
    pub(crate) weakest: Option<Concept>,
    /// Normalized stems of previously generated questions, newest first.
    pub(crate) asked: Vec<String>,
}

/// The lens the player misses most relative to their evidence for it; ties go
/// to the lens with more misses, then to the earlier lens.
pub(crate) fn weakest_lens(mastery: &Mastery) -> Option<Concept> {
    let mut weakest: Option<(Concept, u64, u64)> = None;
    for (concept, record) in mastery {
        let missed = u64::from(record.missed);
        if missed == 0 {
            continue;
        }
        let attempts = missed + u64::from(record.evidence());
        let weaker = weakest.is_none_or(|(_, best_missed, best_attempts)| {
            let (left, right) = (missed * best_attempts, best_missed * attempts);
            left > right || (left == right && missed > best_missed)
        });
        if weaker {
            weakest = Some((*concept, missed, attempts));
        }
    }
    weakest.map(|(concept, _, _)| concept)
}

pub(crate) fn learner_state(
    batches: &[SavedQuestionBatch],
    progress: &SavedQuizProgress,
) -> LearnerState {
    let mut seen = HashSet::new();
    let asked = batches
        .iter()
        .rev()
        .flat_map(|batch| batch.questions.iter().rev())
        .map(|question| question_identity(&question.q))
        .filter(|identity| !identity.is_empty() && seen.insert(identity.clone()))
        .take(MAX_AVOIDED_STEMS)
        .collect();
    LearnerState {
        weakest: weakest_lens(&progress.mastery),
        asked,
    }
}

pub(crate) fn load_learner_state(path: &Path) -> Result<LearnerState, String> {
    Ok(learner_state(
        &load_saved_question_batches(path)?,
        &load_quiz_progress(path)?,
    ))
}

/// Questions in a batch that must use the level's focus lenses: two thirds,
/// rounded up (4 of 6).
fn focus_minimum(count: usize) -> usize {
    (count * 2).div_ceil(3)
}

fn lens_guide(concept: Concept) -> &'static str {
    match concept {
        Concept::Purpose => "what the project or a component is for and whom it serves",
        Concept::Responsibility => "which component owns a duty, and what it must not do",
        Concept::Interaction => "how components collaborate, hand off data, or react to events",
        Concept::Invariant => "a guarantee that must always hold, and what protects it",
        Concept::Tradeoff => "a design choice, what it buys, and what it costs",
    }
}

pub(crate) fn ai_question_prompt(
    project_name: &str,
    level: u32,
    count: usize,
    project_brief: &str,
    learner: &LearnerState,
) -> String {
    let tier = match level {
        0 | 1 => "INITIATE: establish what the project is for and which component owns each duty",
        2 | 3 => "ADEPT: connect how components interact and why the design accepts its tradeoffs",
        _ => "ORACLE-BOUND: probe the invariants the design protects and predict consequences",
    };
    let lenses = Concept::ALL
        .iter()
        .map(|concept| format!("- {}: {}", concept_key(*concept), lens_guide(*concept)))
        .collect::<Vec<_>>()
        .join("\n");
    let lens_names = lens_names();
    let focus = Concept::focus_for_level(level)
        .iter()
        .map(|concept| concept_key(*concept))
        .collect::<Vec<_>>()
        .join(" or ");
    let focus_count = focus_minimum(count);
    let transfer = if level >= 4 {
        format!(
            "TRANSFER: at least {} questions must be PREDICT questions. Begin each with \"PREDICT:\", describe a plausible change or failure (a removed check, a reordered step, a crash mid-operation, a new kind of input), and ask what the design implies would happen. Answer from the project's invariants and tradeoffs, never from implementation locations.\n\n",
            count.min(2)
        )
    } else {
        String::new()
    };
    let weakest = learner.weakest.map_or_else(
        || "WEAKEST LENS: none recorded yet.".to_string(),
        |concept| {
            let lens = concept_key(concept);
            format!("WEAKEST LENS: {lens}. The player misses {lens} questions most often; include at least one {lens} question aimed at a common misconception about it.")
        },
    );
    let asked = if learner.asked.is_empty() {
        "(none yet)".to_string()
    } else {
        learner
            .asked
            .iter()
            .map(|stem| format!("- {stem}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "You write questions for a retro handheld quiz game that teaches how a software project is designed. Generate exactly {count} multiple-choice questions at difficulty level {level} ({tier}).\n\nLENSES: tag every question with exactly one concept lens:\n{lenses}\nFOCUS: at least {focus_count} of the {count} questions must use the {focus} lens.\n\n{transfer}CONCEPTS ONLY: test the project's architecture, purpose, domain model, component responsibilities, interactions, invariants, tradeoffs, design rationale, or enduring behavior. Every question must still make sense if the project were reorganized and all implementation locations changed.\n\nNEVER ask about file names, paths, directories, or extensions; where code lives; repository structure; counts, sizes, or lines; dates, times, or versions; branches or commits; authors or contributors; ordering or recency; or any other state-in-time fact. Never use those facts as choices or rationales.\n\nCHOICES: exactly 4 non-empty, distinct choices and exactly one correct answer. Each wrong choice must be a plausible misconception a newcomer to this project might really hold, similar in length and style to the correct choice; never a joke or an obviously absurd option. Vary which position holds the correct answer across the batch.\n\nRATIONALES: every choice has a \"why\" of at most 90 characters. For the correct choice, explain why it holds. For each wrong choice, name the misconception it represents and why it fails. A why must never cite file names, paths, or extensions.\n\nDISPLAY LIMITS (hard: anything longer is discarded, not shortened): plain ASCII only. Each question, including any prefix, must be at most 100 characters so it wraps into 4 lines of 31. Each choice text must be a short phrase of 2 to 5 words and at most 28 characters; put nuance in its why, never in the choice text. Calibrate on these lengths: \"The headless engine\" is 19 characters, \"Refuses the cartridge\" is 21, \"Keeps the repo untouched\" is 24. Before answering, count the characters of every question and choice and rewrite any that are too long. Do not truncate words or sentences. Do not repeat questions.\n\nLEARNER STATE:\n{weakest}\nALREADY ASKED (never repeat these or close paraphrases of them):\n{asked}\n\nRespond with ONLY a JSON array, no prose and no code fences, where N is the 0-based index of the correct choice:\n[{{\"q\":\"...\",\"concept\":\"{lens_names}\",\"choices\":[{{\"text\":\"...\",\"why\":\"...\"}},{{\"text\":\"...\",\"why\":\"...\"}},{{\"text\":\"...\",\"why\":\"...\"}},{{\"text\":\"...\",\"why\":\"...\"}}],\"answer\":N}}]\n\nPROJECT: {project_name}\n{project_brief}",
    )
}

/// [`ai_question_prompt`], shortening the project brief at a character
/// boundary when needed so the whole prompt stays within
/// [`MAX_PROMPT_BYTES`]. A brief within its own budget is never shortened.
pub(crate) fn bounded_ai_question_prompt(
    project_name: &str,
    level: u32,
    count: usize,
    project_brief: &str,
    learner: &LearnerState,
) -> String {
    let mut brief = project_brief;
    loop {
        let prompt = ai_question_prompt(project_name, level, count, brief, learner);
        let overflow = prompt.len().saturating_sub(MAX_PROMPT_BYTES);
        if overflow == 0 || brief.is_empty() {
            return prompt;
        }
        let mut end = brief.len().saturating_sub(overflow);
        while !brief.is_char_boundary(end) {
            end -= 1;
        }
        brief = &brief[..end];
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    const RATIONALE: &str = "THIS NAMES THE MODEL THE DESIGN DEPENDS ON.";

    /// Parses one provider response into at most `count` playable questions,
    /// as a generation request without a repair pass would.
    fn parse_generated_questions(response: &str, count: usize) -> Result<Vec<QQuestion>, String> {
        parse_generated_batch(response, count)?.into_questions()
    }

    /// A provider reply modeled on real over-length output: one accepted
    /// question, three that fail only on mechanics, a trivia question, and a
    /// malformed one.
    pub(crate) const OVER_LENGTH_RESPONSE: &str = r#"[
        {
            "q": "Why does the engine own every game rule?",
            "concept": "Responsibilities",
            "choices": [
                {"text": "So the shell only draws frames", "why": "One owner keeps rules consistent; the shell just shows pixels."},
                {"text": "So the UI can skip the engine", "why": "Misconception: a bypass would split the rules across layers."},
                {"text": "To make the shell faster", "why": "Misconception: speed is not why rules live in one place."},
                {"text": "To store rules in the styles", "why": "Misconception: presentation cannot enforce game rules."}
            ],
            "answer": 0
        },
        {
            "q": "What happens when a provider reply mixes valid and invalid questions?",
            "concept": "Flows",
            "choices": [
                {"text": "Valid ones survive; batch may be short", "why": "Each question is judged alone, so good ones are kept."},
                {"text": "The whole reply is discarded", "why": "Misconception: one bad question does not sink the rest."},
                {"text": "Bad questions are cut to fit", "why": "Misconception: over-long text is rejected, never cut."},
                {"text": "A filler deck replaces them", "why": "Misconception: the game has no filler question deck."}
            ],
            "answer": 0
        },
        {
            "q": "Why does loading a cartridge never run its code?",
            "concept": "invariant",
            "choices": [
                {"text": "Running code would be too slow", "why": "Misconception: speed is not why loaded code stays inert."},
                {"text": "Trusted handlers keep untrusted repos safe", "why": "Only the app's own handlers act, so a hostile project cannot."},
                {"text": "The shell cannot start programs", "why": "Misconception: the shell could, but loading never asks it to."},
                {"text": "Saves would grow too large", "why": "Misconception: save size has nothing to do with running code."}
            ],
            "answer": 1
        },
        {
            "q": "In a design where the engine owns every rule and the shell only draws the frames it receives each tick, what must the shell never do?",
            "concept": "architecture",
            "choices": [
                "Decide game rules itself",
                {"text": "Draw frames → pixels", "why": "Misconception: drawing frames is exactly the shell's job."},
                {"text": "Forward player input", "why": "Misconception: the shell must forward input to the engine."},
                {"text": "Play the engine's notes", "why": "Misconception: playing notes is fine for the shell because the engine emits them with tick stamps, so this is allowed and not wrong."}
            ],
            "answer": 0
        },
        {
            "q": "Which file owns the game loop?",
            "concept": "responsibility",
            "choices": [
                {"text": "The one that holds the engine and its systems", "why": "It schedules every system."},
                {"text": "The shell script", "why": "Misconception: the shell only draws."},
                {"text": "The style sheet", "why": "Misconception: styles hold no logic."},
                {"text": "The manifest", "why": "Misconception: manifests only configure."}
            ],
            "answer": 0
        },
        {
            "q": "Why keep saves versioned?",
            "concept": "invariant",
            "choices": [
                {"text": "Old saves stay readable by new builds forever", "why": "Versions let loaders migrate."},
                {"text": "Saves get smaller", "why": "Misconception: versions add bytes."},
                {"text": "Loading gets faster", "why": "Misconception: speed is not the goal."}
            ],
            "answer": 0
        }
    ]"#;

    /// [`OVER_LENGTH_RESPONSE`], parsed: one accepted question and five
    /// rejections in reply order.
    pub(crate) fn over_length_batch() -> GeneratedBatch {
        parse_generated_batch(OVER_LENGTH_RESPONSE, 6).unwrap()
    }

    /// A complete payload v2 question with `answer` 0.
    pub(crate) fn question(text: &str, choices: &[&str]) -> QQuestion {
        QQuestion {
            q: text.to_string(),
            concept: Some("responsibility".to_string()),
            choices: choices
                .iter()
                .map(|choice| QChoice::Explained {
                    text: (*choice).to_string(),
                    why: RATIONALE.to_string(),
                })
                .collect(),
            answer: 0,
        }
    }

    fn legacy_question(text: &str, choices: &[&str]) -> QQuestion {
        QQuestion {
            q: text.to_string(),
            concept: None,
            choices: choices
                .iter()
                .map(|choice| QChoice::Plain((*choice).to_string()))
                .collect(),
            answer: 0,
        }
    }

    fn engine_state_question() -> QQuestion {
        question(
            "WHAT SHOULD OWN GAMEPLAY STATE?",
            &[
                "THE GAME ENGINE",
                "THE DEVICE SHELL",
                "THE STYLES",
                "THE VIEW",
            ],
        )
    }

    fn temporary_cartridge_path() -> std::path::PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "codequest-questions-test-{}-{unique}",
            std::process::id()
        ))
    }

    fn remove_save(path: &Path) {
        let _ = std::fs::remove_file(save::path_for(path));
    }

    fn evidence(question: &str, concept: Option<Concept>, correct: bool) -> AnswerEvidence {
        AnswerEvidence {
            question: question.to_string(),
            concept,
            correct,
            review: false,
        }
    }

    const LEGACY_BATCHES: &str = r#"[
        {
            "level": 1,
            "questions": [{
                "q": "WHAT SHOULD OWN GAMEPLAY STATE?",
                "choices": [
                    "THE GAME ENGINE",
                    "THE DEVICE SHELL",
                    "THE STYLES",
                    "THE VIEW"
                ],
                "answer": 0
            }]
        },
        {
            "level": 2,
            "questions": [{
                "q": "WHY KEEP THE DEVICE SHELL THIN?",
                "choices": [
                    "TO CENTRALIZE GAME RULES",
                    "TO DUPLICATE GAME STATE",
                    "TO HIDE ENGINE OUTPUT",
                    "TO BYPASS THE ENGINE"
                ],
                "answer": 0
            }]
        }
    ]"#;

    const V2_RESPONSE: &str = r#"[
        {
            "q": "Why does the engine own every game rule?",
            "concept": "Responsibilities",
            "choices": [
                {"text": "So the shell only draws frames", "why": "One owner keeps rules consistent; the shell just shows pixels."},
                {"text": "So the UI can skip the engine", "why": "Misconception: a bypass would split the rules across layers."},
                {"text": "To make the shell faster", "why": "Misconception: speed is not why rules live in one place."},
                {"text": "To store rules in the styles", "why": "Misconception: presentation cannot enforce game rules."}
            ],
            "answer": 0
        }
    ]"#;

    #[test]
    fn legacy_saved_questions_still_deserialize_and_play_without_rationales() {
        let batches: Vec<SavedQuestionBatch> = serde_json::from_str(LEGACY_BATCHES).unwrap();
        let legacy = &batches[0].questions[0];
        assert!(legacy.has_plain_choices());
        assert!(question_is_acceptable(legacy));
        assert!(
            !generated_question_is_acceptable(legacy),
            "new generations must use payload v2"
        );

        let playable = legacy.quiz_question(false);
        assert_eq!(playable.question, "WHAT SHOULD OWN GAMEPLAY STATE?");
        assert_eq!(playable.choices[0], "THE GAME ENGINE");
        assert_eq!(playable.concept, None);
        assert!(playable.rationales.is_empty());

        let saved_again = serde_json::to_value(&batches).unwrap();
        assert_eq!(
            saved_again,
            serde_json::from_str::<serde_json::Value>(LEGACY_BATCHES).unwrap(),
            "legacy questions round-trip unchanged"
        );
    }

    #[test]
    fn payload_v2_maps_rationales_in_choice_order_and_normalizes_the_lens() {
        let questions = parse_generated_questions(V2_RESPONSE, 6).unwrap();
        assert_eq!(questions.len(), 1);
        let generated = &questions[0];
        assert_eq!(generated.q, "WHY DOES THE ENGINE OWN EVERY GAME RULE?");
        assert_eq!(generated.concept.as_deref(), Some("responsibility"));

        let playable = generated.quiz_question(true);
        assert_eq!(playable.concept, Some(Concept::Responsibility));
        assert_eq!(playable.choices.len(), 4);
        assert_eq!(playable.rationales.len(), 4);
        assert_eq!(playable.choices[1], "SO THE UI CAN SKIP THE ENGINE");
        assert!(playable.rationales[1].starts_with("MISCONCEPTION: A BYPASS"));
        assert!(playable.review);

        let saved = serde_json::to_value(generated).unwrap();
        assert_eq!(saved["concept"], "responsibility");
        assert_eq!(
            saved["choices"][0]["text"],
            "SO THE SHELL ONLY DRAWS FRAMES"
        );
    }

    #[test]
    fn generated_questions_need_a_known_lens_and_a_fitting_why_for_every_choice() {
        assert!(generated_question_is_acceptable(&engine_state_question()));

        let mut no_lens = engine_state_question();
        no_lens.concept = None;
        assert!(!generated_question_is_acceptable(&no_lens));

        let mut unknown_lens = engine_state_question();
        unknown_lens.concept = Some("file layout".into());
        assert!(!generated_question_is_acceptable(&unknown_lens));

        let mut plain_choice = engine_state_question();
        plain_choice.choices[2] = QChoice::Plain("THE STYLES".into());
        assert!(!generated_question_is_acceptable(&plain_choice));
        assert!(
            !question_is_acceptable(&plain_choice),
            "half-explained saves are not legacy"
        );

        let mut missing_why = engine_state_question();
        missing_why.choices[1] = QChoice::Explained {
            text: "THE DEVICE SHELL".into(),
            why: String::new(),
        };
        assert!(!generated_question_is_acceptable(&missing_why));

        let mut long_why = engine_state_question();
        long_why.choices[3] = QChoice::Explained {
            text: "THE VIEW".into(),
            why: "A RATIONALE THAT RUNS ON ".repeat(8),
        };
        assert!(!generated_question_is_acceptable(&long_why));

        let mut located_why = engine_state_question();
        located_why.choices[0] = QChoice::Explained {
            text: "THE GAME ENGINE".into(),
            why: "IT IS DEFINED IN ENGINE.RS UNDER SRC-TAURI/SRC.".into(),
        };
        assert!(!generated_question_is_acceptable(&located_why));

        let mut unrenderable = engine_state_question();
        unrenderable.q = "WHAT SHOULD OWN GAMEPLAY STATE \u{2192} ENGINE?".into();
        assert!(!generated_question_is_acceptable(&unrenderable));
    }

    #[test]
    fn valid_questions_survive_a_mixed_ai_batch() {
        let valid = engine_state_question();
        let file_trivia = question(
            "WHICH FILE DEFINES THE ENGINE?",
            &["engine.rs", "main.js", "styles.css", "README.md"],
        );
        let mut missing_lens = engine_state_question();
        missing_lens.q = "WHY DOES THE ENGINE OWN STATE?".into();
        missing_lens.concept = None;
        let overflowing = question(
            "WHY KEEP OUTPUT WITHIN A FIXED PRESENTATION BOUNDARY?",
            &[
                "THIS RESPONSE CANNOT FIT IN THE AVAILABLE CHOICE ROW",
                "SECOND",
                "THIRD",
                "FOURTH",
            ],
        );

        let accepted = accepted_question_batch(
            vec![
                valid.clone(),
                file_trivia.clone(),
                missing_lens,
                overflowing,
                valid.clone(),
            ],
            6,
        )
        .expect("the valid question should remain playable");
        assert_eq!(accepted, vec![valid], "invalid and repeated questions drop");
        assert!(accepted_question_batch(vec![file_trivia], 1).is_none());
    }

    #[test]
    fn duplicate_choices_are_rejected() {
        let ambiguous = question(
            "WHAT SHOULD OWN GAMEPLAY STATE?",
            &[
                "THE GAME ENGINE",
                "THE GAME ENGINE",
                "THE VIEW",
                "THE DEVICE SHELL",
            ],
        );

        assert!(!question_is_acceptable(&ambiguous));
        assert!(!generated_question_is_acceptable(&ambiguous));
    }

    #[test]
    fn conceptual_questions_may_use_ordinary_nouns_that_once_looked_like_trivia() {
        let conceptual = [
            "WHY WRITE THE SAVE FILE ATOMICALLY?",
            "WHY DOES THE LATEST ATTEMPT DECIDE REVIEW?",
            "WHAT DOES THE HOT PATH AVOID?",
            "WHY USE A READ/WRITE LOCK?",
            "WHEN DOES AN ANSWER COMMIT?",
            "WHY SHOW THE BRANCH ON THE LABEL?",
            "WHY SEND FRAMES AS RAW BYTES?",
            "WHY PICK A DIRECTORY, NOT A FILE?",
            "WHY BUILD THE SHELL ON NODE.JS?",
            "WHY NOT CACHE STATE...CRASHES?",
            "WHAT DO BROWSER EXTENSIONS ADD?",
            "WHY KEEP OLDEST LESSONS FIRST?",
            "WHY DOES THE SERVER TRUST INPUT?",
        ];
        for text in conceptual {
            let candidate = question(
                text,
                &[
                    "TO KEEP ONE OWNER",
                    "TO DUPLICATE STATE",
                    "TO HIDE FAILURES",
                    "TO SKIP VALIDATION",
                ],
            );
            assert!(generated_question_is_acceptable(&candidate), "{text}");
        }

        let conceptual_choices = question(
            "WHY WRITE THE SAVE ATOMICALLY?",
            &[
                "A CRASH CANNOT TEAR THE FILE",
                "IT MAKES FILES SMALLER",
                "IT SKIPS THE LATEST WRITE",
                "IT KEEPS THE PATH SHORT",
            ],
        );
        assert!(generated_question_is_acceptable(&conceptual_choices));
    }

    #[test]
    fn location_and_state_in_time_trivia_is_still_rejected() {
        let choices = [
            "THE GAME ENGINE",
            "THE DEVICE SHELL",
            "THE STYLES",
            "THE VIEW",
        ];
        let trivia = [
            "WHICH FILE OWNS GAME STATE?",
            "WHAT SOURCE FILE OWNS INPUT?",
            "WHICH CONFIG FOLDER HOLDS KEYS?",
            "WHERE DOES THE RENDERER LIVE?",
            "HOW MANY FILES ARE IN THE REPOSITORY?",
            "WHAT DID THE LATEST COMMIT ADD?",
            "HOW MANY COMMITS TOUCHED SAVES?",
            "WHO IS THE MAIN AUTHOR?",
            "WHICH BRANCH IS CHECKED OUT?",
            "WHAT CHANGED MOST RECENTLY?",
            "WHAT DOES ENGINE.RS OWN?",
            "WHAT DOES SRC/ENGINE OWN?",
            "WHAT DOES ./BUILD DO?",
            "WHY DOES MAIN.C EXIST?",
            "WHAT DOES DEPLOY.SH RUN?",
            "WHY ARE .RS SOURCES PREFERRED?",
            "WHAT DOES .GITIGNORE HIDE?",
            "WHAT CHANGED IN 2024?",
            "WHAT DID V0.2.4 SHIP?",
            "WHAT DID 4067E07 CHANGE?",
            "WHAT DOES THE README PROMISE?",
            "WHAT IS THE PROJECT STRUCTURE?",
        ];
        for text in trivia {
            assert!(
                !generated_question_is_acceptable(&question(text, &choices)),
                "{text}"
            );
            assert!(
                !question_is_acceptable(&legacy_question(text, &choices)),
                "{text}"
            );
        }

        let file_answers = question(
            "WHAT OWNS GAME STATE?",
            &["engine.rs", "main.js", "styles.css", "README.md"],
        );
        assert!(!generated_question_is_acceptable(&file_answers));
        let path_answers = question(
            "WHAT OWNS GAME STATE?",
            &["SRC/ENGINE", "THE SHELL", "THE STYLES", "THE VIEW"],
        );
        assert!(!generated_question_is_acceptable(&path_answers));
    }

    #[test]
    fn accepted_ai_questions_are_conceptual_and_fit_the_quiz_layout() {
        let conceptual = question(
            "WHY SEPARATE GAME STATE FROM THE UI?",
            &[
                "TO KEEP RESPONSIBILITIES CLEAR",
                "TO HIDE FAILURES",
                "TO COUPLE COMPONENTS",
                "TO DUPLICATE STATE",
            ],
        );
        assert!(question_is_acceptable(&conceptual));
        assert!(generated_question_is_acceptable(&conceptual));

        let file_trivia = question(
            "WHICH FILE OWNS GAME STATE?",
            &["engine.rs", "main.js", "styles.css", "README.md"],
        );
        assert!(!question_is_acceptable(&file_trivia));

        let state_trivia = question(
            "HOW MANY FILES ARE IN THE REPOSITORY?",
            &["ONE", "TWO", "THREE", "FOUR"],
        );
        assert!(!question_is_acceptable(&state_trivia));

        let overflowing = question(
            &"CONCEPTUAL WORD ".repeat(40),
            &[
                "THIS CHOICE IS LONGER THAN THE DISPLAY CAN POSSIBLY SHOW",
                "SECOND",
                "THIRD",
                "FOURTH",
            ],
        );
        assert!(!question_is_acceptable(&overflowing));
    }

    #[test]
    fn invalid_ai_results_are_rejected_instead_of_truncated() {
        let valid = engine_state_question();
        let file_trivia = question(
            "WHICH FILE DEFINES THE ENGINE?",
            &["engine.rs", "main.js", "styles.css", "README.md"],
        );
        let overflowing = question(
            "WHY KEEP OUTPUT WITHIN A FIXED PRESENTATION BOUNDARY?",
            &[
                "THIS RESPONSE CANNOT FIT IN THE AVAILABLE CHOICE ROW",
                "SECOND",
                "THIRD",
                "FOURTH",
            ],
        );

        let accepted = retain_acceptable_questions(vec![valid.clone(), file_trivia, overflowing]);
        assert_eq!(accepted, vec![valid]);
    }

    #[test]
    fn provider_responses_are_found_inside_prose_fences_and_truncation() {
        let wrapped = format!(
            "Here are the questions [as requested]:\n```json\n{V2_RESPONSE}\n```\nLet me know [if] you need more."
        );
        assert_eq!(parse_generated_questions(&wrapped, 6).unwrap().len(), 1);

        let second = V2_RESPONSE
            .trim()
            .trim_start_matches('[')
            .trim_end_matches(']')
            .replace("engine own every game rule", "shell stay free of rules");
        let truncated = format!(
            "[{},{second}, {{\"q\": \"WHY IS THIS CUT",
            V2_RESPONSE
                .trim()
                .trim_start_matches('[')
                .trim_end_matches(']')
        );
        let salvaged = parse_generated_questions(&truncated, 6).unwrap();
        assert_eq!(
            salvaged.len(),
            2,
            "finished questions survive a cut-off reply"
        );
        assert_eq!(salvaged[1].q, "WHY DOES THE SHELL STAY FREE OF RULES?");

        let typographic = V2_RESPONSE.replace("Why does", "\u{201C}Why\u{201D} does");
        assert_eq!(
            parse_generated_questions(&typographic, 6).unwrap()[0].q,
            "\"WHY\" DOES THE ENGINE OWN EVERY GAME RULE?"
        );

        assert_eq!(
            parse_generated_questions("no questions today", 6).unwrap_err(),
            "NO JSON IN RESPONSE"
        );
        assert_eq!(
            parse_generated_questions("[not json]", 6).unwrap_err(),
            "UNPARSEABLE QUESTIONS"
        );
        let legacy_only = r#"[{"q":"WHAT OWNS STATE?","choices":["A","B","C","D"],"answer":0}]"#;
        assert_eq!(
            parse_generated_questions(legacy_only, 6).unwrap_err(),
            "INCOMPLETE OR INVALID QUESTIONS"
        );
    }

    #[test]
    fn answer_evidence_retires_correct_answers_and_keeps_misses_for_review() {
        let mut progress = SavedQuizProgress::default();
        let lens = Some(Concept::Invariant);

        progress.record(&evidence("WHY WRITE SAVES ATOMICALLY?", lens, false));
        progress.record(&evidence("  why write saves atomically? ", lens, false));
        assert_eq!(progress.missed_questions, ["WHY WRITE SAVES ATOMICALLY?"]);
        assert!(progress.answered_questions.is_empty());
        assert!(progress.retired().is_empty(), "a miss never retires");

        progress.record(&AnswerEvidence {
            review: true,
            ..evidence("WHY WRITE SAVES ATOMICALLY?", lens, true)
        });
        assert!(progress.missed_questions.is_empty());
        assert_eq!(progress.answered_questions, ["WHY WRITE SAVES ATOMICALLY?"]);
        assert_eq!(
            progress.mastery[&Concept::Invariant],
            learning::LensRecord {
                first_try: 0,
                redeemed: 1,
                missed: 2
            }
        );

        progress.record(&evidence("", lens, true));
        assert_eq!(progress.answered_questions.len(), 1);
    }

    #[test]
    fn answered_question_progress_is_namespaced_and_idempotent() {
        let path = temporary_cartridge_path();
        let mut save = save::SaveFile::open_or_create(&path).unwrap();
        save.set(
            AI_QUESTION_BATCHES_KEY,
            &serde_json::json!([{ "level": 1, "questions": [] }]),
        )
        .unwrap();

        persist_answer_evidence(&path, &evidence("WHAT OWNS GAMEPLAY STATE?", None, true)).unwrap();
        persist_answer_evidence(
            &path,
            &evidence("  what owns gameplay state?  ", None, true),
        )
        .unwrap();

        let progress = load_quiz_progress(&path).unwrap();
        assert_eq!(progress.answered_questions, ["WHAT OWNS GAMEPLAY STATE?"]);
        assert!(progress.mastery.is_empty(), "legacy questions have no lens");
        let reloaded = save::SaveFile::open_or_create(&path).unwrap();
        assert!(reloaded
            .get::<serde_json::Value>(AI_QUESTION_BATCHES_KEY)
            .is_some());

        remove_save(&path);
    }

    #[test]
    fn progress_from_earlier_builds_loads_as_retired_questions() {
        let path = temporary_cartridge_path();
        let mut save = save::SaveFile::open_or_create(&path).unwrap();
        save.set(
            QUIZ_PROGRESS_KEY,
            &serde_json::json!({ "answered_questions": ["WHAT SHOULD OWN GAMEPLAY STATE?"] }),
        )
        .unwrap();

        let progress = load_quiz_progress(&path).unwrap();
        assert!(progress.missed_questions.is_empty());
        assert!(progress.mastery.is_empty());
        assert!(progress
            .retired()
            .contains("WHAT SHOULD OWN GAMEPLAY STATE?"));
        assert!(
            progress.question_attempts.is_empty(),
            "earlier builds kept no attempt counts"
        );

        persist_answer_evidence(
            &path,
            &evidence("WHY KEEP THE SHELL THIN?", Some(Concept::Tradeoff), false),
        )
        .unwrap();
        let stored = save::SaveFile::open_or_create(&path)
            .unwrap()
            .get::<serde_json::Value>(QUIZ_PROGRESS_KEY)
            .unwrap();
        assert_eq!(
            stored,
            serde_json::json!({
                "answered_questions": ["WHAT SHOULD OWN GAMEPLAY STATE?"],
                "missed_questions": ["WHY KEEP THE SHELL THIN?"],
                "mastery": { "tradeoff": { "first_try": 0, "redeemed": 0, "missed": 1 } },
                "question_attempts": { "WHY KEEP THE SHELL THIN?": 1 }
            })
        );

        remove_save(&path);
    }

    #[test]
    fn concurrent_batches_and_answers_all_reach_the_save() {
        let path = temporary_cartridge_path();
        std::thread::scope(|scope| {
            for batch in 0..4 {
                let path = &path;
                scope.spawn(move || {
                    let mut generated = engine_state_question();
                    generated.q = format!("WHY DOES LAYER {batch} OWN STATE?");
                    persist_ai_question_batch(path, 1, &[generated]).unwrap();
                });
            }
            for answer in 0..4 {
                let path = &path;
                scope.spawn(move || {
                    persist_answer_evidence(
                        path,
                        &evidence(
                            &format!("WHY IS RULE {answer} FIXED?"),
                            Some(Concept::Purpose),
                            answer % 2 == 0,
                        ),
                    )
                    .unwrap();
                });
            }
        });

        assert_eq!(load_saved_question_batches(&path).unwrap().len(), 4);
        let progress = load_quiz_progress(&path).unwrap();
        assert_eq!(progress.answered_questions.len(), 2);
        assert_eq!(progress.missed_questions.len(), 2);
        assert_eq!(progress.mastery[&Concept::Purpose].evidence(), 2);
        assert_eq!(progress.mastery[&Concept::Purpose].missed, 2);

        remove_save(&path);
    }

    #[test]
    fn saving_a_batch_keeps_entries_this_build_cannot_read() {
        let path = temporary_cartridge_path();
        let mut save = save::SaveFile::open_or_create(&path).unwrap();
        let future = serde_json::json!({ "level": 9, "questions": [{ "format": 3 }] });
        save.set(AI_QUESTION_BATCHES_KEY, &vec![future.clone()])
            .unwrap();

        persist_ai_question_batch(&path, 2, &[engine_state_question()]).unwrap();

        let stored = save::SaveFile::open_or_create(&path)
            .unwrap()
            .get::<Vec<serde_json::Value>>(AI_QUESTION_BATCHES_KEY)
            .unwrap();
        assert_eq!(stored.len(), 2);
        assert_eq!(stored[0], future);
        let playable = load_saved_question_batches(&path).unwrap();
        assert_eq!(playable.len(), 1, "unreadable batches are skipped on load");
        assert_eq!(playable[0].level, 2);

        remove_save(&path);
    }

    #[test]
    fn generated_ai_batch_is_saved_without_replacing_other_game_data() {
        let path = temporary_cartridge_path();
        let mut save = save::SaveFile::open_or_create(&path).unwrap();
        save.set("quest.progress", &serde_json::json!({ "bosses": 2 }))
            .unwrap();
        let generated = vec![engine_state_question()];

        persist_ai_question_batch(&path, 3, &generated).unwrap();
        assert!(persist_ai_question_batch(&path, 3, &[legacy_question("WHY?", &[])]).is_err());

        let reloaded = save::SaveFile::open_or_create(&path).unwrap();
        assert_eq!(
            reloaded.get::<serde_json::Value>("quest.progress"),
            Some(serde_json::json!({ "bosses": 2 }))
        );
        let batches = reloaded
            .get::<Vec<SavedQuestionBatch>>(AI_QUESTION_BATCHES_KEY)
            .unwrap();
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].level, 3);
        assert_eq!(batches[0].questions, generated);

        remove_save(&path);
    }

    #[test]
    fn cartridge_reload_combines_legacy_and_current_ai_batches_oldest_first() {
        let path = temporary_cartridge_path();
        let mut save = save::SaveFile::open_or_create(&path).unwrap();
        let current = SavedQuestionBatch {
            level: 2,
            questions: vec![question(
                "WHY KEEP THE DEVICE SHELL THIN?",
                &[
                    "TO CENTRALIZE GAME RULES",
                    "TO DUPLICATE GAME STATE",
                    "TO HIDE ENGINE OUTPUT",
                    "TO BYPASS THE ENGINE",
                ],
            )],
        };
        let legacy = SavedQuestionBatch {
            level: 1,
            questions: vec![legacy_question(
                "WHAT SHOULD OWN GAMEPLAY STATE?",
                &[
                    "THE GAME ENGINE",
                    "THE DEVICE SHELL",
                    "THE STYLES",
                    "THE VIEW",
                ],
            )],
        };
        save.set(AI_QUESTION_BATCHES_KEY, &[current]).unwrap();
        save.set(LEGACY_CLAUDE_QUESTION_BATCHES_KEY, &[legacy])
            .unwrap();

        let batches = load_saved_question_batches(&path).unwrap();

        assert_eq!(batches.len(), 2);
        assert_eq!(
            batches[0].level, 1,
            "the legacy key holds the older batches"
        );
        assert_eq!(batches[1].level, 2);
        remove_save(&path);
    }

    #[test]
    fn saved_batches_queue_easiest_first_while_the_journal_stays_chronological() {
        let question = |text: &str| QQuestion {
            q: text.into(),
            ..engine_state_question()
        };
        // A run ended with a prefetched Oracle-bound batch unplayed, and an
        // Initiate batch was generated later.
        let batches = vec![
            SavedQuestionBatch {
                level: 4,
                questions: vec![question("PREDICT: WHAT IF THE ENGINE LOST STATE?")],
            },
            SavedQuestionBatch {
                level: 1,
                questions: vec![
                    question("WHAT IS THE PROJECT FOR?"),
                    question("WHO OWNS THE RULES?"),
                ],
            },
        ];
        let mut progress = SavedQuizProgress::default();
        progress.record(&evidence(
            "PREDICT: WHAT IF THE ENGINE LOST STATE?",
            None,
            false,
        ));
        progress.record(&evidence("WHAT IS THE PROJECT FOR?", None, false));

        let loaded = cartridge_questions(batches, progress);

        let queued = loaded
            .questions
            .iter()
            .map(|question| question.question.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            queued,
            [
                "WHAT IS THE PROJECT FOR?",
                "WHO OWNS THE RULES?",
                "PREDICT: WHAT IF THE ENGINE LOST STATE?",
            ],
            "a new Initiate run meets Initiate questions first"
        );
        assert_eq!(loaded.batch_ends, [2, 3]);
        assert_eq!(loaded.batch_levels, [1, 4]);
        let journal = loaded
            .lessons
            .iter()
            .map(|lesson| lesson.question.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            journal,
            [
                "PREDICT: WHAT IF THE ENGINE LOST STATE?",
                "WHAT IS THE PROJECT FOR?"
            ],
            "the journal keeps generation order"
        );
    }

    #[test]
    fn cartridge_load_retires_answers_reviews_misses_and_journals_both() {
        let batches: Vec<SavedQuestionBatch> = serde_json::from_str(LEGACY_BATCHES).unwrap();
        let mut explained = engine_state_question();
        explained.q = "WHY DOES THE ENGINE OWN STATE?".into();
        explained.answer = 1;
        let mut unanswered = engine_state_question();
        unanswered.q = "WHY DOES THE SHELL DRAW ONLY?".into();
        let batches = [
            batches,
            vec![SavedQuestionBatch {
                level: 3,
                questions: vec![
                    explained.clone(),
                    unanswered.clone(),
                    // Repeated by the provider in a later batch.
                    batches_first_question(),
                ],
            }],
        ]
        .concat();
        let mut progress = SavedQuizProgress::default();
        progress.record(&evidence("WHAT SHOULD OWN GAMEPLAY STATE?", None, true));
        progress.record(&evidence("WHY KEEP THE DEVICE SHELL THIN?", None, false));
        progress.record(&evidence(
            &explained.q,
            Some(Concept::Responsibility),
            false,
        ));

        let loaded = cartridge_questions(batches, progress.clone());

        let queued = loaded
            .questions
            .iter()
            .map(|question| (question.question.as_str(), question.review))
            .collect::<Vec<_>>();
        assert_eq!(
            queued,
            [
                ("WHY KEEP THE DEVICE SHELL THIN?", true),
                ("WHY DOES THE ENGINE OWN STATE?", true),
                ("WHY DOES THE SHELL DRAW ONLY?", false),
            ]
        );
        assert_eq!(
            loaded.batch_ends,
            [1, 3],
            "an emptied batch leaves no boundary"
        );
        assert_eq!(loaded.mastery, progress.mastery);

        assert_eq!(
            loaded.lessons,
            [
                Lesson {
                    question: "WHAT SHOULD OWN GAMEPLAY STATE?".into(),
                    answer: "THE GAME ENGINE".into(),
                    rationale: String::new(),
                    concept: None,
                    outstanding: false,
                },
                Lesson {
                    question: "WHY KEEP THE DEVICE SHELL THIN?".into(),
                    answer: "TO CENTRALIZE GAME RULES".into(),
                    rationale: String::new(),
                    concept: None,
                    outstanding: true,
                },
                Lesson {
                    question: explained.q.clone(),
                    answer: "THE DEVICE SHELL".into(),
                    rationale: RATIONALE.into(),
                    concept: Some(Concept::Responsibility),
                    outstanding: true,
                },
            ]
        );
    }

    fn batches_first_question() -> QQuestion {
        let batches: Vec<SavedQuestionBatch> = serde_json::from_str(LEGACY_BATCHES).unwrap();
        batches[0].questions[0].clone()
    }

    #[test]
    fn new_batches_skip_retired_questions_and_bring_missed_ones_back_as_reviews() {
        let mut progress = SavedQuizProgress::default();
        progress.record(&evidence("WHAT SHOULD OWN GAMEPLAY STATE?", None, true));
        progress.record(&evidence("WHY DOES THE ENGINE OWN STATE?", None, false));
        let mut missed = engine_state_question();
        missed.q = "why does the engine  own state?".into();
        let mut fresh = engine_state_question();
        fresh.q = "WHY KEEP THE SHELL THIN?".into();

        let playable =
            playable_new_questions(vec![engine_state_question(), missed, fresh], &progress);

        assert_eq!(
            playable
                .iter()
                .map(|question| (question.question.as_str(), question.review))
                .collect::<Vec<_>>(),
            [
                ("why does the engine  own state?", true),
                ("WHY KEEP THE SHELL THIN?", false)
            ],
            "a regenerated missed stem is a review; a new stem is not"
        );
    }

    #[test]
    fn missed_questions_keep_their_attempt_count_until_redeemed() {
        let mut progress = SavedQuizProgress::default();
        let question = "WHY WRITE SAVES ATOMICALLY?";
        progress.record(&evidence(question, None, false));
        progress.record(&AnswerEvidence {
            review: true,
            ..evidence(" why write saves atomically? ", None, false)
        });
        assert_eq!(progress.question_attempts[question], 2);

        // A save from an earlier build lists a miss without a count; its next
        // miss happened at review attempt 1 at the earliest.
        progress.question_attempts.clear();
        progress.record(&AnswerEvidence {
            review: true,
            ..evidence(question, None, false)
        });
        assert_eq!(progress.question_attempts[question], 2);

        let loaded = cartridge_questions(
            vec![SavedQuestionBatch {
                level: 1,
                questions: vec![QQuestion {
                    q: question.into(),
                    ..engine_state_question()
                }],
            }],
            progress.clone(),
        );
        assert_eq!(loaded.attempts[question], 2, "the count reaches the engine");
        assert!(loaded.questions[0].review);

        progress.record(&AnswerEvidence {
            review: true,
            ..evidence(question, None, true)
        });
        assert!(
            progress.question_attempts.is_empty(),
            "a redeemed question retires with its count"
        );
    }

    #[test]
    fn learner_state_names_the_most_missed_lens_and_recent_stems() {
        let record = |first_try, missed| learning::LensRecord {
            first_try,
            redeemed: 0,
            missed,
        };
        let mut progress = SavedQuizProgress::default();
        assert_eq!(weakest_lens(&progress.mastery), None);
        progress.mastery.insert(Concept::Purpose, record(5, 0));
        assert_eq!(
            weakest_lens(&progress.mastery),
            None,
            "no misses, no weakness"
        );
        progress.mastery.insert(Concept::Interaction, record(1, 1));
        progress.mastery.insert(Concept::Invariant, record(0, 2));
        progress.mastery.insert(Concept::Tradeoff, record(3, 3));
        assert_eq!(weakest_lens(&progress.mastery), Some(Concept::Invariant));
        progress.mastery.insert(Concept::Invariant, record(2, 2));
        assert_eq!(
            weakest_lens(&progress.mastery),
            Some(Concept::Tradeoff),
            "equal miss rates prefer the lens with more misses"
        );

        let batches = (0..30)
            .map(|index| SavedQuestionBatch {
                level: 1,
                questions: vec![question(
                    &format!("why is rule {index}   fixed?"),
                    &["A", "B", "C", "D"],
                )],
            })
            .collect::<Vec<_>>();
        let learner = learner_state(&batches, &progress);
        assert_eq!(learner.asked.len(), MAX_AVOIDED_STEMS);
        assert_eq!(learner.asked[0], "WHY IS RULE 29 FIXED?");
        assert_eq!(learner.weakest, Some(Concept::Tradeoff));
    }

    #[test]
    fn ai_prompt_requests_only_concepts_that_fit_the_display() {
        let prompt = ai_question_prompt(
            "DEMO PROJECT",
            3,
            12,
            "A project that separates its engine from its device shell.",
            &LearnerState::default(),
        );

        assert!(prompt.contains("CONCEPTS ONLY"));
        assert!(prompt.contains("NEVER ask about file names, paths, directories, or extensions"));
        assert!(prompt.contains("still make sense if the project were reorganized"));
        assert!(prompt.contains("wraps into 4 lines of 31"));
        assert!(prompt.contains("at most 28 characters"));
        assert!(prompt.contains("count the characters of every question and choice"));
        assert!(!prompt.contains("FILES:"));
        assert!(!prompt.contains("COMMIT MESSAGES"));
        assert!(prompt.ends_with("A project that separates its engine from its device shell."));
    }

    #[test]
    fn ai_prompt_requests_payload_v2_with_lens_focus_and_misconception_rationales() {
        let prompt = ai_question_prompt("DEMO", 1, 6, "BRIEF", &LearnerState::default());

        assert!(
            prompt.contains(r#""concept":"purpose|responsibility|interaction|invariant|tradeoff""#)
        );
        assert!(prompt.contains(r#""choices":[{"text":"...","why":"..."}"#));
        assert!(prompt
            .contains("at least 4 of the 6 questions must use the purpose or responsibility lens"));
        assert!(prompt.contains("\"why\" of at most 90 characters"));
        assert!(prompt.contains("name the misconception it represents"));
        assert!(prompt.contains("plausible misconception"));
        assert!(prompt.contains("similar in length"));
        assert!(prompt.contains("Vary which position holds the correct answer"));
        assert!(
            !prompt.contains("PREDICT"),
            "transfer waits for the Oracle bond"
        );

        let adept = ai_question_prompt("DEMO", 3, 6, "BRIEF", &LearnerState::default());
        assert!(adept.contains("use the interaction or tradeoff lens"));
    }

    #[test]
    fn oracle_bound_prompts_require_predict_transfer_questions() {
        let prompt = ai_question_prompt("DEMO", 4, 6, "BRIEF", &LearnerState::default());
        assert!(prompt.contains("at least 2 questions must be PREDICT questions"));
        assert!(prompt.contains("Begin each with \"PREDICT:\""));
        assert!(prompt.contains("use the invariant or tradeoff lens"));
        assert!(
            ai_question_prompt("DEMO", 9, 6, "BRIEF", &LearnerState::default())
                .contains("PREDICT questions")
        );
    }

    #[test]
    fn ai_prompt_carries_learner_state() {
        let fresh = ai_question_prompt("DEMO", 2, 6, "BRIEF", &LearnerState::default());
        assert!(fresh.contains("WEAKEST LENS: none recorded yet."));
        assert!(fresh.contains(
            "ALREADY ASKED (never repeat these or close paraphrases of them):\n(none yet)"
        ));

        let learner = LearnerState {
            weakest: Some(Concept::Invariant),
            asked: vec![
                "WHY WRITE SAVES ATOMICALLY?".into(),
                "WHO OWNS THE GAME LOOP?".into(),
            ],
        };
        let prompt = ai_question_prompt("DEMO", 2, 6, "BRIEF", &learner);
        assert!(prompt.contains("WEAKEST LENS: invariant."));
        assert!(prompt.contains("include at least one invariant question"));
        assert!(prompt.contains("- WHY WRITE SAVES ATOMICALLY?\n- WHO OWNS THE GAME LOOP?"));
    }

    #[test]
    fn a_full_brief_fits_the_prompt_budget_and_oversized_briefs_are_trimmed() {
        let brief = "X".repeat(crate::repo_context::BRIEF_BUDGET);
        let learner = LearnerState {
            weakest: Some(Concept::Responsibility),
            asked: vec!["Q".repeat(4 * engine::QUIZ_QUESTION_COLUMNS); MAX_AVOIDED_STEMS],
        };
        let name = "A PROJECT NAME OF SOME LENGTH";
        let prompt = ai_question_prompt(name, 4, 6, &brief, &learner);
        assert!(
            prompt.len() <= MAX_PROMPT_BYTES,
            "a full brief fits untrimmed: {}",
            prompt.len()
        );
        assert_eq!(
            bounded_ai_question_prompt(name, 4, 6, &brief, &learner),
            prompt
        );

        let oversized = "\u{e9}".repeat(MAX_PROMPT_BYTES);
        let bounded = bounded_ai_question_prompt("DEMO", 4, 6, &oversized, &learner);
        assert!(bounded.len() <= MAX_PROMPT_BYTES, "{}", bounded.len());
        assert!(bounded.contains("PROJECT: DEMO\n\u{e9}\u{e9}"));
        assert!(
            bounded.len() > MAX_PROMPT_BYTES - 4,
            "trims only the excess"
        );
        assert_eq!(
            bounded_ai_question_prompt("DEMO", 1, 6, "BRIEF", &learner),
            ai_question_prompt("DEMO", 1, 6, "BRIEF", &learner)
        );
    }

    #[test]
    fn lens_keys_match_the_learning_model_serialization() {
        for concept in Concept::ALL {
            assert_eq!(
                serde_json::to_value(concept).unwrap(),
                serde_json::json!(concept_key(concept))
            );
            assert_eq!(Concept::parse(concept_key(concept)), Some(concept));
        }
    }

    #[test]
    fn diagnostics_measure_each_violation_of_a_rejected_question() {
        let batch = over_length_batch();
        assert_eq!(batch.accepted.len(), 1);
        assert_eq!(
            batch.accepted[0].q,
            "WHY DOES THE ENGINE OWN EVERY GAME RULE?"
        );
        assert_eq!(batch.rejected.len(), 5);
        let violations = batch
            .rejected
            .iter()
            .map(question_violations)
            .collect::<Vec<_>>();

        assert_eq!(
            violations[0],
            [Violation::ChoiceTooLong {
                choice: 0,
                chars: 38
            }],
            "VALID ONES SURVIVE; BATCH MAY BE SHORT"
        );
        assert_eq!(
            batch.rejected[0].concept.as_deref(),
            Some("interaction"),
            "a lens spelling the parser maps is canonicalized, not reported"
        );
        assert_eq!(
            violations[1],
            [Violation::ChoiceTooLong {
                choice: 1,
                chars: 42
            }],
            "TRUSTED HANDLERS KEEP UNTRUSTED REPOS SAFE"
        );
        assert_eq!(
            violations[2],
            [
                Violation::QuestionTooLong {
                    chars: 133,
                    lines: 5
                },
                Violation::UnknownLens {
                    given: Some("architecture".into())
                },
                Violation::WhyMissing { choice: 0 },
                Violation::WhyTooLong {
                    choice: 3,
                    chars: 132,
                    lines: 5
                },
                Violation::NotAscii {
                    field: Field::Choice(1),
                    characters: vec!['\u{2192}']
                },
            ]
        );
        assert!(violations[3].contains(&Violation::Trivia {
            field: Field::Question
        }));
        assert!(violations[3].contains(&Violation::ChoiceTooLong {
            choice: 0,
            chars: 45
        }));
        assert!(violations[4].contains(&Violation::Malformed("A QUESTION NEEDS EXACTLY 4 CHOICES")));

        let repairable = batch.rejected.iter().map(is_repairable).collect::<Vec<_>>();
        assert_eq!(
            repairable,
            [true, true, true, false, false],
            "trivia and malformed questions are never repaired, whatever else they fail"
        );
        assert!(!is_repairable(&batch.accepted[0]), "nothing to repair");
    }

    #[test]
    fn diagnostics_agree_with_the_acceptance_policy() {
        let mut candidates = over_length_batch().rejected;
        candidates.extend(over_length_batch().accepted);
        candidates.push(engine_state_question());
        let mut located_why = engine_state_question();
        located_why.choices[0] = QChoice::Explained {
            text: "THE GAME ENGINE".into(),
            why: "IT IS DEFINED IN ENGINE.RS.".into(),
        };
        candidates.push(located_why);
        let mut empty_why = engine_state_question();
        empty_why.choices[2] = QChoice::Explained {
            text: "THE STYLES".into(),
            why: "  ".into(),
        };
        candidates.push(empty_why);
        let mut out_of_range = engine_state_question();
        out_of_range.answer = 4;
        candidates.push(out_of_range);
        let mut unrenderable_why = engine_state_question();
        unrenderable_why.choices[1] = QChoice::Explained {
            text: "THE DEVICE SHELL".into(),
            why: "MISCONCEPTION: THE SHELL \u{2260} THE ENGINE.".into(),
        };
        candidates.push(unrenderable_why);
        candidates.push(question(
            "WHAT SHOULD OWN GAMEPLAY STATE?",
            &["THE ENGINE", "the engine ", "THE VIEW", "THE SHELL"],
        ));
        candidates.push(question("", &["A", "B", "C", "D"]));
        candidates.push(legacy_question(
            "WHAT SHOULD OWN GAMEPLAY STATE?",
            &["THE GAME ENGINE", "THE SHELL", "THE STYLES", "THE VIEW"],
        ));

        for candidate in &candidates {
            let violations = question_violations(candidate);
            assert_eq!(
                violations.is_empty(),
                generated_question_is_acceptable(candidate),
                "{candidate:?}: {violations:?}"
            );
        }
        let legacy = candidates.last().unwrap();
        assert_eq!(
            question_violations(legacy),
            [
                Violation::UnknownLens { given: None },
                Violation::WhyMissing { choice: 0 },
                Violation::WhyMissing { choice: 1 },
                Violation::WhyMissing { choice: 2 },
                Violation::WhyMissing { choice: 3 },
            ]
        );
    }

    #[test]
    fn repair_prompt_names_each_measured_violation_and_repeats_the_rest_verbatim() {
        let batch = over_length_batch();
        let request = batch
            .repair_request(6)
            .expect("three rejections are mechanical");
        assert_eq!(request.originals, batch.rejected[..3]);
        assert_eq!(request.prompt, repair_prompt(&request.originals), "pure");

        let prompt = &request.prompt;
        for expected in [
            "QUESTION id 1\nFIX:\n- choices[0].text is 38 characters; the limit is 31.\nQUESTION JSON:\n",
            "QUESTION id 2\nFIX:\n- choices[1].text is 42 characters; the limit is 31.\nQUESTION JSON:\n",
            "- \"q\" is 133 characters and wraps into 5 lines of 31; the limit is 4 lines (about 100 characters).",
            "- \"concept\" is \"architecture\", which is not a lens; use exactly one of purpose|responsibility|interaction|invariant|tradeoff.",
            "- choices[0].why is missing; write one of at most 90 characters.",
            "- choices[3].why is 132 characters and wraps into 5 lines of 34; the limit is 3 lines (about 90 characters).",
            "- choices[1].text contains '\u{2192}' (U+2192), which the display cannot draw; use plain ASCII.",
            "Fix ONLY the fields listed under FIX",
            "the same answer index",
            "Never add file names, paths, versions, or dates",
            "each keeping its \"id\"",
        ] {
            assert!(prompt.contains(expected), "missing {expected:?} in\n{prompt}");
        }
        assert!(
            !prompt.contains("WHICH FILE OWNS THE GAME LOOP"),
            "trivia is not repaired"
        );
        assert!(
            !prompt.contains("WHY KEEP SAVES VERSIONED"),
            "malformed is not repaired"
        );
        assert!(!prompt.contains("WHY DOES THE ENGINE OWN EVERY GAME RULE"));

        // Each question travels in the generation schema plus its id, with
        // every field it keeps unchanged.
        let sent = prompt
            .split("QUESTION JSON:\n")
            .skip(1)
            .map(|rest| {
                let line = rest.lines().next().unwrap();
                serde_json::from_str::<serde_json::Value>(line).unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(sent.len(), 3);
        for (index, (value, original)) in sent.iter().zip(&request.originals).enumerate() {
            assert_eq!(value["id"], index + 1);
            assert_eq!(value["q"], original.q.as_str());
            assert_eq!(value["answer"], original.answer);
            assert_eq!(value["choices"].as_array().unwrap().len(), 4);
        }
        assert_eq!(
            serde_json::from_value::<QQuestion>(sent[1].clone()).unwrap(),
            request.originals[1],
            "an explained question round-trips exactly"
        );
        assert_eq!(
            sent[2]["choices"][0],
            serde_json::json!({ "text": "DECIDE GAME RULES ITSELF", "why": "" }),
            "a plain choice shows where its rationale goes"
        );
    }

    #[test]
    fn a_repair_is_requested_only_for_a_short_batch_with_mechanical_rejections() {
        let batch = over_length_batch();
        assert!(batch.repair_request(1).is_none(), "already full");
        assert_eq!(
            batch.repair_request(2).unwrap().originals.len(),
            2,
            "capped"
        );

        let unrepairable = GeneratedBatch {
            accepted: Vec::new(),
            rejected: batch.rejected[3..].to_vec(),
        };
        assert!(unrepairable.repair_request(6).is_none());

        // A rejection repeating an accepted question, or another rejection,
        // is not sent: its repair could only duplicate.
        let mut repeated = batch.rejected[0].clone();
        repeated.q = batch.accepted[0].q.clone();
        let duplicates = GeneratedBatch {
            accepted: batch.accepted.clone(),
            rejected: vec![
                repeated,
                batch.rejected[1].clone(),
                batch.rejected[1].clone(),
            ],
        };
        assert_eq!(
            duplicates.repair_request(6).unwrap().originals,
            [batch.rejected[1].clone()]
        );
    }

    #[test]
    fn merged_repairs_keep_their_answer_and_never_exceed_the_count_or_repeat() {
        let mut batch = over_length_batch();
        let request = batch.repair_request(6).unwrap();
        let reply = r#"Here are the fixes:
        [
            {"id": 1, "q": "What happens when a provider reply mixes valid and invalid questions?", "concept": "interaction", "choices": [
                {"text": "Valid ones survive alone", "why": "Each question is judged alone, so good ones are kept."},
                {"text": "The whole reply is discarded", "why": "Misconception: one bad question does not sink the rest."},
                {"text": "Bad questions are cut to fit", "why": "Misconception: over-long text is rejected, never cut."},
                {"text": "A filler deck replaces them", "why": "Misconception: the game has no filler question deck."}
            ], "answer": 0},
            {"id": 1, "q": "Why are mixed replies kept in part?", "concept": "interaction", "choices": [
                {"text": "Each is judged alone", "why": "A second repair of the same question is ignored."},
                {"text": "B", "why": "B."}, {"text": "C", "why": "C."}, {"text": "D", "why": "D."}
            ], "answer": 0},
            {"id": 2, "q": "Why does the engine own every game rule?", "concept": "invariant", "choices": [
                {"text": "Running code would be too slow", "why": "Misconception: speed is not why loaded code stays inert."},
                {"text": "Only its own handlers act", "why": "Only the app's own handlers act, so a hostile project cannot."},
                {"text": "The shell cannot start programs", "why": "Misconception: the shell could, but loading never asks it to."},
                {"text": "Saves would grow too large", "why": "Misconception: save size has nothing to do with running code."}
            ], "answer": 1},
            {"id": 2, "q": "Why does loading a cartridge never run its code?", "concept": "invariant", "choices": [
                {"text": "Only its own handlers act", "why": "Only the app's own handlers act, so a hostile project cannot."},
                {"text": "Running code would be too slow", "why": "Misconception: speed is not why loaded code stays inert."},
                {"text": "The shell cannot start programs", "why": "Misconception: the shell could, but loading never asks it to."},
                {"text": "Saves would grow too large", "why": "Misconception: save size has nothing to do with running code."}
            ], "answer": 0},
            {"id": 2, "q": "Why does loading a cartridge never run its code?", "concept": "invariant", "choices": [
                {"text": "Running code would be too slow", "why": "Misconception: speed is not why loaded code stays inert."},
                {"text": "Only its own handlers act", "why": "Only the app's own handlers act, so a hostile project cannot."},
                {"text": "The shell cannot start programs", "why": "Misconception: the shell could, but loading never asks it to."},
                {"text": "Saves would grow too large", "why": "Misconception: save size has nothing to do with running code."}
            ], "answer": 1},
            {"id": 3, "q": "What must the shell never do?", "concept": "architecture", "choices": [
                {"text": "Decide game rules itself", "why": "Rules belong to the engine alone."},
                {"text": "Draw frames", "why": "Misconception: drawing frames is the shell's job."},
                {"text": "Forward player input", "why": "Misconception: the shell must forward input."},
                {"text": "Play the engine's notes", "why": "Misconception: the shell plays what the engine emits."}
            ], "answer": 0},
            {"id": 7, "q": "Why is this question new?", "concept": "purpose", "choices": [
                {"text": "It was never requested", "why": "Unrequested questions have no brief behind them."},
                {"text": "B", "why": "B."}, {"text": "C", "why": "C."}, {"text": "D", "why": "D."}
            ], "answer": 0},
            {"q": "Why has this question no id?", "concept": "purpose", "choices": [
                {"text": "It forgot its id", "why": "Repairs must say which question they fix."},
                {"text": "B", "why": "B."}, {"text": "C", "why": "C."}, {"text": "D", "why": "D."}
            ], "answer": 0}
        ]"#;

        let mut capped = batch.clone();
        assert_eq!(capped.merge_repairs(&request, reply, 2), 1);
        assert_eq!(capped.accepted.len(), 2, "never beyond the count");

        assert_eq!(batch.merge_repairs(&request, reply, 6), 2);
        let accepted = batch
            .accepted
            .iter()
            .map(|question| {
                (
                    question.q.as_str(),
                    question.choices[question.answer].text(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            accepted,
            [
                (
                    "WHY DOES THE ENGINE OWN EVERY GAME RULE?",
                    "SO THE SHELL ONLY DRAWS FRAMES"
                ),
                (
                    "WHAT HAPPENS WHEN A PROVIDER REPLY MIXES VALID AND INVALID QUESTIONS?",
                    "VALID ONES SURVIVE ALONE"
                ),
                (
                    "WHY DOES LOADING A CARTRIDGE NEVER RUN ITS CODE?",
                    "ONLY ITS OWN HANDLERS ACT"
                ),
            ],
            "one repair per id; repeats, moved answers, failed fixes, and unrequested questions drop"
        );
        assert!(batch.accepted.iter().all(generated_question_is_acceptable));
        assert_eq!(
            batch.accepted.len(),
            batch
                .accepted
                .iter()
                .map(|question| question_identity(&question.q))
                .collect::<HashSet<_>>()
                .len()
        );

        let mut unchanged = over_length_batch();
        assert_eq!(unchanged.merge_repairs(&request, "no json here", 6), 0);
        assert_eq!(unchanged.merge_repairs(&request, "[not json]", 6), 0);
        assert_eq!(unchanged, over_length_batch());
    }

    #[test]
    fn a_batch_with_no_accepted_questions_is_an_error_until_repaired() {
        let mut batch = over_length_batch();
        batch.accepted.clear();
        assert_eq!(
            batch.clone().into_questions().unwrap_err(),
            "INCOMPLETE OR INVALID QUESTIONS"
        );
        let request = batch.repair_request(6).unwrap();
        let fixed = serde_json::to_string(&serde_json::json!([{
            "id": 1,
            "q": request.originals[0].q,
            "concept": "interaction",
            "choices": request.originals[0].choices.iter().enumerate().map(|(index, choice)| {
                let text = if index == 0 { "VALID ONES SURVIVE" } else { choice.text() };
                serde_json::json!({ "text": text, "why": choice.why() })
            }).collect::<Vec<_>>(),
            "answer": 0,
        }]))
        .unwrap();
        assert_eq!(batch.merge_repairs(&request, &fixed, 6), 1);
        assert_eq!(batch.into_questions().unwrap().len(), 1);
    }
}
