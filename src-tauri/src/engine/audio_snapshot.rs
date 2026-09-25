use super::*;

/// Samples the observable game state the audio director listens to. Sound is
/// derived from these snapshots only; gameplay code never calls audio.
pub(super) fn audio_snapshot(state: &GameState) -> AudioSnapshot {
    let scene = match state.screen {
        Screen::Off => AudioScene::Off,
        Screen::Boot => AudioScene::Boot,
        Screen::Copyright => AudioScene::Copyright,
        Screen::OpeningFanfare => AudioScene::Opening(match state.opening_beat() {
            OpeningBeat::Legacy => audio::OpeningBeat::Legacy,
            OpeningBeat::SourceEmber => audio::OpeningBeat::SourceEmber,
            OpeningBeat::ArchiveAnswer => audio::OpeningBeat::ArchiveAnswer,
            OpeningBeat::MemoryVault => audio::OpeningBeat::MemoryVault,
            OpeningBeat::Convergence => audio::OpeningBeat::Convergence,
            OpeningBeat::OracleAwakening => audio::OpeningBeat::OracleAwakening,
        }),
        Screen::Title => AudioScene::Title,
        Screen::QuizMenu => AudioScene::QuizMenu,
        Screen::CharacterCreation => AudioScene::CharacterCreation,
        Screen::Oracle => AudioScene::Oracle,
        Screen::Quiz => AudioScene::Quiz,
        Screen::LevelUp => AudioScene::LevelUp,
        Screen::GameOver => AudioScene::GameOver,
        Screen::QuestSelect => AudioScene::QuestSelect,
        Screen::Battle => AudioScene::Battle,
        Screen::Victory => AudioScene::Victory,
        Screen::Defeat => AudioScene::Defeat,
        Screen::Codex => AudioScene::Codex,
    };
    let held = state.held.iter().fold(0, |bits, button| {
        bits | match button {
            Button::Up => pad::UP,
            Button::Down => pad::DOWN,
            Button::Left => pad::LEFT,
            Button::Right => pad::RIGHT,
            Button::A => pad::A,
            Button::B => pad::B,
            Button::Start => pad::START,
            Button::Select => pad::SELECT,
            Button::L => pad::L,
            Button::R => pad::R,
        }
    });
    // Mirrors the Oracle's truthful status line.
    let questions = state.question_status();
    AudioSnapshot {
        powered: state.powered,
        scene,
        scene_ticks: state.screen_ticks,
        held,
        menu_selected: state.menu_selected,
        hero_row: state.hero_row,
        hero_name: state.hero_name,
        hero_class: state.hero_class,
        hero_style: state.hero_style,
        quest_selected: state.quest_selected,
        codex_page: state.codex_page,
        codex_revealed: state.codex_revealed,
        run: state.quiz.as_ref().map(|run| RunAudio {
            question: run.question,
            selected: run.selected,
            phase: match run.feedback {
                None => AnswerPhase::Choosing,
                Some((true, _)) => AnswerPhase::Correct,
                Some((false, _)) => AnswerPhase::Wrong,
            },
            hearts: run.hearts,
            multiplier: streak_multiplier(run.streak),
            insight: InsightStage::from_score(run.score).index(),
            level: run.level,
            completed_batches: run.completed_batches,
            redeemed: run.redeemed,
            lens_woke: run.lens_woke.map(|(_, stage)| stage.min(3) as u8),
            leave_armed: run.leave_armed > 0,
        }),
        data: state.oracle_data,
        data_stage: threshold_stage(state.oracle_data, &DATA_CHARGE_THRESHOLDS),
        bugs: state.oracle_bug_hits,
        breach_stage: threshold_stage(state.oracle_bug_hits, &BUG_BREACH_THRESHOLDS),
        questions,
        tier: match state.presentation_tier() {
            PresentationTier::Initiate => Tier::Initiate,
            PresentationTier::Adept => Tier::Adept,
            PresentationTier::OracleBound => Tier::OracleBound,
        },
    }
}

