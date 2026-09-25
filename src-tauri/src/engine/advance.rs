use super::*;

pub(super) fn advance_game(mut state: ResMut<GameState>, mut effects: ResMut<Effects>) {
    state.screen_ticks = state.screen_ticks.saturating_add(1);
    if !matches!(state.screen, Screen::Off | Screen::Boot) {
        state.tick_machine();
    }
    if state.consumed_questions > 0 && !state.run_is_live() {
        retire_consumed_questions(&mut state);
    }
    if state.screen == Screen::Oracle {
        let moving_left = state.held.contains(&Button::Left);
        let moving_right = state.held.contains(&Button::Right);
        if moving_left != moving_right {
            let direction = if moving_left { -1 } else { 1 };
            state.oracle_hero_x = (state.oracle_hero_x + direction * ORACLE_HERO_SPEED)
                .clamp(ORACLE_HERO_MIN_X, ORACLE_HERO_MAX_X);
        }
        if state.screen_ticks % ORACLE_DROP_INTERVAL == 1 {
            let lanes = [112, 112, 54, 210, 24, 142, 82];
            let index = state.oracle_spawned as usize;
            state.oracle_drops.push(OracleDrop {
                x: lanes[index % lanes.len()],
                y: 30,
                kind: if index.is_multiple_of(2) {
                    OracleDropKind::Data
                } else {
                    OracleDropKind::Bug
                },
            });
            state.oracle_spawned = state.oracle_spawned.saturating_add(1);
        }
        let hero_x = state.oracle_hero_x;
        let mut data_hits = 0;
        let mut bug_hits = 0;
        state.oracle_drops.retain_mut(|drop| {
            drop.y += 1;
            let overlaps_hero = drop.x >= hero_x - 4 && drop.x <= hero_x + 28;
            if drop.y >= ORACLE_COLLISION_Y && overlaps_hero {
                match drop.kind {
                    OracleDropKind::Data => data_hits += 1,
                    OracleDropKind::Bug => bug_hits += 1,
                }
                false
            } else {
                drop.y < 128
            }
        });
        state.oracle_data = state.oracle_data.saturating_add(data_hits);
        state.oracle_bug_hits = state.oracle_bug_hits.saturating_add(bug_hits);
    }
    if matches!(state.screen, Screen::Oracle | Screen::LevelUp) {
        if let Some((cartridge_id, questions, level)) = state.pending_questions.take() {
            if state
                .cartridge
                .as_ref()
                .is_some_and(|cartridge| cartridge.id == cartridge_id)
            {
                append_question_batch(&mut state, questions, level);
            }
        }
    }
    match state.screen {
        Screen::Oracle if state.screen_ticks >= 75 && state.has_unanswered_question() => {
            state.signal(SceneSignal::QuestionsReady);
        }
        Screen::Quiz if !state.has_unanswered_question() => {
            state.signal(SceneSignal::NeedsQuestion);
        }
        Screen::Quiz => {
            let prefetch = state.quiz.as_ref().is_some_and(|run| {
                state.question_count().saturating_sub(run.question) <= QUESTION_BATCH_SIZE
            });
            if prefetch {
                let level = state.next_batch_level();
                request_question_batch(&mut state, &mut effects, level);
            }
            if let Some(run) = state.quiz.as_mut() {
                if let Some((_, hold)) = run.feedback.as_mut() {
                    *hold = hold.saturating_sub(1);
                }
                run.leave_armed = run.leave_armed.saturating_sub(1);
            }
        }
        Screen::LevelUp if state.screen_ticks >= 180 => {
            let signal = state.level_up_signal();
            state.signal(signal);
        }
        _ => {}
    }
    // Catch-up request: whenever the powered device shows a quiz screen and
    // the next question does not exist yet, keep a request in flight. This
    // recovers a first request that failed before the batteries were
    // verified, and serves the Oracle and level-up waits alike.
    if state.powered
        && !matches!(state.screen, Screen::Off | Screen::Boot)
        && !state.has_next_question()
    {
        let level = state.next_batch_level();
        request_question_batch(&mut state, &mut effects, level);
    }
    if state.screen == Screen::Quiz {
        present_current_question(&mut state);
    }
    state.question_retry_ticks = state.question_retry_ticks.saturating_sub(1);
    if state.questions_loading {
        state.question_request_ticks = state.question_request_ticks.saturating_add(1);
    }
    // Until the line is a recall, the next recall starts at the next tick.
    if state.screen != Screen::Oracle
        || !matches!(state.oracle_line(), Some(OracleLine::Recall { .. }))
    {
        state.oracle_recall_since = state.screen_ticks.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oracle_moves_the_hero_while_left_or_right_is_held() {
        let mut control = waiting_oracle_engine();
        let mut moved = waiting_oracle_engine();
        issue(
            &mut moved,
            EngineCommand::Input {
                button: Button::Right,
                pressed: true,
            },
        );
        control.update();
        for _ in 0..8 {
            moved.update();
            control.update();
        }

        assert!(
            moved.frame() != control.frame(),
            "Right did not move the hero"
        );
    }

    #[test]
    fn oracle_rains_collectible_data_and_bugs_while_waiting() {
        let mut engine = waiting_oracle_engine();
        for _ in 0..55 {
            engine.update();
        }

        let state = engine.app.world().resource::<GameState>();
        assert!(
            state
                .oracle_drops
                .iter()
                .any(|drop| drop.kind == OracleDropKind::Data),
            "no collectible data appeared"
        );
        assert!(
            state
                .oracle_drops
                .iter()
                .any(|drop| drop.kind == OracleDropKind::Bug),
            "no bug appeared"
        );
    }

    #[test]
    fn oracle_collects_data_on_contact_without_an_action_button() {
        let mut collected = waiting_oracle_engine();
        let mut dodged = waiting_oracle_engine();
        issue(
            &mut dodged,
            EngineCommand::Input {
                button: Button::Left,
                pressed: true,
            },
        );
        collected.update();
        for _ in 0..85 {
            collected.update();
            dodged.update();
        }

        let collected_counter = frame_region(collected.frame(), 0..60, 142..152);
        let dodged_counter = frame_region(dodged.frame(), 0..60, 142..152);
        assert!(
            collected_counter != dodged_counter,
            "contact with data did not update the data counter"
        );
    }

    #[test]
    fn oracle_up_and_down_do_not_change_datafall_gameplay() {
        for button in [Button::Up, Button::Down] {
            let mut control = waiting_oracle_engine();
            let mut pressed = waiting_oracle_engine();
            issue(
                &mut pressed,
                EngineCommand::Input {
                    button,
                    pressed: true,
                },
            );
            control.update();
            for _ in 0..60 {
                control.update();
                pressed.update();
            }

            assert!(
                pressed.frame() == control.frame(),
                "{button:?} still changes Oracle Datafall"
            );
        }
    }

    #[test]
    fn oracle_left_and_right_movement_dodges_bug_collisions() {
        let mut hit = waiting_oracle_engine();
        let mut dodged = waiting_oracle_engine();

        hit.update();
        issue(
            &mut dodged,
            EngineCommand::Input {
                button: Button::Left,
                pressed: true,
            },
        );
        for _ in 0..50 {
            hit.update();
            dodged.update();
        }
        issue(
            &mut dodged,
            EngineCommand::Input {
                button: Button::Left,
                pressed: false,
            },
        );
        hit.update();
        for _ in 0..55 {
            hit.update();
            dodged.update();
        }

        let hit_counter = frame_region(hit.frame(), 190..240, 142..152);
        let dodged_counter = frame_region(dodged.frame(), 190..240, 142..152);
        assert!(
            hit_counter != dodged_counter,
            "dodging did not prevent the hit"
        );
    }

    #[test]
    fn quiz_waits_for_ai_questions_before_entering_play() {
        let mut engine = GameEngine::new();
        let mut cartridge = quiz_cartridge();
        cartridge.questions.clear();
        issue(&mut engine, EngineCommand::Cartridge(Some(cartridge)));
        issue(&mut engine, EngineCommand::Power(true));
        finish_opening(&mut engine);
        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::Start,
                pressed: true,
            },
        );
        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::Start,
                pressed: false,
            },
        );
        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::A,
                pressed: true,
            },
        );
        assert_eq!(engine.screen(), Screen::CharacterCreation);
        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::A,
                pressed: false,
            },
        );
        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::Start,
                pressed: true,
            },
        );
        assert_eq!(engine.screen(), Screen::Oracle);

        for _ in 0..180 {
            engine.update();
        }
        assert_eq!(engine.screen(), Screen::Oracle);

        deliver(
            &mut engine,
            "/tmp/engine-test",
            vec![QuizQuestion {
                question: "WHAT SHOULD THE ENGINE OWN?".into(),
                choices: vec![
                    "GAMEPLAY STATE".into(),
                    "DEVICE STYLES".into(),
                    "WINDOW CHROME".into(),
                    "HOST POINTERS".into(),
                ],
                answer: 0,
                ..Default::default()
            }],
        );
        assert_eq!(engine.screen(), Screen::Quiz);
    }

    #[test]
    fn exhausted_deck_waits_for_a_new_question_instead_of_rendering_blank_quiz() {
        let mut engine = GameEngine::new();
        issue(
            &mut engine,
            EngineCommand::Cartridge(Some(quiz_cartridge())),
        );
        {
            let mut state = engine.app.world_mut().resource_mut::<GameState>();
            state.powered = true;
            state.machine = Some(SceneMachine::new(
                SceneMachineDefinition::compile(
                    "oracle",
                    vec![
                        SceneSpec {
                            id: "oracle".into(),
                            handler: SceneHandler::Oracle,
                            transitions: vec![SceneTransition {
                                signal: SceneSignal::QuestionsReady,
                                target: "quiz".into(),
                                after_ticks: None,
                            }],
                        },
                        SceneSpec {
                            id: "quiz".into(),
                            handler: SceneHandler::ConceptQuiz,
                            transitions: vec![SceneTransition {
                                signal: SceneSignal::NeedsQuestion,
                                target: "oracle".into(),
                                after_ticks: None,
                            }],
                        },
                    ],
                )
                .unwrap(),
            ));
            state.quiz = Some(QuizRun {
                question: 1,
                completed_batches: 1,
                score: 100,
                streak: 1,
                ..QuizRun::new()
            });
            // The live run's quiz ran out of questions.
            state.screen = Screen::Quiz;
            state.transition(Screen::Oracle);
        }

        engine.update();
        assert!(matches!(
            engine.take_effects().as_slice(),
            [EngineEffect::RequestQuestions { level: 1, .. }]
        ));
        for _ in 1..75 {
            engine.update();
        }
        assert_eq!(engine.screen(), Screen::Oracle);

        deliver(
            &mut engine,
            "/tmp/engine-test",
            vec![QuizQuestion {
                question: "WHAT ARRIVED NEXT?".into(),
                choices: vec!["A NEW QUESTION".into(), "NOTHING".into()],
                answer: 0,
                ..Default::default()
            }],
        );
        assert_eq!(engine.screen(), Screen::Quiz);
        let state = engine.app.world().resource::<GameState>();
        assert_eq!(
            state.cartridge.as_ref().unwrap().questions[1].question,
            "WHAT ARRIVED NEXT?"
        );
    }

    #[test]
    fn oracle_result_waits_for_a_safe_screen_boundary() {
        let mut engine = GameEngine::new();
        issue(
            &mut engine,
            EngineCommand::Cartridge(Some(quiz_cartridge())),
        );
        engine.app.world_mut().resource_mut::<GameState>().screen = Screen::Quiz;
        deliver(
            &mut engine,
            "/tmp/engine-test",
            vec![QuizQuestion {
                question: "NEW BATCH".into(),
                choices: vec!["A".into()],
                answer: 0,
                ..Default::default()
            }],
        );
        {
            let state = engine.app.world().resource::<GameState>();
            assert_eq!(
                state.cartridge.as_ref().unwrap().questions[0].question,
                "WHO OWNS THE GAME LOOP?"
            );
            assert!(state.pending_questions.is_some());
        }
        engine.app.world_mut().resource_mut::<GameState>().screen = Screen::Oracle;
        engine.update();
        let state = engine.app.world().resource::<GameState>();
        assert_eq!(state.cartridge.as_ref().unwrap().questions.len(), 2);
        assert_eq!(
            state.cartridge.as_ref().unwrap().questions[1].question,
            "NEW BATCH"
        );
    }

    #[test]
    fn a_delivery_held_through_the_quiz_lands_on_the_level_up() {
        let mut engine = batch_quiz_engine(QUESTION_BATCH_SIZE);
        let _ = engine.take_effects();
        engine.update();
        deliver(
            &mut engine,
            "/tmp/engine-test",
            (QUESTION_BATCH_SIZE..2 * QUESTION_BATCH_SIZE)
                .map(concept_question)
                .collect(),
        );
        assert!(engine_state(&engine).pending_questions.is_some());
        for _ in 0..QUESTION_BATCH_SIZE {
            commit(&mut engine, true);
            finish_lesson(&mut engine);
        }
        assert_eq!(engine.screen(), Screen::LevelUp);
        engine.update();
        let state = engine_state(&engine);
        assert!(state.pending_questions.is_none());
        assert_eq!(state.question_count(), 2 * QUESTION_BATCH_SIZE);

        for _ in 0..200 {
            if engine.screen() != Screen::LevelUp {
                break;
            }
            engine.update();
        }
        assert_eq!(
            engine.screen(),
            Screen::Quiz,
            "questions already delivered skip the Oracle wait"
        );
    }
}
