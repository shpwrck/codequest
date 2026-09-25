use super::*;

pub(super) const QUESTION_BATCH_SIZE: usize = 6;

pub(super) fn request_question_batch(state: &mut GameState, effects: &mut Effects, level: u32) {
    if state.questions_loading
        || state.pending_questions.is_some()
        || state.question_retry_ticks > 0
    {
        return;
    }
    let Some(cartridge) = state
        .cartridge
        .as_ref()
        .filter(|cartridge| cartridge.mode() == CartridgeMode::Quiz)
    else {
        return;
    };
    let cartridge_id = cartridge.id.clone();
    state.question_request_seq = state.question_request_seq.wrapping_add(1);
    state.question_request_level = level;
    effects.0.push_back(EngineEffect::RequestQuestions {
        cartridge_id,
        level,
        count: QUESTION_BATCH_SIZE,
        seq: state.question_request_seq,
    });
    state.questions_loading = true;
    state.question_request_ticks = 0;
}

/// New (non-review) questions in `questions[start..end]`.
pub(super) fn new_question_count(questions: &[QuizQuestion], start: usize, end: usize) -> usize {
    questions
        .iter()
        .take(end)
        .skip(start)
        .filter(|question| !question.review.is_review())
        .count()
}

/// Splits `questions` into batches of `QUESTION_BATCH_SIZE` new questions
/// (review copies ride along without counting), leaving any remainder as an
/// open last batch. Each new batch takes the level of the old batch holding
/// its last question.
pub(super) fn rebatch(
    ends: &[usize],
    levels: &[u32],
    questions: &[QuizQuestion],
) -> (Vec<usize>, Vec<u32>) {
    let level_at = |index: usize| {
        ends.iter()
            .position(|end| index < *end)
            .and_then(|batch| levels.get(batch))
            .or(levels.last())
            .copied()
            .unwrap_or(1)
    };
    let mut new_ends = Vec::new();
    let mut new_levels = Vec::new();
    let mut fresh = 0;
    for (index, question) in questions.iter().enumerate() {
        fresh += usize::from(!question.review.is_review());
        if fresh == QUESTION_BATCH_SIZE || index + 1 == questions.len() {
            new_ends.push(index + 1);
            new_levels.push(level_at(index));
            fresh = 0;
        }
    }
    (new_ends, new_levels)
}

/// Appends a delivery requested at `level`: it first tops the open last
/// batch up to `QUESTION_BATCH_SIZE` new questions, so a level-up always means
/// a full batch was survived, then forms new batches from the rest. Questions
/// already in the deck (queued, or answered this session) or waiting as a
/// deferred review copy are skipped, so a regenerated stem is never queued
/// beside its own review copy, and so are stems the journal holds as
/// answered correctly: a retired (or relearned) question does not replay in
/// this launch even when a delivery parked across runs brings it back.
/// Review copies deferred for their retry gap are then placed in the grown
/// deck.
pub(super) fn append_question_batch(
    state: &mut GameState,
    questions: Vec<QuizQuestion>,
    level: u32,
) {
    let Some(cartridge) = state.cartridge.as_mut() else {
        return;
    };
    let (missed, cleared): (Vec<&Lesson>, Vec<&Lesson>) = cartridge
        .lessons
        .iter()
        .partition(|lesson| lesson.outstanding);
    let missed: HashSet<String> = missed
        .into_iter()
        .map(|lesson| question_identity(&lesson.question))
        .collect();
    let mut queued: HashSet<String> = cartridge
        .questions
        .iter()
        .chain(state.deferred_retries.iter().map(|(_, review)| review))
        .map(|question| question_identity(&question.question))
        .chain(
            cleared
                .into_iter()
                .map(|lesson| question_identity(&lesson.question)),
        )
        .collect();
    let mut incoming = questions
        .into_iter()
        .filter(|question| queued.insert(question_identity(&question.question)))
        .map(|mut question| {
            // A stem the journal holds as missed is a same-launch review,
            // whatever the delivery's (possibly stale) flag says.
            if question.review == Review::Fresh
                && missed.contains(&question_identity(&question.question))
            {
                question.review = Review::InSession;
            }
            question
        });
    // Moves incoming questions onto the deck until `fresh` reaches a full
    // batch of new questions; review items ride along without counting,
    // the same rule `rebatch` and `batch_is_full` use.
    let mut fill = |questions: &mut Vec<QuizQuestion>, mut fresh: usize| {
        while fresh < QUESTION_BATCH_SIZE {
            let Some(question) = incoming.next() else {
                break;
            };
            fresh += usize::from(!question.review.is_review());
            questions.push(question);
        }
    };
    if let Some(last) = state.batch_ends.len().checked_sub(1) {
        let start = last
            .checked_sub(1)
            .map_or(0, |previous| state.batch_ends[previous]);
        let end = state.batch_ends[last];
        if end == cartridge.questions.len() {
            let fresh = new_question_count(&cartridge.questions, start, end);
            fill(&mut cartridge.questions, fresh);
            state.batch_ends[last] = cartridge.questions.len();
        }
    }
    loop {
        let before = cartridge.questions.len();
        fill(&mut cartridge.questions, 0);
        if cartridge.questions.len() == before {
            break;
        }
        state.batch_ends.push(cartridge.questions.len());
        state.batch_levels.push(level);
    }
    place_deferred_retries(state);
}

