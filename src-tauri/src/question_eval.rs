//! Live measurement of question-generation quality against a real repository
//! and the real provider CLI.
//!
//! [`live_question_generation_quality`] follows the app's generation path
//! exactly: the anonymized [`repo_context::project_brief`], the bounded prompt
//! from [`questions::bounded_ai_question_prompt`], the stdin provider call
//! behind [`crate::ask_provider`], and [`questions::parse_generated_batch`].
//! It prints every candidate the provider returned as ACCEPT or REJECT with the
//! limits it violated, then answer-position, lens, focus, and PREDICT tallies
//! and the acceptance rate. The test is ignored by default because it spends
//! provider quota; `scripts/eval-questions.sh` runs it.
//!
//! Configuration comes from the environment:
//!
//! - `CQ_EVAL_REPO`: the repository to brief (default: this checkout).
//! - `CQ_EVAL_PROVIDER`: `claude` or `codex` (default `claude`).
//! - `CQ_EVAL_LEVELS`: comma-separated levels, one request each (default `1,4`).
//! - `CQ_EVAL_ROUNDS`: requests per level (default 1).
//! - `CQ_EVAL_COUNT`: questions per request (default 6, the engine's batch).
//! - `CQ_EVAL_TIMEOUT_SECS`: limit per request (default 120, the app's).
//! - `CQ_EVAL_LEARNER=save`: send the cartridge save's learner state, as the
//!   app does, instead of a new player's.
//!
//! The provider binary and model come from the app's own `CQA_CLAUDE`,
//! `CQA_CODEX`, `CQA_CLAUDE_MODEL`, and `CQA_CODEX_MODEL`.
//!
//! The acceptance verdict for each candidate is the app's own: the candidate is
//! parsed alone by [`questions::parse_generated_batch`]. Only the reasons
//! are diagnosed here, so a policy change cannot change which candidates this
//! tool accepts; at worst it leaves a rejection unexplained or faults an
//! accepted candidate, and the report flags both. Batch assembly (each
//! question once, at most the requested count) is mirrored, and every request
//! checks the mirrored count against the app's.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::learning::{self, Concept};
use crate::questions::{self, LearnerState, QChoice, QQuestion};
use crate::{engine, repo_context, AiProvider};

/// Questions per request in the app: the engine's batch size.
const DEFAULT_COUNT: usize = 6;
/// The app's limit for one generation request.
const DEFAULT_TIMEOUT_SECS: u64 = 120;
/// Choices every quiz question shows.
const CHOICES: usize = 4;

/// Where a text sits inside a question, for naming violations.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Field {
    Question,
    Choice(usize),
    Why(usize),
}

impl Field {
    fn label(self) -> String {
        match self {
            Self::Question => "QUESTION".to_string(),
            Self::Choice(index) => format!("CHOICE {}", index + 1),
            Self::Why(index) => format!("WHY {}", index + 1),
        }
    }

    /// Whether `text` meets this field's own display limit, so any remaining
    /// rejection of it is about content.
    fn fits(self, text: &str) -> bool {
        match self {
            Self::Question => {
                !text.trim().is_empty()
                    && engine::wrap_text(text, engine::QUIZ_QUESTION_COLUMNS).len()
                        <= engine::QUIZ_QUESTION_ROWS
            }
            Self::Choice(_) => {
                !text.trim().is_empty() && text.chars().count() <= engine::QUIZ_CHOICE_CHARS
            }
            Self::Why(_) => learning::rationale_fits(text),
        }
    }

    /// The reference question with this field replaced by `text`. A choice
    /// replaces the reference choice it matches, if any, so the probe never
    /// fails only because two of its choices read the same.
    fn probe(self, text: &str) -> QQuestion {
        let mut probe = reference_question();
        match self {
            Self::Question => probe.q = text.to_string(),
            Self::Choice(_) => {
                let slot = probe
                    .choices
                    .iter()
                    .position(|choice| choice.text().eq_ignore_ascii_case(text.trim()))
                    .unwrap_or(0);
                if let QChoice::Explained { text: choice, .. } = &mut probe.choices[slot] {
                    *choice = text.to_string();
                }
            }
            Self::Why(_) => {
                if let QChoice::Explained { why, .. } = &mut probe.choices[0] {
                    *why = text.to_string();
                }
            }
        }
        probe
    }
}

