//! The screen transcript: the engine's third output channel, beside the
//! framebuffer and the chip sound.
//!
//! The whole game is a canvas, so a screen reader cannot read it. Each tick
//! the engine describes, in plain sentences, what a sighted player can read
//! and act on right now: the question and its choices in display order, which
//! choice has focus, the lesson card, and the controls that work. The shell
//! only places the latest transcript in a polite live region.
//!
//! Every sentence is derived from the same state and helpers the renderers
//! use (the display order, the lesson composition, the feedback banner, the
//! Oracle status rules), so the transcript and the pixels cannot disagree.
//! Decorative motion, blinking prompts, and staged reveals are left out: the
//! transcript changes only when something the player can act on changes, so
//! the live region is not re-announced every tick.
//!
//! A new screen is announced whole. A small change on the same screen, such
//! as focus moving to another choice or a counter ticking up, is announced on
//! its own, so pressing Down reads the newly focused choice instead of the
//! whole question again.

use serde::Serialize;

use super::*;

/// How many publications the channel remembers, so a reader that polls a few
/// ticks late still gets the change relative to what it last presented.
const TRANSCRIPT_HISTORY: usize = 32;

/// The `engine_transcript` payload: what to announce and the sequence number
/// of the transcript it brings the reader up to.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct TranscriptUpdate {
    pub seq: u64,
    pub text: String,
}

#[derive(Debug)]
struct Publication {
    seq: u64,
    screen: Screen,
    sentences: Vec<String>,
}

/// The recent published transcripts, newest last. The sequence number
/// advances only when the words change, so a reader that remembers it hears
/// each change once.
#[derive(Debug, Default)]
pub(super) struct TranscriptChannel {
    seq: u64,
    history: VecDeque<Publication>,
}

impl TranscriptChannel {
    /// Publishes the screen's sentences if they differ from the latest
    /// transcript and reports whether the sequence number advanced.
    pub(super) fn publish(&mut self, screen: Screen, sentences: Vec<String>) -> bool {
        let latest = self
            .history
            .back()
            .map_or(&[][..], |publication| &publication.sentences[..]);
        if latest == sentences.as_slice() {
            return false;
        }
        self.seq += 1;
        self.history.push_back(Publication {
            seq: self.seq,
            screen,
            sentences,
        });
        if self.history.len() > TRANSCRIPT_HISTORY {
            self.history.pop_front();
        }
        true
    }

    /// What a reader that last presented `seq` should announce now, if
    /// anything was published since. A reader that is too far behind, or
    /// holds a sequence number this channel never issued, hears the whole
    /// current screen.
    pub(super) fn since(&self, seq: u64) -> Option<TranscriptUpdate> {
        let latest = self.history.back()?;
        if latest.seq == seq {
            return None;
        }
        let text = match self
            .history
            .iter()
            .find(|publication| publication.seq == seq)
        {
            Some(base) if base.screen == latest.screen => {
                announcement(&base.sentences, &latest.sentences)
            }
            _ => latest.sentences.join(" "),
        };
        Some(TranscriptUpdate {
            seq: latest.seq,
            text,
        })
    }
}

/// What changed between two transcripts of the same screen. Sentences that
/// appear at the end (a prompt arriving) are read alone; a few sentences that
/// change in place are read alone, minus the choice that merely lost focus.
/// A new heading, or a change to most of the screen, reads the whole screen.
fn announcement(old: &[String], new: &[String]) -> String {
    let whole = || new.join(" ");
    if new.len() > old.len() && new.starts_with(old) {
        return new[old.len()..].join(" ");
    }
    if new.len() != old.len() || old.first() != new.first() {
        return whole();
    }
    let changed: Vec<(&String, &String)> = old
        .iter()
        .zip(new)
        .filter(|(before, after)| before != after)
        .collect();
    if changed.len() * 2 > new.len() {
        return whole();
    }
    let gained: Vec<&str> = changed
        .iter()
        .filter(|(before, after)| !lost_focus(before, after))
        .map(|(_, after)| after.as_str())
        .collect();
    if gained.is_empty() {
        changed
            .iter()
            .map(|(_, after)| after.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        gained.join(" ")
    }
}

/// True when `after` is `before` with only its focus marker removed.
fn lost_focus(before: &str, after: &str) -> bool {
    before
        .strip_suffix(", selected.")
        .is_some_and(|stem| after.strip_suffix('.') == Some(stem))
}

impl GameEngine {
    /// The current screen and its transcript's sentences, for publication.
    pub(super) fn transcript_sentences(&self) -> (Screen, Vec<String>) {
        let state = self.app.world().resource::<GameState>();
        (state.screen, screen_sentences(state))
    }

    /// The whole transcript of the screen the engine is showing now.
    #[cfg(test)]
    pub(super) fn transcript(&self) -> String {
        screen_transcript(self.app.world().resource::<GameState>())
    }
}

/// The whole transcript of `state`'s screen, as a new screen is announced.
#[cfg(test)]
pub(super) fn screen_transcript(state: &GameState) -> String {
    screen_sentences(state).join(" ")
}

/// Describes the current screen in plain sentences, in reading order. The
/// device is silent while it is off or showing the boot plate.
fn screen_sentences(state: &GameState) -> Vec<String> {
    let mut out = Transcript::default();
    match state.screen {
        Screen::Off | Screen::Boot => {}
        Screen::Copyright => chronicle(&mut out, state),
        Screen::OpeningFanfare => opening(&mut out, state),
        Screen::Title => title(&mut out, state),
        Screen::QuizMenu => quiz_menu(&mut out, state),
        Screen::CharacterCreation => hero_creation(&mut out, state),
        Screen::Oracle => datafall(&mut out, state),
        Screen::Quiz => trial(&mut out, state),
        Screen::LevelUp => level_up(&mut out, state),
        Screen::GameOver => game_over(&mut out, state),
        Screen::Codex => codex(&mut out, state),
        Screen::QuestSelect => quest_select(&mut out, state),
        Screen::Battle => battle(&mut out, state),
        Screen::Victory => {
            out.say("Quest cleared");
            out.say("A, B, or Start returns to the quest list");
        }
        Screen::Defeat => {
            out.say("Game over. The quest failed");
            out.say("A, B, or Start returns to the quest list");
        }
    }
    out.finish()
}

/// Sentences in reading order.
#[derive(Default)]
struct Transcript(Vec<String>);

impl Transcript {
    /// Adds `text` as one sentence with collapsed whitespace, closing it with
    /// a period unless it already ends in terminal punctuation.
    fn say(&mut self, text: impl AsRef<str>) {
        let text = text
            .as_ref()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        match text.chars().last() {
            None => {}
            Some('.' | '!' | '?') => self.0.push(text),
            Some(_) => self.0.push(format!("{text}.")),
        }
    }

    fn finish(self) -> Vec<String> {
        self.0
    }
}

/// An uppercase HUD label as a spoken word: `ORACLE-BOUND` reads
/// `Oracle-bound`.
fn spoken(label: &str) -> String {
    let lower = label.to_ascii_lowercase();
    let mut chars = lower.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_ascii_uppercase().to_string() + chars.as_str()
    })
}

