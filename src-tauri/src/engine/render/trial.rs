use super::*;

pub(in crate::engine) const TRIAL_CHOICE_TEXT_X: i32 = 39;
pub(in crate::engine) const TRIAL_WARD_X: i32 = 126;
pub(in crate::engine) const TRIAL_STREAK_X: i32 = 180;
pub(in crate::engine) const TRIAL_SCORE_RUNES_X: i32 = 194;
pub(in crate::engine) const TRIAL_SCORE_X: i32 = 215;

pub(in crate::engine) fn crossed_insight_stage(run: &QuizRun) -> Option<InsightStage> {
    if !matches!(run.feedback, Some((true, _))) {
        return None;
    }
    let current = InsightStage::from_score(run.score);
    let previous_score = run.score.saturating_sub(score_award_for_streak(run.streak));
    let previous = InsightStage::from_score(previous_score);
    (current > previous).then_some(current)
}

/// The lesson-card banner. A woken lens rune outranks everything: it is the
/// run's pedagogical reward, while INSIGHT names the score marks.
pub(in crate::engine) fn quiz_feedback_banner(run: &QuizRun) -> String {
    match run.feedback {
        Some((true, _)) => {
            if let Some((concept, stage)) = run.lens_woke {
                format!("{} RUNE {}", concept.label(), mastery_numeral(stage))
            } else if let Some(stage) = crossed_insight_stage(run) {
                format!("INSIGHT {} RISES", stage.label())
            } else if run.redeemed {
                "REDEEMED".into()
            } else if streak_multiplier(run.streak) > 1 {
                format!("FLOW X{}", streak_multiplier(run.streak))
            } else {
                "CLEAR SIGHT".into()
            }
        }
        Some((false, _)) => match run.hearts {
            0 => "WARD BROKEN".into(),
            1 => "WARD FRACTURES".into(),
            _ => "WARD STRAINED".into(),
        },
        None if run.leave_armed > 0 => "B AGAIN:LEAVE".into(),
        None => "A:ANSWER B:LEAVE".into(),
    }
}

pub(in crate::engine) fn quiz_feedback_color(run: &QuizRun) -> Color {
    if let (Some((true, _)), Some((_, stage))) = (run.feedback, run.lens_woke) {
        return mastery_rune_color(stage);
    }
    match run.feedback {
        Some((true, _)) if crossed_insight_stage(run).is_some() => AMBER,
        Some((true, _)) if run.redeemed => GREEN,
        Some((true, _)) => CYAN,
        Some((false, _)) => ward_color(run.hearts),
        None if run.leave_armed > 0 => AMBER,
        None => MIST,
    }
}

/// Left edge of the trial header's banner row: beside the tier label, pulled
/// left just enough that the longest lens banner stays inside the header.
pub(in crate::engine) const TRIAL_BANNER_X: i32 = 126;

pub(in crate::engine) fn trial_banner_x(banner: &str) -> i32 {
    TRIAL_BANNER_X.min(236 - text_width(banner, 1))
}

/// Whether the question on screen is a returning review copy.
pub(in crate::engine) fn current_question_is_review(state: &GameState) -> bool {
    state
        .quiz
        .as_ref()
        .zip(state.cartridge.as_ref())
        .and_then(|(run, cartridge)| cartridge.questions.get(run.question))
        .is_some_and(|question| question.review.is_review())
}

/// The header counter: `trial` (or `retry` in amber while a review copy is on
/// screen) with batch progress `P/N`, or the run's question number when no
/// batch is known.
pub(in crate::engine) fn question_counter(
    state: &GameState,
    (trial, retry): (&str, &str),
    trial_color: Color,
) -> (String, Color) {
    let (label, color) = if current_question_is_review(state) {
        (retry, AMBER)
    } else {
        (trial, trial_color)
    };
    let text = match state.batch_progress() {
        Some((place, length)) => format!("{label}{place}/{length}"),
        None => {
            let question = state.quiz.as_ref().map_or(0, |run| run.question);
            format!("{label}{:02}", (question + 1).min(99))
        }
    };
    (text, color)
}

