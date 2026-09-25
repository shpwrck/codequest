use super::*;

pub(super) const QUIZ_FEEDBACK_TICKS: u16 = 45;

/// Other questions answered between a miss and its review copy. The copy may
/// carry into the next batch to keep this gap.
pub(super) const RETRY_GAP: usize = 3;

#[derive(Clone, Debug)]
pub(super) struct QuizRun {
    pub(super) question: usize,
    pub(super) completed_batches: usize,
    /// The focused display slot; `order` maps it to a source choice index.
    pub(super) selected: usize,
    pub(super) hearts: u8,
    pub(super) score: u32,
    pub(super) level: u32,
    pub(super) streak: u32,
    pub(super) leveled_up: bool,
    /// The committed result and its remaining input hold. The lesson card
    /// stays up after the hold reaches zero until A or Start continues.
    pub(super) feedback: Option<(bool, u16)>,
    /// Display order of the current question: `order[slot]` is the source
    /// choice index drawn in that slot.
    pub(super) order: Vec<usize>,
    /// The question index `order` and `attempt` were derived for.
    pub(super) presented: Option<usize>,
    /// Earlier committed attempts at the current question's identity.
    pub(super) attempt: u32,
    /// True when the committed answer redeemed a previously missed question.
    pub(super) redeemed: bool,
    /// When the committed miss returns, told on its lesson card.
    pub(super) retry_note: Option<RetryNote>,
    /// The lens and mastery stage the committed answer woke, if it crossed
    /// a lens threshold.
    pub(super) lens_woke: Option<(Concept, usize)>,
    /// Ticks left in which a second B leaves the run.
    pub(super) leave_armed: u16,
    /// What this run taught, reported by the Ascension and Aftermath debriefs.
    pub(super) ledger: RunLedger,
}

/// A per-run learning tally for the end-of-batch and end-of-run debriefs. It
/// only reports; mastery evidence lives in the cartridge's `Mastery`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct RunLedger {
    /// First attempts (non-review questions) committed this run.
    pub(super) first_try: u32,
    /// First attempts answered correctly this run.
    pub(super) first_try_right: u32,
    /// Review questions answered correctly this run.
    pub(super) redeemed: u32,
    /// First attempts committed in the batch still in progress.
    pub(super) batch_first_try: u32,
    /// First attempts answered correctly in the batch still in progress.
    pub(super) batch_first_try_right: u32,
    /// `(right, attempted)` first tries of the batch that completed last.
    pub(super) last_batch: (u32, u32),
    /// Lit mastery runes per lens (`Concept::ALL` order) when the run began.
    pub(super) stages_at_start: [usize; 5],
}

impl RunLedger {
    /// Records one committed answer. Only a review can redeem; only a
    /// question's first appearance counts as a first try.
    pub(super) fn record(&mut self, review: bool, correct: bool) {
        if review {
            self.redeemed += u32::from(correct);
        } else {
            self.first_try += 1;
            self.batch_first_try += 1;
            self.first_try_right += u32::from(correct);
            self.batch_first_try_right += u32::from(correct);
        }
    }

    /// Snapshots the batch that just completed for the level-up screen and
    /// starts counting the next one.
    pub(super) fn close_batch(&mut self) {
        self.last_batch = (self.batch_first_try_right, self.batch_first_try);
        self.batch_first_try = 0;
        self.batch_first_try_right = 0;
    }
}

/// When a missed question comes back, as its lesson card tells it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RetryNote {
    /// After this many intervening questions; the copy may carry into the
    /// next batch to keep its gap.
    In(usize),
    /// The deck does not reach the retry gap yet: the copy waits for the
    /// Oracle's next questions (see `RetrySlot::Deferred`).
    Later,
    /// The ward broke: the miss waits in the deck for the next run.
    NextRun,
}

impl RetryNote {
    pub(super) fn label(self) -> String {
        match self {
            Self::In(0) => "UP NEXT".into(),
            Self::In(gap @ 1..=9) => format!("BACK IN {gap}"),
            Self::In(_) | Self::Later => "LATER".into(),
            Self::NextRun => "NEXT RUN".into(),
        }
    }
}

impl QuizRun {
    pub(super) fn new() -> Self {
        Self {
            question: 0,
            completed_batches: 0,
            selected: 0,
            hearts: 3,
            score: 0,
            level: 1,
            streak: 0,
            leveled_up: false,
            feedback: None,
            order: Vec::new(),
            presented: None,
            attempt: 0,
            redeemed: false,
            retry_note: None,
            lens_woke: None,
            leave_armed: 0,
            ledger: RunLedger::default(),
        }
    }

    /// The source choice indices in display order for a question with
    /// `choice_count` choices. Until the current question has been presented,
    /// choices keep their source order.
    pub(super) fn display_order(&self, choice_count: usize) -> Vec<usize> {
        if self.presented == Some(self.question) && self.order.len() == choice_count {
            self.order.clone()
        } else {
            (0..choice_count).collect()
        }
    }

    /// The source choice index shown in display `slot`.
    pub(super) fn source_choice(&self, slot: usize, choice_count: usize) -> usize {
        self.display_order(choice_count)
            .get(slot)
            .copied()
            .unwrap_or(slot)
    }
}

/// Normalized question identity: uppercase with collapsed whitespace, the same
/// rule the cartridge save uses to recognize answered questions.
pub(super) fn question_identity(question: &str) -> String {
    question
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_uppercase()
}

/// Where a missed question's review copy is inserted: exactly `RETRY_GAP`
/// other questions after the miss. A retry always waits `RETRY_GAP` questions
/// and may carry into the next batch, so the current batch still closes on
/// time. `None` when the deck does not yet reach that far.
pub(super) fn retry_insertion_index(current: usize, question_count: usize) -> Option<usize> {
    let index = current + 1 + RETRY_GAP;
    (index <= question_count).then_some(index)
}

/// Where a miss's review copy went.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RetrySlot {
    /// Inserted into the deck at this index.
    At(usize),
    /// Held in `GameState::deferred_retries` until the deck reaches its gap.
    Deferred,
}

/// Inserts a same-launch review copy of the missed question at `current` and
/// shifts every batch end at or beyond the insertion point, so a copy past
/// the current batch end joins the next batch. When the deck is too short for
/// the gap, the copy is pushed onto `deferred` with the index it is due at.
pub(super) fn schedule_retry(
    questions: &mut Vec<QuizQuestion>,
    batch_ends: &mut [usize],
    deferred: &mut Vec<(usize, QuizQuestion)>,
    current: usize,
) -> Option<RetrySlot> {
    let mut review = questions.get(current)?.clone();
    review.review = Review::InSession;
    let Some(index) = retry_insertion_index(current, questions.len()) else {
        deferred.push((current + 1 + RETRY_GAP, review));
        return Some(RetrySlot::Deferred);
    };
    insert_retry(questions, batch_ends, index, review);
    Some(RetrySlot::At(index))
}