/// Question copy exactly as the trial and Codex panels wrap it.
fn shown_question(question: &str) -> String {
    wrap_text(question, QUIZ_QUESTION_COLUMNS)
        .into_iter()
        .take(QUIZ_QUESTION_ROWS)
        .collect::<Vec<_>>()
        .join(" ")
}

fn shown_choice(choice: &str) -> String {
    truncate(choice, QUIZ_CHOICE_CHARS)
}

fn skip_prompt(out: &mut Transcript, state: &GameState) {
    if state.can_signal(SceneSignal::Continue) {
        out.say("A or Start skips");
    }
}

/// The chronicle card's credits, all at once: the plate reveals them over a
/// second, but a screen reader should not hear the card three times.
fn chronicle(out: &mut Transcript, state: &GameState) {
    out.say("Repository chronicle");
    let Some(cartridge) = state.cartridge.as_ref() else {
        out.say("No cartridge");
        return;
    };
    out.say(&cartridge.title);
    let provenance = &cartridge.provenance;
    out.say(
        provenance
            .copyright
            .as_deref()
            .filter(|notice| !notice.trim().is_empty())
            .unwrap_or("No declared copyright notice"),
    );
    if provenance.authors.is_empty() {
        out.say("No commit authors yet");
    } else {
        let authors: Vec<_> = provenance.authors.iter().take(3).cloned().collect();
        out.say(format!("Commit authors: {}", authors.join(", ")));
    }
    out.say(match (provenance.first_year, provenance.latest_year) {
        (Some(first), Some(latest)) if first == latest => format!("Archive year {first}"),
        (Some(first), Some(latest)) => format!("Archive {first} to {latest}"),
        _ => "History not yet written".into(),
    });
    skip_prompt(out, state);
}

/// One short line per opening beat. The Oracle plates carry no text, so each
/// beat is named from its authored scene; the legacy fanfare reads its own.
fn opening(out: &mut Transcript, state: &GameState) {
    if state.uses_visual_template(VisualTemplate::Awakening) {
        out.say(match state.opening_beat() {
            OpeningBeat::SourceEmber => {
                "A lone code-seer finds a cyan source ember in a dormant archive"
            }
            OpeningBeat::ArchiveAnswer => "The ember reaches the altar and wakes the archive",
            OpeningBeat::MemoryVault => "The archive opens into a canyon of commit constellations",
            OpeningBeat::Convergence => {
                "Cyan source light and gold knowledge meet around the Oracle's dark seed"
            }
            OpeningBeat::OracleAwakening | OpeningBeat::Legacy => "The Oracle awakens",
        });
    } else if state.screen_ticks < 120 {
        out.say("Two paths converge");
    } else {
        out.say("History becomes power. The Oracle opens");
    }
    skip_prompt(out, state);
}

fn title(out: &mut Transcript, state: &GameState) {
    let Some(cartridge) = state.cartridge.as_ref() else {
        out.say("No cartridge. Power off to load a game");
        return;
    };
    out.say(&cartridge.title);
    out.say(if state.uses_visual_template(VisualTemplate::Title) {
        "Repository Oracle"
    } else {
        match state.cartridge_mode() {
            Some(CartridgeMode::Custom) => "Every command is a boss",
            _ => "Endless repository quiz",
        }
    });
    out.say("Press Start");
}

fn quiz_menu(out: &mut Transcript, state: &GameState) {
    let oracle = state.uses_visual_template(VisualTemplate::Menu);
    out.say(if oracle {
        "Choose your path"
    } else {
        "Repo quiz"
    });
    // The journal line appears exactly when the menu shows its summary.
    if state.journal_summary().is_some() {
        let lessons = state.lessons();
        let pending = lessons.iter().filter(|lesson| lesson.outstanding).count();
        let noun = if lessons.len() == 1 {
            "lesson"
        } else {
            "lessons"
        };
        out.say(if pending == 0 {
            format!("Journal: {} {noun}, all clear", lessons.len())
        } else {
            format!(
                "Journal: {} {noun}, {pending} awaiting review",
                lessons.len()
            )
        });
    }
    let first = if oracle {
        "BEGIN THE TRIAL"
    } else {
        "BEGIN RUN"
    };
    for (index, label) in [first, quiz_menu_second_option(state)]
        .into_iter()
        .enumerate()
    {
        let selected = if state.menu_selected == index {
            ", selected"
        } else {
            ""
        };
        out.say(format!("Option {} of 2: {label}{selected}", index + 1));
    }
    out.say("Up and Down move, A chooses, B goes back");
}

fn hero_creation(out: &mut Transcript, state: &GameState) {
    let atelier = state.uses_visual_template(VisualTemplate::Atelier);
    let (heading, labels, bind) = if atelier {
        ("Bind your code-seer", ["Name", "Path", "Aura"], "Bind")
    } else {
        (
            "Create your hero",
            ["Name", "Class", "Style"],
            "Begin quest",
        )
    };
    out.say(heading);
    let values = [
        HERO_NAMES[state.hero_name],
        HERO_CLASSES[state.hero_class],
        HERO_STYLES[state.hero_style],
    ];
    let rows = labels
        .iter()
        .zip(values)
        .map(|(label, value)| format!("{label}: {value}"))
        .chain([bind.to_string()]);
    for (index, row) in rows.enumerate() {
        if state.hero_row == index {
            out.say(format!("{row}, selected"));
        } else {
            out.say(row);
        }
    }
    out.say(creation_oracle_status(state, atelier).trim_end_matches('.'));
    out.say("Up and Down choose a row, Left and Right change it, Start begins, B goes back");
}