/// One limit a candidate violates. `kind` groups violations for the tally;
/// `detail` names the field and the measured value.
#[derive(Clone, Debug, PartialEq)]
struct Violation {
    kind: &'static str,
    detail: String,
}

fn violation(kind: &'static str, detail: String) -> Violation {
    Violation { kind, detail }
}

/// A question the acceptance policy keeps, used to test one field at a time:
/// replacing a single field and seeing the policy reject it isolates that
/// field as the cause.
fn reference_question() -> QQuestion {
    let why = "IT NAMES THE DUTY THE DESIGN GIVES THIS PART.";
    QQuestion {
        q: "WHAT DOES THE CORE OWN?".to_string(),
        concept: Some("responsibility".to_string()),
        choices: [
            "THE CORE RULES",
            "THE OUTER SKIN",
            "THE PAINT LAYER",
            "THE SOUND DESK",
        ]
        .iter()
        .map(|text| QChoice::Explained {
            text: (*text).to_string(),
            why: why.to_string(),
        })
        .collect(),
        answer: 0,
    }
}

/// The app's verdict on `question` alone.
fn policy_accepts(question: &QQuestion) -> bool {
    serde_json::to_string(&[question]).is_ok_and(|json| parse_generated_questions(&json, 1).is_ok())
}

/// The app's verdict on one raw candidate alone.
fn policy_accepts_value(value: &serde_json::Value) -> bool {
    let single = serde_json::Value::Array(vec![value.clone()]).to_string();
    parse_generated_questions(&single, 1).is_ok()
}

/// Reads the complete values of an array that starts right after its opening
/// bracket, stopping at the first malformed or truncated element, as the app
/// does.
fn leading_values(after_bracket: &str) -> Vec<serde_json::Value> {
    let mut values = Vec::new();
    let mut rest = after_bracket;
    loop {
        rest = rest.trim_start();
        if rest.is_empty() || rest.starts_with(']') {
            return values;
        }
        let mut stream = serde_json::Deserializer::from_str(rest).into_iter::<serde_json::Value>();
        let Some(Ok(value)) = stream.next() else {
            return values;
        };
        values.push(value);
        let offset = stream.byte_offset();
        rest = rest[offset..].trim_start();
        match rest.strip_prefix(',') {
            Some(after_comma) => rest = after_comma,
            None => return values,
        }
    }
}

/// The raw candidates in a provider reply: the first array whose elements are
/// question objects, found inside prose and code fences as the app finds it.
fn candidate_values(reply: &str) -> Vec<serde_json::Value> {
    reply
        .match_indices('[')
        .map(|(index, _)| leading_values(&reply[index + 1..]))
        .find(|values| values.iter().any(|value| value.get("q").is_some()))
        .unwrap_or_default()
}