pub(super) fn insert_retry(
    questions: &mut Vec<QuizQuestion>,
    batch_ends: &mut [usize],
    index: usize,
    review: QuizQuestion,
) {
    questions.insert(index, review);
    for end in batch_ends.iter_mut().filter(|end| **end >= index) {
        *end += 1;
    }
}

/// Places deferred review copies once a delivery has grown the deck, in due
/// order, each at its due index, so the full retry gap holds and at least one
/// fresh question precedes it after an Oracle wait. A copy the deck does not
/// reach yet waits for a later delivery; if the run ends first, its lesson is
/// still outstanding and requeues for the next run.
pub(super) fn place_deferred_retries(state: &mut GameState) {
    if !state.run_is_live() {
        return;
    }
    let (Some(run), Some(cartridge)) = (state.quiz.as_ref(), state.cartridge.as_mut()) else {
        return;
    };
    let mut deferred = std::mem::take(&mut state.deferred_retries);
    deferred.sort_by_key(|(due, _)| *due);
    let mut waiting = Vec::new();
    for (due, review) in deferred {
        // `due` already exceeds every index the player had reached when the
        // copy was deferred; the guard keeps that true however the deck moved.
        let index = due.max(run.question + 1);
        if index > cartridge.questions.len() {
            waiting.push((due, review));
            continue;
        }
        insert_retry(
            &mut cartridge.questions,
            &mut state.batch_ends,
            index,
            review,
        );
    }
    state.deferred_retries = waiting;
}

/// Lit mastery runes (0-3) for `concept`, gated by recent accuracy and the
/// lens's open misses in `lessons`: the one stage rule the footer, the Codex,
/// and the lens-wake check share.
pub(super) fn lens_stage(mastery: &Mastery, lessons: &[Lesson], concept: Concept) -> usize {
    mastery
        .get(&concept)
        .copied()
        .unwrap_or_default()
        .stage_with(pending_reviews(lessons, concept))
}

/// Upserts the journal entry for a committed question, keyed by identity. A
/// miss remembers the source choice `picked` and the misconception it reveals;
/// a later correct attempt clears the outstanding flag but keeps that
/// misconception. Every attempt clears the peeked flag.
pub(super) fn record_lesson(
    lessons: &mut Vec<Lesson>,
    question: &QuizQuestion,
    correct: bool,
    picked: usize,
) {
    let identity = question_identity(&question.question);
    let existing = lessons
        .iter()
        .position(|entry| question_identity(&entry.question) == identity);
    let misconception = if correct {
        existing.and_then(|index| lessons[index].misconception.clone())
    } else {
        question.choices.get(picked).map(|choice| {
            (
                choice.clone(),
                choice_rationale(question, picked)
                    .unwrap_or_default()
                    .to_string(),
            )
        })
    };
    let lesson = Lesson {
        question: question.question.clone(),
        answer: question
            .choices
            .get(question.answer)
            .cloned()
            .unwrap_or_default(),
        rationale: choice_rationale(question, question.answer)
            .unwrap_or_default()
            .to_string(),
        concept: question.concept,
        outstanding: !correct,
        misconception,
        peeked: false,
        spaced_check: false,
    };
    match existing {
        Some(index) => lessons[index] = lesson,
        None => lessons.push(lesson),
    }
}

/// The rationale for source choice `index`, when the question carries a full
/// set of rationales and that one is not blank.
pub(super) fn choice_rationale(question: &QuizQuestion, index: usize) -> Option<&str> {
    if question.rationales.len() != question.choices.len() {
        return None;
    }
    question
        .rationales
        .get(index)
        .map(|rationale| rationale.trim())
        .filter(|rationale| !rationale.is_empty())
}

/// Starts a fresh run on a deck without the previous runs' answers.
pub(super) fn start_quiz_run(state: &mut GameState) {
    retire_consumed_questions(state);
    state.oracle_data = 0;
    state.oracle_bug_hits = 0;
    let mut run = QuizRun::new();
    run.ledger.stages_at_start = Concept::ALL.map(|concept| state.mastery_stage(concept));
    state.quiz = Some(run);
}

pub(super) fn begin_quiz_run(state: &mut GameState) {
    start_quiz_run(state);
    state.signal(SceneSignal::HeroReady);
}

/// Leaves the active run through the scene graph's Back route, clearing it
/// so the next run starts fresh.
pub(super) fn leave_quiz_run(state: &mut GameState) {
    if state.can_signal(SceneSignal::Back) {
        state.quiz = None;
        state.deferred_retries.clear();
        state.signal(SceneSignal::Back);
    }
}

/// Derives the display order once for the question that just became current.
/// The attempt number counts every earlier commitment at the same identity on
/// this cartridge, across runs and saved launches; a question flagged for
/// review starts at attempt 1 or later. Either way a returning question never
/// reappears in the order the player last saw it in.
pub(super) fn present_current_question(state: &mut GameState) {
    let Some(run) = state.quiz.as_mut() else {
        return;
    };
    if run.presented == Some(run.question) {
        return;
    }
    let Some(cartridge) = state.cartridge.as_ref() else {
        return;
    };
    let Some(question) = cartridge.questions.get(run.question) else {
        return;
    };
    let identity = question_identity(&question.question);
    let attempt = cartridge
        .question_attempts
        .get(&identity)
        .copied()
        .unwrap_or(0)
        .max(u32::from(question.review.is_review()));
    run.order = if question.choices.len() == 4 {
        learning::presentation_order(&identity, attempt).to_vec()
    } else {
        (0..question.choices.len()).collect()
    };
    run.attempt = attempt;
    run.presented = Some(run.question);
    run.selected = 0;
}