fn datafall(out: &mut Transcript, state: &GameState) {
    out.say("Oracle Datafall");
    if state.uses_visual_template(VisualTemplate::Sanctum) {
        out.say(format!("{} bond", spoken(state.visual_tier().label())));
    }
    let provider = state.ai_provider_name();
    // The same rule the Datafall status lines and the audio snapshot read.
    out.say(match state.question_status() {
        QuestionStatus::Ready => "The next question is ready".to_string(),
        QuestionStatus::Writing => format!("{provider} is writing questions"),
        QuestionStatus::Retrying => {
            format!("{provider} could not write questions yet and will retry")
        }
        QuestionStatus::Contacting => format!("Contacting {provider}"),
    });
    let charge = threshold_stage(state.oracle_data, &DATA_CHARGE_THRESHOLDS);
    out.say(format!(
        "Data {}, charge runes {charge} of 3",
        state.oracle_data
    ));
    let breach = threshold_stage(state.oracle_bug_hits, &BUG_BREACH_THRESHOLDS);
    out.say(format!(
        "Bugs {}, containment seals {} of 3 intact",
        state.oracle_bug_hits,
        3usize.saturating_sub(breach)
    ));
    out.say("Left and Right move to catch data and dodge bugs, B leaves");
}

/// Wards, flow, score, and any awakened insight rune, as the trial HUD shows
/// them.
fn run_status(run: &QuizRun) -> String {
    let insight = InsightStage::from_score(run.score);
    let rune = if insight == InsightStage::Unlit {
        String::new()
    } else {
        format!(", insight rune {}", insight.label())
    };
    format!(
        "Wards {} of 3, flow x{}, score {}{rune}",
        run.hearts.min(3),
        streak_multiplier(run.streak),
        run.score
    )
}

fn trial(out: &mut Transcript, state: &GameState) {
    let Some(run) = state.quiz.as_ref() else {
        out.say("Trial. Waiting for a question");
        return;
    };
    let number = run.question + 1;
    let Some(question) = state
        .cartridge
        .as_ref()
        .and_then(|cartridge| cartridge.questions.get(run.question))
    else {
        out.say(format!("Trial {number}. Waiting for the next question"));
        return;
    };
    let Some((correct, _)) = run.feedback else {
        match question.concept {
            Some(concept) => out.say(format!("Trial {number}, {} lens", spoken(concept.label()))),
            None => out.say(format!("Trial {number}")),
        }
        out.say(shown_question(&question.question));
        let order = run.display_order(question.choices.len());
        for (slot, source) in order.into_iter().take(4).enumerate() {
            let choice = shown_choice(&question.choices[source]);
            let selected = if run.selected == slot {
                ", selected"
            } else {
                ""
            };
            out.say(format!("Choice {}: {choice}{selected}", slot + 1));
        }
        out.say(run_status(run));
        if run.leave_armed > 0 {
            out.say("Press B again to leave the run");
        } else {
            out.say("Up and Down choose, A answers, B leaves");
        }
        return;
    };

    out.say(format!(
        "Trial {number}. {}",
        if correct { "Correct" } else { "Missed" }
    ));
    out.say(quiz_feedback_banner(run));
    lesson(out, state);
    if let (true, Some(concept)) = (
        state.uses_visual_template(VisualTemplate::Trial),
        question.concept,
    ) {
        out.say(format!(
            "{} mastery: {} of 3 runes",
            spoken(concept.label()),
            state.mastery_stage(concept)
        ));
    }
    out.say(run_status(run));
    // Stable across the input hold, so the card is announced once.
    out.say("A or Start continues after a short hold");
}

/// The lesson card read from the same composition the renderers draw: the
/// player's pick and why it misleads, then the answer and why it holds.
fn lesson(out: &mut Transcript, state: &GameState) {
    let Some(lines) = current_lesson(state, quiz_lesson_copy_box()) else {
        return;
    };
    fn flush(out: &mut Transcript, label: &str, why: &mut Vec<&str>) {
        if !why.is_empty() {
            out.say(format!("{label}: {}", why.join(" ")));
            why.clear();
        }
    }
    let mut label = "";
    let mut why: Vec<&str> = Vec::new();
    for line in &lines {
        // Choice lines carry a two-character "- " or "+ " marker.
        let choice = line.text.get(2..).unwrap_or_default();
        match line.tone {
            LessonTone::Misconception => {
                flush(out, label, &mut why);
                out.say(format!("You chose: {choice}"));
                label = "Why not";
            }
            LessonTone::Answer => {
                flush(out, label, &mut why);
                out.say(format!("The answer: {choice}"));
                label = "Why it holds";
            }
            LessonTone::MisconceptionWhy | LessonTone::AnswerWhy => why.push(&line.text),
        }
    }
    flush(out, label, &mut why);
}

fn level_up(out: &mut Transcript, state: &GameState) {
    if state.uses_visual_template(VisualTemplate::Ascension) {
        out.say(format!(
            "The Oracle bond ascends: {}",
            spoken(state.visual_tier().label())
        ));
    } else {
        out.say("Level up!");
    }
    if let Some(run) = state.quiz.as_ref() {
        out.say(format!(
            "Level {}. Batch {} survived",
            run.level, run.completed_batches
        ));
    }
    out.say(if state.level_up_can_continue() {
        "A or Start continues"
    } else {
        "Please wait"
    });
}

fn game_over(out: &mut Transcript, state: &GameState) {
    let aftermath = state.uses_visual_template(VisualTemplate::Aftermath);
    out.say(if aftermath {
        "The vision closes"
    } else {
        "Game over"
    });
    match state.quiz.as_ref() {
        Some(run) => {
            out.say(format!("Final score {}", run.score));
            if aftermath {
                out.say(format!(
                    "Bond reached: {}",
                    spoken(state.visual_tier().label())
                ));
            }
            out.say(format!("Level {} reached", run.level));
            let insight = InsightStage::from_score(run.score);
            if insight == InsightStage::Unlit {
                out.say("No insight rune awakened");
            } else {
                out.say(format!("Insight rune {}", insight.label()));
            }
        }
        None => out.say("No questions found"),
    }
    out.say("A, B, or Start returns to the menu");
}

