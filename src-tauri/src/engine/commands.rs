use super::*;

// Commands are short-lived queue entries and cartridge inserts are rare, so
// the large cartridge variant is not worth boxing.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
pub(super) enum EngineCommand {
    Power(bool),
    ReducedMotion(bool),
    AiProvider(Option<String>),
    BootComplete,
    Cartridge(Option<CartridgeSpec>),
    Questions {
        cartridge_id: String,
        /// The generated batch, or why generation failed.
        result: Result<Vec<QuizQuestion>, String>,
        /// The `RequestQuestions` sequence number this reply answers.
        seq: u64,
    },
    Input {
        button: Button,
        pressed: bool,
    },
    QuestOutput {
        line: String,
        stderr: bool,
        /// The `RunQuest` generation this output belongs to; output from an
        /// earlier quest never lands in a later quest's log.
        quest: u64,
    },
    QuestDone {
        success: bool,
        quest: u64,
    },
}

#[derive(Clone, Debug)]
pub(super) enum EngineEffect {
    RunQuest {
        command: String,
        /// Echoed by the quest's output and completion.
        quest: u64,
    },
    AbortQuest,
    RequestQuestions {
        cartridge_id: String,
        level: u32,
        count: usize,
        /// Echoed by the reply so a superseded request cannot land.
        seq: u64,
    },
    RecordAnsweredQuestion {
        cartridge_id: String,
        evidence: AnswerEvidence,
    },
    /// The player revealed a pending lesson's answer in the Codex.
    MarkPeeked {
        cartridge_id: String,
        question: String,
    },
}

#[derive(Resource, Default)]
pub(super) struct Inbox(pub(super) VecDeque<EngineCommand>);

#[derive(Resource, Default)]
pub(super) struct Effects(pub(super) VecDeque<EngineEffect>);

