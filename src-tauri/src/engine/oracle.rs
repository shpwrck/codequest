use super::*;

pub(super) const ORACLE_HERO_MIN_X: i32 = 8;
pub(super) const ORACLE_HERO_MAX_X: i32 = 210;
pub(super) const ORACLE_HERO_SPEED: i32 = 2;
pub(super) const ORACLE_DROP_INTERVAL: u64 = 30;
pub(super) const ORACLE_COLLISION_Y: i32 = 100;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum OracleDropKind {
    Data,
    Bug,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct OracleDrop {
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) kind: OracleDropKind,
}

/// The second line of a waiting Oracle's header.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum OracleLine {
    /// Why the last question request failed, and the whole seconds until the
    /// retry, or `None` while the retry is in flight.
    Failure {
        reason: String,
        retry_in: Option<u64>,
    },
    /// One journal lesson offered as retrieval practice.
    Recall {
        outstanding: bool,
        concept: Option<Concept>,
        answer: String,
    },
}

/// Ticks each recalled lesson stays on the Oracle line: four seconds.
pub(super) const ORACLE_RECALL_TICKS: u64 = 240;
/// Characters a newly recalled lesson reveals per tick.
pub(super) const ORACLE_RECALL_REVEAL_PER_TICK: usize = 2;
/// Ticks a retry after a failure keeps the failure on the Oracle line before
/// a recall may take it over: one second.
pub(super) const ORACLE_RETRY_HOLD_TICKS: u16 = 60;
/// Longest failure reason the Oracle line carries.
pub(super) const ORACLE_FAILURE_CHARS: usize = 24;

/// A provider failure reason as the Oracle line shows it: one upper-case line
/// of letters, digits, and simple punctuation, cut at a word boundary.
pub(super) fn oracle_failure_reason(reason: &str) -> String {
    let cleaned: String = reason
        .chars()
        .map(|ch| {
            let ch = ch.to_ascii_uppercase();
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | ':' | '.' | '/') {
                ch
            } else {
                ' '
            }
        })
        .collect();
    let mut short = String::new();
    for word in cleaned.split_whitespace() {
        let separator = usize::from(!short.is_empty());
        if short.chars().count() + separator + word.chars().count() > ORACLE_FAILURE_CHARS {
            if short.is_empty() {
                short = truncate(word, ORACLE_FAILURE_CHARS);
            }
            break;
        }
        if separator == 1 {
            short.push(' ');
        }
        short.push_str(word);
    }
    if short.is_empty() {
        "GENERATION FAILED".into()
    } else {
        short
    }
}

impl GameState {
    /// Journal lessons in the order the waiting Oracle recalls them:
    /// outstanding misses first, then cleared lessons, each newest first. An
    /// outstanding miss whose question is still queued (a retry copy ahead
    /// in this run, or a review the next run opens with) is left out: its
    /// answer on the Oracle line would turn that review into a reading check.
    pub(super) fn recall_order(&self) -> Vec<&Lesson> {
        let lessons = self.lessons();
        let ahead = self
            .quiz
            .as_ref()
            .filter(|_| self.run_is_live())
            .map_or(0, |run| run.question);
        let queued: HashSet<String> = self
            .cartridge
            .as_ref()
            .map_or(&[][..], |cartridge| cartridge.questions.as_slice())
            .iter()
            .skip(ahead)
            .map(|question| question_identity(&question.question))
            .collect();
        let outstanding = lessons.iter().rev().filter(|lesson| {
            lesson.outstanding && !queued.contains(&question_identity(&lesson.question))
        });
        let cleared = lessons.iter().rev().filter(|lesson| !lesson.outstanding);
        outstanding.chain(cleared).collect()
    }

    /// The lesson the Oracle recalls now. Each recall starts from the top of
    /// the recall order and moves on every `ORACLE_RECALL_TICKS`.
    pub(super) fn recalled_lesson(&self) -> Option<&Lesson> {
        let order = self.recall_order();
        let slot = (self.oracle_recall_ticks() / ORACLE_RECALL_TICKS) as usize;
        order.get(slot.checked_rem(order.len())?).copied()
    }