/// Drops the deck prefix committed in this session so a new run never
/// replays it. Consumed questions whose lesson is still outstanding (missed
/// and not redeemed) return at the front as same-launch review items, then
/// the deck is rebatched from the rebased batch boundaries. Deferred review
/// copies are dropped: their outstanding lessons requeue here instead.
pub(super) fn retire_consumed_questions(state: &mut GameState) {
    state.deferred_retries.clear();
    let consumed = std::mem::take(&mut state.consumed_questions);
    let Some(cartridge) = state.cartridge.as_mut() else {
        return;
    };
    let consumed = consumed.min(cartridge.questions.len());
    if consumed == 0 {
        return;
    }
    let retired: Vec<_> = cartridge.questions.drain(..consumed).collect();
    let outstanding: HashSet<String> = cartridge
        .lessons
        .iter()
        .filter(|lesson| lesson.outstanding)
        .map(|lesson| question_identity(&lesson.question))
        .collect();
    let mut queued: HashSet<String> = cartridge
        .questions
        .iter()
        .map(|question| question_identity(&question.question))
        .collect();
    let reviews: Vec<_> = retired
        .into_iter()
        .filter(|question| {
            let identity = question_identity(&question.question);
            outstanding.contains(&identity) && queued.insert(identity)
        })
        .map(|mut question| {
            // Still the same sitting: the Codex that shows the answer is one
            // screen away, so a success here is relearning.
            question.review = Review::InSession;
            question
        })
        .collect();
    let requeued = reviews.len();
    cartridge.questions.splice(0..0, reviews);

    let mut ends = Vec::new();
    let mut levels = Vec::new();
    for (index, end) in state.batch_ends.iter().enumerate() {
        if *end > consumed {
            ends.push(end - consumed + requeued);
            levels.push(state.batch_levels.get(index).copied().unwrap_or(1));
        }
    }
    (state.batch_ends, state.batch_levels) = rebatch(&ends, &levels, &cartridge.questions);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_quiz_cartridge_requests_the_first_ai_batch() {
        let mut engine = GameEngine::new();
        let mut cartridge = quiz_cartridge();
        cartridge.questions.clear();
        issue(&mut engine, EngineCommand::Cartridge(Some(cartridge)));

        let effects = engine.take_effects();
        assert!(matches!(
            effects.as_slice(),
            [EngineEffect::RequestQuestions {
                cartridge_id,
                level: 1,
                count: 6,
                ..
            }] if cartridge_id == "/tmp/engine-test"
        ));
    }

    #[test]
    fn oracle_retries_a_failed_ai_batch_while_waiting() {
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
        assert_eq!(engine.screen(), Screen::Oracle);
        deliver(&mut engine, "/tmp/engine-test", Vec::new());

        for _ in 0..300 {
            engine.update();
        }

        assert_eq!(engine.screen(), Screen::Oracle);
        assert!(engine.take_effects().iter().any(|effect| matches!(
            effect,
            EngineEffect::RequestQuestions {
                cartridge_id,
                level: 1,
                count: 6,
                ..
            } if cartridge_id == "/tmp/engine-test"
        )));
    }

    #[test]
    fn failed_prefetch_waits_before_requesting_ai_again() {
        let mut engine = GameEngine::new();
        let mut cartridge = quiz_cartridge();
        cartridge.questions = vec![cartridge.questions[0].clone(); 4];
        issue(&mut engine, EngineCommand::Cartridge(Some(cartridge)));
        issue(&mut engine, EngineCommand::Power(true));
        finish_opening(&mut engine);
        let _ = engine.take_effects();
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
        // The four-question batch is short, so the prefetch tops it up at
        // its own level instead of jumping ahead to level 2.
        assert!(matches!(
            engine.take_effects().as_slice(),
            [EngineEffect::RequestQuestions { level: 1, .. }]
        ));

        deliver(&mut engine, "/tmp/engine-test", Vec::new());
        assert!(engine.take_effects().is_empty());

        for _ in 0..299 {
            engine.update();
        }
        assert!(engine.take_effects().is_empty());
        engine.update();
        assert!(matches!(
            engine.take_effects().as_slice(),
            [EngineEffect::RequestQuestions { level: 1, .. }]
        ));
    }

    #[test]
    fn next_batch_prefetch_starts_when_the_first_batch_becomes_playable() {
        let mut engine = GameEngine::new();
        let mut cartridge = quiz_cartridge();
        cartridge.questions = vec![cartridge.questions[0].clone(); 6];
        issue(&mut engine, EngineCommand::Cartridge(Some(cartridge)));
        issue(&mut engine, EngineCommand::Power(true));
        finish_opening(&mut engine);
        let _ = engine.take_effects();
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

        assert!(matches!(
            engine.take_effects().as_slice(),
            [EngineEffect::RequestQuestions {
                level: 2,
                count: 6,
                ..
            }]
        ));
    }

    #[test]
    fn a_delivery_tops_batches_up_by_new_questions_while_reviews_ride_along() {
        let mut cartridge = oracle_template_cartridge();
        cartridge.questions = (0..4).map(concept_question).collect();
        let mut state = GameState {
            cartridge: Some(cartridge),
            batch_ends: vec![4],
            batch_levels: vec![1],
            ..Default::default()
        };
        let mut review = concept_question(20);
        review.review = Review::InSession;
        let delivery = std::iter::once(review)
            .chain((4..9).map(concept_question))
            .collect();
        append_question_batch(&mut state, delivery, 2);
        // The open batch takes the review plus two new questions, so it
        // holds a full six new ones; the other three open the next batch.
        assert_eq!(state.batch_ends, [QUESTION_BATCH_SIZE + 1, 10]);
        assert_eq!(state.batch_levels, [1, 2]);
        assert!(state.batch_is_full(0));
        assert!(!state.batch_is_full(1));
    }

    #[test]
    fn a_parked_delivery_cannot_requeue_a_stem_the_journal_retired() {
        let answered = concept_question(0);
        let missed = concept_question(1);
        let fresh = concept_question(2);
        let mut cartridge = oracle_template_cartridge();
        // The run that answered both has ended, so the deck is drained.
        cartridge.questions.clear();
        record_lesson(&mut cartridge.lessons, &answered, true, answered.answer);
        record_lesson(
            &mut cartridge.lessons,
            &missed,
            false,
            (missed.answer + 1) % 4,
        );
        let mut state = GameState {
            cartridge: Some(cartridge),
            ..Default::default()
        };
        let deck = |state: &GameState| {
            state
                .cartridge
                .as_ref()
                .unwrap()
                .questions
                .iter()
                .map(|question| (question.question.clone(), question.review))
                .collect::<Vec<_>>()
        };
        // The delivery was generated while both were still unanswered, so it
        // carries them as new questions.
        append_question_batch(
            &mut state,
            vec![answered.clone(), missed.clone(), fresh.clone()],
            1,
        );
        assert_eq!(
            deck(&state),
            [
                (missed.question.clone(), Review::InSession),
                (fresh.question.clone(), Review::Fresh)
            ],
            "a retired stem is skipped and an outstanding one is a review"
        );

        // Once the miss is redeemed, a stale copy of it is retired too.
        let mut state = GameState {
            cartridge: state.cartridge.take().map(|mut cartridge| {
                cartridge.questions.clear();
                record_lesson(&mut cartridge.lessons, &missed, true, missed.answer);
                cartridge
            }),
            ..Default::default()
        };
        let mut stale_review = missed.clone();
        stale_review.review = Review::InSession;
        append_question_batch(&mut state, vec![stale_review], 1);
        assert!(deck(&state).is_empty());
    }

    #[test]
    fn a_first_request_that_fails_before_power_on_is_retried_before_the_oracle() {
        let mut engine = GameEngine::new();
        let mut cartridge = quiz_cartridge();
        cartridge.questions.clear();
        issue(&mut engine, EngineCommand::Cartridge(Some(cartridge)));
        assert_eq!(request_seqs(&engine.take_effects()).len(), 1);
        // The unverified provider answers at once with nothing.
        deliver(&mut engine, "/tmp/engine-test", Vec::new());
        issue(&mut engine, EngineCommand::Power(true));
        issue(&mut engine, EngineCommand::BootComplete);
        assert!(
            request_seqs(&engine.take_effects()).is_empty(),
            "the retry delay still holds"
        );
        assert_eq!(
            creation_oracle_status(engine_state(&engine), true),
            "VISION CLOUDY - RETRYING"
        );

        let mut requests = Vec::new();
        for _ in 0..300 {
            engine.update();
            requests.extend(request_seqs(&engine.take_effects()));
        }

        assert_ne!(engine.screen(), Screen::Oracle);
        assert_eq!(requests.len(), 1, "one catch-up request before the Oracle");
        assert_eq!(requests[0].0, 1);
        let state = engine_state(&engine);
        assert!(state.questions_loading);
        assert_eq!(creation_oracle_status(state, true), "ORACLE IS WRITING");
        assert_eq!(creation_oracle_status(state, false), "ORACLE IS WRITING...");
    }

    #[test]
    fn a_new_run_drops_answered_questions_and_brings_outstanding_misses_back_first() {
        let mut engine = batch_quiz_engine(QUESTION_BATCH_SIZE);
        let first_questions: Vec<_> = (0..3).map(concept_question).collect();
        assert_eq!(
            current_question(&engine).question,
            first_questions[0].question
        );
        let broken_ward_slot = {
            for _ in 0..2 {
                commit(&mut engine, false);
                finish_lesson(&mut engine);
            }
            let slot = answer_slot(&engine);
            commit(&mut engine, false);
            finish_lesson(&mut engine);
            slot
        };
        assert_eq!(engine.screen(), Screen::GameOver);

        press(&mut engine, Button::A);
        assert_eq!(engine.screen(), Screen::QuizMenu);
        press(&mut engine, Button::A);
        assert_eq!(engine.screen(), Screen::CharacterCreation);
        press(&mut engine, Button::Start);
        assert_eq!(engine.screen(), Screen::Oracle);
        for _ in 0..75 {
            engine.update();
        }
        assert_eq!(engine.screen(), Screen::Quiz);

        // The ward-breaking miss had no in-run retry, so it returns first as a
        // review in new slots; the two misses whose review copies were never
        // reached come back through those copies, not as duplicates.
        let state = engine_state(&engine);
        let deck: Vec<_> = state
            .cartridge
            .as_ref()
            .unwrap()
            .questions
            .iter()
            .map(|question| (question.question.clone(), question.review))
            .collect();
        let expected: Vec<_> = [
            (2, Review::InSession),
            (3, Review::Fresh),
            (0, Review::InSession),
            (1, Review::InSession),
            (4, Review::Fresh),
            (5, Review::Fresh),
        ]
        .into_iter()
        .map(|(index, review)| (concept_question(index).question, review))
        .collect();
        assert_eq!(deck, expected);
        assert_eq!(state.batch_ends, vec![QUESTION_BATCH_SIZE]);
        let run = state.quiz.as_ref().unwrap();
        assert_eq!((run.question, run.level, run.completed_batches), (0, 1, 0));
        assert_eq!(run.hearts, 3);
        assert_eq!(run.attempt, 1);
        assert_ne!(answer_slot(&engine), broken_ward_slot);
        assert_ne!(
            current_question(&engine).question,
            first_questions[0].question
        );
    }

    #[test]
    fn short_deliveries_merge_until_a_full_batch_is_survived() {
        let mut engine = waiting_oracle_engine();
        deliver(
            &mut engine,
            "/tmp/engine-test",
            (0..4).map(concept_question).collect(),
        );
        for _ in 0..75 {
            engine.update();
        }
        assert_eq!(engine.screen(), Screen::Quiz);
        assert_eq!(engine_state(&engine).batch_ends, vec![4]);
        // The open batch is topped up at its own level.
        assert_eq!(
            request_seqs(&engine.take_effects())
                .iter()
                .map(|(level, _)| *level)
                .collect::<Vec<_>>(),
            vec![1]
        );
        deliver(
            &mut engine,
            "/tmp/engine-test",
            (4..8).map(concept_question).collect(),
        );
        assert!(engine_state(&engine).pending_questions.is_some());

        for _ in 0..4 {
            commit(&mut engine, true);
            finish_lesson(&mut engine);
        }
        assert_eq!(
            engine.screen(),
            Screen::Oracle,
            "four answers are not a batch"
        );
        engine.update();
        {
            let state = engine_state(&engine);
            assert_eq!(state.batch_ends, vec![6, 8]);
            assert_eq!(state.batch_levels, vec![1, 1]);
            assert_eq!(state.quiz.as_ref().unwrap().level, 1);
        }
        for _ in 0..75 {
            engine.update();
        }
        assert_eq!(engine.screen(), Screen::Quiz);
        // The new questions follow one level-up (the full first batch), so
        // the prefetch asks for level 2, not a level already queued.
        assert_eq!(
            request_seqs(&engine.take_effects())
                .iter()
                .map(|(level, _)| *level)
                .collect::<Vec<_>>(),
            vec![2]
        );

        for _ in 0..2 {
            commit(&mut engine, true);
            finish_lesson(&mut engine);
        }
        assert_eq!(engine.screen(), Screen::LevelUp);
        let run = engine_state(&engine).quiz.as_ref().unwrap();
        assert_eq!((run.question, run.level, run.completed_batches), (6, 2, 1));
    }

    #[test]
    fn rebatching_splits_saved_batches_into_full_batches_with_their_levels() {
        let fresh = |count| (0..count).map(concept_question).collect::<Vec<_>>();
        assert_eq!(
            rebatch(&[1, 7], &[2, 3], &fresh(7)),
            (vec![6, 7], vec![3, 3])
        );
        assert_eq!(rebatch(&[], &[], &fresh(4)), (vec![4], vec![1]));
        assert_eq!(
            rebatch(&[6, 12], &[1, 2], &fresh(12)),
            (vec![6, 12], vec![1, 2])
        );
        assert_eq!(rebatch(&[3], &[1], &[]), (vec![], vec![]));
        // Review copies ride along in a batch without counting toward it.
        let mut deck = fresh(9);
        deck[0].review = Review::InSession;
        deck[3].review = Review::Spaced;
        assert_eq!(rebatch(&[9], &[1], &deck), (vec![8, 9], vec![1, 1]));
    }

    #[test]
    fn review_copies_do_not_fill_a_short_batch() {
        let mut engine = waiting_oracle_engine();
        deliver(
            &mut engine,
            "/tmp/engine-test",
            (0..4).map(concept_question).collect(),
        );
        for _ in 0..75 {
            engine.update();
        }
        assert_eq!(engine.screen(), Screen::Quiz);
        deliver(
            &mut engine,
            "/tmp/engine-test",
            (4..8).map(concept_question).collect(),
        );
        assert!(engine_state(&engine).pending_questions.is_some());

        // Two misses add two review copies to the short batch: six questions,
        // but only four of them new.
        for correct in [false, false, true, true, true, true] {
            commit(&mut engine, correct);
            finish_lesson(&mut engine);
        }
        assert_eq!(
            engine.screen(),
            Screen::Oracle,
            "four new questions and two reviews are not a batch"
        );
        engine.update();
        {
            let state = engine_state(&engine);
            let run = state.quiz.as_ref().unwrap();
            assert_eq!((run.question, run.level, run.completed_batches), (6, 1, 0));
            assert_eq!(
                state.batch_ends,
                vec![8, 10],
                "the held delivery tops the batch up to six new questions"
            );
        }
        for _ in 0..75 {
            engine.update();
        }
        assert_eq!(engine.screen(), Screen::Quiz);
        for _ in 0..2 {
            commit(&mut engine, true);
            finish_lesson(&mut engine);
        }
        assert_eq!(engine.screen(), Screen::LevelUp);
        let run = engine_state(&engine).quiz.as_ref().unwrap();
        assert_eq!((run.question, run.level, run.completed_batches), (8, 2, 1));
    }

    #[test]
    fn a_delivery_never_queues_a_question_already_in_the_deck() {
        let mut cartridge = quiz_cartridge();
        cartridge.questions = (0..2).map(concept_question).collect();
        let mut state = GameState {
            cartridge: Some(cartridge),
            batch_ends: vec![2],
            batch_levels: vec![1],
            ..Default::default()
        };
        let mut respelled = concept_question(0);
        respelled.question = respelled.question.to_ascii_lowercase();
        append_question_batch(
            &mut state,
            vec![
                concept_question(1),
                respelled,
                concept_question(2),
                concept_question(2),
            ],
            1,
        );
        assert_eq!(
            state
                .cartridge
                .as_ref()
                .unwrap()
                .questions
                .iter()
                .map(|question| question.question.clone())
                .collect::<Vec<_>>(),
            (0..3)
                .map(|index| concept_question(index).question)
                .collect::<Vec<_>>()
        );
        assert_eq!(state.batch_ends, vec![3]);
    }

    #[test]
    fn a_regenerated_missed_question_plays_as_a_review_and_redeems() {
        let mut engine = waiting_oracle_engine();
        // The loader marks a regenerated stem the player missed as a
        // same-launch review.
        let mut regenerated = concept_question(0);
        regenerated.review = Review::InSession;
        let identity = question_identity(&regenerated.question);
        deliver(&mut engine, "/tmp/engine-test", vec![regenerated]);
        for _ in 0..75 {
            engine.update();
        }
        assert_eq!(engine.screen(), Screen::Quiz);
        let run = engine_state(&engine).quiz.as_ref().unwrap();
        assert_eq!(run.attempt, 1);
        assert_eq!(
            run.order,
            learning::presentation_order(&identity, 1).to_vec(),
            "not the order the player missed it in"
        );

        let _ = engine.take_effects();
        commit(&mut engine, true);
        assert!(engine_state(&engine).quiz.as_ref().unwrap().redeemed);
        assert!(engine.take_effects().iter().any(|effect| matches!(
            effect,
            EngineEffect::RecordAnsweredQuestion { evidence, .. }
                if evidence.correct && evidence.review == Review::InSession
        )));
    }

    #[test]
    fn saved_batch_levels_drive_the_prefetch_while_the_run_starts_at_initiate() {
        let mut engine = GameEngine::new();
        let mut cartridge = quiz_cartridge();
        cartridge.questions = (0..QUESTION_BATCH_SIZE).map(concept_question).collect();
        cartridge.question_batch_ends = vec![QUESTION_BATCH_SIZE];
        cartridge.question_batch_levels = vec![3];
        issue(&mut engine, EngineCommand::Cartridge(Some(cartridge)));
        issue(&mut engine, EngineCommand::Power(true));
        finish_opening(&mut engine);
        let _ = engine.take_effects();
        for button in [Button::Start, Button::A, Button::Start] {
            press(&mut engine, button);
        }
        for _ in 0..75 {
            engine.update();
        }
        assert_eq!(engine.screen(), Screen::Quiz);
        assert_eq!(engine_state(&engine).quiz.as_ref().unwrap().level, 1);
        assert_eq!(
            request_seqs(&engine.take_effects())
                .iter()
                .map(|(level, _)| *level)
                .collect::<Vec<_>>(),
            vec![4],
            "a queued level-3 batch is not requested again"
        );
    }

    #[test]
    fn retiring_a_run_keeps_each_remaining_batch_at_its_own_level() {
        let mut state = GameState::default();
        let mut cartridge = quiz_cartridge();
        cartridge.questions = (0..18).map(concept_question).collect();
        let missed = concept_question(2);
        record_lesson(&mut cartridge.lessons, &missed, false, 1);
        state.cartridge = Some(cartridge);
        state.batch_ends = vec![6, 12, 18];
        state.batch_levels = vec![1, 2, 3];
        state.consumed_questions = QUESTION_BATCH_SIZE;

        retire_consumed_questions(&mut state);

        let questions = &state.cartridge.as_ref().unwrap().questions;
        assert_eq!(questions.len(), 13);
        assert_eq!(questions[0].review, Review::InSession);
        assert_eq!(questions[0].question, missed.question);
        assert_eq!(state.batch_ends, vec![7, 13]);
        assert_eq!(
            state.batch_levels,
            vec![2, 3],
            "the requeued review shifts the old boundaries with it"
        );
    }

    #[test]
    fn rebatching_reads_a_boundary_question_from_the_batch_it_opens() {
        let fresh = (0..12).map(concept_question).collect::<Vec<_>>();
        // Question 5 opens the old level-2 batch, so the new first batch
        // (ending on it) takes level 2.
        assert_eq!(
            rebatch(&[5, 12], &[1, 2], &fresh),
            (vec![6, 12], vec![2, 2])
        );
    }
}