/// Where the trial lesson footer centers `note`: in the free span between the
/// lens runes (or the column start without a lens) and `A:CONTINUE`.
pub(in crate::engine) fn trial_retry_note_x(
    column_x: i32,
    column_end: i32,
    concept: Option<Concept>,
    note: &str,
) -> i32 {
    let left = concept.map_or(column_x, |concept| {
        column_x + text_width(concept.label(), 1) + 4 + RUNE_METER_WIDTH
    });
    let right = column_end - text_width(LESSON_CONTINUE, 1);
    left + (right - left - text_width(note, 1)) / 2
}

/// Where the legacy footer puts `note`: centered in the free span between
/// `banner` and `A:CONTINUE`, when that leaves at least a glyph cell (6px) on
/// each side. Both are amber, so a tight gap would read as one phrase
/// (`WARD STRAINED BACK IN 3`).
pub(in crate::engine) fn quiz_retry_note_x(banner: &str, note: &str) -> Option<i32> {
    let left = 5 + text_width(banner, 1);
    let right = 235 - text_width(LESSON_CONTINUE, 1);
    let spare = right - left - text_width(note, 1);
    (spare >= 12).then_some(left + spare / 2)
}

pub(in crate::engine) fn render_quiz(frame: &mut Framebuffer, state: &GameState) {
    if state.uses_visual_template(VisualTemplate::Trial) {
        render_oracle_trial(frame, state);
        return;
    }
    frame.clear(NAVY);
    let Some(run) = state.quiz.as_ref() else {
        return;
    };
    frame.rect(0, 0, WIDTH as i32, 16, INK);
    let (counter, counter_color) = question_counter(state, ("Q", "R"), SKY);
    frame.text(5, 4, &counter, counter_color, 1);
    draw_hero(frame, 47, 1, 1, state);
    draw_oracle_ward_meter(frame, 84, 4, run.hearts);
    frame.text(
        176,
        4,
        &format!("X{}", streak_multiplier(run.streak)),
        CYAN,
        1,
    );
    let insight = InsightStage::from_score(run.score);
    draw_oracle_rune_meter(frame, 192, 4, insight.index(), insight.color());
    frame.text(
        215,
        4,
        &format!("{:04}", run.score.min(9999)),
        insight.color(),
        1,
    );
    let Some(cart) = state.cartridge.as_ref() else {
        return;
    };
    let Some(question) = cart.questions.get(run.question) else {
        return;
    };
    frame.rect(5, 23, 230, 42, INK);
    frame.outline(5, 23, 230, 42, SKY);
    frame.wrapped_text(
        11,
        30,
        &question.question,
        PARCH,
        QUIZ_QUESTION_COLUMNS,
        QUIZ_QUESTION_ROWS,
    );
    if let Some(lesson) = current_lesson(state, quiz_lesson_copy_box()) {
        let panel = QUIZ_LESSON_BOX;
        frame.rect(panel.x, panel.y, panel.width, panel.height, VOID);
        frame.outline(panel.x, panel.y, panel.width, panel.height, SKY);
        draw_lesson_lines(frame, &lesson);
        // The lesson footer sits on an INK strip, like the header: a woken
        // rune III banner is MAGENTA, which NAVY cannot carry at 4.5:1.
        frame.rect(
            0,
            QUIZ_LESSON_FOOTER_STRIP_Y,
            WIDTH as i32,
            HEIGHT as i32 - QUIZ_LESSON_FOOTER_STRIP_Y,
            INK,
        );
        let banner = quiz_feedback_banner(run);
        frame.text(5, 151, &banner, quiz_feedback_color(run), 1);
        if let Some(note) = run.retry_note.map(RetryNote::label) {
            if let Some(x) = quiz_retry_note_x(&banner, &note) {
                frame.text(x, 151, &note, AMBER, 1);
            }
        }
        if lesson_is_live(run) {
            frame.text(
                235 - text_width(LESSON_CONTINUE, 1),
                151,
                LESSON_CONTINUE,
                CYAN,
                1,
            );
        }
        return;
    }
    let order = run.display_order(question.choices.len());
    for (slot, source) in order.into_iter().take(4).enumerate() {
        let y = 73 + slot as i32 * 20;
        if run.selected == slot {
            frame.rect(5, y - 3, 230, 14, ROYAL);
            frame.text(9, y, ">", GOLD, 1);
        }
        frame.text(
            21,
            y,
            &truncate(&question.choices[source], QUIZ_CHOICE_CHARS),
            PARCH,
            1,
        );
    }
    frame.text(5, 151, "A:ANSWER", MIST, 1);
    if run.leave_armed > 0 {
        let prompt = quiz_feedback_banner(run);
        frame.text(
            235 - text_width(&prompt, 1),
            151,
            &prompt,
            quiz_feedback_color(run),
            1,
        );
    } else {
        frame.text(199, 151, "B:BACK", MIST, 1);
    }
}