/// Display normalization the app applies before judging: typographic
/// punctuation folded to ASCII, whitespace collapsed, upper case.
fn display_form(text: &str) -> String {
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

fn normalized(question: QQuestion) -> QQuestion {
    QQuestion {
        q: display_form(&question.q),
        concept: question.concept.map(|concept| concept.trim().to_string()),
        choices: question
            .choices
            .into_iter()
            .map(|choice| match choice {
                QChoice::Plain(text) => QChoice::Plain(display_form(&text)),
                QChoice::Explained { text, why } => QChoice::Explained {
                    text: display_form(&text),
                    why: display_form(&why),
                },
            })
            .collect(),
        answer: question.answer,
    }
}

fn renderable(text: &str) -> bool {
    text.chars()
        .all(|character| (' '..='~').contains(&character))
}

/// Every text of `question` with the field it fills.
fn fields(question: &QQuestion) -> Vec<(Field, &str)> {
    let mut texts = vec![(Field::Question, question.q.as_str())];
    for (index, choice) in question.choices.iter().enumerate() {
        texts.push((Field::Choice(index), choice.text()));
        if let Some(why) = choice.why() {
            texts.push((Field::Why(index), why));
        }
    }
    texts
}

/// Every limit a normalized candidate violates, measured against the same
/// display constants and policy predicates the app uses.
fn violations(question: &QQuestion) -> Vec<Violation> {
    let mut found = Vec::new();
    let stem_lines = engine::wrap_text(&question.q, engine::QUIZ_QUESTION_COLUMNS).len();
    if question.q.trim().is_empty() {
        found.push(violation("EMPTY TEXT", "QUESTION IS EMPTY".to_string()));
    } else if stem_lines > engine::QUIZ_QUESTION_ROWS {
        found.push(violation(
            "QUESTION TOO LONG",
            format!(
                "QUESTION WRAPS TO {stem_lines} LINES ({} CHARS), LIMIT {} LINES OF {}",
                question.q.chars().count(),
                engine::QUIZ_QUESTION_ROWS,
                engine::QUIZ_QUESTION_COLUMNS
            ),
        ));
    }
    match question.concept.as_deref() {
        None => found.push(violation("LENS", "NO LENS".to_string())),
        Some(lens) if Concept::parse(lens).is_none() => {
            found.push(violation("LENS", format!("UNKNOWN LENS {lens:?}")));
        }
        Some(_) => {}
    }
    if question.choices.len() != CHOICES {
        found.push(violation(
            "CHOICE COUNT",
            format!("{} CHOICES, NEED {CHOICES}", question.choices.len()),
        ));
    }
    if question.answer >= question.choices.len() {
        found.push(violation(
            "ANSWER INDEX",
            format!(
                "ANSWER {} OUT OF RANGE FOR {} CHOICES",
                question.answer,
                question.choices.len()
            ),
        ));
    }
    let distinct = question
        .choices
        .iter()
        .map(|choice| choice.text().trim().to_ascii_uppercase())
        .collect::<HashSet<_>>();
    if distinct.len() < question.choices.len() {
        found.push(violation(
            "DUPLICATE CHOICES",
            "TWO CHOICES READ THE SAME".to_string(),
        ));
    }
    for (index, choice) in question.choices.iter().enumerate() {
        let label = Field::Choice(index).label();
        let chars = choice.text().chars().count();
        if choice.text().trim().is_empty() {
            found.push(violation("EMPTY TEXT", format!("{label} IS EMPTY")));
        } else if chars > engine::QUIZ_CHOICE_CHARS {
            found.push(violation(
                "CHOICE TOO LONG",
                format!(
                    "{label} IS {chars} CHARS, LIMIT {}",
                    engine::QUIZ_CHOICE_CHARS
                ),
            ));
        }
        match choice.why() {
            None => found.push(violation("NO WHY", format!("{label} HAS NO WHY"))),
            Some(why) if why.trim().is_empty() => found.push(violation(
                "EMPTY TEXT",
                format!("{} IS EMPTY", Field::Why(index).label()),
            )),
            Some(why) if !learning::rationale_fits(why) => found.push(violation(
                "WHY TOO LONG",
                format!(
                    "{} WRAPS TO {} LINES ({} CHARS), LIMIT {} LINES OF {}",
                    Field::Why(index).label(),
                    engine::wrap_text(why, learning::RATIONALE_COLUMNS).len(),
                    why.chars().count(),
                    learning::RATIONALE_ROWS,
                    learning::RATIONALE_COLUMNS
                ),
            )),
            Some(_) => {}
        }
    }
    let probing = policy_accepts(&reference_question());
    for (field, text) in fields(question) {
        let label = field.label();
        if !renderable(text) {
            found.push(violation("NON-ASCII", format!("NON-ASCII TEXT IN {label}")));
        } else if let Some(word) = text
            .split_whitespace()
            .find(|word| questions::is_location_word(word))
        {
            found.push(violation(
                "LOCATION",
                format!("LOCATION {word:?} IN {label}"),
            ));
        } else if probing && field.fits(text) && !policy_accepts(&field.probe(text)) {
            let detail = match trivia_trigger(field, text) {
                Some(words) => format!("TRIVIA {words:?} IN {label}"),
                None => format!("TRIVIA IN {label}"),
            };
            found.push(violation("TRIVIA", detail));
        }
    }
    found
}

/// The words of `text` that make the policy call it trivia: each word whose
/// removal alone lets the field pass, in order. A phrase such as PULL REQUESTS
/// is named whole because dropping either word breaks it.
fn trivia_trigger(field: Field, text: &str) -> Option<String> {
    let words = text.split_whitespace().collect::<Vec<_>>();
    let trigger = (0..words.len())
        .filter(|&skipped| {
            let rest = words
                .iter()
                .enumerate()
                .filter(|&(index, _)| index != skipped)
                .map(|(_, word)| *word)
                .collect::<Vec<_>>()
                .join(" ");
            field.fits(&rest) && policy_accepts(&field.probe(&rest))
        })
        .map(|index| words[index])
        .collect::<Vec<_>>();
    (!trigger.is_empty()).then(|| trigger.join(" "))
}

/// What happened to one raw candidate when its batch was accepted.
#[derive(Debug)]
struct Judged {
    question: Option<QQuestion>,
    kept: bool,
    violations: Vec<Violation>,
    /// The policy kept a candidate this diagnosis faults; the diagnosis is out
    /// of date with the policy.
    disputed: bool,
}

/// Judges a reply's candidates as the app accepts a batch: each acceptable
/// question once, in order, at most `count`.
fn judge_reply(reply: &str, count: usize) -> Vec<Judged> {
    let mut seen = HashSet::new();
    let mut kept = 0;
    candidate_values(reply)
        .into_iter()
        .map(|value| {
            let accepted = policy_accepts_value(&value);
            let question = match serde_json::from_value::<QQuestion>(value) {
                Ok(question) => normalized(question),
                Err(error) => {
                    return Judged {
                        question: None,
                        kept: false,
                        violations: vec![violation(
                            "NOT A QUESTION",
                            format!("NOT A QUESTION OBJECT: {error}"),
                        )],
                        disputed: false,
                    };
                }
            };
            let mut found = violations(&question);
            let disputed = accepted && !found.is_empty();
            if accepted {
                found.clear();
                if !seen.insert(questions::question_identity(&question.q)) {
                    found.push(violation(
                        "REPEATED QUESTION",
                        "SAME QUESTION AS AN EARLIER CANDIDATE".to_string(),
                    ));
                } else if kept >= count {
                    found.push(violation(
                        "BEYOND COUNT",
                        format!("BEYOND THE {count} REQUESTED"),
                    ));
                }
            } else if found.is_empty() {
                found.push(violation(
                    "UNISOLATED",
                    "REJECTED BY THE POLICY; NO SINGLE FIELD ISOLATES THE CAUSE".to_string(),
                ));
            }
            let is_kept = found.is_empty();
            kept += usize::from(is_kept);
            Judged {
                question: Some(question),
                kept: is_kept,
                violations: found,
                disputed,
            }
        })
        .collect()
}

fn lens_name(question: &QQuestion) -> String {
    match (question.lens(), question.concept.as_deref()) {
        (Some(lens), _) => format!("{lens:?}").to_ascii_lowercase(),
        (None, Some(raw)) => format!("?{raw}"),
        (None, None) => "none".to_string(),
    }
}

fn is_predict(question: &QQuestion) -> bool {
    question.q.starts_with("PREDICT")
}

/// Questions per batch that must use the level's focus lenses, as the prompt
/// asks: two thirds, rounded up.
fn focus_minimum(count: usize) -> usize {
    (count * 2).div_ceil(3)
}

fn percent(part: usize, whole: usize) -> String {
    if whole == 0 {
        return "n/a".to_string();
    }
    format!("{:.1}%", part as f64 * 100.0 / whole as f64)
}

fn histogram<K: std::fmt::Display>(counts: &BTreeMap<K, usize>) -> String {
    if counts.is_empty() {
        return "(none)".to_string();
    }
    counts
        .iter()
        .map(|(key, count)| format!("{key}={count}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Running totals across every request of one evaluation.
#[derive(Default)]
struct Tally {
    requests: usize,
    failures: Vec<String>,
    seconds: Vec<f64>,
    requested: usize,
    candidates: usize,
    kept: usize,
    app_kept: usize,
    positions_all: BTreeMap<usize, usize>,
    positions_kept: BTreeMap<usize, usize>,
    lenses_kept: BTreeMap<String, usize>,
    focus_kept: usize,
    focus_all: usize,
    focus_asked: usize,
    predict_kept: usize,
    predict_all: usize,
    predict_asked: usize,
    rejections: BTreeMap<&'static str, usize>,
    disputed: usize,
}

impl Tally {
    fn record(&mut self, level: u32, count: usize, judged: &[Judged], app_kept: usize) {
        self.requested += count;
        self.candidates += judged.len();
        self.app_kept += app_kept;
        self.focus_asked += focus_minimum(count);
        if level >= 4 {
            self.predict_asked += count.min(2);
        }
        let focus = Concept::focus_for_level(level);
        for item in judged {
            self.disputed += usize::from(item.disputed);
            let Some(question) = &item.question else {
                self.reject(item);
                continue;
            };
            let in_focus = usize::from(question.lens().is_some_and(|lens| focus.contains(&lens)));
            let predict = usize::from(is_predict(question));
            *self.positions_all.entry(question.answer).or_default() += 1;
            self.focus_all += in_focus;
            self.predict_all += predict;
            if !item.kept {
                self.reject(item);
                continue;
            }
            self.kept += 1;
            *self.positions_kept.entry(question.answer).or_default() += 1;
            *self.lenses_kept.entry(lens_name(question)).or_default() += 1;
            self.focus_kept += in_focus;
            self.predict_kept += predict;
        }
    }

    /// Counts each kind of violation once per rejected candidate.
    fn reject(&mut self, item: &Judged) {
        let kinds = item
            .violations
            .iter()
            .map(|violation| violation.kind)
            .collect::<HashSet<_>>();
        for kind in kinds {
            *self.rejections.entry(kind).or_default() += 1;
        }
    }
}

fn print_candidate(number: usize, item: &Judged) {
    let verdict = if item.kept { "ACCEPT" } else { "REJECT" };
    let Some(question) = &item.question else {
        println!("  {verdict} #{number}");
        for found in &item.violations {
            println!("         x {}", found.detail);
        }
        return;
    };
    println!(
        "  {verdict} #{number} [{}] answer={} {:?}",
        lens_name(question),
        question.answer,
        question.q
    );
    for (index, choice) in question.choices.iter().enumerate() {
        let mark = if index == question.answer { '*' } else { ' ' };
        println!(
            "         {mark}{} {:?} -- {}",
            index + 1,
            choice.text(),
            choice.why().unwrap_or("(no why)")
        );
    }
    for found in &item.violations {
        println!("         x {}", found.detail);
    }
    if item.disputed {
        println!("         ! THE POLICY KEPT THIS, BUT THIS DIAGNOSIS FAULTS IT; UPDATE question_eval.rs");
    }
}

fn environment(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parsed_environment<T: std::str::FromStr>(name: &str, default: T) -> T {
    environment(name).map_or(default, |value| {
        value
            .parse()
            .unwrap_or_else(|_| panic!("{name} must be a number, not {value:?}"))
    })
}

fn default_repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri lives inside the CODE QUEST ADVANCE checkout")
        .to_path_buf()
}

#[test]
#[ignore = "calls the real provider CLI"]
fn live_question_generation_quality() {
    let repo = environment("CQ_EVAL_REPO").map_or_else(default_repository, PathBuf::from);
    let repo = std::fs::canonicalize(&repo)
        .unwrap_or_else(|error| panic!("CQ_EVAL_REPO {}: {error}", repo.display()));
    assert!(
        crate::git_repo_check_within(&repo, crate::GIT_TIMEOUT).unwrap_or(false),
        "CQ_EVAL_REPO {} is not a git repository",
        repo.display()
    );
    let provider =
        AiProvider::parse(&environment("CQ_EVAL_PROVIDER").unwrap_or_else(|| "claude".to_string()))
            .expect("CQ_EVAL_PROVIDER must be claude or codex");
    let levels = environment("CQ_EVAL_LEVELS")
        .unwrap_or_else(|| "1,4".to_string())
        .split(',')
        .map(|level| {
            level
                .trim()
                .parse::<u32>()
                .unwrap_or_else(|_| panic!("CQ_EVAL_LEVELS holds a non-number {level:?}"))
        })
        .collect::<Vec<_>>();
    let rounds = parsed_environment("CQ_EVAL_ROUNDS", 1usize);
    let count = parsed_environment("CQ_EVAL_COUNT", DEFAULT_COUNT);
    let timeout = Duration::from_secs(parsed_environment(
        "CQ_EVAL_TIMEOUT_SECS",
        DEFAULT_TIMEOUT_SECS,
    ));
    let model_variable = match provider {
        AiProvider::Claude => "CQA_CLAUDE_MODEL",
        AiProvider::Codex => "CQA_CODEX_MODEL",
    };
    let model = environment(model_variable).unwrap_or_else(|| "(cli default)".to_string());
    let learner = if environment("CQ_EVAL_LEARNER").as_deref() == Some("save") {
        questions::load_learner_state(&repo).unwrap_or_default()
    } else {
        LearnerState::default()
    };

    // The app's generation path, as in `ai_questions`.
    let name = repo
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    let brief = repo_context::project_brief(&repo, &crate::tracked_files(&repo));
    println!(
        "EVAL provider={} model={model} repo={} levels={levels:?} rounds={rounds} count={count}",
        provider.name(),
        repo.display()
    );
    println!(
        "BRIEF {} bytes of {} budget, {} components, {} design notes, readme={}",
        brief.len(),
        repo_context::BRIEF_BUDGET,
        brief.matches("--- COMPONENT ").count(),
        brief
            .lines()
            .filter(|line| line.starts_with("DESIGN NOTE "))
            .count(),
        brief.contains("README EXCERPT:")
    );

    let mut tally = Tally::default();
    for level in &levels {
        for round in 1..=rounds {
            let prompt =
                questions::bounded_ai_question_prompt(&name, *level, count, &brief, &learner);
            let started = Instant::now();
            let reply = crate::ask_provider(provider, &prompt, timeout);
            let seconds = started.elapsed().as_secs_f64();
            tally.requests += 1;
            let reply = match reply {
                Ok(reply) => reply,
                Err(error) => {
                    println!(
                        "== LEVEL {level} ROUND {round}: prompt {} bytes, FAILED after {seconds:.1} s: {error}",
                        prompt.len()
                    );
                    tally.failures.push(error);
                    continue;
                }
            };
            tally.seconds.push(seconds);
            let judged = judge_reply(&reply, count);
            let app_kept = parse_generated_questions(&reply, count)
                .map(|batch| batch.len())
                .unwrap_or(0);
            let kept = judged.iter().filter(|item| item.kept).count();
            println!(
                "== LEVEL {level} ROUND {round}: prompt {} bytes, reply {} bytes in {seconds:.1} s, {} candidates, {kept} kept (app kept {app_kept})",
                prompt.len(),
                reply.len(),
                judged.len()
            );
            if judged.is_empty() {
                let head = reply.chars().take(400).collect::<String>();
                println!("  NO QUESTION ARRAY IN REPLY; it begins: {head:?}");
            }
            if kept != app_kept {
                println!("  ! THIS TOOL KEPT {kept} BUT THE APP KEPT {app_kept}; UPDATE question_eval.rs");
            }
            for (index, item) in judged.iter().enumerate() {
                print_candidate(index + 1, item);
            }
            tally.record(*level, count, &judged, app_kept);
        }
    }

    let answered = tally.seconds.len();
    let mean = if answered == 0 {
        0.0
    } else {
        tally.seconds.iter().sum::<f64>() / answered as f64
    };
    println!("== SUMMARY provider={} model={model}", provider.name());
    println!(
        "REQUESTS {answered} of {} answered, {} failed, mean {mean:.1} s",
        tally.requests,
        tally.failures.len()
    );
    for failure in &tally.failures {
        println!("  FAILURE {failure}");
    }
    println!(
        "ACCEPTANCE {} of {} candidates ({}); {} of {} requested questions filled ({}); app kept {}",
        tally.kept,
        tally.candidates,
        percent(tally.kept, tally.candidates),
        tally.kept,
        tally.requested,
        percent(tally.kept, tally.requested),
        tally.app_kept
    );
    println!(
        "ANSWER POSITIONS kept: {} | all: {}",
        histogram(&tally.positions_kept),
        histogram(&tally.positions_all)
    );
    println!("LENSES kept: {}", histogram(&tally.lenses_kept));
    println!(
        "FOCUS kept {} | all {} candidates use their level's focus lenses (prompts asked for at least {})",
        tally.focus_kept, tally.focus_all, tally.focus_asked
    );
    println!(
        "PREDICT kept {} | all {} (prompts at level 4+ asked for at least {})",
        tally.predict_kept, tally.predict_all, tally.predict_asked
    );
    println!("REJECTIONS {}", histogram(&tally.rejections));
    if tally.disputed > 0 {
        println!(
            "DISPUTED {} kept candidates this diagnosis faults; update question_eval.rs",
            tally.disputed
        );
    }
    assert!(
        answered > 0,
        "every provider request failed: {:?}",
        tally.failures
    );
}

#[cfg(test)]
/// The app's acceptance of one provider reply: the accepted questions, or an
/// error when none survive (as `ai_questions` reports it before any repair).
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

mod tests {
    use super::*;

    const WHY: &str = "IT NAMES THE DUTY THE DESIGN GIVES THIS PART.";

    fn candidate(q: &str, concept: &str, choices: &[(&str, &str)], answer: usize) -> String {
        serde_json::json!({
            "q": q,
            "concept": concept,
            "choices": choices
                .iter()
                .map(|(text, why)| serde_json::json!({ "text": text, "why": why }))
                .collect::<Vec<_>>(),
            "answer": answer,
        })
        .to_string()
    }

    fn four(texts: [&'static str; 4]) -> Vec<(&'static str, &'static str)> {
        texts.iter().map(|text| (*text, WHY)).collect()
    }

    fn kinds(item: &Judged) -> Vec<&'static str> {
        item.violations.iter().map(|found| found.kind).collect()
    }

    #[test]
    fn the_reference_question_passes_the_acceptance_policy() {
        assert!(policy_accepts(&reference_question()));
        assert!(violations(&reference_question()).is_empty());
    }

    #[test]
    fn a_trivia_phrase_is_named_whole() {
        let mut question = reference_question();
        question.choices[1] = QChoice::Explained {
            text: "COUNTS LINES OF CODE".to_string(),
            why: WHY.to_string(),
        };

        assert!(!policy_accepts(&question));
        assert_eq!(
            violations(&question),
            [violation(
                "TRIVIA",
                "TRIVIA \"LINES OF CODE\" IN CHOICE 2".to_string()
            )]
        );
    }

    #[test]
    fn diagnosis_names_each_violated_limit_and_agrees_with_the_policy() {
        let long_stem = "WHAT DOES THE CORE OWN WHEN EVERY PART OF THE DESIGN MUST AGREE ABOUT WHICH SIDE KEEPS THE RULES AND WHICH SIDE ONLY DRAWS THEM?";
        let long_why = "THIS RATIONALE KEEPS GOING WELL PAST THE THREE LINES THAT THE LESSON PANEL CAN HOLD, SO THE PLAYER WOULD NEVER SEE ITS END.";
        let reply = format!(
            "[{}]",
            [
                candidate(
                    "What does the core own?",
                    "Roles",
                    &four([
                        "The core rules",
                        "The outer skin",
                        "The paint layer",
                        "The sound desk"
                    ]),
                    2,
                ),
                candidate(
                    long_stem,
                    "responsibility",
                    &four([
                        "The core rules",
                        "The outer skin",
                        "The paint layer",
                        "The sound desk"
                    ]),
                    0,
                ),
                candidate(
                    "What does the core own?",
                    "vibes",
                    &four([
                        "The core rules",
                        "The outer skin",
                        "The paint layer",
                        "The sound desk"
                    ])[..3],
                    0,
                ),
                candidate(
                    "Where is the engine started?",
                    "responsibility",
                    &four([
                        "The core rules",
                        "The outer skin",
                        "The paint layer",
                        "The sound desk"
                    ]),
                    0,
                ),
                candidate(
                    "What does the core own?",
                    "responsibility",
                    &[
                        ("The core rules", WHY),
                        ("The rules in src/engine.rs", WHY),
                        ("The paint layer", long_why),
                        ("Rules as of 2024", WHY),
                    ],
                    0,
                ),
            ]
            .join(",")
        );

        let judged = judge_reply(&reply, 6);

        assert_eq!(judged.len(), 5);
        assert!(judged[0].kept, "{:?}", judged[0].violations);
        assert_eq!(judged[0].question.as_ref().unwrap().answer, 2);
        assert!(!judged[1].kept);
        assert_eq!(kinds(&judged[1]), ["QUESTION TOO LONG"]);
        assert!(!judged[2].kept);
        assert_eq!(kinds(&judged[2]), ["LENS", "CHOICE COUNT"]);
        assert!(!judged[3].kept);
        assert_eq!(kinds(&judged[3]), ["TRIVIA"]);
        assert_eq!(
            judged[3].violations[0].detail,
            "TRIVIA \"WHERE IS\" IN QUESTION"
        );
        assert!(!judged[4].kept);
        let details = judged[4]
            .violations
            .iter()
            .map(|found| found.detail.as_str())
            .collect::<Vec<_>>();
        assert!(details
            .iter()
            .any(|detail| detail.starts_with("WHY 3 WRAPS TO")));
        assert!(details.contains(&"LOCATION \"SRC/ENGINE.RS\" IN CHOICE 2"));
        assert!(details.contains(&"TRIVIA \"2024\" IN CHOICE 4"));
        assert!(judged.iter().all(|item| !item.disputed));

        let app_kept = parse_generated_questions(&reply, 6).unwrap().len();
        assert_eq!(judged.iter().filter(|item| item.kept).count(), app_kept);
    }

    #[test]
    fn candidates_are_found_and_kept_the_way_the_app_keeps_a_batch() {
        let good = candidate(
            "What does the core own?",
            "responsibility",
            &four([
                "The core rules",
                "The outer skin",
                "The paint layer",
                "The sound desk",
            ]),
            0,
        );
        let other = candidate(
            "Why does the shell hold no state?",
            "tradeoff",
            &four([
                "It stays replaceable",
                "It runs faster",
                "It saves memory",
                "It hides errors",
            ]),
            1,
        );
        let reply = format!(
            "Here are the questions [1]:\n```json\n[{good}, {good}, {other}, {{\"q\": \"cut off"
        );

        let judged = judge_reply(&reply, 1);

        assert_eq!(judged.len(), 3);
        assert!(judged[0].kept);
        assert_eq!(kinds(&judged[1]), ["REPEATED QUESTION"]);
        assert_eq!(kinds(&judged[2]), ["BEYOND COUNT"]);
        assert_eq!(parse_generated_questions(&reply, 1).unwrap().len(), 1);
        assert!(judge_reply("no questions here", 6).is_empty());
    }

    #[test]
    fn tallies_count_positions_lenses_focus_and_predict_questions() {
        let predict = candidate(
            "PREDICT: what breaks if saves skip the lock?",
            "invariant",
            &four([
                "Writes can interleave",
                "Nothing changes",
                "Saves get smaller",
                "Reads get slower",
            ]),
            3,
        );
        let tradeoff = candidate(
            "Why does the shell hold no state?",
            "tradeoff",
            &four([
                "It stays replaceable",
                "It runs faster",
                "It saves memory",
                "It hides errors",
            ]),
            0,
        );
        let long_predict = candidate(
            "PREDICT: what breaks if a batch skips validation?",
            "invariant",
            &four([
                "Oversized text reaches the screen",
                "Nothing changes",
                "Saves get smaller",
                "Reads get slower",
            ]),
            1,
        );
        let judged = judge_reply(
            &format!("[{predict},{tradeoff},{long_predict}, {{\"q\": 5}}]"),
            6,
        );
        let mut tally = Tally::default();

        tally.record(4, 6, &judged, 2);

        assert_eq!((tally.candidates, tally.kept, tally.app_kept), (4, 2, 2));
        assert_eq!(histogram(&tally.positions_kept), "0=1 3=1");
        assert_eq!(histogram(&tally.positions_all), "0=1 1=1 3=1");
        assert_eq!(histogram(&tally.lenses_kept), "invariant=1 tradeoff=1");
        assert_eq!(
            (tally.focus_kept, tally.focus_all, tally.focus_asked),
            (2, 3, 4)
        );
        assert_eq!(
            (tally.predict_kept, tally.predict_all, tally.predict_asked),
            (1, 2, 2)
        );
        assert_eq!(
            histogram(&tally.rejections),
            "CHOICE TOO LONG=1 NOT A QUESTION=1"
        );
    }
}
