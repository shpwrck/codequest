use super::*;

/// Ticks after a first B in which a second B leaves an active question.
pub(super) const QUIZ_LEAVE_CONFIRM_TICKS: u16 = 90;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Button {
    Up,
    Down,
    Left,
    Right,
    A,
    B,
    Start,
    Select,
    L,
    R,
}

impl Button {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "up" => Some(Self::Up),
            "down" => Some(Self::Down),
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            "a" => Some(Self::A),
            "b" => Some(Self::B),
            "start" => Some(Self::Start),
            "select" => Some(Self::Select),
            "l" => Some(Self::L),
            "r" => Some(Self::R),
            _ => None,
        }
    }
}

pub(super) fn cycle_index(index: &mut usize, count: usize, direction: isize) {
    *index = (*index as isize + direction).rem_euclid(count as isize) as usize;
}

pub(super) fn adjust_hero(state: &mut GameState, direction: isize) {
    match state.hero_row {
        0 => cycle_index(&mut state.hero_name, HERO_NAMES.len(), direction),
        1 => cycle_index(&mut state.hero_class, HERO_CLASSES.len(), direction),
        2 => cycle_index(&mut state.hero_style, HERO_STYLES.len(), direction),
        _ => {}
    }
}

pub(super) fn handle_press(state: &mut GameState, effects: &mut Effects, button: Button) {
    if !state.powered {
        return;
    }
    match state.screen {
        Screen::Off => {}
        Screen::Boot => {}
        Screen::Copyright => {
            if matches!(button, Button::A | Button::Start) {
                state.signal(SceneSignal::Continue);
            }
        }
        Screen::OpeningFanfare => {
            if matches!(button, Button::A | Button::Start) {
                state.signal(SceneSignal::Continue);
            }
        }
        Screen::Title => {
            if matches!(button, Button::A | Button::Start) {
                state.signal(SceneSignal::Continue);
            }
        }
        Screen::QuizMenu => match button {
            Button::Up | Button::Down => state.menu_selected = 1 - state.menu_selected,
            Button::B => {
                state.signal(SceneSignal::Back);
            }
            Button::A | Button::Start => {
                if state.menu_selected == 1 {
                    // The second option opens the Codex when this menu routes
                    // to one and otherwise keeps its original return path.
                    if !state.signal(SceneSignal::OpenCodex) {
                        state.signal(SceneSignal::Back);
                    }
                } else {
                    state.hero_row = 0;
                    state.signal(SceneSignal::NewRun);
                }
            }
            _ => {}
        },
        Screen::CharacterCreation => match button {
            Button::Up => state.hero_row = (state.hero_row + 3) % 4,
            Button::Down => state.hero_row = (state.hero_row + 1) % 4,
            Button::Left => adjust_hero(state, -1),
            Button::Right => adjust_hero(state, 1),
            Button::B => {
                state.signal(SceneSignal::Back);
            }
            Button::A if state.hero_row < 3 => adjust_hero(state, 1),
            Button::A | Button::Start => begin_quiz_run(state),
            _ => {}
        },
        Screen::Oracle => {
            if button == Button::B {
                leave_quiz_run(state);
            }
        }
        Screen::Quiz => {
            present_current_question(state);
            let Some(run) = state.quiz.as_mut() else {
                // Without a run there is nothing to confirm; B still leaves.
                if button == Button::B {
                    leave_quiz_run(state);
                }
                return;
            };
            if let Some((_, hold)) = run.feedback {
                // The lesson card ignores every input during the hold, then
                // only A or Start continues; B cannot abandon mid-lesson.
                if hold == 0 && matches!(button, Button::A | Button::Start) {
                    continue_after_lesson(state);
                }
                return;
            }
            // Any press disarms a pending leave; only a second B consumes it.
            let leave_armed = std::mem::take(&mut run.leave_armed) > 0;
            let choice_count = state
                .cartridge
                .as_ref()
                .and_then(|cart| cart.questions.get(run.question))
                .map_or(1, |question| question.choices.len().max(1));
            match button {
                Button::Up => run.selected = (run.selected + choice_count - 1) % choice_count,
                Button::Down => run.selected = (run.selected + 1) % choice_count,
                Button::B if leave_armed => leave_quiz_run(state),
                Button::B => run.leave_armed = QUIZ_LEAVE_CONFIRM_TICKS,
                Button::A => commit_answer(state, effects),
                _ => {}
            }
        }
        Screen::LevelUp => {
            if matches!(button, Button::A | Button::Start) && state.level_up_can_continue() {
                let signal = state.level_up_signal();
                state.signal(signal);
            }
        }
        Screen::GameOver => {
            if matches!(button, Button::A | Button::B | Button::Start) {
                state.signal(SceneSignal::Replay);
            }
        }
        Screen::Codex => {
            // Paging wraps between the mastery overview and the newest lesson,
            // and every page turn seals a pending answer again. A and Start
            // only reveal a pending lesson's sealed answer; elsewhere they are
            // inactive. The Codex never changes the question deck.
            let pages = state.codex_page_count();
            let page = state.codex_page;
            match button {
                Button::Left | Button::Up | Button::L => {
                    cycle_index(&mut state.codex_page, pages, -1)
                }
                Button::Right | Button::Down | Button::R => {
                    cycle_index(&mut state.codex_page, pages, 1)
                }
                Button::A | Button::Start => {
                    state.reveal_codex_answer(effects);
                }
                Button::B => {
                    state.signal(SceneSignal::Back);
                }
                _ => {}
            }
            if state.codex_page != page {
                state.codex_revealed = false;
            }
        }
        Screen::QuestSelect => {
            let count = state.cartridge.as_ref().map_or(0, |cart| cart.quests.len());
            match button {
                Button::Up if count > 0 => {
                    state.quest_selected = (state.quest_selected + count - 1) % count
                }
                Button::Down if count > 0 => {
                    state.quest_selected = (state.quest_selected + 1) % count
                }
                Button::L if count > 0 => {
                    state.quest_selected = state.quest_selected.saturating_sub(4)
                }
                Button::R if count > 0 => {
                    state.quest_selected = (state.quest_selected + 4).min(count - 1)
                }
                Button::B => {
                    state.signal(SceneSignal::Back);
                }
                Button::A | Button::Start if count > 0 => {
                    let quest =
                        state.cartridge.as_ref().unwrap().quests[state.quest_selected].clone();
                    state.active_boss = quest.boss;
                    state.logs.clear();
                    state.logs.push_back((format!("> {}", quest.name), false));
                    if state.signal(SceneSignal::QuestSelected) && state.screen == Screen::Battle {
                        state.quest_generation = state.quest_generation.wrapping_add(1);
                        effects.0.push_back(EngineEffect::RunQuest {
                            command: quest.command,
                            quest: state.quest_generation,
                        });
                    }
                }
                _ => {}
            }
        }
        Screen::Battle => {
            if button == Button::B {
                effects.0.push_back(EngineEffect::AbortQuest);
                state.logs.push_back(("RETREAT REQUESTED...".into(), true));
            }
        }
        Screen::Victory | Screen::Defeat => match button {
            Button::A | Button::B | Button::Start => {
                state.signal(SceneSignal::Continue);
            }
            _ => {}
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oracle_ignores_a_while_questions_are_loading() {
        let mut control = waiting_oracle_engine();
        let mut pressed = waiting_oracle_engine();

        control.update();
        issue(
            &mut pressed,
            EngineCommand::Input {
                button: Button::A,
                pressed: true,
            },
        );

        assert!(
            pressed.frame() == control.frame(),
            "A changed the Oracle frame"
        );
    }

    #[test]
    fn oracle_b_returns_to_the_quiz_menu_from_an_indefinite_wait() {
        let mut engine = waiting_oracle_engine();
        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::B,
                pressed: true,
            },
        );

        assert_eq!(engine.screen(), Screen::QuizMenu);

        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::B,
                pressed: false,
            },
        );
        assert_eq!(engine.screen(), Screen::QuizMenu);
    }

    #[test]
    fn quiz_result_hold_replaces_active_controls_and_ignores_back() {
        let mut engine = playing_quiz_engine();
        let active_controls = frame_region(engine.frame(), 0..WIDTH, 148..HEIGHT);

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
        let review_controls = frame_region(engine.frame(), 0..WIDTH, 148..HEIGHT);
        assert_ne!(review_controls, active_controls);

        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::B,
                pressed: true,
            },
        );
        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::B,
                pressed: false,
            },
        );
        assert_eq!(engine.screen(), Screen::Quiz);

        // A and Start stay inert until the hold ends; the card then waits.
        press(&mut engine, Button::A);
        press(&mut engine, Button::Start);
        assert!(engine_state(&engine)
            .quiz
            .as_ref()
            .unwrap()
            .feedback
            .is_some());
        for _ in 0..QUIZ_FEEDBACK_TICKS {
            engine.update();
        }
        for _ in 0..300 {
            engine.update();
        }
        let run = engine_state(&engine).quiz.as_ref().unwrap();
        assert_eq!(run.feedback.map(|(_, hold)| hold), Some(0));
        assert_eq!(run.question, 0, "the lesson card must not auto-advance");
        for button in [Button::B, Button::B, Button::Up, Button::Down] {
            press(&mut engine, button);
        }
        assert_eq!(engine.screen(), Screen::Quiz);
        assert!(engine_state(&engine)
            .quiz
            .as_ref()
            .unwrap()
            .feedback
            .is_some());

        press(&mut engine, Button::Start);
        let run = engine_state(&engine).quiz.as_ref().unwrap();
        assert!(run.feedback.is_none());
        assert_eq!(run.question, 1);
    }

    #[test]
    fn oracle_a_press_cannot_answer_a_question_that_arrives_mid_input() {
        let mut engine = waiting_oracle_engine();
        for _ in 0..75 {
            engine.update();
        }
        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::A,
                pressed: true,
            },
        );
        deliver(
            &mut engine,
            "/tmp/engine-test",
            vec![QuizQuestion {
                question: "WHAT ARRIVED SAFELY?".into(),
                choices: vec![
                    "A QUESTION".into(),
                    "A KEY PRESS".into(),
                    "A GLITCH".into(),
                    "A COMMAND".into(),
                ],
                answer: 0,
                ..Default::default()
            }],
        );
        assert_eq!(engine.screen(), Screen::Quiz);
        let unanswered = engine.frame().to_vec();

        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::A,
                pressed: false,
            },
        );

        assert!(
            engine.frame() == unanswered,
            "releasing A answered the question"
        );
    }

    #[test]
    fn character_creation_controls_change_the_visible_setup() {
        let mut engine = GameEngine::new();
        issue(
            &mut engine,
            EngineCommand::Cartridge(Some(quiz_cartridge())),
        );
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
        let before = engine.frame().to_vec();
        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::Down,
                pressed: true,
            },
        );

        assert!(
            engine
                .frame()
                .iter()
                .zip(before.iter())
                .any(|(after, before)| after != before),
            "character selection did not change the framebuffer"
        );
    }

    #[test]
    fn leaving_an_active_question_needs_a_second_b_inside_the_window() {
        let mut engine = batch_quiz_engine(2);

        press(&mut engine, Button::B);
        assert_eq!(engine.screen(), Screen::Quiz);
        assert_eq!(
            quiz_feedback_banner(engine_state(&engine).quiz.as_ref().unwrap()),
            "B AGAIN:LEAVE"
        );

        // Any other button disarms the confirmation.
        press(&mut engine, Button::Down);
        press(&mut engine, Button::B);
        assert_eq!(engine.screen(), Screen::Quiz);

        // So does the timeout.
        for _ in 0..QUIZ_LEAVE_CONFIRM_TICKS {
            engine.update();
        }
        assert_eq!(engine_state(&engine).quiz.as_ref().unwrap().leave_armed, 0);
        press(&mut engine, Button::B);
        assert_eq!(engine.screen(), Screen::Quiz);

        // B cannot arm during the lesson hold or on the lesson card.
        commit(&mut engine, true);
        press(&mut engine, Button::B);
        for _ in 0..QUIZ_FEEDBACK_TICKS {
            engine.update();
        }
        press(&mut engine, Button::B);
        press(&mut engine, Button::B);
        assert_eq!(engine.screen(), Screen::Quiz);
        assert_eq!(engine_state(&engine).quiz.as_ref().unwrap().leave_armed, 0);

        // A second B on the window's last live tick leaves the run.
        press(&mut engine, Button::A);
        press(&mut engine, Button::B);
        for _ in 0..QUIZ_LEAVE_CONFIRM_TICKS - 3 {
            engine.update();
        }
        press(&mut engine, Button::B);
        assert_eq!(engine.screen(), Screen::QuizMenu);
    }

    #[test]
    fn quest_command_only_starts_when_the_graph_enters_battle() {
        let machine = SceneMachineDefinition::compile(
            "quest-select",
            vec![
                SceneSpec {
                    id: "quest-select".into(),
                    handler: SceneHandler::QuestSelect,
                    transitions: vec![SceneTransition {
                        signal: SceneSignal::QuestSelected,
                        target: "title".into(),
                        after_ticks: None,
                    }],
                },
                SceneSpec {
                    id: "title".into(),
                    handler: SceneHandler::Title,
                    transitions: vec![],
                },
            ],
        )
        .unwrap();
        let mut cartridge = quiz_cartridge();
        cartridge.mode = CartridgeMode::Custom;
        cartridge.machine = Box::new(machine);
        cartridge.quests = vec![QuestSpec {
            name: "SAFE ROUTE".into(),
            boss: "NONE".into(),
            command: "should-not-run".into(),
        }];
        let mut engine = GameEngine::new();
        issue(&mut engine, EngineCommand::Cartridge(Some(cartridge)));
        issue(&mut engine, EngineCommand::Power(true));
        issue(&mut engine, EngineCommand::BootComplete);
        engine.take_effects();

        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::Start,
                pressed: true,
            },
        );

        assert_eq!(engine.screen(), Screen::Title);
        assert!(engine.take_effects().is_empty());
    }

    #[test]
    fn held_button_only_generates_one_edge() {
        let mut engine = GameEngine::new();
        issue(
            &mut engine,
            EngineCommand::Cartridge(Some(quiz_cartridge())),
        );
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
                pressed: true,
            },
        );
        assert_eq!(engine.screen(), Screen::QuizMenu);
    }
}