pub(in crate::engine) fn render_oracle_trial(frame: &mut Framebuffer, state: &GameState) {
    let Some(run) = state.quiz.as_ref() else {
        return;
    };
    let tier = state.visual_tier();
    match tier {
        PresentationTier::Initiate => frame.blit_rgb_graded(ORACLE_TRIAL, 226, 244, 188),
        PresentationTier::Adept => frame.blit_rgb_graded(ORACLE_TRIAL, 240, 250, 230),
        PresentationTier::OracleBound => frame.blit_rgb(ORACLE_TRIAL),
    };
    frame.blit_rgba(
        ORACLE_PORTRAITS[state.hero_style],
        HERO_PORTRAIT_SIZE,
        HERO_PORTRAIT_SIZE,
        8,
        5,
        1,
    );
    frame.rect(37, 2, 203, 28, VOID);
    frame.outline(37, 2, 203, 28, CYAN_DIM);
    let (counter, counter_color) = question_counter(state, ("TRIAL ", "RETRY "), CYAN);
    frame.text(44, 7, &counter, counter_color, 1);
    draw_oracle_ward_meter(frame, TRIAL_WARD_X, 7, run.hearts);
    frame.text(
        TRIAL_STREAK_X,
        7,
        &format!("X{}", streak_multiplier(run.streak)),
        CYAN,
        1,
    );
    let insight = InsightStage::from_score(run.score);
    draw_oracle_rune_meter(
        frame,
        TRIAL_SCORE_RUNES_X,
        7,
        insight.index(),
        insight.color(),
    );
    frame.text(
        TRIAL_SCORE_X,
        7,
        &format!("{:04}", run.score.min(9999)),
        insight.color(),
        1,
    );
    frame.text(
        44,
        20,
        tier.label(),
        if tier == PresentationTier::Initiate {
            CYAN_DIM
        } else {
            AMBER
        },
        1,
    );
    let banner = quiz_feedback_banner(run);
    frame.text(
        trial_banner_x(&banner),
        20,
        &banner,
        quiz_feedback_color(run),
        1,
    );

    let Some(cart) = state.cartridge.as_ref() else {
        return;
    };
    let Some(question) = cart.questions.get(run.question) else {
        return;
    };
    frame.wrapped_text(
        28,
        35,
        &question.question,
        PARCH,
        QUIZ_QUESTION_COLUMNS,
        QUIZ_QUESTION_ROWS,
    );

    if let Some(lesson) = current_lesson(state, trial_lesson_copy_box()) {
        let panel = TRIAL_LESSON_BOX;
        frame.rect(panel.x, panel.y, panel.width, panel.height, VOID);
        frame.outline(panel.x, panel.y, panel.width, panel.height, CYAN_DIM);
        draw_lesson_lines(frame, &lesson);
        let column_x = lesson.first().map_or(panel.x + 3, |line| line.x);
        let column_end = column_x + text_width(&"W".repeat(RATIONALE_COLUMNS), 1);
        let footer_y = trial_lesson_footer_y();
        frame.rect(column_x, footer_y - 3, column_end - column_x, 1, INDIGO);
        if let Some(concept) = question.concept {
            let stage = state.mastery_stage(concept);
            let mut styles = mastery_meter_styles(stage, state.mastery_cracks(concept));
            // The rune this answer woke blinks through the input hold, then
            // settles lit; reduced motion keeps it lit throughout. Cracked
            // runes beside it hold still.
            let waking =
                run.lens_woke.is_some_and(|(woke, _)| woke == concept) && !lesson_is_live(run);
            if waking && !state.blink_lit(8) {
                if let Some(rune) = stage.checked_sub(1).and_then(|rune| styles.get_mut(rune)) {
                    *rune = RuneStyle::Unlit;
                }
            }
            frame.text(column_x, footer_y, concept.label(), CYAN_DIM, 1);
            draw_rune_row(
                frame,
                column_x + text_width(concept.label(), 1) + 4,
                footer_y,
                styles,
                CYAN,
            );
        }
        if let Some(note) = run.retry_note.map(RetryNote::label) {
            frame.text(
                trial_retry_note_x(column_x, column_end, question.concept, &note),
                footer_y,
                &note,
                AMBER,
                1,
            );
        }
        if lesson_is_live(run) {
            frame.text(
                column_end - text_width(LESSON_CONTINUE, 1),
                footer_y,
                LESSON_CONTINUE,
                CYAN,
                1,
            );
        }
        return;
    }
    let order = run.display_order(question.choices.len());
    for (slot, source) in order.into_iter().take(4).enumerate() {
        let y = 69 + slot as i32 * 22;
        let focused = run.selected == slot;
        if focused {
            draw_asset_focus(frame, 23, y, 204, 20);
        }
        frame.text(
            TRIAL_CHOICE_TEXT_X,
            y + 7,
            &truncate(&question.choices[source], QUIZ_CHOICE_CHARS),
            if focused { PARCH } else { MIST },
            1,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiz_review_names_rune_flow_and_ward_threshold_changes() {
        let mut run = QuizRun {
            score: 400,
            streak: 3,
            feedback: Some((true, QUIZ_FEEDBACK_TICKS)),
            ..QuizRun::new()
        };
        assert_eq!(quiz_feedback_banner(&run), "INSIGHT I RISES");

        // A woken lens rune outranks the Insight crossing.
        run.lens_woke = Some((Concept::Responsibility, 1));
        assert_eq!(quiz_feedback_banner(&run), "ROLES RUNE I");
        assert_eq!(quiz_feedback_color(&run).1, mastery_rune_color(1).1);
        run.lens_woke = Some((Concept::Invariant, 3));
        assert_eq!(quiz_feedback_banner(&run), "INVARIANTS RUNE III");
        assert_eq!(quiz_feedback_color(&run).1, MAGENTA.1);
        run.lens_woke = None;

        run.score = 600;
        run.streak = 4;
        assert_eq!(quiz_feedback_banner(&run), "FLOW X2");

        run.feedback = Some((false, QUIZ_FEEDBACK_TICKS));
        run.streak = 0;
        run.hearts = 2;
        assert_eq!(quiz_feedback_banner(&run), "WARD STRAINED");
        run.hearts = 1;
        assert_eq!(quiz_feedback_banner(&run), "WARD FRACTURES");
        run.hearts = 0;
        assert_eq!(quiz_feedback_banner(&run), "WARD BROKEN");
    }

    #[test]
    fn rune_crossings_take_precedence_over_redemption() {
        let mut run = QuizRun {
            score: 300,
            streak: 1,
            redeemed: true,
            feedback: Some((true, QUIZ_FEEDBACK_TICKS)),
            ..QuizRun::new()
        };
        assert_eq!(quiz_feedback_banner(&run), "INSIGHT I RISES");
        run.score = 400;
        assert_eq!(quiz_feedback_banner(&run), "REDEEMED");
        assert_eq!(quiz_feedback_color(&run).1, GREEN.1);
        run.redeemed = false;
        // `REVIEW` only ever names retries; a plain success is CLEAR SIGHT.
        assert_eq!(quiz_feedback_banner(&run), "CLEAR SIGHT");
    }

    #[test]
    fn the_header_counts_the_batch_and_labels_the_returning_retry() {
        let mut engine = batch_quiz_engine(QUESTION_BATCH_SIZE);
        assert_eq!(engine_state(&engine).batch_progress(), Some((1, 6)));
        assert_eq!(trial_counter(&engine), ("TRIAL 1/6".into(), channels(CYAN)));

        commit(&mut engine, false);
        let note = retry_note(&engine).unwrap();
        assert_eq!(note, RetryNote::In(RETRY_GAP));
        assert_eq!(note.label(), "BACK IN 3");
        {
            // The note tells the real insertion distance.
            let state = engine_state(&engine);
            let run = state.quiz.as_ref().unwrap();
            let questions = &state.cartridge.as_ref().unwrap().questions;
            let returns = (run.question + 1..questions.len())
                .find(|index| questions[*index].review.is_review())
                .unwrap();
            assert_eq!(RetryNote::In(returns - run.question - 1), note);
        }
        assert_eq!(
            trial_counter(&engine).0,
            "TRIAL 1/7",
            "the miss grows its batch"
        );
        let mut missed = Framebuffer::default();
        render_oracle_trial(&mut missed, engine_state(&engine));
        maybe_write_preview("07e-trial-miss-returns", &missed.pixels);
        finish_lesson(&mut engine);
        assert_eq!(retry_note(&engine), None);
        assert_eq!(trial_counter(&engine).0, "TRIAL 2/7");

        for place in 2..=RETRY_GAP + 1 {
            assert_eq!(trial_counter(&engine).0, format!("TRIAL {place}/7"));
            commit(&mut engine, true);
            assert_eq!(retry_note(&engine), None, "successes carry no note");
            finish_lesson(&mut engine);
        }
        assert_eq!(current_question(&engine).review, Review::InSession);
        assert_eq!(
            trial_counter(&engine),
            ("RETRY 5/7".into(), channels(AMBER))
        );
        assert_eq!(
            question_counter(engine_state(&engine), ("Q", "R"), SKY).0,
            "R5/7"
        );
        let mut retry = Framebuffer::default();
        render_oracle_trial(&mut retry, engine_state(&engine));
        maybe_write_preview("07f-trial-retry", &retry.pixels);
        let mut legacy_retry = Framebuffer::default();
        render_quiz(&mut legacy_retry, engine_state(&engine));
        maybe_write_preview("quiz-legacy-retry", &legacy_retry.pixels);
        commit(&mut engine, true);
        assert_eq!(
            trial_counter(&engine).0,
            "RETRY 5/7",
            "the label holds through the retry's own lesson card"
        );
        finish_lesson(&mut engine);
        assert_eq!(current_question(&engine).review, Review::Fresh);
        assert_eq!(trial_counter(&engine), ("TRIAL 6/7".into(), channels(CYAN)));

        // The next batch counts from one again.
        let next_batch = GameState {
            batch_ends: vec![7, 13],
            quiz: Some(QuizRun {
                completed_batches: 1,
                question: 7,
                ..QuizRun::new()
            }),
            ..Default::default()
        };
        assert_eq!(next_batch.batch_progress(), Some((1, 6)));

        // An open short batch (two carried questions awaiting their top-up,
        // one miss already inserted) counts toward the full batch of new
        // questions plus that review copy, not 3/3 and not 3/6.
        let mut cartridge = oracle_template_cartridge();
        let mut retry = concept_question(0);
        retry.review = Review::InSession;
        cartridge.questions = vec![concept_question(0), concept_question(1), retry];
        let mut open_batch = GameState {
            cartridge: Some(cartridge),
            batch_ends: vec![3],
            quiz: Some(QuizRun {
                question: 2,
                ..QuizRun::new()
            }),
            ..Default::default()
        };
        assert_eq!(
            open_batch.batch_progress(),
            Some((3, QUESTION_BATCH_SIZE + 1))
        );
        // The top-up then fills the batch to exactly that length, so the
        // counter's end never jumps without a miss.
        append_question_batch(&mut open_batch, (2..6).map(concept_question).collect(), 1);
        assert_eq!(open_batch.batch_ends, [QUESTION_BATCH_SIZE + 1]);
        assert_eq!(
            open_batch.batch_progress(),
            Some((3, QUESTION_BATCH_SIZE + 1))
        );
    }

    #[test]
    fn the_legacy_lesson_footer_carries_a_woken_rune_iii_banner_readably() {
        let mut engine = batch_quiz_engine(QUESTION_BATCH_SIZE);
        commit(&mut engine, true);
        state_mut(&mut engine).quiz.as_mut().unwrap().lens_woke = Some((Concept::Invariant, 3));
        let state = engine_state(&engine);
        let run = state.quiz.as_ref().unwrap();
        let banner = quiz_feedback_banner(run);
        assert_eq!(banner, "INVARIANTS RUNE III");
        assert_eq!(channels(quiz_feedback_color(run)), channels(MAGENTA));
        let mut frame = Framebuffer::default();
        render_quiz(&mut frame, state);
        maybe_write_preview("quiz-legacy-rune-iii", &frame.pixels);
        let bounds = text_bounds(5, 151, &banner, 1);
        let x = bounds.x as usize..(bounds.x + bounds.width) as usize;
        let y = bounds.y as usize..(bounds.y + bounds.height) as usize;
        let banner_pixels = color_pixels_in_region(&frame.pixels, MAGENTA, x.clone(), y.clone());
        let ground_pixels = color_pixels_in_region(&frame.pixels, INK, x.clone(), y.clone());
        assert!(banner_pixels > 0);
        assert_eq!(
            banner_pixels + ground_pixels,
            x.len() * y.len(),
            "the banner is drawn on the footer's INK strip"
        );
        for stage in 1..=3 {
            let ratio = contrast_ratio(mastery_rune_color(stage), INK);
            assert!(ratio >= 4.5, "rune {stage} banner contrast {ratio:.2}:1");
        }
    }

    #[test]
    fn waking_a_lens_rune_banners_blinks_and_sounds_above_insight() {
        use audio::Cue;
        let mut engine = batch_quiz_engine(QUESTION_BATCH_SIZE);
        let banner =
            |engine: &GameEngine| quiz_feedback_banner(engine_state(engine).quiz.as_ref().unwrap());

        // 0 -> 1 evidence wakes rune I.
        focus_choice(&mut engine, true);
        assert_eq!(tap_cues(&mut engine, Button::A)[0], Some(Cue::LensWake(1)));
        assert_eq!(
            engine_state(&engine).quiz.as_ref().unwrap().lens_woke,
            Some((Concept::Responsibility, 1))
        );
        assert_eq!(banner(&engine), "ROLES RUNE I");

        // The newest footer rune blinks through the hold, then settles lit.
        let rune_x = (TRIAL_LESSON_BOX.x
            + 1
            + (TRIAL_LESSON_BOX.width - 2 - text_width(&"W".repeat(RATIONALE_COLUMNS), 1)) / 2
            + text_width(Concept::Responsibility.label(), 1)
            + 4) as usize;
        let footer_y = trial_lesson_footer_y() as usize;
        let rune_centers = |engine: &mut GameEngine| {
            let screen_ticks = engine_state(engine).screen_ticks;
            let samples = (0..16u64)
                .map(|tick| {
                    state_mut(engine).screen_ticks = tick;
                    let mut frame = Framebuffer::default();
                    render_oracle_trial(&mut frame, engine_state(engine));
                    frame_region(
                        &frame.pixels,
                        rune_x + 2..rune_x + 3,
                        footer_y + 3..footer_y + 4,
                    )
                })
                .collect::<HashSet<_>>();
            state_mut(engine).screen_ticks = screen_ticks;
            samples
        };
        assert_eq!(
            rune_centers(&mut engine).len(),
            2,
            "the woken rune blinks during the hold"
        );
        state_mut(&mut engine).reduced_motion = true;
        let still = rune_centers(&mut engine);
        assert_eq!(still.len(), 1, "reduced motion keeps the rune lit");
        state_mut(&mut engine).reduced_motion = false;
        for _ in 0..QUIZ_FEEDBACK_TICKS {
            engine.update();
        }
        assert_eq!(
            rune_centers(&mut engine),
            still,
            "the rune settles lit once the card is live"
        );
        press(&mut engine, Button::A);

        // 1 -> 2 evidence crosses nothing.
        commit(&mut engine, true);
        assert_eq!(engine_state(&engine).quiz.as_ref().unwrap().lens_woke, None);
        assert_eq!(banner(&engine), "CLEAR SIGHT");
        finish_lesson(&mut engine);

        // 2 -> 3 evidence wakes rune II on the same commit that crosses
        // Insight I and flow x2: the lens rune takes the banner and the cue.
        focus_choice(&mut engine, true);
        assert_eq!(tap_cues(&mut engine, Button::A)[0], Some(Cue::LensWake(2)));
        let run = engine_state(&engine).quiz.as_ref().unwrap();
        assert!(crossed_insight_stage(run).is_some());
        assert_eq!(banner(&engine), "ROLES RUNE II");
        state_mut(&mut engine).reduced_motion = true;
        let mut woke = Framebuffer::default();
        render_oracle_trial(&mut woke, engine_state(&engine));
        maybe_write_preview("07g-trial-lens-rune", &woke.pixels);
        state_mut(&mut engine).reduced_motion = false;
        finish_lesson(&mut engine);
        assert_eq!(engine_state(&engine).quiz.as_ref().unwrap().lens_woke, None);
    }

    #[test]
    fn lesson_cards_render_the_misconception_and_the_answer() {
        let answer_line = |state: &GameState| {
            current_lesson(state, trial_lesson_copy_box())
                .unwrap()
                .into_iter()
                .find(|line| line.tone == LessonTone::Answer)
                .unwrap()
        };

        let missed = lesson_state(lesson_question(), false, true);
        let mut missed_frame = Framebuffer::default();
        render_oracle_trial(&mut missed_frame, &missed);
        maybe_write_preview("oracle-lesson-missed", &missed_frame.pixels);
        let missed_lines = current_lesson(&missed, trial_lesson_copy_box()).unwrap();
        assert_eq!(missed_lines[0].text, "- TO DUPLICATE RUNTIME STATE");
        assert_eq!(missed_lines.len(), 8);
        let pick = &missed_lines[0];
        assert!(
            color_pixels_in_region(
                &missed_frame.pixels,
                RED,
                pick.x as usize..(pick.x + text_width(&pick.text, 1)) as usize,
                pick.y as usize..pick.y as usize + 7,
            ) > 0,
            "the misconception is drawn in red"
        );
        let answer = answer_line(&missed);
        assert_eq!(answer.text, "+ TO KEEP RESPONSIBILITIES CLEAR");
        assert!(
            color_pixels_in_region(
                &missed_frame.pixels,
                GREEN,
                answer.x as usize..(answer.x + text_width(&answer.text, 1)) as usize,
                answer.y as usize..answer.y as usize + 7,
            ) > 0
        );

        let correct = lesson_state(lesson_question(), true, true);
        let mut correct_frame = Framebuffer::default();
        render_oracle_trial(&mut correct_frame, &correct);
        maybe_write_preview("oracle-lesson-correct", &correct_frame.pixels);
        let panel = TRIAL_LESSON_BOX;
        let panel_x = panel.x as usize..(panel.x + panel.width) as usize;
        let panel_y = panel.y as usize..(panel.y + panel.height) as usize;
        assert_eq!(
            color_pixels_in_region(&correct_frame.pixels, RED, panel_x.clone(), panel_y.clone()),
            0,
            "a correct lesson shows no misconception"
        );
        assert_eq!(answer_line(&correct).tone, LessonTone::Answer);

        let held = lesson_state(lesson_question(), true, false);
        let mut held_frame = Framebuffer::default();
        render_oracle_trial(&mut held_frame, &held);
        let footer_y = trial_lesson_footer_y() as usize;
        assert_ne!(
            frame_region(&held_frame.pixels, panel_x.clone(), footer_y..footer_y + 7),
            frame_region(
                &correct_frame.pixels,
                panel_x.clone(),
                footer_y..footer_y + 7
            ),
            "A:CONTINUE appears only once input is live"
        );

        let mut legacy = lesson_state(
            QuizQuestion {
                rationales: Vec::new(),
                concept: None,
                ..lesson_question()
            },
            false,
            true,
        );
        legacy.cartridge.as_mut().unwrap().codequest = None;
        let legacy_lines = current_lesson(&legacy, quiz_lesson_copy_box()).unwrap();
        assert_eq!(
            legacy_lines
                .iter()
                .map(|line| line.text.as_str())
                .collect::<Vec<_>>(),
            [
                "- TO DUPLICATE RUNTIME STATE",
                "+ TO KEEP RESPONSIBILITIES CLEAR"
            ],
            "legacy questions show only the verdict and answer"
        );
        let mut legacy_frame = Framebuffer::default();
        render_quiz(&mut legacy_frame, &legacy);
        maybe_write_preview("quiz-lesson-legacy", &legacy_frame.pixels);

        let mut untemplated = lesson_state(lesson_question(), false, true);
        untemplated.cartridge.as_mut().unwrap().codequest = None;
        let mut untemplated_frame = Framebuffer::default();
        render_quiz(&mut untemplated_frame, &untemplated);
        maybe_write_preview("quiz-lesson-rationales", &untemplated_frame.pixels);
        assert!(
            color_pixels_in_region(
                &untemplated_frame.pixels,
                RED,
                QUIZ_LESSON_BOX.x as usize..(QUIZ_LESSON_BOX.x + QUIZ_LESSON_BOX.width) as usize,
                QUIZ_LESSON_BOX.y as usize..(QUIZ_LESSON_BOX.y + QUIZ_LESSON_BOX.height) as usize,
            ) > 0,
            "the untemplated quiz shows the misconception too"
        );

        let worst = lesson_state(worst_case_lesson_question(), false, true);
        let mut worst_frame = Framebuffer::default();
        render_oracle_trial(&mut worst_frame, &worst);
        maybe_write_preview("oracle-lesson-worst-case", &worst_frame.pixels);

        // The widest lens banner beside the widest tier label.
        let mut woke = lesson_state(worst_case_lesson_question(), true, true);
        let run = woke.quiz.as_mut().unwrap();
        run.level = 4;
        run.lens_woke = Some((Concept::Invariant, 3));
        woke.cartridge.as_mut().unwrap().mastery.insert(
            Concept::Invariant,
            learning::LensRecord {
                first_try: 5,
                ..Default::default()
            },
        );
        let mut woke_frame = Framebuffer::default();
        render_oracle_trial(&mut woke_frame, &woke);
        maybe_write_preview("oracle-lesson-lens-worst-case", &woke_frame.pixels);
        let banner_x = trial_banner_x("INVARIANTS RUNE III") as usize;
        assert!(
            color_pixels_in_region(
                &woke_frame.pixels,
                MAGENTA,
                banner_x..banner_x + text_width("INVARIANTS RUNE III", 1) as usize,
                20..27,
            ) > 0,
            "the rune III banner is drawn in the stage's magenta"
        );
    }
}