/// Commits the focused choice: scores it, records evidence and the lesson,
/// schedules a spaced retry for a survivable miss, and opens the lesson card.
pub(super) fn commit_answer(state: &mut GameState, effects: &mut Effects) {
    let (Some(run), Some(cartridge)) = (state.quiz.as_mut(), state.cartridge.as_mut()) else {
        return;
    };
    let Some(question) = cartridge.questions.get(run.question).cloned() else {
        return;
    };
    let picked = run.source_choice(run.selected, question.choices.len());
    let correct = picked == question.answer;
    if correct {
        run.streak += 1;
        run.score = run.score.saturating_add(score_award_for_streak(run.streak));
    } else {
        run.hearts = run.hearts.saturating_sub(1);
        run.streak = 0;
    }
    run.feedback = Some((correct, QUIZ_FEEDBACK_TICKS));
    // The banner and cue celebrate every corrected miss; only a later-launch
    // redemption counts as mastery evidence (see `learning::Review`).
    run.redeemed = correct && question.review.is_review();
    cartridge.question_attempts.insert(
        question_identity(&question.question),
        run.attempt.saturating_add(1),
    );

    // A delivery can repeat a stem the journal already holds (a miss or a
    // same-launch relearning is not retired, so the loader keeps it). Such a
    // copy is not a first try: it counts as relearning, never as evidence.
    let identity = question_identity(&question.question);
    let review = match question.review {
        Review::Fresh
            if cartridge
                .lessons
                .iter()
                .any(|lesson| question_identity(&lesson.question) == identity) =>
        {
            Review::InSession
        }
        review => review,
    };
    // The debrief's first-try tally follows the same grading as evidence.
    run.ledger.record(review.is_review(), correct);
    // Revealing a pending answer in the Codex first makes a success
    // relearning too (see `LensRecord::record`).
    let peeked = cartridge
        .lessons
        .iter()
        .any(|lesson| lesson.peeked && question_identity(&lesson.question) == identity);
    let evidence = AnswerEvidence {
        question: question.question.clone(),
        concept: question.concept,
        correct,
        review,
        picked: (!correct).then_some(picked),
        picked_choice: (!correct)
            .then(|| question.choices.get(picked).cloned())
            .flatten(),
        peeked,
    };
    // The lens-wake check reads the gated stage after both the evidence and
    // the journal entry land, since an open miss holds rune III back.
    let stage_before = question
        .concept
        .map(|concept| lens_stage(&cartridge.mastery, &cartridge.lessons, concept));
    learning::record_evidence(&mut cartridge.mastery, &evidence);
    record_lesson(&mut cartridge.lessons, &question, correct, picked);
    run.lens_woke = question
        .concept
        .zip(stage_before)
        .and_then(|(concept, before)| {
            let after = lens_stage(&cartridge.mastery, &cartridge.lessons, concept);
            (after > before).then_some((concept, after))
        });
    run.retry_note = (!correct).then_some(RetryNote::NextRun);
    if !correct && run.hearts > 0 {
        let slot = schedule_retry(
            &mut cartridge.questions,
            &mut state.batch_ends,
            &mut state.deferred_retries,
            run.question,
        );
        run.retry_note = slot.map(|slot| match slot {
            RetrySlot::At(index) => RetryNote::In(index - run.question - 1),
            RetrySlot::Deferred => RetryNote::Later,
        });
    }
    state.consumed_questions = state.consumed_questions.max(run.question + 1);
    effects.0.push_back(EngineEffect::RecordAnsweredQuestion {
        cartridge_id: cartridge.id.clone(),
        evidence,
    });
}