pub(super) fn direct_audio(state: Res<GameState>, mut audio: ResMut<AudioOut>) {
    audio.observe(&audio_snapshot(&state));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_input_edge_starts_at_most_one_distinct_cue() {
        use audio::Cue;
        let mut engine = GameEngine::new();
        issue(
            &mut engine,
            EngineCommand::Cartridge(Some(quiz_cartridge())),
        );
        issue(&mut engine, EngineCommand::Power(true));
        finish_opening(&mut engine);

        let script = [
            (Button::Start, Some(Cue::Confirm)),
            (Button::Down, Some(Cue::Navigate(1))),
            (Button::Up, Some(Cue::Navigate(0))),
            (Button::Left, Some(Cue::Unavailable)),
            (Button::A, Some(Cue::Confirm)),
            (Button::Right, Some(Cue::Trait { row: 0, value: 1 })),
            (Button::Down, Some(Cue::Navigate(1))),
            (Button::A, Some(Cue::Trait { row: 1, value: 1 })),
            (Button::Down, Some(Cue::Navigate(2))),
            (Button::Left, Some(Cue::Trait { row: 2, value: 4 })),
            (Button::Down, Some(Cue::Navigate(3))),
            (Button::Right, Some(Cue::Unavailable)),
            (Button::Start, Some(Cue::BeginRun)),
            (Button::A, Some(Cue::Unavailable)),
            (Button::Left, None),
        ];
        for (button, expected) in script {
            let [press, release] = tap_cues(&mut engine, button);
            assert_eq!(press, expected, "{button:?} on {:?}", engine.screen());
            assert_eq!(release, None, "releasing {button:?} must stay silent");
        }
        assert_eq!(engine.screen(), Screen::Oracle);

        let mut arrival = Vec::new();
        for _ in 0..75 {
            engine.update();
            arrival.extend(last_cue(&engine));
        }
        assert_eq!(engine.screen(), Screen::Quiz);
        // The first falling shard lands on the hero before the vision arrives.
        assert_eq!(arrival, [Cue::DataCollect(1), Cue::QuestionReveal]);

        // Choices are shuffled, so walk the cursor to wherever the answer is.
        let answer = answer_slot(&engine);
        let mut script = vec![
            (Button::Down, Some(Cue::Cursor(1))),
            (Button::Up, Some(Cue::Cursor(0))),
        ];
        script.extend((1..=answer).map(|slot| (Button::Down, Some(Cue::Cursor(slot)))));
        script.extend([
            (Button::Right, Some(Cue::Unavailable)),
            (Button::A, Some(Cue::Correct)),
            // The answer review ignores input, and so does the speaker.
            (Button::Down, None),
            (Button::B, None),
        ]);
        for (button, expected) in script {
            let [press, release] = tap_cues(&mut engine, button);
            assert_eq!(press, expected, "{button:?} in the quiz");
            assert_eq!(release, None);
        }
    }

    #[test]
    fn engine_audio_is_silent_while_off_or_booting() {
        let mut engine = GameEngine::new();
        issue(
            &mut engine,
            EngineCommand::Cartridge(Some(quiz_cartridge())),
        );
        for _ in 0..60 {
            engine.update();
        }
        issue(&mut engine, EngineCommand::Power(true));
        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::Start,
                pressed: true,
            },
        );
        for _ in 0..180 {
            engine.update();
        }
        assert_eq!(engine.screen(), Screen::Boot);
        let silent = engine.take_audio();
        assert!(silent.notes.is_empty(), "{:?}", silent.notes);
        assert!(silent.tick > 240, "the engine tick advances while silent");

        issue(&mut engine, EngineCommand::BootComplete);
        for _ in 0..60 {
            engine.update();
        }
        let chronicle = engine.take_audio();
        assert!(chronicle.notes.iter().any(|note| !note.is_cut()));
        assert!(chronicle
            .notes
            .iter()
            .all(|note| note.tick > silent.tick && note.tick <= chronicle.tick));

        // Power loss may only cut voices, never start one.
        issue(&mut engine, EngineCommand::Power(false));
        for _ in 0..120 {
            engine.update();
        }
        assert!(engine.take_audio().notes.iter().all(|note| note.is_cut()));
    }

    #[test]
    fn datafall_loop_is_cut_on_the_tick_b_leaves_the_oracle() {
        let mut engine = waiting_oracle_engine();
        let _ = engine.take_audio();
        for _ in 0..157 {
            engine.update();
        }
        let before = engine.take_audio();
        assert!(before.notes.iter().any(|note| !note.is_cut()));

        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::B,
                pressed: true,
            },
        );
        assert_eq!(engine.screen(), Screen::QuizMenu);
        assert_eq!(last_cue(&engine), Some(audio::Cue::Leave));
        let exit = engine.take_audio();
        let tails = before
            .notes
            .iter()
            .filter(|note| !note.is_cut() && note.tick + u64::from(note.duration_ticks) > exit.tick)
            .collect::<Vec<_>>();
        assert!(
            !tails.is_empty(),
            "the exit should interrupt the Datafall loop"
        );
        for tail in tails {
            assert!(
                exit.notes
                    .iter()
                    .any(|note| note.voice == tail.voice && note.tick == exit.tick),
                "{:?} carried the Datafall loop into the menu",
                tail.voice
            );
        }
    }

    #[test]
    fn oracle_retry_and_ready_states_are_audible_once() {
        let mut engine = waiting_oracle_engine();
        deliver(&mut engine, "/tmp/engine-test", Vec::new());
        assert_eq!(last_cue(&engine), Some(audio::Cue::Retry));
        engine.update();
        assert_eq!(last_cue(&engine), None);

        deliver(&mut engine, "/tmp/engine-test", quiz_cartridge().questions);
        assert_eq!(last_cue(&engine), Some(audio::Cue::Ready));
    }

    #[test]
    fn audio_snapshot_mirrors_the_observable_run_state() {
        let mut state = GameState {
            powered: true,
            screen: Screen::Oracle,
            screen_ticks: 12,
            questions_loading: true,
            oracle_data: 6,
            oracle_bug_hits: 1,
            ..GameState::default()
        };
        state.held.insert(Button::Left);
        state.held.insert(Button::A);
        state.quiz = Some(QuizRun {
            question: 7,
            completed_batches: 1,
            selected: 2,
            hearts: 1,
            score: 900,
            level: 4,
            streak: 3,
            leveled_up: false,
            feedback: Some((false, 10)),
            lens_woke: Some((Concept::Tradeoff, 2)),
            ..QuizRun::new()
        });
        let snapshot = audio_snapshot(&state);
        assert_eq!(snapshot.scene, AudioScene::Oracle);
        assert_eq!(snapshot.held, pad::LEFT | pad::A);
        assert_eq!((snapshot.data_stage, snapshot.breach_stage), (2, 1));
        assert_eq!(snapshot.questions, QuestionStatus::Writing);
        assert_eq!(snapshot.tier, Tier::OracleBound);
        assert_eq!(
            snapshot.run,
            Some(RunAudio {
                question: 7,
                selected: 2,
                phase: AnswerPhase::Wrong,
                hearts: 1,
                multiplier: 2,
                insight: 2,
                level: 4,
                completed_batches: 1,
                redeemed: false,
                lens_woke: Some(2),
                leave_armed: false,
            })
        );

        state.powered = false;
        state.screen = Screen::Off;
        state.quiz = None;
        let off = audio_snapshot(&state);
        assert_eq!(
            (off.powered, off.scene, off.tier),
            (false, AudioScene::Off, Tier::Initiate)
        );
    }

    #[test]
    fn oracle_opening_soundtrack_grows_from_archival_ticks_to_the_full_cadence() {
        let mut engine = GameEngine::new();
        issue(
            &mut engine,
            EngineCommand::Cartridge(Some(oracle_template_cartridge())),
        );
        issue(&mut engine, EngineCommand::Power(true));
        issue(&mut engine, EngineCommand::BootComplete);
        assert_eq!(engine.screen(), Screen::Copyright);
        let _ = engine.take_audio();

        // Copyright, then the five story scenes, then a breath of title.
        let mut sections = Vec::new();
        for (expected, ticks) in [
            (Screen::Copyright, 180),
            (Screen::OpeningFanfare, 96),
            (Screen::OpeningFanfare, 66),
            (Screen::OpeningFanfare, 66),
            (Screen::OpeningFanfare, 66),
            (Screen::OpeningFanfare, 66),
        ] {
            assert_eq!(engine.screen(), expected);
            for _ in 0..ticks {
                engine.update();
            }
            sections.push(engine.take_audio().notes);
        }
        assert_eq!(engine.screen(), Screen::Title);
        for _ in 0..120 {
            engine.update();
        }
        sections.push(engine.take_audio().notes);

        let voices = |notes: &[audio::Note]| {
            notes
                .iter()
                .filter(|note| !note.is_cut())
                .map(|note| note.voice)
                .collect::<HashSet<_>>()
                .len()
        };
        let peak = |notes: &[audio::Note]| notes.iter().map(|note| note.volume).max().unwrap_or(0);
        assert_eq!(voices(&sections[0]), 1, "the chronicle keeps near silence");
        assert!(peak(&sections[0]) <= 4);
        let story = sections[1..6]
            .iter()
            .map(|notes| voices(notes))
            .collect::<Vec<_>>();
        assert_eq!(story[0], 1, "the source ember is a single pulse");
        assert!(story.windows(2).all(|pair| pair[0] <= pair[1]), "{story:?}");
        assert_eq!(story[4], 4, "the Oracle crescendo is the full arrangement");
        assert!(sections[1..5]
            .iter()
            .all(|notes| peak(notes) < peak(&sections[5])));
        assert!(
            sections[6]
                .iter()
                .all(|note| note.volume <= audio::AMBIENCE_MAX_VOLUME),
            "the title settles into its restrained loop"
        );

        if let Ok(directory) = std::env::var("CQA_AUDIO_DUMP_DIR") {
            let notes = sections.concat();
            std::fs::create_dir_all(&directory).expect("dump directory should be writable");
            std::fs::write(
                std::path::Path::new(&directory).join("oracle-opening.json"),
                serde_json::to_string(&notes).expect("notes should serialize"),
            )
            .expect("dump should be writable");
        }
    }

    #[test]
    fn scripted_play_produces_identical_note_streams() {
        let play = || {
            let mut engine = playing_quiz_engine();
            issue(
                &mut engine,
                EngineCommand::Input {
                    button: Button::A,
                    pressed: true,
                },
            );
            for _ in 0..60 {
                engine.update();
            }
            engine.take_audio()
        };
        let first = play();
        assert!(first.notes.iter().filter(|note| !note.is_cut()).count() > 10);
        assert_eq!(first, play());
    }

    #[test]
    fn the_codex_turns_pages_audibly_and_keeps_reading_quiet() {
        use audio::Cue;
        let mut engine = quiz_menu_engine(journal_cartridge());
        let _ = tap_cues(&mut engine, Button::Down);
        assert_eq!(tap_cues(&mut engine, Button::A)[0], Some(Cue::Confirm));
        assert_eq!(engine.screen(), Screen::Codex);
        assert_eq!(
            tap_cues(&mut engine, Button::Right)[0],
            Some(Cue::PageTurn(1))
        );
        assert_eq!(
            tap_cues(&mut engine, Button::Left)[0],
            Some(Cue::PageTurn(0))
        );
        assert_eq!(
            tap_cues(&mut engine, Button::A)[0],
            Some(Cue::Unavailable),
            "A is inactive on the mastery page"
        );
        let _ = tap_cues(&mut engine, Button::Left);
        let _ = tap_cues(&mut engine, Button::Left);
        assert!(game_state(&engine).codex_lesson().unwrap().1.outstanding);
        assert_eq!(
            tap_cues(&mut engine, Button::A)[0],
            Some(Cue::QuestionReveal),
            "revealing a sealed answer has its own cue"
        );
        assert_eq!(
            tap_cues(&mut engine, Button::A)[0],
            Some(Cue::Unavailable),
            "a revealed page has nothing more for A to do"
        );
        for _ in 0..120 {
            engine.update();
            assert_eq!(last_cue(&engine), None, "no ambience under reading");
        }
        assert_eq!(tap_cues(&mut engine, Button::B)[0], Some(Cue::Cancel));
        assert_eq!(engine.screen(), Screen::QuizMenu);
    }

    #[test]
    fn leaving_warns_once_and_redemption_has_its_own_cadence() {
        use audio::Cue;
        let mut engine = batch_quiz_engine(QUESTION_BATCH_SIZE);
        assert_eq!(tap_cues(&mut engine, Button::B)[0], Some(Cue::LeaveWarning));
        assert_eq!(engine.screen(), Screen::Quiz, "one B only arms the leave");
        for _ in 0..QUIZ_LEAVE_CONFIRM_TICKS {
            engine.update();
        }

        commit(&mut engine, false);
        finish_lesson(&mut engine);
        for _ in 0..RETRY_GAP {
            commit(&mut engine, true);
            finish_lesson(&mut engine);
        }
        assert_eq!(current_question(&engine).review, Review::InSession);
        focus_choice(&mut engine, true);
        assert_eq!(tap_cues(&mut engine, Button::A)[0], Some(Cue::Redeemed));
    }
}