    /// The waiting Oracle's second status line. While a failed request waits
    /// out its retry delay the line says why, and it keeps saying so for the
    /// first `ORACLE_RETRY_HOLD_TICKS` of the retry, so a retry that fails
    /// again at once never flashes a lesson between two failure lines.
    /// Otherwise it recalls a journal lesson, and with no journal it keeps
    /// naming the last failure while the retry is in flight. A ready question
    /// has no failure to explain.
    pub(super) fn oracle_line(&self) -> Option<OracleLine> {
        let failure = self
            .question_failure
            .as_ref()
            .filter(|_| !self.has_unanswered_question());
        if let Some(reason) = failure {
            if self.question_retry_ticks > 0 {
                return Some(OracleLine::Failure {
                    reason: reason.clone(),
                    retry_in: Some(u64::from(self.question_retry_ticks).div_ceil(60)),
                });
            }
            // The retry is about to be sent, or has only just been sent.
            if !self.questions_loading || self.question_request_ticks < ORACLE_RETRY_HOLD_TICKS {
                return Some(OracleLine::Failure {
                    reason: reason.clone(),
                    retry_in: None,
                });
            }
        }
        if let Some(lesson) = self.recalled_lesson() {
            return Some(OracleLine::Recall {
                outstanding: lesson.outstanding,
                concept: lesson.concept,
                answer: lesson.answer.clone(),
            });
        }
        failure.map(|reason| OracleLine::Failure {
            reason: reason.clone(),
            retry_in: None,
        })
    }