/// Leaves the lesson card: ends the run on a broken ward, otherwise advances
/// to the next question and completes the batch exactly at its (possibly
/// retry-shifted) end once it holds a full `QUESTION_BATCH_SIZE` new questions.
pub(super) fn continue_after_lesson(state: &mut GameState) {
    let question_count = state.question_count();
    let Some(batch) = state.quiz.as_ref().map(|run| run.completed_batches) else {
        return;
    };
    let batch_full = state.batch_is_full(batch);
    let next_batch_end = state.batch_ends.get(batch).copied();
    let Some(run) = state.quiz.as_mut() else {
        return;
    };
    run.feedback = None;
    run.redeemed = false;
    run.retry_note = None;
    run.lens_woke = None;
    let mut next_signal = None;
    if run.hearts == 0 {
        next_signal = Some(SceneSignal::HeartsEmpty);
    } else {
        run.question += 1;
        run.selected = 0;
        if next_batch_end == Some(run.question) {
            if batch_full {
                run.completed_batches += 1;
                run.level += 1;
                run.leveled_up = true;
                run.ledger.close_batch();
            } else if run.question < question_count {
                // A short batch with questions after it joins the next one,
                // so the level-up waits for a full batch.
                state.batch_ends.remove(batch);
                if batch < state.batch_levels.len() {
                    let level = state.batch_levels.remove(batch);
                    if let Some(next) = state.batch_levels.get_mut(batch) {
                        *next = (*next).max(level);
                    }
                }
            }
            // A short last batch stays open; NeedsQuestion below tops it up.
        }
        if run.leveled_up {
            run.leveled_up = false;
            next_signal = Some(SceneSignal::BatchComplete);
        } else if run.question >= question_count {
            next_signal = Some(SceneSignal::NeedsQuestion);
        }
    }
    if let Some(signal) = next_signal {
        state.signal(signal);
    }
    if state.screen == Screen::Quiz {
        present_current_question(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn committing_an_answer_records_that_question_for_future_runs() {
        let mut engine = playing_quiz_engine();
        let _ = engine.take_effects();

        commit(&mut engine, true);

        assert!(engine.take_effects().iter().any(|effect| matches!(
            effect,
            EngineEffect::RecordAnsweredQuestion {
                cartridge_id,
                evidence,
            } if cartridge_id == "/tmp/engine-test"
                && evidence.question == "WHO OWNS THE GAME LOOP?"
                && evidence.correct
        )));
    }

    #[test]
    fn the_run_ledger_counts_first_tries_redemptions_and_open_reviews() {
        let mut engine = batch_quiz_engine(QUESTION_BATCH_SIZE);
        // A second batch is queued, so every miss's review keeps its full gap.
        {
            let mut state = engine.app.world_mut().resource_mut::<GameState>();
            let questions = (10..10 + QUESTION_BATCH_SIZE)
                .map(concept_question)
                .collect();
            append_question_batch(&mut state, questions, 2);
        }
        // q0 and q1 right, q2 missed (its review closes the batch three
        // questions later), q3 and q4 right, q5 missed (its review carries
        // into the next batch), the q2 review relearned, and after the
        // level-up two new questions right and the q5 review missed on the
        // last ward.
        for correct in [true, true, false, true, true, false] {
            assert_eq!(current_question(&engine).review, Review::Fresh);
            commit(&mut engine, correct);
            finish_lesson(&mut engine);
        }
        assert_eq!(current_question(&engine).review, Review::InSession);
        commit(&mut engine, true);
        finish_lesson(&mut engine);
        assert_eq!(engine.screen(), Screen::LevelUp);
        while engine.screen() == Screen::LevelUp {
            engine.update();
        }
        for _ in 0..2 {
            assert_eq!(current_question(&engine).review, Review::Fresh);
            commit(&mut engine, true);
            finish_lesson(&mut engine);
        }
        assert_eq!(current_question(&engine).review, Review::InSession);
        commit(&mut engine, false);

        let state = engine_state(&engine);
        let ledger = &state.quiz.as_ref().unwrap().ledger;
        assert_eq!(ledger.first_try, 8);
        assert_eq!(ledger.first_try_right, 6);
        assert_eq!(ledger.redeemed, 1);
        assert_eq!(ledger.last_batch, (4, 6), "the first batch's first tries");
        assert_eq!(
            ledger.batch_first_try, 2,
            "the second batch never completed"
        );
        assert_eq!(
            open_reviews(state),
            state
                .lessons()
                .iter()
                .filter(|lesson| lesson.outstanding)
                .count()
        );
        assert_eq!(open_reviews(state), 1, "q2 was relearned; q5 stays open");
        let rows = ledger_rows(state, state.quiz.as_ref().unwrap());
        assert_eq!(
            rows.iter()
                .map(|(text, _)| text.as_str())
                .collect::<Vec<_>>(),
            // Six first-try successes earn three runes by volume, but three
            // of the newest five graded answers (60%) and q5's open miss
            // hold rune III back; the same-launch relearning is no evidence.
            ["1ST TRY 6/8", "REDEEMED 01", "REVIEW 01", "ROLES II"]
        );
        assert_eq!(state.mastery_cracks(Concept::Responsibility), 1);
        finish_lesson(&mut engine);
        assert_eq!(engine.screen(), Screen::GameOver);

        let mut state = engine.app.world_mut().resource_mut::<GameState>();
        start_quiz_run(&mut state);
        let ledger = &state.quiz.as_ref().unwrap().ledger;
        assert_eq!(
            (ledger.first_try, ledger.first_try_right, ledger.redeemed),
            (0, 0, 0),
            "a new run starts a fresh ledger"
        );
        assert_eq!(
            ledger.stages_at_start[1], 2,
            "from the roles runes already lit"
        );
        assert_eq!(woken_lens(&state), None);
    }

    #[test]
    fn a_completed_batch_snapshots_its_first_tries_for_the_level_up() {
        let mut engine = batch_quiz_engine(QUESTION_BATCH_SIZE);
        // q0 missed; its review arrives after three more and joins the batch.
        commit(&mut engine, false);
        finish_lesson(&mut engine);
        while engine.screen() == Screen::Quiz {
            commit(&mut engine, true);
            finish_lesson(&mut engine);
        }
        assert_eq!(engine.screen(), Screen::LevelUp);
        let ledger = &engine_state(&engine).quiz.as_ref().unwrap().ledger;
        assert_eq!(ledger.last_batch, (5, 6), "six first tries, one missed");
        assert_eq!(ledger.redeemed, 1);
        assert_eq!(
            (ledger.batch_first_try, ledger.batch_first_try_right),
            (0, 0),
            "the next batch counts from zero"
        );
        assert_eq!((ledger.first_try, ledger.first_try_right), (6, 5));
    }

    #[test]
    fn correct_answers_apply_the_staged_flow_multiplier_to_runtime_score() {
        let mut engine = playing_quiz_engine();
        let expected_scores = [100, 200, 400, 600, 800, 1_100, 1_400, 1_700, 2_000];
        focus_choice(&mut engine, true);

        for (index, expected_score) in expected_scores.into_iter().enumerate() {
            issue(
                &mut engine,
                EngineCommand::Input {
                    button: Button::A,
                    pressed: true,
                },
            );
            issue(
                &mut engine,
                EngineCommand::Input {
                    button: Button::A,
                    pressed: false,
                },
            );
            let state = engine.app.world().resource::<GameState>();
            let run = state.quiz.as_ref().unwrap();
            assert_eq!(run.streak, index as u32 + 1);
            assert_eq!(run.score, expected_score);
            assert_eq!(
                InsightStage::from_score(run.score).index(),
                [0, 0, 1, 1, 1, 2, 2, 2, 3][index]
            );
            engine
                .app
                .world_mut()
                .resource_mut::<GameState>()
                .quiz
                .as_mut()
                .unwrap()
                .feedback = None;
        }
    }

    #[test]
    fn surviving_a_complete_ai_batch_levels_up() {
        let mut engine = batch_quiz_engine(QUESTION_BATCH_SIZE);

        // Two misses add two review copies, so the batch closes after eight
        // commitments: the six originals plus both retries.
        let batch_length = QUESTION_BATCH_SIZE + 2;
        for (index, wrong) in [true, false, false, true, false, false, false, false]
            .into_iter()
            .enumerate()
        {
            commit(&mut engine, !wrong);
            finish_lesson(&mut engine);
            if index + 1 < batch_length {
                assert_eq!(engine.screen(), Screen::Quiz, "commitment {index}");
            }
        }

        assert_eq!(engine.screen(), Screen::LevelUp);
        let state = engine_state(&engine);
        let run = state.quiz.as_ref().unwrap();
        assert_eq!(run.level, 2);
        assert_eq!(run.hearts, 1);
        assert_eq!(run.question, batch_length);
        assert_eq!(state.batch_ends, vec![batch_length]);
    }

    #[test]
    fn losing_the_last_heart_at_a_batch_boundary_does_not_level_up() {
        let mut engine = GameEngine::new();
        let mut cartridge = quiz_cartridge();
        cartridge.questions.clear();
        issue(&mut engine, EngineCommand::Cartridge(Some(cartridge)));
        let _ = engine.take_effects();
        issue(&mut engine, EngineCommand::Power(true));
        finish_opening(&mut engine);
        for button in [Button::Start, Button::A, Button::Start] {
            issue(
                &mut engine,
                EngineCommand::Input {
                    button,
                    pressed: true,
                },
            );
            issue(
                &mut engine,
                EngineCommand::Input {
                    button,
                    pressed: false,
                },
            );
        }
        deliver(
            &mut engine,
            "/tmp/engine-test",
            (0..3).map(concept_question).collect(),
        );
        for _ in 0..75 {
            engine.update();
        }

        for _ in 0..3 {
            commit(&mut engine, false);
            finish_lesson(&mut engine);
        }

        assert_eq!(engine.screen(), Screen::GameOver);
        let state = engine.app.world().resource::<GameState>();
        let run = state.quiz.as_ref().unwrap();
        assert_eq!(run.level, 1);
        assert_eq!(run.completed_batches, 0);
        // The deck was too short for either survivable miss to keep its retry
        // gap, so both copies were deferred; the final, ward-breaking miss
        // had none. Once the run is over, the three committed questions leave
        // the deck, the deferred copies are dropped, and each outstanding
        // identity returns once, in play order, as a same-launch review for
        // the next run.
        assert!(state.deferred_retries.is_empty());
        let questions = &state.cartridge.as_ref().unwrap().questions;
        assert_eq!(
            questions
                .iter()
                .map(|question| (question.question.clone(), question.review))
                .collect::<Vec<_>>(),
            [0, 1, 2].map(|index| (concept_question(index).question, Review::InSession))
        );
        assert_eq!(state.batch_ends, vec![3]);
    }

    #[test]
    fn four_correct_answers_do_not_level_up_before_the_batch_ends() {
        let mut engine = GameEngine::new();
        let mut cartridge = quiz_cartridge();
        cartridge.questions = vec![cartridge.questions[0].clone(); 6];
        issue(&mut engine, EngineCommand::Cartridge(Some(cartridge)));
        issue(&mut engine, EngineCommand::Power(true));
        finish_opening(&mut engine);
        for button in [Button::Start, Button::A, Button::Start] {
            issue(
                &mut engine,
                EngineCommand::Input {
                    button,
                    pressed: true,
                },
            );
            issue(
                &mut engine,
                EngineCommand::Input {
                    button,
                    pressed: false,
                },
            );
        }
        for _ in 0..75 {
            engine.update();
        }
        assert_eq!(engine.screen(), Screen::Quiz);

        for _ in 0..4 {
            commit(&mut engine, true);
            finish_lesson(&mut engine);
        }

        assert_eq!(engine.screen(), Screen::Quiz);
        let state = engine.app.world().resource::<GameState>();
        let run = state.quiz.as_ref().unwrap();
        assert_eq!(run.level, 1);
        assert_eq!(run.completed_batches, 0);
        assert!(engine.take_effects().iter().any(|effect| matches!(
            effect,
            EngineEffect::RequestQuestions {
                cartridge_id,
                level: 2,
                count: 6,
                ..
            } if cartridge_id == "/tmp/engine-test"
        )));
    }

    #[test]
    fn question_identity_matches_the_save_normalization() {
        assert_eq!(
            question_identity("  who owns\tthe\n game   loop? "),
            "WHO OWNS THE GAME LOOP?"
        );
        assert_eq!(
            question_identity("WHO OWNS THE GAME LOOP?"),
            question_identity("who owns the game loop?")
        );
    }

    #[test]
    fn committing_maps_the_focused_display_slot_to_its_source_choice() {
        for correct in [true, false] {
            let mut engine = batch_quiz_engine(1);
            let _ = engine.take_effects();
            let question = current_question(&engine);
            let slot = answer_slot(&engine);
            {
                let run = engine_state(&engine).quiz.as_ref().unwrap();
                let mut order = run.order.clone();
                assert_eq!(run.display_order(4)[slot], question.answer);
                order.sort_unstable();
                assert_eq!(order, [0, 1, 2, 3], "the display order is a permutation");
            }

            commit(&mut engine, correct);

            let run = engine_state(&engine).quiz.as_ref().unwrap();
            assert_eq!(run.feedback.map(|(result, _)| result), Some(correct));
            assert!(engine.take_effects().iter().any(|effect| matches!(
                effect,
                EngineEffect::RecordAnsweredQuestion { evidence, .. }
                    if evidence.correct == correct
                        && evidence.concept == Some(Concept::Responsibility)
                        && evidence.review == Review::Fresh
            )));
        }
    }

    #[test]
    fn provider_first_answers_spread_across_every_display_slot() {
        let mut cartridge = quiz_cartridge();
        cartridge.questions = (0..60).map(concept_question).collect();
        let mut state = GameState {
            cartridge: Some(cartridge),
            quiz: Some(QuizRun::new()),
            ..Default::default()
        };
        let mut slot_counts = [0; 4];
        for index in 0..60 {
            state.quiz.as_mut().unwrap().question = index;
            present_current_question(&mut state);
            let run = state.quiz.as_ref().unwrap();
            assert_eq!(run.presented, Some(index));
            assert_eq!(run.selected, 0);
            let slot = run.order.iter().position(|source| *source == 0).unwrap();
            slot_counts[slot] += 1;
        }
        assert!(
            slot_counts.iter().all(|count| *count >= 5),
            "the answer must not favor a slot: {slot_counts:?}"
        );
    }

    #[test]
    fn every_retry_waits_the_full_gap_and_may_carry_into_the_next_batch() {
        assert_eq!(retry_insertion_index(1, 12), Some(5));
        assert_eq!(retry_insertion_index(5, 12), Some(9));
        assert_eq!(retry_insertion_index(2, 6), Some(6), "the deck's end is ok");
        assert_eq!(retry_insertion_index(3, 6), None);
        assert_eq!(retry_insertion_index(0, 1), None);

        // A miss at every slot of a full batch that another batch follows.
        for slot in 0..QUESTION_BATCH_SIZE {
            let mut questions: Vec<_> = (0..12).map(concept_question).collect();
            let mut batch_ends = vec![6, 12];
            let mut deferred = Vec::new();
            let placed = schedule_retry(&mut questions, &mut batch_ends, &mut deferred, slot);
            let Some(RetrySlot::At(index)) = placed else {
                panic!("slot {slot}: {placed:?}");
            };
            assert!(deferred.is_empty());
            let between = index - slot - 1;
            assert_eq!(between, RETRY_GAP, "slot {slot} retried at {index}");
            assert_eq!(questions[index].question, concept_question(slot).question);
            assert_eq!(questions[index].review, Review::InSession);
            assert_eq!(questions[slot].review, Review::Fresh);
            // A copy that fits the current batch extends it; a later one
            // joins the next batch, so the current batch closes on time.
            let expected = if index <= 6 { vec![7, 13] } else { vec![6, 13] };
            assert_eq!(batch_ends, expected, "slot {slot}");
        }

        // With no batch after it, a late miss waits for the next delivery.
        let mut questions: Vec<_> = (0..6).map(concept_question).collect();
        let mut batch_ends = vec![6];
        let mut deferred = Vec::new();
        assert_eq!(
            schedule_retry(&mut questions, &mut batch_ends, &mut deferred, 5),
            Some(RetrySlot::Deferred)
        );
        assert_eq!((questions.len(), batch_ends.clone()), (6, vec![6]));
        assert_eq!(deferred.len(), 1);
        assert_eq!(deferred[0].0, 5 + 1 + RETRY_GAP);
        assert_eq!(deferred[0].1.review, Review::InSession);
        assert_eq!(
            schedule_retry(&mut questions, &mut batch_ends, &mut deferred, 99),
            None
        );
    }

    #[test]
    fn a_miss_on_the_last_slot_levels_up_and_returns_three_questions_later() {
        let mut engine = batch_quiz_engine(QUESTION_BATCH_SIZE);
        // A second batch is already queued behind the first.
        {
            let mut state = engine.app.world_mut().resource_mut::<GameState>();
            let questions = (10..10 + QUESTION_BATCH_SIZE)
                .map(concept_question)
                .collect();
            append_question_batch(&mut state, questions, 2);
            assert_eq!(state.batch_ends, vec![6, 12]);
        }
        for _ in 0..QUESTION_BATCH_SIZE - 1 {
            commit(&mut engine, true);
            finish_lesson(&mut engine);
        }
        let missed = current_question(&engine);
        commit(&mut engine, false);
        assert_eq!(engine_state(&engine).batch_ends, vec![6, 13]);
        finish_lesson(&mut engine);
        assert_eq!(engine.screen(), Screen::LevelUp, "the batch closes at six");
        assert_eq!(engine_state(&engine).quiz.as_ref().unwrap().level, 2);
        while engine.screen() == Screen::LevelUp {
            engine.update();
        }
        assert_eq!(engine.screen(), Screen::Quiz);
        for _ in 0..RETRY_GAP {
            assert_eq!(current_question(&engine).review, Review::Fresh);
            commit(&mut engine, true);
            finish_lesson(&mut engine);
        }
        let retry = current_question(&engine);
        assert_eq!(retry.question, missed.question);
        assert_eq!(retry.review, Review::InSession);
    }

    #[test]
    fn a_deferred_retry_lands_at_its_due_index_after_the_next_delivery() {
        let mut engine = batch_quiz_engine(QUESTION_BATCH_SIZE);
        for _ in 0..3 {
            commit(&mut engine, true);
            finish_lesson(&mut engine);
        }
        let missed = current_question(&engine);
        commit(&mut engine, false);
        {
            let state = engine_state(&engine);
            assert_eq!(state.deferred_retries.len(), 1, "slot 3 cannot fit a gap");
            assert_eq!(state.batch_ends, vec![6]);
            assert_eq!(
                state.quiz.as_ref().unwrap().retry_note,
                Some(RetryNote::Later),
                "the lesson card promises no distance the deck cannot keep yet"
            );
        }
        finish_lesson(&mut engine);
        for _ in 0..2 {
            commit(&mut engine, true);
            finish_lesson(&mut engine);
        }
        assert_eq!(engine.screen(), Screen::LevelUp);
        deliver(
            &mut engine,
            "/tmp/engine-test",
            (10..10 + QUESTION_BATCH_SIZE)
                .map(concept_question)
                .collect(),
        );
        let state = engine_state(&engine);
        let run = state.quiz.as_ref().unwrap();
        assert!(state.deferred_retries.is_empty());
        let questions = &state.cartridge.as_ref().unwrap().questions;
        let due = 3 + 1 + RETRY_GAP;
        assert!(due > run.question, "a fresh question comes first");
        assert_eq!(questions[due].question, missed.question);
        assert_eq!(questions[due].review, Review::InSession);
        assert_eq!(state.batch_ends, vec![6, 13]);
    }

    #[test]
    fn a_delivered_repeat_of_a_journaled_stem_is_relearning_not_a_first_try() {
        let mut engine = batch_quiz_engine(QUESTION_BATCH_SIZE);
        {
            // A later delivery repeated the first stem as a "fresh" question.
            let mut state = engine.app.world_mut().resource_mut::<GameState>();
            state.cartridge.as_mut().unwrap().questions[1] = concept_question(0);
        }
        commit(&mut engine, false);
        finish_lesson(&mut engine);
        let _ = engine.take_effects();
        assert_eq!(current_question(&engine).review, Review::Fresh);
        commit(&mut engine, true);
        assert!(engine.take_effects().iter().any(|effect| matches!(
            effect,
            EngineEffect::RecordAnsweredQuestion { evidence, .. }
                if evidence.correct && evidence.review == Review::InSession
        )));
        let record = engine_state(&engine).lens_record(Concept::Responsibility);
        assert_eq!((record.first_try, record.relearned), (0, 1), "{record:?}");
        assert!(
            !engine_state(&engine).quiz.as_ref().unwrap().redeemed,
            "no REDEEMED banner for a copy that was never flagged as a review"
        );
    }

    #[test]
    fn a_deferred_retry_waits_until_a_delivery_reaches_its_gap() {
        let mut cartridge = quiz_cartridge();
        cartridge.questions = (0..6).map(concept_question).collect();
        let mut state = GameState {
            cartridge: Some(cartridge),
            screen: Screen::Oracle,
            quiz: Some(QuizRun {
                question: 6,
                ..QuizRun::new()
            }),
            batch_ends: vec![6],
            batch_levels: vec![1],
            ..Default::default()
        };
        let mut review = concept_question(5);
        review.review = Review::InSession;
        state.deferred_retries.push((5 + 1 + RETRY_GAP, review));

        // One new question is not enough: placing the copy now would leave
        // only one question between the miss and its retry.
        append_question_batch(&mut state, vec![concept_question(10)], 2);
        assert_eq!(state.deferred_retries.len(), 1);
        assert_eq!(state.cartridge.as_ref().unwrap().questions.len(), 7);

        append_question_batch(&mut state, (11..16).map(concept_question).collect(), 2);
        assert!(state.deferred_retries.is_empty());
        let questions = &state.cartridge.as_ref().unwrap().questions;
        assert_eq!(questions[9].question, concept_question(5).question);
        assert_eq!(questions[9].review, Review::InSession);
        assert_eq!(state.batch_ends, vec![6, 13]);

        // Outside a live run nothing is placed; a new run requeues instead.
        state.screen = Screen::GameOver;
        state.deferred_retries.push((20, concept_question(1)));
        append_question_batch(&mut state, (20..26).map(concept_question).collect(), 3);
        assert_eq!(state.deferred_retries.len(), 1);
    }

    #[test]
    fn a_missed_question_returns_in_a_new_slot_and_redeems_its_lesson() {
        let mut engine = batch_quiz_engine(QUESTION_BATCH_SIZE);
        let missed = current_question(&engine);
        let first_slot = answer_slot(&engine);
        let picked = engine_state(&engine)
            .quiz
            .as_ref()
            .unwrap()
            .display_order(missed.choices.len())[(first_slot + 1) % missed.choices.len()];
        let misconception = Some((
            missed.choices[picked].clone(),
            missed.rationales[picked].clone(),
        ));

        commit(&mut engine, false);
        {
            let state = engine_state(&engine);
            let cartridge = state.cartridge.as_ref().unwrap();
            assert_eq!(cartridge.questions.len(), QUESTION_BATCH_SIZE + 1);
            assert_eq!(state.batch_ends, vec![QUESTION_BATCH_SIZE + 1]);
            assert_eq!(
                cartridge.lessons,
                vec![Lesson {
                    question: missed.question.clone(),
                    answer: missed.choices[0].clone(),
                    rationale: missed.rationales[0].clone(),
                    concept: Some(Concept::Responsibility),
                    outstanding: true,
                    misconception: misconception.clone(),
                    peeked: false,
                    spaced_check: false,
                }],
                "the journal remembers the pick and the misconception it reveals"
            );
            assert_eq!(cartridge.mastery[&Concept::Responsibility].missed, 1);
        }
        finish_lesson(&mut engine);

        for _ in 0..RETRY_GAP {
            assert_eq!(current_question(&engine).review, Review::Fresh);
            commit(&mut engine, true);
            finish_lesson(&mut engine);
        }

        let review = current_question(&engine);
        assert_eq!(review.review, Review::InSession);
        assert_eq!(review.question, missed.question);
        assert_ne!(
            answer_slot(&engine),
            first_slot,
            "a retry must move the answer so position cannot be memorized"
        );
        assert_eq!(engine_state(&engine).quiz.as_ref().unwrap().attempt, 1);

        let _ = engine.take_effects();
        commit(&mut engine, true);
        let state = engine_state(&engine);
        let run = state.quiz.as_ref().unwrap();
        assert!(run.redeemed);
        assert_eq!(quiz_feedback_banner(run), "REDEEMED");
        let cartridge = state.cartridge.as_ref().unwrap();
        assert_eq!(cartridge.lessons.len(), 1 + RETRY_GAP);
        assert!(
            !cartridge.lessons[0].outstanding,
            "redemption clears the miss"
        );
        assert_eq!(
            cartridge.lessons[0].misconception, misconception,
            "a learned lesson still recalls the misconception it replaced"
        );
        assert_eq!(
            cartridge.mastery[&Concept::Responsibility],
            learning::LensRecord {
                first_try: RETRY_GAP as u32,
                missed: 1,
                // A same-launch retry is relearning: it lights no rune and
                // is not graded in the recent window.
                relearned: 1,
                recent: 0b0111,
                recent_len: 1 + RETRY_GAP as u8,
                ..learning::LensRecord::default()
            }
        );
        assert!(engine.take_effects().iter().any(|effect| matches!(
            effect,
            EngineEffect::RecordAnsweredQuestion { evidence, .. }
                if evidence.correct && evidence.review == Review::InSession
        )));
    }

    #[test]
    fn a_broken_ward_sends_the_miss_to_the_next_run() {
        let mut engine = batch_quiz_engine(QUESTION_BATCH_SIZE);
        for hearts in [2, 1] {
            commit(&mut engine, false);
            assert_eq!(engine_state(&engine).quiz.as_ref().unwrap().hearts, hearts);
            assert!(matches!(retry_note(&engine), Some(RetryNote::In(_))));
            finish_lesson(&mut engine);
        }
        commit(&mut engine, false);
        assert_eq!(retry_note(&engine), Some(RetryNote::NextRun));
        assert_eq!(RetryNote::NextRun.label(), "NEXT RUN");
        assert_eq!(RetryNote::In(0).label(), "UP NEXT");
        assert_eq!(RetryNote::In(9).label(), "BACK IN 9");
        assert_eq!(RetryNote::In(12).label(), "LATER");
        assert_eq!(RetryNote::Later.label(), "LATER");
    }

    #[test]
    fn a_review_that_outlives_its_run_returns_in_a_new_layout() {
        let mut engine = batch_quiz_engine(QUESTION_BATCH_SIZE);
        let missed = concept_question(0).question;
        let first_slot = answer_slot(&engine);
        commit(&mut engine, false);
        finish_lesson(&mut engine);
        for _ in 0..RETRY_GAP {
            commit(&mut engine, true);
            finish_lesson(&mut engine);
        }
        assert_eq!(current_question(&engine).question, missed);
        assert_eq!(engine_state(&engine).quiz.as_ref().unwrap().attempt, 1);
        let review_slot = answer_slot(&engine);
        assert_ne!(review_slot, first_slot);
        commit(&mut engine, false);
        finish_lesson(&mut engine);

        // Leave before the second review copy comes up, then start again.
        press(&mut engine, Button::B);
        press(&mut engine, Button::B);
        assert_eq!(engine.screen(), Screen::QuizMenu);
        press(&mut engine, Button::A);
        press(&mut engine, Button::Start);
        for _ in 0..75 {
            engine.update();
        }
        assert_eq!(engine.screen(), Screen::Quiz);
        while current_question(&engine).question != missed {
            commit(&mut engine, true);
            finish_lesson(&mut engine);
        }
        assert_eq!(current_question(&engine).review, Review::InSession);
        assert_eq!(
            engine_state(&engine).quiz.as_ref().unwrap().attempt,
            2,
            "the rotation continues from the run that missed it"
        );
        assert_ne!(answer_slot(&engine), review_slot);
    }

    #[test]
    fn a_saved_review_resumes_its_choice_rotation_after_a_relaunch() {
        let mut review = concept_question(0);
        review.review = Review::Spaced;
        let identity = question_identity(&review.question);
        let mut cartridge = quiz_cartridge();
        cartridge.questions = vec![review];
        cartridge.question_attempts.insert(identity.clone(), 2);
        let mut engine = GameEngine::new();
        issue(&mut engine, EngineCommand::Cartridge(Some(cartridge)));
        issue(&mut engine, EngineCommand::Power(true));
        finish_opening(&mut engine);
        for button in [Button::Start, Button::A, Button::Start] {
            press(&mut engine, button);
        }
        for _ in 0..75 {
            engine.update();
        }
        assert_eq!(engine.screen(), Screen::Quiz);
        let run = engine_state(&engine).quiz.as_ref().unwrap();
        assert_eq!(run.attempt, 2);
        assert_eq!(
            run.order,
            learning::presentation_order(&identity, 2).to_vec()
        );

        commit(&mut engine, false);
        assert_eq!(
            engine_state(&engine)
                .cartridge
                .as_ref()
                .unwrap()
                .question_attempts[&identity],
            3
        );
    }

    #[test]
    fn a_redemption_after_a_codex_peek_counts_as_relearning() {
        // A miss from an earlier launch returns as a spaced check, which
        // would be a redemption; the player peeked at its answer first.
        let mut missed = concept_question(0);
        missed.review = Review::Spaced;
        let mut cartridge = quiz_cartridge();
        cartridge.questions = vec![missed.clone(), concept_question(1)];
        record_lesson(&mut cartridge.lessons, &missed, false, 1);
        cartridge.lessons[0].peeked = true;
        let mut engine = GameEngine::new();
        issue(&mut engine, EngineCommand::Cartridge(Some(cartridge)));
        issue(&mut engine, EngineCommand::Power(true));
        finish_opening(&mut engine);
        for button in [Button::Start, Button::A, Button::Start] {
            press(&mut engine, button);
        }
        for _ in 0..75 {
            engine.update();
        }
        assert_eq!(engine.screen(), Screen::Quiz);
        assert_eq!(current_question(&engine).question, missed.question);
        assert_eq!(current_question(&engine).review, Review::Spaced);
        let before = engine_state(&engine).lens_record(Concept::Responsibility);
        {
            let lessons = engine_state(&engine).lessons();
            assert!(lessons[0].outstanding && lessons[0].misconception.is_some());
        }

        let _ = engine.take_effects();
        commit(&mut engine, true);
        let state = engine_state(&engine);
        let cartridge = state.cartridge.as_ref().unwrap();
        let after = cartridge.mastery[&Concept::Responsibility];
        assert_eq!(after.redeemed, before.redeemed, "no redemption evidence");
        assert_eq!(after.relearned, before.relearned + 1);
        assert_eq!(after.volume_stage(), before.volume_stage());
        assert_eq!(
            (after.recent, after.recent_len),
            (before.recent, before.recent_len),
            "a success after a peek is not graded"
        );
        let lesson = cartridge
            .lessons
            .iter()
            .find(|lesson| lesson.question == missed.question)
            .unwrap();
        assert!(!lesson.outstanding && !lesson.peeked);
        assert!(engine.take_effects().iter().any(|effect| matches!(
            effect,
            EngineEffect::RecordAnsweredQuestion { evidence, .. }
                if evidence.correct && evidence.peeked && evidence.picked.is_none()
        )));
    }

    /// The whole learning loop across the quiz and the Codex: a miss is
    /// journaled as a pending review, returns after the retry gap, is redeemed,
    /// and the Codex rereads the same journal and mastery the quiz wrote.
    #[test]
    fn a_missed_concept_is_journaled_retried_redeemed_and_reread_in_the_codex() {
        let mut engine = batch_quiz_engine(QUESTION_BATCH_SIZE);
        let missed = current_question(&engine);
        commit(&mut engine, false);
        {
            let cartridge = engine_state(&engine).cartridge.as_ref().unwrap();
            let lesson = cartridge
                .lessons
                .iter()
                .find(|lesson| lesson.question == missed.question)
                .expect("committing a miss journals the lesson");
            assert!(lesson.outstanding, "a miss is pending review");
            assert_eq!(lesson.answer, missed.choices[missed.answer]);
            assert_eq!(lesson.rationale, missed.rationales[missed.answer]);
            assert_eq!(cartridge.mastery[&Concept::Responsibility].missed, 1);
        }
        finish_lesson(&mut engine);

        for _ in 0..RETRY_GAP {
            assert_eq!(current_question(&engine).review, Review::Fresh);
            commit(&mut engine, true);
            finish_lesson(&mut engine);
        }
        let retry = current_question(&engine);
        assert!(
            retry.review.is_review(),
            "the miss returns after the retry gap"
        );
        assert_eq!(retry.question, missed.question);
        commit(&mut engine, true);
        {
            let state = engine_state(&engine);
            assert!(state.quiz.as_ref().unwrap().redeemed);
            let cartridge = state.cartridge.as_ref().unwrap();
            let lesson = cartridge
                .lessons
                .iter()
                .find(|lesson| lesson.question == missed.question)
                .unwrap();
            assert!(!lesson.outstanding, "redemption clears the pending review");
            let record = cartridge.mastery[&Concept::Responsibility];
            assert_eq!(
                (record.missed, record.redeemed, record.relearned),
                (1, 0, 1),
                "a same-launch redemption is relearning, not evidence"
            );
        }
        finish_lesson(&mut engine);

        press(&mut engine, Button::B);
        press(&mut engine, Button::B);
        assert_eq!(engine.screen(), Screen::QuizMenu);
        let journal = engine_state(&engine)
            .cartridge
            .as_ref()
            .unwrap()
            .lessons
            .clone();
        if engine_state(&engine).menu_selected == 0 {
            press(&mut engine, Button::Down);
        }
        press(&mut engine, Button::A);
        assert_eq!(engine.screen(), Screen::Codex);
        for _ in 0..journal.len() {
            press(&mut engine, Button::Right);
            let (_, lesson) = engine_state(&engine)
                .codex_lesson()
                .expect("every journal page shows a lesson");
            assert!(journal.contains(lesson));
        }
        assert_eq!(
            engine_state(&engine).cartridge.as_ref().unwrap().lessons,
            journal,
            "reading the Codex never changes the journal"
        );
    }

    // Tests below were added by a mutation-testing sweep over the quiz flow;
    // each one pins a rule that an earlier suite let a small edit break.

    #[test]
    fn a_miss_on_a_batch_opening_question_waits_the_full_gap_in_its_own_batch() {
        let mut questions: Vec<_> = (0..12).map(concept_question).collect();
        let mut batch_ends = vec![6, 12];
        let mut deferred = Vec::new();
        // Index 6 opens the second batch; the first batch's end (6) is behind
        // it, so the review lands RETRY_GAP questions later in the second.
        assert_eq!(
            schedule_retry(&mut questions, &mut batch_ends, &mut deferred, 6),
            Some(RetrySlot::At(7 + RETRY_GAP))
        );
        assert!(deferred.is_empty());
        assert_eq!(batch_ends, vec![6, 13]);
        assert_eq!(questions[7 + RETRY_GAP].review, Review::InSession);
        assert_eq!(questions[7 + RETRY_GAP].question, questions[6].question);
    }

    #[test]
    fn missing_a_review_again_is_not_a_redemption() {
        let mut engine = batch_quiz_engine(QUESTION_BATCH_SIZE);
        commit(&mut engine, false);
        finish_lesson(&mut engine);
        for _ in 0..RETRY_GAP {
            commit(&mut engine, true);
            finish_lesson(&mut engine);
        }
        assert!(current_question(&engine).review.is_review());

        commit(&mut engine, false);
        let run = engine_state(&engine).quiz.as_ref().unwrap();
        assert!(!run.redeemed, "a missed review redeems nothing");
        assert_eq!(run.ledger.redeemed, 0);
        assert_eq!(quiz_feedback_banner(run), "WARD FRACTURES");
    }

    #[test]
    fn a_short_batch_merged_forward_keeps_the_higher_level() {
        let mut state = GameState::default();
        let mut cartridge = quiz_cartridge();
        cartridge.questions = (0..9).map(concept_question).collect();
        state.cartridge = Some(cartridge);
        state.batch_ends = vec![3, 9];
        state.batch_levels = vec![1, 2];
        state.quiz = Some(QuizRun {
            question: 2,
            feedback: Some((true, 0)),
            ..QuizRun::new()
        });

        continue_after_lesson(&mut state);

        let run = state.quiz.as_ref().unwrap();
        assert_eq!((run.question, run.level, run.completed_batches), (3, 1, 0));
        assert_eq!(state.batch_ends, vec![9]);
        assert_eq!(state.batch_levels, vec![2]);
    }

    #[test]
    fn continuing_from_a_level_up_keeps_the_run() {
        let mut engine = batch_quiz_engine(2 * QUESTION_BATCH_SIZE);
        assert_eq!(
            engine_state(&engine).batch_ends,
            vec![QUESTION_BATCH_SIZE, 2 * QUESTION_BATCH_SIZE]
        );
        commit(&mut engine, false);
        finish_lesson(&mut engine);
        while engine.screen() == Screen::Quiz {
            commit(&mut engine, true);
            finish_lesson(&mut engine);
        }
        assert_eq!(engine.screen(), Screen::LevelUp);
        let before = engine_state(&engine).quiz.clone().unwrap();
        assert_eq!((before.level, before.completed_batches), (2, 1));

        for _ in 0..200 {
            if engine.screen() != Screen::LevelUp {
                break;
            }
            engine.update();
        }
        assert_eq!(engine.screen(), Screen::Quiz);
        let run = engine_state(&engine).quiz.as_ref().unwrap();
        assert_eq!(
            (run.question, run.level, run.completed_batches, run.hearts),
            (before.question, 2, 1, 2),
            "the level-up continues the same run"
        );
        assert_eq!(run.score, before.score);
        assert_eq!(run.ledger, before.ledger);
    }
}