fn codex(out: &mut Transcript, state: &GameState) {
    let lessons = state.lessons();
    let Some((index, lesson)) = state.codex_lesson() else {
        out.say("Oracle Codex, mastery");
        for concept in Concept::ALL {
            let pending = pending_reviews(lessons, concept);
            let review = if pending > 0 {
                format!(", {pending} awaiting review")
            } else {
                String::new()
            };
            out.say(format!(
                "{}: {} of 3 runes{review}",
                spoken(concept.label()),
                state.mastery_stage(concept)
            ));
        }
        if lessons.is_empty() {
            out.say("No lessons yet. Answer trials to write lessons");
            out.say("B goes back");
        } else {
            let pending = lessons.iter().filter(|lesson| lesson.outstanding).count();
            let learned = lessons.len() - pending;
            out.say(if pending == 0 {
                format!("{learned} learned, all clear")
            } else {
                format!("{learned} learned, {pending} awaiting review")
            });
            out.say("Left and Right read lessons, B goes back");
        }
        return;
    };
    out.say(format!(
        "Oracle Codex, lesson {} of {}",
        index + 1,
        lessons.len()
    ));
    match lesson.concept {
        Some(concept) => out.say(format!(
            "{} lens, {} of 3 runes",
            spoken(concept.label()),
            state.mastery_stage(concept)
        )),
        None => out.say("General lesson"),
    }
    out.say(spoken(codex_lesson_status(lesson).0));
    out.say(shown_question(&lesson.question));
    let shown_rationale = |text: &str| {
        wrap_text(text, RATIONALE_COLUMNS)
            .into_iter()
            .take(RATIONALE_ROWS)
            .collect::<Vec<_>>()
            .join(" ")
    };
    if state.codex_answer_sealed(lesson) {
        // The self-test page: the pick and its misconception, never the answer.
        if let Some((pick, why)) = lesson.misconception.as_ref() {
            out.say(format!("You chose: {}", shown_choice(pick)));
            out.say(shown_rationale(why));
        }
        out.say("Answer sealed");
        out.say("A reveals the answer, Left and Right turn pages, B goes back");
        return;
    }
    out.say(format!("Answer: {}", shown_choice(&lesson.answer)));
    if lesson.rationale.trim().is_empty() {
        out.say("No rationale was recorded");
    } else {
        out.say(format!(
            "Why it holds: {}",
            shown_rationale(&lesson.rationale)
        ));
    }
    if let (false, Some((pick, _))) = (lesson.outstanding, lesson.misconception.as_ref()) {
        let line = codex_once_chose_line(pick);
        let shown = line.strip_prefix(CODEX_ONCE_CHOSE).unwrap_or(&line);
        out.say(format!("Once chose: {shown}"));
    }
    out.say("Left and Right turn pages, B goes back");
}

fn quest_select(out: &mut Transcript, state: &GameState) {
    out.say("Choose thy quest");
    let Some(cartridge) = state.cartridge.as_ref() else {
        return;
    };
    let Some(quest) = cartridge.quests.get(state.quest_selected) else {
        out.say("No quests on cartridge. B goes back");
        return;
    };
    out.say(format!(
        "Quest {} of {}: {}, against {}",
        state.quest_selected + 1,
        cartridge.quests.len(),
        quest.name,
        quest.boss
    ));
    out.say("Up and Down choose, L and R jump, A fights, B goes back");
}