    /// Characters of the Oracle line drawn this tick. A newly recalled
    /// lesson is written in over a few ticks; reduced motion shows it whole,
    /// and a failure line is always whole.
    pub(super) fn oracle_line_reveal(&self, line: &OracleLine) -> usize {
        match line {
            OracleLine::Recall { .. } if !self.reduced_motion => {
                ((self.oracle_recall_ticks() % ORACLE_RECALL_TICKS) as usize + 1)
                    * ORACLE_RECALL_REVEAL_PER_TICK
            }
            _ => usize::MAX,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failure_reasons_are_sanitized_to_one_short_device_line() {
        assert_eq!(oracle_failure_reason("timed out"), "TIMED OUT");
        assert_eq!(
            oracle_failure_reason("CLI\nUNAVAILABLE \u{2014} \u{2713} not found"),
            "CLI UNAVAILABLE NOT"
        );
        assert_eq!(
            oracle_failure_reason("ONE TWO THREE FOUR FIVE SIX SEVEN"),
            "ONE TWO THREE FOUR FIVE"
        );
        assert_eq!(
            oracle_failure_reason("SUPERCALIFRAGILISTICEXPIALIDOCIOUS"),
            "SUPERCALIFRAGILISTICEXPI"
        );
        for empty in ["", "   ", "\u{2603}\u{2603}"] {
            assert_eq!(oracle_failure_reason(empty), "GENERATION FAILED");
        }
        for reason in [
            "RATE LIMITED",
            "a much longer reason than the oracle line has room to show",
        ] {
            let short = oracle_failure_reason(reason);
            assert!(short.chars().count() <= ORACLE_FAILURE_CHARS, "{short}");
            assert!(short
                .chars()
                .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == ' '));
        }
    }

    #[test]
    fn the_oracle_never_shows_the_answer_of_a_review_still_queued() {
        let missed = concept_question(0);
        let cleared = concept_question(1);
        let missed_answer = missed.choices[missed.answer].clone();
        let cleared_answer = cleared.choices[cleared.answer].clone();
        let mut state = waiting_state_with(Vec::new());
        {
            let cartridge = state.cartridge.as_mut().unwrap();
            record_lesson(&mut cartridge.lessons, &cleared, true, cleared.answer);
            record_lesson(
                &mut cartridge.lessons,
                &missed,
                false,
                (missed.answer + 1) % 4,
            );
            cartridge.questions = vec![missed.clone(), cleared.clone(), concept_question(2)];
        }
        state.batch_ends = vec![3];
        state.batch_levels = vec![1];
        state.consumed_questions = 2;
        state.questions_loading = false;

        // Between runs the whole deck is still ahead of the player.
        assert_eq!(recalled_answers(&state), [cleared_answer.as_str()]);

        // The next run opens with the miss as a review, after the Oracle's
        // ready dwell: that dwell must not print the review's answer.
        state.transition(Screen::Oracle);
        let deck = &state.cartridge.as_ref().unwrap().questions;
        assert!(deck[0].review.is_review() && deck[0].question == missed.question);
        for ticks in 0..75 {
            state.screen_ticks = ticks;
            let texts = oracle_line_texts(&state).join(" ");
            assert!(
                !texts.contains(&missed_answer),
                "tick {ticks}: the queued review's answer is shown: {texts}"
            );
        }
        state.screen_ticks = 0;
        assert_eq!(
            oracle_line_texts(&state),
            ["RECALL", "ROLES:", cleared_answer.as_str()],
            "a cleared lesson is still recalled with its answer"
        );

        // Once the review is behind the player, its lesson may be recalled.
        state.quiz.as_mut().unwrap().question = 1;
        assert_eq!(recalled_answers(&state), [missed_answer, cleared_answer]);
    }

    #[test]
    fn a_fast_failing_retry_never_flashes_a_lesson_and_recall_starts_fresh() {
        let mut engine = waiting_oracle_engine();
        state_mut(&mut engine).cartridge.as_mut().unwrap().lessons = journal_lessons();
        fail(&mut engine, "/tmp/engine-test", "AI DISABLED");
        for cycle in 0..3 {
            // The countdown runs out, the retry fires, and it fails again
            // within a few frames, as CQA_NO_AI or a missing CLI does.
            while engine_state(&engine).question_retry_ticks > 0 || cycle_start(&engine) {
                engine.update();
                assert!(
                    matches!(
                        engine_state(&engine).oracle_line(),
                        Some(OracleLine::Failure { .. })
                    ),
                    "cycle {cycle}: the countdown keeps the failure"
                );
            }
            for _ in 0..4 {
                engine.update();
                assert_eq!(
                    oracle_line_texts(engine_state(&engine)),
                    ["AI DISABLED", "- RETRYING"],
                    "cycle {cycle}: a retry in flight keeps the failure"
                );
            }
            fail(&mut engine, "/tmp/engine-test", "AI DISABLED");
        }

        // A slow retry hands the line to the journal after the hold, from the
        // top of the recall order and written in from its first character.
        while engine_state(&engine).question_retry_ticks > 0 || cycle_start(&engine) {
            engine.update();
        }
        let mut held = 0;
        while matches!(
            engine_state(&engine).oracle_line(),
            Some(OracleLine::Failure { .. })
        ) {
            engine.update();
            held += 1;
            assert!(held <= ORACLE_RETRY_HOLD_TICKS + 1, "the hold ends");
        }
        let state = engine_state(&engine);
        assert!(state.questions_loading);
        let line = state.oracle_line().unwrap();
        let top = state.recall_order()[0].answer.clone();
        assert!(
            matches!(&line, OracleLine::Recall { answer, .. } if *answer == top),
            "{line:?} does not start at {top}"
        );
        assert_eq!(
            state.oracle_line_reveal(&line),
            ORACLE_RECALL_REVEAL_PER_TICK
        );
    }

    #[test]
    fn recall_never_changes_datafall_play() {
        let mut plain = waiting_oracle_engine();
        let mut recalling = waiting_oracle_engine();
        recalling
            .app
            .world_mut()
            .resource_mut::<GameState>()
            .cartridge
            .as_mut()
            .unwrap()
            .lessons = journal_lessons();
        for engine in [&mut plain, &mut recalling] {
            issue(
                engine,
                EngineCommand::Input {
                    button: Button::Right,
                    pressed: true,
                },
            );
            for _ in 0..240 {
                engine.update();
            }
        }
        let play = |engine: &GameEngine| {
            let state = engine_state(engine);
            format!(
                "{:?} {} {} {}",
                state.oracle_drops, state.oracle_hero_x, state.oracle_data, state.oracle_bug_hits
            )
        };
        assert!(engine_state(&recalling).oracle_line().is_some());
        assert_eq!(play(&plain), play(&recalling));
        assert_eq!(recalling.screen(), Screen::Oracle);
    }
}