pub(super) fn apply_commands(
    mut inbox: ResMut<Inbox>,
    mut state: ResMut<GameState>,
    mut effects: ResMut<Effects>,
) {
    while let Some(command) = inbox.0.pop_front() {
        match command {
            EngineCommand::Power(powered) => {
                state.powered = powered;
                state.held.clear();
                state.quiz = None;
                // The run ends; its deferred copies' lessons requeue next run.
                state.deferred_retries.clear();
                state.logs.clear();
                effects.0.push_back(EngineEffect::AbortQuest);
                state.transition(if powered { Screen::Boot } else { Screen::Off });
            }
            EngineCommand::ReducedMotion(reduced) => state.reduced_motion = reduced,
            EngineCommand::AiProvider(provider) => {
                state.ai_provider = provider.map(|name| name.to_ascii_uppercase());
            }
            EngineCommand::BootComplete => {
                if state.screen == Screen::Boot && state.has_game() {
                    state.start_machine();
                }
            }
            EngineCommand::Cartridge(cartridge) => {
                state.machine = cartridge
                    .as_ref()
                    .map(|cartridge| SceneMachine::new((*cartridge.machine).clone()));
                state.cartridge = cartridge;
                state.quest_selected = 0;
                state.menu_selected = 0;
                state.quiz = None;
                state.consumed_questions = 0;
                state.deferred_retries.clear();
                (state.batch_ends, state.batch_levels) = state
                    .cartridge
                    .as_ref()
                    .filter(|cartridge| cartridge.mode() == CartridgeMode::Quiz)
                    .map(|cartridge| {
                        let ends = &cartridge.question_batch_ends;
                        let levels = if cartridge.question_batch_levels.len() == ends.len() {
                            cartridge.question_batch_levels.clone()
                        } else {
                            (1..=ends.len() as u32).collect()
                        };
                        rebatch(ends, &levels, &cartridge.questions)
                    })
                    .unwrap_or_default();
                state.pending_questions = None;
                state.questions_loading = false;
                state.question_retry_ticks = 0;
                state.question_failure = None;
                // Replies to requests for the previous insert no longer land.
                state.question_request_seq = state.question_request_seq.wrapping_add(1);
                if state.cartridge.as_ref().is_some_and(|cartridge| {
                    cartridge.mode() == CartridgeMode::Quiz && cartridge.questions.is_empty()
                }) {
                    request_question_batch(&mut state, &mut effects, 1);
                }
                if state.powered {
                    state.transition(Screen::Boot);
                }
            }
            EngineCommand::Questions {
                cartridge_id,
                result,
                seq,
            } => {
                let is_current_quiz = state.cartridge.as_ref().is_some_and(|cartridge| {
                    cartridge.id == cartridge_id && cartridge.mode() == CartridgeMode::Quiz
                });
                // A superseded request's reply must not end the newer
                // request's loading state, add a batch of its own, or
                // report a failure the newer request has not had.
                if !is_current_quiz || seq != state.question_request_seq {
                    continue;
                }
                state.questions_loading = false;
                let questions = match result {
                    Ok(questions) if !questions.is_empty() => questions,
                    failed => {
                        let reason = failed.err().unwrap_or_else(|| "NO NEW QUESTIONS".into());
                        state.question_failure = Some(oracle_failure_reason(&reason));
                        state.question_retry_ticks = 300;
                        continue;
                    }
                };
                state.question_failure = None;
                state.question_retry_ticks = 0;
                let level = state.question_request_level;
                if state.screen == Screen::Quiz {
                    match state.pending_questions.as_mut() {
                        Some((pending_id, pending, _)) if *pending_id == cartridge_id => {
                            pending.extend(questions);
                        }
                        _ => state.pending_questions = Some((cartridge_id, questions, level)),
                    }
                    continue;
                }
                if !state.run_is_live() {
                    retire_consumed_questions(&mut state);
                }
                append_question_batch(&mut state, questions, level);
            }
            EngineCommand::Input { button, pressed } => {
                let was_held = state.held.contains(&button);
                if pressed {
                    state.held.insert(button);
                    if !was_held {
                        handle_press(&mut state, &mut effects, button);
                    }
                } else {
                    state.held.remove(&button);
                }
            }
            EngineCommand::QuestOutput {
                line,
                stderr,
                quest,
            } => {
                if state.screen == Screen::Battle && quest == state.quest_generation {
                    for wrapped in wrap_text(&line, 37).into_iter().take(3) {
                        state.logs.push_back((wrapped, stderr));
                    }
                    while state.logs.len() > 7 {
                        state.logs.pop_front();
                    }
                }
            }
            EngineCommand::QuestDone { success, quest } => {
                if state.screen == Screen::Battle && quest == state.quest_generation {
                    state.signal(if success {
                        SceneSignal::Victory
                    } else {
                        SceneSignal::Defeat
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn late_oracle_result_cannot_mutate_a_different_cartridge() {
        let mut engine = GameEngine::new();
        let first = quiz_cartridge();
        let mut second = quiz_cartridge();
        second.id = "/tmp/second".into();
        second.questions[0].question = "SECOND CARTRIDGE".into();
        issue(&mut engine, EngineCommand::Cartridge(Some(first)));
        issue(&mut engine, EngineCommand::Cartridge(Some(second)));
        deliver(
            &mut engine,
            "/tmp/engine-test",
            vec![QuizQuestion {
                question: "STALE".into(),
                choices: vec!["A".into()],
                answer: 0,
                ..Default::default()
            }],
        );
        let state = engine.app.world().resource::<GameState>();
        assert_eq!(
            state.cartridge.as_ref().unwrap().questions[0].question,
            "SECOND CARTRIDGE"
        );
    }

    #[test]
    fn powered_off_cartridge_switch_replaces_the_question_deck() {
        let mut engine = GameEngine::new();
        let first = quiz_cartridge();
        let mut second = quiz_cartridge();
        second.id = "/tmp/second".into();
        second.questions[0].question = "FRESH QUESTIONS".into();

        issue(&mut engine, EngineCommand::Cartridge(Some(first)));
        issue(&mut engine, EngineCommand::Power(true));
        issue(&mut engine, EngineCommand::Power(false));
        issue(&mut engine, EngineCommand::Cartridge(Some(second)));

        let state = engine.app.world().resource::<GameState>();
        let cartridge = state.cartridge.as_ref().unwrap();
        assert_eq!(state.screen, Screen::Off);
        assert_eq!(cartridge.id, "/tmp/second");
        assert_eq!(cartridge.questions[0].question, "FRESH QUESTIONS");
        assert!(state.quiz.is_none());
        assert!(state.pending_questions.is_none());
    }

    #[test]
    fn a_superseded_question_reply_cannot_land() {
        let mut engine = GameEngine::new();
        let mut cartridge = quiz_cartridge();
        cartridge.questions.clear();
        issue(
            &mut engine,
            EngineCommand::Cartridge(Some(cartridge.clone())),
        );
        let first = request_seqs(&engine.take_effects());
        issue(&mut engine, EngineCommand::Cartridge(Some(cartridge)));
        let second = request_seqs(&engine.take_effects());
        assert_eq!((first.len(), second.len()), (1, 1));
        assert_ne!(first[0].1, second[0].1);

        issue(
            &mut engine,
            EngineCommand::Questions {
                cartridge_id: "/tmp/engine-test".into(),
                result: Ok(vec![concept_question(0)]),
                seq: first[0].1,
            },
        );
        let state = engine_state(&engine);
        assert!(
            state.questions_loading,
            "the newer request is still in flight"
        );
        assert_eq!(state.question_count(), 0);

        issue(
            &mut engine,
            EngineCommand::Questions {
                cartridge_id: "/tmp/engine-test".into(),
                result: Ok(vec![concept_question(1)]),
                seq: second[0].1,
            },
        );
        let state = engine_state(&engine);
        assert!(!state.questions_loading);
        assert_eq!(
            state.cartridge.as_ref().unwrap().questions[0].question,
            concept_question(1).question
        );
    }

    #[test]
    fn an_empty_batch_is_reported_as_a_failure_too() {
        let mut engine = waiting_oracle_engine();
        deliver(&mut engine, "/tmp/engine-test", Vec::new());
        let state = engine_state(&engine);
        assert_eq!(state.question_failure.as_deref(), Some("NO NEW QUESTIONS"));
        assert!(state.question_retry_ticks > 0);

        // Inserting a cartridge forgets the previous cartridge's failure.
        let mut cartridge = quiz_cartridge();
        cartridge.questions.clear();
        issue(&mut engine, EngineCommand::Cartridge(Some(cartridge)));
        assert_eq!(engine_state(&engine).question_failure, None);
    }

    #[test]
    fn a_reply_during_the_quiz_appends_to_a_pending_batch() {
        let mut engine = GameEngine::new();
        issue(
            &mut engine,
            EngineCommand::Cartridge(Some(quiz_cartridge())),
        );
        {
            let mut state = engine.app.world_mut().resource_mut::<GameState>();
            state.screen = Screen::Quiz;
            state.pending_questions =
                Some(("/tmp/engine-test".into(), vec![concept_question(0)], 1));
            state.questions_loading = true;
        }
        deliver(&mut engine, "/tmp/engine-test", vec![concept_question(1)]);
        let pending = engine_state(&engine).pending_questions.as_ref().unwrap();
        assert_eq!(
            pending
                .1
                .iter()
                .map(|question| question.question.clone())
                .collect::<Vec<_>>(),
            vec![concept_question(0).question, concept_question(1).question]
        );
    }

    #[test]
    fn boot_runs_provenance_and_fanfare_before_title_navigation() {
        let mut engine = GameEngine::new();
        issue(
            &mut engine,
            EngineCommand::Cartridge(Some(quiz_cartridge())),
        );
        issue(&mut engine, EngineCommand::Power(true));
        assert_eq!(engine.screen(), Screen::Boot);
        issue(&mut engine, EngineCommand::BootComplete);
        assert_eq!(engine.screen(), Screen::Copyright);
        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::Start,
                pressed: true,
            },
        );
        assert_eq!(engine.screen(), Screen::Copyright);
        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::Start,
                pressed: false,
            },
        );
        for _ in 0..59 {
            engine.update();
        }
        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::Start,
                pressed: true,
            },
        );
        assert_eq!(engine.screen(), Screen::OpeningFanfare);
        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::Start,
                pressed: false,
            },
        );
        for _ in 0..89 {
            engine.update();
        }
        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::Start,
                pressed: true,
            },
        );
        assert_eq!(engine.screen(), Screen::Title);
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
                button: Button::Start,
                pressed: true,
            },
        );
        assert_eq!(engine.screen(), Screen::QuizMenu);
    }

    #[test]
    fn an_earlier_quests_output_never_lands_in_a_later_battle() {
        let machine = SceneMachineDefinition::compile(
            "quest-select",
            vec![
                SceneSpec {
                    id: "quest-select".into(),
                    handler: SceneHandler::QuestSelect,
                    transitions: vec![SceneTransition {
                        signal: SceneSignal::QuestSelected,
                        target: "battle".into(),
                        after_ticks: None,
                    }],
                },
                SceneSpec {
                    id: "battle".into(),
                    handler: SceneHandler::Battle,
                    transitions: vec![
                        SceneTransition {
                            signal: SceneSignal::Victory,
                            target: "quest-select".into(),
                            after_ticks: None,
                        },
                        SceneTransition {
                            signal: SceneSignal::Defeat,
                            target: "quest-select".into(),
                            after_ticks: None,
                        },
                    ],
                },
            ],
        )
        .unwrap();
        let mut cartridge = quiz_cartridge();
        cartridge.mode = CartridgeMode::Custom;
        cartridge.machine = Box::new(machine);
        cartridge.quests = vec![QuestSpec {
            name: "BUILD".into(),
            boss: "NONE".into(),
            command: "true".into(),
        }];
        let mut engine = GameEngine::new();
        issue(&mut engine, EngineCommand::Cartridge(Some(cartridge)));
        issue(&mut engine, EngineCommand::Power(true));
        issue(&mut engine, EngineCommand::BootComplete);
        engine.take_effects();
        let start_quest = |engine: &mut GameEngine| {
            press(engine, Button::A);
            assert_eq!(engine.screen(), Screen::Battle);
            match engine.take_effects().as_slice() {
                [EngineEffect::RunQuest { quest, .. }] => *quest,
                effects => panic!("expected one RunQuest, got {effects:?}"),
            }
        };
        let output = |line: &str, quest| EngineCommand::QuestOutput {
            line: line.into(),
            stderr: false,
            quest,
        };
        let logged = |engine: &GameEngine, line: &str| {
            engine_state(engine)
                .logs
                .iter()
                .any(|(logged, _)| logged == line)
        };

        let first = start_quest(&mut engine);
        issue(&mut engine, output("FIRST", first));
        assert!(logged(&engine, "FIRST"));
        issue(
            &mut engine,
            EngineCommand::QuestDone {
                success: true,
                quest: first,
            },
        );
        assert_eq!(engine.screen(), Screen::QuestSelect);

        let second = start_quest(&mut engine);
        assert_ne!(first, second);
        // A helper the first quest left running writes after the second began.
        issue(&mut engine, output("LEFTOVER", first));
        assert!(!logged(&engine, "LEFTOVER"), "a stale line is dropped");
        issue(
            &mut engine,
            EngineCommand::QuestDone {
                success: false,
                quest: first,
            },
        );
        assert_eq!(
            engine.screen(),
            Screen::Battle,
            "a stale completion cannot end the current battle"
        );
        issue(&mut engine, output("SECOND", second));
        assert!(logged(&engine, "SECOND"));
    }

    #[test]
    fn boot_waits_for_device_firmware() {
        let mut engine = GameEngine::new();
        issue(
            &mut engine,
            EngineCommand::Cartridge(Some(quiz_cartridge())),
        );
        issue(&mut engine, EngineCommand::Power(true));
        for _ in 0..180 {
            engine.update();
        }
        assert_eq!(engine.screen(), Screen::Boot);
    }

    #[test]
    fn boot_cannot_finish_without_a_cartridge() {
        let mut engine = GameEngine::new();
        issue(&mut engine, EngineCommand::Power(true));
        issue(&mut engine, EngineCommand::BootComplete);
        assert_eq!(engine.screen(), Screen::Boot);
    }

    #[test]
    fn reinserting_a_cartridge_drops_replies_to_its_earlier_requests() {
        let mut engine = waiting_oracle_engine();
        let stale = engine_state(&engine).question_request_seq;
        assert!(engine_state(&engine).questions_loading);

        issue(
            &mut engine,
            EngineCommand::Cartridge(Some(quiz_cartridge())),
        );
        issue(
            &mut engine,
            EngineCommand::Questions {
                cartridge_id: "/tmp/engine-test".into(),
                result: Ok((0..QUESTION_BATCH_SIZE).map(concept_question).collect()),
                seq: stale,
            },
        );
        let state = engine_state(&engine);
        assert_eq!(state.question_count(), 1, "the stale reply never lands");
        assert!(state.question_failure.is_none());
    }

    #[test]
    fn inserting_a_cartridge_drops_a_held_delivery_and_requests_its_own() {
        let mut engine = batch_quiz_engine(QUESTION_BATCH_SIZE);
        engine.update();
        deliver(
            &mut engine,
            "/tmp/engine-test",
            (QUESTION_BATCH_SIZE..2 * QUESTION_BATCH_SIZE)
                .map(concept_question)
                .collect(),
        );
        assert!(engine_state(&engine).pending_questions.is_some());
        let _ = engine.take_effects();

        let mut other = quiz_cartridge();
        other.id = "/tmp/other-test".into();
        other.questions.clear();
        issue(&mut engine, EngineCommand::Cartridge(Some(other)));
        assert!(engine_state(&engine).pending_questions.is_none());
        assert!(
            engine.take_effects().iter().any(|effect| matches!(
                effect,
                EngineEffect::RequestQuestions { cartridge_id, .. } if cartridge_id == "/tmp/other-test"
            )),
            "the new cartridge asks for its first batch at once"
        );
    }
}