/// The battle names its boss; the streamed command output stays on screen
/// only, so every line of it does not re-announce the whole battle.
fn battle(out: &mut Transcript, state: &GameState) {
    if state.active_boss.trim().is_empty() {
        out.say("Quest battle. The command is running");
    } else {
        out.say(format!(
            "Quest battle against {}. The command is running",
            state.active_boss
        ));
    }
    out.say("B aborts");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene_machine::SceneMachineTemplate;

    const ALL_SCREENS: [Screen; 16] = [
        Screen::Off,
        Screen::Boot,
        Screen::Copyright,
        Screen::OpeningFanfare,
        Screen::Title,
        Screen::QuizMenu,
        Screen::CharacterCreation,
        Screen::Oracle,
        Screen::Quiz,
        Screen::LevelUp,
        Screen::GameOver,
        Screen::Codex,
        Screen::QuestSelect,
        Screen::Battle,
        Screen::Victory,
        Screen::Defeat,
    ];

    /// Fails to compile when a screen is added without joining `ALL_SCREENS`.
    #[allow(dead_code)]
    fn every_screen_is_listed(screen: Screen) {
        match screen {
            Screen::Off
            | Screen::Boot
            | Screen::Copyright
            | Screen::OpeningFanfare
            | Screen::Title
            | Screen::QuizMenu
            | Screen::CharacterCreation
            | Screen::Oracle
            | Screen::Quiz
            | Screen::LevelUp
            | Screen::GameOver
            | Screen::Codex
            | Screen::QuestSelect
            | Screen::Battle
            | Screen::Victory
            | Screen::Defeat => {}
        }
    }

    fn question(index: usize) -> QuizQuestion {
        QuizQuestion {
            question: format!("WHICH LAYER DECIDES WHAT HAPPENS NEXT IN ROUND {index}?"),
            choices: vec![
                format!("THE BEVY ENGINE {index}"),
                format!("THE WEB SHELL {index}"),
                format!("THE STYLESHEET {index}"),
                format!("THE INSTALLER {index}"),
            ],
            answer: 0,
            concept: Some(Concept::Responsibility),
            rationales: vec![
                "THE ENGINE OWNS STATE, RULES, AND TIMING.".into(),
                "THE SHELL ONLY FORWARDS INPUT AND PAINTS FRAMES.".into(),
                "STYLES DRAW THE DEVICE CASE, NOT THE GAME.".into(),
                "THE INSTALLER ONLY PACKAGES THE APP.".into(),
            ],
            review: false,
        }
    }

    fn quiz_cartridge() -> CartridgeSpec {
        CartridgeSpec {
            id: "/tmp/transcript-test".into(),
            title: "TRANSCRIPT TEST".into(),
            mode: CartridgeMode::Quiz,
            provenance: RepositoryProvenance {
                authors: vec!["ADA LOVELACE".into(), "GRACE HOPPER".into()],
                first_year: Some(2020),
                latest_year: Some(2024),
                copyright: Some("Copyright (c) 2020-2024 Ada Lovelace".into()),
            },
            codequest: None,
            machine: Box::new(SceneMachineDefinition::template(SceneMachineTemplate::Quiz)),
            quests: vec![],
            questions: vec![question(0)],
            question_batch_ends: Vec::new(),
            question_batch_levels: Vec::new(),
            lessons: Vec::new(),
            mastery: Mastery::new(),
        }
    }

    /// The repository's own Oracle cartridge, with every visual template.
    fn oracle_cartridge() -> CartridgeSpec {
        let mut cartridge = quiz_cartridge();
        let config = CodeQuestConfig::parse(include_str!("../../../CODEQUEST.toml"))
            .expect("the repository Oracle cartridge should parse");
        cartridge.machine = Box::new(
            config
                .runtime_machine()
                .expect("the Oracle scene graph should compile")
                .expect("schema v2 should produce a runtime machine"),
        );
        cartridge.codequest = Some(Box::new(config));
        cartridge
    }

    fn journal() -> (Vec<Lesson>, Mastery) {
        let lessons = vec![
            Lesson {
                question: "WHO OWNS THE GAME LOOP?".into(),
                answer: "THE HEADLESS BEVY ENGINE".into(),
                rationale: "THE SHELL ONLY DRAWS FRAMES AND FORWARDS BUTTON EDGES.".into(),
                concept: Some(Concept::Responsibility),
                outstanding: false,
                misconception: None,
                peeked: false,
            },
            Lesson {
                question: "WHAT MUST STAY TRUE WHEN A SCENE CHANGES?".into(),
                answer: "ONLY THE ENGINE CHANGES SCENES".into(),
                rationale: String::new(),
                concept: Some(Concept::Invariant),
                outstanding: true,
                misconception: None,
                peeked: false,
            },
        ];
        let mastery = Mastery::from([
            (
                Concept::Purpose,
                LensRecord {
                    first_try: 1,
                    ..LensRecord::default()
                },
            ),
            (
                Concept::Responsibility,
                LensRecord {
                    first_try: 3,
                    ..LensRecord::default()
                },
            ),
            (
                Concept::Invariant,
                LensRecord {
                    first_try: 5,
                    missed: 1,
                    ..LensRecord::default()
                },
            ),
        ]);
        (lessons, mastery)
    }

    fn issue(engine: &mut GameEngine, command: EngineCommand) {
        engine.command(command);
        engine.update();
    }

    fn press(engine: &mut GameEngine, button: Button) {
        for pressed in [true, false] {
            issue(engine, EngineCommand::Input { button, pressed });
        }
    }

    fn state(engine: &GameEngine) -> &GameState {
        engine.app.world().resource::<GameState>()
    }

    fn powered(cartridge: CartridgeSpec) -> GameEngine {
        let mut engine = GameEngine::new();
        issue(
            &mut engine,
            EngineCommand::AiProvider(Some("claude".into())),
        );
        issue(&mut engine, EngineCommand::Cartridge(Some(cartridge)));
        issue(&mut engine, EngineCommand::Power(true));
        issue(&mut engine, EngineCommand::BootComplete);
        engine
    }

    /// Presses Start, letting each scene's skip hold elapse, until `screen`.
    fn advance_to(engine: &mut GameEngine, screen: Screen) {
        for _ in 0..40 {
            if engine.screen() == screen {
                return;
            }
            press(engine, Button::Start);
            for _ in 0..30 {
                if engine.screen() == screen {
                    return;
                }
                engine.update();
            }
        }
        panic!("never reached {screen:?}; stuck on {:?}", engine.screen());
    }

    fn deliver(engine: &mut GameEngine, questions: Vec<QuizQuestion>) {
        let seq = state(engine).question_request_seq;
        issue(
            engine,
            EngineCommand::Questions {
                cartridge_id: "/tmp/transcript-test".into(),
                result: Ok(questions),
                seq,
            },
        );
    }

    /// An Oracle trial showing `question`, reached through the real scene
    /// graph so the display order is the one the player sees.
    fn trial_engine(question: QuizQuestion) -> GameEngine {
        let mut cartridge = oracle_cartridge();
        cartridge.questions = vec![question];
        let mut engine = powered(cartridge);
        advance_to(&mut engine, Screen::Quiz);
        engine
    }

    /// Moves the trial's focus to display `slot`.
    fn focus(engine: &mut GameEngine, slot: usize) {
        while state(engine).quiz.as_ref().unwrap().selected != slot {
            press(engine, Button::Down);
        }
    }

    fn display_order(engine: &GameEngine) -> Vec<usize> {
        state(engine).quiz.as_ref().unwrap().display_order(4)
    }

    #[test]
    fn the_device_is_silent_while_off_or_booting() {
        let mut engine = GameEngine::new();
        assert_eq!(engine.transcript(), "");
        issue(
            &mut engine,
            EngineCommand::Cartridge(Some(oracle_cartridge())),
        );
        issue(&mut engine, EngineCommand::Power(true));
        assert_eq!(engine.screen(), Screen::Boot);
        assert_eq!(engine.transcript(), "");
    }

    #[test]
    fn the_chronicle_reads_its_credits_at_once() {
        let mut engine = powered(oracle_cartridge());
        assert_eq!(engine.screen(), Screen::Copyright);
        let credits = "Repository chronicle. TRANSCRIPT TEST. \
                       Copyright (c) 2020-2024 Ada Lovelace. \
                       Commit authors: ADA LOVELACE, GRACE HOPPER. Archive 2020 to 2024.";
        assert_eq!(engine.transcript(), credits);
        for _ in 0..60 {
            engine.update();
        }
        assert_eq!(
            engine.transcript(),
            format!("{credits} A or Start skips."),
            "only the skip prompt arrives later, when Start really skips"
        );
    }

    #[test]
    fn opening_beats_read_one_line_each_and_offer_the_skip() {
        let mut engine = powered(oracle_cartridge());
        advance_to(&mut engine, Screen::OpeningFanfare);
        assert_eq!(state(&engine).opening_beat(), OpeningBeat::SourceEmber);
        let mut beats: Vec<String> = Vec::new();
        let mut offered_skip = false;
        while engine.screen() == Screen::OpeningFanfare {
            let text = engine.transcript();
            offered_skip |= text.ends_with(" A or Start skips.");
            let beat = text.trim_end_matches(" A or Start skips.").to_string();
            if beats.last() != Some(&beat) {
                beats.push(beat);
            }
            engine.update();
        }
        assert_eq!(
            beats,
            [
                "A lone code-seer finds a cyan source ember in a dormant archive.",
                "The ember reaches the altar and wakes the archive.",
                "The archive opens into a canyon of commit constellations.",
                "Cyan source light and gold knowledge meet around the Oracle's dark seed.",
                "The Oracle awakens.",
            ]
        );
        assert!(offered_skip);
    }

    #[test]
    fn the_title_names_the_cartridge_and_its_prompt() {
        let mut engine = powered(oracle_cartridge());
        advance_to(&mut engine, Screen::Title);
        assert_eq!(
            engine.transcript(),
            "TRANSCRIPT TEST. Repository Oracle. Press Start."
        );

        let mut legacy = powered(quiz_cartridge());
        advance_to(&mut legacy, Screen::Title);
        assert_eq!(
            legacy.transcript(),
            "TRANSCRIPT TEST. Endless repository quiz. Press Start."
        );
    }

    #[test]
    fn the_menu_names_its_options_the_focus_and_the_journal() {
        let mut cartridge = oracle_cartridge();
        (cartridge.lessons, cartridge.mastery) = journal();
        let mut engine = powered(cartridge);
        advance_to(&mut engine, Screen::QuizMenu);
        assert_eq!(
            engine.transcript(),
            "Choose your path. Journal: 2 lessons, 1 awaiting review. \
             Option 1 of 2: BEGIN THE TRIAL, selected. Option 2 of 2: OPEN THE CODEX. \
             Up and Down move, A chooses, B goes back."
        );
        press(&mut engine, Button::Down);
        let text = engine.transcript();
        assert!(text.contains("Option 1 of 2: BEGIN THE TRIAL. "), "{text}");
        assert!(
            text.contains("Option 2 of 2: OPEN THE CODEX, selected."),
            "{text}"
        );

        let mut legacy = powered(quiz_cartridge());
        advance_to(&mut legacy, Screen::QuizMenu);
        assert_eq!(
            legacy.transcript(),
            "Repo quiz. Option 1 of 2: BEGIN RUN, selected. Option 2 of 2: OPEN THE CODEX. \
             Up and Down move, A chooses, B goes back.",
            "an empty journal has no summary line"
        );
    }

    #[test]
    fn hero_creation_reads_each_row_the_focus_and_the_oracle() {
        let mut engine = powered(oracle_cartridge());
        advance_to(&mut engine, Screen::QuizMenu);
        press(&mut engine, Button::A);
        assert_eq!(engine.screen(), Screen::CharacterCreation);
        press(&mut engine, Button::Right);
        press(&mut engine, Button::Down);
        assert_eq!(
            engine.transcript(),
            "Bind your code-seer. Name: GREP. Path: CODE KNIGHT, selected. Aura: EMBER. \
             Bind. ORACLE READY. Up and Down choose a row, Left and Right change it, \
             Start begins, B goes back."
        );
    }

    #[test]
    fn datafall_names_the_oracle_status_its_counts_and_its_controls() {
        let mut cartridge = oracle_cartridge();
        cartridge.questions.clear();
        let mut engine = powered(cartridge);
        advance_to(&mut engine, Screen::Oracle);
        let audio_status = |engine: &GameEngine| audio_snapshot(state(engine)).questions;
        assert_eq!(audio_status(&engine), QuestionStatus::Writing);
        assert_eq!(
            engine.transcript(),
            "Oracle Datafall. Initiate bond. CLAUDE is writing questions. \
             Data 0, charge runes 0 of 3. Bugs 0, containment seals 3 of 3 intact. \
             Left and Right move to catch data and dodge bugs, B leaves."
        );

        deliver(&mut engine, Vec::new());
        assert_eq!(audio_status(&engine), QuestionStatus::Retrying);
        assert!(engine
            .transcript()
            .contains(" CLAUDE could not write questions yet and will retry. "));

        deliver(&mut engine, vec![question(1)]);
        assert_eq!(audio_status(&engine), QuestionStatus::Ready);
        assert!(engine
            .transcript()
            .contains(" The next question is ready. "));

        let counted = GameState {
            screen: Screen::Oracle,
            oracle_data: 4,
            oracle_bug_hits: 3,
            ..GameState::default()
        };
        assert_eq!(
            counted.question_status(),
            audio_snapshot(&counted).questions
        );
        assert_eq!(
            screen_transcript(&counted),
            "Oracle Datafall. Contacting AI. Data 4, charge runes 1 of 3. \
             Bugs 3, containment seals 1 of 3 intact. \
             Left and Right move to catch data and dodge bugs, B leaves."
        );
    }

    #[test]
    fn a_trial_reads_the_shuffled_display_order_and_the_focused_choice() {
        let question = question(7);
        let mut engine = trial_engine(question.clone());
        let order = display_order(&engine);
        assert_eq!(
            order,
            learning::presentation_order(&question_identity(&question.question), 0)
        );
        assert_ne!(order, [0, 1, 2, 3], "the fixture must actually be shuffled");

        let choices: Vec<String> = order
            .iter()
            .enumerate()
            .map(|(slot, source)| {
                let selected = if slot == 0 { ", selected" } else { "" };
                format!(
                    "Choice {}: {}{selected}.",
                    slot + 1,
                    question.choices[*source]
                )
            })
            .collect();
        assert_eq!(
            engine.transcript(),
            format!(
                "Trial 1, Roles lens. {} {} Wards 3 of 3, flow x1, score 0. \
                 Up and Down choose, A answers, B leaves.",
                question.question,
                choices.join(" ")
            )
        );

        press(&mut engine, Button::Down);
        let text = engine.transcript();
        assert!(
            text.contains(&format!(
                "Choice 1: {}. Choice 2: {}, selected.",
                question.choices[order[0]], question.choices[order[1]]
            )),
            "{text}"
        );

        press(&mut engine, Button::B);
        assert!(
            engine
                .transcript()
                .ends_with(" Press B again to leave the run."),
            "the first B arms the leave confirmation"
        );
    }

    #[test]
    fn a_missed_lesson_card_reads_the_pick_the_answer_and_both_rationales() {
        let question = question(3);
        let mut engine = trial_engine(question.clone());
        let order = display_order(&engine);
        let answer_slot = order.iter().position(|source| *source == 0).unwrap();
        let wrong_slot = (answer_slot + 1) % 4;
        let picked = order[wrong_slot];
        focus(&mut engine, wrong_slot);
        press(&mut engine, Button::A);

        let card = engine.transcript();
        assert_eq!(
            card,
            format!(
                "Trial 1. Missed. WARD STRAINED. You chose: {}. Why not: {} \
                 The answer: {}. Why it holds: {} Roles mastery: 0 of 3 runes. \
                 Wards 2 of 3, flow x1, score 0. A or Start continues after a short hold.",
                question.choices[picked],
                question.rationales[picked],
                question.choices[0],
                question.rationales[0],
            )
        );
        for _ in 0..QUIZ_FEEDBACK_TICKS {
            engine.update();
        }
        assert!(lesson_is_live(state(&engine).quiz.as_ref().unwrap()));
        assert_eq!(
            engine.transcript(),
            card,
            "the end of the input hold must not re-announce the card"
        );
    }

    #[test]
    fn a_correct_lesson_card_reinforces_the_answer_alone() {
        let question = question(4);
        let mut engine = trial_engine(question.clone());
        let answer_slot = display_order(&engine)
            .iter()
            .position(|source| *source == 0)
            .unwrap();
        focus(&mut engine, answer_slot);
        press(&mut engine, Button::A);
        assert_eq!(
            engine.transcript(),
            format!(
                "Trial 1. Correct. REVIEW ANSWER. The answer: {}. Why it holds: {} \
                 Roles mastery: 1 of 3 runes. Wards 3 of 3, flow x1, score 100. \
                 A or Start continues after a short hold.",
                question.choices[0], question.rationales[0]
            )
        );
    }

    #[test]
    fn the_codex_reads_mastery_rows_and_lesson_pages() {
        let mut cartridge = oracle_cartridge();
        (cartridge.lessons, cartridge.mastery) = journal();
        let mut engine = powered(cartridge);
        advance_to(&mut engine, Screen::QuizMenu);
        press(&mut engine, Button::Down);
        press(&mut engine, Button::A);
        assert_eq!(engine.screen(), Screen::Codex);
        assert_eq!(
            engine.transcript(),
            "Oracle Codex, mastery. Purpose: 1 of 3 runes. Roles: 2 of 3 runes. \
             Flows: 0 of 3 runes. Invariants: 3 of 3 runes, 1 awaiting review. \
             Tradeoffs: 0 of 3 runes. 1 learned, 1 awaiting review. \
             Left and Right read lessons, B goes back."
        );

        press(&mut engine, Button::Right);
        assert_eq!(
            engine.transcript(),
            "Oracle Codex, lesson 1 of 2. Roles lens, 2 of 3 runes. Learned. \
             WHO OWNS THE GAME LOOP? Answer: THE HEADLESS BEVY ENGINE. \
             Why it holds: THE SHELL ONLY DRAWS FRAMES AND FORWARDS BUTTON EDGES. \
             Left and Right turn pages, B goes back."
        );
        press(&mut engine, Button::Right);
        assert_eq!(
            engine.transcript(),
            "Oracle Codex, lesson 2 of 2. Invariants lens, 3 of 3 runes. Review pending. \
             WHAT MUST STAY TRUE WHEN A SCENE CHANGES? Answer sealed. \
             A reveals the answer, Left and Right turn pages, B goes back.",
            "a pending lesson without a recorded pick is sealed, never read out"
        );
        press(&mut engine, Button::A);
        let text = engine.transcript();
        assert!(
            text.starts_with(
                "Oracle Codex, lesson 2 of 2. Invariants lens, 3 of 3 runes. Pending peeked."
            ),
            "{text}"
        );
        assert!(
            text.contains(" Answer: ONLY THE ENGINE CHANGES SCENES. No rationale was recorded. "),
            "{text}"
        );

        // A pick recorded with the miss is read on the sealed page, and a
        // learned lesson recalls the misconception it replaced.
        let mut state = engine.app.world_mut().resource_mut::<GameState>();
        let lessons = &mut state.cartridge.as_mut().unwrap().lessons;
        let misconception = Some((
            "THE WEB SHELL".to_string(),
            "THE SHELL ONLY FORWARDS INPUT AND PAINTS FRAMES.".to_string(),
        ));
        lessons[1].peeked = false;
        lessons[1].misconception = misconception.clone();
        lessons[0].misconception = misconception;
        press(&mut engine, Button::Right);
        press(&mut engine, Button::Left);
        let text = engine.transcript();
        assert!(
            text.contains(
                " Review pending. WHAT MUST STAY TRUE WHEN A SCENE CHANGES? \
                 You chose: THE WEB SHELL. THE SHELL ONLY FORWARDS INPUT AND PAINTS FRAMES. \
                 Answer sealed. "
            ),
            "{text}"
        );
        press(&mut engine, Button::Left);
        let text = engine.transcript();
        assert!(
            text.ends_with(
                " FORWARDS BUTTON EDGES. Once chose: THE WEB SHELL. \
                 Left and Right turn pages, B goes back."
            ),
            "{text}"
        );

        let empty = GameState {
            screen: Screen::Codex,
            ..GameState::default()
        };
        assert!(screen_transcript(&empty)
            .ends_with(" No lessons yet. Answer trials to write lessons. B goes back."));
    }

    #[test]
    fn level_up_and_game_over_read_the_run_totals() {
        let mut state = GameState {
            cartridge: Some(quiz_cartridge()),
            screen: Screen::GameOver,
            quiz: Some(QuizRun {
                score: 900,
                level: 3,
                hearts: 0,
                completed_batches: 2,
                ..QuizRun::new()
            }),
            ..GameState::default()
        };
        assert_eq!(
            screen_transcript(&state),
            "Game over. Final score 900. Level 3 reached. Insight rune II. \
             A, B, or Start returns to the menu."
        );
        state.screen = Screen::LevelUp;
        assert_eq!(
            screen_transcript(&state),
            "Level up! Level 3. Batch 2 survived. Please wait."
        );
    }

    fn sentences(text: &str) -> Vec<String> {
        text.split_inclusive(". ")
            .map(|sentence| sentence.trim().to_string())
            .collect()
    }

    fn publish(channel: &mut TranscriptChannel, engine: &GameEngine) -> bool {
        let (screen, sentences) = engine.transcript_sentences();
        channel.publish(screen, sentences)
    }

    #[test]
    fn publication_advances_the_sequence_only_when_the_text_changes() {
        let mut channel = TranscriptChannel::default();
        assert!(
            !channel.publish(Screen::Off, Vec::new()),
            "the device starts silent"
        );
        assert_eq!(channel.since(0), None);
        assert!(channel.publish(Screen::Title, sentences("TITLE.")));
        assert!(!channel.publish(Screen::Title, sentences("TITLE.")));
        assert_eq!(
            channel.since(0),
            Some(TranscriptUpdate {
                seq: 1,
                text: "TITLE.".into()
            })
        );
        assert_eq!(channel.since(1), None, "a current reader gets nothing");
        assert!(
            channel.publish(Screen::Off, Vec::new()),
            "powering off clears the region"
        );
        assert_eq!(
            channel.since(1),
            Some(TranscriptUpdate {
                seq: 2,
                text: String::new()
            })
        );
        assert_eq!(
            channel.since(99).map(|update| update.seq),
            Some(2),
            "a reader holding a sequence number this channel never issued resynchronizes"
        );

        // A live engine republishes only when its screen text changes, even
        // though the title prompt blinks and Datafall drops move every tick.
        let mut cartridge = oracle_cartridge();
        cartridge.questions.clear();
        let mut engine = powered(cartridge);
        advance_to(&mut engine, Screen::Title);
        let mut channel = TranscriptChannel::default();
        for _ in 0..240 {
            engine.update();
            publish(&mut channel, &engine);
        }
        assert_eq!(channel.since(0).map(|update| update.seq), Some(1));

        advance_to(&mut engine, Screen::Oracle);
        let mut published = 0;
        let mut ticks = 0;
        while state(&engine).oracle_data + state(&engine).oracle_bug_hits == 0 {
            if publish(&mut channel, &engine) {
                published += 1;
            }
            engine.update();
            ticks += 1;
            assert!(ticks < 2_000, "the Datafall should score a drop");
        }
        let entered = channel.since(0).unwrap().seq;
        assert!(publish(&mut channel, &engine), "the first catch is news");
        let catch = channel.since(entered).unwrap().text;
        assert!(
            catch.starts_with("Data ") || catch.starts_with("Bugs "),
            "a catch announces its counter alone, not the whole Datafall: {catch}"
        );
        assert!(ticks > 60, "many ticks passed before the first catch");
        assert_eq!(
            published, 1,
            "only entering the Datafall published before it"
        );
    }

    #[test]
    fn a_new_screen_is_read_whole_and_a_focus_move_reads_only_the_new_focus() {
        let question = question(7);
        let mut engine = trial_engine(question.clone());
        let order = display_order(&engine);
        let mut channel = TranscriptChannel::default();
        assert!(publish(&mut channel, &engine));
        let trial = channel.since(0).unwrap();
        assert_eq!(
            trial.text,
            engine.transcript(),
            "a new screen is read whole"
        );

        press(&mut engine, Button::Down);
        assert!(publish(&mut channel, &engine));
        let moved = channel.since(trial.seq).unwrap();
        assert_eq!(
            moved.text,
            format!("Choice 2: {}, selected.", question.choices[order[1]]),
            "Down reads the newly focused choice, not the question again"
        );

        // A reader that polls late hears the change against what it last
        // presented, not against a publication it never saw.
        press(&mut engine, Button::Down);
        publish(&mut channel, &engine);
        press(&mut engine, Button::Down);
        publish(&mut channel, &engine);
        assert_eq!(
            channel.since(moved.seq).unwrap().text,
            format!("Choice 4: {}, selected.", question.choices[order[3]])
        );
        let armed = channel.since(0).unwrap().seq;
        press(&mut engine, Button::B);
        publish(&mut channel, &engine);
        assert_eq!(
            channel.since(armed).unwrap().text,
            "Press B again to leave the run."
        );

        // Answering changes the heading, so the lesson card is read whole,
        // even by a reader that missed the focus moves before it.
        press(&mut engine, Button::A);
        publish(&mut channel, &engine);
        assert_eq!(channel.since(trial.seq).unwrap().text, engine.transcript());
        assert!(engine.transcript().starts_with("Trial 1. "));
    }

    #[test]
    fn announcements_read_appended_prompts_and_fall_back_to_the_whole_screen() {
        let old = sentences("Credits. Archive 2020 to 2024.");
        let new = sentences("Credits. Archive 2020 to 2024. A or Start skips.");
        assert_eq!(announcement(&old, &new), "A or Start skips.");

        let menu = sentences("Menu. Option 1: A, selected. Option 2: B. Up and Down move.");
        let moved = sentences("Menu. Option 1: A. Option 2: B, selected. Up and Down move.");
        assert_eq!(announcement(&menu, &moved), "Option 2: B, selected.");

        let page = sentences("Codex, lesson 1 of 2. Lens. Question. Answer.");
        let turned = sentences("Codex, lesson 2 of 2. Lens. Question two. Answer two.");
        assert_eq!(
            announcement(&page, &turned),
            turned.join(" "),
            "a new heading reads the whole page"
        );
        let most = sentences("Heading. A. B. C.");
        let changed = sentences("Heading. X. Y. C.");
        assert_eq!(
            announcement(&most, &changed),
            "X. Y.",
            "half the screen changing still reads only the change"
        );
        let nearly = sentences("Heading. A. B. C.");
        let rewritten = sentences("Heading. X. Y. Z.");
        assert_eq!(announcement(&nearly, &rewritten), rewritten.join(" "));
        let shorter = sentences("Heading. A.");
        assert_eq!(announcement(&nearly, &shorter), "Heading. A.");
    }

    #[test]
    fn every_screen_has_a_transcript_that_does_not_panic() {
        let mut quest_cartridge = quiz_cartridge();
        quest_cartridge.mode = CartridgeMode::Custom;
        quest_cartridge.quests = vec![QuestSpec {
            name: "SCRYING POOL".into(),
            boss: "THE STATUS WYRM".into(),
            command: "git status".into(),
        }];
        let lesson_run = |correct| QuizRun {
            feedback: Some((correct, 0)),
            selected: 1,
            leave_armed: 5,
            ..QuizRun::new()
        };
        let states = [
            GameState::default(),
            GameState {
                cartridge: Some(quiz_cartridge()),
                ..GameState::default()
            },
            GameState {
                cartridge: Some(oracle_cartridge()),
                quiz: Some(QuizRun::new()),
                ..GameState::default()
            },
            GameState {
                cartridge: Some(oracle_cartridge()),
                quiz: Some(lesson_run(false)),
                ..GameState::default()
            },
            GameState {
                cartridge: Some(quiz_cartridge()),
                quiz: Some(lesson_run(true)),
                ..GameState::default()
            },
            GameState {
                cartridge: Some(quiz_cartridge()),
                quiz: Some(QuizRun {
                    question: 9,
                    ..QuizRun::new()
                }),
                codex_page: 4,
                ..GameState::default()
            },
            GameState {
                cartridge: Some(quest_cartridge),
                active_boss: "THE STATUS WYRM".into(),
                quest_selected: 3,
                ..GameState::default()
            },
        ];
        for mut state in states {
            for screen in ALL_SCREENS {
                state.screen = screen;
                let text = screen_transcript(&state);
                if matches!(screen, Screen::Off | Screen::Boot) {
                    assert_eq!(text, "", "{screen:?} is silent");
                    continue;
                }
                assert!(!text.is_empty(), "{screen:?} must describe itself");
                assert!(
                    text.ends_with(['.', '!', '?']),
                    "{screen:?} ends mid-sentence: {text}"
                );
                assert!(
                    !text.contains("  ") && !text.contains('\n'),
                    "{screen:?} is not plain prose: {text:?}"
                );
            }
        }
    }
}
